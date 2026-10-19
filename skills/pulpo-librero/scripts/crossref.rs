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
const CAMPOS: &str = "DOI,title,published,is-referenced-by-count,abstract,link,license,URL";
const FILAS_POR_PAGINA: usize = 100;

// license.url compara la cadena exacta: http y https, con y sin barra final,
// son licencias distintas para el filtro. Medido el 2026-10-19 con CC BY 4.0:
// una sola variante dio 41243 obras y las cuatro juntas 136996. Varias
// cláusulas license.url se combinan con OR.
const LICENCIAS_CC: [&str; 13] = [
    "licenses/by/4.0", "licenses/by-sa/4.0", "licenses/by-nc/4.0", "licenses/by-nd/4.0", "licenses/by-nc-sa/4.0", "licenses/by-nc-nd/4.0",
    "licenses/by/3.0", "licenses/by-sa/3.0", "licenses/by-nc/3.0", "licenses/by-nd/3.0", "licenses/by-nc-sa/3.0", "licenses/by-nc-nd/3.0",
    "publicdomain/zero/1.0",
];

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 6 || args[1] != "search" {
        usage();
    }
    let mut query = None;
    let mut out = None;
    let mut desde = None;
    let mut hasta = None;
    let mut max = 25usize;
    let mut orden = "citas".to_string();
    let mut licencia = "abierta".to_string();
    let mut mailto = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--query" if i + 1 < args.len() => { query = Some(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            "--desde" if i + 1 < args.len() => { desde = Some(args[i + 1].clone()); i += 2; }
            "--hasta" if i + 1 < args.len() => { hasta = Some(args[i + 1].clone()); i += 2; }
            "--max" if i + 1 < args.len() => {
                max = args[i + 1].parse().unwrap_or_else(|_| { eprintln!("error: --max no es entero"); std::process::exit(2); });
                i += 2;
            }
            "--orden" if i + 1 < args.len() => { orden = args[i + 1].clone(); i += 2; }
            "--licencia" if i + 1 < args.len() => { licencia = args[i + 1].clone(); i += 2; }
            "--mailto" if i + 1 < args.len() => { mailto = Some(args[i + 1].clone()); i += 2; }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    let query = query.filter(|valor| !valor.trim().is_empty()).unwrap_or_else(|| { eprintln!("error: falta --query o está vacío"); std::process::exit(2); });
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    for fecha in [desde.as_ref(), hasta.as_ref()].iter().flatten() {
        if !fecha_valida(fecha) { eprintln!("error: la fecha {fecha} no es una fecha YYYY-MM-DD válida"); std::process::exit(2); }
    }
    if orden == "fecha" && hasta.is_none() {
        eprintln!("aviso: Crossref tiene obras con fecha futura; con --orden fecha pasa --hasta con la fecha de hoy");
    }
    if max == 0 { eprintln!("error: --max debe ser mayor que cero"); std::process::exit(2); }
    let sort = match orden.as_str() {
        "citas" => "&sort=is-referenced-by-count&order=desc",
        "fecha" => "&sort=published&order=desc",
        "relevancia" => "&sort=score&order=desc",
        _ => { eprintln!("error: --orden debe ser citas, fecha o relevancia"); std::process::exit(2); }
    };
    let mut filtros: Vec<String> = Vec::new();
    if let Some(fecha) = &desde { filtros.push(format!("from-pub-date:{fecha}")); }
    if let Some(fecha) = &hasta { filtros.push(format!("until-pub-date:{fecha}")); }
    match licencia.as_str() {
        "abierta" => {
            for ruta in LICENCIAS_CC {
                for prefijo in ["http://", "https://"] {
                    for fin in ["", "/"] {
                        filtros.push(format!("license.url:{prefijo}creativecommons.org/{ruta}{fin}"));
                    }
                }
            }
        }
        "todas" => {}
        _ => { eprintln!("error: --licencia debe ser abierta o todas"); std::process::exit(2); }
    }
    let filas_por_pagina = max.min(FILAS_POR_PAGINA);
    let mut base = format!("https://api.crossref.org/works?query={}&rows={filas_por_pagina}&select={}{sort}", encode(&query), encode(CAMPOS));
    if !filtros.is_empty() { base.push_str("&filter="); base.push_str(&encode(&filtros.join(","))); }
    if let Some(email) = mailto { base.push_str("&mailto="); base.push_str(&encode(&email)); }
    let raw_path = format!("{out}.raw.jsonl");
    let parcial = format!("{out}.raw.part");
    let _ = fs::remove_file(&raw_path);
    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "{ENCABEZADO}").unwrap();
    let mut count = 0;
    let mut paginas = 0;
    let mut vistos: Vec<String> = Vec::new();
    let mut cursor = "*".to_string();
    // Tope de páginas por si el filtro local de fecha descarta mucho: sin él,
    // una discrepancia entre from-pub-date y 'published' recorrería el índice entero.
    let max_paginas = max / filas_por_pagina + 3;
    // Crossref rechaza el cursor con sort=published (400
    // sort-criteria-incompatible-with-cursor), así que ese orden pagina por
    // offset, y entre páginas por offset se repiten obras con la misma fecha.
    let usar_cursor = orden != "fecha";
    while count < max && paginas < max_paginas {
        let url = if usar_cursor {
            format!("{base}&cursor={}", encode(&cursor))
        } else {
            format!("{base}&offset={}", paginas * filas_por_pagina)
        };
        let raw = match consultar(&url, &parcial) {
            Ok(raw) => raw,
            Err(err) => { eprintln!("error: {err}"); std::process::exit(1); }
        };
        paginas += 1;
        if let Err(err) = guardar_crudo(&raw_path, &raw) { eprintln!("error: {err}"); std::process::exit(1); }
        let _ = fs::remove_file(&parcial);
        let mensaje = json_object(&raw, "message").unwrap_or("");
        let items = match json_array(mensaje, "items") {
            Some(items) => items,
            None => { eprintln!("error: la respuesta de Crossref no trae 'message.items'"); std::process::exit(1); }
        };
        let obras = json_entries(items);
        if obras.is_empty() { break; }
        for obra in obras {
            if count >= max { break; }
            if let Some(fila) = fila_tsv(obra, desde.as_deref()) {
                let id = fila.split('\t').next().unwrap_or("").to_string();
                if vistos.contains(&id) { continue; }
                vistos.push(id);
                writeln!(file, "{fila}").unwrap();
                count += 1;
            }
        }
        if usar_cursor {
            match json_string(mensaje, "next-cursor") {
                Some(siguiente) => cursor = siguiente,
                None => break,
            }
        }
    }
    println!("crossref: {count} obras extraídas en {paginas} página(s); crudo={raw_path}; resultados={out}");
}

