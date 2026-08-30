#!/usr/bin/env python3
"""Sondea una URL con Chromium real mediante nodriver."""

import asyncio
import os
import sys
from pathlib import Path
import nodriver as uc


async def main():
    if len(sys.argv) != 3:
        raise SystemExit("uso: sondear_nodriver.py URL SALIDA")
    url, output = sys.argv[1:]
    chrome_bin = os.environ.get("CHROME_BIN")
    if not chrome_bin:
        raise SystemExit("falta CHROME_BIN: ruta al binario de Chrome/Chromium")
    browser = await uc.start(
        headless=False,
        sandbox=False,
        browser_executable_path=chrome_bin,
    )
    try:
        tab = await browser.get(url)
        await tab.sleep(15)
        pdf_output = os.environ.get("PDF_OUTPUT")
        if pdf_output:
            await tab.download_file(url, filename=pdf_output)
        title = tab.title
        content = await tab.get_content()
        links = await tab.evaluate("Array.from(document.querySelectorAll('a')).map(a => a.href).filter(h => /pdf|download/i.test(h)).slice(0, 20)")
        Path(output).write_text(
            f"title={title}\nurl={tab.url}\nlinks={links}\ncontent={content[:10000]}\n",
            encoding="utf-8",
        )
        print(f"title={title} url={tab.url} bytes={len(content)}")
    finally:
        browser.stop()


asyncio.run(main())
