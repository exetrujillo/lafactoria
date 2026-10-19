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

const ARTICLEMETA: &str = "https://articlemeta.scielo.org/api/v1";

const POR_PAGINA: usize = 100;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) != Some("search") {
        usage();
    }
    let mut query = None;
    let mut ids: Vec<String> = Vec::new();
    let mut out = None;
    let mut colecciones: Vec<String> = Vec::new();
    let mut desde = None;
    let mut issn = None;
    let mut max = 25usize;
    let mut revisar = 150usize;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--query" if i + 1 < args.len() => { query = Some(args[i + 1].clone()); i += 2; }
            "--id" if i + 1 < args.len() => { ids.push(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            "--coleccion" if i + 1 < args.len() => {
                colecciones.extend(args[i + 1].split(',').map(str::trim).filter(|c| !c.is_empty()).map(String::from));
                i += 2;
            }
            "--desde" if i + 1 < args.len() => { desde = Some(args[i + 1].clone()); i += 2; }
            "--issn" if i + 1 < args.len() => { issn = Some(args[i + 1].clone()); i += 2; }
            "--max" if i + 1 < args.len() => { max = entero(&args[i + 1], "--max"); i += 2; }
            "--revisar" if i + 1 < args.len() => { revisar = entero(&args[i + 1], "--revisar"); i += 2; }
            // SciELO no tiene polite pool; se acepta para que la llamada sea la misma que en otros scrapers.
            "--mailto" if i + 1 < args.len() => { i += 2; }
            _ => { eprintln!("argumento desconocido: {}", args[i]); std::process::exit(2); }
        }
    }
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    if query.is_some() == !ids.is_empty() { eprintln!("error: usa --query o al menos un --id, no ambos"); std::process::exit(2); }
    if colecciones.is_empty() { eprintln!("error: falta --coleccion (por ejemplo chl, mex, col, arg, esp, per, scl)"); std::process::exit(2); }
    if !ids.is_empty() && colecciones.len() != 1 { eprintln!("error: --id exige una sola --coleccion"); std::process::exit(2); }
    if let Some(fecha) = &desde {
        if !fecha_valida(fecha) { eprintln!("error: --desde debe tener la forma YYYY-MM-DD"); std::process::exit(2); }
    }
    let terminos = match &query {
        Some(texto) => {
            let terminos: Vec<String> = plegar(texto).split_whitespace().map(String::from).collect();
            if terminos.is_empty() { eprintln!("error: --query está vacío"); std::process::exit(2); }
            terminos
        }
        None => Vec::new(),
    };
    if max == 0 || revisar == 0 { eprintln!("error: --max y --revisar deben ser mayores que cero"); std::process::exit(2); }

    let raw_path = format!("{out}.raw.jsonl");
    let _ = fs::remove_file(&raw_path);
    let parcial = format!("{out}.raw.part");
    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    escribir(&mut file, ENCABEZADO, &out, &parcial);
    let mut filas = 0;
    let mut revisados = 0;
    let mut fuera_de_fecha = 0;
    let mut fallidos = 0;

    let candidatos: Vec<(String, String, String)> = if ids.is_empty() {
        let cupo = revisar.div_ceil(colecciones.len());
        let mut listas = Vec::new();
        for coleccion in &colecciones {
            match listar(coleccion, desde.as_deref(), issn.as_deref(), cupo, &parcial) {
                Ok(codigos) => listas.push(codigos.into_iter().map(|(codigo, doi)| (coleccion.clone(), codigo, doi)).collect::<Vec<_>>()),
                Err(err) => salir(&err, &parcial),
            }
        }
        // Se alternan las colecciones para que --max no se llene con la primera.
        let mut todos = Vec::new();
        for posicion in 0..cupo {
            todos.extend(listas.iter().filter_map(|lista| lista.get(posicion).cloned()));
        }
        todos
    } else {
        ids.iter().map(|id| (colecciones[0].clone(), id.clone(), String::new())).collect()
    };

    for (coleccion, codigo, doi) in &candidatos {
        if filas >= max {
            break;
        }
        let url = format!("{ARTICLEMETA}/article/?code={}&collection={}", encode(codigo), encode(coleccion));
        let raw = match consultar(&url, &parcial) {
            Ok(raw) => raw,
            Err(err) => { eprintln!("advertencia: {coleccion}:{codigo}: {err}"); fallidos += 1; continue; }
        };
        let Some(mut obra) = Obra::leer(&raw) else {
            eprintln!("advertencia: {coleccion}:{codigo}: la respuesta no trae 'article'");
            fallidos += 1;
            continue;
        };
        revisados += 1;
        if obra.doi.is_empty() {
            obra.doi = doi.trim_start_matches("https://doi.org/").to_string();
        }
        if let Some(fecha) = &desde {
            if !posterior(&obra.published, fecha) {
                fuera_de_fecha += 1;
                continue;
            }
        }
        if !terminos.is_empty() && !coincide(&obra, &terminos) {
            continue;
        }
        if let Err(err) = guardar_crudo(&raw_path, &raw) {
            salir(&err, &parcial);
        }
        let cited_by = citas(codigo, &parcial);
        escribir(&mut file, &obra.fila(coleccion, codigo, &cited_by), &out, &parcial);
        filas += 1;
    }
    let _ = fs::remove_file(&parcial);
    if revisados == 0 && fallidos > 0 {
        eprintln!("error: SciELO no devolvió ninguna obra");
        std::process::exit(1);
    }
    println!(
        "scielo: {filas} obras escritas; {revisados} revisadas, {fuera_de_fecha} publicadas antes de --desde, {fallidos} fallidas; crudo={raw_path}; resultados={out}"
    );
}