fn usage() -> ! {
    eprintln!("uso: crossref search --query TEXTO --out RESULTADOS.tsv [--desde YYYY-MM-DD] [--hasta YYYY-MM-DD] [--max N] [--orden citas|fecha|relevancia] [--licencia abierta|todas] [--mailto EMAIL]");
    std::process::exit(2);
}

fn consultar(url: &str, destino: &str) -> Result<String, String> {
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--output"])
        .arg(destino)
        .arg(url)
        .status();
    thread::sleep(Duration::from_secs(3));
    match result {
        Err(err) => Err(format!("no se pudo ejecutar curl: {err}")),
        Ok(status) if !status.success() => Err("Crossref no respondió correctamente".to_string()),
        Ok(_) => fs::read_to_string(destino).map_err(|err| format!("no se pudo leer {destino}: {err}")),
    }
}

// Una respuesta por línea: el crudo de N páginas no entra en un solo JSON.
fn guardar_crudo(destino: &str, raw: &str) -> Result<(), String> {
    let mut archivo = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(destino)
        .map_err(|err| format!("no se pudo abrir {destino}: {err}"))?;
    writeln!(archivo, "{}", raw.replace('\n', "").replace('\r', "")).map_err(|err| format!("no se pudo escribir {destino}: {err}"))
}

fn fila_tsv(obra: &str, desde: Option<&str>) -> Option<String> {
    let doi = json_string(obra, "DOI").unwrap_or_default();
    let title = json_array(obra, "title").and_then(|titulos| json_texts(titulos).into_iter().next()).map(|titulo| texto_plano(&titulo)).unwrap_or_default();
    if doi.is_empty() || title.is_empty() {
        return None;
    }
    let published = json_object(obra, "published").map(fecha).unwrap_or_default();
    if let Some(desde) = desde {
        if published.is_empty() || desde.get(..published.len()).map_or(true, |inicio| published.as_str() < inicio) {
            return None;
        }
    }
    let (licencia, abierta) = licencia(obra);
    let (pdfs, version) = enlaces(obra);
    let mut resumen = texto_plano(&json_string(obra, "abstract").unwrap_or_default());
    if resumen.get(0..9).map_or(false, |inicio| inicio.eq_ignore_ascii_case("abstract ")) {
        resumen = resumen[9..].to_string();
    }
    Some(format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        clean(&format!("crossref:{doi}")),
        clean(&title),
        clean(&doi),
        published.get(0..4).unwrap_or(""),
        if abierta { "true" } else { "" },
        if abierta { "abierto" } else { "desconocido" },
        clean(&licencia),
        clean(&version),
        clean(&json_string(obra, "URL").unwrap_or_default()),
        clean(pdfs.first().map(String::as_str).unwrap_or("")),
        clean(&pdfs.join(";")),
        clean(&resumen),
        published,
        json_number(obra, "is-referenced-by-count").unwrap_or_default(),
    ))
}

