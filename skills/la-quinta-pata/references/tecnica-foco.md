# Técnica: foco desplazado

El foco desplazado prueba si un detalle secundario tiene una vía real hacia el
objetivo. Se parece a un análisis de propagación en FMEA y a seguir una
restricción en STPA, pero no presupone que los efectos de segundo orden existan.

## Procedimiento

1. Elige un detalle que no sea central en la presentación, pero que tenga una
   conexión explícita con una entrada, estado, recurso o decisión.
2. Sigue la cadena: detalle -> cambio local -> propagación -> daño al objetivo.
3. Para cada flecha anota la evidencia, la condición de activación y el estado que
   puede bloquearla.
4. Detén la cadena en el primer salto no respaldado. No lo rellenes con una
   analogía o una intuición sobre "efectos mariposa".
5. Si no llega a una consecuencia relevante o no hay prueba discriminante,
   informa que no produjo un hallazgo sustantivo.

**Ejemplo:** un valor por defecto es secundario solo en apariencia si cambia la
selección de una rama y esa rama omite una validación. La cadena debe mostrar
ambas transiciones; señalar simplemente que "hay un default" no basta.

**Fundamentos:** IEC 60812:2018, *FMEA and FMECA*; N. Leveson, *Engineering a
Safer World* (2011), https://mitpress.mit.edu/9780262533690/.
