// El lector canónico se copia entero aunque este scraper use sólo parte.
#![allow(dead_code)]

use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

include!("json_api.rs");

const ENCABEZADO: &str = "id\ttitle\tdoi\tyear\tis_oa\toa_status\tlicense\tversion\tlanding_url\tpdf_url\tpdf_urls\tabstract\tpublished\tcited_by";

// La API sirve páginas fijas de 30 obras; el cursor es un desplazamiento.
const POR_PAGINA: usize = 30;

// bioRxiv abrió en 2013 y medRxiv en 2019: desde aquí el rango cubre todo.
const INICIO_SERVIDORES: &str = "2013-01-01";

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 || args[1] != "search" {
        usage();
    }
    let mut query = None;
    let mut exact_id = None;
    let mut out = None;
    let mut desde = None;
    let mut hasta = None;
    let mut max = 25usize;
    let mut max_paginas = 20usize;
    let mut servidor = "biorxiv".to_string();
    let mut categoria = None;
    let mut solo_publicadas = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--query" if i + 1 < args.len() => { query = Some(args[i + 1].clone()); i += 2; }
            "--id" if i + 1 < args.len() => { exact_id = Some(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            "--desde" if i + 1 < args.len() => { desde = Some(fecha_valida(&args[i + 1], "--desde")); i += 2; }
            "--hasta" if i + 1 < args.len() => { hasta = Some(fecha_valida(&args[i + 1], "--hasta")); i += 2; }
            "--max" if i + 1 < args.len() => { max = entero(&args[i + 1], "--max"); i += 2; }
            "--max-paginas" if i + 1 < args.len() => { max_paginas = entero(&args[i + 1], "--max-paginas"); i += 2; }
            "--servidor" if i + 1 < args.len() => { servidor = args[i + 1].clone(); i += 2; }
            "--categoria" if i + 1 < args.len() => { categoria = Some(args[i + 1].clone()); i += 2; }
            "--solo-publicadas" => { solo_publicadas = true; i += 1; }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    if query.is_some() == exact_id.is_some() { eprintln!("error: usa exactamente uno de --query o --id"); std::process::exit(2); }
    if servidor != "biorxiv" && servidor != "medrxiv" { eprintln!("error: --servidor debe ser biorxiv o medrxiv"); std::process::exit(2); }
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    let raw_path = format!("{out}.raw.jsonl");
    let _ = fs::remove_file(&raw_path);
    let parcial = format!("{out}.raw.part");

    if let Some(id) = exact_id {
        let doi = normalizar_doi(&id);
        if !doi.starts_with("10.") || !doi.bytes().all(|b| b.is_ascii_alphanumeric() || b"./-_".contains(&b)) { eprintln!("error: --id debe ser un DOI como 10.1101/2024.12.31.630767"); std::process::exit(2); }
        let url = format!("https://api.{servidor}.org/details/{servidor}/{doi}/na/json");
        let raw = consultar(&url, &parcial, &raw_path, false).unwrap_or_else(|err| { eprintln!("error: {err}"); std::process::exit(1); });
        let obras = coleccion(&raw).unwrap_or_else(|err| { eprintln!("error: {err}"); std::process::exit(1); });
        let mut file = crear(&out);
        let mut count = 0;
        // La API devuelve una entrada por versión, de la más antigua a la actual.
        if let Some(obra) = obras.last() {
            if let Some(fila) = fila_tsv(obra, &servidor) {
                writeln!(file, "{fila}").unwrap();
                count += 1;
            }
        }
        let _ = fs::remove_file(&parcial);
        println!("{servidor}: {count} obras extraídas; crudo={raw_path}; resultados={out}");
        return;
    }

    let query = query.unwrap();
    let alternativas = alternativas(&query);
    if alternativas.is_empty() { eprintln!("error: --query está vacío"); std::process::exit(2); }
    let desde = desde.unwrap_or_else(|| INICIO_SERVIDORES.to_string());
    let hasta = hasta.unwrap_or_else(hoy);
    if desde > hasta { eprintln!("error: --desde es posterior a --hasta"); std::process::exit(2); }
    let base = format!("https://api.{servidor}.org/details/{servidor}/{desde}/{hasta}");
    let filtro = categoria.as_ref().map(|valor| format!("?category={}", encode(&valor.to_lowercase()))).unwrap_or_default();

    // La API no busca por texto ni ordena: entrega el rango de la fecha más
    // antigua a la más nueva. Para empezar por lo reciente se lee el total en
    // la primera página y se recorre el cursor hacia atrás. La búsqueda es
    // local, sobre título y abstract.
    let primera = consultar(&format!("{base}/0{filtro}"), &parcial, &raw_path, true).unwrap_or_else(|err| { eprintln!("error: {err}"); std::process::exit(1); });
    let total = total_registros(&primera).unwrap_or_else(|err| { eprintln!("error: {err}"); std::process::exit(1); });
    let mut file = crear(&out);
    let mut vistos: HashSet<String> = HashSet::new();
    let mut count = 0;
    let mut revisados = 0;
    let mut paginas = 0;
    let mut fecha_mas_antigua = String::new();
    let mut completo = total == 0;
    let mut cursor = if total == 0 { None } else { Some((total - 1) / POR_PAGINA * POR_PAGINA) };
    while let Some(actual) = cursor {
        if count >= max || paginas >= max_paginas {
            break;
        }
        let raw = if actual == 0 {
            primera.clone()
        } else {
            consultar(&format!("{base}/{actual}{filtro}"), &parcial, &raw_path, true).unwrap_or_else(|err| { eprintln!("error: {err}"); std::process::exit(1); })
        };
        paginas += 1;
        let obras = coleccion(&raw).unwrap_or_else(|err| { eprintln!("error: {err}"); std::process::exit(1); });
        let mut cortado = false;
        for obra in obras.iter().rev() {
            revisados += 1;
            let fecha = json_string(obra, "date").unwrap_or_default();
            if !fecha.is_empty() { fecha_mas_antigua = fecha; }
            // Cada versión nueva es otra entrada; recorriendo hacia atrás, la
            // primera que aparece es la actual.
            let doi = normalizar_doi(&json_string(obra, "doi").unwrap_or_default());
            if doi.is_empty() || !vistos.insert(doi) { continue; }
            if solo_publicadas && !publicada(obra) { continue; }
            let texto = format!("{} {}", json_string(obra, "title").unwrap_or_default(), json_string(obra, "abstract").unwrap_or_default()).to_lowercase();
            if !alternativas.iter().any(|terminos| terminos.iter().all(|termino| texto.contains(termino.as_str()))) { continue; }
            if let Some(fila) = fila_tsv(obra, &servidor) {
                writeln!(file, "{fila}").unwrap();
                count += 1;
                if count >= max { cortado = true; break; }
            }
        }
        completo = actual == 0 && !cortado;
        cursor = actual.checked_sub(POR_PAGINA);
    }
    let _ = fs::remove_file(&parcial);
    let cubierto = if completo { "rango completo".to_string() } else { format!("rango cubierto hasta {fecha_mas_antigua}") };
    println!("{servidor}: {count} obras extraídas de {revisados} registros en {paginas} página(s) de {total}; {cubierto}; crudo={raw_path}; resultados={out}");
}

fn usage() -> ! {
    eprintln!("uso: biorxiv search (--query TEXTO | --id DOI) --out RESULTADOS.tsv [--desde YYYY-MM-DD] [--hasta YYYY-MM-DD] [--max N] [--max-paginas N] [--servidor biorxiv|medrxiv] [--categoria NOMBRE] [--solo-publicadas]");
    std::process::exit(2);
}

fn fecha_valida(valor: &str, flag: &str) -> String {
    let bytes = valor.as_bytes();
    let forma = bytes.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-' && bytes.iter().enumerate().all(|(pos, b)| pos == 4 || pos == 7 || b.is_ascii_digit());
    if !forma { eprintln!("error: {flag} debe tener la forma YYYY-MM-DD"); std::process::exit(2); }
    valor.to_string()
}

fn entero(valor: &str, flag: &str) -> usize {
    match valor.parse::<usize>() {
        Ok(numero) if numero > 0 => numero,
        _ => { eprintln!("error: {flag} debe ser un entero positivo"); std::process::exit(2); }
    }
}

// "a b OR c" pide (a y b) o c, sin distinguir mayúsculas.
fn alternativas(query: &str) -> Vec<Vec<String>> {
    query
        .split(" OR ")
        .map(|parte| parte.split_whitespace().map(|termino| termino.trim_matches('"').to_lowercase()).filter(|termino| !termino.is_empty()).collect::<Vec<_>>())
        .filter(|terminos| !terminos.is_empty())
        .collect()
}

fn crear(out: &str) -> fs::File {
    let mut file = fs::File::create(out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "{ENCABEZADO}").unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    file
}

fn consultar(url: &str, destino: &str, crudo: &str, pausa: bool) -> Result<String, String> {
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--output"])
        .arg(destino)
        .arg(url)
        .status();
    if pausa {
        thread::sleep(Duration::from_secs(3));
    }
    match result {
        Ok(status) if status.success() => {}
        Ok(_) => return Err("la API de bioRxiv/medRxiv no respondió correctamente".to_string()),
        Err(err) => return Err(format!("no se pudo ejecutar curl: {err}")),
    }
    let raw = fs::read_to_string(destino).map_err(|err| format!("no se pudo leer {destino}: {err}"))?;
    let mut archivo = fs::OpenOptions::new().create(true).append(true).open(crudo).map_err(|err| format!("no se pudo abrir {crudo}: {err}"))?;
    writeln!(archivo, "{}", raw.replace('\n', "").replace('\r', "")).map_err(|err| format!("no se pudo escribir {crudo}: {err}"))?;
    Ok(raw)
}

// Una consulta sin resultados trae status "no posts found" y ninguna colección.
fn coleccion(raw: &str) -> Result<Vec<&str>, String> {
    if let Some(obras) = json_array(raw, "collection") {
        return Ok(json_entries(obras));
    }
    match estado(raw) {
        Some(status) if status != "ok" => Ok(Vec::new()),
        _ => Err("la respuesta de la API no trae 'collection'".to_string()),
    }
}

fn estado(raw: &str) -> Option<String> {
    let mensajes = json_array(raw, "messages")?;
    let primero = *json_entries(mensajes).first()?;
    json_string(primero, "status")
}

// total llega como string ("248") o como número según el endpoint.
fn total_registros(raw: &str) -> Result<usize, String> {
    let mensajes = json_array(raw, "messages").ok_or("la respuesta de la API no trae 'messages'")?;
    let primero = *json_entries(mensajes).first().ok_or("la respuesta de la API trae 'messages' vacío")?;
    if let Some(total) = json_string(primero, "total").or_else(|| json_number(primero, "total")) {
        return total.parse::<usize>().map_err(|_| format!("total no numérico: {total}"));
    }
    match json_string(primero, "status") {
        Some(status) if status != "ok" => Ok(0),
        _ => Err("la respuesta de la API no trae 'total'".to_string()),
    }
}

fn publicada(obra: &str) -> bool {
    !matches!(json_string(obra, "published").as_deref(), None | Some("") | Some("NA"))
}

// La API no da URL de PDF: se arma con el DOI y la versión, como en el sitio.
// cc_no es la licencia por defecto del servidor: lectura libre sin licencia de
// reutilización, así que el PDF se ofrece pero la fila no se marca abierta.
fn fila_tsv(obra: &str, servidor: &str) -> Option<String> {
    let doi = normalizar_doi(&json_string(obra, "doi").unwrap_or_default());
    let title = json_string(obra, "title").unwrap_or_default();
    if doi.is_empty() || title.trim().is_empty() {
        return None;
    }
    let version = json_string(obra, "version").or_else(|| json_number(obra, "version")).unwrap_or_default();
    let fecha = json_string(obra, "date").unwrap_or_default();
    let license = json_string(obra, "license").unwrap_or_default();
    let abierta = license.starts_with("cc") && license != "cc_no";
    let (is_oa, oa_status) = if abierta { ("true", "abierto") } else { ("", "desconocido") };
    let sufijo = if version.is_empty() { String::new() } else { format!("v{version}") };
    let landing_url = format!("https://www.{servidor}.org/content/{doi}{sufijo}");
    let pdf_url = format!("{landing_url}.full.pdf");
    Some(format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t",
        clean(&format!("{servidor}:{doi}{sufijo}")),
        clean(&normalize(&title)),
        clean(&doi),
        clean(fecha.get(0..4).unwrap_or("")),
        is_oa,
        oa_status,
        clean(&license),
        clean(&sufijo),
        clean(&landing_url),
        clean(&pdf_url),
        clean(&pdf_url),
        clean(&normalize(&json_string(obra, "abstract").unwrap_or_default())),
        clean(&fecha),
    ))
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

// Fecha UTC de hoy sin dependencias (algoritmo civil de Howard Hinnant).
fn hoy() -> String {
    let segundos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let z = segundos / 86400 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let anio = yoe + era * 400 + if mes <= 2 { 1 } else { 0 };
    format!("{anio:04}-{mes:02}-{dia:02}")
}

fn normalize(value: &str) -> String { value.split_whitespace().collect::<Vec<_>>().join(" ") }
fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
