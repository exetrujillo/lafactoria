# Bitácora de hallazgos de scraping

Registro fechado de qué funcionó y qué no, para no repetir intentos ya
descartados. **Regla: actualizar esta sección cuando algo cambie.**

Formato de entrada: fecha aprox. de verificación | caso | hallazgo | origen
(medición de producción o documentación pública). Los casos se describen por
tipo de sitio, no por nombre.

## Tácticas medidas en producción

- ~2026-07 | Cloudflare Turnstile | Playwright stealth y `curl_cffi` no
  bastan; `nodriver` headful bajo Xvfb sí pasa. Headless es detectado al
  instante. | producción
- ~2026-07 | WordPress REST tras Cloudflare | el WAF bloquea parámetros
  `search`/`per_page` pero deja pasar `page`/`_fields`; los 403 por burst-rate
  se auto-limpian en ~25–30 s → reintentar tras el pase principal con
  fail-fast. | producción
- ~2026-07 | portal con feed protegido | el feed da 403 pero la búsqueda
  interna del sitio y los artículos pasan sin challenge. | producción
- ~2026-07 | sitios sin charset declarado | requests cae a ISO-8859-1 →
  forzar UTF-8 validando los bytes. | producción
- ~2026-07 | servidores que anuncian `br` | sin `brotli` instalado la respuesta
  llega indescifrable en silencio. | producción
- ~2026-07 | CGI de búsqueda de CMS | responde 200 con 0 bytes si falta
  UA + Referer + Content-Type form-urlencoded. | producción
- ~2026-07 | widgets embebidos en artículos | contaminan el contenido limpio y
  causan falsos positivos aguas abajo; fix: contenedor acotado + descarte de
  clase + truncar en el literal. Perseguir variantes por clase CSS no escala.
  | producción
- ~2026-08 | corrida masiva de homepages | 33% de fallos de fetch; de 280
  re-fetchados en vivo: 15,7% cura con fallback `http://`, 8,6% con
  `verify=False`, 57% son dominios muertos. Más reintentos aportan poco.
  | producción
- ~2026-08 | SPAs en general | detección por razón de texto (HTML ≥2000 chars
  y <200 de texto visible); `networkidle` nunca llega con analytics —
  `domcontentloaded` + 4 s settle (41 s/it vs. fracción). | producción
- ~2026-08 | web services institucionales | suelen funcionar donde las fichas
  web renderizadas devuelven cuerpo vacío; endpoints que responden 200 con
  0 bytes pueden ser transitorios — reintentar otro día antes de concluir que
  el dato no existe. | producción
- ~2026-08 | sitios que 403 sin UA | con requests + UA de navegador suele
  bastar, más barato que browser; la paginación servida desde caché de CDN del
  lado del sitio no la resuelve el navegador. | producción
- ~2026-08 | logins que bloquean CDP | el camino verificado es Chromium real
  headful + sesión persistida + extensión propia que manipula el DOM.
  | producción
- ~2026-08 | redes profesionales agresivas | no atacar de frente: snippets de
  buscador, o crawl con pausa larga (~8 s) y circuit breaker tras varios
  bloqueos seguidos; los negativos por bloqueo no se cachean. | producción
- ~2026 | sitios chicos en general | certificados vencidos o de hostname
  equivocado son frecuentes: `ignore_https_errors=True` en Playwright y
  `verify=False` trazado en metadata para HTTP (~28% de los fallos de render).
  | producción
- ~2026 | urllib (stdlib) | Cloudflare responde 403 al User-Agent default de
  urllib; siempre UA custom. | producción

## Verificación web (2026-08-28)

- 2026-08-28 | Playwright (docs oficiales) | `wait_until="networkidle"` está
  marcado **DISCOURAGED** en la API oficial — confirma la táctica de usar
  `domcontentloaded` + settle fijo. | playwright.dev/python, API class-page
- 2026-08-28 | nodriver | proyecto activo (sucesor de undetected-chromedriver,
  sin selenium). Desde v0.50: flat mode con iframes incluidos en
  `find()`/`select()`, `tab.cf_verify()` (requiere opencv-python),
  `tab.bypass_insecure_connection_warning()`, guardar/cargar cookies.
  Licencia AGPL-3.0. | github.com/ultrafunkamsterdam/nodriver
- 2026-08-28 | curl_cffi | proyecto activo, MIT. Impersona fingerprints
  TLS/JA3/HTTP2 (y HTTP/3 desde v0.15), API compatible con requests, trae CLI
  (`curl-cffi get URL --impersonate chrome`) útil para sondear. Mínimo Python
  3.10. | github.com/lexiforest/curl_cffi
- 2026-08-28 | Playwright (docs oficiales) | `page.add_locator_handler()`
  (v1.42+) remueve overlays predecibles automáticamente antes de cada acción
  — útil contra banners de cookies/popups que tapan el contenido a scrapear.
  | playwright.dev/python, API class-page
