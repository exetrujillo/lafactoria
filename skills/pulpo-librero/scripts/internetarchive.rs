// El lector canónico se copia entero aunque este scraper use sólo parte.
#![allow(dead_code)]

use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

include!("json_api.rs");

const FORMATOS_PDF: [&str; 2] = ["Text PDF", "Additional Text PDF"];

const ENCABEZADO: &str = "id\ttitle\tdoi\tyear\tis_oa\toa_status\tlicense\tversion\tlanding_url\tpdf_url\tpdf_urls\tabstract";

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) != Some("search") {
        usage();
    }
    let mut query = None;
    let mut ids: Vec<String> = Vec::new();
    let mut out = None;
    let mut rows = "25".to_string();
    let mut page = "1".to_string();
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--query" if i + 1 < args.len() => { query = Some(args[i + 1].clone()); i += 2; }
            "--id" if i + 1 < args.len() => { ids.push(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            "--rows" if i + 1 < args.len() => { rows = args[i + 1].clone(); i += 2; }
            "--page" if i + 1 < args.len() => { page = args[i + 1].clone(); i += 2; }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    if query.is_some() == !ids.is_empty() { eprintln!("error: usa --query o al menos un --id, no ambos"); std::process::exit(2); }
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    let mut crudos = Vec::new();
    let identificadores = match query {
        Some(query) => {
            let raw_path = format!("{out}.raw.json");
            crudos.push(raw_path.clone());
            buscar(&query, &rows, &page, &raw_path)
        }
        None => ids,
    };
    let raw_path = format!("{out}.raw.jsonl");
    let _ = fs::remove_file(&raw_path);
    crudos.push(raw_path.clone());
    let parcial = format!("{out}.raw.part");
    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "{ENCABEZADO}").unwrap();
    let (mut abiertos, mut restringidos, mut sin_pdf, mut sin_metadata) = (0, 0, 0, 0);
    for identificador in &identificadores {
        let url = format!("https://archive.org/metadata/{}", encode(identificador));
        let raw = match consultar(&url, &parcial) {
            Ok(raw) => raw,
            Err(err) => { eprintln!("advertencia: {identificador}: {err}"); sin_metadata += 1; continue; }
        };
        if let Err(err) = guardar_crudo(&raw_path, &raw) {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
        let Some(meta) = json_object(&raw, "metadata") else {
            eprintln!("advertencia: {identificador}: la respuesta no trae 'metadata'");
            sin_metadata += 1;
            continue;
        };
        let (fila, estado) = fila_tsv(identificador, &raw, meta);
        match estado {
            "abierto" => abiertos += 1,
            "prestamo-controlado" => restringidos += 1,
            _ => sin_pdf += 1,
        }
        writeln!(file, "{fila}").unwrap();
    }
    let _ = fs::remove_file(&parcial);
    println!(
        "internetarchive: {} ítems; {abiertos} abiertos, {restringidos} préstamo controlado, {sin_pdf} sin pdf, {sin_metadata} sin metadata; crudo={}; resultados={out}",
        identificadores.len(),
        crudos.join(",")
    );
}

fn usage() -> ! {
    eprintln!("uso: internetarchive search (--query TEXTO | --id IDENTIFICADOR ...) --out RESULTADOS.tsv [--rows N] [--page N]");
    std::process::exit(2);
}

// El catálogo mezcla textos con audio y video, así que la restricción de
// mediatype no es opcional; se respeta la del usuario si ya la escribió.
fn buscar(query: &str, rows: &str, page: &str, destino: &str) -> Vec<String> {
    let consulta = if query.contains("mediatype") { query.to_string() } else { format!("{query} AND mediatype:texts") };
    let url = format!(
        "https://archive.org/advancedsearch.php?q={}&fl%5B%5D=identifier&rows={}&page={}&output=json",
        encode(&consulta),
        encode(rows),
        encode(page)
    );
    let raw = match consultar(&url, destino) {
        Ok(raw) => raw,
        Err(err) => { eprintln!("error: {err}"); std::process::exit(1); }
    };
    let docs = json_object(&raw, "response").and_then(|respuesta| json_array(respuesta, "docs"));
    let docs = match docs {
        Some(docs) => docs,
        None => { eprintln!("error: la respuesta de Internet Archive no trae 'response.docs'"); std::process::exit(1); }
    };
    json_entries(docs).into_iter().filter_map(|doc| json_string(doc, "identifier")).collect()
}

// Lo que separa una descarga abierta de un préstamo controlado está en la
// metadata del ítem: un libro prestado expone igual un "Text PDF" que parece
// descargable y responde 401. La fila se emite igual, sin pdf_url, para que el
// orquestador la descarte con un motivo en vez de que la obra desaparezca.
fn fila_tsv(identificador: &str, raw: &str, meta: &str) -> (String, &'static str) {
    let restringido = json_string(meta, "access-restricted-item").map(|valor| valor.eq_ignore_ascii_case("true")).unwrap_or(false)
        || json_texts_or_text(meta, "collection").iter().any(|coleccion| coleccion == "inlibrary" || coleccion == "printdisabled");
    let (estado, pdf_url) = if restringido {
        ("prestamo-controlado", String::new())
    } else {
        match texto_pdf(raw) {
            Some(nombre) => ("abierto", format!("https://archive.org/download/{}/{}", encode(identificador), encode(&nombre))),
            None => ("sin-pdf", String::new()),
        }
    };
    let fila = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t\t{}\t{}\t{}\t{}",
        clean(identificador),
        clean(&json_string(meta, "title").unwrap_or_else(|| identificador.to_string())),
        clean(&json_string(meta, "doi").unwrap_or_default()),
        anio(meta),
        estado == "abierto",
        estado,
        clean(&json_string(meta, "licenseurl").unwrap_or_default()),
        format!("https://archive.org/details/{}", encode(identificador)),
        clean(&pdf_url),
        clean(&pdf_url),
        clean(&json_texts_or_text(meta, "description").join(" ")),
    );
    (fila, estado)
}

// Sólo PDF con capa de texto y sin cifrar, en orden de preferencia. Las
// variantes cifradas (ACS, LCP) no se pueden abrir, y "JPEG-Compressed PDF" e
// "Image Container PDF" son el escaneo en imagen: pasarían la validación del
// descargador y quedarían indexados vacíos.
fn texto_pdf(raw: &str) -> Option<String> {
    let archivos = json_array(raw, "files")?;
    let entradas = json_entries(archivos);
    FORMATOS_PDF.iter().find_map(|formato| {
        entradas.iter().find_map(|archivo| match json_string(archivo, "format") {
            Some(actual) if actual == *formato => json_string(archivo, "name"),
            _ => None,
        })
    })
}

fn anio(meta: &str) -> String {
    if let Some(year) = json_string(meta, "year") {
        return clean(&year);
    }
    let inicio: String = json_string(meta, "date").unwrap_or_default().chars().take(4).collect();
    if inicio.len() == 4 && inicio.chars().all(|c| c.is_ascii_digit()) { inicio } else { String::new() }
}

fn consultar(url: &str, destino: &str) -> Result<String, String> {
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--output"])
        .arg(destino)
        .arg(url)
        .status();
    thread::sleep(Duration::from_millis(1100));
    if !matches!(result, Ok(status) if status.success()) {
        return Err("Internet Archive no respondió correctamente".to_string());
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

fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
