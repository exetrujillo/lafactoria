---
name: prueba-y-error
description: >
  Usa esta skill solo cuando el usuario la invoque explícitamente con
  `/prueba-y-error` para decidir algo midiendo en vez de opinando: elegir
  entre dos o más variantes, calibrar un número puesto a ojo (un umbral, un tope,
  un tamaño de lote), comprobar si un cambio sirve de verdad, o ante frases como
  "probemos", "¿cuál conviene?", "habría que medirlo". Al invocarse no arranca
  sola: primero le plantea al usuario qué costaría medir y qué alternativas hay, y
  recién con esa decisión corre el bucle de experimentación —contrato congelado,
  predicción previa, análisis pre-declarado, ledger en disco— que separa la
  evidencia de la impresión.
---

# Prueba y error

Un bucle de experimentación para que una decisión técnica deje de apoyarse en la
impresión del modelo. La frase que lo resume: **si no está escrito, no ocurrió**.

## Política de continuidad autónoma

Una vez que el usuario eligió medir, esa autorización cubre el ciclo completo y
las rondas siguientes que el contrato permita. Si el veredicto es
**SEGUIR MIDIENDO**, no vuelvas a preguntar si hay que medir: lee el ledger y el
informe, identifica la observación que falta y diseña la siguiente ronda.

`SEGUIR MIDIENDO` no es el cierre de una invocación mientras queden observaciones
con valor de información. El agente principal debe relanzar un subagente nuevo,
leer su parte y repetir el ciclo. La cadena solo termina al alcanzar un criterio
de parada, agotar el presupuesto, comprobar que la próxima observación no puede
cambiar el veredicto, o recibir `RECOMENDAR` o `ABANDONAR` con respaldo suficiente.

La próxima ronda debe cambiar el diseño de manera informativa, no repetir una
corrida determinista equivalente. En este orden:

1. Busca en el ledger la fuente de incertidumbre: banda sin cobertura, posición de
   la respuesta, fuente insuficiente, consulta no representada o fallo de validez.
2. Elige la intervención mínima que pueda cambiar la decisión: ampliar una banda,
   balancear casos, variar la posición objetivo, añadir consultas independientes o
   corregir el instrumento.
3. Escribe un contrato nuevo si cambia el banco, el instrumento o la pregunta; no
   mezcles su ledger con el linaje anterior. Conserva el informe anterior para
   auditoría.
4. Antes de gastar presupuesto, estima el valor de información: si la próxima
   observación no puede cambiar `RECOMENDAR`, `SEGUIR MIDIENDO` o `ABANDONAR`,
   cierra la pregunta con la decisión vigente en vez de repetirla.

Solo pregunta al usuario cuando el contrato no delegue la elección, cuando haga
falta una decisión irreversible fuera del experimento o cuando el presupuesto
requiera ampliación. Un mensaje del usuario como “sigamos” es autorización
explícita para ejecutar la siguiente ronda delegada.

## 0. La compuerta: fija el experimento, no el siguiente paso

Al invocarse esta skill **no se empieza a experimentar**. El primer acto es una
sola llamada a la herramienta disponible para hacer preguntas que fije el
experimento completo y ponga sobre la mesa:

- **Qué se mediría** exactamente: la magnitud y su unidad, no "si funciona mejor".
- **Qué costaría**: cuántas corridas, cuánto tiempo, cuánto contexto.
- **Qué se pierde si no se mide**: el costo de equivocarse en esta decisión.

También fija el presupuesto máximo, el criterio de parada y qué intervención
humana sería necesaria, si alguna. No se vuelve a preguntar al pasar de fase:
desde ese momento la skill autoorquesta el bucle y toma las decisiones que el
contrato haya delegado en la política.

Si todavía no está claro si vale la pena medir, ofrece tres opciones reales, cada
una con su contra explícita:

1. **Bucle completo** — ciclos autónomos hasta que opere un criterio de parada.
   Caro; da una conclusión defendible con tamaño de efecto e intervalo.
