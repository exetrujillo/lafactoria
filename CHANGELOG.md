# Changelog

Todos los cambios notables de este proyecto se documentan en este archivo.

## [1.22.2] - 2026-09-21

### Corregido

- El `CHANGELOG.md` y `references/fuentes.md` nombraban un proyecto particular de quien usa el repositorio al citar la evidencia de dos hallazgos sobre descarga. Se reemplaza por el tipo de caso; la evidencia técnica —cinco de seis fallas por landing HTML servida con `200`— queda intacta.
- Lo versionado tiene que servirle a cualquiera que clone. El README ya exige que un aprendizaje se promueva de `vivencias/` a `references/` sólo si puede describirse sin datos propios, por tipo de caso y no por nombre; esta es la misma regla aplicada al historial, que se había saltado el filtro.

## [1.22.1] - 2026-09-21

### Corregido

- La prosa versionada del repositorio mezclaba registros: 18 puntos repartidos en 11 archivos estaban escritos en voseo rioplatense mientras el resto usaba español neutro. Se unifica todo en español latinoamericano neutro. Alcanza a los tres lugares donde aparecía: prosa de skills (`forjador`, `prueba-y-error`, `pulpo-librero` y sus referencias), mensajes de error en Rust (`src/main.rs` y los cuatro `validar_ajustes.rs`) y la copia versionada de `forjador` en `.claude/skills/`.
- Importa más en los `SKILL.md` que en cualquier otro texto del repositorio, porque son instrucciones que un agente lee y de las que toma el registro con el que después le habla a quien lo usa. Lo versionado fija uno neutro; quien quiera otro lo declara en `registro_lenguaje`, la clave de ajustes de `forjador`, sin tocar los archivos compartidos.

## [1.22.0] - 2026-09-10

### Corregido

- `pulpo descargar` colapsaba en `unreadable_pdf` dos diagnósticos que su propio contrato ya describía como distintos: una respuesta que no es un PDF y un PDF que el extractor no puede abrir. `validate_pdf` los distinguía internamente —lo primero que hace es mirar si el cuerpo empieza con `%PDF-`— y después los unía en un solo estado, quedando el manifiesto en contra de lo que `references/contrato.md` promete. `PdfValidation` pasa de `Invalid` a `NotPdf` y `Unreadable`, y cada uno alimenta su estado.
- Importa porque el caso frecuente es el que quedaba mal etiquetado. Una landing HTML servida con `200` por un host autorizado no falla en transporte: muere recién en la comprobación, y aparecía como "el extractor no pudo abrir ninguna respuesta PDF", que se lee como PDF corrupto y manda a buscar copia en otro repositorio. En una ronda de descarga real fueron cinco de seis fallas, y las cinco se arreglaban raspando el enlace de la landing. El motivo de `not_a_pdf` ahora lo dice: "suele ser la landing en vez del archivo, raspar el enlace".

### Cambiado

- El comentario que documenta las pausas por defecto había quedado sobre las constantes de cabeceras al publicarse `1.20.0`, describiendo la constante equivocada. Se devuelve a su lugar.

## [1.21.0] - 2026-09-10

### Agregado

- `references/fuentes.md` documenta que **el estado de un proveedor caduca**. Onomázein entregó sin fricción el 2026-09-08 —tres obras suyas están en la biblioteca, bajadas de `ojs.uc.cl` con HTTP simple— y el 2026-09-10 el mismo DOI resuelve a otro dominio de la UC y devuelve `403` tras una "Verificación de Seguridad". Que una fuente haya entregado antes no prueba que entregue hoy, y un cambio de dominio del editor mueve una fuente entera de columna sin aviso.
- Nota sobre Zora (`zora.uzh.ch`), que usa **Anubis** y no Cloudflare: responde `200` con "Making sure you're not a bot!" y no emite las cabeceras que mira `is_bot_challenge`, así que cae como `http_error` en vez de `blocked_challenge`. Queda anotado como límite conocido de la detección, con el rescate medido en otro repositorio.
- La sección de repositorios institucionales suma el patrón de **OJS** —la landing es `/article/view/N` y el PDF cuelga de `/article/download/N/M`, con un segundo identificador que hay que leer del HTML— y, sobre todo, el aviso de que una landing HTML servida por un host autorizado no falla: baja con `200` y muere recién en el extractor, de modo que aparece como `unreadable_pdf` y se lee como "el PDF está roto" cuando nunca hubo PDF. Fueron cinco de seis fallas en una misma ronda de descarga.

