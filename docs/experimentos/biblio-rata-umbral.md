# Umbral de conveniencia de biblio-rata

Linaje de doce rondas (2026-08-28) para decidir cuándo conviene invocar
`biblio-rata` en vez de leer el PDF directo.

## Qué se preguntó

`skills/biblio-rata/SKILL.md` traía una regla de uso escrita a ojo: leer directo
por debajo de ~4 páginas, usar el índice desde ~10, y una zona intermedia
resuelta por intuición. El linaje debía medirla.

## Cómo evolucionó el instrumento

Cada ronda corrigió un defecto de la anterior. La secuencia importa más que los
números, porque los tres primeros defectos invalidaban la comparación entera:

| ronda | qué aportó |
|---|---|
| v3–v7 | primeras comparaciones directa vs indexada sobre recortes de PDFs largos |
| v8 | **invalidada**: identidad de caso divergente y fixtures reutilizados sin declararlo |
| v9 | reparación trazable de v8; fuentes aún no independientes |
| v10 | cuatro fuentes nuevas; concluyó `RECOMENDAR` para 4–5 páginas |
| v11 | **suma el costo fijo de invocación y elimina el oráculo**; 12 PDFs naturales |
| v12 | mide el cruce en caracteres, penaliza el fallo de política; 20 PDFs naturales |

Métrica en todo el linaje: el proxy `caracteres de salida / 4`. No hay
tokenizador offline en la máquina, así que **en ningún punto se midieron tokens
reales**, y menos tokens de imagen.

## Los tres defectos que invalidaban la medición

**1. El costo fijo no se contaba (hasta v11).** Cargar `skills/biblio-rata/SKILL.md`
son 6507 caracteres, o `F = 1626.75` en el proxy. v3–v10 lo omitían por completo
del lado indexado. Sumarlo invierte las cuatro celdas con que v10 justificó su
`RECOMENDAR`. En 11 de 12 documentos de v11, invocar la skill cuesta más que todo
lo que la skill devuelve en una consulta. `F` es además una cota inferior: excluye
`references/consultas.md`, que llevaría el fijo a 2662.25.

**2. El instrumento le soplaba la respuesta a la política (hasta v11).** Cuando el
fragmento del buscador no bastaba, `ejecutar.py` de v10 abría `pagina.py <doc>
<PÁGINA_OBJETIVO>`, la página donde ya se sabía que estaba la respuesta. Un agente
real no la conoce. La validez quedaba garantizada por construcción y los fallos de
recuperación eran invisibles. Sin el oráculo, la política falló en 3 de 12 casos
(v11) y en 10 de 120 consultas (v12).

**3. La ventaja de costo se confundía con utilidad (hasta v12).** En v11,
`sintesismusocpy-6p` figuraba como pro-índice con ventaja +1831.5 y **no entregó la
respuesta**: el mejor hit apuntó a la página 6, expandió a la 5 y nunca tocó la 4.
Costo bajo por no encontrar nada. v12 lo corrige con `indexada_efectiva`: una
consulta fallida suma el costo de leer el documento entero, porque eso es lo que
termina haciendo el agente. Seis casos cambian de clase al aplicarlo, todos hacia
la lectura directa, ninguno al revés.

## Qué se midió (v12)

20 PDFs naturales sin solapamiento con v11, de 430 a 100612 caracteres, 240
evaluaciones, 538 registros de ledger, cero fallos de vía 1 y vía 2.

Cruce en volumen de texto, `C(q)`, acotado por observaciones a ambos lados:

| q | `C-(q)` | `C+(q)` | banda de indecisión |
|---|---:|---:|---:|
| 1 | 7450 | 63370 | 12 casos |
| 3 | 10432 | 100612 | 11 casos |
| 5 | 15461 | 100612 | 10 casos |

Tasa de fallo de recuperación por posición de la respuesta: `primera` 0 de 12,
`interior` 8 de 84, `ultima` 2 de 24.

## Qué se concluyó

1. **La regla vigente estaba mal, y su respaldo también.** El `RECOMENDAR` de v10
   para 4–5 páginas se sostenía sólo porque su modelo de costo omitía `F`. Con el
   costo fijo contado, ningún caso de esa banda favorece al índice (v11).
