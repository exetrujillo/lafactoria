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

El forjador de la factoría dirige la creación y actualización de skills en
`skills/<nombre>/SKILL.md`. Objetivo: máxima claridad en entender qué quiere
el usuario y qué necesita, para luego generar o actualizar el resultado.

## 1. Aclarar el propósito

Antes de escribir nada, usa la herramienta disponible para hacer preguntas al usuario y
resolver en una o varias rondas, dependiendo de si una pregunta pudiera depender de
otra respuesta:

- ¿Qué tarea o frase del usuario debe disparar esta skill? (para definir `description`,
  que es lo único que el agente ve antes de decidir invocarla).
- ¿Esta skill es solo para este proyecto, o debería quedar disponible para
  cualquier proyecto? (define alcance de instalación en paso 7).
- ¿Conviene investigar con las herramientas web disponibles (documentación oficial,
  repos de referencia) antes de escribir la skill, o basta con el
  conocimiento ya disponible? Investigar gasta más tokens, así que esto se
  decide explícitamente y no por defecto.

Si tienes una recomendación clara para alguna, ofrécela como opción marcada
"(Recomendado)" dentro de esa misma pregunta; no la respondas tú por el usuario
ni la omitas de la tanda. Esto aplica en particular a la pregunta de investigación:
no importa qué creas que es obvio o mejor: la decisión es del usuario y podemos hacer
varias rondas para llegar a una decisión clara.

Cada opción tiene que llevar su **contra explícita** en la descripción.
Si una alternativa tiene un costo real —más contexto, más fricción,
menos control, un riesgo asumido—, dilo ahí, aunque sea la opción
que estás recomendando.

Si la skill que se está creando va a tener un disparador amplio y su ejecución
cuesta caro, evalúa con el usuario el patrón de **compuerta**: cuándo aplica,
qué es y su contra están en references/patron-compuerta.md, pero léelo solo si
esta condición se cumple.

No repitas esta ronda salvo que una respuesta abra una ambigüedad real que
bloquee continuar o si aún se puede aclarar mejor la intención del usuario y
el alcance de su petición. Si la respuesta a la investigación fue sí, hazla en esta
misma etapa, antes del paso 2 — no la repitas más adelante salvo que surja
una duda puntual imposible de resolver de otro modo.

## 2. Elegir el nombre

- kebab-case, corto, sin redundancia (`forjador`, no `crear-skill-forjador`).
  Si la skill va a ser de las principales de La Factoría, la convención es usar
  un nombre entretenido y memorable, aunque pueda no ser el más descriptivo.
  (Ejemplos: `forjador`, `biblio-rata`, `prueba-y-error`, etc).
- El directorio `skills/<nombre>/` DEBE llamarse igual que el campo `name`
  del frontmatter — `skillcheck` lo exige.
