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

const API: &str = "https://buscador.lareferencia.info/api/v1/search";

// limit=100 responde y limit=1000 da 400 "Invalid limit".
const POR_PAGINA: usize = 100;

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
    let mut red = None;
    let mut repositorio = None;
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
            "--red" if i + 1 < args.len() => { red = Some(args[i + 1].clone()); i += 2; }
            "--repositorio" if i + 1 < args.len() => { repositorio = Some(args[i + 1].clone()); i += 2; }
            // Sin polite pool: se acepta para que el orquestador llame a todos los scrapers igual, pero no se envía.
            "--mailto" if i + 1 < args.len() => { i += 2; }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    let query = query.unwrap_or_else(|| { eprintln!("error: falta --query"); std::process::exit(2); });
    if query.trim().is_empty() { eprintln!("error: --query está vacío"); std::process::exit(2); }
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    if max == 0 { eprintln!("error: --max debe ser mayor que 0"); std::process::exit(2); }
    let sort = match orden.as_str() {
        "relevancia" => "relevance",
        "fecha" => "year",
        _ => { eprintln!("error: --orden debe ser relevancia o fecha"); std::process::exit(2); }
    };
    if let Some(fecha) = &desde {
        if !fecha_valida(fecha) { eprintln!("error: --desde debe tener la forma YYYY-MM-DD"); std::process::exit(2); }
    }
    let mut base = format!(
        "{API}?lookfor={}&type=AllFields&sort={sort}&limit={}",
        encode(&query),
        POR_PAGINA.min(max)
    );
    for campo in ["id", "title", "publicationDates", "urls", "summary", "rawData"] {
        base.push_str("&field%5B%5D=");
        base.push_str(campo);
    }
    if let Some(fecha) = &desde {
        base.push_str("&filter%5B%5D=");
        base.push_str(&encode(&format!("publishDate:[{} TO *]", primer_anio_completo(fecha))));
    }
    if let Some(red) = &red {
        base.push_str("&filter%5B%5D=");
        base.push_str(&encode(&format!("network_acronym_str:\"{red}\"")));
    }
    if let Some(repositorio) = &repositorio {
        base.push_str("&filter%5B%5D=");
        base.push_str(&encode(&format!("reponame_str:\"{repositorio}\"")));
    }
    let raw_path = format!("{out}.raw.jsonl");
    let _ = fs::remove_file(&raw_path);
    let parcial = format!("{out}.raw.part");
    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "{ENCABEZADO}").unwrap();
    let mut count = 0;
    let mut descartadas = 0;
    let mut total = 0usize;
    let mut page = 1;
    while count < max {
        if page > 1 {
            thread::sleep(Duration::from_secs(3));
        }
        let raw = match consultar(&format!("{base}&page={page}"), &parcial) {
            Ok(raw) => raw,
            Err(err) => { eprintln!("error: {err}"); std::process::exit(1); }
        };
        if let Err(err) = guardar_crudo(&raw_path, &raw) {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
        if json_string(&raw, "status").as_deref() != Some("OK") {
            eprintln!("error: LA Referencia respondió sin status OK: {}", json_string(&raw, "statusMessage").unwrap_or_default());
            std::process::exit(1);
        }
        total = json_number(&raw, "resultCount").and_then(|valor| valor.parse().ok()).unwrap_or(0);
        let registros = json_array(&raw, "records").map(json_entries).unwrap_or_default();
        let mut aportadas = 0;
        for registro in &registros {
            if count >= max {
                break;
            }
            match fila_tsv(registro, desde.as_deref()) {
                Some(fila) => { writeln!(file, "{fila}").unwrap(); count += 1; aportadas += 1; }
                None => descartadas += 1,
            }
        }
        if registros.is_empty() || aportadas == 0 || page * POR_PAGINA.min(max) >= total {
            break;
        }
        page += 1;
    }
    let _ = fs::remove_file(&parcial);
    println!("redalyc-lareferencia: {count} obras extraídas de {total} resultados; {descartadas} descartadas; crudo={raw_path}; resultados={out}");
}

fn usage() -> ! {
    eprintln!("uso: redalyc-lareferencia search --query TEXTO --out RESULTADOS.tsv [--desde YYYY-MM-DD] [--max N] [--orden relevancia|fecha] [--red CODIGO] [--repositorio NOMBRE] [--mailto EMAIL]");
    std::process::exit(2);
}

fn consultar(url: &str, destino: &str) -> Result<String, String> {
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--output"])
        .arg(destino)
        .arg(url)
        .status();
    match result {
        Ok(status) if status.success() => {}
        Ok(_) => return Err("LA Referencia no respondió correctamente".to_string()),
        Err(err) => return Err(format!("no se pudo ejecutar curl: {err}")),
    }
    fs::read_to_string(destino).map_err(|err| format!("no se pudo leer {destino}: {err}"))
}

// Una respuesta por línea: el crudo de varias páginas no entra en un solo JSON.
fn guardar_crudo(destino: &str, raw: &str) -> Result<(), String> {
    let mut archivo = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(destino)
        .map_err(|err| format!("no se pudo abrir {destino}: {err}"))?;
    writeln!(archivo, "{}", raw.replace('\n', "").replace('\r', "")).map_err(|err| format!("no se pudo escribir {destino}: {err}"))
}

