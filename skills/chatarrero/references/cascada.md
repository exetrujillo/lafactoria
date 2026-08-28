# Cascada de fetching: detalle por nivel

Se recorre de barato a caro. Un nivel "funciona" solo si devuelve el contenido
buscado de verdad: texto real, no un 200 con shell vacío ni una página de
challenge. Cada táctica indica su origen: medición de producción o doc oficial.

## Nivel 0 — API o web service oficial

El nivel más barato y estable. Se busca antes que nada.

- Sondear: `robots.txt` (suele listar sitemaps), `/sitemap.xml`, rutas obvias
  (`/wp-json/wp/v2/posts` en WordPress, `/api/`), documentación pública, y la
  pestaña Network del navegador mientras se usa el sitio.
- Si el sitio publica web service, usarlo siempre primero: más rápido, estable
  y citable. Medido en producción: web services institucionales que funcionan
  bien mientras sus fichas web renderizadas devuelven cuerpo vacío.
- **WAF que filtra parámetros**: medido en WordPress REST tras Cloudflare — el
  WAF bloqueaba ciertos parámetros (`search`, `per_page`) pero dejaba pasar
  otros (`page`, `_fields`) → paginar sin filtrar y filtrar client-side.
- **Burst-rate con auto-limpieza**: ráfagas largas daban 403 que se limpiaba
  solo en ~25–30 s → encolar esas páginas y reintentarlas una vez tras el pase
  principal con fail-fast (`max_attempts=1`).
- **CGIs de búsqueda de CMS** (tipo Prontus): POST form-urlencoded con UA de
  navegador + `Referer` del propio sitio + `Content-Type` correcto; si falta
  alguno devuelven HTTP 200 con 0 bytes.
- El fin real de paginación WordPress es HTTP **400**, no cualquier excepción.

## Nivel 1 — RSS/feed

- Con cuerpo completo (`<content:encoded>`): un solo request por ciclo.
- Con solo resumen: feed + fetch por artículo, con fallback al resumen del
  feed si el fetch falla o ningún selector matchea.
- Feed con flag `bozo` pero entradas recuperables → WARNING, no descarte.
- Si el RSS/API da 403 pero el sitio HTML pasa: buscar la vía alternativa
  antes de subir de nivel — medido: feeds bloqueados cuya búsqueda interna
  (`/?s=...`) y artículos se servían sin challenge.

## Nivel 2 — HTTP simple (requests / axios + parser)

El caballo de batalla. Configuración mínima probada:

- **Headers**: UA realista (pool de ≥5, rotada por sesión), `Accept` completo,
  `Accept-Language` del país objetivo, `Referer` (Google o el propio sitio
  según el WAF). Cloudflare responde 403 al UA default de urllib.
- **Dos tiers de headers**: primer intento con UA mínima; reintentos con
  headers completos de navegador (medido en corridas masivas).
- **Cascada de conexión** para sitios de calidad variable (medida re-fetchando
  en vivo 280 fallos reales): `https verify=True` → si SSLError `verify=False`
  (dejar constancia en metadata) → si sigue fallando, reintentar en `http://`.
  ROI en scheme-fallback y verify-false; subir el número de reintentos aporta poco.
- **4xx permanente** (400/401/404/410): no se reintenta ni hace fallback.
- **DNS**: pre-chequeo con `getaddrinfo` solo para NXDOMAIN autoritativo
  (`EAI_NONAME`); `EAI_AGAIN` no autoriza a concluir nada. Si el apex no
  resuelve, probar `www.<host>` una vez (rescató ~4,5% de los NXDOMAIN).
- **Encoding**: sin charset declarado, `requests` cae a ISO-8859-1 y produce
  mojibake → validar sobre los bytes y forzar UTF-8 si decodifica limpio.
  Instalar `brotli` si el servidor anuncia `br`, o la respuesta llega
  indescifrable en silencio.
- **Retries**: 3 intentos con backoff exponencial (2/4/8 s); ante 403/429 de
  Cloudflare sumar sleep largo (~60 s) que cuenta como intento fallido.
- **Atrapar `Exception` genérica además de `RequestException`**: urllib3 lanza
  `LocationParseError` fuera de la jerarquía y un solo dominio tóxico puede
  tumbar el lote entero.
- **Escalada por fingerprint TLS**: si el WAF bloquea por huella TLS/JA3 y no
  por contenido, `curl_cffi` es un reemplazo drop-in de requests que impersona
  navegadores (`impersonate="chrome"`), con API idéntica, asyncio y websockets.
  Su CLI (`curl-cffi get URL --impersonate chrome`) sirve para sondear barato
  antes de armar código. Límite conocido: contra Cloudflare Turnstile no bastó;
  ahí hay que subir a navegador.
- **Parsing defensivo**: cascada de selectores con fallback (OG tags, meta
  keywords, `<time>`), y parseo de fechas en texto local con tablas de meses.
- Descubrimiento de URLs: listing por sección/tag, búsqueda interna del sitio,
  sitemap de Google News.

## Nivel 3 — Endpoints descubiertos (interceptar tráfico)

