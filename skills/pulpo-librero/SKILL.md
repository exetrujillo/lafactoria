---
name: pulpo-librero
description: Usa esta skill cuando el usuario quiera reunir o descargar papers y libros desde fuentes abiertas, incluida la literatura reciente y de frontera de un tema, para incorporarlos a una biblioteca local compatible con biblio-rata.
---

# Pulpo-librero

Pulpo-librero hace crecer una biblioteca local a partir de un problema
bibliográfico concreto. No responde preguntas sobre el corpus (para eso está
`biblio-rata`) ni crea scrapers generales (para eso está `chatarrero`). Decide
qué documentación hace falta, consulta sus scrapers acoplados a fuentes
concretas, obtiene copias abiertas, verifica los archivos y deja trazable cómo
entraron a la biblioteca.

## Skills madre

`pulpo-librero` compone dos skills madre independientes y no modifica ninguna
de ellas. Consulta `chatarrero` para reconocer fuentes, sondear interfaces y
elegir el nivel de fetching más barato que entregue contenido real; sus
aprendizajes generales no se convierten en entradas sobre un corpus concreto.
Consulta `biblio-rata` para conocer el contrato del corpus, comprobar que un PDF
pueda abrirse con el extractor disponible e indexar incrementalmente los
documentos aceptados. Pulpo coordina ambos saberes y conserva sus propios
adaptadores, manifiesto y criterios bibliográficos.

## Compuerta

Antes de buscar o descargar, pregunta en una sola tanda:

1. ¿Qué tema, autores, títulos, identificadores o colección se busca?
2. ¿En qué biblioteca entra la ronda y qué límite de cantidad, tamaño o tiempo
   tiene la corrida? Antes de preguntar, busca las que ya tiene el proyecto
   (cada `catalogo.tsv` bajo `docs/literatura/`) y preséntalas: la ronda va a
   una de ellas o abre una sub-biblioteca nueva (ver "Sub-bibliotecas").
3. ¿Qué fuentes abiertas se autorizan? Si no se especifica, propone según el
   campo y el tipo de obra: arXiv, Semantic Scholar y Hugging Face Papers para
   IA y computación; Crossref y OpenAlex para cualquier campo con DOI; OSF
   Preprints, HAL y EconStor para ciencias sociales, estadística y economía;
   Europe PMC y bioRxiv para neurociencias y biomedicina; SciELO y LA
   Referencia para literatura hispana; Internet Archive para libros. La tabla
   de `references/scrapers.md` dice qué entrega cada una.
4. ¿Se quieren obras nuevas, versiones concretas o una lista cerrada de URLs?
5. Si ya existe `catalogo.tsv`, ¿se deben revisar sus pendientes, incorporar una
   nueva fuente o descargar solo las obras ya marcadas como relevantes?
6. Si OpenAlex está entre las fuentes y no hay clave (ver "Clave de OpenAlex",
   en "Herramienta Rust"), ¿se pide una gratuita? Hace falta el correo del
   usuario. Pregúntalo, no lo inventes ni lo saques de la configuración.

No descargues una obra protegida por paywall, préstamo controlado o
autenticación. Registra el impedimento y ofrece una fuente abierta alternativa
si existe. **No uses la cascada de `chatarrero` para evadir un control de
acceso**: un muro de pago, un login o un préstamo controlado son un no
definitivo, y ninguna escalada de la cascada los convierte en un sí.

Un bloqueo técnico sobre una obra abierta es otra cosa. Varias editoriales
responden `403` a cualquier cliente que no parezca un navegador, incluso sobre
artículos de lectura libre: eso es detección de bots, no control de acceso, y
ahí la cascada de `chatarrero` sí corresponde. La distinción se resuelve con
evidencia, no con intuición: **antes de escalar, comprueba que la obra esté
marcada como abierta** (`is_oa`, `oa_status`, licencia declarada en el catálogo)
o que el editor la ofrezca sin credenciales. Si lo está, sube de nivel y
registra qué hizo falta. Si no lo está, o si en algún punto aparece un pedido de
credenciales, para y marca el proveedor como bloqueado.

