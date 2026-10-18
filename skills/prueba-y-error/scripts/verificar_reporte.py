#!/usr/bin/env python3
"""Advierte por números del reporte sin respaldo y valida los claims del cierre.

Los números sueltos del texto son una alarma para revisión humana, nunca un gate:
comparar tokens contra el ledger da falsos positivos y falsos negativos por igual.
Lo que `--strict` exige y verifica son los claims estructurados: cada cifra se
coteja contra el análisis en disco que citan los resultados, y en modo estricto
ese análisis se recalcula desde los crudos.
"""

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

AQUI = Path(__file__).resolve().parent
sys.path.insert(0, str(AQUI))
from parte import numero_finito  # noqa: E402

NUMERO = re.compile(r"(?<![\w.])[-+]?\d+(?:[.,]\d+)?%?")

def numeros(texto):
    return {token.replace(",", ".").rstrip("%") for token in NUMERO.findall(texto)}

def cargar_registros(texto):
    registros = []
    for numero, linea in enumerate(texto.splitlines(), 1):
        if linea.strip():
            try:
                registros.append(json.loads(linea))
            except json.JSONDecodeError as exc:
                raise ValueError(f"ledger línea {numero}: JSON inválido: {exc.msg}") from exc
    return registros

def cargar_analisis(ledger_dir, ruta):
    try:
        datos = (ledger_dir / ruta).read_bytes()
    except OSError as exc:
        raise ValueError(f"no se pudo leer el análisis '{ruta}': {exc}") from exc
    return hashlib.sha256(datos).hexdigest(), json.loads(datos)

def exigir_mismo_analisis(digest, ruta, resultados):
    for resultado in resultados:
        if resultado.get("sha256_analisis") != digest:
            raise ValueError(f"el análisis '{ruta}' no tiene el sha256 que registró el resultado "
                             f"de '{resultado.get('candidato')}' (it{resultado.get('iteracion')})")

def iguales(a, b, tolerancia):
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(iguales(x, y, tolerancia) for x, y in zip(a, b))
    return numero_finito(a) and numero_finito(b) and abs(a - b) <= tolerancia

def exigir_igual(claim, comparacion, campo, ruta):
    if not iguales(claim.get(campo), comparacion.get(campo), 1e-12):
        raise ValueError(f"{campo} del claim ({claim.get(campo)}) no coincide con el análisis "
                         f"'{ruta}' ({comparacion.get(campo)})")

CAMPOS_RECALCULO = ("efecto", "intervalo_efecto", "p", "p_ajustado")

def recalcular(ledger_dir, ruta, analisis, registros):
    """Vuelve a correr analizar.py sobre los crudos que citan los resultados de ese análisis.

    El sha256 ata el ledger al archivo, no el archivo a los datos: quien edita el
    análisis y la línea del ledger a la par deja ambos coherentes. Lo único que
    no puede acomodar es el crudo, y el análisis es determinista por semilla.
    """
    crudos = []
    for registro in registros:
        if registro.get("tipo") == "resultado" and registro.get("analisis", {}).get("ruta") == ruta:
            crudo = ledger_dir / registro.get("crudo", "")
            if not registro.get("crudo") or not crudo.is_file():
                raise ValueError(f"falta el crudo '{registro.get('crudo')}' del resultado de "
                                 f"'{registro.get('candidato')}' (it{registro.get('iteracion')}); "
                                 f"el análisis '{ruta}' no se puede recalcular")
            if crudo not in crudos:
                crudos.append(crudo)
    with tempfile.TemporaryDirectory() as temporal:
        salida = Path(temporal) / "analisis.json"
        hecho = subprocess.run([sys.executable, str(AQUI / "analizar.py"), *map(str, crudos),
                                "--metodo", str(analisis.get("metodo")), "--semilla", str(analisis.get("semilla")),
                                "--correccion", str(analisis.get("correccion")), "--alpha", str(analisis.get("alpha")),
                                "--umbral", str(analisis.get("separacion", {}).get("umbral")),
                                "--remuestreos", str(analisis.get("remuestreos")),
                                "--confianza", str(analisis.get("confianza")),
                                "--sentido", str(analisis.get("sentido")), "--salida", str(salida)],
                               capture_output=True, text=True)
        if hecho.returncode != 0:
            raise ValueError(f"no se pudo recalcular el análisis '{ruta}': {hecho.stdout.strip()[:300]}")
        recalculado = json.loads(salida.read_text(encoding="utf-8"))
    metricas, metricas_esperadas = analisis.get("metricas", {}), recalculado["metricas"]
    comparaciones = {c.get("clave"): c for c in analisis.get("comparaciones", [])}
    comparaciones_esperadas = {c["clave"]: c for c in recalculado["comparaciones"]}
    reproducido = (
        set(metricas) == set(metricas_esperadas)
        and all(metricas[c].get("n") == metricas_esperadas[c]["n"]
                and iguales(metricas[c].get("valor"), metricas_esperadas[c]["valor"], 1e-9)
                and iguales(metricas[c].get("intervalo"), metricas_esperadas[c]["intervalo"], 1e-9)
                for c in metricas_esperadas)
        and set(comparaciones) == set(comparaciones_esperadas)
        and all(iguales(comparaciones[clave].get(campo), comparaciones_esperadas[clave][campo], 1e-9)
                for clave in comparaciones_esperadas for campo in CAMPOS_RECALCULO))
    if not reproducido:
        raise ValueError(f"el análisis '{ruta}' en disco no se reproduce desde los crudos")

