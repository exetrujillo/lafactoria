// El lector canónico se copia entero aunque este binario use sólo parte.
#![allow(dead_code)]

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

include!("json_api.rs");

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
        // El catálogo de otra biblioteca ya trae su identidad. Recalcularla con
        // `--source` cambia la de toda obra sin DOI y la desconecta de su manifiesto.
        let heredada = get("identity");
        let identity = if !heredada.is_empty() { heredada } else if !doi.is_empty() { format!("doi:{doi}") } else { identidad_sin_doi(&clean_value(&source), &id) };
        let values = vec![identity.clone(), id, get("title"), doi, get("year"), get("is_oa"), get("oa_status"), get("license"), get("version"), get("landing_url"), get("pdf_url"), get("pdf_urls"), get("abstract"), get("published"), get("cited_by"), format!("{}@{}", source, now), now.clone(), "pendiente".to_string(), String::new()];
        if let Some(existing) = catalog.iter_mut().find(|old| old[0] == identity) {
            fusionar(existing, values, &format!("{}@{}", source, now));
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
    let Opciones { hosts: allowed_hosts, max_files, max_bytes, max_total_bytes, pausa_ms, pausa_mismo_host_ms, user_agent } = match options(&args[4..]) {
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
    let mut red = Red { hosts: &allowed_hosts, max_bytes, user_agent: &user_agent, pausa_ms, pausa_mismo_host_ms, ultimo_host: None, wayback_limitado: false };
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
        let (id, title, urls, abierta) = if let Some(columns) = &header {
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
            let abierta = index("is_oa").and_then(|pos| fields.get(pos)).copied() == Some("true");
            (id, title, urls, abierta)
        } else { continue; };
        if urls.is_empty() {
            write_row(&mut out, id, title, "", "", "no_pdf_location", "sin ubicación HTTP(S)", 0, "");
            continue;
        }
        if !urls.iter().any(|url| allowed_url(url, &allowed_hosts)) {
            let mut hosts = Vec::new();
            for url in &urls { anotar_host(&mut hosts, url); }
            write_row(&mut out, id, title, &urls.join(";"), "", "host_no_autorizado", &format!("ningún host autorizado: {}", hosts.join(", ")), 0, "");
            continue;
        }
        if processed >= max_files {
            write_row(&mut out, id, title, &urls.join(";"), "", "limit_exceeded", "límite de archivos alcanzado", 0, "");
            continue;
        }
        processed += 1;
        let safe_id = safe_name(if id.is_empty() { "obra" } else { id });
        let final_path = dest.join(format!("{safe_id}.pdf"));
        let temp_path = temp_path(dest, &safe_id);
        let resultado = cascada(&mut red, &urls, abierta, &temp_path);
        if let Some((accepted_url, motivo)) = &resultado.aceptada {
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
                write_row(&mut out, id, title, accepted_url, &ruta_en_biblioteca(dest, &existing), "duplicate", "mismo hash en destino", bytes, &hash);
            } else {
                let destination = next_destination(&final_path);
                if fs::rename(&temp_path, &destination).is_ok() {
                write_row(&mut out, id, title, accepted_url, &ruta_en_biblioteca(dest, &destination), "accepted", motivo, bytes, &hash);
                } else {
                    write_row(&mut out, id, title, accepted_url, "", "failed", "no se pudo mover el temporal", bytes, "");
                }
            }
        } else {
            let (status, reason) = if resultado.too_large {
                ("limit_exceeded", "una respuesta superó el límite por archivo")
            } else if resultado.validator_unavailable {
                ("failed", "no hay extractor de PDF disponible")
            } else if resultado.unreadable_pdf {
                ("unreadable_pdf", "el extractor no pudo abrir ninguna respuesta PDF")
            } else if resultado.non_pdf {
                ("not_a_pdf", "una respuesta fue una landing sin citation_pdf_url ni PDF de DSpace 7; raspar el enlace")
            } else if resultado.challenged {
                ("blocked_challenge", "challenge anti-bot que no cedió por HTTP; buscar otra copia")
            } else {
                ("http_error", "ninguna ubicación respondió correctamente")
            };
            let mut reason = reason.to_string();
            if resultado.wayback_limitado { reason.push_str("; Wayback respondió 429, repetir más tarde"); }
            if !resultado.sin_autorizar.is_empty() { reason.push_str(&format!("; sin autorizar: {}", resultado.sin_autorizar.join(", "))); }
            write_row(&mut out, id, title, &urls.join(";"), "", status, &reason, 0, "");
        }
    }
}

const CATALOG_HEADER: [&str; 19] = ["identity", "id", "title", "doi", "year", "is_oa", "oa_status", "license", "version", "landing_url", "pdf_url", "pdf_urls", "abstract", "published", "cited_by", "provenance", "discovered_at", "decision", "decision_reason"];
const CATALOG_HEADER_17: [&str; 17] = ["identity", "id", "title", "doi", "year", "is_oa", "oa_status", "license", "version", "landing_url", "pdf_url", "pdf_urls", "abstract", "provenance", "discovered_at", "decision", "decision_reason"];

fn read_tsv(path: &Path) -> Result<Vec<Vec<String>>, String> {
    let file = fs::File::open(path).map_err(|err| format!("no se pudo leer {}: {err}", path.display()))?;
    Ok(io::BufReader::new(file).lines().filter_map(Result::ok).filter(|line| !line.trim().is_empty() && !line.starts_with('#')).map(|line| line.split('\t').map(str::to_string).collect()).collect())
}

fn read_catalog(path: &Path) -> Result<Vec<Vec<String>>, String> {
    if !path.exists() { return Ok(vec![CATALOG_HEADER.iter().map(|value| value.to_string()).collect()]); }
    migrar_catalogo(read_tsv(path)?).ok_or_else(|| format!("{} no tiene el encabezado de catalogo.tsv esperado", path.display()))
}

