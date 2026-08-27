---
name: forjador
description: >
  Usa esta skill cuando el usuario quiera crear, esbozar, refinar o publicar
  una skill de Claude Code en este repositorio. El forjador dirige la línea de
  producción: aclara el propósito con preguntas puntuales, genera el SKILL.md
  y sus archivos de soporte, valida con skillcheck, itera con el usuario y
  finalmente instala la skill terminada donde corresponda.
---

# Forjador

El forjador de la factoría dirige la creación de nuevas skills en
`skills/<nombre>/SKILL.md`. Objetivo: mínima cantidad de comandos y rondas de
preguntas, máxima claridad en el resultado.

## 1. Aclarar el propósito (una sola tanda de preguntas)

Antes de escribir nada, usa `AskUserQuestion` para resolver en una sola tanda:

- ¿Qué tarea o frase del usuario debe disparar esta skill? (esto define la
  `description`, que es lo único que Claude ve antes de decidir invocarla).
- ¿La skill la ejecuta el propio Claude en la conversación, o conviene que
  corra en un subagente / en background?
- ¿Necesita archivos de soporte (`references/`, `scripts/`, `assets/`) o le
  basta con instrucciones en el cuerpo del SKILL.md?
- ¿Esta skill es solo para este proyecto, o debería quedar disponible para
  cualquier proyecto? (define el alcance de la instalación en el paso 6).
- ¿Conviene investigar con `WebSearch`/`WebFetch` (documentación oficial,
  repos de referencia) antes de escribir la skill, o basta con el
  conocimiento ya disponible? Investigar gasta más tokens, así que esto se
  decide explícitamente y no por defecto.

No repitas esta ronda salvo que una respuesta abra una ambigüedad real que
bloquee continuar. Si la respuesta a la investigación fue sí, hazla en esta
misma etapa, antes del paso 2 — no la repitas más adelante salvo que surja
una duda puntual imposible de resolver de otro modo.

## 2. Elegir el nombre

- kebab-case, corto, sin redundancia (`forjador`, no `crear-skill-forjador`).
- El directorio `skills/<nombre>/` DEBE llamarse igual que el campo `name`
  del frontmatter — `skillcheck` lo exige.

## 3. Escribir el SKILL.md

Frontmatter mínimo:

```yaml
---
name: <nombre-kebab-case>
description: <una frase en tercera persona: CUÁNDO usar la skill y QUÉ hace>
---
```

Cuerpo: instrucciones paso a paso, concretas, sin relleno. Si hay archivos de
soporte en `scripts/`, `references/` o `assets/`, referénciarlos por su ruta
relativa completa para que `skillcheck` pueda verificar que existen de verdad
(el archivo debe existir en disco, no basta con mencionarlo).

## 4. Validar

Una sola corrida por iteración, desde la raíz del repo:

```sh
cargo run --quiet -- lint skills/<nombre>
```

o `cargo run --quiet -- lint` para validar todo el repo de una vez. Corrige
lo que reporte como `error:` antes de mostrarle nada al usuario; las
`advertencia:` son opcionales de resolver.

## 5. Iterar con el usuario

Muestra el SKILL.md resultante y pide una sola confirmación o ronda de ajustes
concreta — no repitas el ciclo de preguntas del paso 1. Agrupa lecturas y
escrituras; no vuelvas a correr `skillcheck` si no cambiaste el archivo.

## 6. Instalar (publicar la skill terminada)

`skillcheck` valida la skill de nuevo antes de instalarla y se niega a
instalar si quedan errores.

```sh
cargo run --quiet -- install <nombre>            # solo este proyecto: .claude/skills/<nombre>
cargo run --quiet -- install <nombre> --global   # todos los proyectos: ~/.claude/skills/<nombre>
```

Si la skill es `forjador` mismo u otra pensada para este repo, instálala en el
proyecto para que quede disponible en las conversaciones de Claude Code aquí.
Si el usuario definió en el paso 1 que la skill sirve para cualquier
proyecto, usa `--global`. Después de editar una skill ya instalada hay que
volver a correr `install` para refrescar la copia.

## Reglas que aplica skillcheck

- `name` y `description` son obligatorios y no pueden estar vacíos.
- `name` debe coincidir exactamente con el nombre del directorio.
- El cuerpo (fuera del frontmatter) no puede estar vacío.
- Toda ruta relativa a `references/`, `scripts/` o `assets/` mencionada en el
  cuerpo debe existir en disco.
- No puede haber dos skills con el mismo `name`.
- `install` rechaza skills con errores de validación.
