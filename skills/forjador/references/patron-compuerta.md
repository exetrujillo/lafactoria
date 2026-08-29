# Patrón de compuerta

**Cuándo evaluarlo.** La skill que se está diseñando tiene un disparador
amplio (se activa fácil, con frases genéricas) y su ejecución es cara —muchas
corridas, mucho contexto gastado, decisiones difíciles de revertir una vez que
arranca.

**Qué es.** La skill se autoinvoca ante el disparador, pero su cuerpo no actúa
de inmediato: abre preguntando al usuario, con las alternativas disponibles y
el costo de cada una sobre la mesa, y recién después de esa respuesta hace
algo.

**Por qué.** Se paga un clic (la pregunta) para evitar arrancar trabajo que el
usuario no pidió en realidad, o que pidió de una forma distinta a la que la
skill asumiría por defecto.

**Ejemplos en el repo.** `chatarrero` abre con una compuerta antes de escribir
código de scraping (¿editar o crear? ¿dónde guarda la salida?);
`prueba-y-error` abre preguntando qué costaría medir y qué alternativas hay
antes de correr el bucle de experimentación.

**Contra.** Agrega una ronda de ida y vuelta a cada invocación, incluso cuando
el usuario ya sabía exactamente qué quería. No lo apliques a skills baratas o
de disparador angosto — ahí la compuerta es fricción sin beneficio.
