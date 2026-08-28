#!/usr/bin/env python3
"""Imprime el texto exacto de una página o de un rango corto.

  python3 pagina.py SLUG PAGINA [HASTA] [--corpus DIR] [--ficha]

Se usa solo cuando el fragmento que devolvió buscar.py no alcanza.
El tope inicial es de 5 páginas por corrida.
Se asume que si necesitas más de 5 páginas casi siempre la consulta
original estaba mal formulada y conviene volver a buscar.
"""
from __future__ import annotations

import argparse
import json
import sqlite3
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).parent))
import entorno as E

TOPE = 5

def ficha(con: sqlite3.Connection, slug: str) -> None:
    ruta, titulo, autor, paginas, toc = con.execute(
        "SELECT ruta, titulo, autor, paginas, toc FROM doc WHERE slug=?",
        (slug,)).fetchone()
    print(f"{slug}\n  título : {titulo or '(sin metadatos)'}")
    print(f"  autor  : {autor or '(sin metadatos)'}")
    print(f"  páginas: {paginas}\n  ruta   : {ruta}")
    entradas = json.loads(toc or "[]")
    if entradas:
        print(f"  índice ({len(entradas)} entradas):")
        for nivel, titulo_e, pag in entradas:
            print(f"    {'  ' * (nivel - 1)}{titulo_e[:64]}  p.{pag}")
    else:
        print("  índice : el PDF no trae uno embebido")

def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(
        prog="pagina.py", description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("slug", help="documento; admite prefijo del slug")
    p.add_argument("desde", nargs="?", type=int, help="primera página")
    p.add_argument("hasta", nargs="?", type=int, help="última página (opcional)")
    p.add_argument("--corpus", help="directorio con los PDF")
    p.add_argument("--ficha", action="store_true",
                   help="metadatos e índice embebido en vez del texto")
    a = p.parse_args(argv)

    db = E.ruta_db(E.resolver_corpus(a.corpus))
    if not db.is_file():
        raise SystemExit(f"error: no hay índice en {db}; corre indexar.py")
    con = sqlite3.connect(db)

    cands = [s for (s,) in con.execute("SELECT slug FROM doc")
             if s == a.slug or s.startswith(a.slug)]
    if len(cands) != 1:
        raise SystemExit(
            f"error: '{a.slug}' " +
            ("no coincide con ningún documento" if not cands
             else "es ambiguo: " + ", ".join(cands)))
    slug = cands[0]

    if a.ficha:
        ficha(con, slug)
        return 0
    if a.desde is None:
        p.error("falta el número de página (o usa --ficha)")

    desde = a.desde
    hasta = a.hasta if a.hasta is not None else desde
    if hasta - desde + 1 > TOPE:
        print(f"aviso: pediste {hasta - desde + 1} páginas; recorto a {TOPE}. "
              f"Si necesitas más, reformula la búsqueda.", file=sys.stderr)
        hasta = desde + TOPE - 1

    filas = con.execute(
        "SELECT pag, texto FROM pagina WHERE slug=? AND pag BETWEEN ? AND ? "
        "ORDER BY pag", (slug, desde, hasta)).fetchall()
    if not filas:
        total = con.execute("SELECT paginas FROM doc WHERE slug=?",
                            (slug,)).fetchone()[0]
        raise SystemExit(f"error: {slug} no tiene la página {desde} (tiene {total})")
    for pag, texto in filas:
        print(f"───── {slug} p.{pag} ─────")
        print(texto.strip())
    return 0

if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