fn usage() -> ! {
    eprintln!("uso: scielo search (--query TEXTO | --id CODIGO ...) --coleccion CODIGO[,CODIGO...] --out RESULTADOS.tsv [--desde YYYY-MM-DD] [--issn ISSN] [--max N] [--revisar N]");
    std::process::exit(2);
}

fn salir(mensaje: &str, parcial: &str) -> ! {
    let _ = fs::remove_file(parcial);
    eprintln!("error: {mensaje}");
    std::process::exit(1);
}

fn escribir(file: &mut fs::File, linea: &str, out: &str, parcial: &str) {
    if let Err(err) = writeln!(file, "{linea}") {
        salir(&format!("no se pudo escribir {out}: {err}"), parcial);
    }
}

fn entero(valor: &str, nombre: &str) -> usize {
    valor.parse().unwrap_or_else(|_| { eprintln!("error: {nombre} no es entero"); std::process::exit(2); })
}

// ArticleMeta no busca por texto y filtra por fecha de procesamiento, no de
// publicación. Devuelve los códigos en orden ascendente de procesamiento, así
// que se recorre desde la última página para revisar primero lo más reciente.
fn listar(coleccion: &str, desde: Option<&str>, issn: Option<&str>, cupo: usize, parcial: &str) -> Result<Vec<(String, String)>, String> {
    let mut base = format!("{ARTICLEMETA}/article/identifiers/?collection={}", encode(coleccion));
    if let Some(fecha) = desde { base.push_str("&from="); base.push_str(&encode(fecha)); }
    if let Some(issn) = issn { base.push_str("&issn="); base.push_str(&encode(issn)); }
    let raw = consultar(&format!("{base}&limit=1&offset=0"), parcial)?;
    let total: usize = json_object(&raw, "meta")
        .and_then(|meta| json_number(meta, "total"))
        .and_then(|total| total.parse().ok())
        .ok_or_else(|| format!("la lista de {coleccion} no trae 'meta.total'"))?;
    let mut codigos = Vec::new();
    let mut fin = total;
    while fin > 0 && codigos.len() < cupo {
        let inicio = fin.saturating_sub(POR_PAGINA.min(cupo - codigos.len()));
        let raw = consultar(&format!("{base}&limit={}&offset={inicio}", fin - inicio), parcial)?;
        let objetos = json_array(&raw, "objects").ok_or_else(|| format!("la lista de {coleccion} no trae 'objects'"))?;
        for objeto in json_entries(objetos).into_iter().rev() {
            if let Some(codigo) = json_string(objeto, "code") {
                if !codigos.iter().any(|(visto, _)| *visto == codigo) {
                    codigos.push((codigo, json_string(objeto, "doi").unwrap_or_default()));
                }
            }
            if codigos.len() >= cupo {
                break;
            }
        }
        fin = inicio;
    }
    Ok(codigos)
}

