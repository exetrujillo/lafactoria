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
const BASE: &str = "https://api.archives-ouvertes.fr/search/";

// fl tiene que cubrir cada campo que lee fila_tsv; uno que falte deja la
// columna vacía sin error.
const CAMPOS: &str = "docid,halId_s,uri_s,title_s,doiId_s,producedDate_s,abstract_s,openAccess_bool,licence_s,fileMain_s,files_s,linkExtUrl_s";

// HAL acepta hasta 10000 filas por página y recorta en silencio; 100 mantiene
// cada respuesta chica, porque los abstracts llegan a 2000 caracteres.
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
    let mut orden = None;
    let mut tipos = None;
    let mut dominios = None;
    let mut solo_pdf = false;
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
            "--orden" if i + 1 < args.len() => { orden = Some(args[i + 1].clone()); i += 2; }
            "--tipo" if i + 1 < args.len() => { tipos = Some(args[i + 1].clone()); i += 2; }
            "--dominio" if i + 1 < args.len() => { dominios = Some(args[i + 1].clone()); i += 2; }
            "--solo-pdf" => { solo_pdf = true; i += 1; }
            "--mailto" if i + 1 < args.len() => {
                eprintln!("advertencia: HAL no tiene polite pool; --mailto se ignora");
                i += 2;
            }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    if query.is_some() == !ids.is_empty() { eprintln!("error: usa exactamente uno de --query o --id"); std::process::exit(2); }
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    if max == 0 { eprintln!("error: --max debe ser mayor que 0"); std::process::exit(2); }
    if !ids.is_empty() && (desde.is_some() || orden.is_some() || tipos.is_some() || dominios.is_some() || solo_pdf) {
        eprintln!("error: --desde, --orden, --tipo, --dominio y --solo-pdf solo valen con --query");
        std::process::exit(2);
    }
    let orden = orden.unwrap_or_else(|| "relevancia".to_string());
    let tipos = tipos.unwrap_or_else(|| "ART,UNDEFINED".to_string());
    let dominios = dominios.unwrap_or_else(|| "shs,math,stat".to_string());
    // cursorMark exige un desempate único en el orden; sin docid la API
    // devuelve un error genérico (sondeo del 2026-10-19).
    let sort = match orden.as_str() {
        "fecha" => "producedDate_tdate desc,docid desc",
        "relevancia" => "score desc,docid desc",
        _ => { eprintln!("error: --orden acepta fecha o relevancia"); std::process::exit(2); }
    };
    if let Some(fecha) = &desde {
        if !fecha_valida(fecha) { eprintln!("error: --desde debe ser YYYY-MM-DD"); std::process::exit(2); }
    }
    let mut filtros = Vec::new();
    let consulta = if let Some(query) = query {
        let query = query.trim().to_string();
        if query.is_empty() { eprintln!("error: --query está vacío"); std::process::exit(2); }
        filtros.push("openAccess_bool:true".to_string());
        if let Some(filtro) = lista_solr("docType_s", &tipos, "") { filtros.push(filtro); }
        if let Some(filtro) = lista_solr("domain_s", &dominios, "0.") { filtros.push(filtro); }
        if solo_pdf { filtros.push("submitType_s:file".to_string()); }
        // producedDate_s es texto y compara como tal; producedDate_tdate es
        // fecha. Hay obras fechadas en el futuro que encabezan el orden por
        // fecha si no se acota por arriba.
        if let Some(fecha) = &desde {
            filtros.push(format!("producedDate_tdate:[{fecha}T00:00:00Z TO NOW]"));
        } else if orden == "fecha" {
            filtros.push("producedDate_tdate:[* TO NOW]".to_string());
        }
        query
    } else {
        for id in &ids {
            if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                eprintln!("error: --id {id} no tiene la forma de un identificador HAL (hal-01234567)");
                std::process::exit(2);
            }
        }
        format!("halId_s:({})", ids.join(" OR "))
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
        let mut url = format!("{BASE}?q={}&wt=json&fl={CAMPOS}&sort={}&rows={tamano}&cursorMark={}", encode(&consulta), encode(sort), encode(&cursor));
        for filtro in &filtros { url.push_str("&fq="); url.push_str(&encode(filtro)); }
        if paginas > 0 { thread::sleep(Duration::from_secs(3)); }
        let raw = match consultar(&url, &parcial) {
            Ok(raw) => raw,
            Err(err) => { eprintln!("error: {err}"); std::process::exit(1); }
        };
        paginas += 1;
        if let Err(err) = guardar_crudo(&raw_path, &raw) { eprintln!("error: {err}"); std::process::exit(1); }
        let _ = fs::remove_file(&parcial);
        // Una consulta que Solr no acepta vuelve como HTTP 200 con un cuerpo
        // de error genérico y sin 'response'.
        let response = match json_object(&raw, "response") {
            Some(response) => response,
            None => {
                let mensaje = json_object(&raw, "error").and_then(|error| json_string(error, "msg")).unwrap_or_else(|| "sin 'response'".to_string());
                eprintln!("error: HAL rechazó la consulta: {mensaje}");
                std::process::exit(1);
            }
        };
        let docs = match json_array(response, "docs") {
            Some(docs) => docs,
            None => { eprintln!("error: la respuesta de HAL no trae 'response.docs'"); std::process::exit(1); }
        };
        let obras = json_entries(docs);
        let ultima = obras.len() < tamano;
        for obra in obras {
            if count >= max { break; }
            if let Some(fila) = fila_tsv(obra) {
                writeln!(file, "{fila}").unwrap();
                count += 1;
            }
        }
        if ultima { break; }
        match json_string(&raw, "nextCursorMark") {
            Some(siguiente) if siguiente != cursor => cursor = siguiente,
            _ => break,
        }
    }
    println!("hal: {count} obras extraídas en {paginas} página(s); crudo={raw_path}; resultados={out}");
}

