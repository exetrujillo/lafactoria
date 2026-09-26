use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

const MAX_DESCRIPTION_LEN: usize = 1024;
const MAX_NAME_LEN: usize = 64;
const RESOURCE_PREFIXES: [&str; 3] = ["references/", "scripts/", "assets/"];

// Las dos raíces donde puede vivir una skill: `skills/` se versiona y se
// comparte; `priv-skills/` está en .gitignore y es de quien clonó el repo.
const RAICES: [&str; 2] = ["skills", "priv-skills"];

// Copias canónicas de los lectores de JSON que cada skill copia tal cual a su
// propio scripts/ (ver "Escribir el SKILL.md" en forjador/SKILL.md). Viven acá,
// no en runtime, para que lint_skill pueda comparar bytes sin ninguna otra
// fuente de verdad. json_util.rs lo usa el validador de vivencias; json_api.rs,
// los scrapers que leen respuestas de API.
const COPIAS_CANONICAS: [(&str, &str); 2] = [
    ("json_util.rs", include_str!("json_util.rs")),
    ("json_api.rs", include_str!("json_api.rs")),
];

struct Frontmatter {
    name: Option<String>,
    description: Option<String>,
}

struct Report {
    errors: Vec<String>,
}

impl Report {
    fn new() -> Self {
        Report { errors: Vec::new() }
    }
    fn error(&mut self, skill: &str, msg: impl Into<String>) {
        self.errors.push(format!("[{skill}] {}", msg.into()));
    }
}

fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let content = content.strip_prefix("---")?;
    let content = content
        .strip_prefix("\r\n")
        .or_else(|| content.strip_prefix('\n'))?;
    let end = content.find("\n---")?;
    let fm = &content[..end];
    let mut rest = &content[end + 4..];
    rest = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
        .unwrap_or(rest);
    Some((fm, rest))
}

