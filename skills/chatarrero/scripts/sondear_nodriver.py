#!/usr/bin/env python3
"""Sondea una URL con Chromium real mediante nodriver (nivel 5 de la cascada).

uso: sondear_nodriver.py URL SALIDA [--headless]

Arranca headful, que es lo que pasa Turnstile, y por eso exige DISPLAY o
WAYLAND_DISPLAY (Xvfb sirve). --headless es solo para pruebas del script:
Turnstile detecta headless y lo bloquea. CHROME_BIN fija el binario; sin él
nodriver busca chrome o chromium en el sistema. SONDEO_LOCALE fija el idioma
del navegador (por defecto es-CL).

Con PDF_OUTPUT=ruta, después de cargar URL busca en la página el primer
enlace a PDF y lo descarga en esa ruta. URL tiene que ser la página que
contiene el enlace, no el PDF: download_file hace un fetch desde el JS de la
página ya cargada, así que pedir el PDF como URL y después descargarlo lo
pediría dos veces. Si la URL ya es el PDF, el navegador lo abre en su visor y
la salida lo dice en pdf= sin descargar nada.

SALIDA recibe las mismas claves que sondear_browser.py: status= (del
documento principal, capturado por CDP), title=, url=, challenge=si|no y
body= (primeros 2000 caracteres), más pdf= y links= con PDF_OUTPUT; o error=
si la carga falló.
"""

import os
import re
import sys
import time
from pathlib import Path

import nodriver as uc
from nodriver import cdp

TITULOS_CHALLENGE = ("Just a moment...", "Un momento")
MARCAS_CHALLENGE = ("cf-challenge", "challenges.cloudflare.com")
BODY_MAX = 2000
CHALLENGE_TECHO_S = 15
DESCARGA_TECHO_S = 30
JS_LINKS_PDF = (
    "Array.from(document.querySelectorAll('a[href]')).map(a => a.href)"
    ".filter(h => /\\.pdf(\\?|$)|download/i.test(h)).slice(0, 20)"
)


def hay_challenge(title, html):
    return any(t in title for t in TITULOS_CHALLENGE) or any(m in html for m in MARCAS_CHALLENGE)


async def js(tab, expresion):
    # evaluate devuelve el RemoteObject, no el valor, cuando la cadena es vacia
    valor = await tab.evaluate(expresion, return_by_value=True)
    return valor if isinstance(valor, str) else ""


async def js_lista(tab, expresion):
    # con return_by_value los arrays vuelven como RemoteObject; sin él, serializados
    valor = await tab.evaluate(expresion)
    return [x["value"] for x in valor] if isinstance(valor, list) else []


async def estado_pagina(tab):
    return await js(tab, "document.title"), await tab.get_content()


async def esperar_challenge(tab):
    limite = time.monotonic() + CHALLENGE_TECHO_S
    title, html = await estado_pagina(tab)
    while hay_challenge(title, html) and time.monotonic() < limite:
        await tab.sleep(1)
        title, html = await estado_pagina(tab)
    return title, html


async def descargar_pdf(tab, destino):
    links = await js_lista(tab, JS_LINKS_PDF)
    if await js(tab, "document.contentType") == "application/pdf":
        return f"pdf=no descargado: la URL ya es el PDF\nlinks={links}\n"
    if not links:
        return f"pdf=sin enlace a PDF en la pagina\nlinks={links}\n"
    destino = Path(destino)
    await tab.set_download_path(destino.parent)
    await tab.download_file(links[0], filename=destino.name)
    limite = time.monotonic() + DESCARGA_TECHO_S
    while not destino.exists() and time.monotonic() < limite:
        await tab.sleep(1)
    estado = str(destino) if destino.exists() else f"no llego en {DESCARGA_TECHO_S} s desde {links[0]}"
    return f"pdf={estado}\nlinks={links}\n"


async def sondear(browser, url):
    tab = browser.main_tab
    respuestas = []
    tab.add_handler(cdp.network.ResponseReceived, respuestas.append)
    await tab.send(cdp.network.enable())
    tab = await browser.get(url)
    title, html = await esperar_challenge(tab)
    if (await js(tab, "document.location.href")).startswith("chrome-error://"):
        codigo = re.search(r"ERR_[A-Z_]+", html)
        codigo = codigo.group(0) if codigo else "desconocido"
        if not codigo.startswith("ERR_CERT"):
            raise RuntimeError(f"pagina de error de Chrome: {codigo}")
        await tab.bypass_insecure_connection_warning()
        await tab.sleep(3)
        title, html = await estado_pagina(tab)
    if hay_challenge(title, html):
        try:
            await tab.verify_cf()
        except Exception as exc:
            print(f"verify_cf no pudo: {type(exc).__name__}: {exc}", file=sys.stderr)
        title, html = await esperar_challenge(tab)
    frame_principal = (await tab.get_frame_tree()).frame.id_
    status = next((r.response.status for r in reversed(respuestas)
                   if r.type_ == cdp.network.ResourceType.DOCUMENT and r.frame_id == frame_principal), "")
    body = await js(tab, "document.body ? document.body.innerText : ''")
    result = (
        f"status={status}\n"
        f"title={title}\n"
        f"url={await js(tab, 'document.location.href')}\n"
        f"challenge={'si' if hay_challenge(title, html) else 'no'}\n"
        f"body={body[:BODY_MAX]}\n"
    )
    if os.environ.get("PDF_OUTPUT"):
        result += await descargar_pdf(tab, os.environ["PDF_OUTPUT"])
    return result


async def main():
    headless = "--headless" in sys.argv[1:]
    args = [a for a in sys.argv[1:] if a != "--headless"]
    if len(args) != 2 or any(a.startswith("--") for a in args):
        raise SystemExit("uso: sondear_nodriver.py URL SALIDA [--headless]")
    url, output = args
    if not headless and not (os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")):
        raise SystemExit("sin display: nodriver headful exige DISPLAY o WAYLAND_DISPLAY (Xvfb sirve); --headless es solo para pruebas, Turnstile lo detecta")
    browser = await uc.start(
        headless=headless,
        sandbox=False,
        browser_executable_path=os.environ.get("CHROME_BIN"),
        lang=os.environ.get("SONDEO_LOCALE", "es-CL"),
    )
    try:
        result = await sondear(browser, url)
    except Exception as exc:
        result = f"error={type(exc).__name__}: {str(exc).splitlines()[0] if str(exc) else ''}\n"
    finally:
        browser.stop()
    Path(output).write_text(result, encoding="utf-8")
    print(result.splitlines()[0])
    if result.startswith("error="):
        raise SystemExit(1)


uc.loop().run_until_complete(main())