fn usage() -> ! {
    eprintln!("uso: hal search (--query TEXTO | --id HALID ...) --out RESULTADOS.tsv [--desde YYYY-MM-DD] [--max N] [--orden relevancia|fecha]");
    eprintln!("                [--tipo ART,UNDEFINED,...|todos] [--dominio shs,math,1.shs.eco,...|todos] [--solo-pdf]");
    std::process::exit(2);
}

fn lista_solr(campo: &str, valores: &str, prefijo: &str) -> Option<String> {
    if valores == "todos" {
        return None;
    }
    let partes: Vec<String> = valores.split(',').map(str::trim).filter(|valor| !valor.is_empty()).map(|valor| if con_nivel(valor) { valor.to_string() } else { format!("{prefijo}{valor}") }).collect();
    if partes.is_empty() {
        return None;
    }
    Some(format!("{campo}:({})", partes.join(" OR ")))
}

// domain_s lleva el nivel delante (0.shs, 1.shs.eco); sin él se asume 0.
fn con_nivel(valor: &str) -> bool {
    valor.split_once('.').map_or(false, |(nivel, _)| !nivel.is_empty() && nivel.bytes().all(|b| b.is_ascii_digit()))
}

fn fecha_valida(fecha: &str) -> bool {
    let bytes = fecha.as_bytes();
    bytes.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-' && bytes.iter().enumerate().all(|(pos, b)| pos == 4 || pos == 7 || b.is_ascii_digit())
}

fn consultar(url: &str, destino: &str) -> Result<String, String> {
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--retry-max-time", "120", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--output"])
        .arg(destino)
        .arg(url)
        .status();
    match result {
        Ok(status) if status.success() => {}
        Ok(_) => return Err("HAL no respondió correctamente".to_string()),
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

// HAL no expone conteo de citas, así que cited_by va siempre vacío.
fn fila_tsv(obra: &str) -> Option<String> {
    let hal_id = json_string(obra, "halId_s").unwrap_or_default();
    let title = primero(obra, "title_s");
    if hal_id.is_empty() || title.is_empty() {
        return None;
    }
    let published = fecha_publicada(&json_string(obra, "producedDate_s").unwrap_or_default());
    let landing_url = json_string(obra, "uri_s").unwrap_or_default();
    let version = landing_url.rsplit_once(&hal_id).map(|(_, resto)| resto.to_string()).filter(|resto| resto.starts_with('v')).unwrap_or_default();
    let (is_oa, oa_status) = match json_bool(obra, "openAccess_bool") {
        Some(true) => ("true", "abierto"),
        Some(false) => ("false", "desconocido"),
        None => ("", ""),
    };
    let urls = pdf_urls(obra);
    Some(format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t",
        clean(&format!("hal:{hal_id}")),
        clean(&normalize(&title)),
        clean(&json_string(obra, "doiId_s").unwrap_or_default()),
        published.get(0..4).unwrap_or(""),
        is_oa,
        oa_status,
        clean(&json_string(obra, "licence_s").unwrap_or_default()),
        clean(&version),
        clean(&landing_url),
        clean(urls.first().map(String::as_str).unwrap_or("")),
        clean(&urls.join(";")),
        clean(&normalize(&primero(obra, "abstract_s"))),
        clean(&published),
    ))
}

fn primero(obra: &str, clave: &str) -> String {
    json_texts_or_text(obra, clave).into_iter().next().unwrap_or_default()
}

// producedDate_s viene como YYYY-MM-DD, YYYY-MM o YYYY; cualquier otra forma
// se descarta en vez de pasarla al catálogo.
fn fecha_publicada(valor: &str) -> String {
    let corta = valor.get(0..10.min(valor.len())).unwrap_or("");
    let forma_ok = matches!(corta.len(), 4 | 7 | 10)
        && corta.bytes().enumerate().all(|(pos, b)| if pos == 4 || pos == 7 { b == b'-' } else { b.is_ascii_digit() });
    if forma_ok { corta.to_string() } else { String::new() }
}

// fileMain_s es el archivo principal del depósito; files_s puede traer anexos
// y va después. Las obras sin archivo en HAL a veces enlazan el PDF de arXiv
// en linkExtUrl_s; un enlace a doi.org no es un archivo y no entra.
fn pdf_urls(obra: &str) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    let mut candidatas = Vec::new();
    candidatas.extend(json_string(obra, "fileMain_s"));
    candidatas.extend(json_texts_or_text(obra, "files_s"));
    if let Some(externa) = json_string(obra, "linkExtUrl_s") {
        if externa.contains("arxiv.org/pdf/") || externa.to_ascii_lowercase().ends_with(".pdf") {
            candidatas.push(externa);
        }
    }
    for url in candidatas {
        if !url.is_empty() && !urls.contains(&url) {
            urls.push(url);
        }
    }
    urls
}

fn normalize(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
