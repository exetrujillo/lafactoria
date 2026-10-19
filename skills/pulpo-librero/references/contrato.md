# Contrato de incorporación

## `catalogo.tsv`

`catalogo.tsv` vive en el directorio de la biblioteca, junto a
`manifest.tsv`. `pulpo buscar` lo crea y lo actualiza de forma acumulativa; no
usa `/tmp` como almacenamiento final. Tiene una fila por obra descubierta y
estas columnas, en este orden:

```text
identity<TAB>id<TAB>title<TAB>doi<TAB>year<TAB>is_oa<TAB>oa_status<TAB>license<TAB>version<TAB>landing_url<TAB>pdf_url<TAB>pdf_urls<TAB>abstract<TAB>published<TAB>cited_by<TAB>provenance<TAB>discovered_at<TAB>decision<TAB>decision_reason
```

`decision` solo puede ser `pendiente`, `relevante` o `descartado`. Las filas
nuevas entran como `pendiente`; el agente orquestador cambia la decisión según
el criterio bibliográfico y escribe el motivo en `decision_reason`. El scraper
no decide relevancia. `provenance` acumula valores `fuente@timestamp` separados
por punto y coma, y `discovered_at` es el timestamp Unix de la primera
incorporación de esa fila.

`published` es la fecha de publicación en `YYYY-MM-DD`, `YYYY-MM` o `YYYY`,
con la precisión que declare la fuente. `cited_by` es el conteo de citas como
entero sin separadores, tal como lo informa la fuente en el momento de la
consulta. Ambas pueden ir vacías: el scraper las emite con esos mismos nombres
solo si la fuente trae el dato y no las deduce de `year` ni de otra fuente. Un
conteo de citas depende de quién lo cuenta y de cuándo, así que sirve para
ordenar dentro del catálogo, no como dato bibliográfico citable.

La identidad se resuelve así:

- Si hay DOI, se normaliza a minúsculas, quitando `https://doi.org/`,
  `http://doi.org/` o `doi:`, y se usa `doi:<valor>`. Por eso un trabajo con
  DOI visto en OpenAlex y arXiv comparte fila y acumula ambas procedencias.
- Si no hay DOI, se usa `fuente:id` (`openalex:W...`, `arxiv:...`). Si el
  scraper ya trae el `id` con ese prefijo, `buscar` no lo repite. Dos fuentes
  sin DOI no se fusionan automáticamente: el título, la URL o la semejanza
  textual no son una prueba suficiente. El agente puede reconciliar el caso
  explícitamente antes de marcar la fila.

Una nueva observación actualiza los metadatos no vacíos de su identidad,
incluidos `published` y `cited_by`, pero conserva `decision` y
`decision_reason`. Un valor vacío nunca borra uno ya registrado, así que el
conteo de citas que queda es el de la última fuente que lo trajo. `pdf_urls` no
se reemplaza sino que se une: primero las URLs de la observación nueva y
después las anteriores que no estén. Así una fuente que trae otra copia no
borra las ubicaciones que trajo otra. El catálogo permite revisar y decidir sin
descargar nada.

Un `catalogo.tsv` anterior, de 17 columnas sin `published` ni `cited_by`, se
sigue leyendo: `buscar` le agrega en memoria esas dos columnas vacías y la
siguiente escritura lo deja en el formato de 19. `descargar` lee el catálogo
por nombre de columna y acepta ambos. Un encabezado que no sea ninguno de los
dos es un error. Una fila con menos celdas que el encabezado, como la que
pierde los tabuladores finales al editarse a mano, se completa con celdas
vacías.

## `manifest.tsv`

El archivo `manifest.tsv` tiene una fila por candidato y, como mínimo, estas
columnas:

```text
id<TAB>title<TAB>source_url<TAB>path<TAB>status<TAB>reason<TAB>bytes<TAB>sha256
```

Estados:

- `accepted`: descarga que pasó la comprobación de PDF y quedó en el destino.
  `source_url` es la URL que lo entregó y `reason` dice por qué camino: "PDF
  verificado", y a continuación "vía citation_pdf_url", "vía API de DSpace 7",
  "vía el conjunto abierto de PMC", "desde Wayback <fecha>" o "con el UA de
  curl" cuando hizo falta ese nivel de
  la cascada (ver "Herramienta Rust" en el SKILL.md).
