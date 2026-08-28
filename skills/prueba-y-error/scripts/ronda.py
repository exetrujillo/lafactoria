#!/usr/bin/env python3
"""Ejecuta una ronda entera —planes, corridas, análisis, resultados— y devuelve un parte.

El gasto de un bucle experimental no está en lo que el modelo calcula sino en
cuántas veces vuelve a leer su contexto para emitir el siguiente comando. Una
ronda de seis candidatos son treinta y pico de turnos si el agente encadena
`plan`, `correr`, `analizar` y `resultado` a mano; acá es uno, y lo que vuelve
son las cifras necesarias para decidir, no el crudo.
"""

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
sys.path.insert(0, str(AQUI))
from parte import emitir  # noqa: E402
from ledger import LARGO_HASH, leer_registros  # noqa: E402
from verificar_presupuesto import cargar_recibos, formatear_saldo, sumar_recibos  # noqa: E402

def correr(argumentos):
    return subprocess.run([sys.executable, *argumentos], capture_output=True, text=True)

def exigir(condicion, mensaje):
    if not condicion:
        raise ValueError(mensaje)

def diagnosticos_pendientes(ledger, digest, iteracion):
    """Resultados de rondas anteriores del linaje vigente que nadie interpretó."""
    registros = [item for _, item in leer_registros(ledger)
                 if isinstance(item, dict) and item.get("hash_contrato") == digest]
    def clave(item):
        return (item.get("iteracion"), item.get("candidato"))
    escritos = {clave(item) for item in registros if item.get("tipo") == "diagnostico"}
    corridos = {clave(item) for item in registros if item.get("tipo") == "resultado"
                and isinstance(item.get("iteracion"), int) and item["iteracion"] < iteracion}
    return sorted(f"it{it}/{candidato}" for it, candidato in corridos - escritos)

