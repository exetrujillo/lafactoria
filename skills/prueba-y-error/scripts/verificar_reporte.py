#!/usr/bin/env python3
"""Advierte por números del reporte sin respaldo y valida los claims del cierre.

Los números sueltos del texto son una alarma para revisión humana, nunca un gate:
comparar tokens contra el ledger da falsos positivos y falsos negativos por igual.
Lo que `--strict` exige y verifica son los claims estructurados, que sí se
recomputan contra los resultados registrados.
"""

import argparse
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
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

def verificar_claims(ruta, registros):
    claims = json.loads(ruta.read_text(encoding="utf-8"))
    if not isinstance(claims, dict) or claims.get("decision") not in {"RECOMENDAR", "SEGUIR MIDIENDO", "ABANDONAR"}:
        raise ValueError("claims debe declarar una decisión válida")
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
        efecto = comparacion.get("efecto")
        esperado = mejor["metrica"]["valor"] - segundo["metrica"]["valor"]
        if not numero_finito(efecto) or abs(efecto - esperado) > 1e-12:
            raise ValueError("efecto no coincide con las métricas del ledger")
        intervalo = comparacion.get("intervalo")
        if intervalo != mejor["metrica"]["intervalo"]:
            raise ValueError("intervalo no coincide con el mejor resultado")
        intervalo_segundo = segundo["metrica"]["intervalo"]
        if intervalo[0] > intervalo[1] or intervalo_segundo[0] > intervalo_segundo[1]:
            raise ValueError("los intervalos deben estar ordenados")
        umbral = comparacion.get("umbral")
        if not numero_finito(umbral) or umbral < 0:
            raise ValueError("umbral debe ser un número finito no negativo")
        # La separación se exige solo para RECOMENDAR: SEGUIR MIDIENDO y ABANDONAR
        # son precisamente los veredictos que describen su ausencia, y bloquearlos
        # haría inexpresable el resultado honesto. El criterio es el intervalo de
        # la DIFERENCIA, no la disjunción de los marginales, que en diseño pareado
        # arrastra la variación entre casos que el pareo cancela.
        if claims["decision"] == "RECOMENDAR":
            efecto_ic = comparacion.get("intervalo_efecto")
            if (not isinstance(efecto_ic, list) or len(efecto_ic) != 2
                    or not all(numero_finito(v) for v in efecto_ic)):
                raise ValueError("RECOMENDAR exige intervalo_efecto con dos números finitos")
            if efecto_ic[0] > efecto_ic[1]:
                raise ValueError("intervalo_efecto debe estar ordenado")
            if not (efecto_ic[0] > umbral or efecto_ic[1] < -umbral):
                raise ValueError("RECOMENDAR exige que el intervalo del efecto excluya el umbral")
            p_ajustado = comparacion.get("p_ajustado")
            alpha = claims.get("alpha")
            if not all(numero_finito(v) and 0 < v < 1 for v in (p_ajustado, alpha)):
                raise ValueError("RECOMENDAR exige p_ajustado y alpha en (0,1)")
            if p_ajustado >= alpha:
                raise ValueError(f"RECOMENDAR exige p ajustado < alpha ({p_ajustado} >= {alpha})")
    if claims["decision"] == "RECOMENDAR":
        if claims.get("confirmacion_independiente") is not True:
            raise ValueError("RECOMENDAR exige confirmacion_independiente=true")
        fuentes_fase = {
            fase: {fuente for registro in registros if registro.get("tipo") == "resultado" and registro.get("fase") == fase for fuente in registro.get("fuentes", [])}
            for fase in ("A", "B")
        }
        if fuentes_fase["A"] & fuentes_fase["B"]:
            raise ValueError("las fuentes de fase A y B no son disjuntas")
        if not claims.get("correccion_comparaciones"):
            raise ValueError("RECOMENDAR exige declarar la corrección por comparaciones")
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
            verificar_claims(args.claims_json, registros)
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