- `duplicate`: el archivo ya estaba presente con el mismo hash; una colisión de
  nombre con contenido distinto conserva la nueva versión usando un sufijo en
  `path`.
- `no_pdf_location`: no había una ubicación HTTP candidata.
- `host_no_autorizado`: había ubicaciones, pero ninguna en un host autorizado,
  así que no se hizo ningún pedido. `reason` nombra los hosts. Ni esta fila ni
  la de `no_pdf_location` cuentan contra `--max-files`.
- `not_a_pdf`: la fuente respondió con una landing HTML que no trae
  `citation_pdf_url` ni es de DSpace 7, o con otro cuerpo que no es PDF.
- `unreadable_pdf`: el cuerpo parecía PDF, pero el extractor no pudo abrirlo.
- `limit_exceeded`: no se incorporó por alcanzar un límite de archivos o bytes.
- `blocked_challenge`: el proveedor respondió con un challenge anti-bot: las
  cabeceras de Cloudflare `cf-mitigated: challenge` o `server-timing: chlray`,
  un cuerpo HTML cuyo `<title>` es el de una página de challenge conocida
  ("Client Challenge", "Just a moment...") o un cuerpo que carga los scripts de
  Anubis, en cualquier idioma. Un challenge que no es de Cloudflare se repite
  una vez con el UA de curl antes de quedar así; el de Cloudflare no, porque no
  cede con ningún UA. Es distinto de `http_error` y no se reintenta por HTTP: la
  salida es otra copia (paso "Otra copia" del SKILL.md), no insistir.
- `http_error`: todas las ubicaciones fallaron en transporte o status HTTP.

En los estados de fallo, `reason` termina con `sin autorizar: <hosts>` cuando
alguna ubicación, alguna redirección o la copia de Wayback quedó sin probar
por estar en un host no autorizado. Es la lista que el orquestador revisa antes
de la siguiente corrida. Si la API de Wayback respondió `429`, `reason` lo dice
antes de esa lista, y la obra se vuelve a intentar en una corrida posterior.
- `failed`: fallo local al mover, registrar o calcular SHA-256.
- `rejected`: lo escribe el orquestador, nunca el binario. El archivo pasó la
  comprobación de PDF pero no es la obra (otra obra, un fragmento, muchas menos
  páginas de las esperadas) o no pasó la indexación de `biblio-rata`. `reason`
  dice por qué, y el archivo sale de la biblioteca para que el índice no lo
  tome por la obra. `descargar` sólo omite las identidades `accepted`, así que
  una fila `rejected` se vuelve a intentar: corrige antes su ubicación en el
  catálogo o márcala `descartado`, o la próxima corrida baja lo mismo.

El manifiesto es la fuente de trazabilidad, no el índice de `biblio-rata`.
`path` identifica el archivo local; `source_url` identifica su procedencia.

`path` es **siempre relativo al directorio que contiene el `manifest.tsv`**, que
es el de la biblioteca. Un archivo en la raíz queda como `nombre.pdf` a secas y
uno en un subdirectorio conserva su subruta. Nunca se escribe una ruta absoluta
ni un prefijo `./`: el manifiesto y los PDFs viajan juntos, así que una ruta
anclada al disco de quien descargó se rompe apenas la biblioteca cambia de
máquina, y de paso publica el `$HOME` de esa persona en un archivo que suele
terminar versionado. La forma la fija `ruta_en_biblioteca`, no el valor que se
le haya pasado a `--dest`: el mismo archivo produce la misma fila se haya
invocado el descargador con una ruta relativa o con una absoluta.

Cuando el agente disponga de DOI, ISBN, versión o licencia, debe conservarlos en
las columnas de entrada o en un registro bibliográfico paralelo, sin inventar
valores ausentes.

Un `accepted` todavía debe pasar la indexación incremental de `biblio-rata`. Si la extracción
de texto falla, cambia el estado a `rejected`; no lo presentes como documento
citable.

El resultado de un scraper de descubrimiento es distinto del manifiesto de
incorporación. En particular, `openalex.tsv` solo contiene candidatos
bibliográficos y no afirma que exista una copia descargable. El orquestador lo
incorpora con `pulpo buscar`, revisa el catálogo y luego llama a `pulpo
descargar` solo para las filas `relevante`. `descargar` omite cualquier
identidad que ya tenga una fila `accepted` en el manifiesto.
