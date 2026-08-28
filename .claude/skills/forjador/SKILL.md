---
name: forjador
description: >
  Usa esta skill cuando el usuario quiera crear, esbozar, refinar o publicar
  una skill compatible con agentes de código en este repositorio. El forjador dirige la línea de
  producción: aclara el propósito con preguntas puntuales, genera el SKILL.md
  y sus archivos de soporte, valida con skillcheck, itera con el usuario y
  finalmente instala la skill terminada donde corresponda.
---

# Forjador

El forjador de la factoría dirige la creación de nuevas skills en
`skills/<nombre>/SKILL.md`. Objetivo: mínima cantidad de comandos y rondas de
preguntas, máxima claridad en el resultado.

## 1. Aclarar el propósito (una sola tanda de preguntas)

Antes de escribir nada, usa la herramienta disponible para hacer preguntas al usuario y
resolver en una o varias rondas, dependiendo de si una pregunta pudiera depender de
otra respuesta:

- ¿Qué tarea o frase del usuario debe disparar esta skill? (esto define la
  `description`, que es lo único que el agente ve antes de decidir invocarla).
- ¿La skill la ejecuta el agente en la conversación, o conviene que
  corra en un subagente / en background?
- ¿Necesita archivos de soporte (`references/`, `scripts/`, `assets/`) o le
  basta con instrucciones en el cuerpo del SKILL.md?
- ¿Esta skill es solo para este proyecto, o debería quedar disponible para
  cualquier proyecto? (define el alcance de la instalación en el paso 6).
- ¿Conviene investigar con las herramientas web disponibles (documentación oficial,
  repos de referencia) antes de escribir la skill, o basta con el
  conocimiento ya disponible? Investigar gasta más tokens, así que esto se
  decide explícitamente y no por defecto.

Las 5 preguntas van juntas en la misma llamada a la herramienta de preguntas — no
descartes en silencio la que te parezca "obvia". Si tienes una recomendación
clara para alguna, ofrécela como opción marcada "(Recomendado)" dentro de esa
misma pregunta; no la respondas tú por el usuario ni la omitas de la tanda.
Esto aplica en particular a la pregunta de investigación: aunque tu criterio
sea que no hace falta, la decisión es del usuario, no tuya por defecto.

Cada opción tiene que llevar su **contra explícita** en la descripción, no solo
su etiqueta. Una tanda donde todas las opciones suenan bien no es una decisión:
es un trámite. Si una alternativa tiene un costo real —más contexto, más
fricción, menos control, un riesgo asumido—, decilo ahí, aunque sea la opción
que estás recomendando.

Si la skill que se está creando va a tener un disparador amplio y su ejecución
cuesta caro (muchas corridas, mucho contexto, decisiones difíciles de revertir),
evaluá con el usuario el patrón de **compuerta**: la skill se autoinvoca, pero su
cuerpo abre mandando a preguntar antes de actuar, con las alternativas y sus
costos sobre la mesa. Se paga un clic y se evita arrancar trabajo que el usuario
no pidió.

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
cargo run --quiet -- lint
```

`lint` recibe el directorio que **contiene** las skills (por defecto `./skills`),
no una skill suelta. Pasarle `skills/<nombre>` hace que trate a sus
subdirectorios `references/` y `scripts/` como si fueran skills y reporte
errores falsos del tipo `[scripts] falta el archivo SKILL.md`. Valida el repo
entero de una vez, que además es una sola corrida.

Corrige todos los `error:` antes de mostrarle nada al usuario.

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
    proyecto para que quede disponible en las conversaciones de agentes aquí.
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