## [1.20.0] - 2026-09-10

### Cambiado

- `pulpo descargar` se presenta como un navegador corriente y baja despacio, por defecto y sin que haya que pedirlo. El User-Agent pasa de `pulpo-librero/0.1` a uno de Chrome, `Accept` sigue priorizando el PDF pero acepta lo que aceptaría un navegador, se declara `Accept-Language: es-ES` —la biblioteca es hispana y varios repositorios negocian el idioma de la landing— y al seguir una redirección se manda como `Referer` la URL de la que se viene, en vez de ninguna. El motivo no es evasión: varios repositorios institucionales rechazan de plano al cliente que se anuncia como script aunque la obra sea de lectura libre. Contra un challenge real esto no alcanza, y el sondeo de ACM del 2026-09-09 lo dejó medido; `blocked_challenge` sigue siendo terminal y la compuerta sigue tratando el paywall, el login y el préstamo controlado como un no definitivo.
- La pausa entre peticiones sube de 1,1 s a 4,5 s, y a 9 s cuando dos peticiones seguidas van al mismo host, que antes no se distinguía. Además pasa a aplicarse *antes* de cada petición y no después, así la primera de la corrida no espera de arriba y la última no deja al proceso durmiendo sin motivo. El costo de una corrida no lo paga quien la lanza sino el servidor que la atiende, y bajar dos docenas de PDF del mismo repositorio universitario en ráfaga es la forma más rápida de que ese repositorio deje de atender a cualquiera.

### Agregado

- `--pausa-ms`, `--pausa-mismo-host-ms` y `--user-agent` en `pulpo descargar`, con sus claves opcionales `pausa_ms`, `pausa_mismo_host_ms` y `user_agent` en `vivencias/ajustes.json`, siguiendo el mismo camino que los límites de tamaño: los lee el agente y se los pasa al binario. Son opcionales a propósito, así que un `ajustes.json` escrito antes de que existieran sigue validando sin tocarlo. `--user-agent` vacío se rechaza en vez de mandar una cabecera vacía.
- El validador de vivencias comprueba el tipo de esas tres claves solo si están presentes, distinguiendo por primera vez "clave ausente" de "clave con el tipo equivocado". Un ajuste mal tipeado se ignoraría en silencio y la corrida saldría más agresiva de lo que el usuario pidió, que es justo el error que este cambio viene a evitar.
- `options` devuelve un struct `Opciones` en lugar de una tupla: con siete campos, la tupla dejaba de ser legible en el sitio de destructuración.

## [1.19.0] - 2026-09-10

### Agregado

- `pulpo descargar` distingue un challenge anti-bot de un fallo HTTP corriente. `is_bot_challenge` mira las cabeceras volcadas en busca de señales específicas del challenge (`cf-mitigated: challenge`, `server-timing: chlray`) y no la mera presencia de Cloudflare, porque hay sitios servidos por Cloudflare que responden `403` por un motivo legítimo. El manifiesto gana el estado `blocked_challenge`, terminal por definición: ninguna combinación de cabeceras lo pasa, así que probar otra ubicación del mismo proveedor solo gasta cortesía. La salida es buscar copia en otro host.
- `references/fuentes.md` incorpora la nota de sondeo de ACM Digital Library del 2026-09-09: 17 peticiones con 3 s de pausa sobre dos DOI con `is_oa=true`, y ningún nivel de `curl` funciona —ni el UA identificable con correo, ni el cookie jar tomado desde la landing, que también está bloqueada—. Queda documentado como proveedor bloqueado que no se reintenta, con el rescate por repositorio institucional como camino real.
- `cascada.md` de `chatarrero` documenta el puente del nivel 5 al nivel 2 —cosechar la cookie `cf_clearance` con un navegador una sola vez y seguir el lote en HTTP barato— con las tres condiciones que lo hacen fallar si se ignoran: la clearance está atada al User-Agent exacto que la obtuvo, está atada a la IP y caduca. Queda como pendiente de implementar, no como capacidad disponible.

### Cambiado

