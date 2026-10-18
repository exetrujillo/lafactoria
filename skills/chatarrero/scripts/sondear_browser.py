#!/usr/bin/env python3
"""Sondea una URL con Playwright (nivel 4 de la cascada) y guarda el resultado mínimo.

uso: sondear_browser.py URL SALIDA [--headful]

Headless por defecto; --headful exige DISPLAY. SONDEO_LOCALE fija el locale
del contexto (por defecto es-CL). SALIDA recibe status=, title=, url=,
challenge=si|no y body= (primeros 2000 caracteres), o error= si la carga falló.
"""

import asyncio
import os
import sys
from pathlib import Path

from playwright.async_api import async_playwright

USER_AGENT = (
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/{mayor}.0.0.0 Safari/537.36"
)
TITULOS_CHALLENGE = ("Just a moment...", "Un momento")
MARCAS_CHALLENGE = ("cf-challenge", "challenges.cloudflare.com")
BODY_MAX = 2000


def hay_challenge(title, html):
    return any(t in title for t in TITULOS_CHALLENGE) or any(m in html for m in MARCAS_CHALLENGE)


async def main():
    if len(sys.argv) not in (3, 4) or (len(sys.argv) == 4 and sys.argv[3] != "--headful"):
        raise SystemExit("uso: sondear_browser.py URL SALIDA [--headful]")
    url, output = sys.argv[1:3]
    headful = len(sys.argv) == 4
    if headful and not os.environ.get("DISPLAY"):
        raise SystemExit("--headful exige DISPLAY: sin display el navegador no abre ventana (levanta Xvfb o quita --headful)")
    async with async_playwright() as playwright:
        browser = await playwright.chromium.launch(headless=not headful)
        context = await browser.new_context(
            user_agent=USER_AGENT.format(mayor=browser.version.split(".")[0]),
            locale=os.environ.get("SONDEO_LOCALE", "es-CL"),
            ignore_https_errors=True,
        )
        page = await context.new_page()
        try:
            response = await page.goto(url, wait_until="domcontentloaded", timeout=30000)
            await page.wait_for_timeout(4000)
            title = await page.title()
            html = await page.content()
            body = await page.locator("body").inner_text()
            result = (
                f"status={response.status if response else ''}\n"
                f"title={title}\n"
                f"url={page.url}\n"
                f"challenge={'si' if hay_challenge(title, html) else 'no'}\n"
                f"body={body[:BODY_MAX]}\n"
            )
        except Exception as exc:
            result = f"error={type(exc).__name__}: {str(exc).splitlines()[0] if str(exc) else ''}\n"
        finally:
            await browser.close()
    Path(output).write_text(result, encoding="utf-8")
    print(result.splitlines()[0])
    if result.startswith("error="):
        raise SystemExit(1)


asyncio.run(main())
