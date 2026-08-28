#!/usr/bin/env python3
"""Suma el consumo desde los recibos de telemetría y lo contrasta con el límite.

La autoridad sobre el costo es el recibo que escribió el wrapper, no el número
que el agente puso en el ledger. Si ambos discrepan, gana el recibo y la ronda
se bloquea: una diferencia silenciosa es justamente el fallo que esto previene.
"""

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from parte import DIMENSIONES_COSTO, emitir, numero_finito  # noqa: E402

def cargar_recibos(directorio):
    recibos = []
    for ruta in sorted(directorio.glob("*.json")):
        datos = json.loads(ruta.read_text(encoding="utf-8"))
        if not isinstance(datos, dict) or not isinstance(datos.get("costo"), dict):
            raise ValueError(f"{ruta.name}: recibo sin objeto costo")
        for campo in DIMENSIONES_COSTO:
            valor = datos["costo"].get(campo)
            if not numero_finito(valor) or valor < 0:
                raise ValueError(f"{ruta.name}: costo.{campo} no es un número finito no negativo")
        if not datos.get("run_id"):
            raise ValueError(f"{ruta.name}: recibo sin run_id")
        recibos.append(datos)
    identificadores = [recibo["run_id"] for recibo in recibos]
    if len(identificadores) != len(set(identificadores)):
        raise ValueError("hay run_id repetidos entre los recibos")
    return recibos

def cargar_ledger(ruta):
    return [json.loads(linea) for linea in ruta.read_text(encoding="utf-8").splitlines() if linea.strip()]

def conciliar(recibos, registros):
    """Todo resultado debe apoyarse en un recibo, con el mismo costo."""
    errores = []
    por_id = {recibo["run_id"]: recibo for recibo in recibos}
    usados = set()
    for registro in registros:
        if not isinstance(registro, dict) or registro.get("tipo") != "resultado":
            continue
        run_id = registro.get("run_id")
        if not run_id:
            errores.append(f"resultado {registro.get('candidato')}/it{registro.get('iteracion')} sin run_id")
            continue
        if run_id not in por_id:
            errores.append(f"run_id '{run_id}' no tiene recibo")
            continue
        if run_id in usados:
            errores.append(f"run_id '{run_id}' referenciado por más de un resultado")
        usados.add(run_id)
        recibo = por_id[run_id]
        for campo in DIMENSIONES_COSTO:
            if registro.get("costo", {}).get(campo) != recibo["costo"][campo]:
                errores.append(f"run_id '{run_id}': costo.{campo} del ledger no coincide con el recibo")
        for campo in ("iteracion", "candidato", "hash_contrato"):
            if registro.get(campo) != recibo.get(campo):
                errores.append(f"run_id '{run_id}': {campo} del ledger no coincide con el recibo")
    return errores

def sumar_recibos(recibos):
    """Suma las cinco dimensiones usando solo los recibos medidos."""
    consumido = {campo: sum(recibo["costo"][campo] for recibo in recibos)
                 for campo in DIMENSIONES_COSTO}
    consumido["rondas"] = len({recibo.get("iteracion") for recibo in recibos})
    return consumido

def formatear_saldo(consumido, presupuesto):
    return " ".join(f"{campo}={consumido[campo]:.6g}/{presupuesto.get(f'max_{campo}')}"
                    for campo in DIMENSIONES_COSTO)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--recibos", type=Path, required=True)
    parser.add_argument("--presupuesto", type=Path, required=True)
    parser.add_argument("--ledger", type=Path, help="si se pasa, concilia costos declarados vs medidos")
    args = parser.parse_args()
    try:
        presupuesto = json.loads(args.presupuesto.read_text(encoding="utf-8"))
        if not isinstance(presupuesto, dict):
            raise ValueError("el presupuesto debe ser un objeto JSON")
        recibos = cargar_recibos(args.recibos) if args.recibos.exists() else []
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(emitir("presupuesto", "error", {"motivo": exc}))
        return 2

    errores = []
    if args.ledger and args.ledger.exists():
        try:
            errores += conciliar(recibos, cargar_ledger(args.ledger))
        except (OSError, ValueError, json.JSONDecodeError) as exc:
            print(emitir("presupuesto", "error", {"motivo": exc}))
            return 2

    # Las rondas se cuentan por iteración: una ronda con seis candidatos es una.
    consumido = sumar_recibos(recibos)
    agotadas = []
    for campo in DIMENSIONES_COSTO:
        limite = presupuesto.get(f"max_{campo}")
        if not numero_finito(limite) or limite < 0:
            errores.append(f"max_{campo} debe ser un número finito no negativo")
        elif consumido[campo] >= limite:
            agotadas.append(campo)

    campos = {campo: f"{consumido[campo]:.6g}/{presupuesto.get(f'max_{campo}')}" for campo in DIMENSIONES_COSTO}
    campos["recibos"] = len(recibos)
    campos["fallidos"] = sum(1 for recibo in recibos if recibo.get("estado") != "completed")
    if errores:
        campos["motivo"] = "; ".join(errores[:3])
        print(emitir("presupuesto", "error", campos))
        return 2
    if agotadas:
        campos["agotado"] = ",".join(agotadas)
        print(emitir("presupuesto", "bloqueado", campos))
        return 1
    print(emitir("presupuesto", "ok", campos))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