- La compuerta de `pulpo-librero` separa el control de acceso del bloqueo técnico. Un paywall, un login o un préstamo controlado siguen siendo un no definitivo que ninguna escalada convierte en un sí; un `403` a un artículo de lectura libre es detección de bots y ahí la cascada sí corresponde. La distinción se exige resolver con evidencia —`is_oa`, `oa_status`, licencia declarada— antes de escalar, y no con intuición.

## [1.18.0] - 2026-09-10

### Agregado

- `install` deja de destruir en silencio lo que solo existe en la copia instalada. Antes reemplazaba el directorio entero, así que cualquier archivo que la skill hubiera escrito mientras trabajaba —bitácoras, índices, registros de `vivencias/`— desaparecía sin aviso al reinstalar. Ahora `archivos_solo_en_destino` compara ambos árboles antes de copiar y, si encuentra alguno, `install` aborta enumerándolos y ofreciendo las tres salidas posibles. Es el principio 1 del repositorio aplicado a la propia herramienta: ese material no se elimina sin permiso del usuario.
- `install NOMBRE --adoptar-vivencias` resuelve el caso por el camino contrario: copia esos archivos a `skills/NOMBRE/` antes de reemplazar la copia instalada, de modo que la fuente adopta lo que la skill aprendió en vez de perderlo. Cada adopción se informa por su ruta.
- El parser de flags de `install` pasa a leer todas las opciones en vez de mirar solo la primera, y rechaza las desconocidas con un uso explícito. Antes `install x --adoptar-vivencias --global` habría ignorado el segundo flag sin decir nada.

### Cambiado

- `CLAUDE.md` documenta el flag nuevo y fija la convención de commits del repositorio: solo línea de asunto, sin cuerpo —la historia vive en este archivo—, y nunca con coautoría ni trailers que atribuyan el commit a un agente o arnés.

## [1.17.0] - 2026-09-01

### Agregado

- `scripts/internetarchive.rs` completa `pulpo-librero`: la skill prometía cubrir libros desde su primera publicación y hasta ahora sólo tenía scrapers de papers. Consulta `advancedsearch.php` y después la metadata de cada ítem encontrado, con `--query` para buscar o `--id` para pedir identificadores concretos, y fuerza `mediatype:texts` salvo que la consulta ya traiga el suyo, porque el catálogo mezcla libros con audio y video.
- El scraper distingue descarga abierta de préstamo controlado, que es el problema real de esta fuente y no se puede resolver desde la búsqueda: la restricción vive en la metadata del ítem. Un libro en préstamo expone igual un `Text PDF` de aspecto normal —48 MB en el caso medido— que responde `401` al pedirlo. Las filas restringidas se emiten con `oa_status=prestamo-controlado` y sin `pdf_url`, para que el orquestador las descarte con un motivo legible en vez de que la obra desaparezca sin explicación, y nunca se intenta su descarga.
- De los formatos PDF de un ítem se aceptan sólo `Text PDF` y `Additional Text PDF`. Las variantes `ACS` y `LCP` están cifradas aunque figuren como PDF, y `JPEG-Compressed PDF` e `Image Container PDF` son el escaneo en imagen: pasarían la comprobación del descargador y quedarían indexados vacíos. La diferencia se mide sola: en `dli.ernet.286194` el escaneo pesa 335 MB y el PDF con capa de texto del mismo libro pesa 29 MB.
- `references/fuentes.md` reemplaza su sección genérica de Internet Archive por la nota de sondeo del 2026-09-01, con los identificadores para reproducirla: el `401` del ítem restringido, el `206 application/pdf` del abierto, el ítem CC-BY cuyos únicos archivos son metadata —una licencia abierta no prueba que haya PDF— y `collection` llegando como string en un ítem y como arreglo en otro, que es para lo que existe `json_texts_or_text`.

## [1.16.0] - 2026-09-01

### Agregado