fn parse_frontmatter(fm: &str) -> Frontmatter {
    let mut name = None;
    let mut description = None;
    let lines: Vec<&str> = fm.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') || line.starts_with(char::is_whitespace)
        {
            i += 1;
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            let mut value = value.trim().to_string();
            let is_fold_marker = matches!(value.as_str(), ">" | ">-" | ">+" | "|" | "|-" | "|+");
            if value.is_empty() || is_fold_marker {
                let mut cont = Vec::new();
                let mut j = i + 1;
                while j < lines.len() && lines[j].starts_with(char::is_whitespace) && !lines[j].trim().is_empty() {
                    cont.push(lines[j].trim());
                    j += 1;
                }
                value = cont.join(" ");
                i = j;
            } else {
                i += 1;
            }
            let value = strip_quotes(&value);
            match key {
                "name" => name = Some(value),
                "description" => description = Some(value),
                _ => {}
            }
        } else {
            i += 1;
        }
    }
    Frontmatter { name, description }
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 {
        let bytes = s.as_bytes();
        if (bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'')
        {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

fn is_opencode_name(name: &str) -> bool {
    if name.is_empty() || name.len() > MAX_NAME_LEN || name.starts_with('-') || name.ends_with('-') {
        return false;
    }

    let mut previous_hyphen = false;
    for byte in name.bytes() {
        if byte == b'-' {
            if previous_hyphen {
                return false;
            }
            previous_hyphen = true;
        } else if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() {
            return false;
        } else {
            previous_hyphen = false;
        }
    }
    true
}

fn check_referenced_paths(skill_dir: &Path, body: &str, skill: &str, report: &mut Report) {
    for raw_token in body.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '`') {
        let token = raw_token.trim_matches(|c| c == '[' || c == ']' || c == ',' || c == '.');
        if token.contains("://") {
            continue;
        }
        let is_reference = RESOURCE_PREFIXES
            .iter()
            .any(|p| token.starts_with(*p) && token.len() > p.len());
        if is_reference {
            let candidate = skill_dir.join(token);
            if !candidate.exists() {
                report.error(
                    skill,
                    format!("el archivo referenciado '{token}' no existe en el directorio de la skill"),
                );
            }
        }
    }
}

fn check_copias_canonicas(skill_dir: &Path, skill: &str, report: &mut Report) {
    for (nombre, canonico) in COPIAS_CANONICAS {
        let path = skill_dir.join("scripts").join(nombre);
        if !path.is_file() {
            continue;
        }
        match fs::read_to_string(&path) {
            Ok(content) if content == canonico => {}
            Ok(_) => report.error(
                skill,
                format!("scripts/{nombre} difiere de la copia canónica en src/{nombre}; los cambios a ese lector de JSON se hacen ahí y se propagan tal cual a cada skill"),
            ),
            Err(e) => report.error(skill, format!("no se pudo leer scripts/{nombre}: {e}")),
        }
    }
}

fn lint_skill(skill_dir: &Path, seen_names: &mut Vec<(String, String)>, report: &mut Report) {
    let dir_name = skill_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let skill_md = skill_dir.join("SKILL.md");
    if !skill_md.exists() {
        report.error(&dir_name, "falta el archivo SKILL.md");
        return;
    }
    let content = match fs::read_to_string(&skill_md) {
        Ok(c) => c,
        Err(e) => {
            report.error(&dir_name, format!("no se pudo leer SKILL.md: {e}"));
            return;
        }
    };

    let Some((fm_block, body)) = split_frontmatter(&content) else {
        report.error(&dir_name, "SKILL.md debe iniciar con un bloque de frontmatter YAML delimitado por '---'");
        return;
    };

    let fm = parse_frontmatter(fm_block);

    match &fm.name {
        None => report.error(&dir_name, "al frontmatter le falta el campo obligatorio 'name'"),
        Some(n) if n.is_empty() => report.error(&dir_name, "'name' no puede estar vacío"),
        Some(n) if n != &dir_name => report.error(
            &dir_name,
            format!("'name: {n}' no coincide con el nombre del directorio '{dir_name}'"),
        ),
        Some(n) if !is_opencode_name(n) => report.error(
            &dir_name,
            "'name' debe cumplir el formato de OpenCode: minúsculas, números y guiones simples, entre 1 y 64 caracteres",
        ),
        Some(n) => seen_names.push((n.clone(), skill_dir.display().to_string())),
    }

    match &fm.description {
        None => report.error(&dir_name, "al frontmatter le falta el campo obligatorio 'description'"),
        Some(d) if d.trim().is_empty() => report.error(&dir_name, "'description' no puede estar vacía"),
        Some(d) if d.len() > MAX_DESCRIPTION_LEN => report.error(
            &dir_name,
            format!("'description' tiene {} caracteres; no puede superar {MAX_DESCRIPTION_LEN}", d.len()),
        ),
        _ => {}
    }

    if body.trim().is_empty() {
        report.error(&dir_name, "SKILL.md no tiene instrucciones después del frontmatter");
    } else {
        check_referenced_paths(skill_dir, body, &dir_name, report);
    }

    check_copias_canonicas(skill_dir, &dir_name, report);
}

fn lint_all(skills_dirs: &[PathBuf]) -> Report {
    let mut report = Report::new();
    let mut seen_names: Vec<(String, String)> = Vec::new();
    let mut vistas = 0;

    for skills_dir in skills_dirs {
        let entries = match fs::read_dir(skills_dir) {
            Ok(e) => e,
            Err(e) => {
                report.error(
                    skills_dir.to_string_lossy().as_ref(),
                    format!("no se pudo leer el directorio: {e}"),
                );
                continue;
            }
        };

        let mut dirs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        vistas += dirs.len();

        for dir in &dirs {
            lint_skill(dir, &mut seen_names, &mut report);
        }
    }

    if vistas == 0 && report.errors.is_empty() {
        let nombres: Vec<String> = skills_dirs.iter().map(|d| d.display().to_string()).collect();
        println!("(no se encontraron skills en {})", nombres.join(", "));
    }

    for i in 0..seen_names.len() {
        for j in (i + 1)..seen_names.len() {
            if seen_names[i].0 == seen_names[j].0 {
                report.error(
                    &seen_names[j].1,
                    format!(
                        "el nombre de skill '{}' está duplicado; también lo usa '{}'",
                        seen_names[i].0, seen_names[i].1
                    ),
                );
            }
        }
    }

    report
}

fn raices_existentes() -> Vec<PathBuf> {
    let dirs: Vec<PathBuf> = RAICES.iter().map(PathBuf::from).filter(|d| d.is_dir()).collect();
    if dirs.is_empty() {
        vec![PathBuf::from(RAICES[0])]
    } else {
        dirs
    }
}

fn run_lint(dirs: &[PathBuf]) {
    let report = lint_all(dirs);

    for e in &report.errors {
        println!("error: {e}");
    }

    if report.errors.is_empty() {
        println!("OK");
    } else {
        println!(
            "FALLÓ: {} error(es)",
            report.errors.len()
        );
        exit(1);
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        if entry.file_name() == "__pycache__" {
            continue;
        }
        let dest_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

/// Lista las rutas relativas de los archivos que existen en `dir` (recursivo).
/// Ignora `__pycache__` y el marcador `.factoria-origen`, que escribe el propio
/// `install` y por definición nunca está en la fuente.
fn archivos_relativos(dir: &Path, prefijo: &Path, acumulador: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let nombre = entry.file_name();
        if nombre == "__pycache__" || nombre == ".factoria-origen" {
            continue;
        }
        let relativo = prefijo.join(&nombre);
        if entry.file_type()?.is_dir() {
            archivos_relativos(&entry.path(), &relativo, acumulador)?;
        } else {
            acumulador.push(relativo);
        }
    }
    Ok(())
}

/// Archivos que existen en la copia instalada y no en la fuente. Son
/// exactamente los que `install` destruiría al reemplazar el directorio, y en
/// la práctica son vivencias: bitácoras, índices y registros que la skill
/// escribió mientras trabajaba. El principio 1 del repositorio dice que ese
/// material no se elimina sin permiso del usuario, así que `install` se detiene
/// cuando encuentra alguno.
fn archivos_solo_en_destino(source: &Path, dest: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut instalados = Vec::new();
    archivos_relativos(dest, Path::new(""), &mut instalados)?;
    instalados.retain(|relativo| !source.join(relativo).exists());
    instalados.sort();
    Ok(instalados)
}

fn directories_equal(src: &Path, dst: &Path) -> std::io::Result<bool> {
    // `__pycache__` es regenerable y `.factoria-origen` lo escribe `install`
    // después de copiar: ninguno de los dos distingue una copia al día de una
    // desactualizada, así que se ignoran de los dos lados.
    let relevante = |entry: &fs::DirEntry| {
        let nombre = entry.file_name();
        nombre != "__pycache__" && nombre != ".factoria-origen"
    };
    let mut src_entries: Vec<_> = fs::read_dir(src)?.filter_map(Result::ok).filter(relevante).collect();
    let mut dst_entries: Vec<_> = fs::read_dir(dst)?.filter_map(Result::ok).filter(relevante).collect();
    src_entries.sort_by_key(|entry| entry.file_name());
    dst_entries.sort_by_key(|entry| entry.file_name());

    if src_entries.len() != dst_entries.len() {
        return Ok(false);
    }

    for (src_entry, dst_entry) in src_entries.iter().zip(dst_entries.iter()) {
        if src_entry.file_name() != dst_entry.file_name() {
            return Ok(false);
        }
        if src_entry.file_type()?.is_dir() != dst_entry.file_type()?.is_dir() {
            return Ok(false);
        }
        if src_entry.file_type()?.is_dir() {
            if !directories_equal(&src_entry.path(), &dst_entry.path())? {
                return Ok(false);
            }
        } else if fs::read(src_entry.path())? != fs::read(dst_entry.path())? {
            return Ok(false);
        }
    }
    Ok(true)
}

// Convención de las skills con vivencias propias (ver "Vivencias" en el
// README): un validador fijo en `scripts/validar_ajustes.rs`, en Rust y sin
// dependencias externas, que se compila al vuelo con `rustc` y se corre
// contra `vivencias/ajustes.json`. Si la skill no declaró ese validador, o
// `vivencias/ajustes.json` todavía no existe (no está versionado, así que en
// un clon fresco es legítimo que falte), no hay nada que validar.
fn validate_vivencias(source: &Path, name: &str) -> Result<(), String> {
    let validador = source.join("scripts").join("validar_ajustes.rs");
    let ajustes = source.join("vivencias").join("ajustes.json");
    if !validador.is_file() || !ajustes.is_file() {
        return Ok(());
    }

    let binario = env::temp_dir().join(format!("skillcheck-validar-{name}"));
    let compilacion = Command::new("rustc")
        .args(["-O", "-o"])
        .arg(&binario)
        .arg(&validador)
        .output()
        .map_err(|e| format!("no se pudo ejecutar rustc: {e}"))?;
    if !compilacion.status.success() {
        return Err(format!(
            "no compiló {}:\n{}",
            validador.display(),
            String::from_utf8_lossy(&compilacion.stderr)
        ));
    }

    let corrida = Command::new(&binario)
        .arg(&ajustes)
        .output()
        .map_err(|e| format!("no se pudo ejecutar '{}': {e}", binario.display()))?;
    if !corrida.status.success() {
        return Err(String::from_utf8_lossy(&corrida.stderr).into_owned());
    }
    Ok(())
}

fn localizar_skill(name: &str) -> Vec<PathBuf> {
    RAICES
        .iter()
        .map(|raiz| PathBuf::from(raiz).join(name))
        .filter(|candidato| candidato.is_dir())
        .collect()
}

/// Dónde se instala la copia. `Proyecto` es el `.claude/skills` del
/// directorio actual (este repo); `Destino` es el de otro proyecto.
enum Destino {
    Proyecto,
    Global,
    Destino(PathBuf),
}

// Registro local de cada copia instalada, una línea `nombre<TAB>ruta` por
// copia. Existe para responder la pregunta inversa a `.factoria-origen`: la
// copia sabe cuál es su fuente, pero hasta ahora la fuente no sabía dónde
// tenía copias, y editar la fuente sin reinstalar es el error más frecuente
// del repo. Vive en la raíz, fuera de git, y no dentro de `vivencias/` de cada
// skill: `install` copia `vivencias/` a cada destino, así que un registro ahí
// esparciría las rutas absolutas de esta máquina por todos los proyectos.
const REGISTRO_INSTALACIONES: &str = "instalaciones.tsv";

fn leer_registro(registro: &Path) -> Vec<(String, PathBuf)> {
    let Ok(texto) = fs::read_to_string(registro) else {
        return Vec::new();
    };
    texto
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('\t'))
        .map(|(n, r)| (n.to_string(), PathBuf::from(r)))
        .collect()
}

fn registrar_instalacion(registro: &Path, name: &str, copia: &Path) -> std::io::Result<()> {
    let mut filas = leer_registro(registro);
    if filas.iter().any(|(n, r)| n == name && r == copia) {
        return Ok(());
    }
    filas.push((name.to_string(), copia.to_path_buf()));
    filas.sort();
    let mut texto = String::from("# Generado por `skillcheck install`: nombre<TAB>ruta de la copia instalada.\n");
    for (n, r) in &filas {
        texto.push_str(&format!("{n}\t{}\n", r.display()));
    }
    fs::write(registro, texto)
}

/// Estado de una copia registrada frente a su fuente.
fn estado_copia(fuente: &Path, copia: &Path) -> &'static str {
    if !copia.is_dir() {
        return "no_existe";
    }
    match directories_equal(fuente, copia) {
        Ok(true) => "al_dia",
        Ok(false) => "desactualizada",
        Err(_) => "ilegible",
    }
}

