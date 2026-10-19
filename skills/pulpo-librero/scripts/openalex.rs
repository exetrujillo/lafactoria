// El lector canónico se copia entero aunque este scraper use sólo parte.
#![allow(dead_code)]

use std::env;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

include!("json_api.rs");

const ENCABEZADO: &str = "openalex_id\ttitle\tdoi\tyear\tis_oa\toa_status\tlicense\tversion\tlanding_url\tpdf_url\tpdf_urls\tabstract\tpublished\tcited_by";

// select tiene que cubrir cada campo que lee fila_tsv; uno que falte deja la
// columna vacía sin error.
const CAMPOS: &str = "id,doi,title,display_name,publication_year,publication_date,cited_by_count,open_access,best_oa_location,primary_location,locations,abstract_inverted_index";

// La regla de la skill: al menos 3 s entre dos peticiones al mismo host.
const PAUSA_SEGUNDOS: u64 = 3;

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
    eprintln!("                    [--desde YYYY-MM-DD] [--orden citas|fecha|relevancia] [--tipo article|preprint|...] [--min-citas N]");
    eprintln!("     openalex resolver --catalogo CATALOGO.tsv --out RESULTADOS.tsv [--solo-relevante] [--max N] [--mailto EMAIL]");
    std::process::exit(2);
}

fn run_search(args: &[String]) {
    if args.len() < 6 {
        usage();
    }
    let mut query = None;
    let mut out = None;
    let mut per_page = 25u32;
    let mut mailto = None;
    let mut title_only = false;
    let mut desde = None;
    let mut orden = "relevancia".to_string();
    let mut tipo = None;
    let mut min_citas = 0u64;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--query" if i + 1 < args.len() => { query = Some(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            "--title" => { title_only = true; i += 1; }
            "--per-page" | "--max" if i + 1 < args.len() => {
                per_page = match args[i + 1].parse::<u32>() {
                    Ok(n) if (1..=200).contains(&n) => n,
                    _ => { eprintln!("error: --per-page/--max tiene que ser un entero entre 1 y 200"); std::process::exit(2); }
                };
                i += 2;
            }
            "--mailto" if i + 1 < args.len() => { mailto = Some(args[i + 1].clone()); i += 2; }
            "--desde" if i + 1 < args.len() => { desde = Some(args[i + 1].clone()); i += 2; }
            "--orden" if i + 1 < args.len() => { orden = args[i + 1].clone(); i += 2; }
            "--tipo" if i + 1 < args.len() => { tipo = Some(args[i + 1].clone()); i += 2; }
            "--min-citas" if i + 1 < args.len() => {
                min_citas = args[i + 1].parse().unwrap_or_else(|_| { eprintln!("error: --min-citas no es entero"); std::process::exit(2); });
                i += 2;
            }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    let query = query.unwrap_or_else(|| { eprintln!("error: falta --query"); std::process::exit(2); });
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    let sort = match orden.as_str() {
        "citas" => Some("cited_by_count:desc"),
        "fecha" => Some("publication_date:desc"),
        "relevancia" => None,
        otro => { eprintln!("error: --orden {otro} no es citas, fecha ni relevancia"); std::process::exit(2); }
    };
    if let Some(fecha) = &desde {
        if !es_fecha(fecha) { eprintln!("error: --desde {fecha} no es una fecha YYYY-MM-DD válida"); std::process::exit(2); }
    }
    let mut filtros = Vec::new();
    // Con search=, el sort reemplaza a la relevancia y entran obras de otras
    // áreas que solo mencionan alguna palabra. title_and_abstract.search
    // mantiene el tema (sondeo del 2026-10-19).
    let mut url = if title_only {
        filtros.push(format!("title.search:{}", encode(&query)));
        format!("https://api.openalex.org/works?per-page={per_page}")
    } else if sort.is_some() {
        filtros.push(format!("title_and_abstract.search:{}", encode(&query)));
        format!("https://api.openalex.org/works?per-page={per_page}")
    } else {
        format!("https://api.openalex.org/works?search={}&per-page={}", encode(&query), per_page)
    };
    if let Some(fecha) = &desde {
        filtros.push(format!("from_publication_date:{fecha}"));
    }
    // Hay registros con fecha futura (datasets de 2027 con 0 citas) que encabezan
    // el orden por fecha si no se acota por arriba.
    if desde.is_some() || sort == Some("publication_date:desc") {
        filtros.push(format!("to_publication_date:{}", hoy()));
    }
    if let Some(tipo) = &tipo {
        filtros.push(format!("type:{}", encode(tipo)));
    }
    if min_citas > 0 {
        filtros.push(format!("cited_by_count:%3E{}", min_citas - 1));
    }
    if !filtros.is_empty() { url.push_str("&filter="); url.push_str(&filtros.join(",")); }
    if let Some(sort) = sort { url.push_str("&sort="); url.push_str(sort); }
    url.push_str("&select="); url.push_str(CAMPOS);
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
    escribir_linea(&mut file, ENCABEZADO, &out);
    let mut count = 0;
    for obra in json_entries(results) {
        if let Some(fila) = fila_tsv(obra) {
            escribir_linea(&mut file, &fila, &out);
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
    escribir_linea(&mut file, ENCABEZADO, &out);
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
            escribir_linea(&mut file, &fila, &out);
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
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--retry-max-time", "120", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--write-out", "%{http_code} %header{retry-after}", "--output"])
        .arg(destino)
        .arg(url)
        .stderr(Stdio::inherit())
        .output();
    thread::sleep(Duration::from_secs(PAUSA_SEGUNDOS));
    match result {
        Err(err) => return Err(format!("no se pudo ejecutar curl: {err}")),
        Ok(salida) if !salida.status.success() => return Err(motivo_del_fallo(&String::from_utf8_lossy(&salida.stdout))),
        Ok(_) => {}
    }
    fs::read_to_string(destino).map_err(|err| format!("no se pudo leer {destino}: {err}"))
}

