# Changelog

Todos los cambios notables de este proyecto se documentan en este archivo.

## [1.4.0] - 2026-08-28

### Agregado

- Banco de pruebas en `la-quinta-pata`: si el objeto es ejecutable se corre antes de abrir cualquier subagente —camino feliz, criterios contrastados contra su disciplina y falsadores ejecutados—, porque las cinco técnicas buscan qué rompe el objetivo y ninguna detecta un control demasiado estricto que rechaza el caso legítimo más frecuente.
- Estado de verificación obligatorio en cada hallazgo: `confirmado` si el falsador se ejecutó, con comando y salida; `plausible` si no se ejecutó, diciendo si fue por alcance o porque el objeto no es ejecutable; o `no determinable` si falta material.

### Cambiado

- La acción de cada riesgo debe caber en el contexto donde vive el objeto. Si presupone una pieza inexistente —un supervisor externo dentro del proceso que el propio agente controla, un revisor humano en un flujo autónomo— se marca `requiere cambio de contexto` en vez de presentarla como aplicable hoy.
- El veredicto declara si se auditó un objeto ejecutable sin ejecutarlo: no descalifica la auditoría, pero cambia cuánto pesa lo que encontró, porque una lectura solo ve incoherencias entre el código y lo que promete.

## [1.3.0] - 2026-08-28

### Agregado

- Skill `la-quinta-pata`, que audita lateralmente código, arquitectura, ensayos, argumentos y decisiones mediante técnicas de inversión, supuestos, foco desplazado, analogía estructural y contrario fuerte con premortem.
- Contrato de hallazgos con evidencia localizada, mecanismo causal, condición de refutación, confianza, mitigación y criterio de parada.

## [1.2.0] - 2026-08-28

### Agregado

- Skill `biblio-rata`, que indexa PDFs con SQLite FTS5 y devuelve fragmentos
  relevantes con referencias de página, junto con sus scripts, referencias de
  uso e instalación.
- Documentación autosuficiente de los experimentos que establecen el criterio
  de conveniencia de la skill y sus límites operativos.

## [1.1.0] - 2026-08-28

### Agregado

- `lint` valida que `name` cumpla el formato de nombre de OpenCode: minúsculas ASCII, dígitos y guiones simples, sin guion al principio ni al final, entre 1 y 64 caracteres. Cubierto por el test `accepts_only_opencode_skill_names`.
- `install` verifica que la copia instalada quede byte a byte idéntica a `skills/<nombre>` y falla con código 1 si no coincide, para que una instalación a medias no pase inadvertida.

### Cambiado

- Una `description` de más de 1024 caracteres pasa de advertencia a **error**, `lint` termina con código 1 en ese caso, porque ese es el límite que impone OpenCode.

### Eliminado

- El canal de advertencias de `lint`. `skillcheck` reporta solo errores y su salida pasa de `OK (N advertencia(s))` a `OK`, y de `FALLÓ: N error(s), M advertencia(s)` a `FALLÓ: N error(es)`. Lo que vale la pena comprobar bloquea; lo demás no se comprueba.

## [1.0.1] - 2026-08-28

### Cambiado

- `.gitignore` excluye los artefactos que genera el flujo de trabajo interno y que no se versionan: `docs/literatura/` (material de referencia local), `experimentos/` (contratos, bancos, ledgers y crudos de `prueba-y-error`) y las cachés de Python de los scripts de las skills. El resumen versionado de cada experimento vive aparte, en `docs/experimentos/`.

## [1.0.0] - 2026-08-27

### Agregado

- `skillcheck`: binario Rust sin dependencias externas, con subcomandos `lint` (valida frontmatter, cuerpo, referencias a archivos y nombres duplicados de las skills) e `install` (publica una skill en `.claude/skills/` del proyecto o, con `--global`, en `~/.claude/skills/`).
- Skill maestra `forjador`, que guía la creación de nuevas skills de punta a punta: aclarar propósito, nombrar, escribir, validar, instalar e iterar.
- `README.md` y `CLAUDE.md` con la documentación de comandos y arquitectura del repo.
- `.gitignore` para el proyecto Rust y overrides locales de Claude Code.
