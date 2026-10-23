use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

const ENCABEZADO: &str = "id\ttitle\tdoi\tyear\tis_oa\toa_status\tlicense\tversion\tlanding_url\tpdf_url\tpdf_urls\tabstract\tpublished\tcited_by\taward";

const SITIO: &str = "https://aclanthology.org";
const XML_POR_COLECCION: &str = "https://raw.githubusercontent.com/acl-org/acl-anthology/master/data/xml";
const TARBALL: &str = "https://codeload.github.com/acl-org/acl-anthology/tar.gz/refs/heads/master";

// La deja la extracción del tarball. Sin ella, el directorio puede tener solo
// las colecciones que se bajaron una por una.
const COMPLETO: &str = ".completo";

// Medido sobre COLING 2025 y LREC-COLING 2024: desde 0,9 hacia arriba todos
// los emparejamientos revisados eran la misma obra; por debajo aparecen otras.
const UMBRAL: f64 = 0.9;

struct Obra {
    id: String,
    title: String,
    doi: String,
    year: String,
    published: String,
    estado: &'static str,
    version: String,
    landing_url: String,
    pdf_url: String,
    abstract_text: String,
    award: String,
    venues: Vec<String>,
}

struct Preprint {
    id: String,
    base: String,
    version: u32,
    titulo: String,
    pdf_url: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Par {
    Ninguno,
    Ambiguo,
    Con(usize, f64),
}

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("search") => run_search(&args[2..]),
        Some("enlazar") => run_enlazar(&args[2..]),
        _ => usage(),
    }
}

fn usage() -> ! {
    eprintln!("uso: acl-anthology search --datos DIR --out RESULTADOS.tsv (--query TEXTO | --id ID) [--coleccion LISTA] [--venue LISTA] [--desde YYYY-MM-DD] [--max N]");
    eprintln!("     acl-anthology enlazar --catalogo CATALOGO.tsv --arxiv ARXIV.tsv [--arxiv ARXIV2.tsv ...] --out ENLACES.tsv");
    eprintln!("--datos: directorio de la corrida donde se baja y se reusa el XML de la Anthology");
    eprintln!("--query: alternativas separadas por \" OR \"; una alternativa exige todas sus palabras como subcadenas, sin importar el orden, en título o resumen");
    std::process::exit(2);
}

