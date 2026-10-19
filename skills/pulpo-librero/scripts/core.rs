// El lector canónico se copia entero aunque este scraper use sólo parte.
#![allow(dead_code)]

use std::env;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

include!("json_api.rs");

const ENCABEZADO: &str = "id\ttitle\tdoi\tyear\tis_oa\toa_status\tlicense\tversion\tlanding_url\tpdf_url\tpdf_urls\tabstract\tpublished\tcited_by";

// Sin la barra final la API responde 301 hacia la ruta con barra.
const BASE: &str = "https://api.core.ac.uk/v3/search/works/";

// Una página de 10 obras tardó 50 s en responder (2026-10-19) y una de 25 no
// llegó en 60 s; de ahí la página chica y el --max-time más largo que el de
// los otros scrapers.
const POR_PAGINA: usize = 10;

// El filtro de --desde se aplica en el cliente y puede descartar páginas
// enteras; este tope evita recorrer los millones de aciertos de una consulta.
const MAX_PAGINAS: usize = 8;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) != Some("search") {
        usage();
    }
    let mut query = None;
    let mut out = None;
    let mut desde = None;
    let mut max = 25usize;
    let mut orden = "relevancia".to_string();
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
            "--mailto" if i + 1 < args.len() => {
                eprintln!("advertencia: CORE no tiene polite pool; --mailto se ignora");
                i += 2;
            }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    let query = query.unwrap_or_else(|| { eprintln!("error: falta --query"); std::process::exit(2); }).trim().to_string();
    if query.is_empty() { eprintln!("error: --query está vacío"); std::process::exit(2); }
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    if max == 0 { eprintln!("error: --max debe ser mayor que 0"); std::process::exit(2); }
    // sort=citationCount:desc responde 500 (sondeo del 2026-10-19), así que no hay orden por citas.
    let sort = match orden.as_str() {
        "fecha" => Some("publishedDate:desc"),
        "relevancia" => None,
        otro => { eprintln!("error: --orden {otro} no es fecha ni relevancia"); std::process::exit(2); }
    };
    if let Some(fecha) = &desde {
        if !es_fecha(fecha) { eprintln!("error: --desde {fecha} no tiene la forma YYYY-MM-DD"); std::process::exit(2); }
    }
    let hoy = hoy();
    // Un rango sobre publishedDate o con [A TO B] dentro de q responde 500; con
    // yearPublished y comparaciones sueltas funciona. El corte fino por día se
    // hace en el cliente. La cota superior hace falta porque hay obras fechadas
    // en 2028 y 2029 que encabezan el orden por fecha.
    let mut consulta = format!("({query})");
    if let Some(fecha) = &desde {
        consulta.push_str(&format!(" AND yearPublished>={}", &fecha[0..4]));
    }
    let acotar = desde.is_some() || sort.is_some();
    if acotar {
        consulta.push_str(&format!(" AND yearPublished<={}", &hoy[0..4]));
    }
    let raw_path = format!("{out}.raw.jsonl");
    let parcial = format!("{out}.raw.part");
    let _ = fs::remove_file(&raw_path);
    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "{ENCABEZADO}").unwrap();
    let mut count = 0;
    let mut fuera_de_rango = 0;
    let mut repetidas = 0;
    let mut vistos: Vec<String> = Vec::new();
    let mut paginas = 0;
    let mut offset = 0usize;
    while count < max && paginas < MAX_PAGINAS {
        let mut url = format!("{BASE}?q={}&limit={POR_PAGINA}&offset={offset}", encode(&consulta));
        if let Some(sort) = sort { url.push_str("&sort="); url.push_str(sort); }
        if paginas > 0 { thread::sleep(Duration::from_secs(3)); }
        let raw = match consultar(&url, &parcial) {
            Ok(raw) => raw,
            Err(err) => { eprintln!("error: {err}"); std::process::exit(1); }
        };
        paginas += 1;
        if let Err(err) = guardar_crudo(&raw_path, &raw) { eprintln!("error: {err}"); std::process::exit(1); }
        let _ = fs::remove_file(&parcial);
        let results = match json_array(&raw, "results") {
            Some(results) => results,
            None => { eprintln!("error: la respuesta de CORE no trae 'results'"); std::process::exit(1); }
        };
        let obras = json_entries(results);
        if obras.is_empty() { break; }
        for obra in &obras {
            if count >= max { break; }
            let Some(fila) = fila_tsv(obra) else { continue };
            if acotar {
                let published = fila.published.as_str();
                let antes = desde.as_deref().is_some_and(|fecha| published < fecha);
                if published.is_empty() || antes || published > hoy.as_str() {
                    fuera_de_rango += 1;
                    continue;
                }
            }
            if fila.claves.iter().any(|clave| vistos.contains(clave)) {
                repetidas += 1;
                continue;
            }
            vistos.extend(fila.claves);
            writeln!(file, "{}", fila.linea).unwrap();
            count += 1;
        }
        offset += obras.len();
        let total = json_number(&raw, "totalHits").and_then(|total| total.parse::<usize>().ok()).unwrap_or(0);
        if offset >= total { break; }
    }
    if count < max && paginas == MAX_PAGINAS {
        eprintln!("advertencia: se alcanzó el tope de {MAX_PAGINAS} páginas antes de reunir {max} obras");
    }
    println!("core: {count} obras extraídas en {paginas} página(s); {fuera_de_rango} fuera de rango de fecha; {repetidas} repetidas; crudo={raw_path}; resultados={out}");
}

