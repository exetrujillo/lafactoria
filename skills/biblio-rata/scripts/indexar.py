#!/usr/bin/env python3
"""Construye el índice de búsqueda del corpus, una fila por página.

  python3 indexar.py [CORPUS] [--rehacer] [--listar]

Se salta los PDF cuya ruta, mtime y tamaño no cambiaron. Si el
extractor disponible cambió desde la última corrida, reindexa todo,
porque el texto extraído no es comparable entre extractores.
"""
from __future__ import annotations
import json
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).parent))
import entorno as E

UMBRAL_DENSIDAD = 200

ESQUEMA = """
CREATE TABLE IF NOT EXISTS meta(clave TEXT PRIMARY KEY, valor TEXT);
CREATE TABLE IF NOT EXISTS doc(
  slug TEXT PRIMARY KEY, ruta TEXT NOT NULL, titulo TEXT, autor TEXT,
  paginas INTEGER, mtime REAL, tam INTEGER, toc TEXT);
CREATE VIRTUAL TABLE IF NOT EXISTS pagina USING fts5(
  slug UNINDEXED, pag UNINDEXED, texto,
  tokenize = "porter unicode61 remove_diacritics 2");
"""

def _partir_en_paginas(texto: str) -> list[str]:
    partes = texto.split("\f")
    if partes and not partes[-1].strip():
        partes.pop()
    return partes

def _paginas_reales(ruta: Path) -> int | None:
    try:
        r = subprocess.run(["pdfinfo", str(ruta)],
                           capture_output=True, text=True, errors="replace")
    except OSError:
        return None
    for linea in r.stdout.splitlines():
        if linea.lower().startswith("pages:"):
            try:
                return int(linea.split(":", 1)[1].strip().split()[0])
            except (ValueError, IndexError):
                return None
    return None

def _extraer_pagina_a_pagina(ruta: Path, n: int) -> list[str]:
    """Respaldo caro: un subproceso por página. Solo cuando el corte por \\f falla."""
    print(f"    (el corte por página falló; extrayendo {n} páginas de a una)")
    paginas = []
    for i in range(1, n + 1):
        r = subprocess.run(
            ["pdftotext", "-q", "-f", str(i), "-l", str(i), str(ruta), "-"],
            capture_output=True, text=True, errors="replace")
        paginas.append(r.stdout.replace("\f", " "))
    return paginas

def extraer(ruta: Path, extractor: str) -> tuple[list[str], dict]:
    if extractor == "pymupdf":
        try:
            import pymupdf
        except ImportError:
            import fitz as pymupdf
        try:
            pymupdf.TOOLS.mupdf_display_errors(False)
        except Exception:
            pass
        doc = pymupdf.open(ruta)
        try:
            paginas = [p.get_text() for p in doc]
            md = doc.metadata or {}
            info = {
                "titulo": (md.get("title") or "").strip(),
                "autor": (md.get("author") or "").strip(),
                "toc": json.dumps(doc.get_toc(), ensure_ascii=False),
            }
        finally:
            doc.close()
        return paginas, info

    if extractor == "pdftotext":
        r = subprocess.run(["pdftotext", "-q", str(ruta), "-"],
                           capture_output=True, text=True, errors="replace")
        paginas = _partir_en_paginas(r.stdout)
        esperadas = _paginas_reales(ruta)
        if esperadas and len(paginas) != esperadas:
            paginas = _extraer_pagina_a_pagina(ruta, esperadas)
        return paginas, {"titulo": "", "autor": "", "toc": "[]"}

    raise SystemExit(f"error: extractor desconocido '{extractor}'")

def abrir_db(corpus: Path) -> sqlite3.Connection:
    db = E.ruta_db(corpus)
    db.parent.mkdir(parents=True, exist_ok=True)
    con = sqlite3.connect(db)
    con.executescript(ESQUEMA)
    return con

def leer_meta(con: sqlite3.Connection, clave: str) -> str | None:
    fila = con.execute("SELECT valor FROM meta WHERE clave=?", (clave,)).fetchone()
    return fila[0] if fila else None