/// Excluye de git, en el proyecto de destino, lo que no debe versionarse ahí.
/// Usa `.git/info/exclude`, que es local a la máquina, y no el `.gitignore`
/// del proyecto: ese archivo es del equipo que lo mantiene y `skillcheck` no
/// tiene por qué tocarlo. Una skill personal se excluye entera, porque
/// `priv-skills/` nunca sale de la máquina; de una pública se excluyen solo
/// sus vivencias y el marcador con la ruta absoluta de la fuente.
fn excluir_de_git(proyecto: &Path, name: &str, personal: bool) -> std::io::Result<Vec<String>> {
    let git = proyecto.join(".git");
    if !git.is_dir() {
        return Ok(Vec::new());
    }
    let patrones: Vec<String> = if personal {
        vec![format!("/.claude/skills/{name}/")]
    } else {
        vec![
            format!("/.claude/skills/{name}/vivencias/"),
            format!("/.claude/skills/{name}/.factoria-origen"),
        ]
    };
    let exclude = git.join("info").join("exclude");
    let actual = fs::read_to_string(&exclude).unwrap_or_default();
    let faltan: Vec<String> = patrones
        .into_iter()
        .filter(|p| !actual.lines().any(|l| l.trim() == p))
        .collect();
    if faltan.is_empty() {
        return Ok(faltan);
    }
    fs::create_dir_all(git.join("info"))?;
    let mut texto = actual;
    if !texto.is_empty() && !texto.ends_with('\n') {
        texto.push('\n');
    }
    texto.push_str(&format!("# skillcheck install {name}\n"));
    for p in &faltan {
        texto.push_str(p);
        texto.push('\n');
    }
    fs::write(&exclude, texto)?;
    Ok(faltan)
}