fn run_search(args: &[String]) {
    let mut datos = None;
    let mut out = None;
    let mut query = None;
    let mut ids: Vec<String> = Vec::new();
    let mut colecciones: Vec<String> = Vec::new();
    let mut venues: Vec<String> = Vec::new();
    let mut desde = None;
    let mut max = 25usize;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--datos" if i + 1 < args.len() => { datos = Some(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            "--query" if i + 1 < args.len() => { query = Some(args[i + 1].clone()); i += 2; }
            "--id" if i + 1 < args.len() => { ids.push(args[i + 1].trim().trim_start_matches("acl-anthology:").to_string()); i += 2; }
            "--coleccion" if i + 1 < args.len() => { colecciones.extend(lista(&args[i + 1])); i += 2; }
            "--venue" if i + 1 < args.len() => { venues.extend(lista(&args[i + 1]).into_iter().map(|venue| venue.to_lowercase())); i += 2; }
            "--desde" if i + 1 < args.len() => { desde = Some(args[i + 1].clone()); i += 2; }
            "--max" if i + 1 < args.len() => {
                max = args[i + 1].parse().unwrap_or_else(|_| { eprintln!("error: --max no es entero"); std::process::exit(2); });
                i += 2;
            }
            _ => { eprintln!("error: argumento desconocido o sin valor: {}", args[i]); std::process::exit(2); }
        }
    }
    if query.is_some() == !ids.is_empty() { eprintln!("error: usa --query o al menos un --id, no ambos"); std::process::exit(2); }
    let datos = PathBuf::from(datos.unwrap_or_else(|| { eprintln!("error: falta --datos"); std::process::exit(2); }));
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    let alternativas = query.as_deref().map(terminos).unwrap_or_default();
    if query.is_some() && alternativas.is_empty() { eprintln!("error: --query está vacía"); std::process::exit(2); }
    if let Some(fecha) = &desde {
        if !fecha_valida(fecha) { eprintln!("error: --desde debe ser YYYY-MM-DD"); std::process::exit(2); }
    }
    if let Some(malo) = ids.iter().chain(colecciones.iter()).find(|nombre| !nombre_seguro(nombre)) {
        eprintln!("error: identificador o colección no válido: {malo}");
        std::process::exit(2);
    }
    if let Err(err) = fs::create_dir_all(&datos) { eprintln!("error: no se pudo crear {}: {err}", datos.display()); std::process::exit(1); }

    let mut pedidas = colecciones.clone();
    for id in &ids {
        let coleccion = coleccion_de(id);
        if !pedidas.contains(&coleccion) { pedidas.push(coleccion); }
    }
    let archivos: Vec<PathBuf> = if pedidas.is_empty() {
        asegurar_completo(&datos);
        let mut todos: Vec<PathBuf> = fs::read_dir(&datos)
            .unwrap_or_else(|err| { eprintln!("error: no se pudo leer {}: {err}", datos.display()); std::process::exit(1); })
            .filter_map(|entrada| entrada.ok().map(|entrada| entrada.path()))
            .filter(|ruta| ruta.extension().map(|ext| ext == "xml").unwrap_or(false))
            .collect();
        todos.sort();
        todos
    } else {
        pedidas.iter().filter_map(|coleccion| asegurar_coleccion(&datos, coleccion)).collect()
    };

    let mut revisadas = 0;
    let mut elegidas: Vec<Obra> = Vec::new();
    for archivo in &archivos {
        let xml = match fs::read_to_string(archivo) {
            Ok(xml) => xml,
            Err(err) => { eprintln!("advertencia: no se pudo leer {}: {err}", archivo.display()); continue; }
        };
        for obra in obras_de(&xml) {
            revisadas += 1;
            if !ids.is_empty() && !ids.contains(&obra.id) { continue; }
            if !venues.is_empty() && !obra.venues.iter().any(|venue| venues.contains(venue)) { continue; }
            if !pasa_fecha(&obra.published, desde.as_deref()) { continue; }
            if !alternativas.is_empty() && !coincide(&format!("{} {}", obra.title, obra.abstract_text), &alternativas) { continue; }
            elegidas.push(obra);
        }
    }
    for id in &ids {
        if !elegidas.iter().any(|obra| &obra.id == id) { eprintln!("advertencia: {id} no está en la Anthology"); }
    }
    elegidas.sort_by(|a, b| b.published.cmp(&a.published).then(a.id.cmp(&b.id)));

    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "{ENCABEZADO}").unwrap();
    let mut con_pdf = 0;
    for obra in elegidas.iter().take(max) {
        if !obra.pdf_url.is_empty() { con_pdf += 1; }
        writeln!(
            file,
            "{}\t{}\t{}\t{}\t{}\t{}\t\t{}\t{}\t{}\t{}\t{}\t{}\t\t{}",
            clean(&format!("acl-anthology:{}", obra.id)),
            clean(&obra.title),
            clean(&obra.doi),
            clean(&obra.year),
            if obra.estado == "abierto" { "true" } else { "" },
            obra.estado,
            clean(&obra.version),
            clean(&obra.landing_url),
            clean(&obra.pdf_url),
            clean(&obra.pdf_url),
            clean(&obra.abstract_text),
            clean(&obra.published),
            clean(&obra.award),
        ).unwrap();
    }
    println!(
        "acl-anthology: {} obras escritas ({con_pdf} con pdf_url) de {} que coinciden, entre {revisadas} revisadas en {} colección(es); datos={}; resultados={out}",
        elegidas.len().min(max), elegidas.len(), archivos.len(), datos.display()
    );
}