struct Obra {
    titulos: Vec<String>,
    resumenes: Vec<String>,
    palabras: Vec<String>,
    doi: String,
    published: String,
    pdfs: Vec<String>,
    landing_url: String,
    license: String,
}

impl Obra {
    // El registro usa las etiquetas ISIS de SciELO: v40 idioma original, v12
    // título, v83 resumen y v85 palabras clave, con una entrada por idioma. El
    // texto en el idioma original va primero.
    fn leer(raw: &str) -> Option<Obra> {
        let article = json_object(raw, "article")?;
        let idioma = json_array(article, "v40").and_then(|lista| json_entries(lista).first().and_then(|entrada| json_string(entrada, "_"))).unwrap_or_default();
        let textos = |etiqueta: &str, clave: &str| -> Vec<String> {
            let mut lista: Vec<(bool, String)> = json_array(article, etiqueta)
                .map(|lista| json_entries(lista).into_iter().filter_map(|entrada| json_string(entrada, clave).map(|texto| (json_string(entrada, "l").unwrap_or_default() != idioma, normalize(&texto)))).filter(|par| !par.1.is_empty()).collect())
                .unwrap_or_default();
            lista.sort_by_key(|par| par.0);
            lista.into_iter().map(|par| par.1).collect()
        };
        let doi = json_string(raw, "doi")
            .or_else(|| json_array(article, "v237").and_then(|lista| json_entries(lista).first().and_then(|entrada| json_string(entrada, "_"))))
            .unwrap_or_default();
        let mut pdfs = Vec::new();
        let mut landing_url = String::new();
        if let Some(fulltexts) = json_object(raw, "fulltexts") {
            if let Some(pdf) = json_object(fulltexts, "pdf") {
                pdfs = valores_por_idioma(pdf);
            }
            if let Some(html) = json_object(fulltexts, "html") {
                landing_url = valores_por_idioma(html).into_iter().next().unwrap_or_default();
            }
        }
        // La licencia la declara la revista (v541 del registro "title"), no el artículo.
        let license = json_object(raw, "title")
            .and_then(|revista| json_array(revista, "v541"))
            .and_then(|lista| json_entries(lista).first().and_then(|entrada| json_string(entrada, "_")))
            .map(|valor| format!("cc-{}", valor.trim().to_lowercase()))
            .unwrap_or_default();
        Some(Obra {
            titulos: textos("v12", "_"),
            resumenes: textos("v83", "a"),
            palabras: textos("v85", "k"),
            doi: doi.trim().trim_start_matches("https://doi.org/").to_string(),
            published: fecha(&json_string(raw, "publication_date").unwrap_or_default()),
            pdfs,
            landing_url,
            license,
        })
    }

    fn fila(&self, coleccion: &str, codigo: &str, cited_by: &str) -> String {
        let pdf_url = self.pdfs.first().cloned().unwrap_or_default();
        let (is_oa, oa_status) = if self.license.is_empty() { ("", "") } else { ("true", "abierto") };
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            clean(&format!("scielo:{coleccion}:{codigo}")),
            clean(self.titulos.first().map(String::as_str).unwrap_or("")),
            clean(&self.doi),
            clean(self.published.get(0..4).unwrap_or("")),
            is_oa,
            oa_status,
            clean(&self.license),
            "",
            clean(&self.landing_url),
            clean(&pdf_url),
            clean(&self.pdfs.join(";")),
            clean(self.resumenes.first().map(String::as_str).unwrap_or("")),
            clean(&self.published),
            clean(cited_by),
        )
    }
}

