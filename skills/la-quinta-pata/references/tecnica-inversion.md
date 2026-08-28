# Técnica: inversión y sombrero negro

La inversión convierte el objetivo en un fracaso observable y busca qué tendría
que ocurrir para producirlo. Es una adaptación cualitativa de *misuse cases*,
FMEA y threat modeling: no demuestra exhaustividad, pero obliga a recorrer una
cadena causal en vez de enumerar preocupaciones.

## Procedimiento

1. Escribe el objetivo y su negación observable: "el resultado falla cuando...".
2. Enumera inputs, estados, transiciones, incentivos e interpretaciones que pueden
   llevar a esa negación.
3. Separa ataque activo (alguien intenta forzar el fallo) de deterioro pasivo
   (error, carga, cambio, omisión o degradación sin adversario).
4. Para cada candidato, completa condición -> mecanismo -> daño y ancla cada hecho
   en el material. No completes los saltos con intuición.
5. Conserva como hallazgo solo la cadena con consecuencia relevante y una prueba o
   estado que pueda refutarla. Deduplica causas que produzcan el mismo riesgo.

No es una lista de ataques ni una afirmación de que todo fallo posible sea
probable. Si el objetivo no tiene una condición de fracaso observable, pide
aclaración o marca el análisis como no determinable.

**Ejemplo:** si el objetivo es "decidir con datos completos" y una transición
acepta un archivo sin validar su versión, el hallazgo requiere mostrar cómo esa
versión llega a la decisión y qué prueba (por ejemplo, rechazo de versión inválida)
lo falsaría. "Podría haber datos malos" no alcanza.

**Fundamentos:** C. W. Johnson, *Misuse Cases: The Past, the Future* (2002);
IEC 60812:2018, *Failure modes and effects analysis (FMEA and FMECA)*;
MITRE ATLAS, https://atlas.mitre.org/; OWASP, *LLM01:2025 Prompt Injection*,
https://genai.owasp.org/llmrisk/llm01-prompt-injection/.