Cuando el HTML descargado no trae el dato pero el navegador lo muestra:

- Interceptar con `page.on("request"/"response")` + `goto(wait_until="networkidle")`
  y buscar XHR/JSON internos. Medido en producción: así se han encontrado
  endpoints con parámetros que la página visible consume en silencio.
- Si el endpoint exige cookie de sesión: visitar primero la página contenedora
  y reutilizar el contexto (`ctx.request.get` hereda cookies).
- El endpoint descubierto suele ser más estable que raspar el DOM, y a veces
  permite volver a bajar al nivel 2 con ese endpoint + headers correctos.

## Nivel 4 — Render con Playwright (SPAs)

Subir aquí solo con diagnóstico: HTML grande (≥2000 chars) pero <200 chars de
texto visible = shell JS que inyecta contenido. Medido en producción: shells
JS que parecían "sin datos" generaban identidades vacías en silencio.

- `wait_until="domcontentloaded"` + settle fijo (~4 s), **nunca `networkidle`**:
  con analytics/ads/websockets nunca llega (medido: 41 s/it vs. fracción) y la
  documentación oficial de Playwright lo marca DISCOURAGED.
- `page.add_locator_handler(locator, handler)` para overlays predecibles
  (banners de cookies, popups de registro): Playwright ejecuta el handler cada
  vez que el overlay aparece antes de una acción, y el scrape sigue sin
  intervención manual.
- `ignore_https_errors=True`: sitios chicos con certificado vencido o de
  hostname equivocado son comunes (~28% de los fallos de render medidos).
- Timeout (~30 s); si se cumple pero el render parcial supera el umbral de
  texto, guardar igual (mejor que nada).
- Guardar el HTML renderizado junto al original (`index_rendered.html`) y que
  los consumidores aguas abajo lo prefieran sin cambios; invalidar derivados
  stale del folder.
- UA de navegador real + locale del país.
- Si el entorno es cluster/servidor: pinnear la versión de Playwright y
  recordar `playwright install chromium chromium-headless-shell` (el headless
  shell es binario separado en versiones nuevas).
- El flujo puntual de navegador (descarga con `expect_download`, PDFs, etc.)
  puede vivir en un script aparte con venv propio para no volver Playwright
  requisito del resto del proyecto.

## Nivel 5 — Navegador anti-bot (Cloudflare Turnstile / WAF duro)

Último recurso, el más caro y frágil. Todo lo probado:

- **Headless es detectado**: Turnstile lo bloquea al instante → headful
  obligatorio, bajo `Xvfb :99` levantado a mano en el entrypoint
  (`xvfb-run` puede colgarse en imágenes playwright).
- Flags: `headless=False`, `sandbox=False`/`--no-sandbox`,
  `--disable-dev-shm-usage`, `--disable-gpu` para Docker rootless.
- **Playwright stealth** (`--disable-blink-features=AutomationControlled`,
  borrar `navigator.webdriver`, locale/timezone reales): probado en
  producción, **no bastó** contra Turnstile.
- **`curl_cffi` con `impersonate="chrome"`**: probado, no bastó para el caso
  duro; puede servir para WAFs más blandos.
- **`nodriver`** (undetected Chromium): el único que pasó Turnstile en
  nuestras pruebas. Race condition de arranque: reintentar el endpoint
  `version` de la API HTTP hasta 30×0,5 s. Proyecto activo; desde la versión
  0.50 opera en "flat mode" e incluye iframes en `tab.find()`/`tab.select()`,
  así que puede localizar el checkbox "verify you are human" dentro del iframe
  del challenge; `tab.cf_verify()` lo encuentra y lo clickea solo (requiere
  `opencv-python`, solo fuera de expert mode), y
  `tab.bypass_insecure_connection_warning()` cubre certificados inválidos.
  Licencia AGPL-3.0: sin problema para herramientas privadas, pero ojo si el
  scraper se distribuye.
- **Detección de challenge por markers**: título `"Just a moment..."`/
  `"Un momento"`, contenido `cf-challenge`/`challenges.cloudflare.com` →
  polling (~1 s paso, 15 s techo) dejando que el navegador lo resuelva solo,
  luego backoff y reintento. `wait_selector` para re-capturar el DOM después
  del challenge.
- Portada que a veces carga sin cards: re-navegar hasta 3 intentos.
- **Si el bloqueo es a la automatización misma** (logins que bloquean CDP):
  Chromium real headful + sesión real persistida en volumen + extensión de
  navegador propia que manipula el DOM.

## Cuándo NO subir de nivel

- 4xx permanente: ningún transporte lo arregla.
- DNS autoritativamente muerto (NXDOMAIN también desde otra red).
- Paginación servida desde caché de CDN del lado del sitio: el navegador no la
  resuelve.
- Límites del servidor: respuestas 200 con 0 bytes que persisten en cualquier
  modalidad → conviene reintentar otro día antes de concluir que el dato no
  existe.
- Sitios que bloquean todo menos browsers pero con robots.txt/señales que
  prohíben el scraping: la decisión es del dueño del proyecto, documentada,
  no técnica.
