// El lector canónico se copia entero aunque este scraper use sólo parte.
#![allow(dead_code)]

use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

include!("json_api.rs");

const ENCABEZADO: &str = "id\ttitle\tdoi\tyear\tis_oa\toa_status\tlicense\tversion\tlanding_url\tpdf_url\tpdf_urls\tabstract\tpublished\tcited_by\tupvotes";

// La API rechaza con 400 un limit mayor que 100.
const POR_PAGINA: usize = 100;

// daily_papers no dice cuántas páginas hay y, con --desde, una página puede
// no aportar ninguna fila.
const MAX_PAGINAS: usize = 10;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 || args[1] != "search" {
        usage();
    }
    let mut query = None;
    let mut lista = None;
    let mut ids: Vec<String> = Vec::new();
    let mut out = None;
    let mut desde = None;
    let mut max = 25usize;
    let mut por_votos = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--query" if i + 1 < args.len() => { query = Some(args[i + 1].clone()); i += 2; }
            "--lista" if i + 1 < args.len() => { lista = Some(args[i + 1].clone()); i += 2; }
            "--id" if i + 1 < args.len() => { ids.push(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            "--desde" if i + 1 < args.len() => { desde = Some(args[i + 1].clone()); i += 2; }
            "--max" if i + 1 < args.len() => {
                max = args[i + 1].parse().unwrap_or_else(|_| { eprintln!("error: --max no es entero"); std::process::exit(2); });
                i += 2;
            }
            "--orden" if i + 1 < args.len() => {
                por_votos = match args[i + 1].as_str() {
                    "votos" => true,
                    "fuente" => false,
                    otro => { eprintln!("error: --orden acepta votos o fuente, no {otro}"); std::process::exit(2); }
                };
                i += 2;
            }
            // Hugging Face no tiene polite pool: se acepta por uniformidad con
            // los demás scrapers y no se envía.
            "--mailto" if i + 1 < args.len() => { i += 2; }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    let modos = query.is_some() as u8 + lista.is_some() as u8 + !ids.is_empty() as u8;
    if modos != 1 { eprintln!("error: usa exactamente uno de --query, --lista o --id"); std::process::exit(2); }
    if matches!(&query, Some(texto) if texto.trim().is_empty()) { eprintln!("error: --query está vacío"); std::process::exit(2); }
    if max == 0 { eprintln!("error: --max debe ser mayor que 0"); std::process::exit(2); }
    if let Some(fecha) = &desde {
        if !fecha_valida(fecha) { eprintln!("error: --desde debe tener la forma YYYY-MM-DD"); std::process::exit(2); }
    }
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    let raw_path = format!("{out}.raw.jsonl");
    let _ = fs::remove_file(&raw_path);
    let parcial = format!("{out}.raw.part");
    let mut obras: Vec<(String, i64)> = Vec::new();
    let mut vistos: Vec<String> = Vec::new();
    let mut peticiones = 0;

    if let Some(texto) = query {
        // La búsqueda ignora p y devuelve siempre las mismas obras, así que se
        // pide una sola página del tamaño máximo y --desde se aplica acá.
        let url = format!("https://huggingface.co/api/papers/search?q={}&limit={POR_PAGINA}", encode(&texto));
        let raw = pedir(&url, &parcial, &raw_path);
        peticiones += 1;
        recoger(&raw, desde.as_deref(), &mut vistos, &mut obras);
    } else if let Some(nombre) = lista {
        let orden = match nombre.as_str() {
            "tendencia" => "trending",
            "reciente" => "publishedAt",
            otro => { eprintln!("error: --lista acepta tendencia o reciente, no {otro}"); std::process::exit(2); }
        };
        for pagina in 0..MAX_PAGINAS {
            // La primera página va sin p, que es la forma medida: la cabecera
            // link de esa respuesta apunta a p=1 como siguiente.
            let mut url = format!("https://huggingface.co/api/daily_papers?sort={orden}&limit={POR_PAGINA}");
            if pagina > 0 { url.push_str(&format!("&p={pagina}")); }
            let raw = pedir(&url, &parcial, &raw_path);
            peticiones += 1;
            let antes = obras.len();
            let entradas = recoger(&raw, desde.as_deref(), &mut vistos, &mut obras);
            if obras.len() >= max || entradas < POR_PAGINA { break; }
            // Ordenadas por fecha, una página entera anterior a --desde
            // significa que las siguientes también lo son.
            if orden == "publishedAt" && obras.len() == antes { break; }
        }
    } else {
        for id in &ids {
            let url = format!("https://huggingface.co/api/papers/{}", encode(id));
            let raw = pedir(&url, &parcial, &raw_path);
            peticiones += 1;
            if let Some(obra) = fila_tsv(&raw, desde.as_deref()) {
                obras.push(obra);
            }
        }
    }
    let _ = fs::remove_file(&parcial);

    if por_votos {
        obras.sort_by(|a, b| b.1.cmp(&a.1));
    }
    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "{ENCABEZADO}").unwrap();
    let mut count = 0;
    for (fila, _) in obras.iter().take(max) {
        writeln!(file, "{fila}").unwrap();
        count += 1;
    }
    println!("huggingface-papers: {count} obras extraídas en {peticiones} petición(es); crudo={raw_path}; resultados={out}");
}

