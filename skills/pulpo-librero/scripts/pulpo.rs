use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 { usage(); }
    if args[1] == "buscar" {
        run_search(&args[2..]);
        return;
    }
    if args[1] != "descargar" || args.len() < 6 || args[2] != "--catalogo" || args[4] != "--dest" {
        usage();
    }
    run_download(&args[2..]);
}

fn usage() -> ! {
    eprintln!("uso: pulpo buscar --input CANDIDATOS.tsv --dest DIRECTORIO --source FUENTE");
    eprintln!("     pulpo descargar --catalogo CATALOGO.tsv --dest DIRECTORIO --allow-host HOST [--allow-host HOST] [--max-files N] [--max-bytes N] [--max-total-bytes N]");
    std::process::exit(2);
}

fn run_search(args: &[String]) {
    let mut input = None;
    let mut dest = None;
    let mut source = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--input" if i + 1 < args.len() => { input = Some(args[i + 1].clone()); i += 2; }
            "--dest" if i + 1 < args.len() => { dest = Some(args[i + 1].clone()); i += 2; }
            "--source" if i + 1 < args.len() => { source = Some(args[i + 1].clone()); i += 2; }
            _ => { eprintln!("error: argumento desconocido o sin valor: {}", args[i]); std::process::exit(2); }
        }
    }
    let input = input.unwrap_or_else(|| { eprintln!("error: falta --input"); std::process::exit(2); });
    let dest = PathBuf::from(dest.unwrap_or_else(|| { eprintln!("error: falta --dest"); std::process::exit(2); }));
    let source = source.unwrap_or_else(|| { eprintln!("error: falta --source"); std::process::exit(2); });
    if let Err(err) = fs::create_dir_all(&dest) { eprintln!("error: no se pudo crear {}: {err}", dest.display()); std::process::exit(1); }
    let rows = match read_tsv(Path::new(&input)) { Ok(rows) => rows, Err(err) => { eprintln!("error: {err}"); std::process::exit(1); } };
    let catalogo = dest.join("catalogo.tsv");
    let mut catalog = match read_catalog(&catalogo) { Ok(rows) => rows, Err(err) => { eprintln!("error: {err}"); std::process::exit(1); } };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs().to_string();
    let mut added = 0;
    for row in rows.iter().skip(1) {
        let get = |name: &str| field(rows.first().unwrap(), row, name);
        let id = get_any(&get, &["openalex_id", "id"]);
        let doi = normalize_doi(&get("doi"));
        if id.is_empty() && doi.is_empty() { continue; }
        let identity = if !doi.is_empty() { format!("doi:{doi}") } else { format!("{}:{}", clean_value(&source), id) };
        let values = vec![identity.clone(), id, get("title"), doi, get("year"), get("is_oa"), get("oa_status"), get("license"), get("version"), get("landing_url"), get("pdf_url"), get("pdf_urls"), get("abstract"), format!("{}@{}", source, now), now.clone(), "pendiente".to_string(), String::new()];
        if let Some(existing) = catalog.iter_mut().find(|old| old[0] == identity) {
            let decision = existing[15].clone();
            let reason = existing[16].clone();
            let provenance = append_value(&existing[13], &format!("{}@{}", source, now));
            for (index, value) in values.into_iter().enumerate() { if !value.is_empty() && index != 13 && index != 14 && index != 15 && index != 16 { existing[index] = value; } }
            existing[13] = provenance; existing[15] = decision; existing[16] = reason;
        } else {
            catalog.push(values); added += 1;
        }
    }
    if let Err(err) = write_catalog(&catalogo, &catalog) { eprintln!("error: {err}"); std::process::exit(1); }
    println!("catalogo: {added} obras nuevas; total={}", catalog.len().saturating_sub(1));
}