- `scripts/openalex.rs` gana el subcomando `resolver`, el puente entre el catálogo de descubrimiento y la descarga que hasta ahora se cruzaba a mano. Toma del catálogo las filas que ya tienen DOI pero todavía no tienen ubicación —vengan de otro scraper, de una bibliografía que trajo el usuario o de una corrida anterior—, pregunta por sus copias abiertas y devuelve un TSV con las mismas columnas, que vuelve al catálogo con `pulpo buscar --source openalex-doi`. Con `--solo-relevante` se limita a las filas ya aprobadas.
- El resolver agrupa hasta 50 DOI por consulta con el filtro `doi:a|b|c`, así que un catálogo entero cuesta unas pocas peticiones en vez de una por obra, y guarda el crudo de cada tanda como una línea de `{salida}.raw.jsonl`. Los DOI que OpenAlex no conoce se informan por su nombre al terminar: esa fila se queda sin ubicación en lugar de recibir una inventada.
- `references/fuentes.md` documenta por qué el puente es OpenAlex y no Unpaywall: el sondeo del 2026-09-01 mostró que Unpaywall exige el parámetro `email` en toda llamada y rechaza los dominios de relleno con `422`, de modo que cada corrida dependería del correo real de quien la ejecuta. OpenAlex ingiere los mismos datos y responde sin credencial.

### Corregido

- La copia `scripts/json_api.rs` de `pulpo-librero` quedó desincronizada de `src/json_api.rs` por una línea de comentario al publicarse `1.15.0`. Se propaga la canónica, que es justamente el caso que `check_copias_canonicas` existe para detectar.

## [1.15.0] - 2026-09-01

### Agregado

- `src/json_api.rs`: segunda copia canónica, un lector de JSON anidado para respuestas de API. No reemplaza a `json_util.rs` sino que cubre lo que aquel declara fuera de su alcance: cuenta llaves ignorando las que están dentro de un string, desescapa de verdad (incluidos los pares sustitutos, así que un símbolo matemático de un abstract no se pierde) y exige indicar el ámbito en cada consulta, que es lo que impide leer una clave del objeto equivocado.
- `check_json_util` pasa a ser `check_copias_canonicas` y recorre una tabla de copias en vez de un solo archivo, así que `lint` verifica byte a byte tanto `scripts/json_util.rs` como `scripts/json_api.rs` de cada skill, con el mismo criterio y el mismo mensaje parametrizado por nombre.

### Corregido

- `scripts/openalex.rs` leía los campos de acceso abierto buscándolos por nombre sobre la respuesta entera, así que tomaba los de `primary_location` —la versión publicada, habitualmente cerrada— en lugar de los de `open_access` y `best_oa_location`. Medido contra la API real con `W4210318887` (`doi:10.1075/scl`), una obra `green` con copia abierta en HAL: el catálogo la registraba como `is_oa=false`, sin licencia, con la versión equivocada y con la URL de pago como `landing_url`. Son justo las columnas con las que el orquestador decide relevancia y el descargador elige a dónde ir. El scraper ahora lee cada campo en su ámbito con `json_api.rs`, ordena `pdf_urls` poniendo primero la ubicación abierta, y su salida quedó cotejada columna por columna contra el JSON crudo de una tanda de 25 obras.

### Cambiado

- `references/fuentes.md` incorpora la nota de sondeo de OpenAlex del 2026-09-01: el nivel 0 no pide correo ni credencial; `content_urls.pdf` apunta a `content.openalex.org` pero devuelve `401` sin credencial, de modo que la regla "descubrimiento, no descarga" sigue en pie por una razón medida y no sólo de diseño; y el filtro acepta hasta 50 DOI por pedido separados con `|`.

## [1.14.0] - 2026-08-31

### Agregado

- Se publica la skill `pulpo-librero`: reúne papers y libros desde fuentes abiertas y los deja listos para el corpus de `biblio-rata`. Compone dos skills madre sin modificarlas (`chatarrero` para reconocer fuentes, `biblio-rata` para el contrato del corpus) y declara esa crianza en `vivencias/ajustes.json`.
- El descubrimiento y la descarga son dos pasos separados. `pulpo buscar` acumula metadatos en un `catalogo.tsv` persistente en la biblioteca, con una columna de decisión (`pendiente`, `relevante`, `descartado`) y su motivo; `pulpo descargar` opera sólo sobre las filas `relevante` y omite las que ya figuran `accepted` en `manifest.tsv`. Así se puede revisar literatura sin descargar nada, y una segunda corrida no vuelve a descubrir lo ya visto.
- La identidad de una obra se resuelve por DOI normalizado, de modo que el mismo trabajo visto en OpenAlex y arXiv comparte fila y acumula ambas procedencias. Sin DOI se usa `fuente:id` y no se fusiona automáticamente: el título o la URL no son prueba suficiente.
- Scrapers acoplados a la interfaz de cada fuente: `scripts/openalex.rs` (descubrimiento) y `scripts/arxiv.rs` (API Atom oficial, sondeada en vivo el 2026-08-29). El descargador exige allowlist de hosts, rechaza hosts privados, revalida cada redirección y acepta un archivo sólo si el extractor puede abrirlo de verdad, no por su cabecera `%PDF-`.