fn run_enlazar(args: &[String]) {
    let mut catalogo = None;
    let mut arxiv: Vec<String> = Vec::new();
    let mut out = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--catalogo" if i + 1 < args.len() => { catalogo = Some(args[i + 1].clone()); i += 2; }
            "--arxiv" if i + 1 < args.len() => { arxiv.push(args[i + 1].clone()); i += 2; }
            "--out" if i + 1 < args.len() => { out = Some(args[i + 1].clone()); i += 2; }
            _ => { eprintln!("error: argumento desconocido o sin valor: {}", args[i]); std::process::exit(2); }
        }
    }
    let catalogo = catalogo.unwrap_or_else(|| { eprintln!("error: falta --catalogo"); std::process::exit(2); });
    let out = out.unwrap_or_else(|| { eprintln!("error: falta --out"); std::process::exit(2); });
    if arxiv.is_empty() { eprintln!("error: falta al menos un --arxiv"); std::process::exit(2); }

    let (cabecera, filas) = leer_tsv(&catalogo).unwrap_or_else(|err| { eprintln!("error: {err}"); std::process::exit(1); });
    let columna = |nombre: &str| cabecera.iter().position(|celda| celda == nombre);
    let (Some(c_identity), Some(c_id), Some(c_title), Some(c_pdf)) = (columna("identity"), columna("id"), columna("title"), columna("pdf_url")) else {
        eprintln!("error: {catalogo} no tiene las columnas identity, id, title y pdf_url de un catálogo");
        std::process::exit(1);
    };
    let c_estado = columna("oa_status");

    let mut preprints: Vec<Preprint> = Vec::new();
    for archivo in &arxiv {
        let (cabecera, filas) = leer_tsv(archivo).unwrap_or_else(|err| { eprintln!("error: {err}"); std::process::exit(1); });
        let columna = |nombre: &str| cabecera.iter().position(|celda| celda == nombre);
        let (Some(a_id), Some(a_title), Some(a_pdf)) = (columna("id"), columna("title"), columna("pdf_url")) else {
            eprintln!("error: {archivo} no es una salida de arxiv search (faltan id, title o pdf_url)");
            std::process::exit(1);
        };
        for fila in &filas {
            let id = campo(fila, a_id).trim_start_matches("arxiv:").to_string();
            let titulo = normalizar(campo(fila, a_title));
            if id.is_empty() || titulo.is_empty() { continue; }
            let (base, version) = sin_version(&id);
            let nuevo = Preprint { id: id.clone(), base, version, titulo, pdf_url: campo(fila, a_pdf).to_string() };
            match preprints.iter_mut().find(|visto| visto.base == nuevo.base) {
                Some(visto) => if nuevo.version > visto.version { *visto = nuevo; },
                None => preprints.push(nuevo),
            }
        }
    }

    // Una obra retractada o retirada no debe ganar una copia por la puerta de atrás.
    let de_acl: Vec<&Vec<String>> = filas.iter()
        .filter(|fila| campo(fila, c_identity).starts_with("acl-anthology:") || campo(fila, c_id).starts_with("acl-anthology:"))
        .filter(|fila| !matches!(c_estado.map(|c| campo(fila, c)).unwrap_or(""), "retractado" | "retirado"))
        .collect();
    let titulos: Vec<String> = de_acl.iter().map(|fila| normalizar(campo(fila, c_title))).collect();
    let pares = emparejar(&titulos, &preprints);

    let mut file = fs::File::create(&out).unwrap_or_else(|err| { eprintln!("error: no se pudo escribir {out}: {err}"); std::process::exit(1); });
    writeln!(file, "identity\tid\tpdf_urls\tarxiv\tsimilitud").unwrap();
    let (mut exactas, mut tolerantes, mut ambiguas) = (0, 0, 0);
    for (k, (fila, par)) in de_acl.iter().zip(pares).enumerate() {
        match par {
            Par::Ninguno => {}
            Par::Ambiguo => ambiguas += 1,
            Par::Con(j, similitud) => {
                let preprint = &preprints[j];
                if titulos[k] == preprint.titulo { exactas += 1; } else { tolerantes += 1; }
                // La copia de ACL va primero: es la versión publicada, y `buscar`
                // pone delante las URLs de la observación nueva.
                let urls: Vec<&str> = [campo(fila, c_pdf), preprint.pdf_url.as_str()].iter().cloned().filter(|url| !url.is_empty()).collect();
                writeln!(file, "{}\t{}\t{}\t{}\t{similitud:.3}", clean(campo(fila, c_identity)), clean(campo(fila, c_id)), clean(&urls.join(";")), clean(&format!("arxiv:{}", preprint.id))).unwrap();
            }
        }
    }
    println!(
        "acl-anthology enlazar: {} de {} filas de la Anthology enlazadas ({exactas} por título exacto, {tolerantes} con similitud >= {UMBRAL}), {ambiguas} ambiguas sin enlazar; {} preprints leídos; resultados={out}",
        exactas + tolerantes, de_acl.len(), preprints.len()
    );
}

fn asegurar_completo(datos: &Path) {
    if datos.join(COMPLETO).exists() { return; }
    let tarball = datos.join("acl-anthology.tar.gz");
    eprintln!("acl-anthology: bajando el XML completo de la Anthology (unos 60 MB)");
    if let Err((_, err)) = bajar(TARBALL, &tarball, "900") { eprintln!("error: {err}"); std::process::exit(1); }
    let extraccion = Command::new("tar")
        .arg("-xzf").arg(&tarball).arg("-C").arg(datos)
        .args(["--wildcards", "--strip-components=3", "*/data/xml/*.xml"])
        .status();
    let _ = fs::remove_file(&tarball);
    match extraccion {
        Ok(status) if status.success() => {}
        Ok(_) => { eprintln!("error: tar no pudo extraer data/xml del tarball de la Anthology"); std::process::exit(1); }
        Err(err) => { eprintln!("error: no se pudo ejecutar tar: {err}"); std::process::exit(1); }
    }
    if let Err(err) = fs::write(datos.join(COMPLETO), "") { eprintln!("error: no se pudo escribir la marca en {}: {err}", datos.display()); std::process::exit(1); }
}

