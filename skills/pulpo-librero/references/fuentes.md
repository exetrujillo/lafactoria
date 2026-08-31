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

## Internet Archive

Úsalo para localizar libros y otros documentos mediante su catálogo y para
descargar solo archivos que la interfaz exponga como disponibles. Un registro
puede contener varios formatos y restricciones distintas: elige un PDF abierto
cuando exista y no confundas préstamo controlado con descarga abierta. Conserva
el identificador del ítem y la URL de descarga como procedencia.

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