// La fusión indexa hasta `decision_reason`, y una fila editada a mano puede
// perder los tabuladores finales: toda fila se completa hasta el encabezado.
fn migrar_catalogo(mut rows: Vec<Vec<String>>) -> Option<Vec<Vec<String>>> {
    let header = rows.first().cloned().unwrap_or_default();
    if header == CATALOG_HEADER_17 {
        for row in rows.iter_mut() { row.resize(CATALOG_HEADER_17.len(), String::new()); row.splice(13..13, [String::new(), String::new()]); }
        rows[0] = CATALOG_HEADER.iter().map(|value| value.to_string()).collect();
    } else if header != CATALOG_HEADER {
        return None;
    }
    for row in rows.iter_mut() { if row.len() < CATALOG_HEADER.len() { row.resize(CATALOG_HEADER.len(), String::new()); } }
    Some(rows)
}

fn fusionar(existing: &mut Vec<String>, values: Vec<String>, procedencia: &str) {
    let decision = existing[17].clone();
    let reason = existing[18].clone();
    let provenance = append_value(&existing[15], procedencia);
    let pdf_urls = unir_urls(&values[11], &existing[11]);
    for (index, value) in values.into_iter().enumerate() { if !value.is_empty() && index < 15 { existing[index] = value; } }
    existing[11] = pdf_urls; existing[15] = provenance; existing[17] = decision; existing[18] = reason;
}

// Una fuente nueva suma ubicaciones: pisar la lista borraría las que trajo
// otra fuente.
fn unir_urls(nuevas: &str, viejas: &str) -> String {
    let mut urls: Vec<&str> = Vec::new();
    for url in nuevas.split(';').chain(viejas.split(';')).map(str::trim) {
        if !url.is_empty() && !urls.contains(&url) { urls.push(url); }
    }
    urls.join(";")
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
// Los scrapers ya emiten el id como `<fuente>:<id>`; volver a anteponer la
// fuente daba `arxiv:arxiv:...` y rompía el cruce con el manifiesto.
fn identidad_sin_doi(source: &str, id: &str) -> String { if id.starts_with(&format!("{source}:")) { id.to_string() } else { format!("{source}:{id}") } }
fn accepted_ids(path: &Path) -> Vec<String> { read_tsv(path).unwrap_or_default().into_iter().skip(1).filter(|row| row.get(4).map(String::as_str) == Some("accepted")).filter_map(|row| row.first().cloned()).collect() }

/// `NotPdf` y `Unreadable` estaban juntos y son diagnósticos distintos: el
/// primero es una landing HTML servida con `200` por un host autorizado —falta
/// el salto al archivo, la obra está bien— y el segundo es un PDF de verdad que
/// el extractor no abre. Colapsarlos hacía que el caso frecuente se leyera como
/// el raro y mandaba a buscar copia en otro lado en vez de raspar el enlace.
#[derive(PartialEq)]
enum PdfValidation { Valid, NotPdf, Unreadable, Unavailable }

fn validate_pdf(path: &Path) -> PdfValidation {
    if !fs::read(path).map(|bytes| bytes.starts_with(b"%PDF-")).unwrap_or(false) {
        return PdfValidation::NotPdf;
    }
    if let Ok(status) = Command::new("pdftotext").args(["-q"]).arg(path).arg("/dev/null").status() {
        return if status.success() { PdfValidation::Valid } else { PdfValidation::Unreadable };
    }
    let script = "import sys\ntry:\n try:\n  import pymupdf\n except ImportError:\n  import fitz as pymupdf\nexcept ImportError:\n sys.exit(75)\ndoc=pymupdf.open(sys.argv[1])\ndoc.close()";
    match Command::new("python3").args(["-c", script]).arg(path).status() {
        Ok(status) if status.success() => PdfValidation::Valid,
        Ok(status) if status.code() == Some(75) => PdfValidation::Unavailable,
        Ok(_) => PdfValidation::Unreadable,
        Err(_) => PdfValidation::Unavailable,
    }
}

fn is_header(line: &str) -> bool {
    line.split('\t').any(|column| matches!(column, "identity" | "decision" | "openalex_id" | "id" | "title" | "url" | "pdf_url" | "pdf_urls" | "landing_url"))
}

/// User-Agent de navegador corriente para el primer intento. No es evasión:
/// varios repositorios institucionales rechazan de plano al cliente que se
/// anuncia como script, aunque la obra sea de lectura libre. Anubis y el
/// "Client Challenge" de Springer hacen lo contrario, desafían a todo UA
/// "Mozilla" y atienden a curl, así que `cascada` repite con el UA por defecto
/// de curl, que dice lo que es y no nombra a esta herramienta.
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

/// Cabeceras de navegador. `Accept` sigue priorizando el PDF, que es lo que
/// esta herramienta viene a buscar; `Accept-Language` declara español porque la
/// biblioteca es hispana y varios repositorios negocian el idioma de la landing.
const ACCEPT: &str = "Accept: application/pdf,text/html;q=0.9,application/xhtml+xml;q=0.9,*/*;q=0.8";
const ACCEPT_LANGUAGE: &str = "Accept-Language: es-ES,es;q=0.9,en;q=0.8";

/// Pausas por defecto entre peticiones. La del mismo proveedor es mayor a
/// propósito: el costo de una corrida no lo paga quien la lanza sino el
/// servidor que la atiende, y bajar dos docenas de PDF del mismo repositorio
/// universitario en ráfaga es la forma más rápida de que ese repositorio deje
/// de atender a cualquiera.
const PAUSA_MS: u64 = 4500;
const PAUSA_MISMO_HOST_MS: u64 = 9000;

struct Opciones {
    hosts: Vec<String>,
    max_files: usize,
    max_bytes: u64,
    max_total_bytes: u64,
    pausa_ms: u64,
    pausa_mismo_host_ms: u64,
    user_agent: String,
}

fn options(args: &[String]) -> Result<Opciones, String> {
    let mut hosts = Vec::new();
    let mut max_files = 100usize;
    let mut max_bytes = 100 * 1024 * 1024;
    let mut max_total_bytes = 1024 * 1024 * 1024;
    let mut pausa_ms = PAUSA_MS;
    let mut pausa_mismo_host_ms = PAUSA_MISMO_HOST_MS;
    let mut user_agent = USER_AGENT.to_string();
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
            "--pausa-ms" => pausa_ms = value("--pausa-ms", &mut i)?.parse().map_err(|_| "--pausa-ms no es entero".to_string())?,
            "--pausa-mismo-host-ms" => pausa_mismo_host_ms = value("--pausa-mismo-host-ms", &mut i)?.parse().map_err(|_| "--pausa-mismo-host-ms no es entero".to_string())?,
            "--user-agent" => user_agent = value("--user-agent", &mut i)?,
            other => return Err(format!("opción desconocida: {other}")),
        }
        i += 1;
    }
    if user_agent.trim().is_empty() { return Err("--user-agent no puede estar vacío".to_string()); }
    Ok(Opciones { hosts, max_files, max_bytes, max_total_bytes, pausa_ms, pausa_mismo_host_ms, user_agent })
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

