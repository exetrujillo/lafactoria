# Fuentes abiertas iniciales

## arXiv

Usa la API oficial para buscar metadatos y obtener el enlace PDF de cada entrada.
La respuesta es Atom; no descargues el XML completo al contexto. La documentación
de referencia es `https://info.arxiv.org/help/api/basics.html`. Respeta sus
términos de uso y límites. Distingue el identificador de la obra de la versión
(`v1`, `v2`, etc.).
El adaptador `scripts/arxiv.rs` emite candidatos con `id`, `title`, `doi`,
`version`, `landing_url` y `pdf_url`; esa salida todavía requiere pasar por el
descargador y su comprobación de PDF.
Usa `--id` para solicitar una versión exacta mediante `id_list`; no sustituyas
un identificador legado como `hep-ph/0207126v1` por solo su último segmento.
El sondeo del 2026-08-29 confirmó que esta API es el nivel 0 y que el PDF funciona
con HTTP simple: `200`, Atom no vacío, un `User-Agent` identificable y `Accept`
específico bastaron; no hicieron falta cookies, Referer ni navegador. Mantén
pausas de al menos 3 segundos entre consultas API y limita `max-results`.

## OpenAlex: descubrimiento, no descarga

Usa su API únicamente para descubrir obras, DOI, autores, versiones y posibles
ubicaciones de acceso abierto. En este diseño no se descarga el documento desde
OpenAlex. El scraper conserva sus identificadores y entrega al orquestador las
ubicaciones candidatas para que otro adaptador o el descargador las verifique.
Una ubicación registrada no garantiza que el archivo sea descargable. La
documentación de referencia es
`https://docs.openalex.org/how-to-use-the-api/api-overview`.

El sondeo del 2026-09-01 confirmó que esta API es el nivel 0 y no necesita
correo ni credencial: `200` con `User-Agent` identificable, sin cookies ni
navegador. Tres cosas que sí cambian cómo se la lee:

- **Los campos de acceso abierto viven en `open_access` y `best_oa_location`,
  no sueltos en la obra.** Buscarlos por nombre sobre la respuesta entera
  devuelve los de `primary_location`, que es la versión publicada y suele estar
  cerrada aunque exista una copia abierta en un repositorio. Medido con
  `W4210318887` (`doi:10.1075/scl`), que es `green` con copia en HAL: leído mal
  daba `is_oa=false`, sin licencia y con la URL de pago como `landing_url`. Por
  eso el scraper usa `scripts/json_api.rs`, que exige indicar el ámbito.
- **`content_urls.pdf` no es una ruta de descarga.** El campo apareció apuntando
  a `content.openalex.org/works/<id>.pdf`, pero pedirlo devuelve `401` sin
  credencial. La regla de esta sección sigue en pie: OpenAlex descubre, no
  entrega archivos.
- **El filtro acepta varios DOI por pedido** separándolos con `|`
  (`filter=doi:a|b|c`), hasta 50. Verificado con tres DOI en una sola consulta.
  Es lo que hace barato resolver las ubicaciones de un catálogo entero.

`abstract_inverted_index` sigue presente, así que el abstract se reconstruye
ordenando las posiciones de cada palabra. La respuesta ahora trae también
`meta.cost_usd` y `meta.x_query`, que no se usan.

## El puente DOI → PDF

Descubrir una obra y encontrarle una copia abierta son dos preguntas distintas.
Un DOI puede llegar desde otro scraper, desde una bibliografía que trajo el
usuario o desde una corrida vieja, sin ninguna ubicación asociada. Ese salto lo
cubre `openalex resolver`, que consulta `works` filtrando por DOI en tandas.

La fuente natural para esta pregunta sería Unpaywall, y se descartó por una
razón medida: el 2026-09-01 su API devolvió `422` con el mensaje `"Please use
your own email address in API calls"`. El parámetro `email` es obligatorio en
toda llamada y rechaza los dominios de relleno, así que cada corrida exigiría un
correo real de quien la ejecuta. OpenAlex ingiere los mismos datos de Unpaywall
y responde sin credencial, a cambio de algo de frescura. Si alguna vez la
diferencia de actualización importa para un corpus concreto, la salida se puede
volver a resolver contra Unpaywall sin cambiar el contrato del catálogo: lo que
vuelve es un TSV con las mismas columnas.

## Internet Archive

Úsalo para localizar libros y otros documentos mediante su catálogo. El
adaptador es `scripts/internetarchive.rs` y su trabajo real no es buscar sino
distinguir descarga abierta de préstamo controlado, porque el catálogo expone
las dos cosas con la misma apariencia.