fn run_download(args: &[String]) {
    let catalogo = Path::new(&args[1]);
    let dest = Path::new(&args[3]);
    let (allowed_hosts, max_files, max_bytes, max_total_bytes) = match options(&args[4..]) {
        Ok(options) => options,
        Err(err) => { eprintln!("error: {err}"); std::process::exit(2); }
    };
    if allowed_hosts.is_empty() { eprintln!("error: hace falta al menos un --allow-host"); std::process::exit(2); }
    if let Err(err) = fs::create_dir_all(dest) {
        eprintln!("error: no se pudo crear {}: {err}", dest.display());
        std::process::exit(1);
    }
    let manifest = dest.join("manifest.tsv");
    let mut out = match OpenOptions::new().create(true).append(true).open(&manifest) {
        Ok(file) => io::BufWriter::new(file),
        Err(err) => {
            eprintln!("error: no se pudo abrir {}: {err}", manifest.display());
            std::process::exit(1);
        }
    };
    if fs::metadata(&manifest).map(|metadata| metadata.len()).unwrap_or(0) == 0 {
        writeln!(out, "id\ttitle\tsource_url\tpath\tstatus\treason\tbytes\tsha256").unwrap();
    }

    let file = match fs::File::open(catalogo) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("error: no se pudo leer {}: {err}", catalogo.display());
            std::process::exit(1);
        }
    };
    let mut header: Option<Vec<String>> = None;
    let accepted = accepted_ids(&manifest);
    let mut processed = 0usize;
    let mut total_bytes = 0u64;
    for (line_no, line) in io::BufReader::new(file).lines().enumerate() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                eprintln!("advertencia: línea {} ilegible: {err}", line_no + 1);
                continue;
            }
        };
        if line_no == 0 && is_header(&line) { header = Some(line.split('\t').map(str::to_string).collect()); continue; }
        if line.trim().is_empty() || line.starts_with('#') { continue; }
        let fields: Vec<&str> = line.split('\t').collect();
        let (id, title, urls) = if let Some(columns) = &header {
            let index = |name: &str| columns.iter().position(|column| column == name);
            let id = index("identity").or_else(|| index("id")).and_then(|pos| fields.get(pos)).copied().unwrap_or("obra");
            let title = index("title").and_then(|pos| fields.get(pos)).copied().unwrap_or("");
            if index("decision").and_then(|pos| fields.get(pos)).copied() != Some("relevante") { continue; }
            if accepted.iter().any(|value| value == id) { continue; }
            let mut urls = Vec::new();
            for name in ["pdf_urls", "pdf_url", "landing_url", "url"] {
                if let Some(url) = index(name).and_then(|pos| fields.get(pos)).copied() {
                    for url in url.split(';') {
                        let url = url.trim();
                        if (url.starts_with("https://") || url.starts_with("http://")) && !urls.contains(&url) { urls.push(url); }
                    }
                }
            }
            (id, title, urls)
        } else { continue; };
        if processed >= max_files {
            write_row(&mut out, id, title, &urls.join(";"), "", "limit_exceeded", "límite de archivos alcanzado", 0, "");
            continue;
        }
        processed += 1;
        if urls.is_empty() {
            write_row(&mut out, id, title, "", "", "no_pdf_location", "sin ubicación HTTP(S)", 0, "");
            continue;
        }
        let safe_id = safe_name(if id.is_empty() { "obra" } else { id });
        let final_path = dest.join(format!("{safe_id}.pdf"));
        let temp_path = temp_path(dest, &safe_id);
        let mut accepted_url = "";
        let mut downloaded = false;
        let mut non_pdf = false;
        let mut unreadable_pdf = false;
        let mut validator_unavailable = false;
        let mut too_large = false;
        for url in &urls {
            if !allowed_url(url, &allowed_hosts) {
                continue;
            }
            let result = download(url, &temp_path, &allowed_hosts, max_bytes);
            thread::sleep(Duration::from_millis(1100));
            if let Ok(DownloadResult::Fetched) = result {
                if fs::metadata(&temp_path).map(|metadata| metadata.len() > max_bytes).unwrap_or(true) {
                    too_large = true;
                } else {
                    match validate_pdf(&temp_path) {
                        PdfValidation::Valid => {
                            accepted_url = url;
                            downloaded = true;
                            break;
                        }
                        PdfValidation::Invalid => { non_pdf = true; unreadable_pdf = true; }
                        PdfValidation::Unavailable => { validator_unavailable = true; }
                    }
                }
            }
            let _ = fs::remove_file(&temp_path);
        }
        if downloaded {
            let bytes = fs::metadata(&temp_path).map(|m| m.len()).unwrap_or(0);
            if total_bytes.saturating_add(bytes) > max_total_bytes {
                let _ = fs::remove_file(&temp_path);
                write_row(&mut out, id, title, accepted_url, "", "limit_exceeded", "límite total de bytes alcanzado", bytes, "");
                continue;
            }
            let hash = match sha256(&temp_path) {
                Some(hash) => hash,
                None => {
                    let _ = fs::remove_file(&temp_path);
                    write_row(&mut out, id, title, accepted_url, "", "failed", "no se pudo calcular SHA-256", bytes, "");
                    continue;
                }
            };
            total_bytes += bytes;
            if let Some(existing) = find_hash(dest, &hash) {
                let _ = fs::remove_file(&temp_path);
                write_row(&mut out, id, title, accepted_url, &existing.display().to_string(), "duplicate", "mismo hash en destino", bytes, &hash);
            } else {
                let destination = next_destination(&final_path);
                if fs::rename(&temp_path, &destination).is_ok() {
                write_row(&mut out, id, title, accepted_url, &destination.display().to_string(), "accepted", "PDF verificado", bytes, &hash);
                } else {
                    write_row(&mut out, id, title, accepted_url, "", "failed", "no se pudo mover el temporal", bytes, "");
                }
            }
        } else {
            let (status, reason) = if too_large {
                ("limit_exceeded", "una respuesta superó el límite por archivo")
            } else if validator_unavailable {
                ("failed", "no hay extractor de PDF disponible")
            } else if unreadable_pdf {
                ("unreadable_pdf", "el extractor no pudo abrir ninguna respuesta PDF")
            } else if non_pdf {
                ("not_a_pdf", "ninguna respuesta tenía formato PDF válido")
            } else {
                ("http_error", "ninguna ubicación respondió correctamente")
            };
            write_row(&mut out, id, title, &urls.join(";"), "", status, reason, 0, "");
        }
    }
}

