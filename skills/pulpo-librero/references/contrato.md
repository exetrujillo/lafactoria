# Contrato de incorporación

## `catalogo.tsv`

`catalogo.tsv` vive en el directorio de la biblioteca, junto a
`manifest.tsv`. `pulpo buscar` lo crea y lo actualiza de forma acumulativa; no
usa `/tmp` como almacenamiento final. Tiene una fila por obra descubierta y
estas columnas, en este orden:

```text
identity<TAB>id<TAB>title<TAB>doi<TAB>year<TAB>is_oa<TAB>oa_status<TAB>license<TAB>version<TAB>landing_url<TAB>pdf_url<TAB>pdf_urls<TAB>abstract<TAB>provenance<TAB>discovered_at<TAB>decision<TAB>decision_reason
```

`decision` solo puede ser `pendiente`, `relevante` o `descartado`. Las filas
nuevas entran como `pendiente`; el agente orquestador cambia la decisión según
el criterio bibliográfico y escribe el motivo en `decision_reason`. El scraper
no decide relevancia. `provenance` acumula valores `fuente@timestamp` separados
por punto y coma, y `discovered_at` es el timestamp Unix de la primera
incorporación de esa fila.

La identidad se resuelve así:

- Si hay DOI, se normaliza a minúsculas, quitando `https://doi.org/`,
  `http://doi.org/` o `doi:`, y se usa `doi:<valor>`. Por eso un trabajo con
  DOI visto en OpenAlex y arXiv comparte fila y acumula ambas procedencias.
- Si no hay DOI, se usa `fuente:id` (`openalex:W...`, `arxiv:...`). Dos fuentes
  sin DOI no se fusionan automáticamente: el título, la URL o la semejanza
  textual no son una prueba suficiente. El agente puede reconciliar el caso
  explícitamente antes de marcar la fila.

Una nueva observación actualiza los metadatos no vacíos de su identidad, pero
conserva `decision` y `decision_reason`. El catálogo permite revisar y decidir
sin descargar nada.

## `manifest.tsv`

El archivo `manifest.tsv` tiene una fila por candidato y, como mínimo, estas
columnas:

```text
id<TAB>title<TAB>source_url<TAB>path<TAB>status<TAB>reason<TAB>bytes<TAB>sha256
```

Estados:

- `accepted`: descarga que pasó la comprobación de PDF y quedó en el destino.
- `duplicate`: el archivo ya estaba presente con el mismo hash; una colisión de
  nombre con contenido distinto conserva la nueva versión usando un sufijo en
  `path`.
- `no_pdf_location`: no había una ubicación HTTP candidata.
- `not_a_pdf`: la fuente respondió, pero el cuerpo no era un PDF.
- `unreadable_pdf`: el cuerpo parecía PDF, pero el extractor no pudo abrirlo.
- `limit_exceeded`: no se incorporó por alcanzar un límite de archivos o bytes.
- `http_error`: todas las ubicaciones fallaron en transporte o status HTTP.
- `failed`: fallo local al mover, registrar o calcular SHA-256.

El manifiesto es la fuente de trazabilidad, no el índice de `biblio-rata`.
`path` identifica el archivo local; `source_url` identifica su procedencia.
Cuando el agente disponga de DOI, ISBN, versión o licencia, debe conservarlos en
las columnas de entrada o en un registro bibliográfico paralelo, sin inventar
valores ausentes.

Un `accepted` todavía debe pasar la indexación incremental de `biblio-rata`. Si la extracción
de texto falla, cambia el estado a rechazado o a una categoría de revisión
explícita; no lo presentes como documento citable.

El resultado de un scraper de descubrimiento es distinto del manifiesto de
incorporación. En particular, `openalex.tsv` solo contiene candidatos
bibliográficos y no afirma que exista una copia descargable. El orquestador lo
incorpora con `pulpo buscar`, revisa el catálogo y luego llama a `pulpo
descargar` solo para las filas `relevante`. `descargar` omite cualquier
identidad que ya tenga una fila `accepted` en el manifiesto.