fn usage() -> ! {
    eprintln!("uso: core search --query TEXTO --out RESULTADOS.tsv [--desde YYYY-MM-DD] [--max N] [--orden relevancia|fecha]");
    std::process::exit(2);
}

fn consultar(url: &str, destino: &str) -> Result<String, String> {
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--connect-timeout", "20", "--max-time", "120", "--user-agent", "pulpo-librero/0.1", "--write-out", "%{http_code}", "--output"])
        .arg(destino)
        .arg(url)
        .stderr(Stdio::inherit())
        .output();
    match result {
        Ok(salida) if salida.status.success() => {}
        Ok(salida) => {
            let status = String::from_utf8_lossy(&salida.stdout).trim().to_string();
            // CORE responde 500 con un mensaje genérico ante la sintaxis de q o
            // el sort que no admite, no ante una caída.
            if status == "500" { return Err("CORE respondió 500; suele indicar sintaxis no admitida en --query (rangos [A TO B], _exists_, fechas completas)".to_string()); }
            if status == "000" { return Err("CORE no respondió (tiempo agotado o red caída); reintenta más tarde".to_string()); }
            if status == "429" { return Err("CORE respondió 429 (límite de peticiones); espera un minuto antes de reintentar".to_string()); }
            return Err(format!("CORE no respondió correctamente (HTTP {status})"));
        }
        Err(err) => return Err(format!("no se pudo ejecutar curl: {err}")),
    }
    fs::read_to_string(destino).map_err(|err| format!("no se pudo leer {destino}: {err}"))
}

// Una página por línea, igual que el resolver de OpenAlex.
fn guardar_crudo(destino: &str, raw: &str) -> Result<(), String> {
    let mut archivo = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(destino)
        .map_err(|err| format!("no se pudo abrir {destino}: {err}"))?;
    writeln!(archivo, "{}", raw.replace('\n', "").replace('\r', "")).map_err(|err| format!("no se pudo escribir {destino}: {err}"))
}

struct Fila {
    linea: String,
    published: String,
    claves: Vec<String>,
}