/// El challenge de Cloudflare es terminal: no lo pasa ningún UA ni combinación
/// de cabeceras, así que no se reintenta. Un 403 sin `cf-mitigated` sí se
/// repite con el UA de curl, porque hay hosts, como el OKR del Banco Mundial,
/// que rechazan al navegador y atienden a curl.
enum Fallo { Cloudflare, Prohibido, Limite, NoAutorizado(String), Otro }

/// Detecta un challenge de Cloudflare en las cabeceras volcadas. Solo se
/// consultan señales específicas del challenge (`cf-mitigated: challenge`,
/// `server-timing: chlray`), no la mera presencia de Cloudflare: hay sitios
/// servidos por Cloudflare que responden 403 por un motivo legítimo y ese caso
/// debe seguir siendo un `http_error` corriente.
fn is_bot_challenge(headers: &str) -> bool {
    let lower = headers.to_ascii_lowercase();
    lower.contains("cf-mitigated: challenge") || lower.contains("server-timing: chlray")
}

/// Títulos medidos de páginas de challenge que llegan con `200` y sin las
/// cabeceras de `is_bot_challenge`: Cloudflare sin `cf-mitigated`, Anubis, el
/// "Client Challenge" de Springer y la prueba en JS de PMC. Sin mirar el título
/// quedaban como `not_a_pdf`, que manda a raspar una landing que no existe.
const TITULOS_CHALLENGE: [&str; 4] = ["just a moment", "client challenge", "making sure you", "preparing to download"];

/// Anubis traduce su título según `Accept-Language` (a `descargar`, que pide
/// español, HAL le respondió "¡Asegurándonos de que no eres un robot!"), pero
/// la ruta de sus scripts no cambia con el idioma.
const MARCA_ANUBIS: &[u8] = b"within.website/x/cmd/anubis/";

fn es_pagina_de_challenge(cuerpo: &[u8]) -> bool {
    if cuerpo[..cuerpo.len().min(65536)].windows(MARCA_ANUBIS.len()).any(|ventana| ventana == MARCA_ANUBIS) { return true; }
    let inicio = String::from_utf8_lossy(&cuerpo[..cuerpo.len().min(8192)]).to_lowercase();
    let Some(desde) = inicio.find("<title") else { return false };
    let Some(abre) = inicio[desde..].find('>') else { return false };
    let titulo = &inicio[desde + abre + 1..];
    let titulo = titulo[..titulo.find("</title").unwrap_or(titulo.len())].trim_start();
    TITULOS_CHALLENGE.iter().any(|marca| titulo.starts_with(marca))
}

