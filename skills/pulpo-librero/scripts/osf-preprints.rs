// El lector canónico se copia entero aunque este scraper use sólo parte.
#![allow(dead_code)]

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

include!("json_api.rs");

const ENCABEZADO: &str = "id\ttitle\tdoi\tyear\tis_oa\toa_status\tlicense\tversion\tlanding_url\tpdf_url\tpdf_urls\tabstract\tpublished\tcited_by";

const PROVEEDORES: &str = "psyarxiv,socarxiv,metaarxiv";

const API: &str = "https://api.osf.io/v2";

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 || args[1] != "search" {
        usage();
    }
    let mut query = None;
    let mut exact_id = None;
    let mut out = None;
    let mut desde = None;
    let mut max = None;
    let mut max_paginas = None;
    let mut proveedores = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--query" if i + 1 < args.len() => { query = Some(args[i + 1].clone()); i += 2; }
            "--id" if i + 1 < args.len() => { exact_id = Some(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            "--desde" if i + 1 < args.len() => { desde = Some(fecha_valida(&args[i + 1])); i += 2; }
            "--max" if i + 1 < args.len() => { max = Some(entero(&args[i + 1], "--max")); i += 2; }
            "--max-paginas" if i + 1 < args.len() => { max_paginas = Some(entero(&args[i + 1], "--max-paginas")); i += 2; }
            "--proveedor" if i + 1 < args.len() => { proveedores = Some(args[i + 1].clone()); i += 2; }
            // OSF no tiene polite pool: se acepta por uniformidad y no se envía.
            "--mailto" if i + 1 < args.len() => { i += 2; }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    if query.is_some() == exact_id.is_some() { eprintln!("error: usa exactamente uno de --query o --id"); std::process::exit(2); }
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    if exact_id.is_some() && (desde.is_some() || max.is_some() || max_paginas.is_some() || proveedores.is_some()) {
        eprintln!("error: --desde, --max, --max-paginas y --proveedor solo valen con --query");
        std::process::exit(2);
    }
    let max = max.unwrap_or(25);
    let max_paginas = max_paginas.unwrap_or(10);
    let proveedores = proveedores.unwrap_or_else(|| PROVEEDORES.to_string());
    let raw_path = format!("{out}.raw.jsonl");
    let _ = fs::remove_file(&raw_path);
    let parcial = format!("{out}.raw.part");
    let mut licencias: HashMap<String, String> = HashMap::new();

    if let Some(id) = exact_id {
        let id = id.trim().trim_start_matches("osf-preprints:").to_string();
        if id.is_empty() { eprintln!("error: --id está vacío"); std::process::exit(2); }
        let raw = consultar(&format!("{API}/preprints/{}/", encode(&id)), &parcial, &raw_path).unwrap_or_else(|err| { eprintln!("error: {err}"); std::process::exit(1); });
        let obra = json_object(&raw, "data").unwrap_or_else(|| { eprintln!("error: la respuesta de OSF no trae 'data'"); std::process::exit(1); });
        let mut file = crear(&out);
        let mut count = 0;
        if let Some((_, fila)) = fila_tsv(obra, &mut licencias, &parcial, &raw_path) {
            writeln!(file, "{fila}").unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
            count += 1;
        }
        let _ = fs::remove_file(&parcial);
        println!("osf-preprints: {count} obras extraídas; crudo={raw_path}; resultados={out}");
        return;
    }

    let query = query.unwrap();
    let query = query.split_whitespace().collect::<Vec<_>>().join(" ");
    if query.is_empty() { eprintln!("error: --query está vacío"); std::process::exit(2); }
    let proveedores: Vec<String> = proveedores.split(',').map(|valor| valor.trim().to_lowercase()).filter(|valor| !valor.is_empty()).collect();
    if proveedores.is_empty() { eprintln!("error: --proveedor está vacío"); std::process::exit(2); }
    let por_pagina = max.clamp(10, 100);

    // q= se ignora en silencio y filter[title] es la única búsqueda por texto
    // que respeta la API: coincide con la frase completa dentro del título, sin
    // distinguir mayúsculas. Un filtro de proveedor con varios valores separados
    // por coma devuelve cero obras, así que se consulta uno por uno.
    let mut filas: Vec<(String, String)> = Vec::new();
    let mut revisados = 0;
    let mut paginas = 0;
    let mut totales = Vec::new();
    for proveedor in &proveedores {
        let mut url = format!(
            "{API}/preprints/?filter%5Bprovider%5D={}&filter%5Btitle%5D={}&sort=-date_published&page%5Bsize%5D={por_pagina}",
            encode(proveedor),
            encode(&query)
        );
        if let Some(fecha) = &desde {
            url.push_str(&format!("&filter%5Bdate_published%5D%5Bgte%5D={fecha}"));
        }
        let mut propias = 0;
        let mut siguiente = Some(url);
        let mut paginas_proveedor = 0;
        while let Some(actual) = siguiente.take() {
            if propias >= max || paginas_proveedor >= max_paginas {
                break;
            }
            let raw = consultar(&actual, &parcial, &raw_path).unwrap_or_else(|err| { eprintln!("error: {err}"); std::process::exit(1); });
            paginas += 1;
            paginas_proveedor += 1;
            let enlaces = json_object(&raw, "links").unwrap_or("");
            if paginas_proveedor == 1 {
                let total = json_object(enlaces, "meta").and_then(|meta| json_number(meta, "total")).unwrap_or_else(|| "?".to_string());
                totales.push(format!("{proveedor}={total}"));
            }
            let obras = json_array(&raw, "data").unwrap_or_else(|| { eprintln!("error: la respuesta de OSF no trae 'data'"); std::process::exit(1); });
            for obra in json_entries(obras) {
                revisados += 1;
                let atributos = json_object(obra, "attributes").unwrap_or("");
                // Cada versión es una obra aparte en la lista y comparte título
                // con las demás: se conserva solo la vigente.
                if json_bool(atributos, "is_latest_version") == Some(false) { continue; }
                if json_string(atributos, "date_withdrawn").is_some() { continue; }
                if let Some(fila) = fila_tsv(obra, &mut licencias, &parcial, &raw_path) {
                    filas.push(fila);
                    propias += 1;
                    if propias >= max { break; }
                }
            }
            siguiente = json_string(enlaces, "next");
        }
    }
    let _ = fs::remove_file(&parcial);
    filas.sort_by(|a, b| b.0.cmp(&a.0));
    filas.truncate(max);
    let mut file = crear(&out);
    for (_, fila) in &filas {
        writeln!(file, "{fila}").unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    }
    println!(
        "osf-preprints: {} obras extraídas de {revisados} registros en {paginas} página(s); total por proveedor {}; crudo={raw_path}; resultados={out}",
        filas.len(),
        totales.join(",")
    );
}

fn usage() -> ! {
    eprintln!("uso: osf-preprints search --query TEXTO --out RESULTADOS.tsv [--desde YYYY-MM-DD] [--max N] [--max-paginas N] [--proveedor psyarxiv,socarxiv,metaarxiv]");
    eprintln!("     osf-preprints search --id ID --out RESULTADOS.tsv");
    std::process::exit(2);
}

fn fecha_valida(valor: &str) -> String {
    let bytes = valor.as_bytes();
    let forma = bytes.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-' && bytes.iter().enumerate().all(|(pos, b)| pos == 4 || pos == 7 || b.is_ascii_digit());
    let numero = |rango: std::ops::Range<usize>| valor.get(rango).and_then(|parte| parte.parse::<u32>().ok()).unwrap_or(0);
    let (anio, mes, dia) = (numero(0..4), numero(5..7), numero(8..10));
    let bisiesto = anio % 4 == 0 && (anio % 100 != 0 || anio % 400 == 0);
    let dias_mes = match mes { 1 | 3 | 5 | 7 | 8 | 10 | 12 => 31, 4 | 6 | 9 | 11 => 30, 2 if bisiesto => 29, 2 => 28, _ => 0 };
    if !forma || dia == 0 || dia > dias_mes { eprintln!("error: --desde debe ser una fecha válida YYYY-MM-DD: {valor}"); std::process::exit(2); }
    valor.to_string()
}

fn entero(valor: &str, flag: &str) -> usize {
    match valor.parse::<usize>() {
        Ok(numero) if numero > 0 => numero,
        _ => { eprintln!("error: {flag} debe ser un entero positivo"); std::process::exit(2); }
    }
}

fn crear(out: &str) -> fs::File {
    let mut file = fs::File::create(out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "{ENCABEZADO}").unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    file
}

fn consultar(url: &str, destino: &str, crudo: &str) -> Result<String, String> {
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--retry-max-time", "120", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--output"])
        .arg(destino)
        .arg(url)
        .status();
    thread::sleep(Duration::from_secs(3));
    match result {
        Ok(status) if status.success() => {}
        Ok(_) => return Err("la API de OSF no respondió correctamente".to_string()),
        Err(err) => return Err(format!("no se pudo ejecutar curl: {err}")),
    }
    let raw = fs::read_to_string(destino).map_err(|err| format!("no se pudo leer {destino}: {err}"))?;
    let mut archivo = fs::OpenOptions::new().create(true).append(true).open(crudo).map_err(|err| format!("no se pudo abrir {crudo}: {err}"))?;
    writeln!(archivo, "{}", raw.replace('\n', "").replace('\r', "")).map_err(|err| format!("no se pudo escribir {crudo}: {err}"))?;
    Ok(raw)
}

