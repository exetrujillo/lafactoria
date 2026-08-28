#!/usr/bin/env python3
"""Ejercita el arnés contra sus propios modos de fallo. Sin red y sin API.

Una skill que promete garantías tiene que poder demostrarlas de forma barata, o
la única manera de saber si siguen en pie es pagarle a un modelo para que audite.
Esto corre en segundos y no gasta un token.
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

AQUI = Path(__file__).resolve().parent
sys.path.insert(0, str(AQUI))
from analizar import bonferroni
CONTRATO = """## Objetivo
Elegir el tope que maximiza aciertos.

## Métrica primaria
Proporción de aciertos.
unidad: proporción [0,1]

## Traducción a la decisión
Se adopta el tope que gane sobre el umbral.

## Umbral de relevancia
0.05

## Presupuesto
max_corridas: 10
max_segundos: 600
max_consultas_banco: 200
max_rondas: 5
max_consultas_adaptativas: 60

## Protocolo de control
entrada: banco congelado de 30 casos
salida: crudo JSONL por observación

## Criterio de parada
Separación lograda, presupuesto agotado o valor de información bajo.
"""
PROTOCOLO = """import json, random, sys
salida, semilla, candidato, ventaja = sys.argv[1], int(sys.argv[2]), sys.argv[3], float(sys.argv[4])
r = random.Random(semilla)
with open(salida, "w") as f:
    for i in range(30):
        base = random.Random(1000 + i).gauss(0.55, 0.12)
        f.write(json.dumps({"candidato": candidato, "caso": f"q{i:02d}",
                            "valor": round(base + ventaja + r.gauss(0, 0.04), 4)}) + "\\n")