def escribir_meta(con: sqlite3.Connection, clave: str, valor: str) -> None:
    con.execute("INSERT OR REPLACE INTO meta VALUES (?,?)", (clave, str(valor)))

def listar(corpus: Path) -> None:
    db = E.ruta_db(corpus)
    if not db.is_file():
        raise SystemExit(f"error: no hay índice en {db}; corre indexar.py primero")
    con = sqlite3.connect(db)
    filas = con.execute(
        "SELECT slug, paginas, titulo FROM doc ORDER BY slug").fetchall()
    for slug, pags, titulo in filas:
        print(f"{slug:<52} {pags:>5} p.  {(titulo or '')[:40]}")
    print(f"\n{len(filas)} documentos · {sum(f[1] or 0 for f in filas)} páginas")

def main(argv: list[str]) -> int:
    args = [a for a in argv if not a.startswith("--")]
    banderas = {a for a in argv if a.startswith("--")}
    corpus = E.resolver_corpus(args[0] if args else None)

    if "--listar" in banderas:
        listar(corpus)
        return 0
    env = E.cargar()
    con = abrir_db(corpus)
    rehacer = "--rehacer" in banderas
    previo = leer_meta(con, "extractor")
    if previo and previo != env["extractor"]:
        print(f"aviso: el extractor cambió de {previo} a {env['extractor']}; "
              f"reindexando todo")
        rehacer = True
    if rehacer:
        con.execute("DELETE FROM pagina")
        con.execute("DELETE FROM doc")

    conocidos = {
        slug: (ruta, mtime, tam)
        for slug, ruta, mtime, tam in con.execute(
            "SELECT slug, ruta, mtime, tam FROM doc")
    }

    pdfs = sorted(p for p in corpus.rglob("*.pdf")
                  if E.DIR_INDICE not in p.parts)
    if not pdfs:
        raise SystemExit(f"error: no hay PDFs bajo {corpus}")

    t0 = time.time()
    nuevos = saltados = paginas_tot = 0
    for ruta in pdfs:
        slug = ruta.stem
        st = ruta.stat()
        firma = (str(ruta), st.st_mtime, st.st_size)
        if conocidos.get(slug) == firma:
            saltados += 1
            continue

        paginas, info = extraer(ruta, env["extractor"])
        if not any(p.strip() for p in paginas):
            print(f"aviso: '{slug}' no dio texto (¿PDF escaneado sin OCR?), se omite")
            continue
        densidad = sum(len(p) for p in paginas) // max(1, len(paginas))
        if densidad < UMBRAL_DENSIDAD:
            print(f"[!] aviso: '{slug}' solo {densidad} car./pág.: parece un "
                  f"escaneo sin OCR y no será encontrable "
                  f"(arreglo: ocrmypdf entrada.pdf salida.pdf)")
        con.execute("DELETE FROM pagina WHERE slug=?", (slug,))
        con.execute("DELETE FROM doc WHERE slug=?", (slug,))
        con.executemany(
            "INSERT INTO pagina(slug, pag, texto) VALUES (?,?,?)",
            [(slug, i + 1, t) for i, t in enumerate(paginas)])
        con.execute(
            "INSERT INTO doc(slug, ruta, titulo, autor, paginas, mtime, tam, toc)"
            " VALUES (?,?,?,?,?,?,?,?)",
            (slug, str(ruta), info["titulo"], info["autor"], len(paginas),
             st.st_mtime, st.st_size, info["toc"]))
        nuevos += 1
        paginas_tot += len(paginas)
        print(f"  + {slug[:56]:<56} {len(paginas):>5} p.")

    escribir_meta(con, "extractor", env["extractor"])
    con.commit()
    if nuevos:
        con.execute("INSERT INTO pagina(pagina) VALUES ('optimize')")
        con.commit()
    if rehacer:
        con.execute("VACUUM")  # sino cada reconstrucción deja páginas libres
    con.close()

    tam_db = E.ruta_db(corpus).stat().st_size / 1e6
    print(f"\n{nuevos} indexados ({paginas_tot} páginas), {saltados} sin cambios "
          f"· {time.time() - t0:.1f} s · índice {tam_db:.1f} MB "
          f"· extractor {env['extractor']}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