const CATALOG_HEADER: [&str; 17] = ["identity", "id", "title", "doi", "year", "is_oa", "oa_status", "license", "version", "landing_url", "pdf_url", "pdf_urls", "abstract", "provenance", "discovered_at", "decision", "decision_reason"];

fn read_tsv(path: &Path) -> Result<Vec<Vec<String>>, String> {
    let file = fs::File::open(path).map_err(|err| format!("no se pudo leer {}: {err}", path.display()))?;
    Ok(io::BufReader::new(file).lines().filter_map(Result::ok).filter(|line| !line.trim().is_empty() && !line.starts_with('#')).map(|line| line.split('\t').map(str::to_string).collect()).collect())
}

fn read_catalog(path: &Path) -> Result<Vec<Vec<String>>, String> {
    if !path.exists() { return Ok(vec![CATALOG_HEADER.iter().map(|value| value.to_string()).collect()]); }
    let rows = read_tsv(path)?;
    if rows.first().map(|row| row == &CATALOG_HEADER.iter().map(|value| value.to_string()).collect::<Vec<_>>()).unwrap_or(false) { return Ok(rows); }
    Err(format!("{} no tiene el encabezado de catalogo.tsv esperado", path.display()))
}

fn write_catalog(path: &Path, rows: &[Vec<String>]) -> Result<(), String> {
    let temp = path.with_extension("tsv.tmp");
    let mut out = fs::File::create(&temp).map_err(|err| format!("no se pudo escribir {}: {err}", temp.display()))?;
    for row in rows { writeln!(out, "{}", row.iter().map(|value| clean_value(value)).collect::<Vec<_>>().join("\t")).map_err(|err| err.to_string())?; }
    fs::rename(&temp, path).map_err(|err| format!("no se pudo reemplazar {}: {err}", path.display()))
}

