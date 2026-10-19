# Fuentes abiertas

## arXiv

Usa la API oficial para buscar metadatos y obtener el enlace PDF de cada entrada.
La respuesta es Atom. No vuelques el XML completo al contexto. La documentación
de referencia es `https://info.arxiv.org/help/api/basics.html`. Distingue el
identificador de la obra de su versión (`v1`, `v2`, etc.). `--id` pide una
versión exacta mediante `id_list`, y un identificador legado como
`hep-ph/0207126v1` se pasa entero, no solo su último segmento.

El adaptador `scripts/arxiv.rs` emite `id`, `title`, `doi`, `year`, `version`,
`landing_url`, `pdf_url`, `pdf_urls`, `abstract`, `published` (de `<published>`,
la fecha del primer envío) y `cited_by`. Esta última va siempre vacía porque
arXiv no expone citas, así que la tracción hay que pedírsela a OpenAlex con el
DOI o el título. `--desde YYYY-MM-DD` agrega `AND submittedDate:[...]` a la
consulta, con hoy como cota superior, y ordena por fecha de envío descendente
salvo que se pida `--orden relevancia`.

El sondeo del 2026-08-29 confirmó que esta API es el nivel 0 y que el PDF baja
con HTTP simple. El del 2026-10-19 no pudo medir nada de la API. Unas 19
peticiones en dos rondas dieron `429` con cuerpo `Rate exceeded.` desde el
frontal Google/Fastly, sin `Retry-After`, y el bloqueo siguió después de 5
minutos de espera. En ese mismo rato `arxiv.org/pdf/1706.03762v1.pdf` respondió
`200 application/pdf`. El límite es del host de la API, no de los PDF. Ante un
`429` el scraper termina con exit 1 y lo dice, en vez de entregar un TSV vacío.
No reintentes en bucle. Espera bastante o difiere la búsqueda, porque la IP de
salida puede ser compartida. Mantén al menos 3 s entre consultas y limita
`--max-results`.

## OpenAlex: descubrimiento, no descarga

Usa su API únicamente para descubrir obras, DOI, autores, versiones y posibles
ubicaciones de acceso abierto. En este diseño no se descarga el documento desde
OpenAlex. El scraper conserva sus identificadores y entrega al orquestador las
ubicaciones candidatas para que otro adaptador o el descargador las verifique.
Una ubicación registrada no garantiza que el archivo sea descargable. La
documentación de referencia es
`https://docs.openalex.org/how-to-use-the-api/api-overview`.

