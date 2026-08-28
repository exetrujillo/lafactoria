#!/usr/bin/env python3
"""Recalcula métrica, intervalo y corrección múltiple desde el crudo.

Mientras el intervalo llegue ya calculado, nada distingue una cifra
medida de una plausible. Acá el número sale de las observaciones o no sale.
Sin dependencias externas.
"""

import argparse
import hashlib
import json
import math
import random
import statistics
import sys
from pathlib import Path

METODOS = ("bootstrap_pareado",)
sys.path.insert(0, str(Path(__file__).resolve().parent))
from parte import emitir, numero_finito


def cargar_crudo(ruta):
    """raw canónico: JSONL con {candidato, caso, valor} por observación."""
    observaciones = []
    for numero, linea in enumerate(ruta.read_text(encoding="utf-8").splitlines(), 1):
        if not linea.strip():
            continue
        try:
            fila = json.loads(linea)
        except json.JSONDecodeError as exc:
            raise ValueError(f"crudo línea {numero}: JSON inválido: {exc.msg}") from exc
        if not isinstance(fila, dict):
            raise ValueError(f"crudo línea {numero}: la observación debe ser un objeto")
        for campo in ("candidato", "caso"):
            if not isinstance(fila.get(campo), str) or not fila[campo]:
                raise ValueError(f"crudo línea {numero}: falta '{campo}'")
        if not numero_finito(fila.get("valor")):
            raise ValueError(f"crudo línea {numero}: 'valor' debe ser un número finito")
        observaciones.append(fila)
    if not observaciones:
        raise ValueError("el crudo no contiene observaciones")
    return observaciones

def agrupar(observaciones):
    por_candidato = {}
    for fila in observaciones:
        por_candidato.setdefault(fila["candidato"], {})
        caso = fila["caso"]
        if caso in por_candidato[fila["candidato"]]:
            raise ValueError(f"caso duplicado '{caso}' para el candidato '{fila['candidato']}'")
        por_candidato[fila["candidato"]][caso] = fila["valor"]
    return por_candidato

def casos_comunes(por_candidato):
    conjuntos = [set(casos) for casos in por_candidato.values()]
    comunes = set.intersection(*conjuntos) if conjuntos else set()
    if not comunes:
        raise ValueError("los candidatos no comparten ningún caso: el diseño no es pareado")
    return sorted(comunes)

def intervalo_bootstrap(valores, semilla, remuestreos, confianza):
    """Percentil sobre remuestreos con reemplazo. Determinista dada la semilla."""
    if len(valores) < 2:
        raise ValueError("el intervalo exige al menos dos observaciones")
    generador = random.Random(semilla)
    medias = []
    n = len(valores)
    for _ in range(remuestreos):
        medias.append(sum(valores[generador.randrange(n)] for _ in range(n)) / n)
    medias.sort()
    alfa = (1 - confianza) / 2
    bajo = medias[max(0, int(math.floor(alfa * remuestreos)))]
    alto = medias[min(remuestreos - 1, int(math.ceil((1 - alfa) * remuestreos)) - 1)]
    return [bajo, alto]

def p_permutacion(diferencias, semilla, permutaciones):
    """Permutación pareada de signos: bajo H0 el signo de cada par es simétrico."""
    observado = abs(statistics.fmean(diferencias))
    generador = random.Random(semilla)
    extremos = 0
    for _ in range(permutaciones):
        simulado = abs(statistics.fmean([d if generador.random() < 0.5 else -d for d in diferencias]))
        if simulado >= observado:
            extremos += 1
    # corrección de continuidad
    # un p nunca es exactamente cero con n finito.
    return (extremos + 1) / (permutaciones + 1)

def holm(pares_p):
    """Holm-Bonferroni: controla la tasa de error por familia sin suponer independencia."""
    ordenados = sorted(pares_p, key=lambda item: item[1])
    total = len(ordenados)
    ajustados = {}
    maximo = 0.0
    for indice, (clave, valor) in enumerate(ordenados):
        candidato = min(1.0, (total - indice) * valor)
        maximo = max(maximo, candidato)
        ajustados[clave] = maximo
    return ajustados

def bonferroni(pares_p):
    total = len(pares_p)
    return {clave: min(1.0, total * valor) for clave, valor in pares_p}

CORRECCIONES = {"holm": holm, "bonferroni": bonferroni}