"""
FALLOS = []

def correr(argumentos, cwd):
    return subprocess.run([sys.executable, *[str(a) for a in argumentos]],
                          capture_output=True, text=True, cwd=cwd)

def afirmar(descripcion, condicion, detalle=""):
    print(f"  {'ok  ' if condicion else 'FALLA'} {descripcion}")
    if not condicion:
        FALLOS.append(f"{descripcion} {detalle}".strip())

def montar(raiz, ventaja):
    exp = raiz / "EXP"
    (exp / "crudos").mkdir(parents=True)
    (exp / "contrato.md").write_text(CONTRATO, encoding="utf-8")
    (exp / "presupuesto.json").write_text(json.dumps({
        "max_corridas": 10, "max_segundos": 600, "max_consultas_banco": 200,
        "max_rondas": 5, "max_consultas_adaptativas": 60}), encoding="utf-8")
    (exp / "fuentes.json").write_text(json.dumps({"ids": [f"q{i:02d}" for i in range(30)]}), encoding="utf-8")
    (raiz / "protocolo.py").write_text(PROTOCOLO, encoding="utf-8")
    (exp / "ronda.json").write_text(json.dumps({
        "iteracion": 1, "fase": "A", "contrato": "EXP/contrato.md",
        "presupuesto": "EXP/presupuesto.json", "fuentes": "EXP/fuentes.json",
        "hash_banco": "b7d20be", "regla": "halving",
        "test_declarado": "permutación pareada, holm, alpha 0.05",
        "analisis": {"metodo": "bootstrap_pareado", "semilla": "s1", "correccion": "holm",
                     "alpha": 0.05, "umbral": 0.05, "remuestreos": 2000},
        "candidatos": [
            {"nombre": "base", "hipotesis": "referencia", "prediccion": {"direccion": "base"},
             "diseno": {"pareado": True},
             "comando": ["python3", "protocolo.py", "{crudo}", "11", "base", "0.0"]},
            {"nombre": "alto", "hipotesis": "mejora", "prediccion": {"direccion": "mejor"},
             "diseno": {"pareado": True},
             "comando": ["python3", "protocolo.py", "{crudo}", "12", "alto", str(ventaja)]}]}),
        encoding="utf-8")
    return exp

def prueba_flujo(raiz):
    print("\nflujo completo con un efecto real de +0.09")
    exp = montar(raiz, 0.09)
    hecho = correr([AQUI / "ronda.py", "EXP/ronda.json", "--exp", "EXP"], raiz)
    afirmar("la ronda termina bien", hecho.returncode == 0, hecho.stdout + hecho.stderr)
    afirmar("el parte respeta el tope de 2 KB", len(hecho.stdout.encode()) <= 2048)
    afirmar("detecta la separación", "separa=si" in hecho.stdout, hecho.stdout)
    afirmar("el crudo no entra al parte", "\"caso\"" not in hecho.stdout)
    afirmar("el ledger valida", "ledger=OK" in hecho.stdout, hecho.stdout)
    return exp

def prueba_nulo(raiz):
    print("\nsin efecto real: no debe declarar separación")
    montar(raiz, 0.0)
    hecho = correr([AQUI / "ronda.py", "EXP/ronda.json", "--exp", "EXP"], raiz)
    afirmar("no inventa separación", "separa=no" in hecho.stdout, hecho.stdout)

def prueba_manipulacion(exp):
    print("\nmanipulación posterior del registro")
    ledger = exp / "ledger.jsonl"
    registros = [json.loads(l) for l in ledger.read_text(encoding="utf-8").splitlines() if l.strip()]
    for registro in registros:
        if registro.get("tipo") == "resultado":
            registro["metrica"]["valor"] = 0.99
            break
    ledger.write_text("\n".join(json.dumps(r, ensure_ascii=False) for r in registros) + "\n", encoding="utf-8")
    hecho = correr([AQUI / "ledger.py", "check", "EXP/ledger.jsonl", "EXP/contrato.md"], exp.parent)
    afirmar("detecta una métrica inflada a mano", hecho.returncode != 0, hecho.stdout)

    crudo = next((exp / "crudos").glob("*.jsonl"))
    crudo.write_text(crudo.read_text(encoding="utf-8").replace("0.", "0.9"), encoding="utf-8")
    hecho = correr([AQUI / "ledger.py", "check", "EXP/ledger.jsonl", "EXP/contrato.md"], exp.parent)
    afirmar("detecta un crudo reescrito", "cambió después de registrarse" in hecho.stdout, hecho.stdout)

def prueba_costos(exp):
    print("\ncostos: el recibo manda")
    ledger = exp / "ledger.jsonl"
    registros = [json.loads(l) for l in ledger.read_text(encoding="utf-8").splitlines() if l.strip()]
    for registro in registros:
        if registro.get("tipo") == "resultado":
            registro["costo"]["consultas_banco"] = 1
            break
    (exp / "ledger_falso.jsonl").write_text(
        "\n".join(json.dumps(r, ensure_ascii=False) for r in registros) + "\n", encoding="utf-8")
    hecho = correr([AQUI / "verificar_presupuesto.py", "--recibos", "EXP/runs",
                    "--presupuesto", "EXP/presupuesto.json", "--ledger", "EXP/ledger_falso.jsonl"], exp.parent)
    afirmar("detecta un costo subdeclarado", hecho.returncode == 2, hecho.stdout)

    hecho = correr([AQUI / "correr.py", "--run-id", "it1-base", "--recibos", "EXP/runs",
                    "--crudo", "EXP/crudos/x.jsonl", "--iteracion", "1", "--candidato", "base",
                    "--fase", "A", "--hash-contrato", "abc", "--", "echo", "hola"], exp.parent)
    afirmar("rechaza reutilizar un run_id", hecho.returncode == 2, hecho.stdout)

def prueba_veredictos(raiz):
    print("\ngates de decisión")
    exp = raiz / "V"
    exp.mkdir()
    ledger = exp / "ledger.jsonl"
    lineas = []
    for nombre, valor, intervalo in (("a", 0.70, [0.60, 0.80]), ("b", 0.65, [0.55, 0.75])):
        lineas.append({"tipo": "plan", "fase": "A", "iteracion": 1, "candidato": nombre,
                       "hash_contrato": "aaa", "test_declarado": "t", "fuentes": ["c1"]})
        lineas.append({"tipo": "resultado", "fase": "A", "iteracion": 1, "candidato": nombre,
                       "hash_contrato": "aaa", "test_aplicado": "t", "fuentes": ["c1"],
                       "metrica": {"valor": valor, "intervalo": intervalo, "n": 10}})
    ledger.write_text("\n".join(json.dumps(l) for l in lineas) + "\n", encoding="utf-8")
    (exp / "reporte.md").write_text("efecto observado y sin cerrar\n", encoding="utf-8")

    def claim(decision, extra=None):
        base = {"decision": decision, "comparaciones": [{
            "mejor": {"iteracion": 1, "candidato": "a", "hash_contrato": "aaa"},
            "segundo": {"iteracion": 1, "candidato": "b", "hash_contrato": "aaa"},
            "efecto": 0.05, "intervalo": [0.60, 0.80], "umbral": 0.10}]}
        if extra:
            base["comparaciones"][0].update(extra.pop("comparacion", {}))
            base.update(extra)
        ruta = exp / f"claims_{decision.split()[0].lower()}.json"
        ruta.write_text(json.dumps(base), encoding="utf-8")
        return ruta

    for decision in ("SEGUIR MIDIENDO", "ABANDONAR"):
        hecho = correr([AQUI / "verificar_reporte.py", "V/reporte.md", "V/ledger.jsonl",
                        "--claims-json", str(claim(decision).relative_to(raiz))], raiz)
        afirmar(f"'{decision}' se puede reportar con intervalos solapados",
                hecho.returncode == 0, hecho.stdout)

    ruta = claim("RECOMENDAR", {"confirmacion_independiente": True,
                                 "correccion_comparaciones": "holm", "alpha": 0.05,
                                 "comparacion": {"intervalo_efecto": [0.01, 0.09], "p_ajustado": 0.02}})
    hecho = correr([AQUI / "verificar_reporte.py", "V/reporte.md", "V/ledger.jsonl",
                    "--claims-json", str(ruta.relative_to(raiz))], raiz)
    afirmar("'RECOMENDAR' se rechaza si el IC del efecto cruza el umbral",
            hecho.returncode == 1, hecho.stdout)

    lineas.append({"tipo": "resultado", "fase": "B", "iteracion": 1, "candidato": "confirmacion",
                   "hash_contrato": "aaa", "fuentes": ["c2"],
                   "metrica": {"valor": 0.70, "intervalo": [0.60, 0.80], "n": 10}})
    ledger.write_text("\n".join(json.dumps(l) for l in lineas) + "\n", encoding="utf-8")
    ruta = claim("RECOMENDAR", {"confirmacion_independiente": True,
                                "correccion_comparaciones": "holm", "alpha": 0.05,
                                "comparacion": {"intervalo_efecto": [0.11, 0.20], "p_ajustado": 0.02}})
    hecho = correr([AQUI / "verificar_reporte.py", "V/reporte.md", "V/ledger.jsonl",
                    "--strict", "--claims-json", str(ruta.relative_to(raiz))], raiz)
    afirmar("'RECOMENDAR' pasa con separación y confirmación independientes",
            hecho.returncode == 0, hecho.stdout)

def prueba_correccion_bonferroni():
    print("\ncorrección Bonferroni")
    resultado = bonferroni([("a", 0.01), ("b", 0.20), ("c", 0.60)])
    afirmar("multiplica por el tamaño de la familia y acota en 1",
            all(abs(resultado[clave] - esperado) < 1e-12
                for clave, esperado in {"a": 0.03, "b": 0.6, "c": 1.0}.items()), str(resultado))

def prueba_strict(raiz):
    print("\nmodo estricto: valida claims, no números sueltos")
    reporte = raiz / "V" / "reporte.md"
    ledger = raiz / "V" / "ledger.jsonl"
    claims = raiz / "V" / "claims_seguir.json"
    hecho = correr([AQUI / "verificar_reporte.py", str(reporte), str(ledger), "--strict",
                    "--claims-json", str(claims)], raiz)
    afirmar("strict permite números que solo generan advertencias",
            hecho.returncode == 0 and "advertencia" in hecho.stdout, hecho.stdout)
    hecho = correr([AQUI / "verificar_reporte.py", str(reporte), str(ledger), "--strict"], raiz)
    afirmar("strict exige claims-json", hecho.returncode == 2, hecho.stdout)

def diagnostico(raiz, candidato, decision, iteracion=1, via=None):
    orden = [AQUI / "ledger.py", "diagnostico", "EXP/ledger.jsonl", "--contrato", "EXP/contrato.md",
             "--iteracion", str(iteracion), "--candidato", candidato,
             "--veredicto", "prediccion cumplida: +0.09 sobre la base", "--decision", decision]
    if via:
        orden += ["--via", str(via), "--justificacion", "medicion valida y codigo intacto"]
    return correr(orden, raiz)

def prueba_diagnostico(raiz):
    print("\ninterpretación: la ronda siguiente no arranca sin diagnosticar la anterior")
    exp = montar(raiz, 0.09)
    correr([AQUI / "ronda.py", "EXP/ronda.json", "--exp", "EXP"], raiz)
    ronda2 = json.loads((exp / "ronda.json").read_text(encoding="utf-8"))
    ronda2["iteracion"] = 2
    (exp / "ronda2.json").write_text(json.dumps(ronda2), encoding="utf-8")

    hecho = correr([AQUI / "ronda.py", "EXP/ronda2.json", "--exp", "EXP"], raiz)
    afirmar("la ronda 2 se bloquea con el diagnóstico pendiente",
            hecho.returncode == 1 and "sin_diagnostico=[it1/alto,it1/base]" in hecho.stdout, hecho.stdout)

    hecho = diagnostico(raiz, "fantasma", "muere")
    afirmar("rechaza un diagnóstico sin resultado previo", hecho.returncode == 2, hecho.stderr)

    diagnostico(raiz, "base", "muere", via=3)
    hecho = correr([AQUI / "ronda.py", "EXP/ronda2.json", "--exp", "EXP"], raiz)
    afirmar("sigue bloqueada y nombra solo al que falta",
            "sin_diagnostico=[it1/alto]" in hecho.stdout, hecho.stdout)

    hecho = diagnostico(raiz, "base", "sigue")
    afirmar("rechaza un segundo diagnóstico del mismo resultado", hecho.returncode == 2, hecho.stderr)

    diagnostico(raiz, "alto", "sigue")
    hecho = correr([AQUI / "ronda.py", "EXP/ronda2.json", "--exp", "EXP"], raiz)
    afirmar("con todo diagnosticado la ronda 2 corre", hecho.returncode == 0, hecho.stdout)
    hecho = correr([AQUI / "ledger.py", "check", "EXP/ledger.jsonl", "EXP/contrato.md"], raiz)
    afirmar("el ledger con diagnósticos valida", hecho.returncode == 0, hecho.stdout)

def prueba_contrato(raiz):
    print("\ncontrato: campos ejecutables, no prosa")
    (raiz / "c.md").write_text(CONTRATO, encoding="utf-8")
    (raiz / "sin_presupuesto.md").write_text(
        "\n".join(l for l in CONTRATO.splitlines() if not l.startswith("max_")), encoding="utf-8")
    hecho = correr([AQUI / "verificar_contrato.py", "c.md"], raiz)
    afirmar("acepta un contrato completo", hecho.returncode == 0, hecho.stdout)
    hecho = correr([AQUI / "verificar_contrato.py", "sin_presupuesto.md"], raiz)
    afirmar("rechaza un contrato sin límites de presupuesto", hecho.returncode == 1, hecho.stdout)

def main():
    print("autoprueba del arnés de prueba-y-error")
    with tempfile.TemporaryDirectory() as tmp:
        raiz = Path(tmp)
        exp = prueba_flujo(raiz)
        prueba_manipulacion(exp)
        prueba_costos(exp)
        prueba_veredictos(raiz)
        prueba_strict(raiz)
        prueba_contrato(raiz)
    with tempfile.TemporaryDirectory() as tmp:
        prueba_nulo(Path(tmp))
    with tempfile.TemporaryDirectory() as tmp:
        prueba_diagnostico(Path(tmp))
    prueba_correccion_bonferroni()
    print()
    if FALLOS:
        for fallo in FALLOS:
            print(f"error: {fallo[:200]}")
        print(f"\n{len(FALLOS)} comprobación(es) fallaron")
        return 1
    print("todas las comprobaciones pasaron")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