// date-parts trae [[año, mes, día]], con mes y día opcionales. Solo cuenta el
// primer arreglo interno, y una parte fuera de rango corta la fecha ahí.
fn fecha(objeto: &str) -> String {
    let partes = match json_array(objeto, "date-parts") {
        Some(partes) => partes,
        None => return String::new(),
    };
    let primera = partes.trim_start_matches(|c: char| c == '[' || c.is_whitespace()).split(']').next().unwrap_or("");
    let numeros: Vec<u32> = primera.split(',').filter_map(|parte| parte.trim().parse().ok()).collect();
    match numeros.as_slice() {
        [anio, ..] if *anio > 9999 => String::new(),
        [anio, mes, ..] if !(1..=12).contains(mes) => format!("{anio:04}"),
        [anio, mes, dia, ..] if (1..=31).contains(dia) => format!("{anio:04}-{mes:02}-{dia:02}"),
        [anio, mes, ..] => format!("{anio:04}-{mes:02}"),
        [anio] => format!("{anio:04}"),
        [] => String::new(),
    }
}

// Crossref no declara acceso abierto, solo licencias. Una Creative Commons
// cuenta como apertura; cualquier otra (las de TDM o de copyright de la
// editorial que trae has-license:true) queda como desconocida.
fn licencia(obra: &str) -> (String, bool) {
    let mut primera = String::new();
    let mut abierta = String::new();
    if let Some(licencias) = json_array(obra, "license") {
        for entrada in json_entries(licencias) {
            let url = json_string(entrada, "URL").unwrap_or_default();
            if url.is_empty() { continue; }
            if primera.is_empty() { primera = url.clone(); }
            let cc = url.contains("creativecommons.org/");
            let vor = json_string(entrada, "content-version").as_deref() == Some("vor");
            if cc && (abierta.is_empty() || vor) { abierta = url; }
        }
    }
    if abierta.is_empty() { (primera, false) } else { (abierta, true) }
}

// Primero los enlaces declarados como PDF y después los 'unspecified', que a
// veces también lo son; el descargador prueba pdf_urls en orden. Los de
// similarity-checking van al final porque son para el servicio antiplagio de
// Crossref y no siempre abren sin credencial.
fn enlaces(obra: &str) -> (Vec<String>, String) {
    let mut candidatos: Vec<(u8, String, String)> = Vec::new();
    if let Some(links) = json_array(obra, "link") {
        for link in json_entries(links) {
            let url = json_string(link, "URL").unwrap_or_default();
            let tipo = json_string(link, "content-type").unwrap_or_default();
            if url.is_empty() || candidatos.iter().any(|(_, otra, _)| *otra == url) { continue; }
            let antiplagio = json_string(link, "intended-application").as_deref() == Some("similarity-checking");
            let prioridad = match (tipo.as_str(), antiplagio) {
                ("application/pdf", false) => 0,
                ("application/pdf", true) => 2,
                ("unspecified", false) => 1,
                ("unspecified", true) => 3,
                _ => continue,
            };
            candidatos.push((prioridad, url, json_string(link, "content-version").unwrap_or_default()));
        }
    }
    candidatos.sort_by_key(|candidato| candidato.0);
    let version = candidatos.first().map(|candidato| candidato.2.clone()).unwrap_or_default();
    (candidatos.into_iter().map(|candidato| candidato.1).collect(), version)
}

// Títulos y abstracts traen marcado JATS o HTML (<jats:p>, <scp>) y entidades.
// Solo las etiquetas de bloque separan palabras: <scp>LLM</scp>s es una sola.
// Algunas editoriales meten HTML codificado dentro del JATS (&lt;p&gt;), por
// eso se quitan etiquetas antes y después de desescapar.
fn texto_plano(valor: &str) -> String {
    let texto = quitar_etiquetas(valor);
    let texto = texto.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&");
    quitar_etiquetas(&texto).split_whitespace().collect::<Vec<_>>().join(" ")
}

// Un '<' que no abre una etiqueta (a < b, x<0) o que no cierra se conserva.
fn quitar_etiquetas(valor: &str) -> String {
    let mut texto = String::new();
    let mut etiqueta: Option<String> = None;
    let mut chars = valor.chars().peekable();
    while let Some(c) = chars.next() {
        match (&mut etiqueta, c) {
            (None, '<') if chars.peek().map_or(false, |sig| sig.is_ascii_alphabetic() || *sig == '/') => etiqueta = Some(String::new()),
            (Some(nombre), '>') => {
                let nombre = nombre.trim_start_matches('/').split_whitespace().next().unwrap_or("").to_string();
                if matches!(nombre.trim_start_matches("jats:"), "p" | "title" | "sec" | "br" | "br/" | "list-item") {
                    texto.push(' ');
                }
                etiqueta = None;
            }
            (Some(nombre), _) => nombre.push(c),
            (None, _) => texto.push(c),
        }
    }
    if let Some(resto) = etiqueta {
        texto.push('<');
        texto.push_str(&resto);
    }
    texto
}

fn fecha_valida(valor: &str) -> bool {
    let bytes = valor.as_bytes();
    let forma = bytes.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-' && bytes.iter().enumerate().all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit());
    forma && valor[5..7].parse().map_or(false, |mes: u32| (1..=12).contains(&mes)) && valor[8..10].parse().map_or(false, |dia: u32| (1..=31).contains(&dia))
}

fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
