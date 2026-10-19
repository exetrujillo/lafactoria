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
const BASE: &str = "https://www.ebi.ac.uk/europepmc/webservices/rest/search";

// Con resultType=core cada obra pesa unos 16 KB por la lista de autores; la
// API acepta hasta 1000 por página, pero 100 mantiene cada respuesta chica.
const POR_PAGINA: usize = 100;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) != Some("search") {
        usage();
    }
    let mut query = None;
    let mut ids: Vec<String> = Vec::new();
    let mut out = None;
    let mut desde = None;
    let mut max = 25usize;
    let mut orden = "citas".to_string();
    let mut src = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--query" if i + 1 < args.len() => { query = Some(args[i + 1].clone()); i += 2; }
            "--id" if i + 1 < args.len() => { ids.push(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            "--desde" if i + 1 < args.len() => { desde = Some(args[i + 1].clone()); i += 2; }
            "--max" if i + 1 < args.len() => {
                max = args[i + 1].parse().unwrap_or_else(|_| { eprintln!("error: --max no es entero"); std::process::exit(2); });
                i += 2;
            }
            "--orden" if i + 1 < args.len() => { orden = args[i + 1].clone(); i += 2; }
            "--src" if i + 1 < args.len() => { src = Some(args[i + 1].to_ascii_uppercase()); i += 2; }
            "--mailto" if i + 1 < args.len() => {
                eprintln!("advertencia: Europe PMC no tiene polite pool; --mailto se ignora");
                i += 2;
            }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    if query.is_some() == !ids.is_empty() { eprintln!("error: usa exactamente uno de --query o --id"); std::process::exit(2); }
    if !ids.is_empty() && (desde.is_some() || src.is_some()) { eprintln!("error: --id no admite --desde ni --src"); std::process::exit(2); }
    let src = src.unwrap_or_else(|| "MED".to_string());
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    if max == 0 { eprintln!("error: --max debe ser mayor que 0"); std::process::exit(2); }
    let sort = match orden.as_str() {
        "citas" => Some("CITED desc"),
        "fecha" => Some("P_PDATE_D desc"),
        "relevancia" => None,
        _ => { eprintln!("error: --orden acepta citas, fecha o relevancia"); std::process::exit(2); }
    };
    if let Some(fecha) = &desde {
        if !fecha_valida(fecha) { eprintln!("error: --desde debe ser YYYY-MM-DD"); std::process::exit(2); }
    }
    let consulta = if let Some(query) = query {
        let query = query.trim().to_string();
        if query.is_empty() { eprintln!("error: --query está vacío"); std::process::exit(2); }
        armar_consulta(&query, &src, desde.as_deref())
    } else {
        ids.iter().map(|id| consulta_por_id(id.trim())).collect::<Vec<_>>().join(" OR ")
    };
    let raw_path = format!("{out}.raw.jsonl");
    let parcial = format!("{out}.raw.part");
    let _ = fs::remove_file(&raw_path);
    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "{ENCABEZADO}").unwrap();
    let mut count = 0;
    let mut paginas = 0;
    let mut cursor = "*".to_string();
    while count < max {
        let tamano = (max - count).min(POR_PAGINA);
        let mut url = format!("{BASE}?query={}&format=json&resultType=core&pageSize={tamano}&cursorMark={}", encode(&consulta), encode(&cursor));
        if let Some(sort) = sort { url.push_str("&sort="); url.push_str(&encode(sort)); }
        if paginas > 0 { thread::sleep(Duration::from_secs(3)); }
        let raw = match consultar(&url, &parcial) {
            Ok(raw) => raw,
            Err(err) => { eprintln!("error: {err}"); std::process::exit(1); }
        };
        paginas += 1;
        if let Err(err) = guardar_crudo(&raw_path, &raw) { eprintln!("error: {err}"); std::process::exit(1); }
        let _ = fs::remove_file(&parcial);
        // Un pageSize fuera de rango vuelve como HTTP 200 con errCode en el cuerpo.
        if let Some(mensaje) = json_string(&raw, "errMsg") {
            eprintln!("error: Europe PMC rechazó la consulta: {mensaje}");
            std::process::exit(1);
        }
        let results = match json_object(&raw, "resultList").and_then(|lista| json_array(lista, "result")) {
            Some(results) => results,
            None => { eprintln!("error: la respuesta de Europe PMC no trae 'resultList.result'"); std::process::exit(1); }
        };
        let obras = json_entries(results);
        if obras.is_empty() { break; }
        for obra in obras {
            if count >= max { break; }
            if let Some(fila) = fila_tsv(obra) {
                writeln!(file, "{fila}").unwrap();
                count += 1;
            }
        }
        match json_string(&raw, "nextCursorMark") {
            Some(siguiente) if siguiente != cursor => cursor = siguiente,
            _ => break,
        }
    }
    println!("europepmc: {count} obras extraídas en {paginas} página(s); crudo={raw_path}; resultados={out}");
}

