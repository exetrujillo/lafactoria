#!/usr/bin/env python3
"""Ejecuta una corrida y emite un recibo de consumo medido, no declarado.

Mientras el agente escriba sus propios costos, el presupuesto es una nota al pie.
Subdeclarar sale gratis y una corrida abortada desaparece del recuento. Acá el
tiempo lo mide el reloj, las consultas salen del raw y el recibo es inmutable.
"""

import argparse
import hashlib
import json
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from parte import emitir


def casos_del_crudo(ruta):
    """consultas al banco = casos distintos efectivamente observados."""
    casos = set()
    if not ruta.exists():
        return casos
    for linea in ruta.read_text(encoding="utf-8", errors="replace").splitlines():
        if not linea.strip():
            continue
        try:
            fila = json.loads(linea)
        except json.JSONDecodeError:
            continue
        if isinstance(fila, dict) and isinstance(fila.get("caso"), str):
            casos.add(fila["caso"])
    return casos

def casos_previos(directorio, hash_contrato, iteracion):
    """casos consultados en rondas ANTERIORES de este lineage.

    Evaluar varios candidatos sobre el mismo banco dentro de una ronda es diseño
    pareado, no adaptación. Lo que gasta validez es volver al banco después de
    haber visto resultados (Dwork et al.), es decir en una iteración posterior.
    """
    vistos = set()
    if not directorio.exists():
        return vistos
    for recibo in sorted(directorio.glob("*.json")):
        try:
            datos = json.loads(recibo.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        if datos.get("hash_contrato") == hash_contrato and datos.get("iteracion", 0) < iteracion:
            vistos.update(datos.get("casos", []))
    return vistos

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--recibos", type=Path, required=True, help="directorio runs/")
    parser.add_argument("--crudo", type=Path, required=True, help="JSONL que escribe el comando")
    parser.add_argument("--iteracion", type=int, required=True)
    parser.add_argument("--candidato", required=True)
    parser.add_argument("--fase", required=True)
    parser.add_argument("--hash-contrato", required=True)
    parser.add_argument("--timeout", type=float, default=1800.0)
    parser.add_argument("comando", nargs=argparse.REMAINDER,
                        help="tras -- : el protocolo declarado en el contrato")
    args = parser.parse_args()

    comando = args.comando[1:] if args.comando and args.comando[0] == "--" else args.comando
    if not comando:
        print(emitir("corrida", "error", {"motivo": "falta el comando tras --"}))
        return 2

    args.recibos.mkdir(parents=True, exist_ok=True)
    destino = args.recibos / f"{args.run_id}.json"
    if destino.exists():
        # reutilizar un run_id permitiría pisar el costo de una corrida ya contada.
        print(emitir("corrida", "error", {"motivo": f"run_id '{args.run_id}' ya existe"}))
        return 2

    args.crudo.parent.mkdir(parents=True, exist_ok=True)
    previos = casos_previos(args.recibos, args.hash_contrato, args.iteracion)
    inicio = time.monotonic()
    try:
        proceso = subprocess.run(comando, capture_output=True, text=True, timeout=args.timeout)
        codigo, estado = proceso.returncode, ("completed" if proceso.returncode == 0 else "failed")
        error = proceso.stderr[-800:]
    except subprocess.TimeoutExpired:
        codigo, estado, error = None, "aborted", f"timeout tras {args.timeout}s"
    except OSError as exc:
        codigo, estado, error = None, "invalid", str(exc)
    segundos = time.monotonic() - inicio

    casos = casos_del_crudo(args.crudo)
    if estado == "completed" and not casos:
        # el comando terminó bien pero no dejó observaciones legibles. El costo
        # igual se paga, y llamarlo "completed" ocultaría un instrumento roto.
        estado, error = "invalid", "el comando no escribió observaciones en el crudo"

    recibo = {
        "run_id": args.run_id,
        "iteracion": args.iteracion,
        "candidato": args.candidato,
        "fase": args.fase,
        "hash_contrato": args.hash_contrato,
        "estado": estado,
        "exit_code": codigo,
        "comando": comando,
        "crudo": str(args.crudo),
        "sha256_crudo": hashlib.sha256(args.crudo.read_bytes()).hexdigest() if args.crudo.exists() else None,
        "casos": sorted(casos),
        "ts": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "costo": {
            "corridas": 1,
            "segundos": round(segundos, 3),
            "consultas_banco": len(casos),
            "rondas": 1,
            "consultas_adaptativas": len(casos & previos),
        },
    }
    if error:
        recibo["error"] = error
    destino.write_text(json.dumps(recibo, ensure_ascii=False, indent=1), encoding="utf-8")

    print(emitir("corrida", "ok" if estado == "completed" else "bloqueado", {
        "run_id": args.run_id,
        "candidato": args.candidato,
        "estado": estado,
        "segundos": recibo["costo"]["segundos"],
        "consultas_banco": recibo["costo"]["consultas_banco"],
        "consultas_adaptativas": recibo["costo"]["consultas_adaptativas"],
        "motivo": error.splitlines()[-1] if error else None,
    }, detalle=str(destino)))
    return 0 if estado == "completed" else 1

if __name__ == "__main__":
    raise SystemExit(main())