fn asegurar_coleccion(datos: &Path, coleccion: &str) -> Option<PathBuf> {
    let ruta = datos.join(format!("{coleccion}.xml"));
    if ruta.exists() { return Some(ruta); }
    match bajar(&format!("{XML_POR_COLECCION}/{coleccion}.xml"), &ruta, "120") {
        Ok(()) => Some(ruta),
        Err((true, _)) => { eprintln!("advertencia: la colección {coleccion} no existe en la Anthology"); None }
        Err((false, err)) => { eprintln!("error: {err}"); std::process::exit(1); }
    }
}

// El bool del error dice si el servidor respondió con un status HTTP de error
// (curl sale con 22 por --fail), distinto de no haber podido conectar.
fn bajar(url: &str, destino: &Path, max_time: &str) -> Result<(), (bool, String)> {
    let parcial = destino.with_extension("part");
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--retry-delay", "3", "--retry-max-time", "120", "--connect-timeout", "20", "--max-time", max_time, "--user-agent", "pulpo-librero/0.1", "--output"])
        .arg(&parcial)
        .arg(url)
        .status();
    thread::sleep(Duration::from_secs(3));
    let fallo = match result {
        Err(err) => Some((false, format!("no se pudo ejecutar curl: {err}"))),
        Ok(status) if status.code() == Some(22) => Some((true, format!("{url} respondió con un status HTTP de error"))),
        Ok(status) if !status.success() => Some((false, format!("{url} no respondió correctamente"))),
        Ok(_) => None,
    };
    if let Some(fallo) = fallo {
        let _ = fs::remove_file(&parcial);
        return Err(fallo);
    }
    fs::rename(&parcial, destino).map_err(|err| (false, format!("no se pudo mover {}: {err}", parcial.display())))
}

fn obras_de(xml: &str) -> Vec<Obra> {
    let mut obras = Vec::new();
    let Some(coleccion) = etiquetas(xml, "collection").first().and_then(|etiqueta| atributo(etiqueta, "id")) else { return obras };
    for (abre_volumen, volumen) in bloques(xml, "volume") {
        let Some(volumen_id) = atributo(abre_volumen, "id") else { continue };
        let meta = bloques(volumen, "meta").first().map(|(_, cuerpo)| *cuerpo).unwrap_or("");
        let year = texto(meta, "year");
        let mes_volumen = texto(meta, "month");
        let venues: Vec<String> = bloques(meta, "venue").iter().map(|(_, cuerpo)| limpiar(cuerpo).to_lowercase()).collect();
        for (abre_paper, paper) in bloques(volumen, "paper") {
            let Some(paper_id) = atributo(abre_paper, "id") else { continue };
            let id = build_id(&coleccion, &volumen_id, &paper_id);
            // <url> también es marcado dentro del título y del resumen: solo
            // cuenta el que cuelga del paper.
            let propio = quitar_bloques(&quitar_bloques(paper, "abstract"), "title");
            let ultima_revision = etiquetas(&propio, "revision").iter().filter_map(|etiqueta| atributo(etiqueta, "id")?.parse::<u32>().ok()).max();
            let externa = bloques(&propio, "url").iter().map(|(_, cuerpo)| limpiar(cuerpo)).find(|url| url.starts_with("http"));
            let estado = if !etiquetas(&propio, "removed").is_empty() {
                "retirado"
            } else if !etiquetas(&propio, "retracted").is_empty() {
                "retractado"
            } else if !etiquetas(&propio, "pdf").is_empty() {
                "abierto"
            } else if externa.is_some() {
                "externo"
            } else {
                "sin-pdf"
            };
            let version = ultima_revision.map(|n| format!("v{n}")).unwrap_or_default();
            let pdf_url = if estado == "abierto" { format!("{SITIO}/{id}{version}.pdf") } else { String::new() };
            let landing_url = match (estado, &externa) {
                ("externo", Some(url)) => url.clone(),
                _ => format!("{SITIO}/{id}/"),
            };
            let mes = Some(texto(&propio, "month")).filter(|mes| !mes.is_empty()).unwrap_or_else(|| mes_volumen.clone());
            let award = etiquetas(&propio, "award").iter().filter_map(|etiqueta| atributo(etiqueta, "name")).collect::<Vec<_>>().join("; ");
            obras.push(Obra {
                title: texto(paper, "title"),
                doi: texto(&propio, "doi"),
                published: publicacion(&year, &mes),
                year: year.clone(),
                estado,
                version,
                landing_url,
                pdf_url,
                abstract_text: texto(paper, "abstract"),
                award,
                venues: venues.clone(),
                id,
            });
        }
    }
    obras
}

