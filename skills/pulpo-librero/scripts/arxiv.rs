use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 6 || args[1] != "search" {
        eprintln!("uso: arxiv search (--query TEXTO | --id ID) --out RESULTADOS.tsv [--max-results N] [--start N] [--mailto EMAIL]");
        std::process::exit(2);
    }
    let mut query = None;
    let mut exact_id = None;
    let mut out = None;
    let mut max_results = "25".to_string();
    let mut start = "0".to_string();
    let mut mailto = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--query" if i + 1 < args.len() => { query = Some(args[i + 1].clone()); i += 2; }
            "--id" if i + 1 < args.len() => { exact_id = Some(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            "--max-results" if i + 1 < args.len() => { max_results = args[i + 1].clone(); i += 2; }
            "--start" if i + 1 < args.len() => { start = args[i + 1].clone(); i += 2; }
            "--mailto" if i + 1 < args.len() => { mailto = Some(args[i + 1].clone()); i += 2; }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    if query.is_some() == exact_id.is_some() { eprintln!("error: usa exactamente uno de --query o --id"); std::process::exit(2); }
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    let mut url = if let Some(id) = exact_id {
        format!("https://export.arxiv.org/api/query?id_list={}&max_results=1", encode_query(&id))
    } else {
        let query = query.unwrap();
        let search_query = if query.contains(':') { query } else { format!("all:{query}") };
        format!("https://export.arxiv.org/api/query?search_query={}&start={}&max_results={}", encode_query(&search_query), encode(&start), encode(&max_results))
    };
    if let Some(email) = mailto { url.push_str("&mailto="); url.push_str(&encode(&email)); }
    let raw_path = format!("{out}.raw.xml");
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--output"])
        .arg(&raw_path).arg(&url).status();
    thread::sleep(Duration::from_secs(3));
    if !matches!(result, Ok(status) if status.success()) {
        eprintln!("error: arXiv no respondió correctamente");
        std::process::exit(1);
    }
    let raw = match fs::read_to_string(&raw_path) {
        Ok(raw) => raw,
        Err(err) => { eprintln!("error: no se pudo leer {raw_path}: {err}"); std::process::exit(1); }
    };
    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "id\ttitle\tdoi\tyear\tversion\tlanding_url\tpdf_url\tpdf_urls\tabstract").unwrap();
    let mut count = 0;
    for entry in blocks(&raw, "entry") {
        let landing_url = text(entry, "id");
        let id = arxiv_id(&landing_url);
        let title = normalize(&text(entry, "title"));
        if id.is_empty() || title.is_empty() { continue; }
        let version = id.rsplit_once('v').map(|(_, version)| format!("v{version}")).unwrap_or_default();
        let pdf_url = format!("https://arxiv.org/pdf/{id}.pdf");
        let year = text(entry, "published").get(0..4).unwrap_or("").to_string();
        let abstract_text = normalize(&text(entry, "summary"));
        let doi = text(entry, "arxiv:doi");
        writeln!(file, "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", clean(&format!("arxiv:{id}")), clean(&title), clean(&doi), clean(&year), clean(&version), clean(&landing_url), clean(&pdf_url), clean(&pdf_url), clean(&abstract_text)).unwrap();
        count += 1;
    }
    println!("arxiv: {count} obras extraídas; crudo={raw_path}; resultados={out}");
}

fn blocks<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let mut result = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let body_start = match rest[start..].find('>') { Some(pos) => start + pos + 1, None => break };
        let end = match rest[body_start..].find(&close) { Some(pos) => body_start + pos, None => break };
        result.push(&rest[body_start..end]);
        rest = &rest[end + close.len()..];
    }
    result
}

fn text(block: &str, name: &str) -> String {
    let open = format!("<{name}");
    let start = match block.find(&open).and_then(|pos| block[pos..].find('>').map(|end| pos + end + 1)) { Some(pos) => pos, None => return String::new() };
    let close = format!("</{name}>");
    let end = match block[start..].find(&close) { Some(pos) => start + pos, None => return String::new() };
    unescape(&block[start..end])
}

fn arxiv_id(url: &str) -> String {
    let value = url.split("/abs/").nth(1).or_else(|| url.rsplit('/').next()).unwrap_or("").trim();
    value.strip_suffix(".pdf").unwrap_or(value).to_string()
}

fn normalize(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn unescape(value: &str) -> String {
    let mut result = value.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'");
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
    result = decoded;
    result
}

fn encode(value: &str) -> String {
    value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect()
}

fn encode_query(value: &str) -> String {
    value.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b':' | b'+' | b'(' | b')' => (b as char).to_string(),
        other => format!("%{other:02X}"),
    }).collect()
}

fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