2. **Versión mínima** — un solo lote de fase A, sin fase B. Barato; sirve para
   **descartar** candidatos, no para recomendar uno.
3. **No medir** — decidir a ojo y dejar anotada la apuesta: el número elegido, por
   qué, y que no está medido. Gratis; la deuda queda registrada para calibrarla
   cuando importe.

Si el usuario elige (3), escribe esa línea donde viva el número y termina ahí. No
hay bucle. Si elige (1) o (2), la respuesta a la compuerta es el contrato de
trabajo, no una autorización para preguntar de nuevo al terminar la fase A.

Esta skill se autoinvoca con un disparador amplio **precisamente** para poder
ofrecer esta decisión. Saltearse la compuerta y arrancar a experimentar por
cuenta propia es el error más caro que puede cometer, porque gasta el presupuesto
del usuario en una pregunta que quizá no valía la pena responder.

## Los invariantes

Ocho. Valen en todos los nodos. Si uno se rompe, lo que salga del bucle no es evidencia.

1. **Predicción antes del dato.** Cada candidato llega con una predicción
   explícita de qué debería pasar y en qué dirección. Lo que no se predijo no se
   puede refutar: sin predicción previa cualquier resultado se acomoda al relato.
2. **Análisis declarado antes de correr.** El test, el umbral y la regla de
   decisión se fijan en el nodo 2, y el LLM no ve el crudo hasta que ese test ya
   corrió. Gelman & Loken (p.11) explican por qué no alcanza con la buena fe: los
   grados de libertad del investigador no se sienten como grados de libertad —
   condicionado a los datos ya vistos, cada elección de análisis parece la única
   razonable.
3. **Triage de tres vías, en este orden**, cuando la predicción falla: ¿medición
   inválida? → se descarta el dato; ¿código roto? → arreglar y volver a correr;
   recién entonces, hipótesis falsa → evidencia legítima. Las dos primeras vías
   **no cuentan como evidencia** y no entran al recuento. Que la política bajo
   prueba **falle** no es vía 1: el instrumento funcionó. Es vía 3, y entra al
   recuento con todo su costo. Confundirlas borra del registro justamente los
   casos en que la intervención no sirve.
4. **Dos fases autónomas.** Tamizaje barato, ruidoso y masivo, matando temprano;
   después confirmación cara, pareada y repetida. La política del contrato decide
   el pasaje de una a la otra sin pausa humana.
5. **El reporte sale del ledger.** El nodo de salida se redacta leyendo el archivo
   en disco, nunca la memoria de la conversación.

6. **Exploración y confirmación no son intercambiables.** Generar candidatos,
   hacer halving, consultar repetidamente el mismo banco o detenerse al ver
   separación son decisiones adaptativas. Corregir por el número bruto de
   candidatos no basta para dar cobertura nominal a un intervalo elegido después
   de esas decisiones. Para recomendar, congela los finalistas y el análisis antes
   de una evaluación confirmatoria independiente, o aplica un procedimiento
   declarado que cubra la selección y la parada. Si no puedes hacer ninguna de las
   dos cosas, el veredicto máximo es exploratorio, no `RECOMENDAR`.

7. **El presupuesto es una restricción, no una nota.** El contrato debe expresar
   límites separados para corridas, tiempo, consultas al banco, rondas y, cuando
   corresponda, consultas adaptativas. Cada resultado registra esas cinco
   dimensiones y **ninguna la declara el agente**: `scripts/correr.py` mide el
   tiempo con el reloj, cuenta las consultas leyendo el crudo y emite un recibo
   inmutable por corrida, incluidas las que fallan o se abortan.
   `scripts/verificar_presupuesto.py` suma recibos, no campos del ledger, y
   bloquea la ronda si ambos discrepan. Agotar el presupuesto no demuestra
   eficacia: por sí solo produce `SEGUIR MIDIENDO` o `ABANDONAR`, nunca
   `RECOMENDAR`.

