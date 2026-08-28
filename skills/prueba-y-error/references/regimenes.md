# Regímenes: cuándo fase A y cuándo fase B

La elección no es de gusto. Sale de una razón —**cuánto cuesta correr contra
cuánto cuesta razonar**— corregida por dos cosas: cuánto ruido tiene la medición
y cuánta estructura aprovechable tiene el espacio de búsqueda.

Tres preguntas antes de elegir régimen:

1. ¿Cuánto cuesta una corrida comparada con pensar un candidato bueno?
2. ¿Cuánto varía la métrica entre repeticiones del **mismo** candidato?
3. ¿El espacio tiene estructura que un modelo pueda aprovechar, o es plano?

## Fase A: correr es barato y el espacio no tiene estructura conocida

Cuando una corrida cuesta poco, la búsqueda masiva y tonta le gana a la búsqueda
razonada. El resultado de Bergstra & Bengio lo explica: en espacios de **baja
dimensionalidad efectiva** —solo unas pocas dimensiones importan, pero no se sabe
cuáles— una grilla gasta la mayoría de sus corridas variando cosas que no afectan
el resultado, mientras que muestrear al azar prueba valores distintos en la
dimensión que sí importa (p.3–4, p.15).

Corolario operativo: **cuando no sabés qué dimensiones importan, muchas corridas
tontas rinden más que pocas corridas razonadas.** Razonar sobre un espacio que no
entendés todavía es gastar el recurso caro para ahorrar el barato.

## El presupuesto fijo: el problema "n versus B/n"

Con un presupuesto total B, ¿muchos candidatos con poco cada uno, o pocos con
mucho? Li et al. lo llaman el problema "n versus B/n" (p.7) y su respuesta es
incómoda: **no hay una partición universalmente buena**, porque depende de cuánto
presupuesto necesita un candidato para revelar su valor, que es justo lo que no se
sabe. Hyperband cubre varias particiones en vez de apostar a una sola (p.6, p.12).

Regla operativa: si no podés justificar cuánto presupuesto necesita un candidato
para mostrarse, no elijas una partición — corré dos o tres agresividades
distintas. En su experiencia, la muerte temprana agresiva es generalmente segura
(p.21).

## Matar temprano sin correrlo todo

El racing es anterior y más simple: un candidato muere cuando una cota de
confianza, calculada bajo el modelo y la regla de parada declarados, queda por
debajo de la del mejor, sin necesidad de terminar su corrida. Maron & Moore
describen sus garantías bajo condiciones explícitas, no como una licencia para
eliminar con una observación ruidosa (p.6). Si esas condiciones no están
declaradas o no se sostienen, el halving es solo una heurística exploratoria.

Es el criterio de fase A cuando la métrica se puede evaluar de a incrementos.

## Ruido alto: primero varianza, después razonamiento

Antes de gastar en razonar mejor, gastar en **medir** mejor. Las herramientas son
las de siempre y son baratas: repetición, pareo, bloqueo y aleatorización de
orden. La regla de Box, Hunter & Hunter (p.112) es literal: *bloquea lo que puedas
y aleatoriza lo que no*. Un diseño de comparación pareada (p.100–101) detecta
diferencias mucho más chicas con la misma cantidad de corridas, porque cancela lo
que las dos condiciones comparten.

**Señal de alarma:** si la variación entre repeticiones del mismo candidato es del
orden de la diferencia entre candidatos distintos, cualquier ranking que produzcas
es ruido con forma de conclusión. La respuesta no es generar más candidatos: es
volver al nodo 2 y arreglar el diseño.

## Fase B: correr es caro, razonar sale barato en comparación

Invertida la razón, conviene construir un modelo del espacio y elegir el próximo
punto por su valor esperado. Es el territorio de las funciones de adquisición:
probabilidad de mejora y mejora esperada (Garnett p.149), con variantes para
elegir lotes en vez de puntos sueltos cuando se puede correr en paralelo (p.275).

Aquí el LLM interviene en cada corrida, las condiciones van pareadas y todo se
repite. El criterio ya no es matar temprano sino **severidad y tamaño de efecto**:
la pregunta deja de ser "¿cuál sobrevive?" y pasa a ser "¿la diferencia es lo
bastante grande como para importar, y lo bastante firme como para apostar?".

## Tabla rápida

| Correr | Ruido | Estructura | Régimen |
|---|---|---|---|
| barato | cualquiera | desconocida o plana | Fase A masiva, muerte temprana agresiva |
| barato | alto | cualquiera | Fase A, pero con repeticiones antes de rankear |
| caro | bajo | aprovechable | Fase B directa, con adquisición |
| caro | alto | cualquiera | Primero pareo y bloqueo; si aun así no separa, abandonar |

Cuando "caro" y "alto ruido" se juntan y el diseño pareado no logra separar, la
respuesta honesta del nodo 7 es ABANDONAR o SEGUIR MIDIENDO, no forzar un ganador.

## La regla que cierra todo

**El LLM genera candidatos; la política elige entre ellos.** El modelo es bueno
proponiendo variedad y malo eligiendo bajo ruido, porque se convence de patrones
que no están. Pero la división tiene un límite por el otro lado: la ley de
variedad requerida (Ashby p.112) dice que la capacidad de un regulador no puede
exceder su variedad. Si el generador propone tres variantes de la misma idea,
ninguna política va a encontrar algo mejor que esas tres.

## Higiene común a ambos regímenes

El régimen no puede reparar un contrato ambiguo ni una procedencia rota. Antes de
la primera corrida, verificar el contrato; al agregar cada lote, usar el ledger
en modo `plan` y luego `resultado`, nunca editar JSONL existente. Si se revisa el
contrato, comienza un linaje nuevo: los resultados del linaje anterior sirven
como historia, pero no pueden completar un plan faltante del linaje vigente.
