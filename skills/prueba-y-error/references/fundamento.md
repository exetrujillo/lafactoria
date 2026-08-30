# Fundamento profesional

Por qué cada invariante del cuerpo principal no es arbitrario: el respaldo en
diseño experimental, comparaciones múltiples e inferencia secuencial. Consultar
solo si hace falta justificar el diseño ante alguien, no para ejecutar el bucle.

La separación entre bloqueo, aleatorización, pareo y replicación sigue el diseño
experimental de Box, Hunter & Hunter (pp.100-113): bloquear lo que se puede y
aleatorizar lo que no, con replicación genuina. La fase A no debe prometer más que
un tamiz: Hyperband formula explícitamente el compromiso entre muchos candidatos
con poco presupuesto y pocos con mucho, y no existe una partición universalmente
óptima (Li et al., 2018, pp.6-7). Sus garantías de halving dependen de supuestos
sobre la evolución de la métrica, por lo que una sola observación ruidosa no
justifica una recomendación.

La selección y el análisis que se adaptan a los datos cuentan como comparaciones
múltiples implícitas. Dwork et al. (2015, pp.1-2, 6) señalan que las hipótesis,
variables, tests y métodos elegidos después de explorar los mismos datos no
conservan automáticamente los niveles nominales; la salida profesional es usar
datos frescos o particiones disjuntas, o un método de inferencia que modele la
adaptación. La regla de parada también debe formar parte del análisis: los
procedimientos secuenciales pueden cambiar la validez de los valores p y de los
intervalos si se los trata como muestreos fijos (MacKay, 2003, p.476).

Por eso `RECOMENDAR` requiere confirmación independiente o inferencia secuencial
declarada. Demsar (2006, pp.8-9, 15) respalda la comparación global y el ajuste
por múltiples comparaciones, pero ese ajuste no sustituye el control de la
adaptación ni del momento de parada. El ledger aporta procedencia; no reemplaza
el diseño estadístico.

Como patrón de ingeniería de ciclo corto, `autoresearch` de Karpathy usa una
corrida con presupuesto fijo, una métrica comparable, un único archivo bajo
modificación y un registro de conservar o descartar cada cambio
([github.com/karpathy/autoresearch](https://github.com/karpathy/autoresearch)).
Aquí se adopta esa disciplina solo como control de comparabilidad y
reversibilidad: la métrica, el banco, el análisis y los límites siguen siendo
los del contrato, y una mejora en la métrica primaria no puede ocultar un costo
o una regresión declarados como secundarios.
