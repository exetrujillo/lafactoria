# La Factoría

La Factoría es una biblioteca pública de skills para agentes de código que además crean sus propias skills. El flujo de trabajo principal es conversacional. Las skills se escriben, validan e instalan hablando con un agente, apoyándose en **forjador**, la skill maestra del repositorio. Además, las skills de este repo son al mismo tiempo herramientas para crear skills, que pueden interactuar entre ellas para mejorarse o para crear skills hijas.

La única herramienta de soporte del conjunto de skills de este repositorio es `skillcheck`, un binario Rust sin dependencias externas que valida el formato de cada skill y las instala donde corresponda.

## Instalación

```sh
cargo build --release
cargo run --quiet -- install forjador
```

Ese segundo comando copia `skills/forjador` a `.claude/skills/forjador`, una ubicación compatible tanto con Claude Code como con OpenCode. A partir de ahí `forjador` queda disponible para ser usada por los agentes de IA.

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
  `install`, que es lo que los agentes realmente leen en este proyecto. Si
  editas una skill en `skills/`, hay que volver a correr `install` para que
  el cambio se refleje.
- `src/main.rs` — el binario `skillcheck`.

## Crear una skill nueva

Pídele al agente que use la skill **forjador** (o simplemente descríbele qué
quieres automatizar). Forjador se encarga de aclarar el propósito con
preguntas puntuales, escribir el `SKILL.md`, validarlo con `skillcheck` e
instalarlo donde corresponda — en este proyecto o de forma global, según lo
que necesites.

## OpenCode

OpenCode es compatible con este repositorio sin configuración adicional: usa
`CLAUDE.md` como fallback para las instrucciones del proyecto y descubre las
skills instaladas en `.claude/skills/`. No se añade un `AGENTS.md` redundante,
porque tendría prioridad sobre `CLAUDE.md` y podría hacer que ambas fuentes se
desincronicen.

Las skills usan el frontmatter estándar `name` y `description`, y `skillcheck`
valida además las restricciones de nombres y longitud que exige OpenCode.

## Por qué Rust

Las skills que se documentan aquí pueden ser para cualquier lenguaje o
stack — eso es independiente. Pero las herramientas propias de este repo
(`skillcheck`) están en Rust: un binario nativo, sin runtime ni
dependencias, que valida y copia archivos rápido y sin sorpresas.
Se intentará además, que de usar herramientas para una skill en particular,
estas tengan disponibles opciones para usar en Rust o en Python dependiendo
de la necesidad, de la eficiencia y de la disponibilidad de herramientas
en el entorno del usuario.