// Réplica de build_id del paquete acl-anthology: los ids anteriores a 2020
// rellenan con ceros, y los talleres (W) usan dos cifras de volumen.
fn build_id(coleccion: &str, volumen: &str, paper: &str) -> String {
    if coleccion.starts_with(|c: char| c.is_ascii_digit()) { return format!("{coleccion}-{volumen}.{paper}"); }
    let volumen: u32 = volumen.parse().unwrap_or(0);
    let paper: u32 = paper.parse().unwrap_or(0);
    if coleccion.starts_with('W') || coleccion == "C69" || (coleccion == "D19" && volumen >= 5) {
        format!("{coleccion}-{volumen:02}{paper:02}")
    } else {
        format!("{coleccion}-{volumen}{paper:03}")
    }
}

fn coleccion_de(id: &str) -> String {
    id.split('-').next().unwrap_or(id).to_string()
}

fn nombre_seguro(nombre: &str) -> bool {
    !nombre.is_empty() && !nombre.starts_with('.') && !nombre.contains("..") && nombre.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

fn publicacion(year: &str, mes: &str) -> String {
    if year.len() != 4 || !year.chars().all(|c| c.is_ascii_digit()) { return String::new(); }
    match numero_de_mes(mes) {
        Some(numero) => format!("{year}-{numero:02}"),
        None => year.to_string(),
    }
}

// Los meses vienen como "July", "jul", "7" o rangos como "June-July"; vale el primero.
fn numero_de_mes(mes: &str) -> Option<u32> {
    let palabra = mes.trim().chars().take_while(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
    if let Ok(numero) = palabra.parse::<u32>() { return Some(numero).filter(|numero| (1..=12).contains(numero)); }
    ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"].iter().position(|nombre| palabra.starts_with(nombre)).map(|pos| pos as u32 + 1)
}

// El mes es la precisión más fina que da la Anthology: un mes igual al de
// --desde pasa, porque la fuente no dice en qué día cae.
fn pasa_fecha(published: &str, desde: Option<&str>) -> bool {
    let Some(desde) = desde else { return true };
    !published.is_empty() && published >= desde.get(..published.len()).unwrap_or(desde)
}

fn terminos(query: &str) -> Vec<Vec<String>> {
    query
        .split(" OR ")
        .map(|alternativa| alternativa.split_whitespace().map(|palabra| palabra.trim_matches('"').to_lowercase()).filter(|palabra| !palabra.is_empty()).collect::<Vec<_>>())
        .filter(|alternativa| !alternativa.is_empty())
        .collect()
}

fn coincide(texto: &str, alternativas: &[Vec<String>]) -> bool {
    let texto = texto.to_lowercase();
    alternativas.iter().any(|alternativa| alternativa.iter().all(|palabra| texto.contains(palabra.as_str())))
}

fn emparejar(titulos: &[String], preprints: &[Preprint]) -> Vec<Par> {
    let mut pares: Vec<Par> = titulos.iter().map(|titulo| mejor(titulo, preprints)).collect();
    // Un preprint que reclaman dos filas queda con la más parecida; si empatan, con ninguna.
    for j in 0..preprints.len() {
        let rivales: Vec<(usize, f64)> = pares.iter().enumerate().filter_map(|(i, par)| match *par { Par::Con(p, s) if p == j => Some((i, s)), _ => None }).collect();
        if rivales.len() < 2 { continue; }
        let tope = rivales.iter().map(|&(_, s)| s).fold(0.0, f64::max);
        let empate = rivales.iter().filter(|&&(_, s)| s == tope).count() > 1;
        for (i, s) in rivales {
            if empate || s < tope { pares[i] = Par::Ambiguo; }
        }
    }
    pares
}

fn mejor(titulo: &str, preprints: &[Preprint]) -> Par {
    let mut mejor = Par::Ninguno;
    let mut empate = false;
    for (j, preprint) in preprints.iter().enumerate() {
        let s = similitud(titulo, &preprint.titulo);
        if s < UMBRAL { continue; }
        match mejor {
            Par::Con(_, m) if s < m => {}
            Par::Con(_, m) if s == m => empate = true,
            _ => { mejor = Par::Con(j, s); empate = false; }
        }
    }
    if empate { Par::Ambiguo } else { mejor }
}

// Levenshtein capta erratas y cambios de una palabra; Jaccard sobre las
// palabras capta un título reordenado, que Levenshtein castiga como si fuera otro.
// Un año o un número de tarea distinto es otra obra aunque el resto calce:
// SemEval y WMT repiten el título de sus overviews cada año.
fn similitud(a: &str, b: &str) -> f64 {
    if a == b { return 1.0; }
    if numeros(a) != numeros(b) { return 0.0; }
    levenshtein(a, b).max(jaccard(a, b))
}

fn numeros(titulo: &str) -> Vec<&str> {
    let mut numeros: Vec<&str> = titulo.split_whitespace().filter(|palabra| palabra.chars().any(|c| c.is_ascii_digit())).collect();
    numeros.sort();
    numeros.dedup();
    numeros
}

fn jaccard(a: &str, b: &str) -> f64 {
    let mut a: Vec<&str> = a.split_whitespace().collect();
    let mut b: Vec<&str> = b.split_whitespace().collect();
    a.sort(); a.dedup(); b.sort(); b.dedup();
    let comunes = a.iter().filter(|palabra| b.binary_search(palabra).is_ok()).count();
    let union = a.len() + b.len() - comunes;
    if union == 0 { 0.0 } else { comunes as f64 / union as f64 }
}

// Normalizado al largo mayor. Solo interesa si llega al umbral, así que basta
// la banda de distancias que caben bajo él.
fn levenshtein(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let largo = a.len().max(b.len());
    if largo == 0 { return 0.0; }
    let tope = ((1.0 - UMBRAL) * largo as f64 + 1e-9).floor() as usize;
    let diferencia = if a.len() > b.len() { a.len() - b.len() } else { b.len() - a.len() };
    if diferencia > tope { return 0.0; }
    let lejos = usize::MAX / 2;
    let mut previa: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut fila = vec![lejos; b.len() + 1];
        if i <= tope { fila[0] = i; }
        let desde = if i > tope { i - tope } else { 1 };
        let hasta = (i + tope).min(b.len());
        for j in desde..=hasta {
            let costo = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            fila[j] = (previa[j - 1] + costo).min(previa[j] + 1).min(fila[j - 1] + 1);
        }
        previa = fila;
    }
    let distancia = previa[b.len()];
    if distancia > tope { 0.0 } else { 1.0 - distancia as f64 / largo as f64 }
}

fn normalizar(titulo: &str) -> String {
    titulo.chars().map(|c| if c.is_alphanumeric() { c.to_lowercase().next().unwrap_or(c) } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sin_version(id: &str) -> (String, u32) {
    match id.rsplit_once('v') {
        Some((base, version)) if !version.is_empty() && version.chars().all(|c| c.is_ascii_digit()) => (base.to_string(), version.parse().unwrap_or(0)),
        _ => (id.to_string(), 0),
    }
}

fn leer_tsv(ruta: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let texto = fs::read_to_string(ruta).map_err(|err| format!("no se pudo leer {ruta}: {err}"))?;
    let mut lineas = texto.lines().filter(|linea| !linea.trim().is_empty()).map(|linea| linea.split('\t').map(str::to_string).collect::<Vec<_>>());
    let cabecera = lineas.next().ok_or_else(|| format!("{ruta} está vacío"))?;
    Ok((cabecera, lineas.collect()))
}

fn campo(fila: &[String], columna: usize) -> &str {
    fila.get(columna).map(String::as_str).unwrap_or("")
}

fn lista(valor: &str) -> Vec<String> {
    valor.split(',').map(str::trim).filter(|parte| !parte.is_empty()).map(str::to_string).collect()
}

// Bloques <nombre ...>cuerpo</nombre>, con su etiqueta de apertura. Ninguno de
// los elementos que se piden se anida dentro de sí mismo.
fn bloques<'a>(xml: &'a str, nombre: &str) -> Vec<(&'a str, &'a str)> {
    let cierre = format!("</{nombre}>");
    let mut resultado = Vec::new();
    let mut resto = xml;
    while let Some((inicio, fin_etiqueta)) = apertura(resto, nombre) {
        let etiqueta = &resto[inicio..fin_etiqueta];
        if etiqueta.ends_with('/') { resto = &resto[fin_etiqueta + 1..]; continue; }
        let cuerpo = fin_etiqueta + 1;
        let Some(largo) = resto[cuerpo..].find(&cierre) else { break };
        resultado.push((etiqueta, &resto[cuerpo..cuerpo + largo]));
        resto = &resto[cuerpo + largo + cierre.len()..];
    }
    resultado
}

fn etiquetas<'a>(xml: &'a str, nombre: &str) -> Vec<&'a str> {
    let mut resultado = Vec::new();
    let mut resto = xml;
    while let Some((inicio, fin_etiqueta)) = apertura(resto, nombre) {
        resultado.push(&resto[inicio..fin_etiqueta]);
        resto = &resto[fin_etiqueta + 1..];
    }
    resultado
}

