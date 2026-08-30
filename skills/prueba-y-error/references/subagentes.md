# Ejecución aislada mediante agentes

Cuando una ronda necesite trabajo que `ronda.py` no cubra, envíala a un subagente
de bajo contexto. El agente principal conserva la orquestación, pero no vuelca al
contexto el crudo ni ejecuta manualmente lotes mecánicos. El encargo debe incluir:

- el directorio del experimento y el contrato congelado;
- el banco y la política de selección ya declarada;
- el presupuesto restante y los comandos exactos de validación;
- el manifiesto `fuentes.json` con IDs de casos o consultas, igual para plan y resultado;
- la prohibición de modificar `skills/biblio-rata/`, hacer commits o cambiar el
  contrato/banco después de la congelación;
- la instrucción de escribir planes antes de correr, resultados mediante
  `ledger.py`, crudos y resumen en disco.

El subagente puede crear fixtures reproducibles, ejecutar el protocolo, validar el
ledger y redactar el informe solicitado. Si descubre un fallo, debe detenerse en
la primera vía de triage aplicable y devolverlo sin arreglar silenciosamente una
premisa congelada. Una corrección de instrumento abre contrato y ronda nuevos.

El subagente devuelve al agente principal solo un parte breve: estado, decisión,
ruta del informe, hash de contrato y banco, conteo de registros/evaluaciones,
fallos de validez, comandos ejecutados y archivos modificados. El agente principal
lee el informe y el ledger desde disco para decidir la continuidad; no reconstruye
resultados desde la memoria del subagente ni pide el crudo salvo para auditar un
fallo concreto.

Si el parte dice **SEGUIR MIDIENDO** y el valor de información no es bajo, el
agente principal debe lanzar otro subagente con un encargo nuevo y acotado,
incluyendo el hallazgo anterior y la observación que falta. No debe devolver el
control al usuario entre subagentes ni considerar el primer parte un cierre.

Si hay fases o casos independientes, lánzalos en subagentes separados solo cuando
no compartan archivos de escritura. Si comparten ledger, corpus o presupuesto,
usa un único subagente secuencial para evitar carreras y registros intercalados.

Cada subagente ejecuta como máximo una ronda, pero la orquestación puede encadenar
tantas rondas como permita el presupuesto global. Antes de relanzar, registra por
qué la ronda anterior no decidió, qué dato nuevo busca y qué condición hará parar.
Si el diseño siguiente sería equivalente o no podría cambiar el veredicto, cierra
como `SEGUIR MIDIENDO` por valor de información bajo y explica qué experimento
futuro, con contrato distinto, sería necesario.

Un ciclo puede corregir un instrumento o un error de ejecución, pero no puede
editar retrospectivamente contrato, banco, plan ni resultado. Una corrección
abre un linaje o una iteración nueva y deja el diagnóstico en el ledger.