/// Una fuente enlazada (`priv-skills/x` -> `<proyecto>/.claude/skills/x`) puede
/// resolver al mismo directorio que el destino. `install` borra el destino antes
/// de copiar, así que sin esta comprobación eliminaría la fuente; la de vivencias
/// en riesgo no lo detecta porque compara el directorio consigo mismo.
fn mismo_directorio(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn run_install(name: &str, destino: Destino, adoptar: bool) {
    let candidatos = localizar_skill(name);
    // `lint` ya trata el mismo name en las dos raíces como duplicado, pero
    // `install` valida una sola skill y no pasaría por esa regla: sin este
    // corte elegiría una raíz en silencio y podría reemplazar la copia
    // instalada de la otra, o adoptar sus vivencias hacia el árbol equivocado.
    if candidatos.len() > 1 {
        let rutas: Vec<String> = candidatos.iter().map(|c| c.display().to_string()).collect();
        eprintln!("error: '{name}' existe en más de una raíz: {}", rutas.join(", "));
        eprintln!("Dos skills con el mismo nombre divergen en silencio. Renombra una");
        eprintln!("de las dos antes de instalar; 'lint' también lo rechaza.");
        exit(1);
    }
    let Some(source) = candidatos.into_iter().next() else {
        let buscadas: Vec<String> = RAICES.iter().map(|raiz| format!("{raiz}/{name}")).collect();
        eprintln!("error: no existe {}", buscadas.join(" ni "));
        exit(1);
    };
    let fuente = source.display().to_string();

    let mut report = Report::new();
    let mut seen_names = Vec::new();
    lint_skill(&source, &mut seen_names, &mut report);
    if !report.errors.is_empty() {
        for e in &report.errors {
            println!("error: {e}");
        }
        eprintln!("error: la skill '{name}' tiene errores; corrígelos antes de instalarla");
        exit(1);
    }
    if let Err(e) = validate_vivencias(&source, name) {
        eprintln!("error: el validador de vivencias de '{name}' falló:\n{e}");
        exit(1);
    }

    let dest_root = match &destino {
        Destino::Global => match env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(".claude").join("skills"),
            None => {
                eprintln!("error: no se pudo determinar $HOME para la instalación global");
                exit(1);
            }
        },
        Destino::Proyecto => PathBuf::from(".claude").join("skills"),
        Destino::Destino(proyecto) => {
            if !proyecto.is_dir() {
                eprintln!("error: el proyecto de destino '{}' no existe o no es un directorio", proyecto.display());
                exit(1);
            }
            proyecto.join(".claude").join("skills")
        }
    };

    let dest = dest_root.join(name);
    if mismo_directorio(&source, &dest) {
        eprintln!("error: la fuente de '{name}' ({fuente}) y el destino {} son el mismo directorio", dest.display());
        eprintln!("La fuente es un enlace a esa copia: instalar la borraría antes de copiarla.");
        eprintln!("No hay nada que instalar; la skill ya está donde se la lee.");
        exit(1);
    }
    if dest.exists() {
        // `install` reemplaza el directorio entero, así que todo lo que viva
        // solo en la copia instalada se pierde. Antes eso ocurría en silencio.
        let en_riesgo = match archivos_solo_en_destino(&source, &dest) {
            Ok(lista) => lista,
            Err(e) => {
                eprintln!("error: no se pudo inspeccionar la copia instalada: {e}");
                exit(1);
            }
        };
        if !en_riesgo.is_empty() {
            if adoptar {
                for relativo in &en_riesgo {
                    let origen = dest.join(relativo);
                    let destino = source.join(relativo);
                    if let Some(padre) = destino.parent() {
                        if let Err(e) = fs::create_dir_all(padre) {
                            eprintln!("error: no se pudo crear '{}': {e}", padre.display());
                            exit(1);
                        }
                    }
                    if let Err(e) = fs::copy(&origen, &destino) {
                        eprintln!("error: no se pudo adoptar '{}': {e}", relativo.display());
                        exit(1);
                    }
                    println!("adoptado: {fuente}/{}", relativo.display());
                }
            } else {
                eprintln!("error: la copia instalada de '{name}' tiene {} archivo(s) que no están en {fuente}:", en_riesgo.len());
                for relativo in &en_riesgo {
                    eprintln!("  {}", relativo.display());
                }
                eprintln!();
                eprintln!("Instalar los borraría. Suelen ser vivencias que la skill escribió");
                eprintln!("mientras trabajaba, y este repositorio no elimina ese material sin");
                eprintln!("permiso. Elige una opción:");
                eprintln!("  - copiarlos a mano a {fuente}/ si quieres conservarlos;");
                eprintln!("  - volver a instalar con --adoptar-vivencias para que install los");
                eprintln!("    copie a la fuente antes de reemplazar la copia instalada;");
                eprintln!("  - borrarlos de {} si son basura de una versión vieja.", dest.display());
                exit(1);
            }
        }
        if let Err(e) = fs::remove_dir_all(&dest) {
            eprintln!("error: no se pudo reemplazar '{}': {e}", dest.display());
            exit(1);
        }
    }
    if let Err(e) = copy_dir_recursive(&source, &dest) {
        eprintln!("error: no se pudo copiar la skill a '{}': {e}", dest.display());
        exit(1);
    }
    match directories_equal(&source, &dest) {
        Ok(true) => {}
        Ok(false) => {
            eprintln!("error: la copia instalada no coincide con {fuente}");
            exit(1);
        }
        Err(e) => {
            eprintln!("error: no se pudo verificar la copia instalada: {e}");
            exit(1);
        }
    }

    // La copia instalada es la que leen los agentes, pero `install` la reemplaza
    // entera en cada corrida. Dejar acá la ruta de la fuente permite que una skill
    // escriba su memoria en `skills/<nombre>/memoria/`, que sí sobrevive, en vez de
    // en esta copia, que es efímera. Se escribe después de `directories_equal` para
    // no romper la verificación de que la copia es idéntica a la fuente.
    let origen = fs::canonicalize(&source).unwrap_or_else(|_| source.clone());
    let marcador = dest.join(".factoria-origen");
    if let Err(e) = fs::write(&marcador, format!("{}\n", origen.display())) {
        eprintln!("aviso: no se pudo escribir '{}': {e}", marcador.display());
    }

    let copia = fs::canonicalize(&dest).unwrap_or_else(|_| dest.clone());
    if let Err(e) = registrar_instalacion(Path::new(REGISTRO_INSTALACIONES), name, &copia) {
        eprintln!("aviso: no se pudo registrar la instalación en {REGISTRO_INSTALACIONES}: {e}");
    }
    if let Destino::Destino(proyecto) = &destino {
        let personal = source.starts_with(RAICES[1]);
        match excluir_de_git(proyecto, name, personal) {
            Ok(agregados) if !agregados.is_empty() => {
                println!("excluido de git en el proyecto (.git/info/exclude): {}", agregados.join(" "));
            }
            Ok(_) => {}
            Err(e) => eprintln!("aviso: no se pudo actualizar .git/info/exclude: {e}"),
        }
    }

    let alcance = match destino {
        Destino::Global => "global (disponible en todos los proyectos)".to_string(),
        Destino::Proyecto => "de este proyecto".to_string(),
        Destino::Destino(p) => format!("del proyecto {}", p.display()),
    };
    println!("'{name}' instalada en {} — alcance {alcance}", dest.display());
}

