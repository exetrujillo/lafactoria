// Lectura mínima de JSON plano para validadores de vivencias/ajustes.json.
// Sin dependencias externas, a propósito (mismo criterio que skillcheck).
// No parsea JSON general: alcanza para comprobar presencia y tipo de claves
// conocidas de antemano, que es todo lo que necesita un validar_ajustes.rs.

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

// Busca una clave JSON y comprueba que su valor empiece por un número.
fn json_number<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\"");
    let key_pos = s.find(&needle)?;
    let after_key = &s[key_pos + needle.len()..];
    let colon = after_key.find(':')?;
    let rest = after_key[colon + 1..].trim_start();
    if rest.starts_with('-') || rest.chars().next()?.is_ascii_digit() {
        Some(rest)
    } else {
        None
    }
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

// Extrae cada objeto de nivel superior dentro de un arreglo ya delimitado
// por [ ... ] (lo que devuelve json_array).
fn object_entries(array: &str) -> Vec<&str> {
    let inner = &array[1..array.len() - 1];
    let bytes = inner.as_bytes();
    let mut entries = Vec::new();
    let mut depth = 0i32;
    let mut start = None;
    for (i, byte) in bytes.iter().enumerate() {
        let c = *byte as char;
        if c == '{' {
            if depth == 0 {
                start = Some(i);
            }
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                if let Some(s) = start {
                    entries.push(&inner[s..=i]);
                }
                start = None;
            }
        }
    }
    entries
}
