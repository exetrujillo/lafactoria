# El ledger: formato, reglas y criterios de parada

El ledger es la única memoria que cuenta. Vive en `experimentos/<nombre>/ledger.jsonl`,
en disco y fuera del contexto. **Si no está escrito, no ocurrió.**

## Por qué tres registros por iteración y no uno

El invariante "predicción antes del dato" y la regla "append-only" son
incompatibles con una sola línea por iteración: escribirla al final permitiría
acomodar la predicción a lo que ya se vio, y editarla después rompe el
append-only. Por eso cada iteración deja **tres** registros enlazados por
`iteracion` y `candidato`:

- `plan` — se escribe **antes de correr**. Contiene hipótesis, predicción, diseño,
  el test declarado y la regla de selección. A partir de acá el test ya no se
  puede cambiar.
- `resultado` — se escribe **después de correr** y lo deriva el arnés del recibo y
  del análisis: métrica con intervalo, test aplicado, crudo y costo. Ninguna cifra
  la escribe el modelo y ninguna interpretación entra en esta línea.
- `diagnostico` — se escribe **cuando el agente interpreta**: veredicto, la vía del
  triage si la predicción falló, y la decisión del selector. Va aparte porque llega
  después del dato y el append-only prohíbe volver sobre la línea del resultado.
  Separarlo además hace visible su ausencia, que como campo relleno quedaría
  disimulada.

Un `resultado` sin su `plan` previo en el archivo es evidencia inválida, y el
nodo 7 debe descartarlo. Un `diagnostico` sin su `resultado` se rechaza igual, y
solo puede haber uno por resultado: dos lecturas de la misma corrida no dejarían
manera de saber cuál vale.

## Formato

Un objeto JSON por línea, sin saltos internos.

```json
{"tipo":"plan","ts":"2026-08-28T14:02:11Z","hash_contrato":"a3f9c1","hash_banco":"7d20be","fase":"A","iteracion":3,"candidato":"tope-8","fuentes":["consulta-03","consulta-04"],"hipotesis":"un tope mayor reduce las re-consultas sin encarecer la respuesta media","prediccion":{"magnitud":"aciertos por kilotoken","direccion":"sube","tamano_esperado":"+10% o mas"},"diseno":{"semilla":41,"orden":"aleatorizado","repeticiones":1,"entorno":"hash-e0c4","pareado_con":["tope-5"]},"test_declarado":"diferencia de medias pareada, relevante si supera el umbral del contrato","decision_si_falla":"muere en el halving","regla":"halving sobre el tercio inferior"}
{"tipo":"resultado","ts":"2026-08-28T14:09:47Z","hash_contrato":"a3f9c1","fase":"A","iteracion":3,"candidato":"tope-8","run_id":"it3-tope-8","crudo":"crudos/it3-tope-8.jsonl","sha256_crudo":"9f2b8c...","metrica":{"valor":0.82,"intervalo":[0.71,0.93],"n":18},"analisis":{"metodo":"bootstrap_pareado","correccion":"holm","alpha":0.05,"semilla":"s1","ruta":"analisis/it3.json"},"test_aplicado":"diferencia de medias pareada","costo":{"corridas":1,"segundos":412,"consultas_banco":18,"rondas":1,"consultas_adaptativas":0},"fuentes":["consulta-03","consulta-04"]}
{"tipo":"diagnostico","ts":"2026-08-28T14:12:03Z","hash_contrato":"a3f9c1","iteracion":3,"candidato":"tope-8","veredicto":"prediccion fallida: sube 2%, por debajo del umbral","triage":{"via":3,"justificacion":"medicion valida y codigo intacto; la hipotesis no se sostiene"},"decision":"muere"}
```

### Campos

