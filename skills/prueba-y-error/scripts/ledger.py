#!/usr/bin/env python3
"""Agrega y valida el ledger de procedencia de prueba-y-error."""

import argparse
import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from parte import DIMENSIONES_COSTO, numero_finito

LARGO_HASH = 12

def hash_contrato(ruta):
    return hashlib.sha256(ruta.read_bytes()).hexdigest()[:LARGO_HASH]

def leer_registros(ruta):
    if not ruta.exists():
        return []
    registros = []
    for numero, linea in enumerate(ruta.read_text(encoding="utf-8").splitlines(), 1):
        if not linea.strip():
            continue
        try:
            registros.append((numero, json.loads(linea)))
        except json.JSONDecodeError as exc:
            raise ValueError(f"línea {numero}: JSON inválido: {exc.msg}") from exc
    return registros

def cargar_valor_json(argumento):
    ruta = Path(argumento)
    texto = ruta.read_text(encoding="utf-8") if ruta.is_file() else argumento
    valor = json.loads(texto)
    if not isinstance(valor, dict):
        raise ValueError("el registro debe ser un objeto JSON")
    return valor

def cargar_fuentes_json(argumento):
    ruta = Path(argumento)
    texto = ruta.read_text(encoding="utf-8") if ruta.is_file() else argumento
    valor = json.loads(texto)
    ids = valor.get("ids") if isinstance(valor, dict) else None
    if (not isinstance(valor, dict) or not isinstance(ids, list) or not ids
            or any(not isinstance(item, str) or not item for item in ids)):
        raise ValueError(
            '--fuentes-json debe tener la forma {"ids": ["caso-1", ...]} '
            'con al menos un identificador de caso (no un nombre de archivo ni una lista suelta)'
        )
    return ids

def validar_resultado(registro):
    metrica = registro.get("metrica")
    if not isinstance(metrica, dict) or not isinstance(metrica.get("intervalo"), list) or len(metrica["intervalo"]) != 2:
        raise ValueError("metrica debe incluir intervalo con dos valores")
    if not numero_finito(metrica.get("valor")):
        raise ValueError("metrica.valor debe ser numérico")
    if any(not numero_finito(valor) for valor in metrica["intervalo"]):
        raise ValueError("metrica.intervalo debe contener números")
    if metrica["intervalo"][0] > metrica["intervalo"][1]:
        raise ValueError("metrica.intervalo debe estar ordenado")
    if not numero_finito(metrica.get("n")) or metrica["n"] <= 0:
        raise ValueError("metrica.n debe ser positivo")
    costos = registro.get("costo")
    if not isinstance(costos, dict):
        raise ValueError("costo debe ser un objeto")
    for campo in DIMENSIONES_COSTO:
        valor = costos.get(campo)
        if not numero_finito(valor) or valor < 0:
            raise ValueError(f"costo.{campo} debe ser un número no negativo")
    fuentes = registro.get("fuentes")
    if not isinstance(fuentes, list) or not fuentes or any(not isinstance(item, str) or not item for item in fuentes):
        raise ValueError("fuentes debe ser una lista no vacía de identificadores")
    if len(fuentes) != len(set(fuentes)):
        raise ValueError("fuentes no puede contener identificadores repetidos")
    if not registro.get("run_id"):
        raise ValueError("todo resultado debe citar el run_id del recibo que lo respalda")
    if not registro.get("sha256_crudo"):
        raise ValueError("todo resultado debe citar el sha256 del crudo analizado")

def armar_resultado(args):
    """Arma el resultado desde el recibo y el análisis: el LLM no escribe cifras.

    Métrica, intervalo, costo y fuentes salen de archivos que produjeron scripts
    deterministas, y nada más entra acá. La interpretación —veredicto, triage,
    decisión— es de otro nodo y vive en su propio registro `diagnostico`, porque
    llega después del dato y el append-only prohíbe editar esta línea.
    """
    receipt = json.loads(args.recibo.read_text(encoding="utf-8"))
    analysis = json.loads(args.analisis.read_text(encoding="utf-8"))
    if receipt.get("estado") != "completed":
        raise ValueError(f"el recibo '{receipt.get('run_id')}' no está completo: {receipt.get('estado')}")
    for field in ("iteracion", "candidato"):
        if receipt.get(field) != getattr(args, field):
            raise ValueError(f"el recibo no corresponde a {field}={getattr(args, field)}")
    digests = analysis.get("sha256_por_crudo", {})
    if receipt.get("sha256_crudo") not in digests.values():
        raise ValueError("el crudo del recibo no participó del análisis citado")
    metric = analysis.get("metricas", {}).get(args.candidato)
    if not isinstance(metric, dict):
        raise ValueError(f"el análisis no contiene métrica para el candidato '{args.candidato}'")
    return {
        "fase": args.fase,
        "iteracion": args.iteracion,
        "candidato": args.candidato,
        "run_id": receipt["run_id"],
        "crudo": receipt.get("crudo"),
        "sha256_crudo": receipt.get("sha256_crudo"),
        "metrica": metric,
        "analisis": {
            "metodo": analysis.get("metodo"),
            "correccion": analysis.get("correccion"),
            "alpha": analysis.get("alpha"),
            "semilla": analysis.get("semilla"),
            "ruta": str(args.analisis),
        },
        "test_aplicado": args.test_aplicado,
        "costo": receipt["costo"],
        "fuentes": receipt.get("casos"),
    }