## [1.13.0] - 2026-08-30

### Agregado

- `chatarrero` scaffoldea `vivencias/`: sección "Vivencias propias" en su `SKILL.md` (`pausa_entre_pedidos_s`, `navegador_por_defecto`, y la declaración de `criados` hacia `pulpo-librero`) y su propio `scripts/validar_ajustes.rs` con `json_util.rs`.
- `scripts/sondear_browser.py` y `scripts/sondear_nodriver.py`: scripts de reconocimiento puntual (nivel 4 y 5 de la cascada) para que los subagentes de sondeo prueben una URL con Playwright o `nodriver` sin escribir el scraper todavía.

### Cambiado

- `chatarrero` ya no trata un 4xx como terminal por default: antes de declararlo permanente hay que agotar las rutas alternativas de la fuente y la escalada que autorizó el usuario; solo un 404/410 confirmado en todas las rutas, o un bloqueo por credencial faltante, sigue siendo un límite real. `references/cascada.md` y las reglas no negociables del `SKILL.md` quedan alineadas con este criterio.

## [1.12.0] - 2026-08-30

### Agregado

- `prueba-y-error` scaffoldea `vivencias/`: sección "Vivencias propias" en su `SKILL.md` (`confianza`, `remuestreos`, `alpha`) y su propio `scripts/validar_ajustes.rs` con `json_util.rs`.

### Cambiado

- `prueba-y-error/SKILL.md` baja de 492 a 343 líneas: cuatro secciones que se consultan puntualmente, no en cada corrida —fundamento bibliográfico, autoaplicación del instrumento, señales de alarma y el protocolo de subagentes— se mueven a `references/fundamento.md`, `autoaplicacion.md`, `senales-de-alerta.md` y `subagentes.md`. La descripción de los ocho nodos, que estaba duplicada entre el cuerpo y `references/grafo.md`, queda solo en `grafo.md`, que ahora incluye también los headers obligatorios del contrato del nodo 0.

## [1.11.0] - 2026-08-30

### Agregado

- `biblio-rata` scaffoldea `vivencias/`: sección "Vivencias propias" en su `SKILL.md` (`extractor_preferido`, `resultados_por_defecto`, `fragmento_tokens`, y la declaración de `criados` hacia `pulpo-librero`) y su propio `scripts/validar_ajustes.rs` con `json_util.rs`.

### Corregido

- `lint` compara byte a byte `scripts/json_util.rs` de cada skill contra una copia canónica embebida desde `src/json_util.rs` (`check_json_util` en `src/main.rs`), y falla si divergen. Antes, la sincronización entre las copias existentes dependía solo de que `forjador` se acordara de propagarlas "en la misma edición" — sin nada que lo verificara. `forjador/SKILL.md` documenta a `src/json_util.rs` como la única fuente de verdad y explica por qué no se puede importar como crate: cada `validar_ajustes.rs` se compila suelto con `rustc`, sin `Cargo.toml`.

## [1.10.0] - 2026-08-30

### Agregado

- `forjador/SKILL.md` documenta la convención de copiar `json_util.rs` tal cual a cada skill nueva, cómo declarar `familia`/`criados` recíprocos, y que `version` en `ajustes.json` es el número de esquema del contrato (no la versión de la skill ni del ecosistema) — incluida la disciplina de migrar el propio `ajustes.json` de forjador en la misma edición en que sube su `ESQUEMA_ESPERADO`, para no autobloquearse.
- El validador de vivencias de `forjador` compara `version` contra un `ESQUEMA_ESPERADO` fijo y usa el lector de JSON compartido extraído a `json_util.rs`.

### Cambiado

- El paso 7 de `forjador/SKILL.md` refleja que `install` ya corre el gate de vivencias antes de copiar (implementado en `main.rs` en 1.9.0, pendiente de documentar acá).

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