El sondeo del 2026-09-01 confirmó que esta API es el nivel 0, con `200` para
un `User-Agent` identificable, sin cookies ni navegador. Hoy cobra cada consulta
en créditos diarios, y sin clave el cupo no alcanza para una corrida (ver "Cupo
y clave", más abajo). Tres cosas que sí cambian cómo se la lee:

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
  (`filter=doi:a|b|c`), hasta 100. Verificado con 100 DOI en una sola consulta
  el 2026-10-19. Un DOI puede devolver dos obras (fueron 101 para 100 DOI), así
  que `per-page` va en 200 y no en el tamaño de la tanda, o la obra que sobra
  deja fuera a otro DOI. Es lo que hace barato resolver las ubicaciones de un
  catálogo entero.

`abstract_inverted_index` sigue presente, así que el abstract se reconstruye
ordenando las posiciones de cada palabra. La respuesta ahora trae también
`meta.cost_usd` y `meta.x_query`, que no se usan.

### Frontera: fecha, orden, tipo y citas

El sondeo del 2026-10-19 midió los filtros que hacen falta para pedir
literatura reciente y con tracción, y `search` los expone como `--desde`,
`--orden citas|fecha|relevancia`, `--tipo` y `--min-citas`. Cada fila trae
ahora `published` (`publication_date`) y `cited_by` (`cited_by_count`).

- **Con `search=`, cualquier `sort` reemplaza a la relevancia.** Ordenado por
  citas, un tema amplio devolvía las obras más citadas que mencionan alguna de
  las palabras: YOLOv9 o MEGA12 para "large language models". Por eso, con
  `--orden citas` o `fecha`, el scraper pasa a `filter=title_and_abstract.search`.
  Igual entra algo de ruido. Con la consulta entre comillas baja.
- **El orden por fecha necesita tope.** Sin `to_publication_date`, encabezan
  datasets fechados en 2027 con 0 citas. El scraper agrega la fecha de hoy
  siempre que hay `--desde` u `--orden fecha`. Ese orden da novedad, no
  tracción. Las obras de 2026-09 y 2026-10 tienen entre 6 y 47 citas.
- **`type:article|preprint` excluye `conference-paper`**, que en computación
  es la literatura principal. En IA conviene pedir los tres.
- **`select=` reduce la respuesta** (285 KB a 75 KB con 10 obras y abstract) y
  devuelve las mismas columnas. El scraper lo usa siempre.
- Un `--tipo` que OpenAlex no conoce da `200` con cero resultados, sin error.
- `cited_by` es el conteo de OpenAlex y tiene errores puntuales. Un paper de
  ECOOP 2025 figura con 15 249 citas. Sirve para ordenar, no para citar.
- `openalex_id` sale ahora como `W...`, sin la URL, así que la identidad de una
  obra sin DOI queda `openalex:W...` como dice el contrato. Una fila vieja que
  tenga la URL en el id no se fusiona con la nueva.

### Cupo y clave

Medido el 2026-10-19. OpenAlex mide el uso en créditos diarios que se renuevan a
medianoche UTC, y `x-ratelimit-reset` da los segundos que faltan. Una búsqueda
cuesta 10 créditos y una consulta por filtro de DOI cuesta 1, lleve 100 DOI o
uno. Sin clave el cupo es de 1000 créditos por IP de salida. Una corrida lo
agotó tras unas 67 búsquedas, y si la IP de salida es compartida puede estar
gastado antes de empezar. Una clave gratuita da 10.000 créditos a la
persona que la pidió.

- La clave va como `Authorization: Bearer`, que según la documentación equivale
  a `api_key=` en la URL. El adaptador usa la cabecera y la pasa por
  `curl --config -`, para que no quede en la línea de comandos.
- `x-ratelimit-remaining` y `x-ratelimit-limit` vienen en créditos (`9998` de
  `10000`). Hay otras en dólares (`x-ratelimit-remaining-usd`) que el adaptador
  no usa.
- Una clave inexistente da `401` con un JSON `{"error", "message"}`, y una sin
  activar da `403 key_not_activated`. En los dos casos el adaptador avisa y
  repite la consulta sin clave.
- La clave se pide con `POST https://api.openalex.org/api-keys` y
  `{"email": ...}`. Responde `201` con `api_key` y `status:
  pending_activation`, y la clave sirve cuando se abre el enlace del correo,
  que vence en 7 días. Un correo mal formado da `400` (medido), uno con cuenta
  `409 account_exists` y uno con una clave pendiente `409 activation_pending`.
  Ese `POST` no se reintenta, porque repetido puede crear dos claves.
- Según la documentación, sin clave solo sirve la primera página. La clave
  habilita el paginado por cursor, que el adaptador no usa porque `search` trae
  hasta 200 obras por llamada.

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
y no exige correo, a cambio de algo de frescura. Si alguna vez la
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

## Semantic Scholar

El sondeo del 2026-10-19 midió solo `/paper/search/bulk` de la Graph API, que responde JSON sin clave ni correo. El endpoint de relevancia `/paper/search` devolvió 429 en todas sus peticiones, así que el scraper no ordena por relevancia. El texto de bulk filtra de forma laxa y no rankea. Una consulta por "diffusion models" ordenada por citas trajo "Heat and mass transfer", así que la revisión del catálogo no se puede saltar.

Bulk filtra por fecha con `publicationDateOrYear=desde:`, ordena por `citationCount:desc` o `publicationDate:desc` y acepta `minCitationCount` y `fieldsOfStudy`. Cada página trae hasta 1000 obras (unos 2,7 MB con abstract) y un `token` de continuación. La paginación con token no se pudo medir. Entrega `citationCount` por obra, abstract completo, DOI en `externalIds` (los preprints de arXiv traen el DOI 10.48550/arXiv.*) y `url` a la página de la obra en semanticscholar.org. No mide votos ni descargas.

Algunas obras traen solo `year`, sin `publicationDate` (63 de 1000 en el sondeo y 10 de 1000 en la verificación), y el rango de fecha las deja pasar por el año. El scraper descarta las que no puede ubicar después de `--desde`.

`openAccessPdf.url` viene vacía en más de la mitad de las obras, también en preprints de arXiv que declaran `isOpenAccess=false`. Cuando hay `externalIds.ArXiv`, `pdf_urls` agrega primero `https://arxiv.org/pdf/<id>`, el único host que dio PDF en el sondeo, y `pdf_url` queda con lo que declara la fuente. Las URL de `doi.org` y de editoriales pueden devolver una landing HTML (Nature lo hizo). Un PDF de `link.springer.com` sí bajó con HTTP simple. Autoriza `arxiv.org` en el descargador y deja que la comprobación de `%PDF-` decida el resto.

Sin clave el cupo es compartido y se agota sin aviso. El 429 no trae `Retry-After` ni cabeceras de límite, y en el sondeo se encadenó durante varios minutos. El scraper informa el 429 aparte y sale con 1. No lo relances en seguida: espera varios minutos o usa otra fuente. `fieldsOfStudy` usa la lista de campos de Semantic Scholar (Computer Science, Psychology, Sociology, Economics, Medicine, Biology, Mathematics y otros). No hay un campo Neuroscience: para neurociencias usa Psychology, Medicine o Biology.

## Crossref

Usa la API REST (`api.crossref.org/works`) para descubrir obras recientes con
DOI y ordenarlas por citas. El sondeo del 2026-10-19 la midió como nivel 0:
JSON con HTTP simple, sin clave, sin correo y sin navegador. Sin `mailto` cae en
el pool público, que anuncia 1 petición por segundo y concurrencia 1. Con pausas
de 3 s no hubo `429`. El adaptador `scripts/crossref.rs` entrega `published`
(de `date-parts`, a veces solo año o mes) y `cited_by` (`is-referenced-by-count`).

- **Crossref no declara acceso abierto, solo licencias.** `has-license:true`
  incluye licencias de copyright y de TDM de la editorial, como la de ACM. El
  filtro `license.url` compara la cadena exacta: con CC BY 4.0, una sola
  variante dio 41243 obras y las cuatro variantes juntas (http y https, con y
  sin barra final) dieron 136996. Por eso el adaptador manda las cuatro de cada
  licencia CC y CC0, que se combinan con OR. Solo una licencia Creative Commons
  marca `is_oa=true`. Cualquier otra queda como `desconocido`.
- **El cursor no admite el orden por fecha.** `sort=published` con `cursor`
  responde `400` (`sort-criteria-incompatible-with-cursor`). El adaptador pagina
  ese orden por `offset` y descarta las obras que se repiten entre páginas.
- **`link[]` ofrece la URL de la editorial, no una copia verificada.** En una
  muestra de 120 obras CC, 100 traían PDF. Nature y Springer bajaron como
  `%PDF-` tras el paso de cookies del mismo host. MDPI respondió `403`. Wiley
  respondió un challenge de Cloudflare: es terminal, hay que buscar otra copia.
  Elsevier solo trae `text/xml` de su API.
- `gte-citation` figura entre los filtros válidos, pero no filtra. La tracción
  se pide con `sort=is-referenced-by-count`.

El abstract viene en JATS y falta en cerca de un tercio de las obras.

## Hugging Face Papers

El sondeo del 2026-10-19 midió la API pública de Hugging Face Papers como fuente de literatura reciente de IA y ML. Es nivel 0. Responde JSON con HTTP simple, sin credencial, sin correo y sin challenge. El adaptador es `scripts/huggingface-papers.rs`.

Tiene tres entradas:
- `--lista tendencia` usa `daily_papers?sort=trending`.
- `--lista reciente` usa `daily_papers?sort=publishedAt`.
- `--query` usa `papers/search?q=`.

`limit` llega hasta 100 y con más responde `400`. `daily_papers` pagina con `p`, y la primera página va sin ese parámetro. La búsqueda ignora `p` y devuelve siempre las mismas obras, así que da como mucho 100 resultados por consulta. Su orden no sigue la fecha ni los votos.

Los límites van en buckets separados. Lectura admite 500 pedidos por 300 s y búsqueda 50 por 300 s.

Ninguna entrada filtra por fecha. `month=` fue aceptado pero no filtró, y tanto la búsqueda como la tendencia traen obras de 2019 o 2023. Por eso `--desde` se aplica en el cliente sobre `paper.publishedAt`, que es la fecha de arXiv. El `publishedAt` de afuera es la fecha de envío a Daily Papers y no sirve para esto.

La única señal de tracción es `paper.upvotes`, un voto social de una sola plataforma. El TSV la lleva en una columna extra, `upvotes`, y `--orden votos` ordena por ella. No hay citas, así que `cited_by` queda vacío. Tampoco hay DOI ni licencia, y `is_oa`, `oa_status` y `license` quedan vacíos.

Hugging Face no sirve el archivo. `paper.id` es el identificador de arXiv, sin versión, y el PDF se arma como `https://arxiv.org/pdf/<id>`. Respondió `200` con `%PDF-` y sin cambio de host, así que alcanza `--allow-host arxiv.org`. Si un id no tiene forma de arXiv, la celda queda vacía.

Cuidados:
- La identidad del catálogo queda `huggingface-papers:<id>` y no se fusiona sola con `arxiv:<id>vN`. Antes de descargar conviene reconciliar las dos filas.
- La cobertura se limita a lo que alguien subió a Daily Papers o indexó Hugging Face.
- Algunos PDF de arXiv pasan de 20 MB.

## OSF Preprints (PsyArXiv, SocArXiv, MetaArXiv)

El sondeo del 2026-10-19 midió la API JSON:API de OSF (`api.osf.io/v2/preprints/`).
Responde HTTP 200 sin credenciales ni JavaScript y no anuncia límites de tasa en
las cabeceras. El adaptador `scripts/osf-preprints.rs` consulta un proveedor a la
vez (por defecto psyarxiv, socarxiv y metaarxiv), porque un `filter[provider]`
con varios valores separados por coma devuelve cero obras.

La única búsqueda por texto que funciona es `filter[title]`. Busca la frase
completa dentro del título, sin distinguir mayúsculas. `q=` se ignora en silencio
y no hay búsqueda en el abstract, así que conviene probar variantes cortas del
término. `filter[date_published][gte]` y `sort=-date_published` funcionan, y el
adaptador siempre ordena del más nuevo al más viejo.

Cada versión de un preprint es una obra aparte en la lista (`fwjpq_v1` y
`fwjpq_v2`). `is_latest_version` no se puede filtrar en la API, así que el
adaptador descarta las versiones viejas y las retiradas después de recibirlas.
El DOI sale de `links.preprint_doi` (`10.31234/osf.io/<id>_vN` en PsyArXiv,
`10.31235/...` en SocArXiv) y resuelve en doi.org. `attributes.doi`, que es el
del artículo publicado después, casi siempre viene en null.

No hay señal de tracción. La lista no trae citas ni descargas, `sort=-downloads`
se ignora y `cited_by` queda vacío. Las descargas están solo en
`/v2/files/<id>/`, con una petición por obra. `embed=primary_file` y
`meta[metrics]` devolvieron 502.

La licencia se pide aparte, con una petición por licencia distinta, que se
guarda en memoria durante la corrida. Las CC salen como `abierto` y "No license"
como `desconocido`. El `pdf_url` es `https://osf.io/download/<id del archivo
primario>/`. Redirige a `files.osf.io` y de ahí a un bucket firmado de
`*.compat.objectstorage.us-ashburn-1.oraclecloud.com`, que vence en una hora.
Por eso `descargar` necesita `--allow-host osf.io --allow-host oraclecloud.com`,
y conviene descargar poco después de buscar. Se bajó un PDF completo (%PDF-,
5808554 bytes). No se midió si hay archivos primarios que no sean PDF. Esos los
rechaza la comprobación del descargador.

## Europe PMC: descubrimiento biomédico, no descarga

El adaptador `scripts/europepmc.rs` consulta `/search` de la API REST
(`www.ebi.ac.uk/europepmc/webservices/rest`) con `resultType=core`. Entrega DOI,
PMID/PMCID dentro del `id`, licencia, `firstPublicationDate` como `published` y
`citedByCount` como `cited_by`. El sondeo del 2026-10-19 confirmó que es nivel 0:
`200` sin clave, correo ni cookies. No se midió límite de tasa y no hubo `429` con
una petición cada 3 segundos. La página máxima es de 1000 obras y la paginación va
por `cursorMark`. Un `pageSize` fuera de rango vuelve como HTTP 200 con `errCode`
en el cuerpo.

- **Un término sin campo busca también en el texto completo.** Con
  `(neuroscience OR "brain")` el primer resultado fue "Cancer statistics, 2025".
  Por eso el adaptador envuelve en `TITLE_ABS:(...)` toda consulta sin `:`. Aun así
  `neural` atrapa redes neuronales artificiales y `cortex` otros tejidos.
- **Los preprints vienen con `isOpenAccess=N` aunque tengan licencia CC.** Con
  `OPEN_ACCESS:y` desaparecen todos. Por eso `--src PPR` no aplica ese filtro y
  sus filas salen `oa_status=desconocido`, `version=preprint` y solo con DOI.
- **No entrega PDF.** `europepmc.org/articles/<pmcid>?pdf=render` y
  `ptpmcrender.fcgi` devolvieron `403` con `cf-mitigated: challenge` en tres obras
  abiertas. Un enlace PDF ajeno que la fuente lista (MDPI) dio `403` con
  `pulpo-librero/0.1`. El adaptador pone esos enlaces ajenos antes que los
  propios en `pdf_urls`. El texto completo en JATS sí sale por
  `/<pmcid>/fullTextXML` con `200`, pero no es PDF.
- Para conseguir el archivo, la salida es pasar los DOI por `openalex resolver`.
  `sort=CITED desc` ordena por citas, y en estadística biomédica ese orden lo
  dominan los estudios de carga de enfermedad, no la metodología.

## bioRxiv y medRxiv

El sondeo del 2026-10-19 contra api.biorxiv.org y api.medrxiv.org midió una API JSON sin clave, cookie ni límite de tasa anunciado en las cabeceras. Las dos comparten el mismo formato y lo lee el mismo adaptador, `scripts/biorxiv.rs`, con `--servidor`.

La API no busca por texto. El parámetro `query` se ignora y la respuesta sale idéntica a la que se obtiene sin él. Solo lista por rango de fechas (`/details/<servidor>/<desde>/<hasta>/<cursor>`), con `category` como único filtro efectivo (en minúsculas, con espacio o guion bajo). Entrega 30 registros por página, en orden de fecha ascendente y sin parámetro de orden. Por eso el adaptador lee el `total` en la primera página, recorre el cursor hacia atrás para empezar por lo más reciente y filtra localmente por título y abstract. `total` cuenta también las revisiones: cada versión nueva es otro registro, y el adaptador se queda con la actual de cada DOI.

Entrega DOI (prefijo `10.1101/` hasta 2025 y `10.64898/` en 2026), versión, fecha de esa versión, licencia, categoría, abstract y el campo `published`, que trae el DOI de la revista cuando la obra ya se publicó y `NA` cuando no. No entrega citas, votos ni descargas, así que `cited_by` sale vacío. Para saber qué obras tienen tracción hay que cruzar el DOI con OpenAlex, o quedarse con las que llegaron a revista (`--solo-publicadas`), que es una señal de calidad y no un conteo.

La API no da la URL del PDF. Se arma como `https://www.<servidor>.org/content/<doi>v<versión>.full.pdf`, y así llegaron como application/pdf real, desde el mismo host y sin challenge, un PDF cc_by y uno cc_no de bioRxiv y uno cc_by de medRxiv. Se autoriza con `--allow-host www.biorxiv.org` o `www.medrxiv.org`.

`cc_no` es la licencia por defecto: permite leer gratis, pero no da licencia de reutilización. El adaptador ofrece el PDF de esas obras y no las marca abiertas (`is_oa` queda vacío y `oa_status` dice `desconocido`). La decisión es del orquestador.

Cuidados: un rango de 24 meses en neurociencia ocupa unas 926 páginas, así que conviene acotar el rango con `--desde`, poner `--categoria` y fijar un tope con `--max-paginas`. El resumen de la corrida dice hasta qué fecha llegó la revisión. El adaptador deja 3 s entre páginas. No se midió cómo se comporta Cloudflare con descargas en volumen.

## SciELO

El sondeo del 2026-10-19 midió SciELO por ArticleMeta (`articlemeta.scielo.org/api/v1`), que es el nivel 0: JSON por HTTP simple, sin clave, correo ni cookies, y sin cabeceras de límite de tasa. La búsqueda por texto de `search.scielo.org` responde `403` con un challenge de BunnyCDN. Es terminal y no se insiste.

ArticleMeta no busca por texto. `scripts/scielo.rs` lista los códigos de cada colección con `identifiers` (filtros `from`, `until` e `issn`, hasta 1000 por página), los recorre desde el más reciente, pide cada registro con `article/?code=` y filtra en el cliente: la fecha de publicación contra `--desde` y los términos de `--query` (todos, sin tildes ni mayúsculas) contra título, resumen y palabras clave en todos los idiomas. Por eso cada obra revisada cuesta una petición y unos 7 s. Con `--query "salud"` en `chl` hicieron falta 50 revisiones para 10 filas.

- **`from` filtra por fecha de procesamiento, no de publicación.** Con processing de 2005 salieron artículos de 1987. El scraper descarta lo publicado antes de `--desde`; una fecha parcial (`2024-10`) pasa si su mes contiene la fecha pedida.
- **`id` lleva la colección** (`scielo:chl:S...`), porque el mismo código existe en más de una (`scl` y `spa`).
- **El DOI sale de la lista `identifiers`**, no del registro, y no siempre está.
- **`license` es la de la revista** (v541 de `title`), no la del artículo. Con licencia declarada la fila sale `is_oa=true` y `oa_status=abierto`.
- **`cited_by` viene de Citedby** (`citedby.scielo.org/api/v1/pid/`) y cuenta solo las citas desde otros documentos de SciELO. Un artículo de 1987 dio 4 y los de 2026, 0.

El PDF está en el sitio de cada colección (`www.scielo.cl`, `www.scielo.org.mx`, ...), no en ArticleMeta. Llega como `http://` y redirige con 302 a `https://` en el mismo host, así que basta autorizar ese host en el descargador. Medido con 200, `application/pdf` y `%PDF-` en `www.scielo.cl`.

La colección `mex` no tiene nada procesado después de octubre de 2025, así que no sirve para lo más reciente. Las colecciones se piden por código y el scraper no elige ninguna por defecto.

## Redalyc y LA Referencia

LA Referencia agrega los repositorios y revistas de las redes nacionales de
Iberoamérica, Redalyc incluido. El adaptador es
`scripts/redalyc-lareferencia.rs` y consulta `buscador.lareferencia.info/api/v1/search`
(VuFind API v1). El sondeo del 2026-10-19 confirmó que es el nivel 0: JSON con
HTTP simple, sin clave, correo ni login. El host viejo
`www.lareferencia.info/vufind/api/v1/` redirige con 301 al nuevo.

- **La fecha es solo el año.** `publishDate` trae `"2026"` y nada más, así que
  `--desde 2024-10-19` se traduce en `publishDate:[2025 TO *]`: un `2024` no
  prueba que la obra sea posterior al corte, y el scraper solo deja pasar la
  fecha cuyo primer día posible ya cumple `--desde`.
- **No hay tracción.** Ningún campo de citas, votos ni descargas, y `cited_by`
  sale siempre vacío. El orden por defecto es `relevance`: con `sort=year` las
  100 primeras de 2026 salieron todas de Chile y Ecuador, aunque España y
  Brasil tenían más obras en el rango.
- **No hospeda PDF.** `rawData.bitstream_url_str_mv` apunta al bitstream de un
  DSpace en 13 de 100 registros (25 de 130 en la red CL). Probados dos, UC y
  Mackenzie, bajaron PDF real; el de Mackenzie redirige a otro subdominio
  (`adelpha-api.mackenzie.br`), así que conviene autorizar el dominio base.
- **El DOI es escaso y sucio.** `identifier_str_mv` mezcla DOI, ISSN, números de
  inventario y citas de tesis, con algún DOI mal formado (`00.51915/...`). Solo
  cuenta lo que empieza con `10.<registrante>/`; cerca de 1 de cada 6 lo trae.
- **Redalyc no sirve sola.** Su búsqueda es AngularJS, su OAI-PMH y su ruta
  `/pdf/<id>.pdf` dieron 404. Sus revistas entran por LA Referencia con
  `--repositorio Redalyc-UAEMEX` (o `-IPN`, `-UNAM`), y salen solo con landing.

`limit` tope 100 (1000 da 400) y pagina con `page`. Sin DOI, la identidad queda
`redalyc-lareferencia:<id>` con el id interno del agregador (`CL_<hash>`).

## EconStor (RePEc)

EconStor es el repositorio abierto de economía de la ZBW (DSpace 6.4). El adaptador es `scripts/repec-econstor.rs` y recolecta por OAI-PMH, porque no hay búsqueda por texto que se pueda usar: `/discover` y `/simple-search` están vedados en `robots.txt`, `open-search` responde `400` y la API de DSpace 7 (`/server/api`) responde `404`. RePEc/IDEAS queda fuera: `api.repec.org` exige clave (responde `error 2` con una falsa) y `htsearch` está bajo `/cgi-bin/`, prohibido en `robots.txt`.

El sondeo del 2026-10-19 confirmó que es el nivel 0 (`200` sin credencial ni navegador) y midió lo que define cómo se la lee:

- **`from` filtra por fecha de modificación, no de publicación.** La ventana desde 2024-10-19 tiene 49073 registros y las páginas no vienen ordenadas por fecha. El adaptador vuelve a filtrar en el cliente.
- **`dim` y no `oai_dc`.** `oai_dc` trae solo el año. `dim` trae además `dc.date.accessioned` con día, el DOI en `dc.identifier.pi` y la licencia real en `dc.rights.license` (CC BY en 10419/344156), separada del `Nutzungsbedingungen` genérico. `published` es el día de depósito cuando cae en el mismo año que `dc.date.issued`, y si no, solo el año.
- **La consulta se resuelve en el cliente** sobre título, resumen, palabras clave y JEL, así que cada página de 100 registros cuesta unos 20 s y una consulta específica puede no encontrar nada en 10 páginas. Con `inflation` salieron 10 obras en 1000 registros. Acota con `--desde` y `--set`.
- **Sin señal de tracción.** `cited_by` sale siempre vacío.
- **El PDF está en el mismo host.** `/rest/handle/<hdl>?expand=bitstreams` lista los archivos; se toma solo el bundle `ORIGINAL` con `application/pdf` (`TEXT`, `THUMBNAIL` y `SWORD` son derivados). El `retrieve` entregó `%PDF-` sin redirección, pero a unos 16 KB/s: uno de 2 MB no terminó en 60 s.

Basta `--allow-host www.econstor.eu`. Sin DOI, la identidad queda `repec-econstor:<handle>`.

## HAL (archives-ouvertes)

El sondeo del 2026-10-19 midió la API de búsqueda de HAL en `https://api.archives-ouvertes.fr/search/`. Es Solr, responde JSON con HTTP simple y no pide clave, correo ni login. No aparecieron cabeceras de límite de tasa.

Entrega búsqueda por texto en `q` (sintaxis Solr), filtros `fq` por tipo (`docType_s`), dominio (`domain_s`, con `0.shs`, `0.math` y `0.stat`), acceso abierto (`openAccess_bool`) y fecha. El filtro de fecha va sobre `producedDate_tdate`, que es una fecha. `producedDate_s` es texto y compara como tal. `published` sale de `producedDate_s` y puede tener precisión de día, mes o año. Hay obras fechadas en el futuro, así que `hal.rs` acota con `TO NOW`.

No entrega señal de tracción: la lista completa de campos de una obra no trae citas, descargas ni votos, y `cited_by` sale siempre vacío. Para ordenar por tracción hay que pasar los DOI por `openalex resolver`. Tampoco hay orden por citas.

Para descubrir literatura de frontera conviene incluir `UNDEFINED` (preprints y documentos de trabajo) además de `ART`. Muchas obras de estadística recientes salen como `UNDEFINED`, y con solo `ART` quedan fuera.

`openAccess_bool:true` no garantiza archivo. Los registros `submitType_s:notice` son abiertos solo por un enlace externo (`linkExtUrl_s`, casi siempre doi.org). En el sondeo, 41 de 106 artículos abiertos venían así. `--solo-pdf` filtra `submitType_s:file`. El PDF es `fileMain_s` (`https://<portal>.hal.science/<halId>/document`), que en el sondeo devolvió `application/pdf` con `%PDF-` sin redirección. El 2026-10-19 `hal.science` ya respondía con el challenge de Anubis (ver Zora, más abajo): `descargar` lo marca `blocked_challenge` y la copia hay que buscarla en otro host. `files_s` puede traer anexos y va como alternativa. Los hosts son `hal.science` y subdominios de portal (`inria.hal.science`, `shs.hal.science`, `amu.hal.science`), distintos del host de la API, así que hay que autorizarlos en `descargar`.

Cuidados. Una consulta mal formada o un campo de orden inexistente vuelve con status 200 y el cuerpo `{"error":{"msg":...}}`, así que el scraper exige la clave `response`. `cursorMark` exige un desempate único en el orden (`docid`) y sin él da ese mismo error. `rows` tiene un tope silencioso de 10000. `halId_s` es el identificador estable y no coincide con `docid`.

## CORE

El sondeo del 2026-10-19 midió la API v3 de CORE (`api.core.ac.uk/v3/search/works/`)
sin clave. Responde JSON con `totalHits`, `offset` y `results`, y pagina con
`limit` y `offset`. La ruta sin barra final da 301 hacia la de barra final. Sirve
para descubrir obras de repositorios institucionales de cualquier campo y
entrega título, abstract, DOI (en 8 de 10 filas de una corrida), `publishedDate`
y `yearPublished`.

El filtro de fecha solo funciona por año dentro de `q` (`yearPublished>=2025`,
`yearPublished<=2026`). Un rango `[A TO B]` o una comparación sobre
`publishedDate` responden 500, así que `core.rs` corta por día en el cliente.
`sort=publishedDate:desc` funciona, pero sin cota superior lo encabezan obras
fechadas en 2028 y 2029. `sort=citationCount:desc` responde 500, y
`citationCount` valió 0 en todas las obras vistas, así que CORE no sirve como
señal de tracción. El scraper deja `cited_by` vacío en vez de escribir ese cero,
que en el catálogo pisaría el conteo de otra fuente. Muchas fechas son
`AAAA-01-01` o solo el año, y hay duplicados con distinto id y mismo DOI o mismo
archivo.

No entrega PDF descargable. `downloadUrl` apunta a `core.ac.uk/download/`, que
respondió 403 con `cf-mitigated: challenge`, y la única alternativa de
`sourceFulltextUrls` en otro host (chemrxiv.org) tuvo el mismo challenge. El
camino es tomar los DOI y pasarlos por `openalex resolver`. La API es lenta, con
50 s por página de 10 obras y timeouts con páginas de 25, y la cabecera
`x-ratelimit-limit: 10` vino acompañada de un 429 con pausas de 3 s.

## Fuentes sondeadas y descartadas

Sondeadas el 2026-10-19 con el mismo criterio que las demás. Quedan acá para que
una ronda futura no vuelva a medirlas desde cero.

### OpenReview: sin PDF ni tracción

La búsqueda (`api2.openreview.net/notes/search`) responde `200` en JSON sin
clave, correo ni cookie, cubre ICLR, NeurIPS, ICML y ACL más entradas importadas
de dblp, acepta texto y filtro por `group` (`ICLR.cc/2026/Conference`) y ordena
por `cdate:desc`. El orden por `pdate` da `400`, y `mintcdate` y `maxtcdate` se
ignoran, así que el filtro por fecha tendría que hacerse en el cliente. No hay
señal de tracción: las reseñas no se pueden leer porque `/notes` responde con
challenge. Límite de 20 peticiones por 60 s. El PDF de `openreview.net` y de
`api2.openreview.net` da `403` con challenge, sin `%PDF-`. Las entradas dblp
traen una URL de arXiv en `content.pdf`, sin medir. Para esas conferencias,
arXiv, Semantic Scholar y Hugging Face Papers cubren lo mismo con PDF.

### SSRN: todo el dominio tras un challenge

`papers.ssrn.com`, `api.ssrn.com` y `www.ssrn.com` responden un challenge de
Cloudflare con JavaScript (`403`, `cf-mitigated: challenge`), incluidas la
búsqueda, la ficha por `abstract_id` y `Delivery.cfm`. `robots.txt` se sirve sin
challenge y declara `Crawl-delay` de 5 s. Los metadatos de las obras con prefijo
DOI `10.2139` salen por Crossref (título, fecha, `is-referenced-by-count`), sin
PDF. Se descarta como fuente directa; por Crossref sirve solo como índice de
DOI, y mezcla economía con aprendizaje automático.

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
misma obra suele estar en un repositorio institucional. Si no hay espejo, la vía
es el captcha manual de `chatarrero`, con el usuario resolviendo el challenge.

### El estado de un proveedor caduca: revisa antes de confiar en la ronda pasada

Onomázein (`10.7764/onomazein.*`) entregó sin fricción en las rondas del
2026-09-08: tres obras suyas entraron a una biblioteca local, bajadas de
`ojs.uc.cl` con HTTP simple. El **2026-09-10 el mismo DOI resuelve a
`revistadelaconstruccion.uc.cl`** —el OJS multirrevista de la UC— y devuelve
`403` con una página de 304 KB titulada "Verificación de Seguridad — Pontificia
Universidad Católica de Chile".

La lección no es sobre esta revista sino sobre el catálogo: **que una fuente
haya entregado antes no prueba que entregue hoy**, y un cambio de dominio del
editor puede mover una fuente entera de la columna fácil a la bloqueada sin
aviso. Cuando una obra falla en un proveedor que la bitácora daba por dócil,
verifica a dónde resuelve el DOI ahora antes de dar por rota la obra.

### Zora (`zora.uzh.ch`) — challenge de Anubis

El repositorio de la Universidad de Zúrich responde `200` con
`<title>Making sure you're not a bot!</title>` y hojas de estilo de
`within.website/x/xess`: es **Anubis**, una prueba de trabajo, no Cloudflare.
Las cabeceras que mira `is_bot_challenge` (`cf-mitigated`, `server-timing: chlray`)
no lo delatan; el descargador lo registra como `blocked_challenge` (ver la
sección siguiente). Desde 2.17.1 lo reconoce por la ruta de sus scripts
(`within.website/x/cmd/anubis/`) y no solo por el `<title>`, porque Anubis traduce
el título según `Accept-Language`: a `descargar`, que pide español, HAL le
respondió "¡Asegurándonos de que no eres un robot!".

Dos consecuencias prácticas: el PDF directo por `id/eprint/NNNN/1/archivo.pdf`
tampoco pasa (devolvió `500`), y conviene buscar la obra en otro repositorio.
Medido: *Accent mark and visual word recognition in Spanish* está bloqueada en
Zora y se obtuvo entera en `access.archive-ouverte.unige.ch` (562 KB).

### Challenges que sólo se ven en el título

Varios challenges llegan con `200 text/html` y sin ninguna cabecera de
Cloudflare, así que por las cabeceras parecen una landing más. Medidos sobre
obras declaradas abiertas, en dos rondas distintas:

- `link.springer.com` (y SpringerOpen, que redirige ahí):
  `<title>Client Challenge</title>` y "Please enable JavaScript";
- `pmc.ncbi.nlm.nih.gov/.../pdf/`: `<title>Preparing to download ...`, una
  prueba en JavaScript;
- Zora: `<title>Making sure you're not a bot!</title>` (Anubis, arriba). En
  otro idioma el título cambia, y a Anubis lo delata la ruta de sus scripts.

`pulpo descargar` compara el `<title>` del cuerpo con esos títulos y con el
"Just a moment..." de Cloudflare, y los marca `blocked_challenge`: terminal,
la salida es otro host. Lo que no ve es un `403` con "Just a moment..." y sin
`cf-mitigated`, porque con `curl --fail` el cuerpo de un `403` no se guarda.
Ese caso sigue cayendo en `http_error`, y si es la única pista, mirar el
cuerpo con un `curl` a mano lo aclara.

### La salida: repositorios institucionales

Para una obra con DOI bloqueada en la editorial, el camino barato es la copia de
repositorio. Medido en el mismo sondeo: `10.1145/3411764.3445483` se obtuvo en
`aaltodoc.aalto.fi` (DSpace) con un simple `curl -sL`, sin cookies ni Referer —
2.080.349 bytes, `%PDF-` verificado, texto extraíble.

Dos cuidados:

- OpenAlex y Unpaywall suelen traer la landing del repositorio pero con
  `url_for_pdf` vacío. Hay que raspar la landing y extraer el enlace: en DSpace
  6 o anterior es un `href` con `/bitstreams/.../download` (DSpace 7 no lo
  trae, ver abajo); en Pure, `/files/NNNN/*.pdf`; en
  **OJS** —que es casi toda la revista universitaria hispana— la landing es
  `/article/view/N` y el PDF cuelga de `/article/download/N/M`, con un segundo
  identificador que no se puede adivinar y hay que leer del HTML.
- **Ojo con el síntoma.** Una landing HTML entregada por un host autorizado no
  falla: baja con `200` y muere recién en la comprobación de PDF. En una ronda
  de descarga real fueron cinco de seis fallas. Desde `1.22.0` ese caso se
  registra como `not_a_pdf` y no como `unreadable_pdf`, así que el manifiesto ya
  distingue "me dieron la landing" —raspa el enlace, la obra está bien— de "el
  PDF está roto", que sí manda a buscar copia en otro lado.
- **Repositorio no implica abierto.** En el mismo sondeo, `aaltodoc.aalto.fi`
  (DSpace) entregó sin fricción, mientras que `research.aalto.fi` (Pure), de la
  *misma universidad*, está también tras Cloudflare y devolvió el mismo "Just a
  moment...". El host se evalúa caso a caso.

### DSpace 7: el PDF sale de la API REST

En DSpace 7 la landing es una SPA de Angular: el HTML servido no trae
`citation_pdf_url` ni un `href` a `/bitstreams/`, así que rasparla no da nada y
el descargador la registra como `not_a_pdf`. El PDF sale de la API REST, sin
credenciales:

1. `GET /server/api/pid/find?id=hdl:<handle>` resuelve el handle al ítem y da su
   `uuid`. Si la landing ya es `/items/<uuid>`, este paso sobra.
2. `GET /server/api/core/items/<uuid>/bundles?embed=bitstreams`: en el bundle
   `ORIGINAL`, cada bitstream trae `_links.content.href`.
3. Esa URL (`.../core/bitstreams/<uuid>/content`) va como `pdf_url` de la fila
   del catálogo y `descargar` la baja como cualquier otra.

Medido en una ronda sobre tres repositorios institucionales, con nueve obras
resueltas así, y vuelto a ver en otra ronda sobre dos repositorios más. Un `401`
del bitstream por esta misma vía pide credencial: es un no definitivo, no un
bloqueo técnico.
