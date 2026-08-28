# CLAUDE.md

This file provides project guidance for Claude Code, OpenCode, and other coding agents working in this repository.

## Qué es este repo

Repositorio de trabajo para diseñar y validar skills para agentes de código. El flujo
principal no es "compilar software": es conversacional. Se trabaja con un agente
para redactar, iterar y validar cada `skills/<nombre>/SKILL.md`, apoyándose en
la skill maestra `skills/forjador/SKILL.md`, que define el proceso paso a
paso (aclarar propósito → nombrar → escribir → validar → instalar → iterar).

El lenguaje principal del repo es **Rust**, usado exclusivamente para las
herramientas de soporte (el validador/instalador `skillcheck`). Las skills en
sí pueden documentar flujos de trabajo en cualquier lenguaje o stack.

OpenCode usa este `CLAUDE.md` como fallback si no existe un `AGENTS.md`, y
descubre las copias instaladas en `.claude/skills/`. No crear un `AGENTS.md`
redundante: tendría prioridad y podría divergir de este archivo.

Distinción importante: `skills/<nombre>/` es el código fuente editable de
cada skill; `.claude/skills/<nombre>/` es la copia instalada (generada por
`skillcheck install`) que los agentes realmente leen. Editar una skill sin
reinstalarla no tiene efecto en las conversaciones.

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
  (`--global`), reemplazando el destino si ya existe.
- `skills/<nombre>/SKILL.md` — cada subdirectorio de `skills/` es una skill
  candidata. Regla no negociable: el campo `name` del frontmatter debe ser
  idéntico al nombre del directorio que la contiene y cumplir el formato de
  OpenCode (`^[a-z0-9]+(-[a-z0-9]+)*$`, máximo 64 caracteres).
- `skills/forjador/SKILL.md` — skill maestra: orquesta la creación de nuevas
  skills (una sola tanda de preguntas, sin repetir rondas), las valida
  y las instala, y documenta explícitamente las reglas que aplica
  `skillcheck`. Si se cambia una regla de validación en `src/main.rs`,
  actualizar esa sección también para que no queden desalineadas.
