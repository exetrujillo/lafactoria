# Dónde guarda memorias cada arnés

Lo que se comprobó en la documentación oficial el 2026-10-24. Si el arnés
declara otra cosa en sus instrucciones de sesión, manda lo que declara.

## Claude Code

- Directorio por proyecto: `~/.claude/projects/<proyecto>/memory/`. El nombre
  del proyecto sale del repositorio git, así que todos los worktrees y
  subdirectorios del mismo repo comparten el directorio. Fuera de git, se usa
  la raíz del proyecto. `autoMemoryDirectory` en `settings.json` lo cambia.
- Índice: `MEMORY.md`, una línea por memoria, sin contenido. Se cargan solo
  sus primeras 200 líneas o 25 KB en cada sesión; lo que sigue no se lee. Los
  archivos de cada memoria no se cargan al inicio, se abren a demanda.
- Un archivo por memoria, markdown con frontmatter. El arnés dicta las claves
  en sus instrucciones de sesión; observadas el 2026-10-24: `name`,
  `description` y `type` (o `metadata.type`) con uno de `user`, `feedback`,
  `project`, `reference`. El arnés agrega `modified` al escribir. Para
  `feedback` y `project`, el cuerpo lleva líneas **Why:** y **How to
  apply:**. Enlaces entre memorias con `[[name]]`.
- Es local a la máquina. `/memory` dentro de una sesión abre el directorio.
- Si el arnés avisa que el índice está cerca del límite, se acorta: una línea
  por entrada, el detalle al archivo de la memoria, y se funden o borran las
  entradas viejas.

## OpenCode

- No guarda memorias por proyecto. Lo único persistente son las reglas:
  `AGENTS.md` en el proyecto (o `CLAUDE.md` si no hay) y
  `~/.config/opencode/AGENTS.md` global, que escribe el usuario.
- Existen plugins de terceros de memoria persistente (`opencode-mem` y
  otros), cada uno con su propio almacén. Si el proyecto usa uno, el usuario
  dice dónde y cómo se lee; si no, en OpenCode esta skill no tiene nada que
  curar y lo dice en una línea.
