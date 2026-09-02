// El lector canónico se copia entero aunque este scraper use sólo parte.
#![allow(dead_code)]

use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

include!("json_api.rs");

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 6 || args[1] != "search" {
        eprintln!("uso: openalex search --query TEXTO --out RESULTADOS.tsv [--title] [--per-page N] [--mailto EMAIL]");
        std::process::exit(2);
    }
    let mut query = None;
    let mut out = None;
    let mut per_page = "25".to_string();
    let mut mailto = None;
    let mut title_only = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--query" if i + 1 < args.len() => { query = Some(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            "--title" => { title_only = true; i += 1; }
            "--per-page" if i + 1 < args.len() => { per_page = args[i + 1].clone(); i += 2; }
            "--mailto" if i + 1 < args.len() => { mailto = Some(args[i + 1].clone()); i += 2; }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    let query = query.unwrap_or_else(|| { eprintln!("error: falta --query"); std::process::exit(2); });
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    let mut url = if title_only {
        format!("https://api.openalex.org/works?filter=title.search:{}&per-page={}", encode(&query), per_page)
    } else {
        format!("https://api.openalex.org/works?search={}&per-page={}", encode(&query), per_page)
    };
    if let Some(email) = mailto { url.push_str("&mailto="); url.push_str(&encode(&email)); }
    let raw_path = format!("{out}.raw.json");
    let result = Command::new("curl").args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--output"])
        .arg(&raw_path).arg(&url).status();
    thread::sleep(Duration::from_millis(1100));
    if !matches!(result, Ok(status) if status.success()) {
        eprintln!("error: OpenAlex no respondió correctamente");
        std::process::exit(1);
    }
    let raw = match fs::read_to_string(&raw_path) {
        Ok(raw) => raw,
        Err(err) => { eprintln!("error: no se pudo leer {raw_path}: {err}"); std::process::exit(1); }
    };
    let results = match json_array(&raw, "results") {
        Some(results) => results,
        None => { eprintln!("error: la respuesta de OpenAlex no trae 'results'"); std::process::exit(1); }
    };
    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "openalex_id\ttitle\tdoi\tyear\tis_oa\toa_status\tlicense\tversion\tlanding_url\tpdf_url\tpdf_urls\tabstract").unwrap();
    let mut count = 0;
    for obra in json_entries(results) {
        if let Some(fila) = fila_tsv(obra) {
            writeln!(file, "{fila}").unwrap();
            count += 1;
        }
    }
    println!("openalex: {count} obras extraídas; crudo={raw_path}; resultados={out}");
}

// Cada campo se lee en su propio ámbito. Buscarlos sueltos sobre la obra
// entera devuelve los de primary_location, que es la versión publicada y
// suele estar cerrada aunque la obra tenga una copia abierta en otro lado.
fn fila_tsv(obra: &str) -> Option<String> {
    let id = json_string(obra, "id").unwrap_or_default();
    let title = json_string(obra, "title").or_else(|| json_string(obra, "display_name")).unwrap_or_default();
    if id.is_empty() || title.is_empty() {
        return None;
    }
    let acceso = json_object(obra, "open_access").unwrap_or("");
    let abierta = json_object(obra, "best_oa_location");
    let publicada = json_object(obra, "primary_location");
    let ubicacion = abierta.or(publicada).unwrap_or("");
    let campo = |clave: &str| json_string(ubicacion, clave).unwrap_or_default();
    Some(format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        clean(&id),
        clean(&title),
        clean(&json_string(obra, "doi").unwrap_or_default()),
        json_number(obra, "publication_year").unwrap_or_default(),
        json_bool(acceso, "is_oa").map(|valor| valor.to_string()).unwrap_or_default(),
        clean(&json_string(acceso, "oa_status").unwrap_or_default()),
        clean(&campo("license")),
        clean(&campo("version")),
        clean(&campo("landing_page_url")),
        clean(&campo("pdf_url")),
        clean(&pdf_urls(obra, ubicacion).join(";")),
        clean(&abstract_text(obra)),
    ))
}

// La ubicación abierta va primero: es la que el descargador debe probar antes
// que el resto.
fn pdf_urls(obra: &str, ubicacion: &str) -> Vec<String> {
    let mut urls = Vec::new();
    if let Some(url) = json_string(ubicacion, "pdf_url") {
        urls.push(url);
    }
    if let Some(ubicaciones) = json_array(obra, "locations") {
        for otra in json_entries(ubicaciones) {
            if let Some(url) = json_string(otra, "pdf_url") {
                if !urls.contains(&url) {
                    urls.push(url);
                }
            }
        }
    }
    urls
}

// OpenAlex no entrega el abstract como texto sino como índice invertido:
// cada palabra apunta a las posiciones donde aparece.
fn abstract_text(obra: &str) -> String {
    let indice = match json_object(obra, "abstract_inverted_index") {
        Some(indice) => indice,
        None => return String::new(),
    };
    let bytes = indice.as_bytes();
    let mut palabras: Vec<(usize, String)> = Vec::new();
    let mut i = 1;
    while i < indice.len() {
        let (palabra, after) = match json_string_at(indice, i) {
            Some(valor) => valor,
            None => { i += 1; continue; }
        };
        let mut j = after;
        while j < bytes.len() && (bytes[j] as char).is_ascii_whitespace() { j += 1; }
        if j < bytes.len() && bytes[j] == b':' { j += 1; }
        while j < bytes.len() && (bytes[j] as char).is_ascii_whitespace() { j += 1; }
        let posiciones = match json_span(&indice[j..]) {
            Some(span) if span.starts_with('[') => span,
            _ => { i = j; continue; }
        };
        for numero in posiciones.trim_matches(|c| c == '[' || c == ']').split(',') {
            if let Ok(posicion) = numero.trim().parse::<usize>() {
                palabras.push((posicion, palabra.clone()));
            }
        }
        i = j + posiciones.len();
    }
    palabras.sort_by_key(|par| par.0);
    palabras.into_iter().map(|par| par.1).collect::<Vec<_>>().join(" ")
}

fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
