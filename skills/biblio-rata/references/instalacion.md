# Instalación y portabilidad

Se lee solo cuando algo falla por falta de un extractor de PDF, o cuando hay
que decidir dónde instalarlo. En la operación normal no hace falta.

El único requisito real es `python3` más **un** extractor de PDF. El índice no
necesita nada instalado: el módulo `sqlite3` de la biblioteca estándar ya trae
FTS5.

La detección de la máquina ocurre **una sola vez** y queda cacheada en
`~/.cache/biblio-rata/entorno.json`; los scripts la leen solos, así que no hay
que inspeccionar el entorno en cada invocación ni gastar un turno en eso. La
escalera de extractores es `pymupdf` (el mejor: páginas, índice y metadatos) →
`pdftotext`.

**Si no hay ninguno**, los scripts fallan con `error: no hay con qué extraer
texto de PDF`. En ese caso no hay que instalar nada por cuenta propia: usar
la herramienta disponible para que el usuario elija dónde instalarlo.

- **En el sistema** (recomendado para esta skill): `pipx install pymupdf`, o el
  gestor del sistema (`apt install python3-fitz`, `brew install pymupdf`), o
  `apt install poppler-utils` si se prefiere `pdftotext`, que es más liviano.
  Como el biblio-rata es global y se usa desde cualquier proyecto, el extractor
  conviene que esté disponible siempre y no dentro de un entorno que haya que
  activar.
- **Sin instalar nada, si hay `uv`**: la mejor opción cuando no se puede o no se
  quiere tocar el sistema.

  ```sh
  uv run --with pymupdf python3 $SK/scripts/indexar.py /ruta/al/corpus
  ```

  Arma un entorno efímero y cacheado, no hay venv que crear ni que activar, y
  después de la primera vez son ~0,2 s de sobrecosto. Como no queda ningún
  entorno que recordar, es la que menos se rompe cuando la skill se invoca desde
  otro proyecto.

- **En un entorno virtual** del proyecto, si no hay `uv`: `python3 -m venv .venv
  && .venv/bin/pip install pymupdf`, y después invocar los scripts con
  `.venv/bin/python3`. Funciona en todas partes porque `venv` y `pip` vienen con
  Python, pero hay que acordarse de usar ese intérprete **cada vez**: invocar
  `python3` a secas falla con `ModuleNotFoundError` y parece un problema de la
  skill.

En distribuciones recientes `pip install --user` falla con
`externally-managed-environment`: ahí las salidas son `pipx`, el paquete del
sistema, `uv`, o el entorno virtual.

El orden de preferencia no es por velocidad —instalar un paquete una sola vez da
igual que tarde 8 s o 2 s— sino por cuántas cosas hay que recordar después. El
sistema y `uv run --with` no obligan a activar nada; el venv sí, y ese olvido es
la falla más común. Como dato medido en una máquina para este caso exacto (un
paquete, `n=3`): `pip + venv` tardó 8,3 s en frío y ~3,8 s con caché, contra
1,7 s y ~0,1 s de `uv`. Los benchmarks públicos que reportan diferencias de
10–100x miden otra cosa: resolución de árboles de dependencias grandes.

`BIBLIO_RATA_EXTRACTOR=pdftotext` fuerza uno en concreto, y
`python3 $SK/scripts/entorno.py --redetectar` rehace la detección. Cambiar de
extractor obliga a reindexar, y el script lo hace solo al notarlo, porque el
texto de dos extractores distintos no es comparable. Si no se puede instalar ningún
extractor, se lo debe decir al usuario. 