fn download(url: &str, output: &Path, allowed_hosts: &[String], max_bytes: u64, user_agent: Option<&str>) -> Result<String, Fallo> {
    let mut current = url.to_string();
    let mut previa: Option<String> = None;
    for _ in 0..=5 {
        if !allowed_url(&current, allowed_hosts) {
            let nombre = host(&current).unwrap_or("").to_ascii_lowercase();
            return Err(if nombre.is_empty() || is_private_host(&nombre) { Fallo::Otro } else { Fallo::NoAutorizado(nombre) });
        }
        let headers = output.with_extension("headers");
        let mut comando = Command::new("curl");
        // HAL y EconStor entregan a 13-16 KB/s: se corta lo que se estanca, no
        // lo que es lento.
        comando
            .args(["--fail", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--retry-max-time", "120", "--connect-timeout", "20", "--speed-limit", "1024", "--speed-time", "60", "--max-time", "900", "--max-redirs", "0", "--max-filesize"])
            .arg(max_bytes.to_string()).args(["--header", ACCEPT, "--header", ACCEPT_LANGUAGE]);
        if let Some(agente) = user_agent { comando.args(["--user-agent", agente]); }
        if let Some(anterior) = &previa { comando.args(["--referer", anterior]); }
        let result = comando.args(["--dump-header"])
            .arg(&headers).arg("--output").arg(output).arg(&current).status();
        let crudas = fs::read(&headers).unwrap_or_default();
        if let Some(next) = ubicacion_redirigida(&crudas) {
            let _ = fs::remove_file(output);
            let _ = fs::remove_file(&headers);
            previa = Some(current.clone());
            current = resolve_location(&current, &next).ok_or(Fallo::Otro)?;
            continue;
        }
        let dumped = String::from_utf8_lossy(&crudas).into_owned();
        let _ = fs::remove_file(&headers);
        if matches!(result, Ok(status) if status.success()) {
            return Ok(current);
        }
        let _ = fs::remove_file(output);
        if is_bot_challenge(&dumped) { return Err(Fallo::Cloudflare); }
        match estado_http(&dumped) {
            Some(403) => return Err(Fallo::Prohibido),
            Some(429) => return Err(Fallo::Limite),
            _ => return Err(Fallo::Otro),
        }
    }
    Err(Fallo::Otro)
}

// Hay servidores que mandan el Location sin codificar, con espacios y bytes
// Latin-1 (un DSpace con "evaluaci\xf3n" en el nombre del archivo). No es UTF-8
// y curl no acepta esa URL tal cual, así que se codifica byte a byte, como hace
// curl con -L.
fn ubicacion_redirigida(cabeceras: &[u8]) -> Option<String> {
    let linea = cabeceras.split(|b| *b == b'\n').filter(|linea| linea.len() > 9 && linea[..9].eq_ignore_ascii_case(b"location:")).last()?;
    let valor = linea[9..].trim_ascii();
    if valor.is_empty() { return None; }
    Some(valor.iter().map(|&b| if b > 0x20 && b < 0x7f { (b as char).to_string() } else { format!("%{b:02X}") }).collect())
}

fn estado_http(cabeceras: &str) -> Option<u16> {
    cabeceras.lines().filter(|linea| linea.starts_with("HTTP/")).last()?.split_whitespace().nth(1)?.parse().ok()
}

fn resolve_location(current: &str, location: &str) -> Option<String> {
    if location.starts_with("https://") || location.starts_with("http://") {
        return Some(location.to_string());
    }
    if location.starts_with("//") {
        return Some(format!("{}:{location}", current.split("://").next()?));
    }
    if location.starts_with('/') {
        return Some(format!("{}{location}", origen(current)?));
    }
    None
}

fn origen(url: &str) -> Option<String> {
    let scheme = url.split("://").next()?;
    let authority = url.split("//").nth(1)?.split('/').next()?;
    Some(format!("{scheme}://{authority}"))
}

fn anotar_host(hosts: &mut Vec<String>, url: &str) {
    if let Some(nombre) = host(url).map(str::to_ascii_lowercase) {
        if !nombre.is_empty() && !hosts.contains(&nombre) { hosts.push(nombre); }
    }
}

/// La pausa depende del host del pedido anterior, sea de la obra que sea.
struct Red<'a> {
    hosts: &'a [String],
    max_bytes: u64,
    user_agent: &'a str,
    pausa_ms: u64,
    pausa_mismo_host_ms: u64,
    ultimo_host: Option<String>,
    wayback_limitado: bool,
}

#[derive(Default)]
struct Cascada {
    aceptada: Option<(String, String)>,
    too_large: bool,
    validator_unavailable: bool,
    unreadable_pdf: bool,
    non_pdf: bool,
    challenged: bool,
    sin_autorizar: Vec<String>,
    wayback_limitado: bool,
}

enum Examen { Pdf, Grande, SinExtractor, Ilegible, Challenge, Landing(Vec<u8>) }

fn pedir(red: &mut Red, url: &str, salida: &Path, user_agent: Option<&str>) -> Result<String, Fallo> {
    let actual = host(url).map(|nombre| nombre.to_ascii_lowercase());
    if let Some(previo) = &red.ultimo_host {
        let espera = if actual.as_deref() == Some(previo.as_str()) { red.pausa_mismo_host_ms } else { red.pausa_ms };
        thread::sleep(Duration::from_millis(espera));
    }
    red.ultimo_host = actual;
    download(url, salida, red.hosts, red.max_bytes, user_agent)
}

fn examinar(path: &Path, max_bytes: u64) -> Examen {
    if fs::metadata(path).map(|metadata| metadata.len() > max_bytes).unwrap_or(true) { return Examen::Grande; }
    match validate_pdf(path) {
        PdfValidation::Valid => Examen::Pdf,
        PdfValidation::Unreadable => Examen::Ilegible,
        PdfValidation::Unavailable => Examen::SinExtractor,
        PdfValidation::NotPdf => {
            let mut cuerpo = fs::read(path).unwrap_or_default();
            cuerpo.truncate(262144);
            if es_pagina_de_challenge(&cuerpo) { Examen::Challenge } else { Examen::Landing(cuerpo) }
        }
    }
}

/// Una obra cerrada no pasa por Wayback: su copia archivada sería saltarse el
/// muro, no sortear un bloqueo técnico.
fn cascada(red: &mut Red, urls: &[&str], abierta: bool, temp: &Path) -> Cascada {
    let mut resultado = Cascada::default();
    let mut cola: Vec<(String, String)> = urls.iter().map(|url| (url.to_string(), String::new())).collect();
    let mut probadas: Vec<String> = Vec::new();
    let mut fallidas: Vec<String> = Vec::new();
    let mut wayback = false;
    let mut i = 0;
    loop {
        if i == cola.len() {
            if wayback || !abierta || fallidas.is_empty() { break; }
            wayback = true;
            let copias = copias_wayback(red, &fallidas, temp, &mut resultado);
            if copias.is_empty() { break; }
            cola.extend(copias);
            continue;
        }
        let (url, via) = cola[i].clone();
        i += 1;
        if probadas.contains(&url) { continue; }
        if let Some(id) = pmcid(&url).filter(|_| via.is_empty()) {
            if allowed_url(PMC_S3, red.hosts) {
                probadas.push(url.clone());
                if let Some(pdf) = copia_pmc(red, &id, temp) { cola.push((pdf, "vía el conjunto abierto de PMC".to_string())); }
                continue;
            }
            anotar_host(&mut resultado.sin_autorizar, PMC_S3);
        }
        if !allowed_url(&url, red.hosts) { anotar_host(&mut resultado.sin_autorizar, &url); continue; }
        probadas.push(url.clone());
        let navegador = red.user_agent;
        for user_agent in [Some(navegador), None] {
            let reintentar = match pedir(red, &url, temp, user_agent) {
                Ok(final_url) => match examinar(temp, red.max_bytes) {
                    Examen::Pdf => {
                        let mut motivo = String::from("PDF verificado");
                        if !via.is_empty() { motivo.push(' '); motivo.push_str(&via); }
                        if user_agent.is_none() { motivo.push_str(" con el UA de curl"); }
                        resultado.aceptada = Some((url, motivo));
                        return resultado;
                    }
                    Examen::Grande => { resultado.too_large = true; false }
                    Examen::SinExtractor => { resultado.validator_unavailable = true; false }
                    Examen::Ilegible => { resultado.unreadable_pdf = true; false }
                    Examen::Challenge => { resultado.challenged = true; fallidas.push(url.clone()); true }
                    Examen::Landing(cuerpo) => {
                        let _ = fs::remove_file(temp);
                        let nuevas = if via.is_empty() { desde_landing(red, &final_url, &cuerpo, temp) } else { Vec::new() };
                        if nuevas.is_empty() { resultado.non_pdf = true; }
                        cola.extend(nuevas);
                        false
                    }
                },
                Err(Fallo::Cloudflare) => { resultado.challenged = true; fallidas.push(url.clone()); false }
                Err(Fallo::Prohibido) => { fallidas.push(url.clone()); true }
                Err(Fallo::NoAutorizado(nombre)) => {
                    if !resultado.sin_autorizar.contains(&nombre) { resultado.sin_autorizar.push(nombre); }
                    false
                }
                Err(Fallo::Limite) | Err(Fallo::Otro) => { fallidas.push(url.clone()); false }
            };
            let _ = fs::remove_file(temp);
            if !reintentar { break; }
        }
    }
    resultado
}

fn desde_landing(red: &mut Red, landing: &str, cuerpo: &[u8], temp: &Path) -> Vec<(String, String)> {
    if let Some(enlace) = enlace_pdf_citado(cuerpo, landing) {
        return vec![(enlace, "vía citation_pdf_url".to_string())];
    }
    let Some(raiz) = origen(landing) else { return Vec::new() };
    let uuid = match uuid_dspace(landing) {
        Some(uuid) => Some(uuid.to_string()),
        None => handle_dspace(landing)
            .and_then(|handle| pedir_json(red, &format!("{raiz}/server/api/pid/find?id=hdl:{handle}"), temp).ok())
            .and_then(|item| json_string(&item, "uuid"))
            .filter(|uuid| es_uuid(uuid)),
    };
    let Some(uuid) = uuid else { return Vec::new() };
    let Ok(raw) = pedir_json(red, &format!("{raiz}/server/api/core/items/{uuid}/bundles?embed=bitstreams"), temp) else { return Vec::new() };
    bitstreams_originales(&raw).into_iter().map(|url| (url, "vía API de DSpace 7".to_string())).collect()
}

fn pedir_json(red: &mut Red, url: &str, temp: &Path) -> Result<String, Fallo> {
    let ruta = temp.with_extension("json");
    let cuerpo = pedir(red, url, &ruta, None).and_then(|_| fs::read_to_string(&ruta).map_err(|_| Fallo::Otro));
    let _ = fs::remove_file(&ruta);
    cuerpo
}

// Un resolvedor (doi.org, handle.net) solo redirige: su instantánea no es el
// PDF y cada consulta gasta el cupo de la API de Wayback, que responde 429
// cuando se la consulta seguido. Con el primer 429 la corrida deja de usarla.
// PMC y Europe PMC sirven el PDF tras un reCAPTCHA o un challenge. El mismo
// archivo, cuando lo hay, está en el conjunto abierto que NCBI publica en S3.
const PMC_S3: &str = "https://pmc-oa-opendata.s3.amazonaws.com";

fn pmcid(url: &str) -> Option<String> {
    let nombre = host(url)?.to_ascii_lowercase();
    if !(nombre.ends_with("ncbi.nlm.nih.gov") || nombre.ends_with("europepmc.org")) { return None; }
    let ruta = url.split("//").nth(1)?.split(|c| c == '?' || c == '#').next()?;
    let partes: Vec<&str> = ruta.split('/').collect();
    let digitos = |valor: &str| !valor.is_empty() && valor.bytes().all(|b| b.is_ascii_digit());
    if let Some(id) = partes.iter().filter_map(|parte| parte.strip_prefix("PMC")).find(|id| digitos(id)) {
        return Some(format!("PMC{id}"));
    }
    let i = partes.iter().position(|parte| *parte == "articles")?;
    partes.get(i + 1).filter(|id| digitos(id)).map(|id| format!("PMC{id}"))
}

fn copia_pmc(red: &mut Red, pmcid: &str, temp: &Path) -> Option<String> {
    let lista = pedir_json(red, &format!("{PMC_S3}/?list-type=2&prefix=metadata/{pmcid}."), temp).ok()?;
    let version = ultima_version(&lista, pmcid)?;
    let raw = pedir_json(red, &format!("{PMC_S3}/metadata/{pmcid}.{version}.json"), temp).ok()?;
    let clave = json_string(&raw, "pdf_url")?.strip_prefix("s3://pmc-oa-opendata/")?.split('?').next()?.to_string();
    Some(format!("{PMC_S3}/{clave}"))
}

fn ultima_version(lista: &str, pmcid: &str) -> Option<u32> {
    let prefijo = format!("<Key>metadata/{pmcid}.");
    lista.match_indices(&prefijo).filter_map(|(pos, _)| lista[pos + prefijo.len()..].split(".json").next()?.parse().ok()).max()
}

fn copias_wayback(red: &mut Red, fallidas: &[String], temp: &Path, resultado: &mut Cascada) -> Vec<(String, String)> {
    if !allowed_url("https://archive.org/", red.hosts) || !allowed_url("https://web.archive.org/", red.hosts) {
        if !resultado.sin_autorizar.iter().any(|nombre| nombre == "archive.org") { resultado.sin_autorizar.push("archive.org".to_string()); }
        return Vec::new();
    }
    let mut copias = Vec::new();
    let mut vistas: Vec<&str> = Vec::new();
    for url in fallidas {
        let resolvedor = host(url).map(str::to_ascii_lowercase).is_some_and(|nombre| ["doi.org", "dx.doi.org", "hdl.handle.net"].contains(&nombre.as_str()));
        if resolvedor || vistas.contains(&url.as_str()) { continue; }
        if red.wayback_limitado { resultado.wayback_limitado = true; break; }
        vistas.push(url);
        let sin_query = url.split('?').next().unwrap_or(url);
        let variantes = if sin_query == url { vec![url.as_str()] } else { vec![url.as_str(), sin_query] };
        for variante in variantes {
            let consulta = format!("https://archive.org/wayback/available?url={}", codificar(variante));
            match pedir_json(red, &consulta, temp) {
                Ok(raw) => if let Some(marca) = instantanea(&raw) {
                    copias.push((format!("https://web.archive.org/web/{marca}id_/{variante}"), format!("desde Wayback {marca}")));
                    break;
                },
                Err(Fallo::Limite) => { red.wayback_limitado = true; resultado.wayback_limitado = true; return copias; }
                Err(_) => {}
            }
        }
    }
    copias
}

fn instantanea(raw: &str) -> Option<String> {
    let cercana = json_object(raw, "archived_snapshots").and_then(|instantaneas| json_object(instantaneas, "closest"))?;
    if json_bool(cercana, "available") != Some(true) || json_string(cercana, "status").as_deref() != Some("200") { return None; }
    json_string(cercana, "timestamp").filter(|marca| !marca.is_empty() && marca.bytes().all(|b| b.is_ascii_digit()))
}

fn codificar(valor: &str) -> String {
    valor.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn enlace_pdf_citado(cuerpo: &[u8], base: &str) -> Option<String> {
    let texto = String::from_utf8_lossy(&cuerpo[..cuerpo.len().min(262144)]).into_owned();
    let bajo = texto.to_ascii_lowercase();
    let mut desde = 0;
    while let Some(pos) = bajo[desde..].find("<meta") {
        let inicio = desde + pos;
        let fin = bajo[inicio..].find('>').map(|largo| inicio + largo).unwrap_or(bajo.len());
        let etiqueta = &bajo[inicio..fin];
        if atributo(etiqueta, etiqueta, "name").as_deref() == Some("citation_pdf_url") {
            if let Some(valor) = atributo(&texto[inicio..fin], etiqueta, "content") {
                return resolve_location(base, &valor.trim().replace("&amp;", "&"));
            }
        }
        desde = fin;
    }
    None
}

// `bajo` es `original` en minúsculas ASCII, con los mismos índices: el nombre
// se busca sin distinguir mayúsculas y el valor sale con las suyas.
fn atributo(original: &str, bajo: &str, nombre: &str) -> Option<String> {
    let clave = format!("{nombre}=");
    let mut desde = 0;
    while let Some(pos) = bajo[desde..].find(&clave) {
        let inicio = desde + pos;
        desde = inicio + clave.len();
        if inicio > 0 && !bajo.as_bytes()[inicio - 1].is_ascii_whitespace() { continue; }
        let resto = &original[desde..];
        let comilla = resto.chars().next()?;
        if comilla != '"' && comilla != '\'' { return None; }
        let valor = &resto[1..];
        return valor.find(comilla).map(|largo| valor[..largo].to_string());
    }
    None
}

fn uuid_dspace(url: &str) -> Option<&str> {
    let ruta = url.split("//").nth(1)?.split(|c| c == '?' || c == '#').next()?;
    let partes: Vec<&str> = ruta.split('/').collect();
    for (i, parte) in partes.iter().enumerate() {
        let candidato = match *parte { "items" => partes.get(i + 1), "entities" => partes.get(i + 2), _ => None };
        if let Some(uuid) = candidato.copied().filter(|valor| es_uuid(valor)) { return Some(uuid); }
    }
    None
}

fn es_uuid(valor: &str) -> bool {
    valor.len() == 36 && valor.char_indices().all(|(i, c)| if matches!(i, 8 | 13 | 18 | 23) { c == '-' } else { c.is_ascii_hexdigit() })
}

fn handle_dspace(url: &str) -> Option<String> {
    let ruta = url.split("//").nth(1)?.split(|c| c == '?' || c == '#').next()?;
    let partes: Vec<&str> = ruta.split('/').collect();
    let i = partes.iter().position(|parte| *parte == "handle")?;
    match (partes.get(i + 1), partes.get(i + 2)) {
        (Some(prefijo), Some(sufijo)) if !prefijo.is_empty() && !sufijo.is_empty() => Some(format!("{prefijo}/{sufijo}")),
        _ => None,
    }
}

// Un ítem puede traer en ORIGINAL planillas o imágenes además del PDF: se
// prueban solo los que se llaman .pdf, o el primero si ninguno se llama así.
fn bitstreams_originales(raw: &str) -> Vec<String> {
    let mut pdf = Vec::new();
    let mut otros = Vec::new();
    let bundles = json_object(raw, "_embedded").and_then(|embebido| json_array(embebido, "bundles"));
    for bundle in bundles.map(json_entries).unwrap_or_default() {
        if json_string(bundle, "name").as_deref() != Some("ORIGINAL") { continue; }
        let lista = json_object(bundle, "_embedded")
            .and_then(|embebido| json_object(embebido, "bitstreams"))
            .and_then(|pagina| json_object(pagina, "_embedded"))
            .and_then(|embebido| json_array(embebido, "bitstreams"));
        for bitstream in lista.map(json_entries).unwrap_or_default() {
            let Some(href) = json_object(bitstream, "_links").and_then(|enlaces| json_object(enlaces, "content")).and_then(|contenido| json_string(contenido, "href")) else { continue };
            if json_string(bitstream, "name").unwrap_or_default().to_ascii_lowercase().ends_with(".pdf") { pdf.push(href); } else { otros.push(href); }
        }
    }
    if pdf.is_empty() { otros.truncate(1); otros } else { pdf }
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

fn ruta_en_biblioteca(dest: &Path, path: &Path) -> String {
    path.strip_prefix(dest).unwrap_or(path).display().to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconoce_los_titulos_de_challenge_medidos() {
        assert!(es_pagina_de_challenge(b"<html><head><title>Client Challenge</title>"));
        assert!(es_pagina_de_challenge(b"<title>Making sure you&#39;re not a bot!</title>"));
        assert!(es_pagina_de_challenge(b"<TITLE lang=\"en\">\n  Just a moment...</TITLE>"));
        assert!(es_pagina_de_challenge(b"<title>Preparing to download ...</title>"));
    }

    #[test]
    fn una_landing_comun_no_es_challenge() {
        assert!(!es_pagina_de_challenge(b"<title>Accent mark and visual word recognition</title>"));
        assert!(!es_pagina_de_challenge(b"<title>Waiting: just a moment of history</title>"));
        assert!(!es_pagina_de_challenge(b"<p>Just a moment</p>"));
        assert!(!es_pagina_de_challenge(b""));
        assert!(!es_pagina_de_challenge(b"<title>Anubis and the afterlife</title><p>Anubis guards the tomb</p>"));
    }

    #[test]
    fn reconoce_anubis_en_cualquier_idioma() {
        let hal = "<title>¡Asegurándonos de que no eres un robot!</title><script id=\"anubis_challenge\" type=\"application/json\"></script><script src=\"/.within.website/x/cmd/anubis/static/js/main.mjs\"></script>";
        assert!(es_pagina_de_challenge(hal.as_bytes()));
    }

    fn fila(celdas: &[&str]) -> Vec<String> { celdas.iter().map(|celda| celda.to_string()).collect() }

    #[test]
    fn la_identidad_no_repite_la_fuente() {
        assert_eq!(identidad_sin_doi("arxiv", "arxiv:2610.23618v1"), "arxiv:2610.23618v1");
        assert_eq!(identidad_sin_doi("openalex", "W123"), "openalex:W123");
        assert_eq!(identidad_sin_doi("scielo", "scielo:chl:S1"), "scielo:chl:S1");
    }

    #[test]
    fn migra_el_catalogo_de_17_columnas() {
        let viejo = vec![fila(&CATALOG_HEADER_17), fila(&["doi:10.1/aaa", "W1", "T", "10.1/aaa", "2020", "", "", "", "", "", "", "", "", "openalex@1", "1", "relevante", "central"]), fila(&["arxiv:1", "1", "U"])];
        let rows = migrar_catalogo(viejo).unwrap();
        assert!(rows.iter().all(|row| row.len() == 19));
        assert_eq!(rows[0], fila(&CATALOG_HEADER));
        assert_eq!((rows[1][13].as_str(), rows[1][14].as_str(), rows[1][15].as_str()), ("", "", "openalex@1"));
        assert_eq!((rows[1][17].as_str(), rows[1][18].as_str()), ("relevante", "central"));
        assert!(migrar_catalogo(vec![fila(&["identity", "id", "title"])]).is_none());
    }

    #[test]
    fn completa_las_filas_cortas_del_catalogo_de_19() {
        let mut corta = fila(&CATALOG_HEADER);
        corta.truncate(18);
        corta[17] = "relevante".to_string();
        let rows = migrar_catalogo(vec![fila(&CATALOG_HEADER), corta]).unwrap();
        assert_eq!(rows[1].len(), 19);
        assert_eq!((rows[1][17].as_str(), rows[1][18].as_str()), ("relevante", ""));
    }

    #[test]
    fn la_fusion_conserva_la_decision_y_no_borra_con_vacios() {
        let mut existente = fila(&["doi:10.1/aaa", "W1", "T", "10.1/aaa", "2020", "", "", "", "", "", "", "", "", "2020-03-15", "42", "openalex@1", "1", "relevante", "central"]);
        let nuevos = fila(&["doi:10.1/aaa", "W1", "T2", "10.1/aaa", "2020", "", "", "", "", "", "", "", "", "", "50", "crossref@2", "2", "pendiente", ""]);
        fusionar(&mut existente, nuevos, "crossref@2");
        assert_eq!(existente[2], "T2");
        assert_eq!((existente[13].as_str(), existente[14].as_str()), ("2020-03-15", "50"));
        assert_eq!(existente[15], "openalex@1;crossref@2");
        assert_eq!(existente[16], "1");
        assert_eq!((existente[17].as_str(), existente[18].as_str()), ("relevante", "central"));
    }

    #[test]
    fn la_fusion_suma_ubicaciones_sin_borrar_las_viejas() {
        assert_eq!(unir_urls("https://b.org/x.pdf;https://a.org/x.pdf", "https://a.org/x.pdf;https://c.org/x.pdf"), "https://b.org/x.pdf;https://a.org/x.pdf;https://c.org/x.pdf");
        assert_eq!(unir_urls("", "https://a.org/x.pdf"), "https://a.org/x.pdf");
        let mut existente = fila(&["doi:10.1/aaa", "W1", "T", "10.1/aaa", "2020", "", "", "", "", "", "https://editorial.com/x.pdf", "https://editorial.com/x.pdf", "", "", "", "crossref@1", "1", "relevante", "central"]);
        let nuevos = fila(&["doi:10.1/aaa", "W1", "T", "10.1/aaa", "2020", "true", "green", "", "", "", "https://repo.edu/x.pdf", "https://repo.edu/x.pdf", "", "", "", "openalex-doi@2", "2", "pendiente", ""]);
        fusionar(&mut existente, nuevos, "openalex-doi@2");
        assert_eq!(existente[11], "https://repo.edu/x.pdf;https://editorial.com/x.pdf");
        assert_eq!(existente[10], "https://repo.edu/x.pdf");
    }

    #[test]
    fn lee_el_ultimo_estado_http_de_las_cabeceras() {
        assert_eq!(estado_http("HTTP/2 403 \r\ncontent-type: text/html\r\n"), Some(403));
        assert_eq!(estado_http("HTTP/2 429 \r\nretry-after: 60\r\n"), Some(429));
        assert_eq!(estado_http("HTTP/1.1 503 Service Unavailable\r\n\r\nHTTP/1.1 403 Forbidden\r\n"), Some(403));
        assert_eq!(estado_http(""), None);
    }

    #[test]
    fn codifica_el_location_que_llega_crudo() {
        let cabeceras = b"HTTP/1.1 301 Movido permanentemente\r\nlocation: /bitstream/handle/1/Paraguay evaluaci\xf3n.pdf;jsessionid=AB?sequence=1\r\n\r\n";
        assert_eq!(ubicacion_redirigida(cabeceras).as_deref(), Some("/bitstream/handle/1/Paraguay%20evaluaci%F3n.pdf;jsessionid=AB?sequence=1"));
        assert_eq!(ubicacion_redirigida(b"HTTP/2 302\r\nLocation: https://a.org/x\r\n").as_deref(), Some("https://a.org/x"));
        assert_eq!(ubicacion_redirigida(b"HTTP/2 200\r\ncontent-type: application/pdf\r\n"), None);
    }

    #[test]
    fn resuelve_redirecciones_absolutas_relativas_y_sin_esquema() {
        assert_eq!(resolve_location("https://a.org/x/y", "/z.pdf").as_deref(), Some("https://a.org/z.pdf"));
        assert_eq!(resolve_location("https://a.org/x/y", "//b.org/z.pdf").as_deref(), Some("https://b.org/z.pdf"));
        assert_eq!(resolve_location("https://a.org/x/y", "http://c.org/").as_deref(), Some("http://c.org/"));
        assert_eq!(resolve_location("https://a.org/x/y", "z.pdf"), None);
    }

    #[test]
    fn sigue_el_citation_pdf_url_de_una_landing() {
        let base = "https://research.vu.nl/en/publications/x";
        assert_eq!(enlace_pdf_citado(b"<head><meta name=\"citation_title\" content=\"T\"><meta name=\"citation_pdf_url\" content=\"https://research.vu.nl/files/1/a.pdf\" /></head>", base).as_deref(), Some("https://research.vu.nl/files/1/a.pdf"));
        assert_eq!(enlace_pdf_citado(b"<META CONTENT='/bitstream/1/A.pdf?x=1&amp;y=2' NAME='citation_pdf_url'>", base).as_deref(), Some("https://research.vu.nl/bitstream/1/A.pdf?x=1&y=2"));
        assert_eq!(enlace_pdf_citado(b"<meta content=\"//cdn.example.org/a.pdf\" name=\"citation_pdf_url\">", base).as_deref(), Some("https://cdn.example.org/a.pdf"));
        assert_eq!(enlace_pdf_citado(b"<meta name=\"description\" content=\"citation_pdf_url\">", base), None);
        assert_eq!(enlace_pdf_citado(b"<meta data-name=\"citation_pdf_url\" content=\"https://x.org/a.pdf\">", base), None);
        assert_eq!(enlace_pdf_citado(b"<p>sin meta</p>", base), None);
    }

    #[test]
    fn reconoce_las_landings_de_dspace_7() {
        let uuid = "10b535a7-e9d4-51bd-96ed-6b917d5eb09e";
        assert_eq!(uuid_dspace(&format!("https://openknowledge.worldbank.org/entities/publication/{uuid}")), Some(uuid));
        assert_eq!(uuid_dspace(&format!("https://repo.example.org/items/{uuid}?mode=full")), Some(uuid));
        assert_eq!(uuid_dspace("https://repo.example.org/items/123"), None);
        assert_eq!(handle_dspace("https://gupea.ub.gu.se/handle/2077/58371").as_deref(), Some("2077/58371"));
        assert_eq!(handle_dspace("https://hal.science/hal-00582147v1"), None);
        assert_eq!(handle_dspace("https://repo.example.org/handle/2077/"), None);
    }

    #[test]
    fn toma_los_pdf_del_bundle_original() {
        let bitstream = |nombre: &str, n: u32| format!("{{\"name\":\"{nombre}\",\"_links\":{{\"content\":{{\"href\":\"https://r.org/server/api/core/bitstreams/{n}/content\"}}}}}}");
        let bundle = |nombre: &str, bitstreams: &[String]| format!("{{\"name\":\"{nombre}\",\"_embedded\":{{\"bitstreams\":{{\"_embedded\":{{\"bitstreams\":[{}]}}}}}}}}", bitstreams.join(","));
        let raw = format!("{{\"_embedded\":{{\"bundles\":[{},{}]}}}}", bundle("THUMBNAIL", &[bitstream("a.pdf.jpg", 1)]), bundle("ORIGINAL", &[bitstream("datos.xlsx", 2), bitstream("Informe.PDF", 3)]));
        assert_eq!(bitstreams_originales(&raw), vec!["https://r.org/server/api/core/bitstreams/3/content"]);
        let sin_pdf = format!("{{\"_embedded\":{{\"bundles\":[{}]}}}}", bundle("ORIGINAL", &[bitstream("informe", 4), bitstream("anexo", 5)]));
        assert_eq!(bitstreams_originales(&sin_pdf), vec!["https://r.org/server/api/core/bitstreams/4/content"]);
        assert!(bitstreams_originales("{}").is_empty());
    }

    #[test]
    fn lleva_las_urls_de_pmc_al_conjunto_abierto() {
        assert_eq!(pmcid("https://www.ncbi.nlm.nih.gov/pmc/articles/3889475").as_deref(), Some("PMC3889475"));
        assert_eq!(pmcid("https://pmc.ncbi.nlm.nih.gov/articles/PMC6559782/").as_deref(), Some("PMC6559782"));
        assert_eq!(pmcid("https://europepmc.org/articles/PMC11981939?pdf=render").as_deref(), Some("PMC11981939"));
        assert_eq!(pmcid("https://europepmc.org/article/PMC/PMC11981939").as_deref(), Some("PMC11981939"));
        assert_eq!(pmcid("https://www.ncbi.nlm.nih.gov/pubmed/28867964"), None);
        assert_eq!(pmcid("https://repo.edu/articles/PMC123"), None);
        let lista = "<ListBucketResult><Contents><Key>metadata/PMC5473177.1.json</Key></Contents><Contents><Key>metadata/PMC5473177.10.json</Key></Contents><Contents><Key>metadata/PMC5473177.2.json</Key></Contents></ListBucketResult>";
        assert_eq!(ultima_version(lista, "PMC5473177"), Some(10));
        assert_eq!(ultima_version("<ListBucketResult><KeyCount>0</KeyCount></ListBucketResult>", "PMC1"), None);
    }

    #[test]
    fn lee_la_instantanea_de_wayback() {
        assert_eq!(instantanea(r#"{"url":"x","archived_snapshots":{"closest":{"status":"200","available":true,"url":"http://web.archive.org/web/20261013133446/https://www.mdpi.com/a/pdf","timestamp":"20261013133446"}}}"#).as_deref(), Some("20261013133446"));
        assert_eq!(instantanea(r#"{"url":"x","archived_snapshots":{}}"#), None);
        assert_eq!(instantanea(r#"{"archived_snapshots":{"closest":{"status":"404","available":true,"timestamp":"2020"}}}"#), None);
        assert_eq!(instantanea(r#"{"archived_snapshots":{"closest":{"status":"200","available":true,"timestamp":"2020/../x"}}}"#), None);
        assert_eq!(codificar("https://a.org/x?y=1&z=2"), "https%3A%2F%2Fa.org%2Fx%3Fy%3D1%26z%3D2");
    }
}
