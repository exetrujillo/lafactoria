#!/usr/bin/env python3
"""Detección de capacidades de la máquina solo una vez y cacheada.

Los otros scripts importan este módulo, no debería ser necesario invocar este archivo a mano.

La caché vive en ~/.cache/biblio-rata/entorno.json.
"""
from __future__ import annotations
import json
import os
import shutil
import sqlite3
import sys
from pathlib import Path

VERSION = 2
# cascada de extractores de PDF, de mejor a peor
CASCADA_EXTRACTORES = ("pymupdf", "pdftotext")

AYUDA_SIN_EXTRACTOR = """no hay con qué extraer texto de PDF. Instala uno:
  pip install pymupdf          (páginas, índice y metadatos)
  apt install poppler-utils    (pdftotext)"""

def ruta_cache() -> Path:
    base = os.environ.get("XDG_CACHE_HOME") or (Path.home() / ".cache")
    return Path(base) / "biblio-rata" / "entorno.json"

def _hay_pymupdf() -> bool:
    for modulo in ("pymupdf", "fitz"):
        try:
            __import__(modulo)
            return True
        except Exception:
            continue
    return False

def detectar() -> dict:
    disponibles = []
    if _hay_pymupdf():
        disponibles.append("pymupdf")
    if shutil.which("pdftotext"):
        disponibles.append("pdftotext")
    disponibles.sort(key=CASCADA_EXTRACTORES.index)
    return {
        "version": VERSION,
        "extractores": disponibles,
        "extractor": disponibles[0] if disponibles else None,
        "sqlite": sqlite3.sqlite_version,
        "python": sys.version.split()[0],
    }

def cargar(redetectar: bool = False) -> dict:
    cache = ruta_cache()
    env = None
    if not redetectar and cache.is_file():
        try:
            env = json.loads(cache.read_text())
            if env.get("version") != VERSION:
                env = None
        except Exception:
            env = None
    if env is None:
        env = detectar()
        try:
            cache.parent.mkdir(parents=True, exist_ok=True)
            cache.write_text(json.dumps(env, indent=2, ensure_ascii=False))
        except OSError:
            pass

    forzado = os.environ.get("BIBLIO_RATA_EXTRACTOR")
    if forzado:
        if forzado not in env["extractores"]:
            raise SystemExit(
                f"error: BIBLIO_RATA_EXTRACTOR={forzado} no está disponible "
                f"(hay: {', '.join(env['extractores']) or 'ninguno'})"
            )
        env = dict(env, extractor=forzado)
    if not env["extractor"]:
        raise SystemExit(f"error: {AYUDA_SIN_EXTRACTOR}")
    return env

DIR_INDICE = ".biblio-rata"
NOMBRE_DB = "indice.db"

def resolver_corpus(explicito: str | None = None) -> Path:
    if explicito:
        p = Path(explicito).expanduser().resolve()
        if not p.is_dir():
            raise SystemExit(f"error: no existe el directorio '{p}'")
        return p
    if os.environ.get("BIBLIO_RATA_CORPUS"):
        return Path(os.environ["BIBLIO_RATA_CORPUS"]).expanduser().resolve()
    aqui = Path.cwd().resolve()
    for d in (aqui, *aqui.parents):
        if (d / DIR_INDICE / NOMBRE_DB).is_file():
            return d
    raise SystemExit(
        "error: no sé sobre qué corpus buscar. Pasa --corpus DIR "
        "o exporta BIBLIO_RATA_CORPUS."
    )

def ruta_db(corpus: Path) -> Path:
    return corpus / DIR_INDICE / NOMBRE_DB

if __name__ == "__main__":
    env = cargar(redetectar="--redetectar" in sys.argv)
    print(json.dumps(env, indent=2, ensure_ascii=False))
    print(f"\ncaché: {ruta_cache()}")