Escalar cuesta cortesía además de tokens: mantén las pausas entre peticiones y
un límite de intentos por proveedor aunque el nivel más caro funcione.

Un `blocked_challenge` se resuelve primero buscando la obra en otro host. Si no
hay espejo y la obra es abierta, ofrécele al usuario el captcha manual de
`chatarrero`: resuelve el challenge en una ventana visible y la descarga sigue
con esa sesión.

## Sub-bibliotecas

Un proyecto que trabaja temas distintos no los mezcla en una sola biblioteca:
cada tema es una sub-biblioteca autónoma en `docs/literatura/<tema>/`, con su
`catalogo.tsv`, su `manifest.tsv`, sus PDFs y su índice de `biblio-rata`.
`--dest` apunta siempre a la sub-biblioteca, nunca a `docs/literatura/`.

Se abre una nueva cuando el problema trae criterios de suficiencia propios y
quien consulta un tema no buscaría en el otro: el método para construir un
indicador compuesto y la literatura sobre codificación con agentes son dos
sub-bibliotecas aunque las use el mismo proyecto. Una ronda más sobre un tema
ya abierto va a la sub-biblioteca existente. Ante la duda, pregunta: separar
después obliga a partir a mano el catálogo y el manifiesto. El nombre es un
slug corto del tema (`construccion-indicadores`), no del proyecto ni de la
fecha.

**Copiar obras entre bibliotecas.** Una obra que sirve a dos temas se copia, no
se enlaza: cada sub-biblioteca tiene que poder viajar sola.

1. Filtra del `catalogo.tsv` de origen las filas que se copian y pásalas a
   `pulpo buscar --source <biblioteca-de-origen>`. `buscar` conserva la columna
   `identity` que traen, así que una obra sin DOI sigue casando con su fila del
   manifiesto; `--source` queda sólo en `provenance`. Las filas entran como
   `pendiente`: márcalas `relevante` con su motivo.
2. Copia el PDF y comprueba que su `sha256` coincida con el del manifiesto de
   origen.
3. Agrega su fila al `manifest.tsv` de destino como `accepted`, con ese
   `sha256`, el `path` relativo a la nueva biblioteca y `reason` "copiado desde
   <biblioteca-de-origen>".
4. Comprueba que cada fila `relevante` del catálogo tenga su `accepted` en el
   manifiesto: es lo que delata una copia que no casó.

**Consultar.** `biblio-rata` con `--corpus docs/literatura/<tema>` responde
sobre un tema; con `--corpus docs/literatura` indexa todas las sub-bibliotecas
juntas, porque recorre subdirectorios. En ese índice combinado, una obra
copiada en dos temas aparece dos veces.

## Flujo

1. Examina el proyecto y sus bibliotecas antes de elegir rutas, nombres o
   herramientas. La ronda entra en una sub-biblioteca existente o abre otra
   según "Sub-bibliotecas"; si el usuario propone una ruta, úsala y no inventes
   otra estructura.
2. Convierte el problema en consultas y criterios de suficiencia: qué regiones,
   nombres alternativos, periodos, idiomas y tipos de obra cuentan. Anota esos
   criterios para poder detectar documentación faltante. Si el problema pide
   literatura de frontera, fija un corte de fecha (24 meses suele bastar) y
   decide si la señal es novedad, tracción o las dos: eso define `--desde` y
   `--orden` en cada adaptador, y cuántas filas recientes sin citas se aceptan.
3. Consulta `references/fuentes.md` y ejecuta los scrapers Rust específicos de
   las fuentes pertinentes; los adaptadores de frontera se invocan como dice
   `references/scrapers.md`. OpenAlex sirve para descubrir metadatos y posibles
   ubicaciones abiertas; no es una fuente de descarga en este flujo. No vuelques
   al contexto respuestas completas de APIs ni PDFs.
