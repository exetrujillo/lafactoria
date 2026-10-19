# Scrapers de frontera

Un adaptador por fuente, todos con el mismo contrato: un archivo Rust sin
dependencias que se compila con `rustc -O` dentro de `<tmp>`, llama a `curl`,
deja al menos 3 s entre peticiones y escribe un TSV con las columnas de
`contrato.md`, incluidas `published` y `cited_by`. Todos aceptan `--query`,
`--out`, `--desde YYYY-MM-DD` y `--max N` (25 por defecto); el resto de flags es
propio de cada fuente. Salen con 2 por uso incorrecto, 1 por fallo de red o de
archivo y 0 si escribieron el TSV. Los fallos transitorios se reintentan dos
veces (Semantic Scholar no reintenta), pero un `Retry-After` de más de dos
minutos no se espera: el adaptador sale con 1 en el acto. En CORE, cuyo
`--max-time` es de 120 s, un timeout tampoco se reintenta. Lo que cada API entrega y lo que no está
medido en `fuentes.md`; acá va solo cómo invocarlos. Los tres adaptadores
originales (arXiv, OpenAlex e Internet Archive) siguen documentados en el
SKILL.md.

Qué señal de tracción trae cada uno:

| Adaptador | Campos | `cited_by` | PDF |
|---|---|---|---|
| `semantic-scholar.rs` | todos | citas de Semantic Scholar | arXiv seguro; editoriales a veces |
| `crossref.rs` | todos, con DOI | `is-referenced-by-count` | el de la editorial; Nature y Springer sí, Wiley y MDPI no |
| `huggingface-papers.rs` | IA y ML | vacío (`upvotes` aparte) | arXiv |
| `osf-preprints.rs` | psicología, ciencias sociales | vacío | sí, por bucket firmado |
| `europepmc.rs` | biomedicina, neurociencias | `citedByCount` | no (challenge); resolver por DOI |
| `biorxiv.rs` | neurociencias, ciencias de la vida | vacío | sí |
| `scielo.rs` | hispana y latinoamericana | citas dentro de SciELO | sí, en el sitio de cada colección |
| `redalyc-lareferencia.rs` | iberoamericana | vacío | solo cuando el repositorio es DSpace |
| `repec-econstor.rs` | economía | vacío | sí, lento |
| `hal.rs` | ciencias sociales, matemáticas, estadística | vacío | sí, con `--solo-pdf` |
| `core.rs` | todos | vacío | no (challenge); resolver por DOI |

Para una fuente sin citas, la tracción se pide después: `openalex resolver`
sobre los DOI del catálogo trae `cited_by` y la fusión lo conserva.

## Semantic Scholar

Usa la búsqueda bulk, la única que responde sin clave. Ordena por citas o por
fecha y no por relevancia, así que conviene acotar con `--min-citas` y
`--campo`:

```sh
rustc scripts/semantic-scholar.rs -O -o <tmp>/pulpo-s2
<tmp>/pulpo-s2 search --query "diffusion models" --out <tmp>/s2.tsv \
  --desde 2024-10-19 --max 25 --min-citas 20 [--orden fecha] [--campo "Psychology,Sociology"]
<tmp>/pulpo-librero buscar --input <tmp>/s2.tsv --dest /ruta/biblioteca --source semantic-scholar
```

Flags: `--orden citas|fecha` (citas por defecto), `--min-citas N`, `--campo
LISTA` (campos de Semantic Scholar separados por coma; no hay Neuroscience, usa
Psychology, Medicine o Biology). Si la obra declara un identificador de arXiv,
`pdf_urls` pone primero su PDF en `arxiv.org`, aunque `openAccessPdf` venga
vacío. Un 429 es el cupo compartido sin clave: el binario sale con 1 y no hay
que reintentar en seguida.

## Crossref

Ordena por citas y por defecto se limita a obras con licencia Creative Commons:

```sh
rustc scripts/crossref.rs -O -o <tmp>/pulpo-crossref
<tmp>/pulpo-crossref search --query "tema" --out <tmp>/crossref.tsv --desde 2024-10-19 --max 50

# lo más reciente primero, antes de que junte citas
<tmp>/pulpo-crossref search --query "tema" --out <tmp>/crossref.tsv \
  --desde 2024-10-19 --hasta 2026-10-19 --orden fecha
```

Flags: `--hasta YYYY-MM-DD`, `--orden citas|fecha|relevancia` (citas por
defecto), `--licencia abierta|todas` (con `todas` las filas sin CC salen
`oa_status=desconocido`), `--mailto` opcional para el polite pool. Las
`pdf_urls` son de la editorial y algunas responden `403` o un challenge, como
Wiley y MDPI. Autoriza solo los hosts que hayas comprobado.

## Hugging Face Papers

Lista lo que la comunidad está votando y busca por tema. Entrega el id de arXiv
y el PDF de arXiv, sin DOI y sin citas:

