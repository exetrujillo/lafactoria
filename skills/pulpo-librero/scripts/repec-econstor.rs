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

const OAI: &str = "https://www.econstor.eu/oai/request";

struct Campo {
    schema: String,
    element: String,
    qualifier: String,
    lang: String,
    value: String,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) != Some("search") {
        usage();
    }
    let mut query = None;
    let mut ids: Vec<String> = Vec::new();
    let mut out = None;
    let mut desde = None;
    let mut set = None;
    let mut max = 25usize;
    let mut paginas = 10usize;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--query" if i + 1 < args.len() => { query = Some(args[i + 1].clone()); i += 2; }
            "--id" if i + 1 < args.len() => { ids.push(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            "--desde" if i + 1 < args.len() => { desde = Some(args[i + 1].clone()); i += 2; }
            "--set" if i + 1 < args.len() => { set = Some(args[i + 1].clone()); i += 2; }
            "--max" if i + 1 < args.len() => {
                max = args[i + 1].parse().unwrap_or_else(|_| { eprintln!("error: --max no es entero"); std::process::exit(2); });
                i += 2;
            }
            "--paginas" if i + 1 < args.len() => {
                paginas = args[i + 1].parse().unwrap_or_else(|_| { eprintln!("error: --paginas no es entero"); std::process::exit(2); });
                i += 2;
            }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    if query.is_some() == !ids.is_empty() { eprintln!("error: usa --query o al menos un --id, no ambos"); std::process::exit(2); }
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    let alternativas = query.as_deref().map(terminos).unwrap_or_default();
    if query.is_some() && alternativas.is_empty() { eprintln!("error: --query está vacía"); std::process::exit(2); }
    if let Some(fecha) = &desde {
        if !fecha_valida(fecha) { eprintln!("error: --desde debe ser YYYY-MM-DD"); std::process::exit(2); }
    }
    let raw_path = format!("{out}.raw.xml");
    let parcial = format!("{out}.raw.part");
    let _ = fs::remove_file(&raw_path);
    let mut registros: Vec<String> = Vec::new();
    let mut revisados = 0;
    let mut paginas_pedidas = 0;
    if !ids.is_empty() {
        for id in &ids {
            let handle = id.trim_start_matches("repec-econstor:").trim_start_matches("oai:econstor.eu:");
            let url = format!("{OAI}?verb=GetRecord&metadataPrefix=dim&identifier={}", encode(&format!("oai:econstor.eu:{handle}")));
            let raw = pedir_oai(&url, &parcial, &raw_path);
            registros.extend(blocks(&raw, "record").into_iter().map(str::to_string));
        }
    } else {
        let mut url = format!("{OAI}?verb=ListRecords&metadataPrefix=dim");
        if let Some(fecha) = &desde { url.push_str("&from="); url.push_str(fecha); }
        if let Some(spec) = &set { url.push_str("&set="); url.push_str(&encode(spec)); }
        while paginas_pedidas < paginas {
            let raw = pedir_oai(&url, &parcial, &raw_path);
            paginas_pedidas += 1;
            for record in blocks(&raw, "record") {
                revisados += 1;
                if coincide(record, &alternativas) && pasa_fecha(record, desde.as_deref()) { registros.push(record.to_string()); }
            }
            let token = text(&raw, "resumptionToken");
            if registros.len() >= max || token.trim().is_empty() { break; }
            url = format!("{OAI}?verb=ListRecords&resumptionToken={}", encode(token.trim()));
        }
    }
    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "{ENCABEZADO}").unwrap();
    let bitstreams_path = format!("{out}.raw.jsonl");
    let _ = fs::remove_file(&bitstreams_path);
    let mut count = 0;
    let mut con_pdf = 0;
    for record in &registros {
        if count >= max { break; }
        if record.contains("status=\"deleted\"") { continue; }
        let campos = dim_campos(record);
        let published = fecha_publicacion(&campos);
        let handle = text(record, "identifier").trim().trim_start_matches("oai:econstor.eu:").to_string();
        let title = normalize(&valor(&campos, "dc", "title", ""));
        if handle.is_empty() || title.is_empty() { continue; }
        let pdfs = match pdf_urls(&handle, &parcial, &bitstreams_path) {
            Ok(pdfs) => pdfs,
            Err((true, err)) => { eprintln!("advertencia: {handle}: {err}"); Vec::new() }
            Err((false, err)) => { eprintln!("error: {handle}: {err}"); std::process::exit(1); }
        };
        if !pdfs.is_empty() { con_pdf += 1; }
        let doi = campos.iter()
            .filter(|campo| campo.schema == "dc" && campo.element == "identifier")
            .find_map(|campo| campo.value.trim().strip_prefix("doi:").or_else(|| campo.value.trim().strip_prefix("https://doi.org/")).map(str::to_string))
            .unwrap_or_default();
        let abierto = record.contains("<setSpec>open_access</setSpec>");
        let landing_url = Some(valor(&campos, "dc", "identifier", "uri")).filter(|url| !url.is_empty()).unwrap_or_else(|| format!("https://www.econstor.eu/handle/{handle}"));
        writeln!(
            file,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t",
            clean(&format!("repec-econstor:{handle}")),
            clean(&title),
            clean(&doi),
            published.get(0..4).unwrap_or(""),
            if abierto { "true" } else { "" },
            if abierto { "abierto" } else { "" },
            clean(&valor(&campos, "dc", "rights", "license")),
            clean(&valor(&campos, "econstor", "documentversion", "")),
            clean(&landing_url),
            clean(pdfs.first().map(String::as_str).unwrap_or("")),
            clean(&pdfs.join(";")),
            clean(&normalize(&resumen(&campos))),
            clean(&published),
        ).unwrap();
        count += 1;
    }
    let _ = fs::remove_file(&parcial);
    if ids.is_empty() {
        println!("repec-econstor: {count} obras extraídas ({con_pdf} con pdf_url) de {revisados} registros revisados en {paginas_pedidas} página(s); crudo={raw_path}; resultados={out}");
    } else {
        println!("repec-econstor: {count} obras extraídas ({con_pdf} con pdf_url); crudo={raw_path}; resultados={out}");
    }
}