4. Pasa cada salida de scraper a `scripts/pulpo.rs buscar`. El comando fusiona
   los metadatos en `catalogo.tsv`, junto a `manifest.tsv`, sin borrar filas ni
   decisiones previas. El agente orquestador inspecciona el catálogo y marca cada
   fila como `relevante` o `descartado`, siempre con un motivo; las nuevas quedan
   `pendiente`. El catálogo trae `published` y `cited_by`, que el scraper llena
   solo cuando la fuente informa el dato: una celda vacía significa que no se
   sabe, no que sea cero ni antigua. Para priorizar entre las `pendiente`,
   ordena por `published` y lee primero lo más reciente; usa `cited_by` para
   comparar obras de una misma época, nunca entre épocas, porque lo reciente
   tiene pocas citas por falta de tiempo y no de calidad. Ninguna de las dos
   decide la relevancia: la decisión sigue saliendo del criterio del paso 2, y
   `decision_reason` puede citarlas ("2025, 40 citas, estado del arte del
   método").
5. Si quedan filas `relevante` con DOI pero sin `pdf_url`, tiéndeles el puente
   antes de descargar: `scripts/openalex.rs resolver` pregunta por las copias
   abiertas de esos DOI y su salida vuelve al catálogo con `pulpo buscar`. Un DOI
   que no aparezca en la respuesta se informa como tal; no se le inventa una
   ubicación. El mismo puente trae `cited_by` para las filas que vinieron de
   una fuente sin citas (arXiv, bioRxiv, HAL, OSF, EconStor, LA Referencia),
   así que resolver también sirve para medir tracción, no solo para ubicar.
6. Solo después ejecuta `scripts/pulpo.rs descargar`: opera sobre filas
   `relevante`, prueba primero `pdf_url` y después `landing_url`, y no repite una
   fila ya `accepted` en el manifiesto. Usa un identificador de agente y límites
   de red, escribe primero en temporal, comprueba que el archivo sea un PDF real,
   conserva la URL que funcionó y produce una línea por obra.
7. No incorpores automáticamente un archivo que no sea PDF, esté vacío o no
   pueda abrirse con el extractor que usa `biblio-rata`. Déjalo con un estado de
   fallo o revisión en el manifiesto y registra la causa. Si la URL no salió de
   los metadatos —la armaste a mano o la dedujiste del patrón de nombres del
   sitio—, compara además el número de páginas (`pdfinfo`) con lo esperado antes
   de darla por buena: un PDF válido de otra obra, o sólo su prefacio, pasa la
   comprobación del extractor. Si no es la obra, su fila pasa a `rejected` (ver
   `references/contrato.md`).
8. Separa duplicado exacto de versión distinta. El mismo hash puede evitar una
   segunda copia; título parecido, DOI o URL por sí solos no autorizan borrar
   una edición, traducción o preprint.
9. Si una fuente termina pidiendo login para entregar el documento, marca ese
   proveedor como bloqueado y no sigas intentando con él. Conserva el DOI,
   título y autores como pistas para que otros scrapers busquen una copia
   abierta en otra fuente. Esta parada es propia de pulpo-librero: no convierte
   un diagnóstico de `chatarrero` en un veto global.
10. Consulta `biblio-rata` sobre el corpus existente y revisa si satisface los
   criterios del problema. Si faltan autores,
   periodos, perspectivas o tipos de fuente, formula otra ronda de búsquedas y
   repite solo los scrapers necesarios.
11. Solo después mueve los documentos aceptados al corpus definitivo y ejecuta la
   indexación incremental de `biblio-rata`.

## Herramienta Rust

Compila sin añadir dependencias al repositorio. Los binarios y los TSV
intermedios de la corrida van a un directorio propio, nunca a rutas fijas de
`/tmp`, que es compartido. Si otra persona crea antes una de esas rutas, la
compilación falla y el comando siguiente corre su archivo. Crea el directorio
al empezar con `mktemp -d` y anota la ruta que imprime, que en los comandos de
esta sección aparece como `<tmp>`. Escribe esa ruta literal en cada comando y
no una variable, porque una variable de shell puede no sobrevivir de un comando
al siguiente. El directorio se borra en "Cierre".

```sh
mktemp -d
rustc scripts/pulpo.rs -O -o <tmp>/pulpo-librero
```

Los scrapers entregan a `buscar` un TSV con encabezado. El comando exige
`--source`, conserva los metadatos conocidos y escribe o actualiza
`catalogo.tsv` en el directorio de la biblioteca:

```sh
<tmp>/pulpo-librero buscar --input <tmp>/openalex.tsv \
  --dest /ruta/biblioteca --source openalex
```

El descargador exige hosts autorizados y aplica límites por defecto ajustables:

```sh
<tmp>/pulpo-librero descargar --catalogo /ruta/biblioteca/catalogo.tsv \
  --dest /ruta/biblioteca \
  --allow-host arxiv.org --max-files 100 --max-bytes 104857600 \
  --max-total-bytes 1073741824 \
  --pausa-ms 4500 --pausa-mismo-host-ms 9000
```

Se presenta como un navegador corriente y descarga despacio, que es el
comportamiento por defecto y no hace falta pedirlo: User-Agent de Chrome,
`Accept` que prioriza el PDF, `Accept-Language: es-ES` y, al seguir una
redirección, el `Referer` de la URL de la que viene. La pausa por defecto es de
4,5 s entre peticiones y **9 s cuando dos seguidas van al mismo host**, porque
el costo de una corrida no lo paga quien la lanza sino el servidor que la
atiende. `--pausa-ms`, `--pausa-mismo-host-ms` y `--user-agent` permiten
subirlas; conviene bajarlas solo contra un host propio.

Presentarse como navegador **no es evasión y no cambia la doctrina de la
compuerta**: un paywall, un login o un préstamo controlado siguen siendo un no
definitivo, y un challenge anti-bot sigue siendo terminal. Es que varios
repositorios institucionales rechazan de plano al cliente que se anuncia como
script aunque la obra sea de lectura libre; el sondeo de ACM del 2026-09-09 dejó
medido que contra un challenge real esto tampoco alcanza.

Las redirecciones se siguen una a una solo si el host de cada destino también
está autorizado. No se permiten hosts locales o privados. Un archivo se acepta
solo después de pasar la comprobación de apertura del extractor disponible
(`pdftotext` o PyMuPDF), no solo por su cabecera `%PDF-`.

El primer scraper acoplado es OpenAlex. Compílalo para producir un catálogo de
descubrimiento, sin descargar obras:

```sh
rustc scripts/openalex.rs -O -o <tmp>/pulpo-openalex
<tmp>/pulpo-openalex search --query "palabra OR palabra2" --out <tmp>/candidatos.tsv --per-page 25
```

El TSV de OpenAlex incluye `is_oa`, `oa_status`, `license`, `version`,
`landing_url`, `pdf_url`, `pdf_urls` y `abstract`, pero no es una lista de archivos
aceptados. El orquestador debe inspeccionar sus ubicaciones abiertas, ampliar la
búsqueda con variantes del nombre cuando corresponda y decidir en el catálogo
qué candidatos pasan al descargador. El esquema completo está en
`references/contrato.md`.

Esas columnas salen de `open_access` y `best_oa_location`, no de la ubicación
publicada: una obra puede estar cerrada en la editorial y abierta en un
repositorio, y el catálogo registra la abierta. El scraper lee la respuesta con
`scripts/json_api.rs`, copia canónica del lector de JSON anidado; si hay que
tocarlo, se edita `src/json_api.rs` del repositorio y se propaga, porque `lint`
compara ambas copias byte a byte.

Para literatura de frontera, `search` acota por fecha, ordena y filtra por
tipo y por citas. `--desde` agrega como tope la fecha de hoy. Con `--orden
citas` o `fecha`, la consulta va contra título y resumen y no contra el índice
general, que con un orden distinto de relevancia trae obras de otras áreas; pon
la frase entre comillas. `--max` es sinónimo de `--per-page`:

```sh
<tmp>/pulpo-openalex search --query '"tema exacto"' --out <tmp>/frontera.tsv \
  --desde 2024-10-19 --orden citas --tipo "article|preprint|conference-paper" \
  --min-citas 6 --max 50
```

El TSV suma `published` y `cited_by`. `--orden fecha` da lo último publicado,
que todavía tiene pocas citas; para tracción usa `--orden citas`. En
computación pide los tres tipos, porque `conference-paper` es la literatura
principal y `article|preprint` lo excluye.

El mismo adaptador tiende el puente entre el catálogo y la descarga. `resolver`
toma las filas que ya tienen DOI pero todavía no tienen ubicación —vengan de
otro scraper, de una lista cerrada del usuario o de una corrida anterior— y
pregunta por sus copias abiertas:

```sh
<tmp>/pulpo-openalex resolver --catalogo /ruta/biblioteca/catalogo.tsv \
  --out <tmp>/resueltos.tsv --solo-relevante --max 200

<tmp>/pulpo-librero buscar --input <tmp>/resueltos.tsv \
  --dest /ruta/biblioteca --source openalex-doi
```

Agrupa hasta 100 DOI por consulta, así que resolver un catálogo entero cuesta
unas pocas peticiones. Con `--solo-relevante` se limita a las filas ya
aprobadas, que es lo que conviene cuando la revisión bibliográfica ya se hizo.
Los DOI que OpenAlex no conoce se informan por su nombre al terminar: esa fila
se queda sin ubicación y el orquestador decide si buscarla en otra fuente. La
fusión posterior conserva `decision` y `decision_reason`, de modo que resolver
nunca pisa el trabajo de revisión.

**Clave de OpenAlex.** OpenAlex cobra cada consulta en créditos diarios que se
renuevan a medianoche UTC. Una búsqueda cuesta 10 y una consulta de `resolver`
cuesta 1, aunque lleve 100 DOI. Sin clave el cupo es de 1000 créditos por IP de
salida, y en una red compartida puede estar agotado antes de empezar. Con una
clave gratuita sube a 10.000 y es solo de quien la pidió.

Al comenzar una corrida que use OpenAlex, averigua si hay clave sin mirarla. Si
`ajustes.json` declara `openalex_clave_archivo` y el archivo existe, pasa
`--clave-archivo` con su ruta a cada `search` y `resolver`. La ruta es relativa
a `vivencias/` salvo que empiece con `/`. Si no, comprueba la variable con
`test -n "${OPENALEX_API_KEY:-}" && echo hay`. Si está, el adaptador la toma
solo. Nunca abras, imprimas ni copies el archivo de la clave, ni pongas la clave
en una URL o en un argumento, que cualquiera ve con `ps`. El adaptador se la
manda a curl por stdin y rechaza un archivo que otros usuarios puedan leer.

Si no hay clave, ofrécela en la compuerta y pídela con el correo que dé el
usuario:

```sh
<tmp>/pulpo-openalex pedir-clave --email CORREO \
  --guardar <fuente>/vivencias/openalex.key
```

El adaptador guarda la clave con permisos 600 y no la muestra. Sirve cuando el
usuario abre el enlace que OpenAlex le manda por correo. Después agrega
`"openalex_clave_archivo": "openalex.key"` a `ajustes` y corre el validador. Si
OpenAlex responde `409 account_exists`, ese correo ya tiene cuenta. El usuario
copia la clave desde Settings > API key en openalex.org y la guarda él mismo en
ese archivo con permisos 600, sin pegarla en la conversación. Mientras no esté
activada, el adaptador avisa del `403` y sigue sin clave, y hace lo mismo con
una clave mal copiada (`401`).

La línea de resumen de `search` y `resolver` dice si la consulta fue con clave y
cuántos créditos quedan. Úsala para reservar cupo para `resolver`, que es el
paso que no tiene reemplazo. Agotado el cupo, el adaptador sale con 1 y dice
cuándo vuelve. No es una búsqueda sin resultados y no se reintenta antes.

Para buscar en arXiv, compila el adaptador oficial Atom y conserva su salida
como catálogo de candidatos. `--desde` acota por fecha de envío y ordena de lo
más nuevo a lo más viejo; con `--orden relevancia` se mantiene el rango y se
prioriza lo pertinente. arXiv no da citas, así que `cited_by` llega vacía. Un
`429` corta la corrida con exit 1, y no es una búsqueda sin resultados: el
límite es del frontal de la API, persiste minutos y la IP de salida puede ser
compartida, así que no reintentes en bucle.

```sh
rustc scripts/arxiv.rs -O -o <tmp>/pulpo-arxiv
<tmp>/pulpo-arxiv search --query "au:Apellido AND ti:tema" --out <tmp>/arxiv.tsv --max-results 25
<tmp>/pulpo-arxiv search --query "all:\"tema\"" --desde 2024-10-19 --out <tmp>/arxiv-reciente.tsv --max 50

# versión concreta, sin que la búsqueda pueda sustituirla por la actual
<tmp>/pulpo-arxiv search --id 1706.03762v1 --out <tmp>/arxiv-v1.tsv
```

Para corridas recurrentes, informa un contacto con `--mailto` y respeta la
pausa de 3 segundos que aplica el adaptador entre consultas. No subas a
Playwright o a un navegador anti-bot: el sondeo de arXiv confirmó que la API y
el PDF funcionan con HTTP simple.

El identificador `arxiv:NNNN.NNNNNvN` y la URL del PDF conservan la versión
encontrada. Si se desea la versión actual, quita el sufijo `vN` solo después de
decidirlo explícitamente.

Para libros, el adaptador de Internet Archive consulta el catálogo y después la
metadata de cada ítem, que es lo único que distingue una descarga abierta de un
préstamo controlado:

```sh
rustc scripts/internetarchive.rs -O -o <tmp>/pulpo-ia
<tmp>/pulpo-ia search --query "subject:\"linguistics\"" --out <tmp>/ia.tsv --rows 25

# ítems concretos, sin pasar por la búsqueda
<tmp>/pulpo-ia search --id nasa_techdoc_20040171231 --out <tmp>/ia.tsv
```

La consulta se restringe a `mediatype:texts` salvo que traigas el tuyo, porque
el catálogo mezcla libros con audio y video. Cada fila declara su acceso en
`oa_status`: `abierto` cuando hay un PDF con capa de texto, `prestamo-controlado`
cuando el ítem está en préstamo y `sin-pdf` cuando no hay archivo aprovechable.
Las dos últimas salen sin `pdf_url` a propósito: descártalas con ese motivo y no
insistas, porque un ítem en préstamo responde `401` aunque exponga un PDF de
aspecto perfectamente normal. Para las abiertas basta `--allow-host
archive.org`, que cubre también los nodos de descarga `dn*` e `ia*`. Sin DOI, la
identidad del catálogo queda `internetarchive:<identifier>` al pasar
`--source internetarchive`.

Los scrapers y el descargador usan `curl` del sistema para HTTP(S); no
implementan evasión, JavaScript ni autenticación. Si `curl` no está disponible,
informan el fallo sin instalar paquetes. Para lotes grandes, el orquestador
conserva solo el resumen de salida y los manifiestos, no el cuerpo de las
respuestas.

## Adaptadores de frontera

Once adaptadores más cubren lo que arXiv, OpenAlex e Internet Archive no
alcanzan: Semantic Scholar, Crossref, Hugging Face Papers, OSF Preprints, Europe
PMC, bioRxiv y medRxiv, SciELO, LA Referencia (con Redalyc), EconStor, HAL y
CORE. Todos siguen el mismo contrato (`--query`, `--out`, `--desde`, `--max`,
pausa de 3 s, TSV con `published` y `cited_by`) y se invocan como dice
`references/scrapers.md`, que trae además la tabla de qué señal de tracción y
qué PDF entrega cada uno. Lo medido sobre cada API, y lo que se sondeó y se
descartó (OpenReview, SSRN), está en `references/fuentes.md`.

Dos reglas valen para todos. La primera: una fuente sin citas no es una fuente
sin tracción, porque `openalex resolver` la trae después por DOI. La segunda:
varias de estas APIs no buscan por texto (bioRxiv, SciELO, EconStor) y el
adaptador filtra en el cliente, así que cada corrida tiene un presupuesto de
páginas o de obras revisadas que conviene acotar con `--desde` y las facetas
de la fuente antes de subir `--max`.

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

Lee `vivencias/ajustes.json` al comenzar una corrida y respeta sus preferencias:

- `max_files` (number): límite predeterminado de obras procesadas por corrida.
- `max_bytes_por_archivo` (number): tamaño máximo predeterminado de cada PDF.
- `max_total_bytes` (number): tamaño máximo predeterminado del lote aceptado.
- `pausa_ms` (number, opcional): pausa entre peticiones a hosts distintos.
- `pausa_mismo_host_ms` (number, opcional): pausa entre peticiones al mismo host.
- `user_agent` (string, opcional): con qué cliente se presenta el descargador.
- `openalex_clave_archivo` (string, opcional): ruta del archivo con la clave de
  OpenAlex, que va a `scripts/openalex.rs` como `--clave-archivo`. Es una ruta
  y nunca la clave (ver "Clave de OpenAlex").

Pasa esos valores a `scripts/pulpo.rs` mediante `--max-files`, `--max-bytes`,
`--max-total-bytes`, `--pausa-ms`, `--pausa-mismo-host-ms` y `--user-agent`. Las
tres últimas son opcionales: si faltan, el descargador ya trae los valores
corteses por defecto. Los ajustes solo pueden reducir los límites de seguridad
efectivos de una corrida; no autorizan hosts, evaden bloqueos ni habilitan
descargas restringidas.

Escribe en `vivencias/` según "Cierre".

`vivencias/ajustes.json` también declara `familia`, la relación de crianza con
sus dos skills madre (`chatarrero`, `biblio-rata`; ver "Skills madre" arriba),
con la fecha en que quedó registrada. El validador la exige porque esta skill
siempre depende de ambas.

El validador comprueba la forma del archivo, las claves declaradas y sus tipos;
no valida contenido bibliográfico.

```sh
(d=$(mktemp -d) && trap 'rm -rf "$d"' EXIT && rustc scripts/validar_ajustes.rs -O -o "$d/validar" && "$d/validar" <fuente>/vivencias/ajustes.json)
```

## Cierre

Informa cuántos candidatos fueron aceptados, rechazados, repetidos o fallaron,
con sus identificadores y causas, y qué criterios del problema siguen sin
cubrirse. Conserva procedencia, fecha de incorporación, URL, tamaño y hash
cuando estén disponibles. No afirmes que una obra quedó incorporada hasta que
el PDF haya pasado la validación y la indexación.

Cerrado el informe, mira si la corrida contradijo lo que esta skill documenta:
una fuente que `references/fuentes.md` daba por dócil y hoy bloquea, un
identificador que resolvió a otra obra sin fallar, un formato que se disfrazó
de lo que no era, un estado del manifiesto que `references/contrato.md` describe
de otro modo. Si es así, abre `vivencias/registro/<fecha>-<tema>.md` con el
hallazgo y agrega su línea a `vivencias/INDICE.md` (`fecha | tema | resultado en
pocas palabras | archivo`).

Una corrida que salió como estaba previsto no se registra, por grande que haya
sido: qué se bajó ya lo guardan el catálogo y el manifiesto.

Al terminar, borra el directorio de la corrida con `rm -rf <tmp>`. Lo que valía
conservar ya está en el catálogo y el manifiesto de la biblioteca.
