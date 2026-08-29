// Valida la forma de vivencias/ajustes.json contra el esquema fijo que
// documenta forjador/SKILL.md.
use std::env;
use std::fs;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("uso: validar_ajustes RUTA/A/ajustes.json");
        exit(2);
    }
    let path = &args[1];
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: no se pudo leer '{path}': {e}");
            exit(1);
        }
    };

    let mut errores = Vec::new();

    if !balanced(&content) {
        errores.push("las llaves o corchetes no están balanceados".to_string());
    }

    for key in ["skill", "version", "actualizado"] {
        if json_string(&content, key).is_none() {
            errores.push(format!("falta la clave de texto '{key}'"));
        }
    }
    if json_object(&content, "ajustes").is_none() {
        errores.push("falta el objeto 'ajustes'".to_string());
    }
    if json_array(&content, "notas").is_none() {
        errores.push("falta el arreglo 'notas'".to_string());
    }

    if let Some(ajustes) = json_object(&content, "ajustes") {
        if ajustes.contains("\"registro_lenguaje\"") && json_string(ajustes, "registro_lenguaje").is_none() {
            errores.push("'ajustes.registro_lenguaje' debería ser un string".to_string());
        }
    }

    if !errores.is_empty() {
        for e in &errores {
            eprintln!("error: {e}");
        }
        exit(1);
    }
    println!("OK");
}

fn balanced(s: &str) -> bool {
    let mut curly = 0i32;
    let mut square = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for c in s.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => curly += 1,
            '}' if !in_string => curly -= 1,
            '[' if !in_string => square += 1,
            ']' if !in_string => square -= 1,
            _ => {}
        }
        if curly < 0 || square < 0 {
            return false;
        }
    }
    curly == 0 && square == 0 && !in_string
}

// Busca "key": "value" y devuelve value, sin desescapar (alcanza para validar presencia y tipo).
fn json_string<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\"");
    let key_pos = s.find(&needle)?;
    let after_key = &s[key_pos + needle.len()..];
    let colon = after_key.find(':')?;
    let rest = after_key[colon + 1..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let inner = &rest[1..];
    let end = inner.find('"')?;
    Some(&inner[..end])
}

fn json_object<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    json_bracketed(s, key, '{', '}')
}

fn json_array<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    json_bracketed(s, key, '[', ']')
}

fn json_bracketed<'a>(s: &'a str, key: &str, open: char, close: char) -> Option<&'a str> {
    let needle = format!("\"{key}\"");
    let key_pos = s.find(&needle)?;
    let after_key = &s[key_pos + needle.len()..];
    let colon = after_key.find(':')?;
    let rest = after_key[colon + 1..].trim_start();
    if !rest.starts_with(open) {
        return None;
    }
    let bytes = rest.as_bytes();
    let mut depth = 0i32;
    for (i, &b) in bytes.iter().enumerate() {
        let c = b as char;
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(&rest[..=i]);
            }
        }
    }
    None
}