fn usage() -> ! {
    eprintln!("uso: repec-econstor search (--query TEXTO | --id 10419/NNNN) --out RESULTADOS.tsv [--desde YYYY-MM-DD] [--max N] [--paginas N] [--set SPEC]");
    eprintln!("--query: alternativas separadas por \" OR \"; una alternativa exige todas sus palabras como subcadenas, sin importar el orden, en título, resumen o palabras clave");
    std::process::exit(2);
}

fn pedir_oai(url: &str, parcial: &str, raw_path: &str) -> String {
    let raw = consultar(url, parcial).unwrap_or_else(|(_, err)| { eprintln!("error: {err}"); std::process::exit(1); });
    if let Err(err) = guardar_crudo(raw_path, &raw) { eprintln!("error: {err}"); std::process::exit(1); }
    if let Some(codigo) = error_oai(&raw) {
        if codigo != "noRecordsMatch" {
            eprintln!("error: EconStor respondió el error OAI {codigo}: {}", normalize(&text(&raw, "error")));
            std::process::exit(1);
        }
    }
    raw
}

fn error_oai(raw: &str) -> Option<String> {
    let start = raw.find("<error code=\"")? + "<error code=\"".len();
    let end = raw[start..].find('"')? + start;
    Some(raw[start..end].to_string())
}

