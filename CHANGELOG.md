# Changelog

Todos los cambios notables de este proyecto se documentan en este archivo.

## [1.0.1] - 2026-08-28

### Cambiado

- `.gitignore` excluye los artefactos que genera el flujo de trabajo interno y
  que no se versionan: `docs/literatura/` (material de referencia local),
  `experimentos/` (contratos, bancos, ledgers y crudos de `prueba-y-error`) y
  las cachés de Python de los scripts de las skills. El resumen versionado de
  cada experimento vive aparte, en `docs/experimentos/`.

## [1.0.0] - 2026-08-27

### Agregado

- `skillcheck`: binario Rust sin dependencias externas, con subcomandos
  `lint` (valida frontmatter, cuerpo, referencias a archivos y nombres
  duplicados de las skills) e `install` (publica una skill en
  `.claude/skills/` del proyecto o, con `--global`, en `~/.claude/skills/`).
- Skill maestra `forjador`, que guía la creación de nuevas skills de punta a
  punta: aclarar propósito, nombrar, escribir, validar, instalar e iterar.
- `README.md` y `CLAUDE.md` con la documentación de comandos y arquitectura
  del repo.
- `.gitignore` para el proyecto Rust y overrides locales de Claude Code.