8. **El halving necesita una garantía de supervivencia.** Una observación por
   candidato solo sirve para tamizar si el contrato acepta explícitamente el riesgo
   de eliminar candidatos buenos y la política tiene una cota o una simulación de
   supervivencia bajo el ruido esperado. Si la variación intra-candidato es del
   orden del efecto relevante, primero se repite, bloquea o parean las mediciones;
   no se rankea con una sola observación.

El arnés existe para que estos invariantes no dependan de la disciplina del
agente, y descansa en una regla: **el LLM no escribe ninguna cifra**. Las produce
un script y el agente aporta sólo lo que no se puede automatizar —la hipótesis,
el triage, la decisión.

- `scripts/correr.py` ejecuta el protocolo y emite el recibo del consumo medido.
- `scripts/analizar.py` recalcula métrica, intervalo, efecto, valor p por
  permutación pareada y corrección Holm o Bonferroni desde el crudo.
- `scripts/ledger.py` deriva cada `resultado` del recibo y del análisis, y su
  `check` relee ambos en disco: si alguien edita una métrica ya escrita o
  reescribe un crudo, la verificación falla. La interpretación —veredicto, triage,
  decisión— no entra ahí: va en un registro aparte, `ledger.py diagnostico`, que
  escribe el agente.
- `scripts/verificar_presupuesto.py` suma recibos y los concilia con el ledger.
- `scripts/verificar_contrato.py` cierra el nodo 0 con campos ejecutables.
- `scripts/verificar_reporte.py --strict --claims-json` exige y valida los claims;
  los números del texto que no aparecen en el ledger siempre quedan como advertencias.
- `scripts/ronda.py` encadena todo lo anterior para una ronda entera, y no abre
  una ronda nueva mientras queden resultados sin diagnosticar.

`scripts/autoprueba.py` ejercita todo lo anterior contra sus propios modos de
fallo —métrica inflada a mano, crudo reescrito, costo subdeclarado, `run_id`
reutilizado, separación inventada— en segundos y sin gastar un token. Correrla
después de tocar cualquier script es más barato y más fiable que pedir una
auditoría a un modelo.

Lo que el arnés todavía no puede hacer: comprobar que el protocolo mida lo que
el contrato dice que mide. Un instrumento mal diseñado produce recibos, hashes y
valores p impecables sobre la magnitud equivocada. Y el agente sigue siendo quien
corre los scripts: el arnés hace que falsear cueste más trabajo que medir, no que
sea imposible.

El presupuesto ejecutable vive en `presupuesto.json`, junto al contrato, y se
congela con él. No se edita para que entre una corrida nueva.

## Fundamento profesional

Por qué cada invariante no es arbitrario —el respaldo en diseño experimental,
comparaciones múltiples e inferencia secuencial, y la referencia a
`autoresearch` de Karpathy— está en `references/fundamento.md`; consultarlo
solo si hace falta justificar el diseño ante alguien, no para ejecutar el
bucle.

## El recorrido

Ocho nodos. Qué recibe, qué produce y quién ejecuta cada uno —incluidas las
secciones obligatorias del contrato del nodo 0— está en `references/grafo.md`,
junto con los diagramas.

### Orquestación del ciclo

Después de la compuerta, ejecuta estos pasos sin pedir confirmación entre ellos:

1. Congela el contrato, el banco y `presupuesto.json`, y corre
   `verificar_contrato.py`.
2. Escribe `ronda.json` con **todos** los candidatos del lote, sus predicciones y
   el comando del protocolo; corre `ronda.py ronda.json --exp EXP`.
3. Lee el parte que devuelve. Trae mejor, segundo, efecto, intervalo del efecto,
   p ajustado, si hay separación, el estado del ledger y el saldo. **Con eso
   alcanza para decidir**: no abras los crudos ni el ledger salvo para auditar un
   fallo concreto.