2. **Lo que decide es el volumen de texto, no las páginas.** 21 páginas con 7785
   caracteres no favorecen el índice; 4 páginas con 10432 sí. Con `chars_directo`
   el cruce queda acotado para q=1, 3 y 5; con páginas queda sin cota superior
   para q=3 y q=5.
3. **`"desde ~10 páginas conviene el índice"` no se sostiene.** Con q=1, 3 de 9
   documentos de ≥10 páginas no lo favorecen, y 6 documentos de menos de 10
   páginas sí lo favorecen, todos con ≥10432 caracteres.
4. **Más preguntas favorecen la lectura directa, al revés de lo que decía la
   skill.** La salida del índice se paga por consulta y el documento se lee una
   sola vez: 12 casos pro-índice con q=1, 7 con q=3, 2 con q=5.
5. **Lo único de la regla original que sobrevive** es el extremo corto: los cuatro
   documentos de menos de 4 páginas favorecen la lectura directa.

Dos sesgos declarados corren **en contra** del índice y no se corrigieron: `F`
excluye `consultas.md`, y `directa` se modela con `pdftotext` en vez de `Read`,
que mete las páginas como imagen y cuesta bastante más. Que el índice pierda aun
así refuerza la conclusión.

## Qué queda pendiente

El veredicto formal de v12 fue `SEGUIR MIDIENDO`, no por los hallazgos anteriores
—que son firmes y replicados fuera de muestra— sino por una condición de la regla
de decisión que no se cumplió: que `chars_directo` separara *estrictamente* mejor
que las páginas en los tres valores de q. En q=5 el ancho de banda da 10 contra 5
a favor de las páginas, pero es un artefacto: con q=5 hay 17 de 20 casos
`directa`, así que "hasta 14 páginas" cubre 15 casos sin que exista ninguna cota
superior. **La métrica de ancho del contrato no distingue una banda angosta de un
cruce inexistente.** Es un defecto del análisis pre-declarado, y se dejó sin
corregir para no ajustar la regla después de ver el dato.

Una ronda futura, con contrato nuevo, tendría que: arreglar esa métrica de ancho;
aislar el efecto de q usando prefijos anidados de las mismas consultas, en vez de
mezclar "más consultas" con "otras consultas"; densificar el banco entre 7450 y
63370 caracteres, donde la banda de indecisión es ancha; y declarar el divisor
diferencial como análisis primario.

Ese último punto merece su propia nota. Se pre-declaró un chequeo de robustez
recalculando todo con divisores 3.0, 3.5, 4.0 y 4.5, y resultó **vacío por
construcción**: un divisor común se cancela en la ventaja y en el umbral
relativo, así que los cuatro valores dan idénticamente lo mismo. Lo que sí mueve
el resultado es un divisor *diferencial* entre prosa y salida estructurada —el
buscador devuelve `slug p.12 · -8.43 · …`, que tokeniza peor que la prosa—: hasta
6 de 20 casos cambian de clase. Quedó como análisis exploratorio y no decisorio.

## Auditoría del método

Qué aguantó, contra la checklist del propio `prueba-y-error`:

**Aguantó.** El triage descartó una preparación completa de v12 —11 mediciones
inválidas por consultas con caracteres que FTS5 lee como operadores— y reinició
ledger y crudos en vez de mezclarlas. Los planes se escribieron antes de cada
medición en las dos rondas. v11 y v12 verificaron procedencia por SHA-256 contra
todos los bancos anteriores, que es el control que faltó cuando v8 se invalidó, y
v12 detectó y reportó por su cuenta que su propio chequeo de robustez era vacío.
Ninguna ronda tocó `skills/` mientras medía.

**Lo que ningún invariante atrapó.** Los tres defectos de la sección anterior
sobrevivieron entre cuatro y ocho rondas cada uno. Ninguno es un incumplimiento
del método: el ledger estaba completo, las predicciones eran previas, el análisis
estaba declarado. Eran defectos del **modelo de costo** y del **instrumento**, y
el bucle no tiene ningún nodo que pregunte si la magnitud declarada mide lo que la
decisión necesita. Es la misma clase de hallazgo que apareció también en el
experimento del tope de páginas, y ahora con tres casos más.

Las lecciones incorporadas al método son: contabilidad completa del costo,
filtración de oráculo, distinción entre medición inválida y política fallida, y
ventaja de costo con utilidad cero.
