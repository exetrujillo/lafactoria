// El lector canónico se copia entero aunque este scraper use sólo parte.
#![allow(dead_code)]

use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

include!("json_api.rs");

const ENCABEZADO: &str = "openalex_id\ttitle\tdoi\tyear\tis_oa\toa_status\tlicense\tversion\tlanding_url\tpdf_url\tpdf_urls\tabstract";

// OpenAlex acepta hasta 50 DOI por consulta separados con '|'.
const DOIS_POR_TANDA: usize = 50;

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("search") => run_search(&args),
        Some("resolver") => run_resolver(&args[2..]),
        _ => usage(),
    }
}

fn usage() -> ! {
    eprintln!("uso: openalex search --query TEXTO --out RESULTADOS.tsv [--title] [--per-page N] [--mailto EMAIL]");
    eprintln!("     openalex resolver --catalogo CATALOGO.tsv --out RESULTADOS.tsv [--solo-relevante] [--max N] [--mailto EMAIL]");
    std::process::exit(2);
}

fn run_search(args: &[String]) {
    if args.len() < 6 {
        usage();
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
    let raw = match consultar(&url, &raw_path) {
        Ok(raw) => raw,
        Err(err) => { eprintln!("error: {err}"); std::process::exit(1); }
    };
    let results = match json_array(&raw, "results") {
        Some(results) => results,
        None => { eprintln!("error: la respuesta de OpenAlex no trae 'results'"); std::process::exit(1); }
    };
    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "{ENCABEZADO}").unwrap();
    let mut count = 0;
    for obra in json_entries(results) {
        if let Some(fila) = fila_tsv(obra) {
            writeln!(file, "{fila}").unwrap();
            count += 1;
        }
    }
    println!("openalex: {count} obras extraídas; crudo={raw_path}; resultados={out}");
}

// El puente entre el catálogo de descubrimiento y la descarga: toma los DOI
// que todavía no tienen ubicación y pregunta por sus copias abiertas. No
// descarga ni decide relevancia; la salida vuelve al catálogo con
// `pulpo buscar`, que fusiona por identidad y conserva las decisiones ya
// tomadas.
fn run_resolver(args: &[String]) {
    let mut catalogo = None;
    let mut out = None;
    let mut mailto = None;
    let mut max = 200usize;
    let mut solo_relevante = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--catalogo" if i + 1 < args.len() => { catalogo = Some(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            "--mailto" if i + 1 < args.len() => { mailto = Some(args[i + 1].clone()); i += 2; }
            "--max" if i + 1 < args.len() => {
                max = args[i + 1].parse().unwrap_or_else(|_| { eprintln!("error: --max no es entero"); std::process::exit(2); });
                i += 2;
            }
            "--solo-relevante" => { solo_relevante = true; i += 1; }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    let catalogo = catalogo.unwrap_or_else(|| { eprintln!("error: falta --catalogo"); std::process::exit(2); });
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    let pendientes = match dois_pendientes(&catalogo, solo_relevante, max) {
        Ok(pendientes) => pendientes,
        Err(err) => { eprintln!("error: {err}"); std::process::exit(1); }
    };
    if pendientes.is_empty() {
        eprintln!("advertencia: no hay filas con DOI y sin pdf_url que resolver");
    }
    let raw_path = format!("{out}.raw.jsonl");
    let _ = fs::remove_file(&raw_path);
    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "{ENCABEZADO}").unwrap();
    let mut resueltos: Vec<String> = Vec::new();
    let mut con_pdf = 0;
    let mut tandas = 0;
    for tanda in pendientes.chunks(DOIS_POR_TANDA) {
        let filtro = tanda.iter().map(|doi| encode(doi)).collect::<Vec<_>>().join("%7C");
        let mut url = format!("https://api.openalex.org/works?filter=doi:{filtro}&per-page={DOIS_POR_TANDA}");
        if let Some(email) = &mailto { url.push_str("&mailto="); url.push_str(&encode(email)); }
        let parcial = format!("{out}.raw.part");
        let raw = match consultar(&url, &parcial) {
            Ok(raw) => raw,
            Err(err) => { eprintln!("error: {err}"); std::process::exit(1); }
        };
        tandas += 1;
        if let Err(err) = guardar_crudo(&raw_path, &raw) {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
        let _ = fs::remove_file(&parcial);
        let results = match json_array(&raw, "results") {
            Some(results) => results,
            None => { eprintln!("error: la respuesta de OpenAlex no trae 'results'"); std::process::exit(1); }
        };
        for obra in json_entries(results) {
            let Some(fila) = fila_tsv(obra) else { continue };
            resueltos.push(normalizar_doi(&json_string(obra, "doi").unwrap_or_default()));
            if !fila.split('\t').nth(9).unwrap_or("").is_empty() {
                con_pdf += 1;
            }
            writeln!(file, "{fila}").unwrap();
        }
    }
    let sin_respuesta: Vec<&String> = pendientes.iter().filter(|doi| !resueltos.contains(doi)).collect();
    println!(
        "openalex resolver: {} DOI consultados en {tandas} tanda(s); {} con respuesta; {con_pdf} con pdf_url; crudo={raw_path}; resultados={out}",
        pendientes.len(),
        resueltos.len()
    );
    if !sin_respuesta.is_empty() {
        let muestra: Vec<&str> = sin_respuesta.iter().take(10).map(|doi| doi.as_str()).collect();
        eprintln!("sin respuesta ({}): {}{}", sin_respuesta.len(), muestra.join(", "), if sin_respuesta.len() > 10 { ", ..." } else { "" });
    }
}