4. Aplica la política de selección. Si el criterio de parada no se cumple, escribe
   el `ronda.json` siguiente y vuelve al paso 2. `ronda.py` comprueba el
   presupuesto antes de gastar y se bloquea solo.
5. Cuando se cumple un criterio de parada, redacta el informe y su `claims.json`
   desde el ledger; corre `verificar_reporte.py --strict --claims-json EXP/claims.json`.
6. Decide `RECOMENDAR`, `SEGUIR MIDIENDO` o `ABANDONAR`. Solo después de esa
   decisión pregunta al usuario si hace falta una acción que el contrato no haya
   delegado en la política. Antes de `RECOMENDAR`, comprueba que la confirmación
   usó casos o consultas no usados para seleccionar candidatos; de lo contrario,
   el resultado es exploratorio.

Correr los nodos sueltos, un comando por candidato, es válido para depurar, pero
multiplica por nueve los turnos del orquestador y mete el crudo en su contexto.
El camino normal es `ronda.py`.

### El parte: lo único que sube al orquestador

Un bucle experimental no se encarece por lo que calcula sino por lo que el
orquestador vuelve a leer en cada turno. Un crudo de 80 KB volcado en la ronda 3
se relee en las treinta rondas siguientes; ese efecto compuesto, no el
razonamiento, es el que domina la factura.

Por eso cada nodo escribe todo en disco y devuelve un parte con tope duro de
2 KB: estado, cifras de decisión y la ruta del detalle. `scripts/parte.py` lo
impone por construcción, no por buena voluntad. La regla para el orquestador es
simétrica: **decide con el parte**. Abrir un crudo, un ledger o un análisis
completo se justifica sólo para auditar un fallo concreto que el parte señaló.

### Ejecución aislada mediante agentes

Cuando una ronda necesite trabajo que `ronda.py` no cubra, delegala en un
subagente de bajo contexto en vez de volcar el crudo al contexto principal. Qué
incluir en el encargo, cómo encadenar rondas sin devolver el control al usuario
y cuándo paralelizar está en `references/subagentes.md`.

## Autoaplicación: cuando el instrumento es parte de lo medido

Es la trampa más fácil de pisar: calibrar una herramienta usándola como medio de
medición, o evaluar un método con el método mismo. La checklist completa —banco
congelado antes de tocar el objeto, contabilidad de costos, filtración de
oráculo, y el resto— está en `references/autoaplicacion.md`; revisarla siempre
que el objeto bajo prueba sea el propio instrumento de medición.

## Dónde viven los artefactos

Un directorio por experimento, fuera del control de versiones:

```
experimentos/<nombre>/
  contrato.md      # nodo 0, congelado, con los hashes
  presupuesto.json # límites por corridas, tiempo, consultas, rondas, adaptativas
  banco.md         # el banco de evaluación, si aplica
  fuentes.json     # {"ids": [...]} con los casos congelados
  ronda.json       # la ronda a ejecutar; se reescribe en cada iteración
  ledger.jsonl     # append-only, una línea por plan y por resultado
  runs/            # recibos de consumo medido, uno por corrida
  crudos/          # observaciones del ejecutor, sin interpretar
  analisis/        # métricas, intervalos y p ajustados recalculados
```

## Herramientas de procedencia

Desde el directorio base de la skill:

El camino normal es un comando por ronda más el diagnóstico de cada candidato:

```sh
python3 scripts/verificar_contrato.py EXP/contrato.md
python3 scripts/ronda.py EXP/ronda.json --exp EXP        # --dry-run valida sin gastar
python3 scripts/ledger.py diagnostico EXP/ledger.jsonl --contrato EXP/contrato.md \
  --iteracion 1 --candidato tope-4 --veredicto '...' --decision muere   # uno por candidato
python3 scripts/verificar_reporte.py EXP/reporte.md EXP/ledger.jsonl \
  --strict --claims-json EXP/claims.json
```