// curl no espera un Retry-After de más de 120 s (--retry-max-time), así que un
// 429 que pide más es el cupo diario, que dura hasta la medianoche UTC, y no
// el límite por segundo.
fn motivo_del_fallo(write_out: &str) -> String {
    let mut partes = write_out.split_whitespace();
    let status = partes.next().unwrap_or("000");
    if status != "429" {
        return format!("OpenAlex no respondió correctamente (HTTP {status})");
    }
    match partes.next().and_then(|valor| valor.parse::<u64>().ok()) {
        Some(segundos) if segundos > 120 => {
            let falta = if segundos >= 3600 { format!("unas {} h", (segundos + 1800) / 3600) } else { format!("unos {} min", (segundos + 59) / 60) };
            format!("OpenAlex respondió 429: se agotó el cupo diario y vuelve en {falta}; no reintentes antes")
        }
        _ => "OpenAlex respondió 429 (límite de peticiones); espera un minuto antes de reintentar".to_string(),
    }
}

fn escribir_linea(file: &mut fs::File, linea: &str, out: &str) {
    if let Err(err) = writeln!(file, "{linea}") {
        eprintln!("error: no se pudo escribir {out}: {err}");
        std::process::exit(1);
    }
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
    // pulpo buscar arma la identidad sin DOI como openalex:<id>, y el contrato
    // la fija en openalex:W..., no en la URL que devuelve la API.
    let id = json_string(obra, "id").unwrap_or_default().trim_start_matches("https://openalex.org/").to_string();
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
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        clean(&id),
        clean(&title),
        clean(&normalizar_doi(&json_string(obra, "doi").unwrap_or_default())),
        json_number(obra, "publication_year").unwrap_or_default(),
        json_bool(acceso, "is_oa").map(|valor| valor.to_string()).unwrap_or_default(),
        clean(&json_string(acceso, "oa_status").unwrap_or_default()),
        clean(&campo("license")),
        clean(&campo("version")),
        clean(&campo("landing_page_url")),
        clean(&campo("pdf_url")),
        clean(&pdf_urls(obra, ubicacion).join(";")),
        clean(&abstract_text(obra)),
        clean(&json_string(obra, "publication_date").unwrap_or_default()),
        json_number(obra, "cited_by_count").unwrap_or_default(),
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

fn es_fecha(valor: &str) -> bool {
    let bytes = valor.as_bytes();
    let forma = bytes.len() == 10 && bytes.iter().enumerate().all(|(pos, b)| if pos == 4 || pos == 7 { *b == b'-' } else { b.is_ascii_digit() });
    if !forma {
        return false;
    }
    let numero = |rango: std::ops::Range<usize>| valor[rango].parse::<u32>().unwrap_or(0);
    let (anio, mes, dia) = (numero(0..4), numero(5..7), numero(8..10));
    let bisiesto = anio % 4 == 0 && (anio % 100 != 0 || anio % 400 == 0);
    let dias_mes = match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if bisiesto => 29,
        2 => 28,
        _ => return false,
    };
    (1..=dias_mes).contains(&dia)
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

fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distingue_el_cupo_diario_del_limite_por_segundo() {
        assert!(motivo_del_fallo("429 63890").contains("unas 18 h"));
        assert!(motivo_del_fallo("429 300").contains("unos 5 min"));
        assert!(motivo_del_fallo("429 2").contains("un minuto"));
        assert!(motivo_del_fallo("429 ").contains("un minuto"));
        assert!(motivo_del_fallo("503 ").contains("HTTP 503"));
    }
}
