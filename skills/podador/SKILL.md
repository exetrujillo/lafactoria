---
name: podador
description: >
  Usa esta skill al cerrar una sesión de trabajo: cuando el usuario diga
  "cerremos", "cerrar sesión" o "poda las memorias", cuando la invoque por su
  nombre, o cuando el workflow del proyecto la nombre en su paso de cierre.
  Revisa las memorias que el arnés guarda para el proyecto contra lo que la
  sesión enseñó y propone agregar, corregir o borrar, sin duplicar lo que ya
  dicen el código, git, el CHANGELOG o el CLAUDE.md, y no escribe nada sin el
  visto bueno del usuario. No es para guardar un dato suelto a mitad de
  sesión: eso el arnés lo hace solo.
---

# Podador

Durante una sesión el arnés guarda memorias al vuelo, de a una, y otras que ya
tenía dejan de ser ciertas sin que nadie las toque. Nadie hace la pasada de
cierre: leer el conjunto completo contra lo que la sesión mostró, y decidir
qué entra, qué se corrige y qué sobra. Eso hace esta skill. Propone un diff y
escribe solo lo aprobado.

## Antes de empezar

- Lee `vivencias/INDICE.md` como indica "Vivencias propias".
- Ubica el directorio de memorias del proyecto. El arnés lo declara en sus
  instrucciones de sesión, junto con el formato de cada memoria y el nombre
  del índice. Si no lo declara, `references/arneses.md` dice dónde lo guarda
  cada arnés conocido y cuáles no guardan ninguno. Si no hay directorio, dilo
  en una línea y termina: no lo crees, y no inventes un lugar.
- Lee el índice entero y cada memoria. Son cortas a propósito; si el índice
  supera lo que el arnés carga por sesión (ver la referencia), ese es el
  primer hallazgo, porque lo que sobra no se lee nunca.

## 1. Reunir lo que la sesión enseñó

Fuentes, en este orden:

- La conversación: correcciones del usuario, enfoques que confirmó después
  de una duda, decisiones con un motivo que el código no muestra, hechos del
  entorno que el repositorio no puede decir (una herramienta que falta en la
  máquina, un servicio que bloquea, un límite medido).
- Los commits de la sesión (`git log` desde que empezó) y `git status`, para
  no proponer como memoria lo que ya quedó versionado.
- El `HANDOFF.md`, el `PROMPT.md` o el CHANGELOG del proyecto, si los tiene:
  lo que la sesión escribió ahí ya tiene dueño y no se repite en una memoria.

Merece memoria lo que cambia cómo se trabaja la próxima vez y no se puede
derivar leyendo el repositorio. No merece: el arreglo (vive en el código), la
historia (vive en git), los detalles de la tarea, los pendientes (viven en el
HANDOFF), y cualquier cosa que ya diga el CLAUDE.md o la documentación del
proyecto. Una memoria que repite una regla del CLAUDE.md la contradice en
cuanto una de las dos cambia.

## 2. Cruzar con lo que ya hay

Para cada memoria existente, dos preguntas. ¿Sigue siendo cierta? Si nombra un
archivo, un comando, una opción o una ruta, compruébalo contra el repositorio
antes de decidir, no de memoria. ¿Duplica otra memoria, el CLAUDE.md o un
documento del proyecto? Si lo que dice ya vive en otro lado, la memoria apunta
ahí y no lo copia: una memoria es una referencia a donde está el detalle, no
una segunda copia que diverge.

Clasifica cada una, y cada hallazgo del paso 1, en una de cuatro: nueva,
corregir, borrar, queda. Dos hallazgos que dicen lo mismo se funden en uno.

## 3. Proponer

Muestra el diff como una lista: acción, nombre de la memoria, su contenido en
una línea y qué de la sesión lo sostiene. Todavía no escribas nada. Pide una
sola confirmación por la lista entera; el usuario acepta, cambia o saca
ítems. Una memoria que se borra se borra con su permiso, nunca por tu cuenta.

Si no hay nada que cambiar, dilo en una línea y termina. Es un resultado
legítimo, y una lista rellenada para parecer trabajo deja memorias que nadie
pidió.

## 4. Escribir

Solo lo aprobado, en el formato que el arnés dicta para sus memorias (el
frontmatter, el índice de una línea por memoria, un hecho por archivo). Una
memoria corregida conserva su nombre, para que lo que la enlazaba siga
sirviendo. Una borrada sale también del índice. Las fechas relativas ("ayer",
"la semana pasada") se escriben absolutas. Al terminar, el índice tiene una
línea por archivo y ningún contenido.

## Antes de cerrar: qué queda registrado

Entregado el diff, mira si esta pasada contradijo lo que esta skill
documenta: una clase de hallazgo que las listas del paso 1 no prevén, un
arnés cuyo directorio o formato no coincide con `references/arneses.md`, un
paso que el usuario saltó por inútil. Si es así, abre
`vivencias/registro/<fecha>-<tema>.md` con el hallazgo y agrega su línea a
`vivencias/INDICE.md` (`fecha | tema | resultado en pocas palabras |
archivo`).

Una pasada que salió como estaba previsto no se registra. Y lo que la sesión
enseñó sobre el proyecto va en las memorias del arnés, no en las vivencias de
esta skill, que solo guardan lo que la pasada enseñó sobre curar.

## Vivencias propias

**Dónde están las vivencias.** Este SKILL.md puede ser una copia instalada, y
`install` reemplaza la copia entera. Antes de leer o escribir cualquier ruta
`vivencias/` de este archivo, fíjate si junto a él hay un `.factoria-origen`:
su única línea es la ruta absoluta de la fuente, y `vivencias/` se lee y se
escribe en `<fuente>/vivencias/`, nunca en la copia. Sin marcador, este
directorio es la fuente. Si el marcador apunta a una ruta que ya no existe,
sigue sin vivencias (no leas ni escribas ninguna) y avísale al usuario que
reinstale la skill desde donde esté ahora su fuente.

Al comenzar, lee `vivencias/INDICE.md`, que tiene una línea por cada
hallazgo registrado, y abre la entrada de `vivencias/registro/` que nombra una
línea sólo si toca la tarea en curso. Escribir el registro sin leerlo no sirve:
una trampa ya documentada se repite igual.

`vivencias/ajustes.json` no declara claves todavía: la skill no tiene
preferencias que varíen por usuario. Escribe en `vivencias/` según "Antes de
cerrar: qué queda registrado".
