# El grafo, nodo por nodo

Los tres diagramas vienen del diseño original del bucle. Debajo de cada uno, el
contrato de los nodos que aparecen: qué recibe, qué produce, **quién lo ejecuta**
y qué escribe en el ledger.

La columna de ejecutores es lo que impide que el LLM se meta donde no va. El
ejecutor no interpreta; el selector no opina.

## 1. El bucle

```
        +-- 0. CONTRATO --------------------------- [ CONGELADO ]
        |   objetivo . métrica primaria . umbral de relevancia
        |   presupuesto . protocolo de control . criterio de parada
        +-+-- restringe a todos los nodos de abajo
          |
          v
   +--> +-- 1. GENERADOR ------------------------------------ (LLM)
   |    |   propone k candidatos
   |    |   cada uno con PREDICCIÓN explícita de qué debería pasar
   |    +-+
   |      |
   |      v
   |    +-- 2. DISEÑADOR ---------------------------- (determinista)
   |    |   semillas pareadas . orden aleatorizado
   |    |   entorno congelado . ANÁLISIS PRE-DECLARADO
   |    +-+
   |      |
   |      v
   |    +-- 3. EJECUTOR --------------------------------- (sin LLM)
   |    |   corrida aislada, reproducible por hash
   |    |   escribe el crudo, NO lo interpreta
   |    +-+
   |      |
   |      v
   |    +-- 4. ANALIZADOR ------------- (test primero, LLM después)
   |    |   aplica el test PRE-DECLARADO en el nodo 2
   |    |   el LLM no ve el crudo hasta que el test ya corrió
   |    +-+-------------------------+
   |      |                         |
   |  predicción                PREDICCIÓN
   |  cumplida                   FALLIDA
   |      |                         |
   |      v                         v
   |    +-- 6. SELECTOR         +-- 5. DIAGNÓSTICO ---- (LLM + delta debugging)
   |    |   política, NO el LLM |   triage obligatorio, en este orden:
   |    |   fase A: halving     |     1) ¿medición inválida? -> DESCARTA el dato
   |    |   fase B: adquisición |     2) ¿código roto?       -> arregla, RE-corre
   |    |   mata o promueve     |     3) hipótesis falsa     -> evidencia legítima
   |    +-+----------+          +-+
   |      |          |            |
   +------+          |            +--> vuelve al bucle por la vía que corresponda
   sigue el bucle    |                 (1 y 2 NO cuentan como evidencia)
                     |
                     | presupuesto agotado / separación lograda
                     v
                   +-- 7. NODO DE SALIDA ------------ (LLM leyendo el ledger)
                   |   comparación múltiple con corrección
                   |   tamaño de efecto + intervalo, no solo el ganador
                   |   -> RECOMENDAR | SEGUIR MIDIENDO | ABANDONAR
                   +--
```

### Contrato de cada nodo

**0. Contrato** · ejecuta: el usuario y el LLM, juntos, una sola vez.
Recibe la pregunta que se quiere responder. Produce `contrato.md` con objetivo,
métrica primaria (una sola), umbral de relevancia, presupuesto (corridas, tiempo,
y consultas al banco), protocolo de control y criterio de parada. Al ledger: el
hash del contrato, que encabeza todas las líneas siguientes. **Se congela**: si
cambia, cambia el hash y empieza otro experimento.

Antes de avanzar, `scripts/verificar_contrato.py` exige encabezados no vacíos y
campos ejecutables para
objetivo, métrica primaria, traducción de la métrica a la decisión, umbral,
presupuesto, control y parada. `scripts/ledger.py` calcula el hash, conserva el
linaje con `hash_contrato_anterior` y `check` impide que un resultado se apoye en
un plan huérfano.

**1. Generador** · ejecuta: el LLM.
Recibe el contrato y el estado del ledger. Produce k candidatos, cada uno con
identificador, descripción y **predicción explícita** (qué magnitud, en qué
dirección, de qué tamaño). Al ledger: una línea por candidato con su predicción,
escrita antes de que exista ningún dato.

**2. Diseñador** · ejecuta: determinista, sin criterio del modelo.
Recibe los k candidatos. Produce el plan de corrida: semillas pareadas, orden
aleatorizado, entorno congelado, número de repeticiones, y el **análisis
pre-declarado** (qué test, con qué umbral, y qué decisión sigue a cada resultado
posible). Al ledger: el diseño y el test declarado.