/// `skillcheck instalaciones [NOMBRE]`: lista las copias registradas y si
/// están al día con su fuente.
fn run_instalaciones(filtro: Option<&str>) {
    let filas = leer_registro(Path::new(REGISTRO_INSTALACIONES));
    let mut hubo = false;
    for (name, copia) in filas.iter().filter(|(n, _)| filtro.map_or(true, |f| f == n)) {
        hubo = true;
        let estado = match localizar_skill(name).first() {
            Some(fuente) => estado_copia(fuente, copia),
            None => "sin_fuente",
        };
        println!("{name:24} {estado:15} {}", copia.display());
    }
    if !hubo {
        println!("sin instalaciones registradas{}", filtro.map(|f| format!(" para '{f}'")).unwrap_or_default());
        println!("(el registro empieza con la primera `install` posterior a 2.1.0; reinstala para registrar las anteriores)");
    }
}

/// `skillcheck refrescar NOMBRE | --todas`: reinstala cada copia registrada
/// que quedó desactualizada, en el mismo lugar donde estaba. Pasa por la
/// misma validación que `install`, y se detiene en la primera que falle.
fn run_refrescar(filtro: Option<&str>, adoptar: bool) {
    let filas = leer_registro(Path::new(REGISTRO_INSTALACIONES));
    let mut refrescadas = 0;
    for (name, copia) in filas.iter().filter(|(n, _)| filtro.map_or(true, |f| f == n)) {
        let Some(fuente) = localizar_skill(name).into_iter().next() else {
            println!("omitida: '{name}' ya no existe en skills/ ni en priv-skills/ ({})", copia.display());
            continue;
        };
        match estado_copia(&fuente, copia) {
            "al_dia" => continue,
            "no_existe" => {
                println!("omitida: la copia de '{name}' ya no existe en {}", copia.display());
                continue;
            }
            _ => {}
        }
        let Some(dest_root) = copia.parent() else { continue };
        let home_skills = env::var_os("HOME").map(|h| PathBuf::from(h).join(".claude").join("skills"));
        let destino = if home_skills.as_deref() == Some(dest_root) {
            Destino::Global
        } else if fs::canonicalize(".claude/skills").ok().as_deref() == Some(dest_root) {
            Destino::Proyecto
        } else {
            match dest_root.parent().and_then(Path::parent) {
                Some(proyecto) => Destino::Destino(proyecto.to_path_buf()),
                None => continue,
            }
        };
        run_install(name, destino, adoptar);
        refrescadas += 1;
    }
    println!("{refrescadas} copia(s) refrescada(s)");
}

