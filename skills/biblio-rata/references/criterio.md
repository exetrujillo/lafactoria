# Criterio de uso

Esta referencia sirve para decidir si conviene pagar el costo fijo de cargar la
skill y construir el índice antes de una pregunta.

Lo que importa es el volumen de texto, no el número de páginas. Como aproximación,
cuenta los caracteres que produciría `pdftotext` sin volcar el resultado al
contexto. Las mediciones disponibles cubren PDFs de 430 a 100612 caracteres:

- Para una pregunta, por debajo de unos 7500 caracteres suele ser más barato leer
  el documento entero; desde unos 63000 gana el índice en todos los casos medidos.
- Para tres preguntas, el corte inferior sube a unos 10000 caracteres; para cinco,
  a unos 15000. Más preguntas favorecen la lectura directa porque el documento se
  lee una vez, mientras cada consulta vuelve a pagar la salida del índice.
- La zona intermedia no está calibrada. Si la skill ya se cargó en la sesión, el
  costo fijo ya está pagado y conviene usarla.
- Si hace falta citar página, conviene el índice a cualquier tamaño porque la
  referencia sale del buscador.

Las páginas no predicen bien el costo: 21 páginas con 7785 caracteres no
favorecieron el índice, mientras 4 páginas con 10432 sí. No extrapoles fuera del
rango medido.

El índice también puede no encontrar la respuesta. Si los primeros resultados no
sirven, reformula la consulta antes de ampliar mucho `--n` o abrir páginas.
