// El lector canónico se copia entero aunque este scraper use sólo parte.
#![allow(dead_code)]

use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

include!("json_api.rs");

const ENCABEZADO: &str = "id\ttitle\tdoi\tyear\tis_oa\toa_status\tlicense\tversion\tlanding_url\tpdf_url\tpdf_urls\tabstract\tpublished\tcited_by";
const CAMPOS: &str = "paperId,url,title,year,publicationDate,citationCount,isOpenAccess,openAccessPdf,externalIds,abstract";

// /paper/search/bulk no acepta limit: cada página trae hasta 1000 obras.
const OBRAS_POR_PAGINA: usize = 1000;

// Solo /paper/search/bulk responde sin clave. /paper/search (relevancia) dio
// 429 en todas las peticiones del sondeo del 2026-10-19, así que no hay orden
// por relevancia: el texto de bulk filtra, no ordena.
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 6 || args[1] != "search" {
        usage();
    }
    let mut query = None;
    let mut out = None;
    let mut desde = None;
    let mut max = 25usize;
    let mut orden = "citas".to_string();
    let mut min_citas = 0u64;
    let mut campo = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--query" if i + 1 < args.len() => { query = Some(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            "--desde" if i + 1 < args.len() => { desde = Some(args[i + 1].clone()); i += 2; }
            "--max" if i + 1 < args.len() => {
                max = args[i + 1].parse().unwrap_or_else(|_| { eprintln!("error: --max no es entero"); std::process::exit(2); });
                i += 2;
            }
            "--orden" if i + 1 < args.len() => { orden = args[i + 1].clone(); i += 2; }
            "--min-citas" if i + 1 < args.len() => {
                min_citas = args[i + 1].parse().unwrap_or_else(|_| { eprintln!("error: --min-citas no es entero"); std::process::exit(2); });
                i += 2;
            }
            "--campo" if i + 1 < args.len() => { campo = Some(args[i + 1].clone()); i += 2; }
            // Semantic Scholar no tiene polite pool. Se acepta para que el
            // orquestador pase los mismos flags a todos los scrapers, pero no se manda.
            "--mailto" if i + 1 < args.len() => { i += 2; }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    let query = query.filter(|valor| !valor.trim().is_empty()).unwrap_or_else(|| { eprintln!("error: falta --query o está vacío"); std::process::exit(2); });
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    if let Some(fecha) = &desde {
        if !es_fecha(fecha) { eprintln!("error: --desde {fecha} no tiene la forma YYYY-MM-DD"); std::process::exit(2); }
    }
    if max == 0 { eprintln!("error: --max debe ser mayor que cero"); std::process::exit(2); }
    let sort = match orden.as_str() {
        "citas" => "citationCount:desc",
        "fecha" => "publicationDate:desc",
        otro => { eprintln!("error: --orden {otro} no es citas ni fecha"); std::process::exit(2); }
    };
    let mut base = format!("https://api.semanticscholar.org/graph/v1/paper/search/bulk?query={}&fields={CAMPOS}&sort={sort}", encode(&query));
    if let Some(fecha) = &desde { base.push_str(&format!("&publicationDateOrYear={fecha}:")); }
    if min_citas > 0 { base.push_str(&format!("&minCitationCount={min_citas}")); }
    if let Some(campo) = &campo { base.push_str("&fieldsOfStudy="); base.push_str(&encode(campo)); }
    let raw_path = format!("{out}.raw.jsonl");
    let parcial = format!("{out}.raw.part");
    let _ = fs::remove_file(&raw_path);
    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "{ENCABEZADO}").unwrap();
    let mut count = 0;
    let mut paginas = 0;
    let mut token: Option<String> = None;
    let mut cortada = false;
    // El filtro local de fecha descarta las obras que el rango deja pasar sin
    // publicationDate; el tope evita recorrer el índice si descarta mucho.
    let max_paginas = max / OBRAS_POR_PAGINA + 3;
    while count < max && paginas < max_paginas {
        let url = match &token { Some(token) => format!("{base}&token={}", encode(token)), None => base.clone() };
        let raw = match consultar(&url, &parcial) {
            Ok(raw) => raw,
            Err(err) if paginas > 0 => { eprintln!("advertencia: {err}; se conservan las {count} obras ya extraídas"); cortada = true; break; }
            Err(err) => {
                // Un TSV con solo el encabezado se leería como una búsqueda sin resultados.
                let _ = fs::remove_file(&out);
                let _ = fs::remove_file(&parcial);
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        };
        paginas += 1;
        if let Err(err) = guardar_crudo(&raw_path, &raw) { eprintln!("error: {err}"); std::process::exit(1); }
        let _ = fs::remove_file(&parcial);
        let data = match json_array(&raw, "data") {
            Some(data) => data,
            None => break,
        };
        for obra in json_entries(data) {
            if count >= max { break; }
            if let Some(fila) = fila_tsv(obra, desde.as_deref()) {
                writeln!(file, "{fila}").unwrap();
                count += 1;
            }
        }
        token = json_string(&raw, "token");
        if token.is_none() { break; }
    }
    let corte = if cortada { format!(" (incompleto: falló la página {})", paginas + 1) } else { String::new() };
    println!("semantic-scholar: {count} obras extraídas en {paginas} página(s){corte}; crudo={raw_path}; resultados={out}");
}