fn usage() -> ! {
    eprintln!("uso: europepmc search (--query TEXTO | --id ID ...) --out RESULTADOS.tsv [--desde YYYY-MM-DD] [--max N] [--orden citas|fecha|relevancia] [--src MED|PPR|...|TODAS]");
    std::process::exit(2);
}

// Un término sin campo también busca en el texto completo: ordenado por
// citas, "brain" trae RSEM, cuyo resumen no habla de cerebro.
// Europe PMC marca isOpenAccess=N a los preprints aunque tengan licencia CC,
// así que el filtro OPEN_ACCESS:y los dejaría todos fuera.
fn armar_consulta(query: &str, src: &str, desde: Option<&str>) -> String {
    let mut consulta = if tiene_campo(query) { format!("({query})") } else { format!("TITLE_ABS:({query})") };
    if src != "TODAS" {
        consulta.push_str(&format!(" AND SRC:{src}"));
    }
    if src != "PPR" {
        consulta.push_str(" AND OPEN_ACCESS:y");
    }
    if let Some(fecha) = desde {
        consulta.push_str(&format!(" AND FIRST_PDATE:[{fecha} TO *]"));
    }
    consulta
}

// Un ':' dentro de una frase entre comillas no nombra un campo.
fn tiene_campo(query: &str) -> bool {
    let mut entre_comillas = false;
    for c in query.chars() {
        match c {
            '"' => entre_comillas = !entre_comillas,
            ':' if !entre_comillas => return true,
            _ => {}
        }
    }
    false
}

fn consulta_por_id(id: &str) -> String {
    let mayusculas = id.to_ascii_uppercase();
    if mayusculas.starts_with("PMC") {
        format!("PMCID:{mayusculas}")
    } else if mayusculas.starts_with("PPR") {
        format!("(EXT_ID:{mayusculas} AND SRC:PPR)")
    } else if id.contains('/') {
        format!("DOI:\"{}\"", id.trim_start_matches("https://doi.org/").replace('"', ""))
    } else {
        format!("(EXT_ID:{id} AND SRC:MED)")
    }
}

fn fecha_valida(fecha: &str) -> bool {
    let bytes = fecha.as_bytes();
    if !(bytes.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-' && bytes.iter().enumerate().all(|(pos, b)| pos == 4 || pos == 7 || b.is_ascii_digit())) {
        return false;
    }
    let numero = |rango: std::ops::Range<usize>| fecha[rango].parse::<u32>().unwrap_or(0);
    let (anio, mes, dia) = (numero(0..4), numero(5..7), numero(8..10));
    let dias = match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if anio % 4 == 0 && (anio % 100 != 0 || anio % 400 == 0) => 29,
        2 => 28,
        _ => return false,
    };
    (1..=dias).contains(&dia)
}

