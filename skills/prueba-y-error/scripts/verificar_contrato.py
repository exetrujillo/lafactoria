#!/usr/bin/env python3
"""comprueba que el contrato tenga todos los encabezados no vacíos del nodo 0."""

import argparse
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from parte import DIMENSIONES_COSTO  # noqa: E402

OBLIGATORIAS = (
    "Objetivo", "Métrica primaria", "Traducción a la decisión",
    "Umbral de relevancia", "Presupuesto", "Protocolo de control",
    "Criterio de parada",
)
MARCADORES = {
    "Métrica primaria": (r"(?im)^\s*(?:unidad|unit)\s*:",),
    "Umbral de relevancia": (r"[-+]?\d+(?:[.,]\d+)?",),
    "Presupuesto": (r"(?im)^\s*max_(?:" + "|".join(DIMENSIONES_COSTO) + r")\s*:",),
    "Protocolo de control": (r"(?im)^\s*(?:entrada|input)\s*:", r"(?im)^\s*(?:salida|output)\s*:"),
    "Criterio de parada": (r"(?im)\b(?:presupuesto|separaci[oó]n|valor de informaci[oó]n)\b",),
}

RELLENO = re.compile(r"(?i)\b(?:todo por definir|pendiente|tbd|hacerlo bien|por decidir)\b")

def secciones(texto):
    encontradas = {}
    coincidencias = list(re.finditer(r"^##[ \t]+(.+?)[ \t]*$", texto, re.MULTILINE))
    for indice, coincidencia in enumerate(coincidencias):
        fin = coincidencias[indice + 1].start() if indice + 1 < len(coincidencias) else len(texto)
        encontradas[coincidencia.group(1).strip()] = texto[coincidencia.end():fin].strip()
    return encontradas

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("contrato", type=Path)
    args = parser.parse_args()
    try:
        encontradas = secciones(args.contrato.read_text(encoding="utf-8"))
    except OSError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    errores = []
    for encabezado in OBLIGATORIAS:
        if encabezado not in encontradas:
            errores.append(f"falta el encabezado '## {encabezado}'")
        elif not encontradas[encabezado]:
            errores.append(f"el encabezado '## {encabezado}' está vacío")
        elif RELLENO.search(encontradas[encabezado]):
            errores.append(f"el encabezado '## {encabezado}' contiene un marcador de relleno")
        else:
            for marcador in MARCADORES.get(encabezado, ()):
                if not re.search(marcador, encontradas[encabezado]):
                    errores.append(f"el encabezado '## {encabezado}' carece de un campo ejecutable")
    for error in errores:
        print(f"error: {error}")
    if errores:
        return 1
    print(f"OK: contrato válido ({len(OBLIGATORIAS)} secciones obligatorias)")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