fn print_help() {
    println!("uso:");
    println!("  skillcheck lint [DIR]            valida las skills en DIR (por defecto: skills y priv-skills)");
    println!("  skillcheck install NOMBRE        instala la skill en .claude/skills (este proyecto)");
    println!("  skillcheck install NOMBRE --global   instala en ~/.claude/skills (todos los proyectos)");
    println!("  skillcheck install NOMBRE --destino DIR  instala en DIR/.claude/skills (otro proyecto)");
    println!("  skillcheck instalaciones [NOMBRE]     lista las copias registradas y si están al día");
    println!("  skillcheck refrescar NOMBRE|--todas   reinstala las copias registradas desactualizadas");
    println!("  skillcheck install NOMBRE --adoptar-vivencias");
    println!("      copia a la fuente los archivos que solo existen en la copia instalada");
    println!("      (vivencias) antes de reemplazarla, en vez de abortar");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut it = args.iter();

    match it.next().map(|s| s.as_str()) {
        Some("--help") | Some("-h") => print_help(),
        Some("install") => {
            let Some(name) = it.next() else {
                eprintln!("uso: skillcheck install NOMBRE [--global]");
                exit(2);
            };
            let uso = "uso: skillcheck install NOMBRE [--global | --destino DIR] [--adoptar-vivencias]";
            let mut global = false;
            let mut adoptar = false;
            let mut proyecto: Option<PathBuf> = None;
            while let Some(flag) = it.next() {
                match flag.as_str() {
                    "--global" => global = true,
                    "--adoptar-vivencias" => adoptar = true,
                    "--destino" => match it.next() {
                        Some(dir) => proyecto = Some(PathBuf::from(dir)),
                        None => {
                            eprintln!("error: --destino necesita el directorio del proyecto\n{uso}");
                            exit(2);
                        }
                    },
                    otro => {
                        eprintln!("error: opción desconocida '{otro}'\n{uso}");
                        exit(2);
                    }
                }
            }
            let destino = match (global, proyecto) {
                (true, Some(_)) => {
                    eprintln!("error: --global y --destino son excluyentes\n{uso}");
                    exit(2);
                }
                (true, None) => Destino::Global,
                (false, Some(p)) => Destino::Destino(fs::canonicalize(&p).unwrap_or(p)),
                (false, None) => Destino::Proyecto,
            };
            run_install(name, destino, adoptar);
        }
        Some("instalaciones") => run_instalaciones(it.next().map(|s| s.as_str())),
        Some("refrescar") => {
            let resto: Vec<&str> = it.map(|s| s.as_str()).collect();
            let adoptar = resto.contains(&"--adoptar-vivencias");
            let objetivo: Vec<&str> = resto.into_iter().filter(|a| *a != "--adoptar-vivencias").collect();
            match objetivo.as_slice() {
                ["--todas"] => run_refrescar(None, adoptar),
                [nombre] if !nombre.starts_with('-') => run_refrescar(Some(nombre), adoptar),
                _ => {
                    eprintln!("uso: skillcheck refrescar NOMBRE|--todas [--adoptar-vivencias]");
                    exit(2);
                }
            }
        }
        Some("lint") => match it.next() {
            Some(dir) => run_lint(&[PathBuf::from(dir)]),
            None => run_lint(&raices_existentes()),
        },
        Some(other) => run_lint(&[PathBuf::from(other)]),
        None => run_lint(&raices_existentes()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Crea un árbol temporal y devuelve su raíz. Sin dependencias externas:
    /// el repositorio no las tiene y este test no justifica introducir una.
    fn arbol_temporal(sufijo: &str) -> PathBuf {
        let raiz = env::temp_dir().join(format!("skillcheck-test-{sufijo}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        fs::create_dir_all(&raiz).unwrap();
        raiz
    }

    fn escribir(raiz: &Path, relativo: &str, contenido: &str) {
        let ruta = raiz.join(relativo);
        fs::create_dir_all(ruta.parent().unwrap()).unwrap();
        fs::write(ruta, contenido).unwrap();
    }

    #[test]
    fn reconoce_una_fuente_enlazada_a_su_propio_destino() {
        let raiz = arbol_temporal("enlace");
        let real = raiz.join("proyecto/.claude/skills/x");
        escribir(&real, "SKILL.md", "---\nname: x\ndescription: y\n---\n");
        let enlace = raiz.join("priv-skills/x");
        fs::create_dir_all(enlace.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&real, &enlace).unwrap();

        assert!(mismo_directorio(&enlace, &real));
        assert!(!mismo_directorio(&enlace, &raiz.join("otro/.claude/skills/x")),
            "un destino que todavía no existe no puede ser la fuente");
        fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn detecta_vivencias_que_solo_viven_en_la_copia_instalada() {
        let raiz = arbol_temporal("vivencias");
        let source = raiz.join("source");
        let dest = raiz.join("dest");
        escribir(&source, "SKILL.md", "---\nname: x\ndescription: y\n---\n");
        escribir(&dest, "SKILL.md", "---\nname: x\ndescription: y\n---\n");
        // Escrita por la skill mientras trabajaba: existe solo en la instalada.
        escribir(&dest, "vivencias/registro/2026-09-hallazgo.md", "no me borres");
        escribir(&dest, ".factoria-origen", "/ruta/a/la/fuente");

        let en_riesgo = archivos_solo_en_destino(&source, &dest).unwrap();

        assert_eq!(en_riesgo, vec![PathBuf::from("vivencias/registro/2026-09-hallazgo.md")],
            "el marcador .factoria-origen no debe contarse y la vivencia sí");
        fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn no_reporta_nada_cuando_la_copia_instalada_no_agrega_archivos() {
        let raiz = arbol_temporal("iguales");
        let source = raiz.join("source");
        let dest = raiz.join("dest");
        escribir(&source, "SKILL.md", "a");
        escribir(&source, "references/guia.md", "b");
        escribir(&dest, "SKILL.md", "a");
        escribir(&dest, "references/guia.md", "b distinto pero presente");

        assert!(archivos_solo_en_destino(&source, &dest).unwrap().is_empty(),
            "un archivo con contenido distinto no está en riesgo: install lo sobrescribe con la fuente");
        fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn el_registro_no_duplica_la_misma_copia_y_ordena() {
        let raiz = arbol_temporal("registro");
        let registro = raiz.join("instalaciones.tsv");
        registrar_instalacion(&registro, "zeta", Path::new("/p/.claude/skills/zeta")).unwrap();
        registrar_instalacion(&registro, "alfa", Path::new("/q/.claude/skills/alfa")).unwrap();
        registrar_instalacion(&registro, "zeta", Path::new("/p/.claude/skills/zeta")).unwrap();
        let filas = leer_registro(&registro);
        assert_eq!(filas, vec![
            ("alfa".to_string(), PathBuf::from("/q/.claude/skills/alfa")),
            ("zeta".to_string(), PathBuf::from("/p/.claude/skills/zeta")),
        ]);
        fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn el_estado_ignora_el_marcador_y_detecta_copias_viejas() {
        let raiz = arbol_temporal("estado");
        let fuente = raiz.join("fuente");
        let copia = raiz.join("copia");
        escribir(&fuente, "SKILL.md", "v2");
        escribir(&copia, "SKILL.md", "v2");
        escribir(&copia, ".factoria-origen", "/ruta/a/la/fuente");
        assert_eq!(estado_copia(&fuente, &copia), "al_dia", "el marcador no es una diferencia");
        escribir(&fuente, "SKILL.md", "v3");
        assert_eq!(estado_copia(&fuente, &copia), "desactualizada");
        assert_eq!(estado_copia(&fuente, &raiz.join("no-esta")), "no_existe");
        fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn excluye_de_git_una_sola_vez_y_segun_la_raiz() {
        let raiz = arbol_temporal("exclude");
        fs::create_dir_all(raiz.join(".git")).unwrap();
        let publica = excluir_de_git(&raiz, "demo", false).unwrap();
        assert_eq!(publica, vec!["/.claude/skills/demo/vivencias/", "/.claude/skills/demo/.factoria-origen"]);
        assert!(excluir_de_git(&raiz, "demo", false).unwrap().is_empty(), "no repite patrones");
        assert_eq!(excluir_de_git(&raiz, "mia", true).unwrap(), vec!["/.claude/skills/mia/"],
            "una skill personal se excluye entera");
        let sin_git = arbol_temporal("exclude-sin-git");
        assert!(excluir_de_git(&sin_git, "demo", false).unwrap().is_empty(), "sin .git no toca nada");
        fs::remove_dir_all(&raiz).unwrap();
        fs::remove_dir_all(&sin_git).unwrap();
    }

    #[test]
    fn splits_simple_frontmatter() {
        let content = "---\nname: foo\ndescription: bar\n---\nbody text\n";
        let (fm, body) = split_frontmatter(content).unwrap();
        assert_eq!(fm.trim(), "name: foo\ndescription: bar");
        assert_eq!(body, "body text\n");
    }

    #[test]
    fn returns_none_without_frontmatter() {
        assert!(split_frontmatter("# just a heading\n").is_none());
    }

    #[test]
    fn parses_folded_description() {
        let fm = "name: foo\ndescription:\n  first line\n  second line\n";
        let parsed = parse_frontmatter(fm);
        assert_eq!(parsed.name.as_deref(), Some("foo"));
        assert_eq!(parsed.description.as_deref(), Some("first line second line"));
    }

    #[test]
    fn strips_surrounding_quotes() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
        assert_eq!(strip_quotes("'hello'"), "hello");
        assert_eq!(strip_quotes("hello"), "hello");
    }

    #[test]
    fn accepts_only_opencode_skill_names() {
        assert!(is_opencode_name("prueba-y-error"));
        assert!(is_opencode_name("skill2"));
        assert!(!is_opencode_name("Prueba"));
        assert!(!is_opencode_name("dos--guiones"));
        assert!(!is_opencode_name("-invalido"));
    }
}
