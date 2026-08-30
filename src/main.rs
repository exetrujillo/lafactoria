use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

const MAX_DESCRIPTION_LEN: usize = 1024;
const MAX_NAME_LEN: usize = 64;
const RESOURCE_PREFIXES: [&str; 3] = ["references/", "scripts/", "assets/"];

// Copia canónica del lector de JSON que cada skill con validador de
// vivencias copia tal cual a su propio scripts/json_util.rs (ver "Escribir
// el SKILL.md" en forjador/SKILL.md). Vive acá, no en runtime, para que
// lint_skill pueda comparar bytes sin ninguna otra fuente de verdad.
const JSON_UTIL_CANONICO: &str = include_str!("json_util.rs");

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

fn check_json_util(skill_dir: &Path, skill: &str, report: &mut Report) {
    let path = skill_dir.join("scripts").join("json_util.rs");
    if !path.is_file() {
        return;
    }
    match fs::read_to_string(&path) {
        Ok(content) if content == JSON_UTIL_CANONICO => {}
        Ok(_) => report.error(
            skill,
            "scripts/json_util.rs difiere de la copia canónica en src/json_util.rs; los cambios a ese lector de JSON se hacen ahí y se propagan tal cual a cada skill",
        ),
        Err(e) => report.error(skill, format!("no se pudo leer scripts/json_util.rs: {e}")),
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
        Some(n) => seen_names.push((n.clone(), dir_name.clone())),
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

    check_json_util(skill_dir, &dir_name, report);
}

fn lint_all(skills_dir: &Path) -> Report {
    let mut report = Report::new();
    let mut seen_names: Vec<(String, String)> = Vec::new();

    let entries = match fs::read_dir(skills_dir) {
        Ok(e) => e,
        Err(e) => {
            report.error(
                skills_dir.to_string_lossy().as_ref(),
                format!("no se pudo leer el directorio: {e}"),
            );
            return report;
        }
    };

    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    if dirs.is_empty() {
        println!("(no se encontraron skills en {})", skills_dir.display());
    }

    for dir in &dirs {
        lint_skill(dir, &mut seen_names, &mut report);
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

fn run_lint(dir: &str) {
    let report = lint_all(&PathBuf::from(dir));

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

fn directories_equal(src: &Path, dst: &Path) -> std::io::Result<bool> {
    let mut src_entries: Vec<_> = fs::read_dir(src)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() != "__pycache__")
        .collect();
    let mut dst_entries: Vec<_> = fs::read_dir(dst)?.filter_map(Result::ok).collect();
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

fn run_install(name: &str, global: bool) {
    let source = PathBuf::from("skills").join(name);
    if !source.is_dir() {
        eprintln!("error: no existe skills/{name}");
        exit(1);
    }

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

    let dest_root = if global {
        match env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(".claude").join("skills"),
            None => {
                eprintln!("error: no se pudo determinar $HOME para la instalación global");
                exit(1);
            }
        }
    } else {
        PathBuf::from(".claude").join("skills")
    };

    let dest = dest_root.join(name);
    if dest.exists() {
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
            eprintln!("error: la copia instalada no coincide con skills/{name}");
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

    let alcance = if global { "global (disponible en todos los proyectos)" } else { "de este proyecto" };
    println!("'{name}' instalada en {} — alcance {alcance}", dest.display());
}

fn print_help() {
    println!("uso:");
    println!("  skillcheck lint [DIR]            valida las skills en DIR (por defecto: skills)");
    println!("  skillcheck install NOMBRE        instala skills/NOMBRE en .claude/skills (este proyecto)");
    println!("  skillcheck install NOMBRE --global   instala en ~/.claude/skills (todos los proyectos)");
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
            let global = it.next().map(|s| s.as_str()) == Some("--global");
            run_install(name, global);
        }
        Some("lint") => {
            let dir = it.next().cloned().unwrap_or_else(|| "skills".to_string());
            run_lint(&dir);
        }
        Some(other) => run_lint(other),
        None => run_lint("skills"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
