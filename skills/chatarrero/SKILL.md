---
name: chatarrero
description: >
  Usa esta skill cuando el usuario quiera crear un scraper nuevo o
  modificar/arreglar uno existente para extraer datos de un sitio web. No es
  para descargas puntuales de una sola página ni para consumir APIs ya
  documentadas del propio stack. Al invocarse no arranca sola: abre con una
  compuerta de preguntas, busca scrapers existentes en el repo y dirige el
  reconocimiento del sitio con subagentes de prueba puntuales para elegir el
  nivel más barato de la cascada de fetching —API o RSS, HTTP simple,
  endpoints descubiertos, render con Playwright o navegador anti-bot— antes
  de escribir código.
---

# Chatarrero

Guía para crear o reparar scrapers eligiendo siempre el nivel más barato de la
cascada de fetching que devuelva contenido **verificado**. Frase que lo
resume: **un 200 con cuerpo vacío es un fracaso, no un éxito**.

## 0. Compuerta

Al invocarse no se escribe código. Primero busca por tu cuenta, sin preguntar:

- Scrapers existentes en el repo: grep por `requests`, `playwright`,
  `beautifulsoup`, `cheerio`, carpetas `scrapers/`, `cmds/`, `data_sources/`.
- Convenciones del proyecto: clases base, contrato de salida, dónde se guarda
  el crudo, settings de cortesía.
- Skills ya instaladas en `.claude/skills/` del proyecto: si hay una skill de
  navegador/fetching propia del repo, respetala y no la dupliques.

Después, una sola tanda de preguntas al usuario:

1. ¿Qué sitio y qué datos? ¿Corrida única o scraper recurrente? Si es una
   descarga puntual sin código, esta skill no es el camino: decilo y ofrecé
   hacer la descarga directamente.
2. Presentá los scrapers existentes que encontraste y preguntá: ¿editar uno de
   ellos o crear uno nuevo?
3. ¿Dónde se guarda la salida y con qué esquema/campos?

## 1. ¿Editar o crear de cero?

Criterios, en orden:

1. **Ya hay scraper del mismo sitio** → editar. Casi siempre selectores que
   rotaron. Antes lee la bitácora del proyecto (docs/handoff, CHANGELOG) para
   no reintentar tácticas ya descartadas.
2. **Hay scraper de un sitio de la misma familia/CMS** (WordPress, Prontus,
   Drupal, Astro, SPA del mismo framework) → duplicar y ajustar selectores y
   headers; la estructura ya está probada.
3. **El proyecto tiene clases base o contrato de scraper** → heredar/implementar
   el contrato con lo mínimo (constantes, selectores). No inventar arquitectura
   paralela.
4. **Nada de lo anterior** → crear de cero, imitando las convenciones de
   fetching que el repo ya use; si el repo está vacío, stack por defecto:
   Python + requests + BeautifulSoup, crudo al disco, salida estructurada.

## 2. Reconocimiento con subagentes

El nivel de la cascada se decide midiendo, no opinando. Lanza un subagente con
este contrato corto:

- Entrada: URL objetivo, qué dato se busca, tope duro de requests (≤20).
- Tarea: probar en orden los niveles de references/cascada.md hasta que uno
  devuelva el contenido buscado de verdad.
- Salida en ≤10 líneas: nivel mínimo que funcionó, evidencia (status, cantidad
  de texto/selectores que matchearon), bloqueos detectados (challenges, cuerpo
  vacío, WAF) y headers que hicieron falta.

El subagente sondea e informa; el código de producción lo escribe la
conversación principal con ese veredicto. Si el sondeo contradice la intuición
(el sitio "parecía" necesitar browser y respondió a requests), vale lo medido.

## 3. Cascada de fetching

Regla: se recorre de barato a caro y se sube solo cuando el nivel actual falla
**o devuelve contenido vacío/insuficiente**. Detalle completo de cada nivel,
con tácticas probadas en proyectos reales, en references/cascada.md.

| Nivel | Técnica | Sube si... |
|---|---|---|
| 0 | API o web service oficial | no existe o bloquea |
| 1 | RSS/feed con cuerpo completo | feed solo trae resumen |
| 2 | HTTP simple (requests/axios + parser) | 403 persistente, challenge, cuerpo vacío |
| 3 | Endpoints descubiertos (interceptar tráfico XHR) | el dato solo existe tras JS |
| 4 | Render Playwright (SPA) | hay WAF/anti-bot que detecta headless |
| 5 | Navegador anti-bot (headful + Xvfb / sesión real) | último recurso |

También hay que saber **no subir**: 4xx permanente, DNS muerto, caché de CDN
del lado del servidor y límites del servidor no se resuelven con más
tecnología. Esos casos y todos los hallazgos fechados están en
references/bitacora.md — leela antes de sondear y actualizala cuando un
sondeo nuevo cambie algo.

## 4. Reglas no negociables de cualquier scraper

- **Crudo siempre**: guardar la respuesta original junto al output procesado;
  es la evidencia citable y lo que permite reparsear sin re-descargar.
- **Clasificar el error antes de reintentar**: ssl/conexión/transitorio se
  reintenta con backoff; 4xx permanente (400/401/404/410) no se reintenta ni
  hace fallback.
- **Idempotencia = reanudar**: cargar URLs/IDs ya vistos antes de descargar;
  los estados de fallo son terminales salvo pasada explícita de reintento.
- **catch-log-continue**: un ítem malo loguea y se salta, nunca tumba el lote.
  Campo obligatorio faltante → log con marcador claro + skip.
- **Nunca inventar datos**: ante cambio de estructura devolver vacío + warning.
- **Cortesía**: delay entre requests (referencia 5 s), sleep adaptativo si la
  página no trae nada nuevo, circuit breaker tras varios bloqueos seguidos.
- **Fechas UTC ISO 8601 con `Z`**; URLs canónicas (sin utm, sin www, sin
  slash final) para deduplicar.
- **Documentar con números**: cada táctica adoptada o descartada se anota con
  su medición o su motivo, en la bitácora del proyecto.

## 5. Cierre

Con el nivel elegido: escribir el scraper según el criterio del paso 1,
probarlo contra el sitio real con pocas URLs, verificar que el output tenga el
contenido buscado (no solo status 200), y anotar en la bitácora del proyecto
qué nivel funcionó y cuáles quedaron descartados.
