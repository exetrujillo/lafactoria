# Señales de que el bucle se soltó

Auditar contra esta lista al terminar un experimento. Cada punto marcado es un
invariante que el modelo no sostuvo por sí solo:

- Una línea del ledger sin predicción, o con la predicción escrita después del dato.
- El test aplicado no es el que estaba declarado antes de correr.
- Un dato descartado sin que el triage diga por cuál de las tres vías.
- El reporte final cita un número que no aparece en el ledger.
- El banco cambió en la misma iteración que el instrumento.
- Se pasó a fase B sin aplicar la regla pre-declarada del contrato.
- Se pidió confirmación humana entre fases aunque el contrato había delegado la
  transición en la política.
- Se terminó una fase sin seleccionar, iterar o emitir una decisión según el
  criterio de parada.
- Se escribió documentación definitiva antes del informe provisional y su
  auditoría.
- Un rango de repeticiones deterministas se presentó como intervalo de
  incertidumbre.
- La identidad de un caso no coincide entre banco, ledger, crudos y reporte.
- Rondas con instrumentos distintos se agregaron como si fueran comparables.
- Un chequeo de robustez que por álgebra no podía cambiar el resultado se contó
  como si lo respaldara.
