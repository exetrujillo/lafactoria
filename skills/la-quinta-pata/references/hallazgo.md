# Contrato de un hallazgo

Esta ficha adapta el modelo de Toulmin: una conclusión no vale por sonar
plausible, sino por la relación explícita entre afirmación, evidencia y garantía.
Úsala antes de incluir una objeción.

1. **Objetivo afectado:** qué resultado deja de conseguir y para quién.
2. **Afirmación:** qué riesgo concreto se sostiene, sin redactarlo como pregunta.
3. **Evidencia:** qué está explícitamente en el material y dónde. Usa una cita y
   sección, página, línea, ruta o identificador equivalente.
4. **Garantía:** por qué esa evidencia permite activar el mecanismo; separa el
   hecho observado de la interpretación que lo conecta con el objetivo.
5. **Mecanismo:** premisa, estado, transición, incentivo o interpretación que
   transforma la condición en daño.
6. **Condición:** input, contexto o estado necesario para que el mecanismo se
   active; indica si está observado o es una inferencia.
7. **Refutación:** evidencia o prueba que haría desaparecer o bajar el riesgo.
   Si el objeto es ejecutable, córrela: una refutación descrita y no ejecutada
   deja el hallazgo a mitad de camino.
8. **Estado:** `confirmado` si la refutación se ejecutó y el objeto falló como
   predice el hallazgo —incluye comando y salida—; `plausible` si no se ejecutó,
   diciendo si fue por no ser ejecutable o por alcance; `no determinable` si falta
   material.
9. **Magnitud:** probabilidad, impacto y razón del rango bajo, medio o alto.
10. **Acción:** prueba o mitigación concreta, responsable, punto de control y
    criterio de parada. Debe ser aplicable con los recursos que el objeto tiene
    hoy; si presupone una pieza inexistente, márcala `requiere cambio de contexto`
    y di qué haría falta.

No presentes una hipótesis como hecho. Si falta localizador, cobertura del corpus
o una dependencia causal, marca la parte como inferencia o `no determinable`; la
ausencia de contexto no es evidencia negativa. Una posibilidad sin mecanismo,
condición y consecuencia relevante no es un hallazgo.

El criterio de parada debe ser observable: qué resultado de la prueba permite
cerrar, rebajar o reformular el riesgo. Si la acción solo dice "revisar" o
"mejorar", todavía no es una acción verificable.

Dos formas de hallazgo que la lectura no alcanza, y que conviene buscar a
propósito: el control **demasiado estricto**, que rechaza el caso legítimo más
frecuente —el objeto y su documentación pueden coincidir perfectamente y estar
ambos equivocados—, y el criterio **correctamente documentado pero inválido en su
disciplina**. Ninguno de los dos aparece comparando el objeto consigo mismo.

**Fundamentos:** S. Toulmin, *The Uses of Argument* (1958); NIST, *AI Risk
Management Framework 1.0* (2023), https://doi.org/10.6028/NIST.AI.100-1.