fn fila_tsv(obra: &str) -> Option<Fila> {
    let id = json_number(obra, "id").or_else(|| json_string(obra, "id")).unwrap_or_default();
    let title = normalize(&json_string(obra, "title").unwrap_or_default());
    if id.is_empty() || title.is_empty() {
        return None;
    }
    let doi = normalizar_doi(&json_string(obra, "doi").unwrap_or_default());
    let fecha = json_string(obra, "publishedDate").unwrap_or_default();
    let published = match fecha.get(0..10) {
        Some(dia) => dia.to_string(),
        None => json_number(obra, "yearPublished").unwrap_or_default(),
    };
    let year = published.get(0..4).unwrap_or("").to_string();
    let download = json_string(obra, "downloadUrl").unwrap_or_default();
    let urls = pdf_urls(obra, &download);
    // CORE solo indexa como texto completo lo que es de lectura abierta, pero no
    // declara acceso por obra: sin archivo no se afirma nada.
    let (is_oa, oa_status) = if urls.is_empty() { ("", "desconocido") } else { ("true", "abierto") };
    let landing_url = enlace(obra, "display").unwrap_or_else(|| format!("https://core.ac.uk/works/{id}"));
    // citationCount vale 0 en todas las obras medidas. Un 0 escrito acá pisaría
    // en el catálogo el conteo real que trajo otra fuente, así que solo pasa si
    // es positivo.
    let cited_by = json_number(obra, "citationCount").filter(|valor| valor.parse::<u64>().is_ok_and(|n| n > 0)).unwrap_or_default();
    let mut claves = Vec::new();
    if !doi.is_empty() { claves.push(format!("doi:{doi}")); }
    if !download.is_empty() { claves.push(download.clone()); }
    let linea = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t\t\t{}\t{}\t{}\t{}\t{}\t{}",
        clean(&format!("core:{id}")),
        clean(&title),
        clean(&doi),
        clean(&year),
        is_oa,
        oa_status,
        clean(&landing_url),
        clean(urls.first().map(String::as_str).unwrap_or("")),
        clean(&urls.join(";")),
        clean(&normalize(&json_string(obra, "abstract").unwrap_or_default())),
        clean(&published),
        cited_by,
    );
    Some(Fila { linea, published, claves })
}

// Las URL del repositorio de origen van antes que la copia de core.ac.uk,
// que en el sondeo respondió con un challenge de Cloudflare.
fn pdf_urls(obra: &str, download: &str) -> Vec<String> {
    let mut propias = Vec::new();
    let mut ajenas = Vec::new();
    let origen = json_array(obra, "sourceFulltextUrls").map(json_texts).unwrap_or_default();
    for url in origen.into_iter().chain(std::iter::once(download.to_string())) {
        if !url.starts_with("http") { continue; }
        let destino = if url.contains("://core.ac.uk/") { &mut propias } else { &mut ajenas };
        if !destino.contains(&url) { destino.push(url); }
    }
    ajenas.extend(propias);
    ajenas
}

fn enlace(obra: &str, tipo: &str) -> Option<String> {
    let links = json_array(obra, "links")?;
    json_entries(links).into_iter().find(|link| json_string(link, "type").as_deref() == Some(tipo)).and_then(|link| json_string(link, "url"))
}

fn normalizar_doi(valor: &str) -> String {
    valor
        .trim()
        .to_ascii_lowercase()
        .trim_start_matches("https://doi.org/")
        .trim_start_matches("http://doi.org/")
        .trim_start_matches("doi:")
        .to_string()
}

fn es_fecha(valor: &str) -> bool {
    let bytes = valor.as_bytes();
    bytes.len() == 10 && bytes.iter().enumerate().all(|(pos, b)| if pos == 4 || pos == 7 { *b == b'-' } else { b.is_ascii_digit() })
}

// Fecha UTC de hoy sin dependencias: conversión de días desde 1970 al
// calendario civil (algoritmo de Howard Hinnant).
fn hoy() -> String {
    let dias = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64 / 86_400 + 719_468;
    let era = dias.div_euclid(146_097);
    let dia_era = dias - era * 146_097;
    let anio_era = (dia_era - dia_era / 1460 + dia_era / 36_524 - dia_era / 146_096) / 365;
    let dia_anio = dia_era - (365 * anio_era + anio_era / 4 - anio_era / 100);
    let mp = (5 * dia_anio + 2) / 153;
    let dia = dia_anio - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let anio = anio_era + era * 400 + if mes <= 2 { 1 } else { 0 };
    format!("{anio:04}-{mes:02}-{dia:02}")
}

fn normalize(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