// La lista solo trae el id de la licencia; el nombre exige una petición por
// licencia distinta, que se guarda para no repetirla. Si falla, la columna
// queda vacía en vez de abortar la corrida.
fn licencia(id: &str, cache: &mut HashMap<String, String>, parcial: &str, crudo: &str) -> String {
    if let Some(nombre) = cache.get(id) {
        return nombre.clone();
    }
    let nombre = match consultar(&format!("{API}/licenses/{}/", encode(id)), parcial, crudo) {
        Ok(raw) => json_object(&raw, "data").and_then(|data| json_object(data, "attributes")).and_then(|atributos| json_string(atributos, "name")).unwrap_or_default(),
        Err(err) => { eprintln!("advertencia: licencia {id} sin nombre: {err}"); String::new() }
    };
    cache.insert(id.to_string(), nombre.clone());
    nombre
}

fn id_relacion(relaciones: &str, clave: &str) -> Option<String> {
    json_object(relaciones, clave).and_then(|relacion| json_object(relacion, "data")).and_then(|data| json_string(data, "id"))
}

// attributes.doi es el DOI del artículo publicado después y casi siempre
// viene en null; el del preprint, con sufijo de versión, está en
// links.preprint_doi. La API no expone citas ni descargas en la lista, así que
// cited_by queda vacío. pdf_url es /download/<id del archivo primario>/, que
// redirige a files.osf.io y de ahí a un bucket de otro host. Ese archivo es a
// menudo un .docx y la lista no trae su nombre ni su tipo.
fn fila_tsv(obra: &str, licencias: &mut HashMap<String, String>, parcial: &str, crudo: &str) -> Option<(String, String)> {
    let id = json_string(obra, "id").unwrap_or_default();
    let atributos = json_object(obra, "attributes").unwrap_or("");
    let title = normalize(&json_string(atributos, "title").unwrap_or_default());
    if id.is_empty() || title.is_empty() {
        return None;
    }
    let enlaces = json_object(obra, "links").unwrap_or("");
    let relaciones = json_object(obra, "relationships").unwrap_or("");
    let doi = normalizar_doi(&json_string(enlaces, "preprint_doi").unwrap_or_default());
    let published = json_string(atributos, "date_published").unwrap_or_default().get(0..10).unwrap_or("").to_string();
    let version = json_number(atributos, "version").or_else(|| json_string(atributos, "version")).map(|valor| format!("v{valor}")).unwrap_or_default();
    let license = id_relacion(relaciones, "license").map(|id| licencia(&id, licencias, parcial, crudo)).unwrap_or_default();
    let abierta = license.to_ascii_uppercase().starts_with("CC");
    let (is_oa, oa_status) = if abierta { ("true", "abierto") } else { ("", "desconocido") };
    let pdf_url = id_relacion(relaciones, "primary_file").map(|archivo| format!("https://osf.io/download/{archivo}/")).unwrap_or_default();
    let fila = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t",
        clean(&format!("osf-preprints:{id}")),
        clean(&title),
        clean(&doi),
        clean(published.get(0..4).unwrap_or("")),
        is_oa,
        oa_status,
        clean(&license),
        clean(&version),
        clean(&json_string(enlaces, "html").unwrap_or_default()),
        clean(&pdf_url),
        clean(&pdf_url),
        clean(&normalize(&json_string(atributos, "description").unwrap_or_default())),
        clean(&published),
    );
    Some((json_string(atributos, "date_published").unwrap_or_default(), fila))
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

fn normalize(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