El sondeo del 2026-09-01 confirmó que esta API es el nivel 0 —`200` con HTTP
simple, sin cookies, credencial ni navegador— y midió cuatro cosas que definen
cómo se la lee:

- **La restricción vive en la metadata del ítem, no en su lista de archivos.**
  `2005guidetoliter00kath` tiene `access-restricted-item: "true"` y su
  `collection` incluye `inlibrary` y `printdisabled`; expone igual un `Text PDF`
  de 48 MB que parece descargable, y pedirlo devuelve **401** tras redirigir a
  un nodo `dn*.eu.archive.org`. Por eso el scraper pide `/metadata/<id>` por
  ítem en vez de confiar en la búsqueda, y por eso nunca intenta la descarga de
  un ítem restringido.
- **El camino feliz existe y es limpio.** `nasa_techdoc_20040171231`, sin
  `access-restricted-item`, entrega su `Text PDF` con **206
  `application/pdf`** desde un nodo `ia*.us.archive.org`.
- **Una licencia abierta no prueba que haya PDF.**
  `5c6fe374-b9c1-44a2-8444-4f92174a3a74` es CC-BY y sus únicos archivos son de
  formato `Metadata`.
- **`collection` viene como string cuando el ítem pertenece a una sola y como
  arreglo cuando pertenece a varias**, verificado en los dos ítems de arriba.
  Para eso está `json_texts_or_text` en `scripts/json_api.rs`.

De los formatos PDF del ítem se aceptan sólo dos, en este orden: `Text PDF` y
`Additional Text PDF`. El resto se descarta por motivos distintos y ninguno es
recuperable. `ACS Encrypted PDF` y `LCP Encrypted PDF` están cifrados aunque
figuren como PDF. `JPEG-Compressed PDF` e `Image Container PDF` son el escaneo
en imagen: pasarían la comprobación de PDF del descargador y quedarían
indexados vacíos, que es peor que no tenerlos. La diferencia se mide sola —en
`dli.ernet.286194`, el `Image Container PDF` pesa 335 MB y el
`Additional Text PDF` del mismo libro pesa 29 MB y sí entrega texto.

La búsqueda fuerza `AND mediatype:texts` salvo que la consulta ya traiga un
`mediatype` propio: sin ese filtro el catálogo devuelve audio y video mezclados
con los libros. Los nodos de descarga (`dn*.ca.archive.org`,
`ia*.us.archive.org`) no necesitan autorización aparte: `--allow-host
archive.org` los cubre porque el descargador acepta el sufijo `.archive.org`.

Conserva el identificador del ítem como procedencia; sin DOI, la identidad del
catálogo queda `internetarchive:<identifier>`.

## Regla de fuentes

Estas fuentes son un punto de partida, no una lista de permisos universales.
Antes de una corrida nueva, verifica que la fuente y el recurso concreto sean
abiertos y que el uso local sea compatible con lo que pidió el usuario. Un
paywall, login, préstamo, challenge o respuesta que no sea el documento es un
estado terminal para ese candidato.

## Papel de los scrapers

Cada fuente debe tener un scraper Rust acoplado a su interfaz, con salida
pequeña y estable para el orquestador: identificador, título, procedencia,
identificadores bibliográficos y URLs candidatas. El scraper no decide qué
constituye una biblioteca suficiente ni descarga indiscriminadamente todo lo que
encuentra. Esa decisión pertenece a `pulpo-librero`.

## Proveedores que exigen navegador

Algunas editoriales sirven artículos de **lectura libre** detrás de un challenge
anti-bot. La obra es abierta, pero el fetch no lo es: son dos cosas distintas y
conviene no confundirlas al decidir si insistir.

### ACM Digital Library (`dl.acm.org`) — bloqueado, no insistir

Sondeo del 2026-09-09, 17 peticiones con 3 s de pausa, sobre dos DOI con
`is_oa=true` (uno con licencia `cc-by-sa` declarada). **Ningún nivel de `curl`
funciona.** Fallaron con `403` y un cuerpo HTML de ~5,6 KB:

1. el UA de pulpo con `Accept: */*`;
2. UA identificable con correo de contacto;
3. lo anterior más `Referer` y `-L`;
4. cookie jar tomado desde la landing `doi/10.1145/...` — **la landing también
   responde 403 y no emite ninguna cookie**, así que el jar queda vacío;
