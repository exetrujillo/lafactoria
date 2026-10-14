# Captcha manual: el usuario resuelve, el scraper sigue

Origen: patrón documentado, sin medición propia todavía. Las llamadas a
`nodriver` están verificadas contra su código fuente (2026-10). La primera vez
que se use en un sitio real, el resultado va a `vivencias/` y, si se confirma,
se anota acá.

## Cuándo

- El challenge pide una interacción que la automatización no supera: captcha
  de imágenes (reCAPTCHA v2, hCaptcha), un Turnstile que no se resuelve solo,
  un "verify you are human" que vuelve a aparecer.
- El nivel 5 automático (`nodriver`, `verify_cf` y las alternativas de
  `cascada.md`) ya se intentó y no pasó, o seguir probando herramientas cuesta
  más que pedirle un minuto al usuario.
- El dato no está detrás de un login ni de un pago. Resolver un captcha no es
  presentar una credencial; un formulario de acceso sí lo es, y ahí sigue
  valiendo la regla de no inventar credenciales.

No es para cada ítem de un lote: una resolución tiene que cubrir muchas
peticiones, o el costo para el usuario no se sostiene.

## Requisitos

- Un display gráfico donde el usuario vea la ventana: `DISPLAY` o
  `WAYLAND_DISPLAY` definidos. Bajo Xvfb nadie ve nada. En una sesión por SSH o
  en un servidor, ofrece reenvío X (`ssh -X`) o un VNC; si no hay ninguno,
  dilo y no simules la resolución.
- El usuario disponible en ese momento. Se le pregunta con la herramienta
  disponible para hacer preguntas al usuario **antes** de abrir la ventana.

## Procedimiento

1. **Pedir el momento.** Explica qué sitio es, qué va a ver y cuántos ítems
   cubre la sesión. Si el usuario no está, el lote espera; mientras tanto no se
   insiste por HTTP.
2. **Abrir un navegador visible con perfil persistente.** Headful sobre el
   display real, con un `user_data_dir` propio del scraper, fuera del repo o
   ignorado por git. El perfil conserva la sesión entre corridas y hace que el
   sitio pida menos captchas después. El navegador es el de
   `navegador_por_defecto` en los ajustes.
3. **Navegar a la URL que dispara el challenge** y avisar al usuario que lo
   resuelva en la ventana.
4. **Esperar el contenido, no la ausencia del captcha.** Polling cada 1-2 s
   del selector o el texto que prueba que llegó el dato, con techo (unos 5
   minutos). Una página puede seguir cargando scripts de reCAPTCHA para un
   formulario y estar ya resuelta; lo que decide es el contenido. Si se cumple
   el techo, pregunta al usuario si sigue; no reintentes solo.
5. **Exportar la sesión**: las cookies y el **User-Agent exacto** del
   navegador, como una unidad, a un archivo fuera de git. Son secretos de
   sesión: nunca van al crudo, al log ni a la bitácora.
6. **Seguir el lote**, de una de dos maneras:
   - en el mismo navegador, si son pocos ítems o el sitio revalida en cada
     página;
   - bajando al nivel 2 con el par cookie + User-Agent (el puente de
     `cascada.md`): misma IP y mismo UA, y la cookie caduca.
7. **Cuando caduca** (reaparece el challenge, `cf-mitigated: challenge` o un
   403 que antes no estaba), se vuelve al paso 1. No se reintenta por HTTP ni
   se rota el UA: eso invalida la sesión.

Los lotes se agrupan por dominio, con una resolución por dominio y por sesión,
no por ítem. La cortesía no cambia con la sesión resuelta: las pausas de
`ajustes.json` siguen valiendo, y un captcha resuelto no habilita ráfagas.

## Esqueleto con `nodriver`

```python
import json, os, pathlib, time
import nodriver as uc

async def sesion_manual(url, selector_listo, perfil, salida, techo_s=300):
    if not (os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")):
        raise SystemExit("sin display: el usuario no vería la ventana del captcha")
    browser = await uc.start(headless=False, user_data_dir=str(perfil))
    tab = await browser.get(url)
    print("Resuelve el captcha en la ventana del navegador.")
    limite = time.monotonic() + techo_s
    while time.monotonic() < limite:
        try:
            await tab.select(selector_listo, timeout=2)
            break
        except Exception:
            await tab.sleep(1)
    else:
        raise SystemExit("techo cumplido sin contenido: preguntar antes de seguir")
    ua = await tab.evaluate("navigator.userAgent", return_by_value=True)
    cookies = await browser.cookies.get_all()
    pathlib.Path(salida).write_text(json.dumps({
        "user_agent": ua,
        "cookies": [{"name": c.name, "value": c.value, "domain": c.domain, "path": c.path}
                    for c in cookies],
    }))
    return browser, tab

# uc.loop().run_until_complete(sesion_manual(...)), como indica el README de nodriver
```

`browser.cookies.get_all(requests_cookie_format=True)` entrega las cookies como
objetos de `http.cookiejar`, listos para una sesión de `requests`.
`browser.cookies.save(archivo)` y `load(archivo)` las persisten sin pasar por
JSON. En los dos casos, el archivo queda fuera de git.
