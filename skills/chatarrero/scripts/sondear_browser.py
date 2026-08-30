#!/usr/bin/env python3
"""Sondea una URL pública con Playwright y guarda el resultado mínimo."""

import asyncio
import sys
from pathlib import Path

from playwright.async_api import async_playwright


async def main():
    if len(sys.argv) not in (3, 4):
        raise SystemExit("uso: sondear_browser.py URL SALIDA [--headful]")
    url, output = sys.argv[1:3]
    headless = len(sys.argv) == 3
    async with async_playwright() as playwright:
        browser = await playwright.chromium.launch(headless=headless)
        context = await browser.new_context(
            user_agent="Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/131 Safari/537.36",
            locale="en-US",
        )
        page = await context.new_page()
        response = await page.goto(url, wait_until="domcontentloaded", timeout=30000)
        await page.wait_for_timeout(10000)
        body = await page.locator("body").inner_text()
        result = (
            f"status={response.status if response else ''}\n"
            f"title={await page.title()}\n"
            f"url={page.url}\n"
            f"body={body[:2000]}\n"
        )
        Path(output).write_text(result, encoding="utf-8")
        await browser.close()


asyncio.run(main())
