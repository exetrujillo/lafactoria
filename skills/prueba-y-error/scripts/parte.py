#!/usr/bin/env python3
"""Parte acotado: lo único que un nodo devuelve al orquestador.

El costo de contexto de un nodo no es lo que calcula, sino lo que imprime:
cada byte que entra al orquestador se relee en todos sus turnos siguientes.
Por eso el tope es duro y el detalle vive en disco.

Aloja además lo que comparten todos los nodos y no puede divergir entre ellos:
las dimensiones del costo y el predicado de número finito.
"""

import math

TOPE_BYTES = 2048
DIMENSIONES_COSTO = ("corridas", "segundos", "consultas_banco", "rondas", "consultas_adaptativas")

def numero_finito(valor):
    return isinstance(valor, (int, float)) and not isinstance(valor, bool) and math.isfinite(valor)

def emitir(nodo, estado, campos, detalle=None):
    """Devuelve el parte como texto, truncado al tope y con puntero al detalle."""
    if estado not in {"ok", "bloqueado", "error"}:
        raise ValueError(f"estado inválido: {estado}")
    lineas = [f"PARTE nodo={nodo} estado={estado}"]
    for clave, valor in campos.items():
        if valor is None:
            continue
        if isinstance(valor, (list, tuple)):
            valor = "[" + ",".join(formatear(item) for item in valor) + "]"
        else:
            valor = formatear(valor)
        lineas.append(f"{clave}={valor}")
    if detalle:
        lineas.append(f"detalle={detalle}")
    texto = "\n".join(lineas)
    if len(texto.encode("utf-8")) > TOPE_BYTES:
        recorte = texto.encode("utf-8")[:TOPE_BYTES - 80].decode("utf-8", "ignore")
        aviso = f"\n... parte truncado en {TOPE_BYTES} bytes; leer {detalle or 'el directorio del experimento'}"
        texto = recorte + aviso
    return texto

def formatear(valor):
    if isinstance(valor, float):
        # seis decimales bastan para decidir y evitan volcar ruido de coma flotante.
        return f"{valor:.6g}"
    if isinstance(valor, bool):
        return "si" if valor else "no"
    return str(valor)