def verificar_claims(ruta, registros, ledger_dir, strict):
    claims = json.loads(ruta.read_text(encoding="utf-8"))
    if not isinstance(claims, dict) or claims.get("decision") not in {"RECOMENDAR", "SEGUIR MIDIENDO", "ABANDONAR"}:
        raise ValueError("claims debe declarar una decisión válida")
    recomendar = claims["decision"] == "RECOMENDAR"
    por_clave = {}
    for registro in registros:
        if registro.get("tipo") == "resultado":
            clave = (registro.get("iteracion"), registro.get("candidato"), registro.get("hash_contrato"))
            if clave in por_clave:
                raise ValueError("hay resultados duplicados para una referencia de claim")
            por_clave[clave] = registro
    comparaciones = claims.get("comparaciones", [])
    if not isinstance(comparaciones, list) or not comparaciones:
        raise ValueError("claims debe incluir comparaciones")
    analisis_por_ruta = {}
    for comparacion in comparaciones:
        mejor_ref = comparacion.get("mejor", {})
        segundo_ref = comparacion.get("segundo", {})
        if (not isinstance(mejor_ref, dict) or not isinstance(segundo_ref, dict)
                or not mejor_ref.get("hash_contrato") or not segundo_ref.get("hash_contrato")):
            raise ValueError("cada referencia debe incluir hash_contrato")
        mejor = por_clave.get((mejor_ref.get("iteracion"), mejor_ref.get("candidato"), mejor_ref.get("hash_contrato")))
        segundo = por_clave.get((segundo_ref.get("iteracion"), segundo_ref.get("candidato"), segundo_ref.get("hash_contrato")))
        if not mejor or not segundo:
            raise ValueError("comparación referencia un resultado inexistente")
        ruta_analisis = mejor.get("analisis", {}).get("ruta")
        if not ruta_analisis or ruta_analisis != segundo.get("analisis", {}).get("ruta"):
            raise ValueError(f"mejor y segundo citan análisis distintos ('{ruta_analisis}' y "
                             f"'{segundo.get('analisis', {}).get('ruta')}'); una comparación sale de un solo análisis")
        if ruta_analisis not in analisis_por_ruta:
            analisis_por_ruta[ruta_analisis] = cargar_analisis(ledger_dir, ruta_analisis)
        digest, analisis = analisis_por_ruta[ruta_analisis]
        exigir_mismo_analisis(digest, ruta_analisis, [mejor, segundo])
        nombre_mejor, nombre_segundo = mejor["candidato"], segundo["candidato"]
        medida = next((c for c in analisis.get("comparaciones", [])
                       if c.get("clave") == f"{nombre_mejor}::{nombre_segundo}"), None)
        if medida is None:
            raise ValueError(f"el análisis '{ruta_analisis}' no tiene a '{nombre_mejor}' por encima de '{nombre_segundo}'")
        # las cifras del claim no se validan por su forma sino contra el análisis
        # que las produjo: el agente las copia, el verificador las coteja.
        exigir_igual(comparacion, medida, "efecto", ruta_analisis)
        for campo in ("intervalo_efecto", "p_ajustado"):
            if recomendar and campo not in comparacion:
                raise ValueError(f"RECOMENDAR exige {campo} en cada comparación")
            if campo in comparacion:
                exigir_igual(comparacion, medida, campo, ruta_analisis)
        intervalo = comparacion.get("intervalo")
        if intervalo != mejor["metrica"]["intervalo"]:
            raise ValueError("intervalo no coincide con el mejor resultado")
        intervalo_segundo = segundo["metrica"]["intervalo"]
        if intervalo[0] > intervalo[1] or intervalo_segundo[0] > intervalo_segundo[1]:
            raise ValueError("los intervalos deben estar ordenados")
        umbral = comparacion.get("umbral")
        if not iguales(umbral, analisis.get("separacion", {}).get("umbral"), 1e-12):
            raise ValueError(f"umbral del claim ({umbral}) no coincide con el del análisis "
                             f"'{ruta_analisis}' ({analisis.get('separacion', {}).get('umbral')})")
        if recomendar or "alpha" in claims:
            if not iguales(claims.get("alpha"), analisis.get("alpha"), 1e-12):
                raise ValueError(f"alpha del claim ({claims.get('alpha')}) no coincide con el del análisis "
                                 f"'{ruta_analisis}' ({analisis.get('alpha')})")
        # La separación se exige solo para RECOMENDAR: SEGUIR MIDIENDO y ABANDONAR
        # son precisamente los veredictos que describen su ausencia, y bloquearlos
        # haría inexpresable el resultado honesto. El criterio es el intervalo de
        # la DIFERENCIA, no la disjunción de los marginales, que en diseño pareado
        # arrastra la variación entre casos que el pareo cancela.
        if recomendar:
            efecto_ic = medida["intervalo_efecto"]
            if not (efecto_ic[0] > umbral or efecto_ic[1] < -umbral):
                raise ValueError("RECOMENDAR exige que el intervalo del efecto excluya el umbral")
            if medida["p_ajustado"] >= analisis["alpha"]:
                raise ValueError(f"RECOMENDAR exige p ajustado < alpha ({medida['p_ajustado']} >= {analisis['alpha']})")
            for registro in (mejor, segundo):
                if registro.get("fase") != "B":
                    raise ValueError(f"RECOMENDAR exige comparaciones de fase B; el resultado de "
                                     f"'{registro.get('candidato')}' (it{registro.get('iteracion')}) "
                                     f"es de fase {registro.get('fase')}")
    if recomendar:
        if claims.get("confirmacion_independiente") is not True:
            raise ValueError("RECOMENDAR exige confirmacion_independiente=true")
        fuentes_fase = {
            fase: {fuente for registro in registros if registro.get("tipo") == "resultado" and registro.get("fase") == fase for fuente in registro.get("fuentes", [])}
            for fase in ("A", "B")
        }
        if not fuentes_fase["A"]:
            raise ValueError("RECOMENDAR exige resultados de fase A: sin tamizaje no hay nada que la fase B confirme")
        if fuentes_fase["A"] & fuentes_fase["B"]:
            raise ValueError("las fuentes de fase A y B no son disjuntas")
        if not claims.get("correccion_comparaciones"):
            raise ValueError("RECOMENDAR exige declarar la corrección por comparaciones")
    if strict:
        for ruta_analisis, (_, analisis) in analisis_por_ruta.items():
            recalcular(ledger_dir, ruta_analisis, analisis, registros)
    return claims

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("ledger", type=Path)
    parser.add_argument("--strict", action="store_true",
                        help="exige y valida --claims-json; los números sueltos solo advierten")
    parser.add_argument("--claims-json", type=Path,
                        help="claims estructurados; obligatorio con --strict")
    args = parser.parse_args()
    try:
        ledger_text = args.ledger.read_text(encoding="utf-8")
        report_text = args.report.read_text(encoding="utf-8")
    except OSError as exc:
        print(f"error: {exc}")
        return 2
    try:
        registros = cargar_registros(ledger_text)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"error: {exc}")
        return 2
    if args.strict and not args.claims_json:
        print("error: --strict exige --claims-json")
        return 2
    if args.claims_json:
        try:
            verificar_claims(args.claims_json, registros, args.ledger.resolve().parent, args.strict)
        except (OSError, ValueError, json.JSONDecodeError) as exc:
            print(f"error: claims inválidos: {exc}")
            return 1
    faltantes = sorted(numeros(report_text) - numeros(ledger_text), key=lambda valor: (float(valor), valor))
    for valor in faltantes:
        print(f"advertencia: el número {valor} del reporte no aparece en el ledger")
    cierre = "claims validados" if args.strict else "claims opcionales"
    print(f"OK: reporte revisado; {len(faltantes)} advertencia(s), {cierre}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