// El bool del error dice si el servidor respondió con un status HTTP de error
// (curl sale con 22 por --fail), distinto de no haber podido conectar.
fn consultar(url: &str, destino: &str) -> Result<String, (bool, String)> {
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--output"])
        .arg(destino)
        .arg(url)
        .status();
    thread::sleep(Duration::from_secs(3));
    match result {
        Err(err) => return Err((false, format!("no se pudo ejecutar curl: {err}"))),
        Ok(status) if status.code() == Some(22) => return Err((true, "EconStor respondió con un status HTTP de error".to_string())),
        Ok(status) if !status.success() => return Err((false, "EconStor no respondió correctamente".to_string())),
        Ok(_) => {}
    }
    fs::read_to_string(destino).map_err(|err| (false, format!("no se pudo leer {destino}: {err}")))
}

fn guardar_crudo(destino: &str, raw: &str) -> Result<(), String> {
    let mut archivo = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(destino)
        .map_err(|err| format!("no se pudo abrir {destino}: {err}"))?;
    writeln!(archivo, "{}", raw.replace('\n', "").replace('\r', "")).map_err(|err| format!("no se pudo escribir {destino}: {err}"))
}

// OAI-PMH no entrega los archivos. La API REST de DSpace 6 los lista por
// handle; el bundle ORIGINAL es el depositado, TEXT y THUMBNAIL son derivados.
fn pdf_urls(handle: &str, parcial: &str, crudo: &str) -> Result<Vec<String>, (bool, String)> {
    let raw = consultar(&format!("https://www.econstor.eu/rest/handle/{handle}?expand=bitstreams"), parcial)?;
    guardar_crudo(crudo, &raw).map_err(|err| (false, err))?;
    let mut urls = Vec::new();
    let Some(bitstreams) = json_array(&raw, "bitstreams") else { return Ok(urls) };
    for bitstream in json_entries(bitstreams) {
        let original = json_string(bitstream, "bundleName").map(|bundle| bundle == "ORIGINAL").unwrap_or(true);
        let pdf = json_string(bitstream, "mimeType").as_deref() == Some("application/pdf");
        if let (true, true, Some(link)) = (original, pdf, json_string(bitstream, "retrieveLink")) {
            let url = if link.starts_with("/rest/") { format!("https://www.econstor.eu{link}") } else { format!("https://www.econstor.eu/rest{link}") };
            if !urls.contains(&url) { urls.push(url); }
        }
    }
    Ok(urls)
}

// No hay búsqueda por texto que se pueda usar: /discover y /simple-search
// están vedados en robots.txt y open-search responde 400. La consulta se
// resuelve aquí sobre título, resumen, palabras clave y códigos JEL.
fn terminos(query: &str) -> Vec<Vec<String>> {
    query
        .split(" OR ")
        .map(|alternativa| alternativa.split_whitespace().map(|palabra| palabra.trim_matches('"').to_lowercase()).filter(|palabra| !palabra.is_empty()).collect::<Vec<_>>())
        .filter(|alternativa| !alternativa.is_empty())
        .collect()
}

fn coincide(record: &str, alternativas: &[Vec<String>]) -> bool {
    let texto = dim_campos(record)
        .into_iter()
        .filter(|campo| campo.schema == "dc" && (campo.element == "title" || campo.element == "subject" || (campo.element == "description" && campo.qualifier == "abstract")))
        .map(|campo| campo.value)
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    alternativas.iter().any(|alternativa| alternativa.iter().all(|palabra| texto.contains(palabra.as_str())))
}

// dc.date.issued trae casi siempre sólo el año, así que se compara con la
// precisión que tenga: un año igual al de --desde pasa, porque la fuente no
// dice si la obra cae antes o después de ese día.
fn pasa_fecha(record: &str, desde: Option<&str>) -> bool {
    let Some(desde) = desde else { return true };
    let published = fecha_publicacion(&dim_campos(record));
    !published.is_empty() && published.as_str() >= desde.get(..published.len()).unwrap_or(desde)
}

