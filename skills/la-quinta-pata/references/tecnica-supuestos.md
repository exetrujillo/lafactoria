# Técnica: reglas no escritas y supuestos

Esta técnica busca dependencias que el argumento usa sin declarar. Se relaciona
con el análisis basado en supuestos y con STPA: una premisa importa solo cuando
su falsedad rompe una restricción o una transición del objetivo.

## Procedimiento

1. Completa para cada candidato: "esto solo funciona si...".
2. Ordena los supuestos por dependencia del objetivo, no por lo llamativos que
   sean. Empieza por los que sostienen otras partes del argumento.
3. Escribe la cadena exacta: premisa falsa -> mecanismo de ruptura -> observación
   comprobable -> daño.
4. Distingue supuesto sobre el mundo (input, actor, recurso, entorno) de supuesto
   sobre el propio análisis (cobertura, medición, interpretación).
5. Conserva hasta tres supuestos con conexión causal. Si la observación no puede
   obtenerse del material o de una prueba concreta, marca `no determinable`.

No uses nombres de sesgos como diagnóstico. Un supuesto no es un hallazgo porque
sea implícito: debe existir una condición bajo la que falle y una consecuencia que
importe.

**Ejemplo:** "esto solo funciona si el operador ve todas las alertas" produce un
   hallazgo solo si existe una cola o filtro que puede ocultarlas, ese ocultamiento
   cambia una decisión y puede comprobarse midiendo alertas emitidas frente a
   alertas observadas.

**Fundamentos:** N. Leveson, *Engineering a Safer World: Systems Thinking Applied
to Safety* (2011), https://mitpress.mit.edu/9780262533690/; N. Leveson,
*STPA Handbook* (2018); S. Toulmin, *The Uses of Argument* (1958).
