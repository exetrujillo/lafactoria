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
        for key in ["max_files", "max_bytes_por_archivo", "max_total_bytes"] {
            if json_number(ajustes, key).is_none() {
                errores.push(format!("falta la clave numérica 'ajustes.{key}'"));
            }
        }
    }

    // pulpo-librero declara dependencias reales de chatarrero y biblio-rata
    // (ver "Skills madre" en el SKILL.md), así que 'familia' es obligatoria acá,
    // a diferencia de las skills sin dependencias declaradas.
    match json_array(&content, "familia") {
        None => errores.push("falta el arreglo 'familia'".to_string()),
        Some(familia) => {
            let entradas = object_entries(familia);
            if entradas.is_empty() {
                errores.push("'familia' está vacío; pulpo-librero declara dependencias reales".to_string());
            }
            for entrada in entradas {
                for key in ["skill", "relacion", "desde"] {
                    if json_string(entrada, key).is_none() {
                        errores.push(format!("una entrada de 'familia' no tiene la clave de texto '{key}'"));
                    }
                }
            }
        }
    }

    if let Some(v) = json_string(&content, "version") {
        if v != ESQUEMA_ESPERADO {
            errores.push(format!(
                "'version' es \"{v}\" pero el esquema esperado de 'pulpo-librero' es \"{ESQUEMA_ESPERADO}\": no hay ningún cambio de esquema documentado todavía para esta skill, así que si ves este error revisá skills/pulpo-librero/scripts/validar_ajustes.rs — ESQUEMA_ESPERADO subió sin dejar la nota de qué cambió y qué clave de ajustes.json revisar"
            ));
        }
    }

    if !errores.is_empty() {
        for error in &errores {
            eprintln!("error: {error}");
        }
        exit(1);
    }
    println!("OK");
}
