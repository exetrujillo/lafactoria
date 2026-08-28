#!/usr/bin/env python3
"""Busca en el corpus y devuelve fragmentos en lugar de páginas enteras.

  python3 buscar.py "CONSULTA" [--corpus DIR] [--n 8] [--doc SLUG] [--tokens 24]

Cada resultado es una línea:  slug p.123 · -9.84 · …fragmento…
El puntaje es BM25: más negativo es mejor, y vienen ya ordenados.
Si falta el índice, lo construye solo.
"""
from __future__ import annotations
import argparse
import re
import sqlite3
import subprocess
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # no ensuciar caché
sys.path.insert(0, str(Path(__file__).parent))
import entorno as E

BLANCOS = re.compile(r"\s+")

def _una_linea(s: str) -> str:
    return BLANCOS.sub(" ", s).strip()

def _asegurar_indice(corpus: Path) -> None:
    if E.ruta_db(corpus).is_file():
        return
    print(f"(no había índice en {corpus}; construyéndolo)", file=sys.stderr)
    r = subprocess.run(
        [sys.executable, str(Path(__file__).parent / "indexar.py"), str(corpus)],
        stdout=sys.stderr)
    if r.returncode != 0:
        raise SystemExit("error: falló la indexación")

def _resolver_doc(con: sqlite3.Connection, doc: str) -> str:
    cands = [s for (s,) in con.execute("SELECT slug FROM doc")
             if s == doc or s.startswith(doc)]
    if len(cands) == 1:
        return cands[0]
    if not cands:
        raise SystemExit(f"error: ningún documento coincide con '{doc}'")
    raise SystemExit(f"error: '{doc}' es ambiguo: {', '.join(cands)}")

def _buscar(con, consulta, n, doc, tokens):
    filtro = " AND slug = ?" if doc else ""
    extra = [doc] if doc else []
    try:
        filas = con.execute(
            f"SELECT slug, pag, snippet(pagina, 2, '[', ']', '…', ?), bm25(pagina) "
            f"FROM pagina WHERE pagina MATCH ?{filtro} "
            f"ORDER BY bm25(pagina) LIMIT ?",
            [tokens, consulta] + extra + [n]).fetchall()
        total = con.execute(
            f"SELECT count(*) FROM pagina WHERE pagina MATCH ?{filtro}",
            [consulta] + extra).fetchone()[0]
    except sqlite3.OperationalError as e:
        raise SystemExit(
            f"error de sintaxis en la consulta: {e}\n"
            f"recuerda: frases entre comillas dobles, NEAR(a b, 5), "
            f"col:termino, y AND/OR/NOT en mayúsculas. "
            f"Ver references/consultas.md")
    return filas, total

def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(
        prog="buscar.py", description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("consulta", nargs="?", help="consulta FTS5")
    p.add_argument("--corpus", help="directorio con los PDF")
    p.add_argument("--n", type=int, default=8, help="cuántos resultados (8)")
    p.add_argument("--tokens", type=int, default=24, help="largo del fragmento (24)")
    p.add_argument("--doc", help="acotar a un documento; admite prefijo del slug")
    p.add_argument("--listar", action="store_true", help="listar los documentos")
    a = p.parse_args(argv)

    corpus = E.resolver_corpus(a.corpus)
    if a.listar:
        from indexar import listar
        listar(corpus)
        return 0
    if not a.consulta:
        p.error("falta la consulta (o usa --listar)")

    _asegurar_indice(corpus)
    con = sqlite3.connect(E.ruta_db(corpus))
    doc = _resolver_doc(con, a.doc) if a.doc else None
    filas, total = _buscar(con, a.consulta, a.n, doc, a.tokens)

    if not filas:
        print(f"sin resultados para: {a.consulta}")
        return 1
    for slug, pag, frag, puntaje in filas:
        print(f"{slug} p.{pag} · {puntaje:.2f} · {_una_linea(frag)}")
    print(f"— {len(filas)} de {total} páginas con coincidencia")
    return 0

if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