fn usage() -> ! {
    eprintln!("uso: huggingface-papers search (--query TEXTO | --lista tendencia|reciente | --id ARXIV_ID...) --out RESULTADOS.tsv [--desde YYYY-MM-DD] [--max N] [--orden votos|fuente] [--mailto EMAIL]");
    std::process::exit(2);
}

fn pedir(url: &str, parcial: &str, raw_path: &str) -> String {
    let raw = match consultar(url, parcial) {
        Ok(raw) => raw,
        Err(err) => { eprintln!("error: {err}"); std::process::exit(1); }
    };
    if let Err(err) = guardar_crudo(raw_path, &raw) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
    raw
}

fn consultar(url: &str, destino: &str) -> Result<String, String> {
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--retry-max-time", "120", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--output"])
        .arg(destino)
        .arg(url)
        .status();
    thread::sleep(Duration::from_secs(3));
    match result {
        Err(err) => Err(format!("no se pudo ejecutar curl: {err}")),
        Ok(status) if status.code() == Some(23) => Err(format!("no se pudo escribir {destino}")),
        Ok(status) if !status.success() => Err("Hugging Face no respondió correctamente".to_string()),
        Ok(_) => fs::read_to_string(destino).map_err(|err| format!("no se pudo leer {destino}: {err}")),
    }
}

// Una respuesta por línea: el crudo de N páginas no entra en un solo JSON.
fn guardar_crudo(destino: &str, raw: &str) -> Result<(), String> {
    let mut archivo = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(destino)
        .map_err(|err| format!("no se pudo abrir {destino}: {err}"))?;
    writeln!(archivo, "{}", raw.replace('\n', "").replace('\r', "")).map_err(|err| format!("no se pudo escribir {destino}: {err}"))
}

// Devuelve cuántas entradas traía la respuesta antes de filtrar, para que la
// paginación sepa si llegó al final.
fn recoger(raw: &str, desde: Option<&str>, vistos: &mut Vec<String>, obras: &mut Vec<(String, i64)>) -> usize {
    let texto = raw.trim_start();
    if !texto.starts_with('[') {
        eprintln!("error: la respuesta de Hugging Face no es una lista");
        std::process::exit(1);
    }
    let entradas = json_entries(texto);
    for entrada in &entradas {
        let paper = json_object(entrada, "paper").unwrap_or(entrada);
        let id = json_string(paper, "id").unwrap_or_default();
        if id.is_empty() || vistos.contains(&id) { continue; }
        vistos.push(id);
        if let Some(obra) = fila_tsv(paper, desde) {
            obras.push(obra);
        }
    }
    entradas.len()
}

// La búsqueda y daily_papers envuelven la obra en "paper"; papers/<id> la
// entrega suelta. Afuera también hay un publishedAt, pero es la fecha de envío
// a Daily Papers y no la de arXiv.
fn fila_tsv(paper: &str, desde: Option<&str>) -> Option<(String, i64)> {
    let id = json_string(paper, "id").unwrap_or_default();
    let title = normalize(&json_string(paper, "title").unwrap_or_default());
    if id.is_empty() || title.is_empty() {
        return None;
    }
    let published = json_string(paper, "publishedAt").unwrap_or_default().get(0..10).unwrap_or("").to_string();
    if let Some(fecha) = desde {
        if published.is_empty() || published.as_str() < fecha {
            return None;
        }
    }
    let votos = json_number(paper, "upvotes").unwrap_or_default();
    // Hugging Face no sirve el PDF. Sólo un id con forma de arXiv permite
    // armar la URL; con otro id la celda queda vacía antes que inventada.
    let pdf_url = if es_arxiv(&id) { format!("https://arxiv.org/pdf/{id}") } else { String::new() };
    let fila = format!(
        "{}\t{}\t\t{}\t\t\t\t\t{}\t{}\t{}\t{}\t{}\t\t{}",
        clean(&id),
        clean(&title),
        clean(published.get(0..4).unwrap_or("")),
        clean(&format!("https://huggingface.co/papers/{id}")),
        clean(&pdf_url),
        clean(&pdf_url),
        clean(&normalize(&json_string(paper, "summary").unwrap_or_default())),
        clean(&published),
        clean(&votos),
    );
    Some((fila, votos.parse().unwrap_or(0)))
}

fn es_arxiv(id: &str) -> bool {
    match id.split_once('.') {
        Some((prefijo, numero)) => prefijo.len() == 4 && prefijo.bytes().all(|b| b.is_ascii_digit()) && (4..=5).contains(&numero.len()) && numero.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

fn fecha_valida(fecha: &str) -> bool {
    let bytes = fecha.as_bytes();
    bytes.len() == 10 && bytes.iter().enumerate().all(|(i, b)| if i == 4 || i == 7 { *b == b'-' } else { b.is_ascii_digit() })
}

fn normalize(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