| Campo | Dónde | Qué es |
|---|---|---|
| `tipo` | los tres | `plan`, `resultado` o `diagnostico` |
| `ts` | los tres | timestamp ISO-8601, en UTC |
| `hash_contrato` | los tres | encabeza todo; si cambia, es otro experimento |
| `hash_banco` | `plan` | hash del banco congelado, cuando el instrumento es parte de lo medido |
| `fase` | `plan`, `resultado` | `A` o `B` |
| `iteracion`, `candidato` | los tres | la clave que enlaza los tres registros |
| `fuentes` | `plan`, `resultado` | lista no vacía de IDs de casos o consultas; debe coincidir entre plan y resultado |
| `hipotesis` | `plan` | qué se cree y por qué |
| `prediccion` | `plan` | magnitud, dirección y tamaño esperado. Las tres, o no es predicción |
| `diseno` | `plan` | semilla, orden, repeticiones, hash del entorno, con quién va pareado |
| `test_declarado` | `plan` | el test y su umbral, fijados antes de ver nada |
| `decision_si_falla` | `plan` | qué pasa si la predicción no se cumple, decidido de antemano |
| `regla` | `plan` | qué regla de la política decidirá quién sobrevive, fijada antes de correr |
| `crudo`, `sha256_crudo` | `resultado` | ruta a la salida sin interpretar y su hash: si el crudo cambia, `check` lo detecta |
| `run_id` | `resultado` | el recibo de `correr.py` que respalda el costo |
| `metrica` | `resultado` | valor, **intervalo** y n. Un valor sin intervalo no sirve |
| `analisis` | `resultado` | método, corrección, alfa, semilla y ruta del análisis que produjo la métrica |
| `test_aplicado` | `resultado` | debe coincidir con `test_declarado`. Si no coincide, la línea es inválida |
| `veredicto` | `diagnostico` | cumplida o fallida, con el número |
| `triage` | `diagnostico` | solo si falló: vía 1, 2 o 3, con justificación |
| `decision` | `diagnostico` | `muere`, `sigue`, `promueve` |
| `costo` | `resultado` | corridas, segundos, **consultas al banco**, rondas y consultas adaptativas, obligatorios, finitos y no negativos |

Las vías 1 y 2 del triage (medición inválida, código roto) quedan registradas pero
**no cuentan** en el recuento de evidencia del nodo 7.

### Linaje del contrato

`hash_contrato` es el prefijo de 12 caracteres del SHA-256 de los bytes de
`contrato.md`. No se debe escribir a mano: `scripts/ledger.py` lo calcula. El
ledger puede conservar varios linajes porque una revisión del contrato no borra
la historia. La primera línea de un linaje nuevo lleva además
`hash_contrato_anterior` con el hash vigente inmediatamente anterior; las líneas
siguientes de ese linaje ya no necesitan repetirlo.

Un `resultado` solo es válido si existe un `plan` anterior con la misma pareja
`(iteracion, candidato)` **y el mismo `hash_contrato`**. Por tanto, un resultado
que parece enlazar con un plan antiguo pero no tiene plan bajo el linaje vigente
es huérfano y se rechaza. `scripts/ledger.py check` aplica esta regla sin
reescribir ninguna línea.

Cuando el experimento usa un banco, todos los `plan` de un mismo linaje deben
llevar el mismo `hash_banco`; `ledger.py check` rechaza cambios de ese hash. Si
el banco, instrumento, entorno o protocolo cambia, no se corrige el ledger:
se congela otro contrato y se abre otro linaje.

La interfaz de escritura es deliberadamente pequeña:

```sh
python3 scripts/ledger.py plan LEDGER --contrato CONTRATO \
  --fase A --iteracion 1 --candidato nombre --hipotesis '...' \
  --prediccion-json PREDICCION.json --diseno-json DISENO.json \
  --test-declarado '...' --decision-si-falla '...' --fuentes-json FUENTES.json \
  --regla halving
python3 scripts/ledger.py resultado LEDGER --contrato CONTRATO \
  --fase A --iteracion 1 --candidato nombre \
  --recibo runs/it1-nombre.json --analisis analisis/it1.json --test-aplicado '...'
python3 scripts/ledger.py diagnostico LEDGER --contrato CONTRATO \
  --iteracion 1 --candidato nombre --veredicto '...' --decision muere \
  --via 3 --justificacion '...'
python3 scripts/ledger.py check LEDGER CONTRATO
```

El `resultado` no recibe ninguna cifra: las saca del recibo de `correr.py` y del
análisis de `analizar.py`, y falla si el recibo no corresponde a ese candidato o
si su crudo no participó del análisis citado. En el `diagnostico`, `--via` y
`--justificacion` van juntas y solo cuando la predicción falló; el veredicto y la
decisión van siempre.

`FUENTES.json` tiene la forma `{"ids":["caso-1","caso-2"]}`. Los mismos IDs
se declaran en el plan y se verifican en el resultado. Para cerrar con
`RECOMENDAR`, `claims.json` debe identificar cada resultado por iteración, candidato
y `hash_contrato`, derivar el efecto y el intervalo de los resultados, declarar
`correccion_comparaciones` y `confirmacion_independiente: true`; el verificador
rechaza fuentes compartidas entre las fases A y B y rechaza intervalos superpuestos.

Los argumentos `*-json` aceptan una ruta a un archivo JSON o el objeto JSON
directamente. El comando fuerza el campo `tipo` y agrega el timestamp UTC si falta.

## Reglas de escritura

1. **Append-only.** Nada se edita hacia atrás. Una corrección es un registro nuevo
   que referencia la iteración anterior, nunca una línea reescrita.