// Posición de `<nombre` seguido de espacio, `>` o `/`, para que <title> no
// encuentre <titles> ni <paper> encuentre <paperx>, y la del `>` que la cierra.
fn apertura(xml: &str, nombre: &str) -> Option<(usize, usize)> {
    let marca = format!("<{nombre}");
    let mut desde = 0;
    while let Some(pos) = xml[desde..].find(&marca) {
        let inicio = desde + pos;
        let siguiente = xml[inicio + marca.len()..].chars().next();
        if matches!(siguiente, Some(' ') | Some('>') | Some('/') | Some('\n') | Some('\t')) {
            return xml[inicio..].find('>').map(|largo| (inicio, inicio + largo));
        }
        desde = inicio + marca.len();
    }
    None
}

fn quitar_bloques(xml: &str, nombre: &str) -> String {
    let cierre = format!("</{nombre}>");
    let mut resultado = String::new();
    let mut resto = xml;
    while let Some((inicio, fin_etiqueta)) = apertura(resto, nombre) {
        resultado.push_str(&resto[..inicio]);
        resto = match resto[fin_etiqueta..].find(&cierre) {
            Some(largo) => &resto[fin_etiqueta + largo + cierre.len()..],
            None => "",
        };
    }
    resultado.push_str(resto);
    resultado
}