**3. Ejecutor** · ejecuta: un subagente acotado, usando la máquina sin LLM en el
circuito de la corrida.
Recibe el plan. Produce el crudo en `crudos/`, reproducible por hash del entorno.
No interpreta, no resume, no comenta. Al ledger: la ruta del crudo y el costo real
de la corrida.

**4. Analizador** · ejecuta: el mismo subagente, primero el test y después el LLM
solo para redactar el parte.
Recibe el crudo y el test del nodo 2. Aplica **ese** test —no otro— y escribe el
veredicto. Recién con el veredicto en disco el LLM puede leer el crudo, y solo
para redactar, nunca para reelegir el análisis. Al ledger: métrica con intervalo y
test aplicado en el `resultado`; el veredicto abre el `diagnostico`.

**5. Diagnóstico** · ejecuta: el LLM, con búsqueda binaria sobre los cambios.
Se entra solo si la predicción falló. Recorre el triage en orden y **para en la
primera vía que aplique**: (1) medición inválida → descarta el dato y vuelve al
nodo 3; (2) código roto → arregla y vuelve al nodo 3; (3) hipótesis falsa →
evidencia legítima, sigue al nodo 6. Al ledger: la vía y su justificación, en el
`diagnostico` del candidato. Las vías 1 y 2 no cuentan en el recuento de evidencia.

**6. Selector** · ejecuta: la política, no el LLM.
Recibe los veredictos del lote. Produce qué candidatos mueren y cuáles siguen. En
fase A la regla es halving; en fase B, una función de adquisición. El LLM puede
proponer candidatos nuevos, pero no decidir quién sobrevive. Al ledger: la
decisión, en el `diagnostico`; la regla ya venía declarada en el `plan`.

**7. Nodo de salida** · ejecuta: el LLM leyendo el ledger.
Recibe el `ledger.jsonl` completo, no la conversación. Produce el reporte:
RECOMENDAR, SEGUIR MIDIENDO o ABANDONAR, con tamaño de efecto, intervalo y
corrección por comparación múltiple.

`scripts/verificar_reporte.py --strict --claims-json` valida claims estructurados
contra resultados identificados por linaje: recomputa el efecto y comprueba que
los intervalos estén ordenados. Los requisitos de separación se exigen sólo a
`RECOMENDAR` —intervalo del efecto por encima del umbral, p ajustado bajo el
alfa, fuentes A/B disjuntas y confirmación independiente— porque `SEGUIR
MIDIENDO` y `ABANDONAR` son precisamente los veredictos que reportan su ausencia.
Los números del texto sin respaldo son siempre advertencias, incluso con `--strict`.
El modo estricto solo exige y valida los claims estructurados; el cierre debe usar
claims y modo estricto.

## 2. El ledger

```
   1  2  3  4  5  6  7      <- todos los nodos escriben, append-only
   |  |  |  |  |  |  |
   v  v  v  v  v  v  v
 +-- LEDGER DE PROCEDENCIA -------------- (en disco, fuera del contexto)
 |   plan        : hash_contrato . hipótesis . predicción . diseño . regla
 |   resultado   : crudo . métrica con intervalo . test aplicado . costo
 |   diagnostico : veredicto . vía del triage . decisión del selector
 +-- el reporte del nodo 7 se genera DEL LEDGER, nunca de la memoria
     del agente. Si no está escrito, no ocurrió.
```

El formato exacto está en `ledger.md`, en este mismo directorio.

## 3. Las dos fases

```
  FASE A: TAMIZAJE                  ||  FASE B: CONFIRMACIÓN
  barato . ruidoso . masivo         ||  caro . pareado . repetido
  correr >> razonar                 ||  razonar >> correr
  ................................  ||  ................................
  ~10-100 candidatos                ||  ~2-5 sobrevivientes
  1 repetición, corrida corta       ||  k repeticiones, corrida completa
  el LLM interviene POR LOTES       ||  el LLM interviene en cada corrida
  autónomo                          ||  autónomo
  criterio: matar temprano          ||  criterio: severidad + tamaño de efecto
                                    ||
     o o o o o o o o o o o o        ||         O            O
      \  \  \  |  |  /  /  /        ||         |            |
       o   o   o   o   o            ||         O            O
         \   \ | /   /      +-------+|         |            |
            o    o    o      | REGLA ||         O            O
              \  |  /        | CONTRATO||       |            |
               o o          +-------+|     +---+------------+---+
                                     ||     |  comparación final  |
                                     ||     +---------+-----------+
                                     ||               v
                                     ||          NODO DE SALIDA
```

Cuál de las dos corresponde, y con qué presupuesto, está en `regimenes.md`.