fn dois_pendientes(catalogo: &str, solo_relevante: bool, max: usize) -> Result<Vec<String>, String> {
    let contenido = fs::read_to_string(catalogo).map_err(|err| format!("no se pudo leer {catalogo}: {err}"))?;
    let mut lineas = contenido.lines();
    let encabezado: Vec<&str> = lineas.next().ok_or_else(|| format!("{catalogo} está vacío"))?.split('\t').collect();
    let columna = |nombre: &str| encabezado.iter().position(|valor| *valor == nombre);
    let doi = columna("doi").ok_or_else(|| format!("{catalogo} no tiene columna 'doi'"))?;
    let pdf_url = columna("pdf_url").ok_or_else(|| format!("{catalogo} no tiene columna 'pdf_url'"))?;
    let decision = columna("decision");
    let mut dois = Vec::new();
    for linea in lineas {
        if linea.trim().is_empty() || linea.starts_with('#') {
            continue;
        }
        let campos: Vec<&str> = linea.split('\t').collect();
        let valor = normalizar_doi(campos.get(doi).unwrap_or(&""));
        if valor.is_empty() || !campos.get(pdf_url).unwrap_or(&"").is_empty() || dois.contains(&valor) {
            continue;
        }
        if solo_relevante && decision.and_then(|pos| campos.get(pos)).copied() != Some("relevante") {
            continue;
        }
        dois.push(valor);
        if dois.len() >= max {
            break;
        }
    }
    Ok(dois)
}

fn consultar(url: &str, destino: &str) -> Result<String, String> {
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--output"])
        .arg(destino)
        .arg(url)
        .status();
    thread::sleep(Duration::from_millis(1100));
    if !matches!(result, Ok(status) if status.success()) {
        return Err("OpenAlex no respondió correctamente".to_string());
    }
    fs::read_to_string(destino).map_err(|err| format!("no se pudo leer {destino}: {err}"))
}

// Una respuesta por línea: el crudo de N consultas no entra en un solo JSON.
// Los saltos de línea sobran porque JSON los escapa dentro de los strings.
fn guardar_crudo(destino: &str, raw: &str) -> Result<(), String> {
    let mut archivo = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(destino)
        .map_err(|err| format!("no se pudo abrir {destino}: {err}"))?;
    writeln!(archivo, "{}", raw.replace('\n', "").replace('\r', "")).map_err(|err| format!("no se pudo escribir {destino}: {err}"))
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