fn atributo(etiqueta: &str, nombre: &str) -> Option<String> {
    let marca = format!(" {nombre}=\"");
    let inicio = etiqueta.find(&marca)? + marca.len();
    etiqueta[inicio..].find('"').map(|largo| unescape(&etiqueta[inicio..inicio + largo]))
}

fn texto(xml: &str, nombre: &str) -> String {
    bloques(xml, nombre).first().map(|(_, cuerpo)| limpiar(cuerpo)).unwrap_or_default()
}

// Quita el marcado interno (<fixed-case>, <tex-math>, <i>, <url>) y deja el texto.
fn limpiar(xml: &str) -> String {
    let mut sin_marcado = String::with_capacity(xml.len());
    let mut dentro = false;
    for c in xml.chars() {
        match c {
            '<' => dentro = true,
            '>' if dentro => dentro = false,
            _ if !dentro => sin_marcado.push(c),
            _ => {}
        }
    }
    unescape(&sin_marcado).split_whitespace().collect::<Vec<_>>().join(" ")
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

fn fecha_valida(fecha: &str) -> bool {
    let bytes = fecha.as_bytes();
    bytes.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-' && bytes.iter().enumerate().all(|(pos, b)| pos == 4 || pos == 7 || b.is_ascii_digit())
}

fn clean(value: &str) -> String { value.replace('\t', " ").replace('\n', " ").replace('\r', " ") }

#[cfg(test)]
mod tests {
    use super::*;

    fn preprint(id: &str, titulo: &str) -> Preprint {
        let (base, version) = sin_version(id);
        Preprint { id: id.to_string(), base, version, titulo: normalizar(titulo), pdf_url: format!("https://arxiv.org/pdf/{id}.pdf") }
    }

    #[test]
    fn arma_los_ids_como_la_anthology() {
        assert_eq!(build_id("2022.acl", "long", "42"), "2022.acl-long.42");
        assert_eq!(build_id("P18", "1", "1"), "P18-1001");
        assert_eq!(build_id("W19", "38", "1"), "W19-3801");
        assert_eq!(build_id("D19", "5", "3"), "D19-0503");
        assert_eq!(build_id("D19", "1", "3"), "D19-1003");
        assert_eq!(coleccion_de("2025.coling-main.420"), "2025.coling");
        assert_eq!(coleccion_de("P19-1001"), "P19");
    }

    #[test]
    fn lee_estado_version_y_fecha_del_xml() {
        let xml = r#"<collection id="2024.x"><volume id="main"><meta><booktitle>B</booktitle><month>June-July</month><year>2024</year><venue>x</venue><venue>coling</venue></meta>
<paper id="1"><title><fixed-case>A</fixed-case> &amp; B</title><abstract>See <url>https://github.com/a</url></abstract><pdf hash="1"/><revision id="1" hash="1"/><revision id="2" hash="2" date="2025-01-01">Fix.</revision><award name="Best Paper"/><doi>10.18653/v1/2024.x-main.1</doi></paper>
<paper id="2"><title>R</title><pdf hash="3"/><retracted date="2024-05-21">Retracted.</retracted></paper>
<paper id="3"><title>E</title><url>https://link.springer.com/x</url></paper>
<paper id="4"><title>S</title><abstract>Only <url>https://x.org</url></abstract></paper>
</volume></collection>"#;
        let obras = obras_de(xml);
        assert_eq!(obras.len(), 4);
        assert_eq!(obras[0].id, "2024.x-main.1");
        assert_eq!(obras[0].title, "A & B");
        assert_eq!(obras[0].estado, "abierto");
        assert_eq!(obras[0].version, "v2");
        assert_eq!(obras[0].pdf_url, "https://aclanthology.org/2024.x-main.1v2.pdf");
        assert_eq!(obras[0].published, "2024-06");
        assert_eq!(obras[0].award, "Best Paper");
        assert_eq!(obras[0].venues, vec!["x".to_string(), "coling".to_string()]);
        assert_eq!((obras[1].estado, obras[1].pdf_url.as_str()), ("retractado", ""));
        assert_eq!((obras[2].estado, obras[2].landing_url.as_str()), ("externo", "https://link.springer.com/x"));
        assert_eq!(obras[3].estado, "sin-pdf");
    }

    #[test]
    fn filtra_por_mes_y_por_alternativas() {
        assert!(pasa_fecha("2024-10", Some("2024-10-19")));
        assert!(!pasa_fecha("2024-09", Some("2024-10-19")));
        assert!(pasa_fecha("2024", Some("2024-10-19")));
        assert!(!pasa_fecha("", Some("2024-10-19")));
        let alternativas = terminos("hate speech OR toxicity");
        assert!(coincide("A Dataset for Hate and Offensive Speech", &alternativas));
        assert!(!coincide("Hate crimes", &alternativas));
    }

    #[test]
    fn la_similitud_llega_al_umbral_solo_con_cambios_menores() {
        let a = normalizar("Architectural Patterns for Retrieval-Augmented Generation");
        let b = normalizar("Architechtural patterns for retrieval augmented generation");
        assert!(similitud(&a, &b) >= UMBRAL);
        assert!(similitud(&normalizar("BanglaQuAD: A Bengali Open-domain QA Dataset"), &normalizar("KazQAD: Kazakh Open-Domain QA Dataset")) < UMBRAL);
        let reordenado = similitud(
            &normalizar("A Review of Prominent Paradigms for LLM-Based Agents: Tool Use, Planning (Including RAG), and Feedback Learning"),
            &normalizar("A Review of Prominent Paradigms for LLM-Based Agents: Tool Use (Including RAG), Planning, and Feedback Learning"),
        );
        assert!(reordenado >= UMBRAL);
        assert!(similitud(&normalizar("Overview of the SemEval-2024 Task 1"), &normalizar("Overview of the SemEval-2025 Task 1")) < UMBRAL);
        assert_eq!(similitud("abc", "abc"), 1.0);
    }

    #[test]
    fn un_preprint_disputado_no_se_asigna_a_ciegas() {
        let preprints = vec![preprint("2401.00001v2", "Learning to rank with large language models"), preprint("2402.00002v1", "A survey of dialogue systems")];
        let titulos = vec![normalizar("Learning to Rank with Large Language Models"), normalizar("Learning to rank with large language model"), normalizar("Something else entirely")];
        let pares = emparejar(&titulos, &preprints);
        assert_eq!(pares[0], Par::Con(0, 1.0));
        assert_eq!(pares[1], Par::Ambiguo);
        assert_eq!(pares[2], Par::Ninguno);
        assert_eq!(sin_version("2401.00001v12"), ("2401.00001".to_string(), 12));
        assert_eq!(sin_version("solv-int/9901001"), ("solv-int/9901001".to_string(), 0));
    }
}
