// Valida la forma de vivencias/ajustes.json contra el esquema fijo que
// documenta prueba-y-error/SKILL.md.
use std::env;
use std::fs;
use std::process::exit;

include!("json_util.rs");

// Número de esquema del contrato ajustes/familia/criados de esta skill, no
// la versión de la skill ni del ecosistema. Sube solo cuando cambia la forma
// o el significado de una clave (ver "Vivencias" en el README).
const ESQUEMA_ESPERADO: &str = "1";

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
        for key in ["confianza", "remuestreos", "alpha"] {
            if json_number(ajustes, key).is_none() {
                errores.push(format!("falta la clave numérica 'ajustes.{key}'"));
            }
        }
    }

    if let Some(v) = json_string(&content, "version") {
        if v != ESQUEMA_ESPERADO {
            errores.push(format!(
                "'version' es \"{v}\" pero el esquema esperado de 'prueba-y-error' es \"{ESQUEMA_ESPERADO}\": no hay ningún cambio de esquema documentado todavía para esta skill, así que si ves este error revisá skills/prueba-y-error/scripts/validar_ajustes.rs — ESQUEMA_ESPERADO subió sin dejar la nota de qué cambió y qué clave de ajustes.json revisar"
            ));
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
