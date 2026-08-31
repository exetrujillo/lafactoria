use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

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
    let result = Command::new("curl").args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--connect-timeout", "20", "--max-time", "60", "--output"])
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
    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "openalex_id\ttitle\tdoi\tyear\tis_oa\toa_status\tlicense\tversion\tlanding_url\tpdf_url\tpdf_urls\tabstract").unwrap();
    let mut count = 0;
    for object in result_objects(&raw) {
        let id = json_string(object, "id");
        let title = json_string(object, "title");
        if id.is_empty() || title.is_empty() { continue; }
        let mut pdf_locations = json_strings(object, "pdf_url"); pdf_locations.sort(); pdf_locations.dedup();
        let pdf_urls = pdf_locations.join(";");
        writeln!(file, "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", clean(&id), clean(&title), clean(&json_string(object, "doi")), json_number(object, "publication_year"), json_bool(object, "is_oa"), clean(&json_string(object, "oa_status")), clean(&json_string(object, "license")), clean(&json_string(object, "version")), clean(&json_string(object, "landing_page_url")), clean(&json_string(object, "pdf_url")), clean(&pdf_urls), clean(&abstract_text(object))).unwrap();
        count += 1;
    }
    println!("openalex: {count} obras extraídas; crudo={raw_path}; resultados={out}");
}

fn result_objects(json: &str) -> Vec<&str> {
    let start = match json.find("\"results\"") { Some(pos) => pos, None => return Vec::new() };
    let array_start = match json[start..].find('[') { Some(pos) => start + pos + 1, None => return Vec::new() };
    let bytes = json.as_bytes(); let mut objects = Vec::new(); let mut depth = 0; let mut begin = 0; let mut quoted = false; let mut escaped = false;
    for pos in array_start..bytes.len() {
        let c = bytes[pos] as char;
        if quoted { if escaped { escaped = false; } else if c == '\\' { escaped = true; } else if c == '"' { quoted = false; } continue; }
        if c == '"' { quoted = true; } else if c == '{' { if depth == 0 { begin = pos; } depth += 1; } else if c == '}' && depth > 0 { depth -= 1; if depth == 0 { objects.push(&json[begin..=pos]); } } else if c == ']' && depth == 0 { break; }
    }
    objects
}

fn json_string(object: &str, key: &str) -> String {
    let marker = format!("\"{key}\""); let pos = match object.find(&marker) { Some(pos) => pos + marker.len(), None => return String::new() };
    let rest = object[pos..].trim_start().strip_prefix(':').unwrap_or("").trim_start(); if !rest.starts_with('"') { return String::new(); }
    let mut result = String::new(); let mut escaped = false; let mut chars = rest[1..].chars().peekable();
    while let Some(c) = chars.next() {
        if escaped {
            if c == 'u' {
                let hex: String = chars.by_ref().take(4).collect();
                if let Ok(code) = u32::from_str_radix(&hex, 16) { if let Some(decoded) = char::from_u32(code) { result.push(decoded); } }
            } else { result.push(match c { 'n' => '\n', 'r' => '\r', 't' => '\t', other => other }); }
            escaped = false;
        } else if c == '\\' { escaped = true; } else if c == '"' { break; } else { result.push(c); }
    }
    result
}

fn json_strings(object: &str, key: &str) -> Vec<String> {
    let marker = format!("\"{key}\""); let mut values = Vec::new(); let mut offset = 0;
    while let Some(found) = object[offset..].find(&marker) {
        let start = offset + found; let end = start + marker.len();
        let value = json_string(&object[start..], key); if !value.is_empty() { values.push(value); }
        offset = end;
    }
    values
}

fn json_number(object: &str, key: &str) -> String {
    let marker = format!("\"{key}\""); let pos = match object.find(&marker) { Some(pos) => pos + marker.len(), None => return String::new() };
    object[pos..].trim_start().strip_prefix(':').unwrap_or("").trim_start().chars().take_while(|c| c.is_ascii_digit()).collect()
}

fn json_bool(object: &str, key: &str) -> String {
    let marker = format!("\"{key}\""); let pos = match object.find(&marker) { Some(pos) => pos + marker.len(), None => return String::new() };
    let rest = object[pos..].trim_start().strip_prefix(':').unwrap_or("").trim_start();
    if rest.starts_with("true") { "true".to_string() } else if rest.starts_with("false") { "false".to_string() } else { String::new() }
}

fn abstract_text(object: &str) -> String {
    let marker = "\"abstract_inverted_index\"";
    let start = match object.find(marker).and_then(|pos| object[pos..].find('{').map(|inner| pos + inner)) { Some(pos) => pos, None => return String::new() };
    let bytes = object.as_bytes(); let mut depth = 0; let mut quoted = false; let mut escaped = false; let mut end = start;
    for pos in start..bytes.len() {
        let c = bytes[pos] as char;
        if quoted { if escaped { escaped = false; } else if c == '\\' { escaped = true; } else if c == '"' { quoted = false; } continue; }
        if c == '"' { quoted = true; } else if c == '{' { depth += 1; } else if c == '}' { depth -= 1; if depth == 0 { end = pos; break; } }
    }
    let body = &object[start + 1..end]; let mut words: Vec<(usize, String)> = Vec::new(); let mut pos = 0;
    while pos < body.len() {
        let key_start = match body[pos..].find('"') { Some(value) => pos + value, None => break };
        let key_end = match body[key_start + 1..].find('"') { Some(value) => key_start + 1 + value, None => break };
        let key = &body[key_start + 1..key_end];
        let array_start = match body[key_end..].find('[') { Some(value) => key_end + value + 1, None => break };
        let array_end = match body[array_start..].find(']') { Some(value) => array_start + value, None => break };
        for number in body[array_start..array_end].split(',') {
            if let Ok(index) = number.trim().parse::<usize>() { words.push((index, key.to_string())); }
        }
        pos = array_end + 1;
    }
    words.sort_by_key(|item| item.0); words.into_iter().map(|item| item.1).collect::<Vec<_>>().join(" ")
}

fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
