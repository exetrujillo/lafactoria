# Autoaplicación: cuando el instrumento es parte de lo medido

Es la trampa más fácil de pisar: calibrar una herramienta usándola como medio de
medición, o evaluar un método con el método mismo. Revisar esta lista siempre
que el objeto bajo prueba sea el propio instrumento de medición.

- **Congelar el banco de evaluación antes de tocar el objeto.** Se escribe
  primero, se hashea, y el hash entra en el contrato del nodo 0.
- **Nunca modificar banco e instrumento en la misma iteración.** Si el banco tiene
  que cambiar, es otro experimento con otro contrato y los resultados anteriores
  no se mezclan con los nuevos.
- **La clave de respuesta se pre-declara y se coteja mecánicamente.** Si juzgar un
  resultado exige el criterio del mismo modelo que corre el experimento, el
  instrumento quedó contaminado por una segunda vía, distinta de la obvia.
- **Contabilidad completa del costo.** El contrato enumera **todas** las
  componentes, incluidos los costos fijos de adoptar la intervención, no solo los
  variables. Omitir una no sesga el resultado: lo da vuelta. Un linaje de ocho
  rondas recomendó una regla que se invirtió entera al sumarle el costo fijo que
  nadie había contado (`docs/experimentos/biblio-rata-umbral.md`).
- **Filtración de oráculo.** El instrumento no le puede pasar a la política bajo
  prueba información que su usuario real no tendría. Si el camino de respaldo ya
  sabe dónde está la respuesta, la validez queda garantizada por construcción y
  los fallos de la intervención se vuelven invisibles.
- **Ventaja de costo no es utilidad.** Un candidato que no entrega el resultado
  siempre gana en costo. El contrato declara de antemano qué se paga cuando la
  política falla; si no, el bucle premia justamente lo que no sirve.
- **Una clave de cadenas mide recuperación, no corrección.** Cotejar términos
  verifica que el texto apareció, no que la respuesta sea correcta. El reporte
  tiene que nombrar lo que la clave no cubre.
- **Cada consulta al banco gasta validez.** Reutilizar el mismo conjunto con un
  proceso que se adapta a las respuestas produce sobreajuste al banco aunque nadie
  haga trampa (Dwork et al. p.2). El presupuesto del contrato incluye **cuántas
  veces** se puede consultar, no solo cuántas corridas se pueden hacer.
