---
name: pulpo-librero
description: Usa esta skill cuando el usuario quiera reunir o descargar papers y libros desde fuentes abiertas para incorporarlos a una biblioteca local compatible con biblio-rata.
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
2. ¿En qué directorio vive la biblioteca y qué límite de cantidad, tamaño o
   tiempo tiene la corrida?
3. ¿Qué fuentes abiertas se autorizan? Si no se especifica, propone arXiv,
   OpenAlex e Internet Archive según se trate de papers o libros.
4. ¿Se quieren obras nuevas, versiones concretas o una lista cerrada de URLs?
5. Si ya existe `catalogo.tsv`, ¿se deben revisar sus pendientes, incorporar una
   nueva fuente o descargar solo las obras ya marcadas como relevantes?

No descargues una obra protegida por paywall, préstamo controlado,
autenticación o challenge. Registra el impedimento y ofrece una fuente abierta
alternativa si existe. No uses la cascada de `chatarrero` para evadir un control
de acceso.

## Flujo

1. Examina el proyecto y la biblioteca existente antes de elegir rutas, nombres
   o herramientas. Si el usuario propone `docs/literatura/<directorio>`, úsala como biblioteca del proyecto y no inventes otra estructura.
2. Convierte el problema en consultas y criterios de suficiencia: qué regiones,
   nombres alternativos, periodos, idiomas y tipos de obra cuentan. Anota esos
   criterios para poder detectar documentación faltante.
3. Consulta `references/fuentes.md` y ejecuta los scrapers Rust específicos de
   las fuentes pertinentes. OpenAlex sirve para descubrir metadatos y posibles
   ubicaciones abiertas; no es una fuente de descarga en este flujo. No vuelques
   al contexto respuestas completas de APIs ni PDFs.
4. Pasa cada salida de scraper a `scripts/pulpo.rs buscar`. El comando fusiona
   los metadatos en `catalogo.tsv`, junto a `manifest.tsv`, sin borrar filas ni
   decisiones previas. El agente orquestador inspecciona el catálogo y marca cada
   fila como `relevante` o `descartado`, siempre con un motivo; las nuevas quedan
   `pendiente`.
5. Solo después ejecuta `scripts/pulpo.rs descargar`: opera sobre filas
   `relevante`, prueba primero `pdf_url` y después `landing_url`, y no repite una
   fila ya `accepted` en el manifiesto. Usa un identificador de agente y límites
   de red, escribe primero en temporal, comprueba que el archivo sea un PDF real,
   conserva la URL que funcionó y produce una línea por obra.
6. No incorpores automáticamente un archivo que no sea PDF, esté vacío o no
   pueda abrirse con el extractor que usa `biblio-rata`. Déjalo con un estado de
   fallo o revisión en el manifiesto y registra la causa.
7. Separa duplicado exacto de versión distinta. El mismo hash puede evitar una
   segunda copia; título parecido, DOI o URL por sí solos no autorizan borrar
   una edición, traducción o preprint.
8. Si una fuente termina pidiendo login para entregar el documento, marca ese
   proveedor como bloqueado y no sigas intentando con él. Conserva el DOI,
   título y autores como pistas para que otros scrapers busquen una copia
   abierta en otra fuente. Esta parada es propia de pulpo-librero: no convierte
   un diagnóstico de `chatarrero` en un veto global.
9. Consulta `biblio-rata` sobre el corpus existente y revisa si satisface los
   criterios del problema. Si faltan autores,
   periodos, perspectivas o tipos de fuente, formula otra ronda de búsquedas y
   repite solo los scrapers necesarios.
10. Solo después mueve los documentos aceptados al corpus definitivo y ejecuta la
   indexación incremental de `biblio-rata`.

## Herramienta Rust

Compila sin añadir dependencias al repositorio:

```sh
rustc scripts/pulpo.rs -O -o /tmp/pulpo-librero
```

Los scrapers entregan a `buscar` un TSV con encabezado. El comando exige
`--source`, conserva los metadatos conocidos y escribe o actualiza
`catalogo.tsv` en el directorio de la biblioteca:

```sh
/tmp/pulpo-librero buscar --input /tmp/openalex.tsv \
  --dest /ruta/biblioteca --source openalex
```

El descargador exige hosts autorizados y aplica límites por defecto ajustables:

```sh
/tmp/pulpo-librero descargar --catalogo /ruta/biblioteca/catalogo.tsv \
  --dest /ruta/biblioteca \
  --allow-host arxiv.org --max-files 100 --max-bytes 104857600 \
  --max-total-bytes 1073741824
```

Las redirecciones se siguen una a una solo si el host de cada destino también
está autorizado. No se permiten hosts locales o privados. Un archivo se acepta
solo después de pasar la comprobación de apertura del extractor disponible
(`pdftotext` o PyMuPDF), no solo por su cabecera `%PDF-`.

El primer scraper acoplado es OpenAlex. Compílalo para producir un catálogo de
descubrimiento, sin descargar obras:

```sh
rustc scripts/openalex.rs -O -o /tmp/pulpo-openalex
/tmp/pulpo-openalex search --query "palabra OR palabra2" --out /tmp/candidatos.tsv --per-page 25
```

El TSV de OpenAlex incluye `is_oa`, `oa_status`, `license`, `version`,
`landing_url`, `pdf_url`, `pdf_urls` y `abstract`, pero no es una lista de archivos
aceptados. El orquestador debe inspeccionar sus ubicaciones abiertas, ampliar la
búsqueda con variantes del nombre cuando corresponda y decidir en el catálogo
qué candidatos pasan al descargador. El esquema completo está en
`references/contrato.md`.

Para buscar en arXiv, compila el adaptador oficial Atom y conserva su salida
como catálogo de candidatos:

```sh
rustc scripts/arxiv.rs -O -o /tmp/pulpo-arxiv
/tmp/pulpo-arxiv search --query "au:Apellido AND ti:tema" --out /tmp/arxiv.tsv --max-results 25

# versión concreta, sin que la búsqueda pueda sustituirla por la actual
/tmp/pulpo-arxiv search --id 1706.03762v1 --out /tmp/arxiv-v1.tsv
```

Para corridas recurrentes, informa un contacto con `--mailto` y respeta la
pausa de 3 segundos que aplica el adaptador entre consultas. No subas a
Playwright o a un navegador anti-bot: el sondeo de arXiv confirmó que la API y
el PDF funcionan con HTTP simple.

El identificador `arxiv:NNNN.NNNNNvN` y la URL del PDF conservan la versión
encontrada. Si se desea la versión actual, quita el sufijo `vN` solo después de
decidirlo explícitamente.

Los scrapers y el descargador usan `curl` del sistema para HTTP(S); no
implementan evasión, JavaScript ni autenticación. Si `curl` no está disponible,
informan el fallo sin instalar paquetes. Para lotes grandes, el orquestador
conserva solo el resumen de salida y los manifiestos, no el cuerpo de las
respuestas.

## Vivencias propias

Lee `vivencias/ajustes.json` al comenzar una corrida y respeta sus preferencias:

- `max_files` (number): límite predeterminado de obras procesadas por corrida.
- `max_bytes_por_archivo` (number): tamaño máximo predeterminado de cada PDF.
- `max_total_bytes` (number): tamaño máximo predeterminado del lote aceptado.

Pasa esos valores a `scripts/pulpo.rs` mediante `--max-files`, `--max-bytes` y
`--max-total-bytes`. Los ajustes solo pueden reducir los límites de seguridad
efectivos de una corrida; no autorizan hosts, evaden bloqueos ni habilitan
descargas restringidas.

`vivencias/ajustes.json` también declara `familia`, la relación de crianza con
sus dos skills madre (`chatarrero`, `biblio-rata`; ver "Skills madre" arriba),
con la fecha en que quedó registrada. El validador la exige porque esta skill
siempre depende de ambas.

El validador comprueba la forma del archivo, las claves declaradas y sus tipos;
no valida contenido bibliográfico.

```sh
rustc scripts/validar_ajustes.rs -O -o /tmp/pulpo-librero-validar
/tmp/pulpo-librero-validar vivencias/ajustes.json
```

## Cierre

Informa cuántos candidatos fueron aceptados, rechazados, repetidos o fallaron,
con sus identificadores y causas, y qué criterios del problema siguen sin
cubrirse. Conserva procedencia, fecha de incorporación, URL, tamaño y hash
cuando estén disponibles. No afirmes que una obra quedó incorporada hasta que
el PDF haya pasado la validación y la indexación.