5. UA de Chrome 131 completo con `Accept-Language`, `Sec-Fetch-*` y `sec-ch-ua`,
   probado en los dos DOI.

El WAF es un **Cloudflare managed challenge**: `cf-mitigated: challenge`,
`server-timing: chlray`, `cf-ray`, cuerpo `<title>Just a moment...</title>`. Es
un challenge con JavaScript; ningún cliente sin motor JS lo pasa.

**Contraejemplo explícito** de la nota de la bitácora de `chatarrero` que dice
"403 sin UA de navegador → basta requests con UA de navegador". Acá no basta, y
la diferencia se ve en la cabecera `cf-mitigated`, no en el status.

El descargador detecta este caso y lo marca `blocked_challenge`, que es
terminal. **No subas a Playwright por esto**: el costo no se justifica cuando la
misma obra suele estar en un repositorio institucional.

### El estado de un proveedor caduca: revisá antes de confiar en la ronda pasada

Onomázein (`10.7764/onomazein.*`) entregó sin fricción en las rondas del
2026-09-08: tres obras suyas están en la biblioteca de `espalol`, bajadas de
`ojs.uc.cl` con HTTP simple. El **2026-09-10 el mismo DOI resuelve a
`revistadelaconstruccion.uc.cl`** —el OJS multirrevista de la UC— y devuelve
`403` con una página de 304 KB titulada "Verificación de Seguridad — Pontificia
Universidad Católica de Chile".

La lección no es sobre esta revista sino sobre el catálogo: **que una fuente
haya entregado antes no prueba que entregue hoy**, y un cambio de dominio del
editor puede mover una fuente entera de la columna fácil a la bloqueada sin
aviso. Cuando una obra falla en un proveedor que la bitácora daba por dócil,
verificá a dónde resuelve el DOI ahora antes de dar por rota la obra.

### Zora (`zora.uzh.ch`) — challenge de Anubis

El repositorio de la Universidad de Zúrich responde `200` con
`<title>Making sure you're not a bot!</title>` y hojas de estilo de
`within.website/x/xess`: es **Anubis**, una prueba de trabajo, no Cloudflare.
Las señales que mira `is_bot_challenge` (`cf-mitigated`, `server-timing: chlray`)
**no lo detectan**, así que el descargador lo registra como `http_error` o
`unreadable_pdf` según qué haya llegado, y no como `blocked_challenge`.

Dos consecuencias prácticas: el PDF directo por `id/eprint/NNNN/1/archivo.pdf`
tampoco pasa (devolvió `500`), y conviene buscar la obra en otro repositorio.
Medido: *Accent mark and visual word recognition in Spanish* está bloqueada en
Zora y se obtuvo entera en `access.archive-ouverte.unige.ch` (562 KB).

### La salida: repositorios institucionales

Para una obra con DOI bloqueada en la editorial, el camino barato es la copia de
repositorio. Medido en el mismo sondeo: `10.1145/3411764.3445483` se obtuvo en
`aaltodoc.aalto.fi` (DSpace) con un simple `curl -sL`, sin cookies ni Referer —
2.080.349 bytes, `%PDF-` verificado, texto extraíble.

Dos cuidados:

- OpenAlex y Unpaywall suelen traer la landing del repositorio pero con
  `url_for_pdf` vacío. Hay que raspar la landing y extraer el enlace: en DSpace
  es un `href` con `/bitstreams/.../download`; en Pure, `/files/NNNN/*.pdf`; en
  **OJS** —que es casi toda la revista universitaria hispana— la landing es
  `/article/view/N` y el PDF cuelga de `/article/download/N/M`, con un segundo
  identificador que no se puede adivinar y hay que leer del HTML.
- **Ojo con el síntoma.** Una landing HTML entregada por un host autorizado no
  falla: baja con `200` y muere recién en la comprobación de PDF. En la tercera
  ronda de `espalol` fueron cinco de seis fallas. Desde `1.22.0` ese caso se
  registra como `not_a_pdf` y no como `unreadable_pdf`, así que el manifiesto ya
  distingue "me dieron la landing" —raspá el enlace, la obra está bien— de "el
  PDF está roto", que sí manda a buscar copia en otro lado.
- **Repositorio no implica abierto.** En el mismo sondeo, `aaltodoc.aalto.fi`
  (DSpace) entregó sin fricción, mientras que `research.aalto.fi` (Pure), de la
  *misma universidad*, está también tras Cloudflare y devolvió el mismo "Just a
  moment...". El host se evalúa caso a caso.