// fulltexts trae un objeto {idioma: url}; el orden de la respuesta se conserva.
fn valores_por_idioma(objeto: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let bytes = objeto.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        if bytes[i] != b'"' { i += 1; continue; }
        let Some((clave, after)) = json_string_at(objeto, i) else { break };
        if let Some(url) = json_string(objeto, &clave) {
            if !url.is_empty() && !urls.contains(&url) {
                urls.push(url);
            }
        }
        let mut j = after;
        while j < bytes.len() && bytes[j] != b',' && bytes[j] != b'}' {
            if bytes[j] == b'"' {
                j = match json_string_at(objeto, j) { Some((_, fin)) => fin, None => bytes.len() };
            } else {
                j += 1;
            }
        }
        i = j + 1;
    }
    urls
}

// Citedby sólo cuenta citas desde otros documentos de SciELO. Si no responde,
// la celda queda vacía: un cero sería un dato inventado.
fn citas(codigo: &str, parcial: &str) -> String {
    let url = format!("https://citedby.scielo.org/api/v1/pid/?q={}", encode(codigo));
    match consultar(&url, parcial) {
        Ok(raw) => match json_array(&raw, "cited_by") {
            Some(lista) => json_entries(lista).len().to_string(),
            None => String::new(),
        },
        Err(err) => { eprintln!("advertencia: citedby {codigo}: {err}"); String::new() }
    }
}

fn coincide(obra: &Obra, terminos: &[String]) -> bool {
    let texto = plegar(&[obra.titulos.join(" "), obra.resumenes.join(" "), obra.palabras.join(" ")].join(" "));
    terminos.iter().all(|termino| texto.contains(termino.as_str()))
}

// Minúsculas y sin tildes, para que "educacion" encuentre "Educação" y "educación".
fn plegar(texto: &str) -> String {
    texto
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            otro => otro,
        })
        .collect()
}

// publication_date llega como "2024-10" o "2024-10-00"; los ceros de relleno
// no son un día ni un mes.
fn fecha(valor: &str) -> String {
    let partes: Vec<&str> = valor.trim().split('-').take_while(|parte| !parte.chars().all(|c| c == '0')).collect();
    partes.join("-")
}

// Una fecha parcial cuenta como posterior si su mes o año contiene a --desde.
fn posterior(published: &str, desde: &str) -> bool {
    !published.is_empty() && published >= desde.get(0..published.len()).unwrap_or(desde)
}

fn fecha_valida(valor: &str) -> bool {
    let bytes = valor.as_bytes();
    bytes.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-' && bytes.iter().enumerate().all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
}

fn consultar(url: &str, destino: &str) -> Result<String, String> {
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--connect-timeout", "20", "--max-time", "60", "--user-agent", "pulpo-librero/0.1", "--output"])
        .arg(destino)
        .arg(url)
        .status();
    thread::sleep(Duration::from_secs(3));
    match result {
        Ok(status) if status.success() => fs::read_to_string(destino).map_err(|err| format!("no se pudo leer {destino}: {err}")),
        Ok(_) => Err("SciELO no respondió correctamente".to_string()),
        Err(err) => Err(format!("no se pudo ejecutar curl: {err}")),
    }
}

fn guardar_crudo(destino: &str, raw: &str) -> Result<(), String> {
    let mut archivo = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(destino)
        .map_err(|err| format!("no se pudo abrir {destino}: {err}"))?;
    writeln!(archivo, "{}", raw.replace('\n', "").replace('\r', "")).map_err(|err| format!("no se pudo escribir {destino}: {err}"))
}

fn normalize(value: &str) -> String { value.split_whitespace().collect::<Vec<_>>().join(" ") }
fn encode(value: &str) -> String { value.bytes().map(|b| match b { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(), other => format!("%{other:02X}") }).collect() }
fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }
