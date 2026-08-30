# Changelog

Todos los cambios notables de este proyecto se documentan en este archivo.

## [1.9.0] - 2026-08-30

### Agregado

- `install` corre un gate de vivencias antes de copiar una skill: si la fuente tiene tanto `scripts/validar_ajustes.rs` como `vivencias/ajustes.json`, compila el validador con `rustc -O` y lo ejecuta contra ese `ajustes.json`, abortando la instalación con el mismo criterio que un error de `lint` si falla. Si falta cualquiera de los dos archivos, no valida nada y sigue de largo.
- `install` escribe `.factoria-origen` en la copia instalada con la ruta canónica de la skill fuente, después de verificar que la copia es idéntica a la fuente, para que una skill pueda ubicar su propia fuente sin depender de la copia efímera.
- README: documenta `familia` y su reciprocidad `criados` en `vivencias/ajustes.json` — `herencia` es una foto fija del fork, `crianza` es un acoplamiento vivo y por eso es la única que registra la relación inversa en el padre.

### Corregido

- `copy_dir_recursive` y `directories_equal` ignoran `__pycache__` al copiar e instalar una skill, en vez de fallar la comparación byte a byte por una caché de Python que no debería viajar a la copia instalada.

## [1.8.0] - 2026-08-29

### Cambiado

- `forjador` mueve el patrón de compuerta y la auditoría de lógica del cuerpo fijo de su `SKILL.md` a `references/patron-compuerta.md` y `references/auditoria-logica.md`, con punteros condicionales en vez de texto que se carga en cada corrida.
- `CLAUDE.md` refleja que `forjador` permite varias rondas de preguntas, sin repetirlas salvo ambigüedad real, y la sección "Reglas que aplica skillcheck" de `forjador` incorpora las tres reglas que le faltaban frente a `src/main.rs`: frontmatter delimitado, formato/longitud de `name` y límite de `description`.

### Agregado

- `forjador` scaffoldea `vivencias/` al nombrar una skill nueva y, cuando esa skill declara claves propias de `ajustes.json`, le escribe un validador en Rust sin dependencias. Primera instancia real: `skills/forjador/vivencias/`, con un ajuste de registro de lenguaje del propio usuario.

## [1.7.0] - 2026-08-29

### Cambiado

- `README.md` reescrito como fuente de verdad del sentido del repositorio: cuatro principios de diseño (vivencias que no se destruyen, lo que varía por usuario no se versiona, neutralidad entre arneses, economía de contexto), el ecosistema de relaciones entre skills (autointeracción, uso, estudio, herencia, crianza), el criterio de qué entra y qué no como skill nueva, el ciclo de vida completo (nacer, iterar, publicar, versionar, retirar) y el esquema de `vivencias/` con su ciclo de promoción a `references/`. `CLAUDE.md` se recorta a comandos, arquitectura de `skillcheck` y reglas del validador, y remite al README para el propósito. Arranca la migración hacia `2.0.0`, que se declara cuando el resto de los TODOs de la migración estén cerrados.

## [1.6.1] - 2026-08-28

### Corregido

- `prueba-y-error`: `ledger.py plan --fuentes-json` valida de inmediato que el JSON tenga la forma `{"ids": [...]}` con al menos un identificador de caso. Antes, una entrada mal formada (por ejemplo, referenciar el archivo del banco en vez de los ids de caso consultados) guardaba `fuentes: null` en el plan sin ningún error, y el problema solo aparecía después, de forma confusa, al comparar con el `resultado`. Los mensajes de discrepancia de `test_aplicado` y `fuentes` entre `plan` y `resultado` ahora muestran ambos valores en conflicto en vez de solo avisar que no coinciden.

## [1.6.0] - 2026-08-28

### Agregado

- Skill `chatarrero`, para crear y reparar scrapers: abre con una compuerta de preguntas, decide entre editar un scraper existente o crear uno nuevo según las convenciones del repo destino, sondea el sitio objetivo con subagentes de prueba puntuales y elige el nivel más barato de la cascada de fetching —API o RSS, HTTP simple, endpoints descubiertos, render con Playwright o navegador anti-bot— antes de escribir código.
- Cascada documentada nivel por nivel (`references/cascada.md`) con tácticas medidas en producción: escalada de headers, cascada de conexión para sitios inestables, clasificación de errores antes de reintentar, detección de SPAs por razón de texto, `domcontentloaded` + settle en vez de `networkidle`, escalada por fingerprint TLS y navegadores anti-detección para WAFs duros, además de los casos donde subir de nivel no sirve.
- Bitácora fechada (`references/bitacora.md`) de qué funcionó y qué no, para no repetir intentos descartados, sembrada con tácticas de producción y una verificación web de Playwright, nodriver y curl_cffi. Los casos se describen por tipo de sitio, sin nombres de proyectos ni objetivos concretos.

## [1.5.0] - 2026-08-28

### Agregado

- Skill `prueba-y-error`, un bucle de experimentación que convierte una decisión técnica en una medición en vez de una impresión: contrato congelado antes de empezar, predicción antes del dato, análisis declarado antes de correr, dos fases autónomas —tamizaje barato y confirmación pareada— y un ledger append-only en disco como única memoria.
- Arnés de procedencia bajo la regla de que el LLM no escribe ninguna cifra: recibos de consumo medidos con el reloj, métrica, intervalo, efecto y corrección múltiple recalculados desde el crudo, verificación de contrato, presupuesto y reporte, y `ronda.py` para encadenar una ronda entera en un solo comando. El ledger lleva tres registros por iteración —`plan` antes de correr, `resultado` derivado del recibo y del análisis, y `diagnostico` con la interpretación del agente—, y una ronda nueva se bloquea mientras queden resultados sin diagnosticar.
- `autoprueba.py`, que ejercita el arnés contra sus propios modos de fallo —métrica inflada a mano, crudo reescrito, costo subdeclarado, `run_id` reutilizado, separación inventada, diagnóstico huérfano o duplicado— en segundos y sin gastar un token.
- Política de continuidad autónoma: la decisión de medir cubre el ciclo completo, y un veredicto `SEGUIR MIDIENDO` relanza una ronda de diseño distinto en lugar de devolverle el control al usuario.

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
