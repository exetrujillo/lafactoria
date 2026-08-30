---
name: biblio-rata
description: >
  Usa esta skill cuando la respuesta a algo esté dentro de PDFs guardados en el
  disco: una carpeta de literatura, un paper suelto, un libro o un manual largo,
  y también cuando haga falta citar con número de página. El biblio-rata indexa
  los PDFs una vez y después responde con fragmentos rankeados por relevancia,
  de modo que se consulta el contenido sin volcar páginas enteras al contexto.
---

# Biblio-rata

Un PDF de 600 páginas son ~150 mil tokens. Leerlo para responder una pregunta
puntual es tirar contexto a la basura. Esta skill construye un índice SQLite
FTS5, una fila por página, y devuelve solo los fragmentos relevantes.

Los scripts están junto a este archivo, en el directorio base de la skill que
el agente informa al cargarla. En los ejemplos, `$SK` es ese directorio.

## Cuándo invocarla

Antes de indexar un corpus pequeño para una sola pregunta, lee
`references/criterio.md`: el costo fijo de cargar la skill puede hacer preferible
leer el texto directamente. Si hay que citar una página, usa el índice aunque el
corpus sea pequeño.

## Procedimiento

**1. Fijar el corpus una vez por sesión** (evita repetir `--corpus` en cada
llamada):

```sh
export BIBLIO_RATA_CORPUS=/ruta/a/la/carpeta/con/pdfs
```

También sirve pasar `--corpus DIR` en cada comando, o dejar que lo deduzca solo:
si no se le dice nada, busca hacia arriba desde el directorio actual la carpeta
que ya tenga un índice.

**2. Buscar.** El índice se construye solo la primera vez (unos segundos por
cada cien páginas); no hace falta indexar a mano.

```sh
python3 $SK/scripts/buscar.py "successive halving" --n 8
```

Cada línea es `slug p.123 · BM25 · …fragmento…`. Más negativo es mejor y ya
vienen ordenados. Opciones: `--n` cuántos resultados, `--doc SLUG` para acotar a
un documento (acepta prefijos del slug), `--tokens` para alargar el fragmento,
`--listar` para ver qué documentos hay. Para sintaxis FTS5 compleja, lee
`references/consultas.md` antes de buscar.

**3. Leer la página exacta solo si el fragmento no alcanza:**

```sh
python3 $SK/scripts/pagina.py li-2018 6        # una página
python3 $SK/scripts/pagina.py garnett --ficha  # metadatos e índice del PDF
```

**4. Citar siempre `documento p.N`.** Es lo que hace verificable la respuesta, y
sale gratis porque el buscador ya devuelve la página.

## Reglas de gasto

- **Nunca** leer un PDF con `Read`, ni volcar su texto completo, ni encadenar
  `pdftotext` a mano. Para eso está el índice.
- **Leer el fragmento antes de abrir ninguna página.** En buena parte de las
  consultas ya contiene la respuesta, y abrir la página es gasto puro.
- **Pedir de a una página y expandir**, no pedir un bloque de entrada. Está
  medido: la respuesta está en la página del mejor hit en 8 de 17 casos y a ±1
  en 13 de 17 casos medidos.
- El tope de 5 páginas por corrida sigue en pie (`pagina.py` recorta ahí), pero
  como **cota** nunca se midió. Si hacen falta más de 5 seguidas, la consulta
  estaba mal formulada y conviene volver a buscar.
- Empezar con `--n 5` o menos. Ampliar solo si los primeros no sirven.

## Mantenimiento

```sh
python3 $SK/scripts/indexar.py            # incremental: solo lo nuevo o cambiado
python3 $SK/scripts/indexar.py --rehacer  # reconstruye desde cero
python3 $SK/scripts/indexar.py --listar   # documentos, páginas y títulos
python3 $SK/scripts/entorno.py            # qué detectó de esta máquina
```

El índice vive en `<corpus>/.biblio-rata/indice.db`, junto a los PDFs. Conviene
agregar `.biblio-rata/` al `.gitignore` del proyecto que lo use, preguntándole
antes al usuario con la herramienta disponible.

Hace falta `python3` y **un** extractor de PDF: `pymupdf` (el mejor, trae
metadatos e índice embebido) o `pdftotext`. Si un script falla con `error: no
hay con qué extraer texto de PDF`, **no instalar nada por cuenta propia**: lee
`references/instalacion.md`, que explica las opciones y manda preguntarle al
usuario dónde instalarlo.

## Vivencias propias

Lee `vivencias/ajustes.json` al comenzar una corrida y respeta sus preferencias:

- `extractor_preferido` (string): extractor que se intentará usar primero,
  `pymupdf` por defecto; si no está disponible, usa la cascada documentada.
- `resultados_por_defecto` (number): cantidad inicial de resultados de `buscar.py`,
  8 por defecto; amplía solo si los primeros no sirven.
- `fragmento_tokens` (number): longitud inicial del fragmento de `buscar.py`,
  24 por defecto; auméntala solo si el fragmento no alcanza.

Aplica las dos últimas preferencias pasando `--n` y `--tokens`. Para preferir
un extractor disponible, usa `BIBLIO_RATA_EXTRACTOR`; no instales dependencias
por cuenta propia si el extractor elegido no está disponible.

`vivencias/ajustes.json` también declara `criados`: `pulpo-librero` depende de
`biblio-rata` en una relación de crianza (ver "Skills madre" en
`skills/pulpo-librero/SKILL.md`), así que un cambio de comportamiento acá
puede afectarlo hoy mismo, no solo en el momento en que se declaró la
relación.

El validador comprueba la forma del archivo, las claves declaradas y sus tipos;
no valida contenido de negocio.

```sh
rustc scripts/validar_ajustes.rs -O -o /tmp/biblio-rata-validar
/tmp/biblio-rata-validar vivencias/ajustes.json
```