def buscar_registro(registros, tipo, clave):
    """Primer registro de ese tipo con la misma (iteracion, candidato, linaje)."""
    for _, item in registros:
        if (isinstance(item, dict) and item.get("tipo") == tipo
                and (item.get("iteracion"), item.get("candidato"), item.get("hash_contrato")) == clave):
            return item
    return None

def agregar_registro(ledger, contrato, registro, tipo):
    registros = leer_registros(ledger)
    digest = hash_contrato(contrato)
    clave = (registro.get("iteracion"), registro.get("candidato"), digest)
    if tipo == "resultado":
        plan = buscar_registro(registros, "plan", clave)
        if plan is None:
            raise ValueError("resultado huérfano: no existe un plan previo bajo el linaje vigente")
        if registro.get("test_aplicado") != plan.get("test_declarado"):
            raise ValueError(
                f"test_aplicado no coincide con test_declarado del plan: "
                f"declarado={plan.get('test_declarado')!r} "
                f"aplicado={registro.get('test_aplicado')!r}"
            )
        if registro.get("fuentes") != plan.get("fuentes"):
            raise ValueError(
                f"fuentes del resultado no coinciden con las declaradas en el plan: "
                f"declaradas={plan.get('fuentes')!r} resultado={registro.get('fuentes')!r}"
            )
        validar_resultado(registro)
    elif tipo == "diagnostico":
        # la interpretación necesita el dato ya escrito. Sin resultado previo no
        # hay nada que diagnosticar, y un segundo diagnóstico dejaría dos lecturas
        # de la misma corrida sin manera de saber cuál vale.
        if buscar_registro(registros, "resultado", clave) is None:
            raise ValueError("diagnóstico huérfano: no existe un resultado previo bajo el linaje vigente")
        if buscar_registro(registros, "diagnostico", clave) is not None:
            raise ValueError("ya existe un diagnóstico para esa iteración y candidato")
    anterior = registros[-1][1].get("hash_contrato") if registros else None
    registro = dict(registro)
    registro["tipo"] = tipo
    registro["hash_contrato"] = digest
    registro.setdefault("ts", datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"))
    if anterior and anterior != digest:
        registro["hash_contrato_anterior"] = anterior
    ledger.parent.mkdir(parents=True, exist_ok=True)
    with ledger.open("a", encoding="utf-8") as archivo:
        archivo.write(json.dumps(registro, ensure_ascii=False, separators=(",", ":")) + "\n")
    print(json.dumps(registro, ensure_ascii=False, separators=(",", ":")))

def verificar_cruce(registro):
    """Recalcula nada: relee el análisis en disco y compara.
    Sin esto, el ledger queda editable después del hecho: nada impide subir una
    métrica una vez escrita la línea. El análisis es el testigo independiente.
    """
    problemas = []
    ruta = registro.get("analisis", {}).get("ruta")
    if not ruta:
        return ["resultado sin ruta de análisis"]
    try:
        analisis = json.loads(Path(ruta).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return [f"no se pudo releer el análisis '{ruta}': {exc}"]
    esperada = analisis.get("metricas", {}).get(registro.get("candidato"))
    if esperada is None:
        return [f"el análisis '{ruta}' no contiene al candidato '{registro.get('candidato')}'"]
    if registro.get("metrica") != esperada:
        problemas.append(f"la métrica de '{registro.get('candidato')}' no coincide con el análisis en disco")
    if registro.get("sha256_crudo") not in analisis.get("sha256_por_crudo", {}).values():
        problemas.append("el sha256 del crudo no participó del análisis citado")
    crudo = registro.get("crudo")
    if crudo and Path(crudo).exists():
        # reescribir el raw después del análisis dejaría un ledger coherente
        # consigo mismo pero desconectado de lo que realmente se midió.
        actual = hashlib.sha256(Path(crudo).read_bytes()).hexdigest()
        if actual != registro.get("sha256_crudo"):
            problemas.append(f"el crudo '{crudo}' cambió después de registrarse")
    return problemas

def comprobar(ledger, contrato):
    registros = leer_registros(ledger)
    vigente = hash_contrato(contrato)
    errores = []
    planes = {}
    resultados = {}
    diagnosticos = {}
    hashes_banco = {}
    hash_anterior = None
    for numero, registro in registros:
        if not isinstance(registro, dict):
            errores.append(f"línea {numero}: el registro no es un objeto")
            continue
        digest = registro.get("hash_contrato")
        if not digest:
            errores.append(f"línea {numero}: falta hash_contrato")
        elif hash_anterior and digest != hash_anterior and registro.get("hash_contrato_anterior") != hash_anterior:
            errores.append(f"línea {numero}: cambio de linaje sin hash_contrato_anterior correcto")
        hash_anterior = digest
        clave = (registro.get("iteracion"), registro.get("candidato"), digest)
        if registro.get("tipo") == "plan":
            planes[clave] = numero
            banco = registro.get("hash_banco")
            if banco:
                previo = hashes_banco.get(digest)
                if previo and previo != banco:
                    errores.append(f"línea {numero}: hash_banco cambia dentro del linaje {digest}")
                hashes_banco[digest] = banco
        elif registro.get("tipo") == "resultado" and clave not in planes:
            errores.append(f"línea {numero}: resultado huérfano bajo el linaje {digest}")
        elif registro.get("tipo") == "resultado":
            resultados[clave] = numero
            try:
                validar_resultado(registro)
            except ValueError as exc:
                errores.append(f"línea {numero}: {exc}")
            errores.extend(f"línea {numero}: {problema}" for problema in verificar_cruce(registro))
        elif registro.get("tipo") == "diagnostico":
            # un resultado sin diagnóstico no es un error acá: la ronda escribe sus
            # resultados y corre esta comprobación antes de que el agente interprete nada.
            # El pendiente lo cobra ronda.py al abrir la ronda siguiente.
            if clave not in resultados:
                errores.append(f"línea {numero}: diagnóstico huérfano bajo el linaje {digest}")
            elif clave in diagnosticos:
                errores.append(f"línea {numero}: segundo diagnóstico para la línea {diagnosticos[clave]}")
            else:
                diagnosticos[clave] = numero
    hashes = {registro.get("hash_contrato") for _, registro in registros if isinstance(registro, dict)}
    if registros and vigente not in hashes:
        errores.append(f"el contrato vigente ({vigente}) no aparece en el ledger")
    if errores:
        for error in errores:
            print(f"error: {error}")
        return 1
    print(f"OK: {len(registros)} registro(s), linaje vigente {vigente}")
    return 0

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    sub = subparsers.add_parser("plan")
    sub.add_argument("ledger", type=Path)
    sub.add_argument("--contrato", dest="contrato", type=Path, required=True)
    sub.add_argument("--fase", required=True)
    sub.add_argument("--iteracion", type=int, required=True)
    sub.add_argument("--candidato", required=True)
    sub.add_argument("--hipotesis", required=True)
    sub.add_argument("--prediccion-json", required=True)
    sub.add_argument("--diseno-json", required=True)
    sub.add_argument("--test-declarado", required=True)
    sub.add_argument("--decision-si-falla", required=True)
    sub.add_argument("--fuentes-json", required=True)
    sub.add_argument("--regla", required=True, help="la política de selección se pre-declara")
    sub.add_argument("--hash-banco")
    sub = subparsers.add_parser("resultado")
    sub.add_argument("ledger", type=Path)
    sub.add_argument("--contrato", dest="contrato", type=Path, required=True)
    sub.add_argument("--fase", required=True)
    sub.add_argument("--iteracion", type=int, required=True)
    sub.add_argument("--candidato", required=True)
    sub.add_argument("--recibo", type=Path, required=True, help="runs/<run_id>.json de correr.py")
    sub.add_argument("--analisis", type=Path, required=True, help="salida de analizar.py")
    sub.add_argument("--test-aplicado", required=True)
    sub = subparsers.add_parser("diagnostico")
    sub.add_argument("ledger", type=Path)
    sub.add_argument("--contrato", dest="contrato", type=Path, required=True)
    sub.add_argument("--iteracion", type=int, required=True)
    sub.add_argument("--candidato", required=True)
    sub.add_argument("--veredicto", required=True, help="predicción cumplida o fallida, con el número")
    sub.add_argument("--decision", required=True, choices=("muere", "sigue", "promueve"))
    sub.add_argument("--via", type=int, choices=(1, 2, 3), help="triage; solo si la predicción falló")
    sub.add_argument("--justificacion")
    sub = subparsers.add_parser("check")
    sub.add_argument("ledger", type=Path)
    sub.add_argument("contrato", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "check":
            return comprobar(args.ledger, args.contrato)
        if args.command == "plan":
            registro = {
                "fase": args.fase,
                "iteracion": args.iteracion,
                "candidato": args.candidato,
                "hipotesis": args.hipotesis,
                "prediccion": cargar_valor_json(args.prediccion_json),
                "diseno": cargar_valor_json(args.diseno_json),
                "test_declarado": args.test_declarado,
                "decision_si_falla": args.decision_si_falla,
                "fuentes": cargar_fuentes_json(args.fuentes_json),
                "regla": args.regla,
            }
            if args.hash_banco:
                registro["hash_banco"] = args.hash_banco
        elif args.command == "diagnostico":
            if (args.via is None) != (args.justificacion is None):
                raise ValueError("--via y --justificacion van juntas: una vía sin motivo no es un triage")
            registro = {
                "iteracion": args.iteracion,
                "candidato": args.candidato,
                "veredicto": args.veredicto,
                "decision": args.decision,
            }
            if args.via is not None:
                registro["triage"] = {"via": args.via, "justificacion": args.justificacion}
        else:
            registro = armar_resultado(args)
        agregar_registro(args.ledger, args.contrato, registro, args.command)
        return 0
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

if __name__ == "__main__":
    raise SystemExit(main())
