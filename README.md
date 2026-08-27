# La Factoría

La Factoría es una biblioteca pública de skills de Claude Code. El flujo de trabajo principal no es "compilar software": es conversacional — las skills se escriben, validan e instalan hablando con Claude, apoyándose en **forjador**, la skill maestra del repositorio.

La única herramienta de soporte es `skillcheck`, un binario Rust sin dependencias externas que valida el formato de cada skill y las instala donde corresponda.

## Instalación

```sh
cargo build --release
cargo run --quiet -- install forjador
```

Ese segundo comando copia `skills/forjador` a `.claude/skills/forjador`, que es la ubicación donde Claude Code busca skills de este proyecto. A partir de ahí `forjador` queda disponible en cualquier conversación de Claude Code dentro de este repo.

## Comandos

```sh
cargo build --release                    # compilar skillcheck
cargo run --quiet -- lint [DIR]          # validar skills en DIR (por defecto: skills)
cargo run --quiet -- install NOMBRE      # publicar skills/NOMBRE en .claude/skills (este proyecto)
cargo run --quiet -- install NOMBRE --global   # publicar en ~/.claude/skills (todos los proyectos)
cargo test --quiet                       # tests unitarios del parser de frontmatter
```

`skillcheck lint` termina con código de salida `1` si hay algún `error:`.
`skillcheck install` corre la validación primero y se niega a instalar si
quedan errores.

## Estructura

- `skills/<nombre>/SKILL.md` — código fuente de cada skill: aquí se edita.
- `.claude/skills/<nombre>/` — copia instalada de una skill, generada por
  `install`, que es lo que Claude Code realmente lee en este proyecto. Si
  editas una skill en `skills/`, hay que volver a correr `install` para que
  el cambio se refleje.
- `src/main.rs` — el binario `skillcheck`.

## Crear una skill nueva

Pídele a Claude que use la skill **forjador** (o simplemente descríbele qué
quieres automatizar). Forjador se encarga de aclarar el propósito con
preguntas puntuales, escribir el `SKILL.md`, validarlo con `skillcheck` e
instalarlo donde corresponda — en este proyecto o de forma global, según lo
que necesites.

## Por qué Rust

Las skills que se documentan aquí pueden ser para cualquier lenguaje o
stack — eso es independiente. Pero las herramientas propias de este repo
(`skillcheck`) están en Rust: un binario nativo, sin runtime ni
dependencias, que valida y copia archivos rápido y sin sorpresas.