def cargar_plan(ruta):
    plan = json.loads(ruta.read_text(encoding="utf-8"))
    exigir(isinstance(plan, dict), "el plan de ronda debe ser un objeto JSON")
    for campo in ("iteracion", "fase", "contrato", "presupuesto", "test_declarado",
                  "analisis", "candidatos"):
        exigir(campo in plan, f"el plan de ronda no declara '{campo}'")
    exigir(isinstance(plan["candidatos"], list) and len(plan["candidatos"]) >= 2,
           "una ronda comparativa necesita al menos dos candidatos")
    for campo in ("metodo", "semilla", "correccion", "alpha", "umbral"):
        exigir(campo in plan["analisis"], f"analisis no declara '{campo}'")
    nombres = [candidato.get("nombre") for candidato in plan["candidatos"]]
    exigir(all(nombres) and len(nombres) == len(set(nombres)),
           "cada candidato necesita un 'nombre' distinto")
    return plan

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("plan", type=Path, help="ronda.json con la ronda completa")
    parser.add_argument("--exp", type=Path, required=True, help="directorio del experimento")
    parser.add_argument("--dry-run", action="store_true", help="valida el plan sin gastar presupuesto")
    args = parser.parse_args()

    try:
        plan = cargar_plan(args.plan)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(emitir("ronda", "error", {"motivo": exc}))
        return 2

    exp, iteracion, fase = args.exp, plan["iteracion"], plan["fase"]
    contrato, presupuesto = Path(plan["contrato"]), Path(plan["presupuesto"])
    ledger, recibos = exp / "ledger.jsonl", exp / "runs"
    analisis_ruta = exp / "analisis" / f"it{iteracion}.json"
    fuentes = plan.get("fuentes") or str(exp / "fuentes.json")
    conf = plan["analisis"]

    # 1. El presupuesto se consulta antes de gastar, no después.
    previo = correr([str(AQUI / "verificar_presupuesto.py"), "--recibos", str(recibos),
                     "--presupuesto", str(presupuesto), "--ledger", str(ledger)])
    if previo.returncode != 0:
        print(emitir("ronda", "bloqueado", {"iteracion": iteracion, "motivo": "presupuesto"},
                     detalle=previo.stdout.strip().replace("\n", " ")[:600]))
        return 1
    digest = hashlib.sha256(contrato.read_bytes()).hexdigest()[:LARGO_HASH]

    # 2. La ronda anterior tiene que estar interpretada. El arnés escribe el dato;
    #    la vía del triage solo la puede poner el agente, y si no la puso, medir
    #    otra vez es acumular corridas sin saber si la anterior fue evidencia.
    try:
        pendientes = diagnosticos_pendientes(ledger, digest, iteracion)
    except (OSError, ValueError) as exc:
        print(emitir("ronda", "error", {"motivo": exc}))
        return 2
    if pendientes:
        print(emitir("ronda", "bloqueado", {"iteracion": iteracion,
                                            "motivo": "diagnóstico pendiente",
                                            "sin_diagnostico": pendientes},
                     detalle=f"{AQUI / 'ledger.py'} diagnostico {ledger} --contrato {contrato}"
                             " --iteracion N --candidato NOMBRE --veredicto '...' --decision muere"))
        return 1
    if args.dry_run:
        print(emitir("ronda", "ok", {"iteracion": iteracion, "candidatos": len(plan["candidatos"]),
                                     "modo": "dry-run", "presupuesto": "disponible"}))
        return 0

    fallos, crudos, corridas = [], [], []
    for candidato in plan["candidatos"]:
        nombre = candidato["nombre"]
        crudo = exp / "crudos" / f"it{iteracion}-{nombre}.jsonl"
        run_id = f"it{iteracion}-{nombre}"

        # 3. Plan antes del dato: la predicción se escribe sin haber corrido nada.
        hecho = correr([str(AQUI / "ledger.py"), "plan", str(ledger), "--contrato", str(contrato),
                        "--fase", fase, "--iteracion", str(iteracion), "--candidato", nombre,
                        "--hipotesis", candidato.get("hipotesis", ""),
                        "--prediccion-json", json.dumps(candidato.get("prediccion", {})),
                        "--diseno-json", json.dumps(candidato.get("diseno", {})),
                        "--test-declarado", plan["test_declarado"],
                        "--decision-si-falla", candidato.get("decision_si_falla", "descartar"),
                        "--fuentes-json", fuentes, "--regla", plan.get("regla", "halving")]
                       + (["--hash-banco", plan["hash_banco"]] if plan.get("hash_banco") else []))
        if hecho.returncode != 0:
            fallos.append(f"plan/{nombre}: {hecho.stderr.strip()[:120]}")
            continue

        # 4. La corrida la mide el reloj, no el agente.
        comando = [str(pieza).replace("{crudo}", str(crudo)) for pieza in candidato["comando"]]
        hecho = correr([str(AQUI / "correr.py"), "--run-id", run_id, "--recibos", str(recibos),
                        "--crudo", str(crudo), "--iteracion", str(iteracion), "--candidato", nombre,
                        "--fase", fase, "--hash-contrato", digest]
                       + (["--timeout", str(plan["timeout"])] if "timeout" in plan else [])
                       + ["--"] + comando)
        if hecho.returncode != 0:
            fallos.append(f"corrida/{nombre}: {hecho.stdout.strip().splitlines()[-1][:120]}")
            continue
        crudos.append(str(crudo))
        corridas.append(nombre)

    if len(crudos) < 2:
        print(emitir("ronda", "bloqueado", {"iteracion": iteracion, "corridas_ok": len(crudos),
                                            "motivo": "menos de dos corridas válidas: no hay comparación"},
                     detalle="; ".join(fallos)[:600]))
        return 1

    # 5. El análisis recalcula desde el crudo; el agente no toca una cifra.
    hecho = correr([str(AQUI / "analizar.py"), *crudos, "--metodo", conf["metodo"],
                    "--semilla", str(conf["semilla"]), "--correccion", conf["correccion"],
                    "--alpha", str(conf["alpha"]), "--umbral", str(conf["umbral"]),
                    "--remuestreos", str(conf.get("remuestreos", 10000)),
                    "--confianza", str(conf.get("confianza", 0.95)), "--salida", str(analisis_ruta)])
    if hecho.returncode != 0:
        print(emitir("ronda", "error", {"iteracion": iteracion, "motivo": "análisis falló"},
                     detalle=hecho.stdout.strip().replace("\n", " ")[:600]))
        return 2
    analisis = json.loads(analisis_ruta.read_text(encoding="utf-8"))

    # 6. Los resultados se derivan de recibo y análisis, nunca se redactan. La
    #    interpretación es del agente y va en su propio registro `diagnostico`.
    for nombre in corridas:
        hecho = correr([str(AQUI / "ledger.py"), "resultado", str(ledger), "--contrato", str(contrato),
                        "--fase", fase, "--iteracion", str(iteracion), "--candidato", nombre,
                        "--recibo", str(recibos / f"it{iteracion}-{nombre}.json"),
                        "--analisis", str(analisis_ruta), "--test-aplicado", plan["test_declarado"]])
        if hecho.returncode != 0:
            fallos.append(f"resultado/{nombre}: {hecho.stderr.strip()[:120]}")

    verificacion = correr([str(AQUI / "ledger.py"), "check", str(ledger), str(contrato)])
    saldo = correr([str(AQUI / "verificar_presupuesto.py"), "--recibos", str(recibos),
                    "--presupuesto", str(presupuesto), "--ledger", str(ledger)])

    orden = analisis["orden"]
    top = next(c for c in analisis["comparaciones"] if c["mejor"] == orden[0])
    presupuesto_datos = json.loads(presupuesto.read_text(encoding="utf-8"))
    linea_saldo = formatear_saldo(sumar_recibos(cargar_recibos(recibos)), presupuesto_datos)
    print(emitir("ronda", "ok" if verificacion.returncode == 0 and not fallos else "bloqueado", {
        "iteracion": iteracion, "fase": fase,
        "corridas_ok": f"{len(corridas)}/{len(plan['candidatos'])}",
        "mejor": orden[0], "metrica": analisis["metricas"][orden[0]]["valor"],
        "ic": analisis["metricas"][orden[0]]["intervalo"],
        "segundo": top["segundo"], "efecto": top["efecto"],
        "ic_efecto": top["intervalo_efecto"], "p_ajustado": top["p_ajustado"],
        "umbral": conf["umbral"], "separa": analisis["separacion"]["separa"],
        "orden": orden, "ledger": "OK" if verificacion.returncode == 0 else "CON ERRORES",
        "saldo": linea_saldo,
        "por_diagnosticar": corridas,
        "presupuesto": "disponible" if saldo.returncode == 0 else "agotado",
        "fallos": "; ".join(fallos)[:200] if fallos else None,
    }, detalle=str(analisis_ruta)))
    return 0 if verificacion.returncode == 0 and not fallos else 1

if __name__ == "__main__":
    raise SystemExit(main())