2. **El `plan` se escribe antes de correr.** No "antes de analizar": antes de
   correr.
3. **El crudo no entra al ledger**, solo su ruta. El ledger se lee entero muchas
   veces; los crudos, casi nunca.
4. **Todo número que aparezca en el reporte final tiene que estar en el ledger.**
   Si al redactar el nodo 7 hace falta un número que no está, no se estima: se
   corre otra iteración o se dice que no se midió.
5. **La interpretación va en su propio registro.** El `resultado` lo deriva el
   arnés; el `diagnostico` lo escribe el agente. `ronda.py` bloquea la ronda
   siguiente mientras queden resultados sin diagnosticar en el linaje vigente:
   acumular corridas sin saber si la anterior fue evidencia es gastar presupuesto
   a ciegas.

## Presupuesto ejecutable

El presupuesto no se deja solo en prosa. `presupuesto.json` contiene límites
congelados, por ejemplo:

```json
{"max_corridas": 40, "max_segundos": 7200,
 "max_consultas_banco": 400, "max_rondas": 12,
 "max_consultas_adaptativas": 40}
```

Antes de escribir un nuevo `plan`, el orquestador ejecuta:

```sh
python3 scripts/verificar_presupuesto.py --recibos EXP/runs \
  --presupuesto EXP/presupuesto.json --ledger EXP/ledger.jsonl
```

El verificador suma los **recibos** de `runs/`, no los campos del ledger, y cuenta
las rondas por `iteracion` para no multiplicar una ronda por cada candidato. Con
`--ledger` además concilia: si un resultado no cita un recibo, cita uno inexistente
o declara un costo distinto del medido, la ronda se bloquea. Las corridas fallidas
y abortadas también tienen recibo y también gastan presupuesto; invalidar una
medición no hace desaparecer lo que costó. El agotamiento se registra como motivo de
cierre; no convierte el resultado en evidencia de eficacia. La comprobación no
reemplaza un lock externo cuando varios procesos escriben el mismo ledger: en ese
caso se usa un solo subagente secuencial.

## Criterios de parada

Se declaran en el contrato, antes de empezar. Son tres y basta con uno:

- **Presupuesto agotado.** Corridas, tiempo o consultas al banco: lo que se acabe
  primero. El límite de consultas importa tanto como el de corridas, porque cada
  consulta al mismo banco gasta validez.
- **Separación lograda.** El intervalo de **la diferencia** entre el mejor y el
  segundo queda íntegramente por encima del umbral de relevancia, y el valor p
  ajustado por comparaciones múltiples cae bajo el alfa del contrato. Que sea
  distinguible no alcanza: tiene que importar.

  No se usa la disjunción de los intervalos marginales. En un diseño pareado esos
  intervalos arrastran la variación entre casos que el pareo justamente cancela,
  así que pueden solaparse mientras la diferencia es contundente: exigir que no se
  toquen rechaza efectos reales. `analizar.py` reporta los intervalos marginales
  pero decide con el del efecto. En una selección adaptativa esto solo habilita `RECOMENDAR` si existe
  confirmación independiente o un análisis declarado que cubra la regla de
  selección y parada.
- **La próxima corrida no paga.** Si el dato que falta ya no puede cambiar la
  decisión, medirlo es gasto. Es el criterio no arbitrario: se para cuando el
  valor de la información residual cae por debajo del costo de obtenerla.

## El nodo de salida

Se redacta **leyendo el archivo**, no la conversación. Tres piezas obligatorias:

1. **Corrección por comparación múltiple.** Con muchos candidatos, alguno se ve
   bien por azar. Demšar (p.9, p.15) da el procedimiento para comparar varios
   sistemas sin engañarse, con las tablas de valores críticos y el ajuste paso a
   paso de los `p` según el número de comparaciones.
2. **Tamaño de efecto con intervalo, no solo el ganador.** "A le ganó a B" sin
   cuánto ni con qué precisión no es un resultado.
3. **Un veredicto de los tres**, explícito:
   - **RECOMENDAR** — hay separación por encima del umbral, y se dice cuál y cuánto.
   - **SEGUIR MIDIENDO** — la dirección se ve pero el intervalo todavía cruza el
     umbral; se dice cuántas corridas más harían falta.
   - **ABANDONAR** — el ruido domina o la diferencia no llega al umbral aunque
     exista. Es un resultado válido, no un fracaso: cierra la pregunta.

Cerrá siempre con las **vías 1 y 2 del triage** que hayan aparecido, aunque no
cuenten como evidencia. Un experimento donde la mitad de las corridas se
descartaron por mediciones inválidas dice algo importante sobre el instrumento,
aunque no diga nada sobre la hipótesis.
