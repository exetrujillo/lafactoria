// Lectura de JSON anidado para respuestas de API. Sin dependencias externas,
// a propósito (mismo criterio que skillcheck).
//
// Es el hermano de json_util.rs, no su reemplazo: aquel alcanza para comprobar
// presencia y tipo de claves conocidas en un ajustes.json plano, y no sirve
// acá porque su conteo de llaves no distingue una '{' real de una escrita
// dentro de un string. Una respuesta de API trae abstracts con llaves,
// comillas escapadas y \uXXXX.
//
// Toda función recibe el ámbito donde buscar y sólo mira las claves de ese
// nivel, nunca las de un objeto anidado. Esa es la diferencia que importa:
// leer "is_oa" de open_access no puede devolver el de primary_location.
//
// Un valor null se lee como ausente. Ninguna fuente distingue hoy "no hay
// dato" de "el dato es null", y tratarlos igual evita un Option anidado en
// cada llamada.

fn json_value<'a>(scope: &'a str, key: &str) -> Option<&'a str> {
    let bytes = scope.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i] as char).is_ascii_whitespace() {
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'{' {
        i += 1;
    }
    let mut depth = 0i32;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                let (text, after) = json_string_at(scope, i)?;
                if depth > 0 {
                    i = after;
                    continue;
                }
                let mut j = after;
                while j < bytes.len() && (bytes[j] as char).is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b':' {
                    j += 1;
                    while j < bytes.len() && (bytes[j] as char).is_ascii_whitespace() {
                        j += 1;
                    }
                    if text == key {
                        return Some(&scope[j..]);
                    }
                }
                i = j;
            }
            b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

// Decodifica el string que empieza en scope[start] y devuelve dónde sigue el
// documento. Junta las unidades UTF-16 en vez de decodificar cada \u por
// separado para que un par sustituto (un carácter fuera del plano básico,
// como los símbolos matemáticos de un abstract) sobreviva entero.
fn json_string_at(scope: &str, start: usize) -> Option<(String, usize)> {
    let rest = scope.get(start..)?;
    if !rest.starts_with('"') {
        return None;
    }
    let mut units: Vec<u16> = Vec::new();
    let mut buffer = [0u16; 2];
    let mut chars = rest.char_indices().skip(1);
    while let Some((offset, c)) = chars.next() {
        match c {
            '"' => return Some((String::from_utf16_lossy(&units), start + offset + 1)),
            '\\' => match chars.next()?.1 {
                'u' => {
                    let mut hex = String::new();
                    for _ in 0..4 {
                        hex.push(chars.next()?.1);
                    }
                    units.push(u16::from_str_radix(&hex, 16).ok()?);
                }
                'n' => units.push(b'\n' as u16),
                'r' => units.push(b'\r' as u16),
                't' => units.push(b'\t' as u16),
                'b' => units.push(8),
                'f' => units.push(12),
                other => units.extend_from_slice(other.encode_utf16(&mut buffer)),
            },
            other => units.extend_from_slice(other.encode_utf16(&mut buffer)),
        }
    }
    None
}

// Delimita el objeto o arreglo que empieza al principio de rest, ignorando
// los corchetes que estén dentro de un string.
fn json_span(rest: &str) -> Option<&str> {
    let (open, close) = match rest.as_bytes().first()? {
        b'{' => (b'{', b'}'),
        b'[' => (b'[', b']'),
        _ => return None,
    };
    let bytes = rest.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        if byte == b'"' {
            i = json_string_at(rest, i)?.1;
            continue;
        }
        if byte == open {
            depth += 1;
        } else if byte == close {
            depth -= 1;
            if depth == 0 {
                return Some(&rest[..=i]);
            }
        }
        i += 1;
    }
    None
}

fn json_string(scope: &str, key: &str) -> Option<String> {
    let rest = json_value(scope, key)?;
    if !rest.starts_with('"') {
        return None;
    }
    Some(json_string_at(rest, 0)?.0)
}

fn json_number(scope: &str, key: &str) -> Option<String> {
    let rest = json_value(scope, key)?;
    let first = rest.chars().next()?;
    if first != '-' && !first.is_ascii_digit() {
        return None;
    }
    Some(
        rest.chars()
            .take_while(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))
            .collect(),
    )
}

fn json_bool(scope: &str, key: &str) -> Option<bool> {
    let rest = json_value(scope, key)?;
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn json_object<'a>(scope: &'a str, key: &str) -> Option<&'a str> {
    let rest = json_value(scope, key)?;
    if rest.starts_with('{') {
        json_span(rest)
    } else {
        None
    }
}

fn json_array<'a>(scope: &'a str, key: &str) -> Option<&'a str> {
    let rest = json_value(scope, key)?;
    if rest.starts_with('[') {
        json_span(rest)
    } else {
        None
    }
}

// Objetos de primer nivel dentro de un arreglo ya delimitado por json_array.
fn json_entries(array: &str) -> Vec<&str> {
    let mut entries = Vec::new();
    let bytes = array.as_bytes();
    let mut i = 1;
    while i + 1 < bytes.len() {
        match bytes[i] {
            b'"' => match json_string_at(array, i) {
                Some((_, after)) => i = after,
                None => break,
            },
            b'{' | b'[' => match json_span(&array[i..]) {
                Some(span) => {
                    if bytes[i] == b'{' {
                        entries.push(span);
                    }
                    i += span.len();
                }
                None => break,
            },
            _ => i += 1,
        }
    }
    entries
}

// Strings de primer nivel dentro de un arreglo ya delimitado por json_array.
fn json_texts(array: &str) -> Vec<String> {
    let mut texts = Vec::new();
    let bytes = array.as_bytes();
    let mut i = 1;
    while i + 1 < bytes.len() {
        match bytes[i] {
            b'"' => match json_string_at(array, i) {
                Some((text, after)) => {
                    texts.push(text);
                    i = after;
                }
                None => break,
            },
            b'{' | b'[' => match json_span(&array[i..]) {
                Some(span) => i += span.len(),
                None => break,
            },
            _ => i += 1,
        }
    }
    texts
}

// Internet Archive devuelve 'collection' como string cuando el ítem pertenece
// a una sola colección y como arreglo cuando pertenece a varias.
fn json_texts_or_text(scope: &str, key: &str) -> Vec<String> {
    if let Some(array) = json_array(scope, key) {
        return json_texts(array);
    }
    match json_string(scope, key) {
        Some(text) => vec![text],
        None => Vec::new(),
    }
}