fn usage() -> ! {
    eprintln!("uso: semantic-scholar search --query TEXTO --out RESULTADOS.tsv [--desde YYYY-MM-DD] [--max N] [--orden citas|fecha]");
    eprintln!("                            [--min-citas N] [--campo \"Computer Science,Psychology\"] [--mailto EMAIL]");
    std::process::exit(2);
}

// Sin clave el cupo es compartido y se agota sin aviso: el 429 no trae
// Retry-After. Se informa aparte para que nadie lo reintente en seguida, y
// curl va sin --retry porque reintentaría el 429 a los pocos segundos.
fn consultar(url: &str, destino: &str) -> Result<String, String> {
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--write-out", "%{http_code}", "--output"])
        .arg(destino)
        .arg(url)
        .output();
    thread::sleep(Duration::from_secs(3));
    match result {
        Err(err) => Err(format!("no se pudo ejecutar curl: {err}")),
        Ok(salida) if String::from_utf8_lossy(&salida.stdout).trim() == "429" => {
            Err("Semantic Scholar respondió 429 (cupo sin clave agotado); espera varios minutos antes de reintentar".to_string())
        }
        Ok(salida) if !salida.status.success() => {
            let detalle = String::from_utf8_lossy(&salida.stderr).trim().to_string();
            Err(format!("Semantic Scholar no respondió correctamente: {detalle}"))
        }
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

fn fila_tsv(obra: &str, desde: Option<&str>) -> Option<String> {
    let paper_id = json_string(obra, "paperId").unwrap_or_default();
    let title = json_string(obra, "title").unwrap_or_default();
    if paper_id.is_empty() || title.is_empty() {
        return None;
    }
    let published = json_string(obra, "publicationDate").unwrap_or_default();
    let year = json_number(obra, "year").unwrap_or_else(|| published.get(0..4).unwrap_or("").to_string());
    // Con publicationDateOrYear, una obra sin fecha entra por su año: un 2024
    // a secas puede ser anterior a --desde y se descarta.
    if let Some(desde) = desde {
        let posterior = if !published.is_empty() { published.as_str() >= desde } else { year.len() == 4 && year.as_str() > &desde[..4] };
        if !posterior {
            return None;
        }
    }
    let ids = json_object(obra, "externalIds").unwrap_or("");
    let acceso = json_object(obra, "openAccessPdf").unwrap_or("");
    let pdf_url = json_string(acceso, "url").unwrap_or_default();
    Some(format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        clean(&format!("semantic-scholar:{paper_id}")),
        clean(&title),
        clean(&json_string(ids, "DOI").unwrap_or_default()),
        clean(&year),
        json_bool(obra, "isOpenAccess").map(|valor| valor.to_string()).unwrap_or_default(),
        clean(&json_string(acceso, "status").unwrap_or_default()),
        clean(&json_string(acceso, "license").unwrap_or_default()),
        "",
        clean(&json_string(obra, "url").unwrap_or_default()),
        clean(&pdf_url),
        clean(&pdf_urls(&pdf_url, json_string(ids, "ArXiv")).join(";")),
        clean(&json_string(obra, "abstract").unwrap_or_default()),
        clean(&published),
        json_number(obra, "citationCount").unwrap_or_default(),
    ))
}

// openAccessPdf.url apunta a menudo a doi.org o a la editorial, que el sondeo
// vio devolver HTML; arxiv.org fue el único host que entregó PDF. Por eso el
// PDF de arXiv, armado con el identificador que declara la fuente como hace
// arxiv.rs y con su misma forma, va primero cuando existe.
fn pdf_urls(pdf_url: &str, arxiv: Option<String>) -> Vec<String> {
    let mut urls = Vec::new();
    if let Some(arxiv) = arxiv.filter(|valor| !valor.is_empty()) {
        urls.push(format!("https://arxiv.org/pdf/{arxiv}.pdf"));
    }
    if !pdf_url.is_empty() && !urls.iter().any(|url| url == pdf_url) {
        urls.push(pdf_url.to_string());
    }
    urls
}

fn es_fecha(valor: &str) -> bool {
    let bytes = valor.as_bytes();
    bytes.len() == 10 && bytes.iter().enumerate().all(|(pos, b)| if pos == 4 || pos == 7 { *b == b'-' } else { b.is_ascii_digit() })
}

fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