`ronda.json` declara la ronda entera. El protocolo recibe `{crudo}`, que
`ronda.py` sustituye por la ruta donde debe escribir sus observaciones:

```json
{"iteracion": 1, "fase": "A",
 "contrato": "EXP/contrato.md", "presupuesto": "EXP/presupuesto.json",
 "fuentes": "EXP/fuentes.json", "hash_banco": "b7d20be", "regla": "halving",
 "test_declarado": "permutación pareada, holm, alpha 0.05",
  "analisis": {"metodo": "bootstrap_pareado", "semilla": "s1",
               "correccion": "holm", "alpha": 0.05, "umbral": 0.05,
               "remuestreos": 10000},
 "timeout": 1800.0,
  "candidatos": [
   {"nombre": "tope-4", "hipotesis": "...", "prediccion": {"direccion": "base"},
    "diseno": {"pareado": true}, "comando": ["python3", "protocolo.py", "{crudo}", "tope-4"]}]}
```

El crudo es JSONL con una observación por línea —
`{"candidato": "...", "caso": "...", "valor": 0.83}` — y el mismo conjunto de
`caso` en todos los candidatos: de ahí sale el pareo. Los nodos sueltos existen
para depurar:

```sh
python3 scripts/correr.py --run-id it1-c --recibos EXP/runs --crudo EXP/crudos/it1-c.jsonl \
  --iteracion 1 --candidato c --fase A --hash-contrato HASH -- python3 protocolo.py EXP/crudos/it1-c.jsonl
python3 scripts/analizar.py EXP/crudos/*.jsonl --metodo bootstrap_pareado --semilla s1 \
  --correccion holm --alpha 0.05 --umbral 0.05 --salida EXP/analisis/it1.json
python3 scripts/ledger.py resultado EXP/ledger.jsonl --contrato EXP/contrato.md \
  --fase A --iteracion 1 --candidato c --recibo EXP/runs/it1-c.json \
  --analisis EXP/analisis/it1.json --test-aplicado '...'
python3 scripts/ledger.py diagnostico EXP/ledger.jsonl --contrato EXP/contrato.md \
  --iteracion 1 --candidato c --veredicto '...' --decision muere \
  --via 3 --justificacion '...' 
python3 scripts/ledger.py check EXP/ledger.jsonl EXP/contrato.md
python3 scripts/verificar_presupuesto.py --recibos EXP/runs \
  --presupuesto EXP/presupuesto.json --ledger EXP/ledger.jsonl
```

Los dos primeros comandos agregan una línea, calculan `hash_contrato` como los
primeros 12 caracteres del SHA-256 del contrato y, si cambió respecto de la
línea anterior, agregan `hash_contrato_anterior`. `check` conserva los linajes
históricos pero rechaza todo `resultado` sin un `plan` previo del mismo linaje,
incluido el linaje vigente. El verificador de reporte imprime advertencias para
números que no aparecen en el ledger y termina con éxito: es una alarma para
revisión humana, no un gate duro.

Si el resultado vale documentarse, el resumen versionado va aparte, en la
documentación del proyecto. El ledger crudo no se versiona.

## Señales de que el bucle se soltó

Checklist de auditoría para correr al terminar cada experimento, en
`references/senales-de-alerta.md`.

## Vivencias propias

Lee `vivencias/ajustes.json` al comenzar una corrida y respeta sus defaults del
analizador: `confianza` (0.95), `remuestreos` (10000) y `alpha` (0.05). Estos
valores pueden cambiarse de forma estable entre experimentos; la corrección y
el umbral siguen siendo decisiones obligatorias del experimento y no ajustes
personales.

Para validar la forma del archivo, ejecuta:

```sh
rustc scripts/validar_ajustes.rs -O -o /tmp/prueba-y-error-validar && /tmp/prueba-y-error-validar vivencias/ajustes.json
```