```sh
rustc scripts/huggingface-papers.rs -O -o <tmp>/pulpo-hf
<tmp>/pulpo-hf search --lista tendencia --desde 2024-10-19 --max 25 --orden votos --out <tmp>/hf.tsv
<tmp>/pulpo-hf search --query "reasoning language model" --desde 2024-10-19 --max 25 --out <tmp>/hf.tsv
<tmp>/pulpo-hf search --id 2501.11223 --out <tmp>/hf.tsv
```

`--lista reciente` trae lo último que entró a Daily Papers e `--id` (repetible)
pide obras concretas. La búsqueda devuelve una sola página de 100 y `--desde`
se aplica sobre esa página. La columna extra `upvotes` es un voto social, no
una cita; `buscar` no la lleva al catálogo, así que si el voto decide una
revisión, léelo del TSV antes de fusionar. Para descargar basta `--allow-host
arxiv.org`. La identidad queda `huggingface-papers:<id>` y no se fusiona sola
con la fila `arxiv:<id>vN` de la misma obra: reconcílialas antes de descargar.

## OSF Preprints

Consulta PsyArXiv, SocArXiv y MetaArXiv, uno por uno, y junta los resultados de
lo más nuevo a lo más viejo. Busca solo en el título, porque la API no busca en
el abstract, así que prueba variantes cortas del término:

```sh
rustc scripts/osf-preprints.rs -O -o <tmp>/pulpo-osf
<tmp>/pulpo-osf search --query "working memory" --out <tmp>/osf.tsv --desde 2024-10-19 --max 25

# otros proveedores de OSF, o un preprint concreto con su versión
<tmp>/pulpo-osf search --query "misinformation" --out <tmp>/osf.tsv --proveedor socarxiv,edarxiv
<tmp>/pulpo-osf search --id cfr7e_v2 --out <tmp>/osf-id.tsv
```

Flags: `--proveedor LISTA`, `--max-paginas N`, `--id`. Cada fila es la versión
vigente (`osf-preprints:<id>_vN`), con el DOI del preprint. `cited_by` sale
vacío. El PDF redirige a un bucket firmado de otro host que vence en una hora,
así que descarga con `--allow-host osf.io --allow-host oraclecloud.com` poco
después de buscar.

## Europe PMC

Busca en título y resumen, filtra por fecha de primera publicación y ordena por
citas:

```sh
rustc scripts/europepmc.rs -O -o <tmp>/pulpo-europepmc
<tmp>/pulpo-europepmc search --query '"causal inference" OR neuroscience' --out <tmp>/europepmc.tsv --desde 2024-10-19 --max 25
<tmp>/pulpo-europepmc search --query 'cortex' --src PPR --orden fecha --out <tmp>/europepmc-ppr.tsv --desde 2024-10-19
<tmp>/pulpo-europepmc search --id PMC11981939 --id 10.1038/s41586-025-09235-0 --out <tmp>/europepmc-ids.tsv
```

Flags: `--orden citas|fecha|relevancia` (citas por defecto), `--src
MED|PPR|...|TODAS` (MED por defecto, con `OPEN_ACCESS:y`; PPR trae preprints sin
ese filtro), `--id` (PMCID, PPR, DOI o PMID, repetible). Sin `:` en la
consulta, el adaptador la envuelve en `TITLE_ABS:(...)`. Sus PDF de
`europepmc.org` están detrás de un challenge, así que la fila sirve por su DOI:
pásala por `openalex resolver` antes de descargar.

## bioRxiv y medRxiv

Recorre por fechas, de lo más nuevo a lo más viejo. La API no busca por texto,
así que `--query` filtra localmente sobre título y abstract (`a b OR c` pide a
y b, o c), y `--max-paginas` limita cuántas páginas de 30 registros revisa:

```sh
rustc scripts/biorxiv.rs -O -o <tmp>/pulpo-biorxiv
<tmp>/pulpo-biorxiv search --query "hippocampus OR hippocampal" --categoria neuroscience \
  --desde 2024-10-19 --max 25 --max-paginas 40 --out <tmp>/biorxiv.tsv

# medRxiv con la misma API; una obra concreta por DOI
<tmp>/pulpo-biorxiv search --servidor medrxiv --categoria neurology --query "stroke" --out <tmp>/medrxiv.tsv
<tmp>/pulpo-biorxiv search --id 10.1101/2024.12.31.630767 --out <tmp>/biorxiv-doi.tsv
```

Flags: `--servidor biorxiv|medrxiv`, `--categoria NOMBRE` (minúsculas, con
espacios), `--hasta`, `--max-paginas N` (20 por defecto), `--solo-publicadas`
(solo preprints que ya tienen versión en revista). `cited_by` sale vacío. El
resumen dice hasta qué fecha llegó la revisión. El `pdf_url` se arma a partir
del DOI y la versión, y se descarga con `--allow-host www.biorxiv.org` o
`www.medrxiv.org`.

## SciELO

Usa ArticleMeta. Como esa API no busca por texto, el adaptador recorre lo último
procesado en cada colección y filtra en el cliente por fecha de publicación y
por los términos de la consulta:

```sh
rustc scripts/scielo.rs -O -o <tmp>/pulpo-scielo
<tmp>/pulpo-scielo search --query "salud mental" --coleccion chl,col,arg --desde 2024-10-19 --max 25 --revisar 150 --out <tmp>/scielo.tsv
<tmp>/pulpo-scielo search --query "bioetica" --coleccion chl --issn 1726-569X --desde 2024-10-19 --out <tmp>/scielo.tsv
<tmp>/pulpo-scielo search --id S1726-569X2024000200263 --coleccion chl --out <tmp>/scielo-id.tsv
```

Flags: `--coleccion LISTA` (obligatoria; el adaptador no elige ninguna),
`--revisar N` (presupuesto de obras revisadas, cada una es una petición con
pausa de 3 s), `--issn`, `--id` (una sola colección). `cited_by` cuenta solo
las citas dentro de SciELO. Para descargar, autoriza el host de la colección
(`www.scielo.cl`, `www.scielo.org.mx`, ...), no `articlemeta.scielo.org`.

## Redalyc y LA Referencia

LA Referencia agrega los repositorios y revistas iberoamericanas, Redalyc
incluido. Filtra por año y pagina de a 100. No da citas, y `pdf_url` solo existe
cuando el repositorio de origen es un DSpace que expone el bitstream:

```sh
rustc scripts/redalyc-lareferencia.rs -O -o <tmp>/pulpo-redalyc-lareferencia
<tmp>/pulpo-redalyc-lareferencia search --query "inteligencia artificial" \
  --out <tmp>/lareferencia.tsv --desde 2024-10-19 --max 25

# una red nacional o un repositorio, con los nombres de las facetas de la API
<tmp>/pulpo-redalyc-lareferencia search --query "educación" --out <tmp>/redalyc.tsv \
  --repositorio Redalyc-UAEMEX --red MX --orden fecha
```

Flags: `--orden relevancia|fecha`, `--red CODIGO` (CL, BR, ES, MX, ...),
`--repositorio NOMBRE` (por ejemplo `Redalyc-UAEMEX`).

## EconStor

Recolecta por OAI-PMH y filtra la consulta en el cliente, porque el repositorio
no ofrece búsqueda por texto a scripts. Cada página son 100 registros y unos
20 s, así que acota con `--desde` y `--paginas`:

```sh
rustc scripts/repec-econstor.rs -O -o <tmp>/pulpo-repec
<tmp>/pulpo-repec search --query "inflation OR monetary policy" --desde 2024-10-19 --out <tmp>/econstor.tsv --max 25 --paginas 10
<tmp>/pulpo-repec search --id 10419/344156 --out <tmp>/econstor-id.tsv
```

Flags: `--paginas N` (10 por defecto), `--set SPEC` (setSpec de OAI, por
ejemplo `doc-type:workingPaper`), `--id HANDLE` (repetible). `published` trae
el día de depósito cuando cae en el año de publicación, y si no, solo el año.
`cited_by` va siempre vacío. El PDF sale del mismo host, así que basta
`--allow-host www.econstor.eu`, pero el servidor entrega a unos 16 KB/s y un
PDF de varios MB tarda minutos. `descargar` no lo corta mientras siga llegando.

## HAL

Por defecto busca artículos y preprints (`ART,UNDEFINED`) abiertos de
`shs,math,stat`, ordenados por relevancia. Un registro abierto puede no tener
archivo propio; con `--solo-pdf` quedan solo los que lo tienen:

```sh
rustc scripts/hal.rs -O -o <tmp>/pulpo-hal
<tmp>/pulpo-hal search --query "causal inference" --out <tmp>/hal.tsv --desde 2024-10-19 --max 25
<tmp>/pulpo-hal search --query "inequality" --out <tmp>/hal.tsv --desde 2024-10-19 --orden fecha --solo-pdf
<tmp>/pulpo-hal search --id hal-05354491 --out <tmp>/hal-ids.tsv
```

Flags: `--orden relevancia|fecha`, `--tipo LISTA|todos`, `--dominio
LISTA|todos`, `--solo-pdf`, `--id` (repetible). `cited_by` va vacío. Los PDF
salen de `hal.science` y de subdominios de portal (`inria.hal.science`,
`shs.hal.science`, ...), que `descargar` necesita en `--allow-host`.

## CORE

Agrega repositorios institucionales de todos los campos y sirve para descubrir
obras recientes que no aparecen en arXiv. Su PDF queda detrás de un challenge
de Cloudflare, así que la salida se lleva al catálogo y sus DOI se resuelven
con `openalex resolver`, no con `descargar` contra `core.ac.uk`:

```sh
rustc scripts/core.rs -O -o <tmp>/pulpo-core
<tmp>/pulpo-core search --query "glacier retreat" --desde 2024-10-19 --max 25 --out <tmp>/core.tsv
```

Flags: `--orden relevancia|fecha` (no hay orden por citas, la API responde
500). `--desde` filtra por año en el servidor y por día en el cliente. Cada
página tarda hasta un minuto, por eso este adaptador usa `--max-time 120` en
vez de 60. `cited_by` sale vacío porque CORE informa 0 en todas las obras.