fn field(header: &[String], row: &[String], name: &str) -> String { header.iter().position(|column| column == name).and_then(|pos| row.get(pos)).cloned().unwrap_or_default() }
fn get_any(get: &dyn Fn(&str) -> String, names: &[&str]) -> String { names.iter().map(|name| get(name)).find(|value| !value.is_empty()).unwrap_or_default() }
fn normalize_doi(value: &str) -> String { value.trim().to_ascii_lowercase().trim_start_matches("https://doi.org/").trim_start_matches("http://doi.org/").trim_start_matches("doi:").to_string() }
fn append_value(old: &str, value: &str) -> String { if old.is_empty() { value.to_string() } else if old.split(';').any(|part| part == value) { old.to_string() } else { format!("{old};{value}") } }
fn clean_value(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
fn accepted_ids(path: &Path) -> Vec<String> { read_tsv(path).unwrap_or_default().into_iter().skip(1).filter(|row| row.get(4).map(String::as_str) == Some("accepted")).filter_map(|row| row.first().cloned()).collect() }

#[derive(PartialEq)]
enum PdfValidation { Valid, Invalid, Unavailable }

enum DownloadResult { Fetched }

fn validate_pdf(path: &Path) -> PdfValidation {
    if !fs::read(path).map(|bytes| bytes.starts_with(b"%PDF-")).unwrap_or(false) {
        return PdfValidation::Invalid;
    }
    if let Ok(status) = Command::new("pdftotext").args(["-q"]).arg(path).arg("/dev/null").status() {
        return if status.success() { PdfValidation::Valid } else { PdfValidation::Invalid };
    }
    let script = "import sys\ntry:\n try:\n  import pymupdf\n except ImportError:\n  import fitz as pymupdf\nexcept ImportError:\n sys.exit(75)\ndoc=pymupdf.open(sys.argv[1])\ndoc.close()";
    match Command::new("python3").args(["-c", script]).arg(path).status() {
        Ok(status) if status.success() => PdfValidation::Valid,
        Ok(status) if status.code() == Some(75) => PdfValidation::Unavailable,
        Ok(_) => PdfValidation::Invalid,
        Err(_) => PdfValidation::Unavailable,
    }
}

fn is_header(line: &str) -> bool {
    line.split('\t').any(|column| matches!(column, "identity" | "decision" | "openalex_id" | "id" | "title" | "url" | "pdf_url" | "pdf_urls" | "landing_url"))
}

fn options(args: &[String]) -> Result<(Vec<String>, usize, u64, u64), String> {
    let mut hosts = Vec::new();
    let mut max_files = 100usize;
    let mut max_bytes = 100 * 1024 * 1024;
    let mut max_total_bytes = 1024 * 1024 * 1024;
    let mut i = 0;
    while i < args.len() {
        let value = |name: &str, i: &mut usize| -> Result<String, String> {
            if *i + 1 >= args.len() { return Err(format!("{name} necesita un valor")); }
            *i += 1;
            Ok(args[*i].clone())
        };
        match args[i].as_str() {
            "--allow-host" => hosts.push(value("--allow-host", &mut i)?),
            "--max-files" => max_files = value("--max-files", &mut i)?.parse().map_err(|_| "--max-files no es entero".to_string())?,
            "--max-bytes" => max_bytes = value("--max-bytes", &mut i)?.parse().map_err(|_| "--max-bytes no es entero".to_string())?,
            "--max-total-bytes" => max_total_bytes = value("--max-total-bytes", &mut i)?.parse().map_err(|_| "--max-total-bytes no es entero".to_string())?,
            other => return Err(format!("opción desconocida: {other}")),
        }
        i += 1;
    }
    Ok((hosts, max_files, max_bytes, max_total_bytes))
}

fn host(url: &str) -> Option<&str> {
    let authority = url.split("//").nth(1)?.split('/').next()?;
    authority.rsplit('@').next()?.split(':').next()
}

fn allowed_url(url: &str, allowed_hosts: &[String]) -> bool {
    let scheme_ok = url.starts_with("https://") || url.starts_with("http://");
    let hostname = match host(url) { Some(value) => value.to_ascii_lowercase(), None => return false };
    if !scheme_ok || is_private_host(&hostname) { return false; }
    allowed_hosts.iter().any(|allowed| hostname == allowed.to_ascii_lowercase() || hostname.ends_with(&format!(".{allowed}")))
}

fn is_private_host(hostname: &str) -> bool {
    hostname == "localhost" || hostname == "::1" || hostname == "0.0.0.0" || hostname == "169.254.169.254" || hostname.starts_with("127.") || hostname.starts_with("10.") || hostname.starts_with("192.168.") || hostname.starts_with("172.16.") || hostname.starts_with("172.17.") || hostname.starts_with("172.18.") || hostname.starts_with("172.19.") || hostname.starts_with("172.2") || hostname.starts_with("172.3")
}

fn download(url: &str, output: &Path, allowed_hosts: &[String], max_bytes: u64) -> Result<DownloadResult, String> {
    let mut current = url.to_string();
    for _ in 0..=5 {
        if !allowed_url(&current, allowed_hosts) { return Err("host o redirección no autorizados".to_string()); }
        let headers = output.with_extension("headers");
        let result = Command::new("curl")
            .args(["--fail", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--connect-timeout", "20", "--max-time", "60", "--max-redirs", "0", "--max-filesize"])
            .arg(max_bytes.to_string()).args(["--user-agent", "pulpo-librero/0.1", "--header", "Accept: application/pdf", "--dump-header"])
            .arg(&headers).arg("--output").arg(output).arg(&current).status();
        let location = fs::read_to_string(&headers).ok().and_then(|headers| headers.lines().rev().find_map(|line| {
            if line.to_ascii_lowercase().starts_with("location:") { Some(line[9..].trim().to_string()) } else { None }
        }));
        if let Some(next) = location {
            let _ = fs::remove_file(output);
            let _ = fs::remove_file(&headers);
            current = resolve_location(&current, &next).ok_or_else(|| "redirección relativa o inválida".to_string())?;
            continue;
        }
        let _ = fs::remove_file(&headers);
        if matches!(result, Ok(status) if status.success()) {
            return Ok(DownloadResult::Fetched);
        }
        let _ = fs::remove_file(output);
        return Err("fallo HTTP o de transporte".to_string());
    }
    Err("demasiadas redirecciones".to_string())
}

fn resolve_location(current: &str, location: &str) -> Option<String> {
    if location.starts_with("https://") || location.starts_with("http://") {
        return Some(location.to_string());
    }
    if location.starts_with('/') {
        let origin = current.split("//").nth(1)?.split('/').next()?;
        let scheme = current.split("://").next()?;
        return Some(format!("{scheme}://{origin}{location}"));
    }
    None
}

fn sha256(path: &Path) -> Option<String> {
    Command::new("sha256sum").arg(path).output().ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|line| line.split_whitespace().next().map(str::to_string))
        .filter(|hash| !hash.is_empty())
}

fn find_hash(dest: &Path, hash: &str) -> Option<PathBuf> {
    for entry in fs::read_dir(dest).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_hash(&path, hash) { return Some(found); }
        } else if path.extension().and_then(|value| value.to_str()) == Some("pdf") && sha256(&path).as_deref() == Some(hash) {
            return Some(path);
        }
    }
    None
}

fn next_destination(preferred: &Path) -> PathBuf {
    if !preferred.exists() { return preferred.to_path_buf(); }
    let stem = preferred.file_stem().and_then(|value| value.to_str()).unwrap_or("obra");
    let parent = preferred.parent().unwrap_or_else(|| Path::new("."));
    for suffix in 2.. {
        let candidate = parent.join(format!("{stem}-{suffix}.pdf"));
        if !candidate.exists() { return candidate; }
    }
    unreachable!()
}

fn temp_path(dest: &Path, id: &str) -> PathBuf {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    dest.join(format!(".{id}.{stamp}.part"))
}

fn safe_name(value: &str) -> String {
    let result: String = value.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    if result.is_empty() { "obra".to_string() } else { result }
}

fn write_row(out: &mut impl Write, id: &str, title: &str, url: &str, path: &str, status: &str, reason: &str, bytes: u64, hash: &str) {
    let clean = |value: &str| value.replace('\t', " ").replace('\n', " ").replace('\r', " ");
    let _ = writeln!(out, "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", clean(id), clean(title), clean(url), clean(path), clean(status), clean(reason), bytes, clean(hash));
}