- En este paso se crea también la estructura completa de `vivencias/` para la
  skill nueva: `ajustes.json` mínimo e `INDICE.md` vacío, aunque todavía no
  haya nada que registrar. Es un esqueleto que existe desde el nacimiento de
  la skill, no algo que se agrega recién cuando hace falta.

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
(el archivo debe existir en disco, no basta con mencionarlo). Las menciones a
`vivencias/` no pasan por esa verificación (ver "Reglas que aplica
skillcheck").

Si el cuerpo declara claves propias de `ajustes.json`, escribe también su
validador de vivencias en `scripts/`, en Rust y sin dependencias externas
(mismo patrón que `skills/pulpo-librero/scripts/`: `rustc archivo.rs -O -o
/tmp/binario`, JSON extraído a mano, sin `serde`). Comprueba que el
`ajustes.json` real tiene las llaves balanceadas, las claves declaradas y sus
tipos coinciden. No valida contenido de negocio, solo la forma del archivo.

No reescribas a mano el lector de JSON: copia `skills/forjador/scripts/json_util.rs`
tal cual al `scripts/` de la skill nueva y agrégale un `include!("json_util.rs");`
al `validar_ajustes.rs` (mismo patrón en las cinco skills existentes con
validador). Es una copia deliberada, no un módulo compartido en `src/`: cada
skill viaja sola cuando se instala `--global` en otro proyecto, así que su
`scripts/` tiene que alcanzarle sin depender de nada fuera de
`skills/<nombre>/`. Si `json_util.rs` gana una función nueva, propagala a las
demás copias en la misma edición.

Si el cuerpo declara **herencia** o **crianza** de otra skill (una sección de
tipo "Skills madre", "hereda de..." — ver "El ecosistema" en el README para la
distinción), escribe o actualiza también la clave `familia` en
`vivencias/ajustes.json`, con un objeto por relación (`skill`, `relacion`,
`desde`; formato y ejemplo en la sección "Vivencias" del README). `desde` es
la fecha de esta edición, no una fecha histórica que no se pueda verificar. No
declares acá una relación de uso, estudio o autointeracción: esas no llevan
`familia` (ver README para el motivo).

Si la relación es **crianza**, además escribe la entrada recíproca en la clave
`criados` del `ajustes.json` de cada padre declarado (`skill`, `desde`, sin
`relacion` porque `criados` es solo para crianza). Esto implica tocar el
`ajustes.json` de una skill distinta a la que estás editando: hacelo en la
misma sesión, con la misma fecha en `desde`. La herencia no lleva reciprocidad.

Si la skill (hija o padre) ya tenía un validador de vivencias, extiende sus
comprobaciones para cubrir también la forma de `familia` y/o `criados` según
corresponda (mismo patrón: presencia del arreglo, y que cada objeto tenga sus
claves de texto).

`version` en `ajustes.json` es el número de esquema del contrato
`ajustes`/`familia`/`criados` de esa skill (ver "Vivencias" en el README), no
la versión de la skill ni la del ecosistema. `forjador` es quien sube el
`ESQUEMA_ESPERADO` del `validar_ajustes.rs` de una skill cuando una edición
cambia el **significado** de una clave existente (no solo agrega o quita
claves, que ya cubre la comprobación de forma existente). El mensaje de error
de esa comparación tiene que ser autosuficiente — qué cambió y qué clave de
`ajustes.json` revisar — para que cualquier agente, no solo uno que invoque
`forjador`, pueda arreglar el archivo a mano.

Si el cambio de esquema es al propio `vivencias/ajustes.json` de `forjador`,
migralo en la **misma edición** en que subís su `ESQUEMA_ESPERADO` — mismo
criterio que la reciprocidad `familia`/`criados` de más arriba. Si no, la
skill queda autobloqueada sin salida: la copia instalada de `forjador`
todavía no conoce el cambio nuevo (vive sin instalar en la fuente, bloqueada
por su propio gate), así que no hay forma de invocarlo para que ayude a
migrarse a sí mismo. Con cualquier otra skill esto no pasa, porque `forjador`
sigue instalado y disponible mientras la migra.

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

Corrige todos los `error:` antes de mostrarle nada al usuario. Si la skill
tiene validador de vivencias (paso 3), corre también ese script contra su
`ajustes.json` — `skillcheck lint` no lo conoce todavía, así que es una
corrida aparte.

## 5. Auditar la lógica (condicional)

Si la skill tiene lógica no trivial —compuerta, cascada, coordinación de
subagentes, o un contrato que otras skills van a consumir—, sigue
references/auditoria-logica.md antes del paso 6. Una skill es lineal y simple
cuando no tiene bifurcaciones de decisión, no abre con compuerta y no
coordina subagentes; en ese caso, salta directo al paso 6.

## 6. Iterar con el usuario

Muestra el SKILL.md resultante y pide una sola confirmación o ronda de ajustes
concreta — no repitas el ciclo de preguntas del paso 1. Agrupa lecturas y
escrituras; no vuelvas a correr `skillcheck` si no cambiaste el archivo.

## 7. Instalar (publicar la skill terminada)

`skillcheck` valida la skill de nuevo antes de instalarla y se niega a
instalar si quedan errores. Si la skill declaró `scripts/validar_ajustes.rs`
(paso 3) y ya existe `vivencias/ajustes.json` en la fuente, `install` compila
ese validador con `rustc` y lo corre contra el `ajustes.json` real antes de
copiar; un validador que falla aborta la instalación con el mismo criterio
que un error de `lint`. Si falta cualquiera de los dos archivos (skill sin
validador propio, o vivencias todavía no creadas en un clon fresco), `install`
no valida nada y sigue de largo.

```sh
cargo run --quiet -- install <nombre>            # solo este proyecto: .claude/skills/<nombre>
cargo run --quiet -- install <nombre> --global   # todos los proyectos: ~/.claude/skills/<nombre>
```

Si la skill es `forjador` mismo u otra pensada para este repo, instálala en el
proyecto para que quede disponible en las conversaciones de agentes aquí.
Si el usuario definió en el paso 1 que la skill sirve para cualquier
proyecto, usa `--global`. Después de editar una skill ya instalada hay que
volver a correr `install` para refrescar la copia.

## Vivencias propias

`forjador` declara una clave en su propio `vivencias/ajustes.json`:
`registro_lenguaje` (string) — el registro en el que este usuario quiere que
se escriban las skills y las respuestas de esta skill. Antes de escribir
cualquier texto (preguntas, `SKILL.md`, referencias), lee ese archivo y
respeta el valor declarado.

`scripts/validar_ajustes.rs` valida que el archivo tenga las llaves
balanceadas, las claves declaradas y sus tipos esperados; no valida
contenido de negocio.

```sh
rustc skills/forjador/scripts/validar_ajustes.rs -O -o /tmp/forjador-validar
/tmp/forjador-validar skills/forjador/vivencias/ajustes.json
```

## Reglas que aplica skillcheck

- El `SKILL.md` debe empezar con un bloque de frontmatter YAML delimitado por
  `---`.
- `name` y `description` son obligatorios y no pueden estar vacíos.
- `name` debe coincidir exactamente con el nombre del directorio y cumplir el
  formato de OpenCode: minúsculas ASCII, dígitos y guiones simples, sin guion
  al principio ni al final, máximo 64 caracteres.
- `description` no puede superar 1024 caracteres.
- El cuerpo (fuera del frontmatter) no puede estar vacío.
- Toda ruta relativa a `references/`, `scripts/` o `assets/` mencionada en el
  cuerpo debe existir en disco.
- No puede haber dos skills con el mismo `name`.
- `install` rechaza skills con errores de validación.
- `vivencias/` queda fuera de la verificación anterior a propósito: no está
  versionada, así que en un clon fresco legítimamente puede no existir
  todavía.