fn fila_tsv(registro: &str, desde: Option<&str>) -> Option<String> {
    let id = json_string(registro, "id").unwrap_or_default();
    let title = json_string(registro, "title").unwrap_or_default();
    if id.is_empty() || title.is_empty() {
        return None;
    }
    let crudo = json_object(registro, "rawData").unwrap_or("");
    let published = json_texts_or_text(crudo, "publishDate")
        .into_iter()
        .chain(json_texts_or_text(registro, "publicationDates"))
        .map(|valor| fecha_publicada(&valor))
        .find(|valor| !valor.is_empty())
        .unwrap_or_default();
    if let Some(desde) = desde {
        if published.is_empty() || !cubre_desde(&published, desde) {
            return None;
        }
    }
    let urls: Vec<String> = json_array(registro, "urls")
        .map(|arreglo| json_entries(arreglo).into_iter().filter_map(|url| json_string(url, "url")).collect())
        .unwrap_or_default();
    let doi = json_texts_or_text(crudo, "identifier_str_mv")
        .iter()
        .chain(urls.iter())
        .map(|valor| doi_de(valor))
        .find(|valor| !valor.is_empty())
        .unwrap_or_default();
    let landing_url = urls.iter().find(|url| !url.contains("doi.org/")).or(urls.first()).cloned().unwrap_or_default();
    let bitstreams = json_texts_or_text(crudo, "bitstream_url_str_mv");
    let derechos = json_texts_or_text(crudo, "eu_rights_str_mv").into_iter().next().unwrap_or_default();
    let is_oa = match derechos.as_str() {
        "" => String::new(),
        "openAccess" => "true".to_string(),
        _ => "false".to_string(),
    };
    let abstract_text = json_texts_or_text(registro, "summary").into_iter().next().unwrap_or_default();
    Some(format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        clean(&format!("redalyc-lareferencia:{id}")),
        clean(&normalize(&title)),
        clean(&doi),
        published.get(0..4).unwrap_or(""),
        is_oa,
        clean(&derechos),
        "",
        clean(&json_string(crudo, "status_str").unwrap_or_default()),
        clean(&landing_url),
        clean(bitstreams.first().map(String::as_str).unwrap_or("")),
        clean(&bitstreams.join(";")),
        clean(&normalize(sin_rotulo(&abstract_text))),
        clean(&published),
        "",
    ))
}

// La fuente fecha casi todo con el año a secas. Un "2024" no prueba que la obra
// sea posterior a 2024-10-19, así que sólo pasa la fecha cuyo primer día
// posible ya es igual o posterior a --desde.
fn cubre_desde(published: &str, desde: &str) -> bool {
    let inicio = match published.len() {
        4 => format!("{published}-01-01"),
        7 => format!("{published}-01"),
        _ => published.to_string(),
    };
    inicio.as_str() >= desde
}

fn primer_anio_completo(desde: &str) -> u32 {
    let anio: u32 = desde[0..4].parse().unwrap_or(0);
    if &desde[4..] == "-01-01" { anio } else { anio + 1 }
}

fn fecha_valida(valor: &str) -> bool {
    let bytes = valor.as_bytes();
    bytes.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-' && bytes.iter().enumerate().all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
}

fn fecha_publicada(valor: &str) -> String {
    let valor = valor.trim();
    let digitos = |parte: &str| !parte.is_empty() && parte.bytes().all(|b| b.is_ascii_digit());
    let partes: Vec<&str> = valor.split('-').collect();
    match partes.as_slice() {
        [a] if a.len() == 4 && digitos(a) => valor.to_string(),
        [a, m] if a.len() == 4 && m.len() == 2 && digitos(a) && digitos(m) => valor.to_string(),
        [a, m, d] if a.len() == 4 && m.len() == 2 && d.len() == 2 && digitos(a) && digitos(m) && digitos(d) => valor.to_string(),
        _ => valor.get(0..4).filter(|anio| digitos(anio)).map(str::to_string).unwrap_or_default(),
    }
}

// identifier_str_mv mezcla DOI, ISSN, números de inventario y hasta la cita
// completa de una tesis, y a veces trae un DOI mal formado ("00.51915/...").
// Sólo cuenta lo que empieza con "10.<registrante>/".
fn doi_de(valor: &str) -> String {
    let valor = valor.trim();
    let valor = match valor.find("doi.org/") {
        Some(pos) => &valor[pos + "doi.org/".len()..],
        None => valor.strip_prefix("doi:").unwrap_or(valor),
    };
    let Some(resto) = valor.strip_prefix("10.") else { return String::new() };
    match resto.split_once('/') {
        Some((registrante, sufijo)) if registrante.len() >= 4 && registrante.bytes().all(|b| b.is_ascii_digit() || b == b'.') && !sufijo.is_empty() && !valor.contains(' ') => valor.to_string(),
        _ => String::new(),
    }
}

fn sin_rotulo(valor: &str) -> &str {
    let valor = valor.trim();
    for rotulo in ["Resumen:", "Abstract:", "Resumo:"] {
        if let Some(resto) = valor.strip_prefix(rotulo) {
            return resto.trim_start();
        }
    }
    valor
}

fn normalize(value: &str) -> String { value.split_whitespace().collect::<Vec<_>>().join(" ") }
fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
