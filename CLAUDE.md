# CLAUDE.md

This file provides project guidance for Claude Code, OpenCode, and other coding agents working in this repository.

## Qué es este repo

Este documento cubre la ejecución: comandos, arquitectura de `skillcheck` y
las reglas que aplica el validador. Para el propósito del repositorio, los
principios de diseño, el ecosistema de skills y su ciclo de vida, ver
`README.md` — es la fuente de verdad sobre el sentido del proyecto. Ante una
contradicción entre ambos documentos, manda el README.

El binario `skillcheck` (herramienta de soporte, no una skill) está escrito
en **Rust** y no tiene dependencias externas; ver "Lenguajes del repo" en el
README para el criterio sobre el lenguaje de las skills en sí.

OpenCode usa este `CLAUDE.md` como fallback si no existe un `AGENTS.md`, y
descubre las copias instaladas en `.claude/skills/`. No crear un `AGENTS.md`
redundante: tendría prioridad y podría divergir de este archivo.

## Comandos

```sh
cargo build --release                        # compilar skillcheck
cargo run --quiet -- lint [DIR]              # validar skills en DIR (default: ./skills)
cargo run --quiet -- install NOMBRE          # instalar en .claude/skills (este proyecto)
cargo run --quiet -- install NOMBRE --global # instalar en ~/.claude/skills (todos los proyectos)
cargo test --quiet                           # tests unitarios del parser de frontmatter
```

`lint` termina con exit code `1` si hay algún `error:`. `install` corre `lint`
sobre la skill primero, verifica que la copia resultante sea idéntica a la
fuente y se niega a instalar si hay errores.

## Arquitectura

- `src/main.rs` — todo `skillcheck` vive en un solo binario, **sin
  dependencias externas** (ni `serde` ni `serde_yaml`). El frontmatter de un
  SKILL.md es siempre plano (`name`, `description`), así que se parsea a mano
  con un lector minimalista (`split_frontmatter` + `parse_frontmatter`) que
  soporta valores de una línea, comillas y bloques plegados (`description: >`
  seguido de líneas indentadas). No reintroducir `serde_yaml` u otra
  dependencia salvo que el frontmatter deje de ser plano.
- `lint_skill` valida un directorio de skill contra las reglas obligatorias
  (ver abajo); `lint_all` recorre `skills/*` y además detecta nombres
  duplicados entre skills distintas.
- `check_referenced_paths` escanea el cuerpo del SKILL.md buscando tokens que
  empiecen con `references/`, `scripts/` o `assets/` seguidos de algo más (no
  la sola mención del directorio) y verifica que ese archivo exista relativo
  al directorio de la skill. Al escribir instrucciones dentro de una skill,
  no usar rutas de ejemplo inventadas con esos prefijos — el linter las trata
  como referencias reales que deben existir en disco.
- `copy_dir_recursive` + `run_install` implementan `skillcheck install`: validan
  la skill, y copian (no symlink) `skills/<nombre>` completo a
  `.claude/skills/<nombre>` (proyecto) o `$HOME/.claude/skills/<nombre>`
  (`--global`), reemplazando el destino si ya existe. Después de copiar,
  `directories_equal` compara ambos árboles byte a byte y aborta si difieren.
- Terminada esa verificación, `run_install` escribe `.factoria-origen` en la
  copia instalada (para qué sirve este marcador, ver "Memoria" en el
  README). **Se escribe después de `directories_equal` a propósito**:
  escribirlo antes hace fallar la comparación de árboles. Está en
  `.gitignore` porque contiene una ruta local.
- `skills/<nombre>/SKILL.md` — cada subdirectorio de `skills/` es una skill
  candidata. Regla no negociable: el campo `name` del frontmatter debe ser
  idéntico al nombre del directorio que la contiene y cumplir el formato de
  OpenCode (`^[a-z0-9]+(-[a-z0-9]+)*$`, máximo 64 caracteres).
- `skills/forjador/SKILL.md` — skill maestra: orquesta la creación de nuevas
  skills (una o varias rondas de preguntas, sin repetirlas salvo ambigüedad
  real), las valida y las instala, y documenta explícitamente las reglas que
  aplica `skillcheck`. Si se cambia una regla de validación en `src/main.rs`,
  actualizar esa sección también para que no queden desalineadas.

## Convenciones de código

`src/main.rs` ya practica un estilo concreto; mantenerlo al tocar el binario:

- Sin traits, genéricos ni wrappers para lo que hoy es un binario único y simple. Funciones planas y structs de datos llanos (`Frontmatter`, `Report`), no jerarquías.
- Errores como `String`/mensajes formateados, no un enum de error propio ni una dependencia como `thiserror`, mientras el binario siga siendo un CLI de un solo archivo de este tamaño.
- `unwrap`/`expect` sólo cuando el propio código ya descartó el caso malo unas líneas antes (p. ej. tras un `match` que ya filtró `None`). Si la precondición viene de fuera (archivo, CLI), usar `Result`/`?`, no asumir.
- Sin comentarios que parafraseen la línea siguiente — el archivo está a cero comentarios de ese tipo. Un comentario sólo se justifica cuando documenta un motivo no obvio, como la nota sobre el orden de escritura de `.factoria-origen` más arriba.
- No sumar una dependencia (`serde`, `serde_yaml`, etc.) mientras el frontmatter siga siendo plano: es el mismo criterio de "no resolver problemas hipotéticos" aplicado a este binario en concreto.

Después de un cambio no trivial en `src/main.rs`, releer el diff contra
estas reglas antes de darlo por terminado — con la herramienta de revisión
de código que ofrezca el arnés si hay una disponible, o a mano si no. (No
nombrar acá el comando propio de un arnés concreto: violaría "Neutralidad
entre arneses" del README, que aplica a este documento igual que a una
skill.)

## Flujo de cambios

- Diff chico: tocar sólo lo que pide el cambio puntual, sin reformatear ni
  arrastrar código no relacionado en el mismo edit.
- Comandos de shell atómicos por defecto, no como regla absoluta. Para un
  cambio chico con una secuencia ya conocida y de bajo riesgo (por ejemplo
  `cargo build --release && cargo test --quiet` después de un ajuste
  trivial), encadenar está bien. La regla se pone estricta cuando el
  resultado es incierto o se está diagnosticando un fallo: ahí sí, un
  comando a la vez, leer su salida y código de salida antes de decidir el
  siguiente paso — encadenado, un fallo intermedio queda enmascarado y hace
  perder la traza real del error.
- Tras editar `src/main.rs`, correr `cargo test --quiet` antes de seguir con
  el próximo cambio — no acumular varias ediciones sin haber corrido los
  tests entre medio.