def analizar(observaciones, metodo, semilla, remuestreos, confianza, correccion, alfa):
    por_candidato = agrupar(observaciones)
    if len(por_candidato) < 2:
        raise ValueError("se necesitan al menos dos candidatos para comparar")
    comunes = casos_comunes(por_candidato)
    metricas = {}
    for candidato, casos in sorted(por_candidato.items()):
        valores = [casos[caso] for caso in comunes]
        intervalo = intervalo_bootstrap(valores, f"{semilla}:{candidato}", remuestreos, confianza)
        metricas[candidato] = {
            "valor": statistics.fmean(valores),
            "intervalo": intervalo,
            "n": len(valores),
        }
    orden = sorted(metricas, key=lambda nombre: metricas[nombre]["valor"], reverse=True)
    crudos_p = []
    comparaciones = []
    for indice, uno in enumerate(orden):
        for otro in orden[indice + 1:]:
            diferencias = [por_candidato[uno][caso] - por_candidato[otro][caso] for caso in comunes]
            clave = f"{uno}::{otro}"
            valor_p = p_permutacion(diferencias, f"{semilla}:{clave}", remuestreos)
            crudos_p.append((clave, valor_p))
            intervalo = intervalo_bootstrap(diferencias, f"{semilla}:d:{clave}", remuestreos, confianza)
            comparaciones.append({
                "clave": clave, "mejor": uno, "segundo": otro,
                "efecto": statistics.fmean(diferencias),
                "intervalo_efecto": intervalo,
                "p": valor_p,
            })
    ajustados = CORRECCIONES[correccion](crudos_p)
    for comparacion in comparaciones:
        comparacion["p_ajustado"] = ajustados[comparacion["clave"]]
        comparacion["significativa"] = comparacion["p_ajustado"] < alfa
    return {"metricas": metricas, "orden": orden, "comparaciones": comparaciones,
            "familia": sorted(ajustados), "alpha": alfa, "correccion": correccion,
            "metodo": metodo, "semilla": semilla, "confianza": confianza,
            "remuestreos": remuestreos, "casos": len(comunes)}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("crudo", type=Path, nargs="+",
                        help="uno por candidato; el análisis es conjunto y pareado")
    parser.add_argument("--metodo", choices=METODOS, required=True)
    parser.add_argument("--semilla", required=True)
    parser.add_argument("--confianza", type=float, default=0.95)
    parser.add_argument("--remuestreos", type=int, default=10000)
    parser.add_argument("--correccion", choices=sorted(CORRECCIONES), required=True)
    parser.add_argument("--alpha", type=float, default=0.05)
    parser.add_argument("--umbral", type=float, required=True)
    parser.add_argument("--salida", type=Path, required=True)
    args = parser.parse_args()
    try:
        if not 0 < args.confianza < 1:
            raise ValueError("--confianza debe estar entre 0 y 1")
        if not 0 < args.alpha < 1:
            raise ValueError("--alpha debe estar entre 0 y 1")
        if not numero_finito(args.umbral) or args.umbral < 0:
            raise ValueError("--umbral debe ser finito y no negativo")
        if args.remuestreos < 100:
            raise ValueError("--remuestreos debe ser al menos 100")
        observaciones = []
        for ruta in args.crudo:
            observaciones.extend(cargar_crudo(ruta))
        analisis = analizar(observaciones, args.metodo, args.semilla, args.remuestreos,
                            args.confianza, args.correccion, args.alpha)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(emitir("analisis", "error", {"motivo": exc}))
        return 2
    # un hash por crudo permite que cada resultado del ledger cite el suyo.
    analisis["sha256_por_crudo"] = {
        str(ruta): hashlib.sha256(ruta.read_bytes()).hexdigest() for ruta in args.crudo}
    primero = analisis["orden"][0]
    top = next(c for c in analisis["comparaciones"] if c["mejor"] == primero)
    # en un diseño pareado los IC marginales arrastran la variación entre casos,
    # que el pareo justamente cancela: exigir que no se solapen rechaza efectos
    # reales. La separación se decide sobre el intervalo de la diferencia.
    bajo, alto = top["intervalo_efecto"]
    separa = bajo > args.umbral or alto < -args.umbral
    analisis["separacion"] = {"criterio": "IC del efecto excluye el umbral",
                              "umbral": args.umbral, "separa": separa}
    args.salida.parent.mkdir(parents=True, exist_ok=True)
    args.salida.write_text(json.dumps(analisis, ensure_ascii=False, indent=1), encoding="utf-8")
    print(emitir("analisis", "ok", {
        "candidatos": len(analisis["metricas"]),
        "casos": analisis["casos"],
        "mejor": primero,
        "metrica": analisis["metricas"][primero]["valor"],
        "ic": analisis["metricas"][primero]["intervalo"],
        "segundo": top["segundo"],
        "efecto": top["efecto"],
        "ic_efecto": top["intervalo_efecto"],
        "umbral": args.umbral,
        "supera_umbral": abs(top["efecto"]) >= args.umbral,
        "separa": separa,
        "p_ajustado": top["p_ajustado"],
        "correccion": f"{args.correccion}(familia={len(analisis['familia'])},alpha={args.alpha})",
        "significativa": top["significativa"],
    }, detalle=str(args.salida)))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