fn consultar(url: &str, destino: &str) -> Result<String, String> {
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--retry-max-time", "120", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--output"])
        .arg(destino)
        .arg(url)
        .status();
    match result {
        Ok(status) if status.success() => {}
        Ok(_) => return Err("Europe PMC no respondió correctamente".to_string()),
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

fn fila_tsv(obra: &str) -> Option<String> {
    let source = json_string(obra, "source").unwrap_or_default();
    let id = json_string(obra, "id").unwrap_or_default();
    let title = normalize(&sin_etiquetas(&json_string(obra, "title").unwrap_or_default()));
    if source.is_empty() || id.is_empty() || title.is_empty() {
        return None;
    }
    let published = json_string(obra, "firstPublicationDate").unwrap_or_default();
    let year = match published.get(0..4) {
        Some(year) => year.to_string(),
        None => json_string(obra, "pubYear").unwrap_or_default(),
    };
    let (is_oa, oa_status) = match json_string(obra, "isOpenAccess").as_deref() {
        Some("Y") => ("true", "abierto"),
        Some("N") => ("false", "desconocido"),
        _ => ("", ""),
    };
    let version = if source == "PPR" { "preprint" } else { "" };
    let urls = pdf_urls(obra);
    Some(format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        clean(&format!("europepmc:{source}/{id}")),
        clean(&title),
        clean(&json_string(obra, "doi").unwrap_or_default()),
        clean(&year),
        is_oa,
        oa_status,
        clean(&json_string(obra, "license").unwrap_or_default()),
        version,
        clean(&format!("https://europepmc.org/article/{source}/{id}")),
        clean(urls.first().map(String::as_str).unwrap_or("")),
        clean(&urls.join(";")),
        clean(&normalize(&sin_etiquetas(&json_string(obra, "abstractText").unwrap_or_default()))),
        clean(&published),
        json_number(obra, "citedByCount").unwrap_or_default(),
    ))
}

// Sólo las entradas de estilo pdf con acceso abierto o libre. Las de
// europepmc.org van al final: el sondeo las encontró tras un challenge de
// Cloudflare, y cualquier otro host que la fuente nombre merece probarse antes.
fn pdf_urls(obra: &str) -> Vec<String> {
    let mut propias = Vec::new();
    let mut ajenas = Vec::new();
    let lista = json_object(obra, "fullTextUrlList").and_then(|lista| json_array(lista, "fullTextUrl"));
    for enlace in lista.map(json_entries).unwrap_or_default() {
        if json_string(enlace, "documentStyle").as_deref() != Some("pdf") { continue; }
        if !matches!(json_string(enlace, "availabilityCode").as_deref(), Some("OA") | Some("F")) { continue; }
        let Some(url) = json_string(enlace, "url") else { continue };
        let destino = if json_string(enlace, "site").as_deref() == Some("Europe_PMC") { &mut propias } else { &mut ajenas };
        if !destino.contains(&url) { destino.push(url); }
    }
    ajenas.extend(propias);
    ajenas
}

// Títulos y abstracts traen marcado en línea (<sup>, <i>, <h4>). Sólo se
// quita lo que tiene forma de etiqueta, porque un abstract de estadística
// puede decir "p < 0.05" y eso no debe perderse.
fn sin_etiquetas(value: &str) -> String {
    let mut result = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('<') {
        result.push_str(&rest[..start]);
        let tras = &rest[start + 1..];
        let nombre = tras.strip_prefix('/').unwrap_or(tras);
        let largo = nombre.bytes().take_while(|b| b.is_ascii_alphanumeric()).count();
        let es_etiqueta = largo > 0 && nombre.as_bytes()[0].is_ascii_alphabetic() && nombre[largo..].starts_with('>');
        if es_etiqueta {
            result.push(' ');
            rest = &nombre[largo + 1..];
        } else {
            result.push('<');
            rest = tras;
        }
    }
    result.push_str(rest);
    result
}

fn normalize(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