// dc.date.accessioned es el depósito en EconStor, no la publicación, y el
// contrato no deja deducir published de otro dato.
fn fecha_publicacion(campos: &[Campo]) -> String {
    let issued = valor(campos, "dc", "date", "issued").trim().to_string();
    issued.get(0..10).unwrap_or(&issued).to_string()
}

fn resumen(campos: &[Campo]) -> String {
    let resumenes: Vec<&Campo> = campos.iter().filter(|campo| campo.schema == "dc" && campo.element == "description" && campo.qualifier == "abstract").collect();
    resumenes.iter().find(|campo| campo.lang == "en").or(resumenes.first()).map(|campo| campo.value.clone()).unwrap_or_default()
}

fn valor(campos: &[Campo], schema: &str, element: &str, qualifier: &str) -> String {
    campos.iter().find(|campo| campo.schema == schema && campo.element == element && campo.qualifier == qualifier).map(|campo| campo.value.clone()).unwrap_or_default()
}

fn dim_campos(record: &str) -> Vec<Campo> {
    let mut campos = Vec::new();
    let mut rest = record;
    while let Some(start) = rest.find("<dim:field") {
        let tag_end = match rest[start..].find('>') { Some(pos) => start + pos, None => break };
        let tag = &rest[start..tag_end];
        let end = match rest[tag_end..].find("</dim:field>") { Some(pos) => tag_end + pos, None => break };
        campos.push(Campo {
            schema: atributo(tag, "mdschema"),
            element: atributo(tag, "element"),
            qualifier: atributo(tag, "qualifier"),
            lang: atributo(tag, "lang"),
            value: unescape(&rest[tag_end + 1..end]),
        });
        rest = &rest[end + "</dim:field>".len()..];
    }
    campos
}

fn atributo(tag: &str, name: &str) -> String {
    let marca = format!(" {name}=\"");
    let Some(start) = tag.find(&marca).map(|pos| pos + marca.len()) else { return String::new() };
    tag[start..].find('"').map(|end| tag[start..start + end].to_string()).unwrap_or_default()
}

fn fecha_valida(fecha: &str) -> bool {
    let bytes = fecha.as_bytes();
    bytes.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-' && bytes.iter().enumerate().all(|(pos, b)| pos == 4 || pos == 7 || b.is_ascii_digit())
}

fn blocks<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let mut result = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let body_start = start + open.len();
        let end = match rest[body_start..].find(&close) { Some(pos) => body_start + pos, None => break };
        result.push(&rest[body_start..end]);
        rest = &rest[end + close.len()..];
    }
    result
}

fn text(block: &str, name: &str) -> String {
    let open = format!("<{name}");
    let start = match block.find(&open).and_then(|pos| block[pos..].find('>').map(|end| pos + end + 1)) { Some(pos) => pos, None => return String::new() };
    if block[..start].ends_with("/>") { return String::new(); }
    let close = format!("</{name}>");
    let end = match block[start..].find(&close) { Some(pos) => start + pos, None => return String::new() };
    unescape(&block[start..end])
}

fn normalize(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn unescape(value: &str) -> String {
    let mut result = value.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'");
    let mut decoded = String::new();
    let mut rest = result.as_str();
    while let Some(start) = rest.find("&#") {
        decoded.push_str(&rest[..start]);
        let end = match rest[start..].find(';') { Some(pos) => start + pos, None => { decoded.push_str(&rest[start..]); rest = ""; break; } };
        let entity = &rest[start + 2..end];
        let code = if let Some(hex) = entity.strip_prefix('x').or_else(|| entity.strip_prefix('X')) { u32::from_str_radix(hex, 16).ok() } else { entity.parse::<u32>().ok() };
        if let Some(character) = code.and_then(char::from_u32) { decoded.push(character); } else { decoded.push_str(&rest[start..=end]); }
        rest = &rest[end + 1..];
    }
    decoded.push_str(rest);
    result = decoded.replace("&amp;", "&");
    result
}

fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
