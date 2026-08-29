# La Factoría

Las skills son habilidades que los agentes de IA pueden usar para realizar
tareas y que guardamos en un archivo `SKILL.md` para no tener que escribir la
instrucción completa cada vez que las usamos. Escritas sueltas se degradan, se
duplican entre proyectos, se contradicen, envejecen sin que nadie lo note y
nadie sabe cuál es la versión buena. La Factoría es una librería de skills y una
fábrica de skills al mismo tiempo, es el taller donde se escriben, se validan, se
versionan y se retiran con cierta lógica del repositorio.

El flujo de trabajo principal es conversacional. Las skills se escriben, validan
e instalan hablando con un agente, apoyándose en **`/forjador`**, la skill
maestra del repositorio que se encarga de generar, validar e instalar las
skills. Editar un `SKILL.md` a mano es igualmente legítimo; simplemente no es el
camino principal.

Este documento define el sentido del repositorio. Para los comandos, la
arquitectura de `skillcheck` y las reglas del validador, ver `CLAUDE.md`.

## Principios

Cuatro decisiones de diseño que rigen todo lo que se escribe acá.

**1. Las skills tienen vivencias, y esas vivencias no se destruyen.** Como
`forjador`, hay otras skills principales de este repo que son, al mismo
tiempo, herramientas para crear skills. Es por esta dinámica de interacción
entre herramientas (para desarrollarse a sí mismas y desarrollar otras) que
necesitan guardar vivencias propias: bitácoras, archivos de estado o
información de contexto. Ninguna skill puede corromper ni eliminar ese
material sin permiso del usuario. Ante la duda, se pregunta.

**2. Lo que varía de usuario a usuario no se versiona.** Si una skill tiene
referencias a cómo maneja datos, búsquedas o cualquier otra cosa que cambia de
usuario a usuario, eso debe vivir en un lugar no versionado, dentro del
`.gitignore`, para no contaminar el repositorio con los detalles de uso de cada
quien. Por el contrario, los `SKILL.md` y los archivos auxiliares que necesitan
deben estar versionados para que el repositorio sea reproducible y las skills
puedan compartirse.

**3. Neutralidad entre arneses.** Una skill no nombra herramientas propietarias
de un agente concreto. Se escribe "la herramienta disponible para hacer
preguntas al usuario", no el nombre que le da un arnés en particular. Lo mismo
para búsqueda web, lectura de archivos o subagentes. Así la misma skill sirve en
Claude Code, OpenCode y lo que venga después.

**4. Economía de contexto.** Cada token que ocupa una skill es un token que la
tarea no tiene. Por eso existen las referencias: lo que no se usa en cada corrida
se indexa en `references/` y se consulta sólo cuando hace falta. Una skill que
carga todo por las dudas está mal diseñada, aunque funcione.

## El ecosistema

Si pensamos las skills de esta fábrica como poseedoras de una historia propia, y
consideramos que al mismo tiempo interactúan entre sí y consigo mismas, podemos
encontrar que se dan ciertos tipos de relaciones particulares, como si fueran
árboles o algún otro tipo de grafo. Ninguno de estos casos es excluyente y hasta
cierto punto sus definiciones son antojadizas, pero me parece útil para pensar
cómo organizar el trabajo de desarrollo y mantenimiento de las skills, y para
que quien encuentre este repo lo pueda usar como punto de partida para entender
cómo se estructura este ecosistema.

**Autointeracción.** Una skill se llama a sí misma. `forjador` puede invocarse
para mejorarse; `la-quinta-pata`, una skill de pensamiento lateral, podría
llamarse a sí misma para encontrar nuevos ángulos de análisis y descubrir sus
propias debilidades.

**Uso.** Una skill llama a otra dentro de su propio proceso, para un fin que es
suyo. Por ejemplo, `forjador` podría llamar a `la-quinta-pata` para buscar
fallas en la lógica de un plan antes de que este se ejecute.

**Estudio.** Una skill se usa *por sobre* otra, no para usarla dentro de un
proceso sino con el fin de estudiarla o modificarla. Es lo que se hizo con
`prueba-y-error` (que busca aplicar el método científico en bucles iterativos de
experimentos) por sobre `biblio-rata`, la skill que indexa documentos en una
estructura consultable para no gastar tokens leyendo PDFs enteros. Al estudiarla
medimos en qué casos valía la pena ejecutar la skill y en cuáles convenía leer
el PDF completo.

**Herencia.** Una skill hereda toda o parte de la lógica de otra y desarrolla
capacidades más específicas a partir de ella, mientras se desarrolla de forma
independiente.

**Crianza.** Una skill necesita los conocimientos específicos de más de una skill
para desarrollarse y funcionar. `pulpo-librero` necesita a `biblio-rata` para
leer documentos y a `chatarrero` para obtener información de fuentes públicas.
Aunque exista una vida independiente de la skill respecto de los nodos que le
entregan conocimientos, probablemente necesite seguir consultando a sus padres
especializados ante bugs, alucinaciones y casos límite. No es una subcategoría
de herencia: una skill hereda de una fuente y muta esa lógica para su propio
caso, mientras que cría cuando depende de varias skills completas a la vez,
sin absorber su lógica. Ambas relaciones pueden combinarse en la misma skill,
pero son categorías distintas.

En general estas no son categorías o relaciones excluyentes, de hecho la idea es que
no lo sean y que puedas probar combinaciones según necesites. Las **dependencias** de
una skill deben quedar declaradas en su propio `SKILL.md`, con el nombre de la
skill y una breve descripción de para qué la usa. Una skill también puede
recomendar o preguntar al usuario si usar otras como "combos de uso". Si la
skill sugerida no está instalada y el usuario decide usarla, corresponde ofrecer
instalarla con `skillcheck install NOMBRE`.

## Qué entra y qué no

No todo lo automatizable merece ser una skill. **No lo es**:

- Una tarea de una sola vez. Se hace y listo.
- Un script que no toma decisiones. Eso es un script, y vive en `scripts/`
  dentro de la skill que lo necesita.
- Conocimiento que el modelo ya tiene sin ayuda.
- Un envoltorio de un comando que se escribe más rápido a mano que invocando.
- Algo que sólo sirve en un repositorio concreto. Eso es una skill de proyecto y
  vive en el `.claude/skills/` de ese repositorio, no acá. Aunque un usuario puede
  crearla aquí de todas maneras.

**Crear una skill nueva o extender una existente**, en este orden de preferencia:

1. Si comparte objetivo con una skill existente y sólo cambia el caso de uso, se
   extiende esa skill o se le agrega una referencia.
2. Si necesita los conocimientos de dos o más skills existentes, es una relación
   de crianza: se declara la dependencia, no se copia el contenido.
3. Sólo si el disparador y el objetivo son distintos de todo lo que hay,
   corresponde una skill nueva.

## Ciclo de vida

**Nacer.** Pídele al agente que use la skill **forjador**, o simplemente
descríbele qué quieres automatizar. Forjador se encarga de aclarar el propósito
con preguntas puntuales, escribir el `SKILL.md`, validarlo con `skillcheck` e
instalarlo donde corresponda, ya sea en este proyecto o de forma global.

**Iterar.** El proceso es iterativo: luego de crear una skill nueva, léela,
prueba sus resultados, detecta errores o áreas de mejora y pídele a **forjador**
que la modifique, o edítala a mano si lo prefieres. Acá se decide qué va en el
cuerpo y qué en `references/`. Las referencias se guardan en la carpeta
`references` de cada skill y se marcan en el `SKILL.md` con un enlace: sirven
para lo que no se usa en cada corrida pero debe estar disponible como facultad
si llegara a hacer falta. En `prueba-y-error`, por ejemplo, hay muchas técnicas
distintas de experimentación, algunas más recomendables que otras según el caso;
indexarlas mantiene la información disponible sin gastar tokens cada vez. Los
`scripts/` y `assets/` que una skill necesite van en sus carpetas
correspondientes dentro de `skills/<nombre>/`.

**Publicar.** Una skill está publicada cuando pasa `lint`, está instalada y
figura en el `CHANGELOG.md`.

**Versionar.** El repositorio usa versionado semántico sobre el ecosistema, no
sobre el binario:

- **patch** — corrección que no cambia el contrato de ninguna skill.
- **minor** — capacidad nueva compatible, o una skill nueva publicada.
- **major** — cambia el sentido del repositorio de forma incompatible.

**Retirar.** Una skill se retira cuando nada la invoca, cuando su formato quedó
incompatible con los arneses vigentes, o cuando otra la absorbió. Se desinstala,
se saca de `skills/` y se registra en el `CHANGELOG.md`. Una skill instalada que
nadie usa no es inofensiva, ocupa espacio de decisión del agente y envejece sin
que nadie la mire.

## Vivencias

Cuando alguien clona este repo y empieza a usar una skill, casi siempre va a
querer ajustarle cosas para su propio caso y a acumular aprendizajes sobre qué
resultados le dio. Eso no es memoria de un corpus ni de un proyecto: son las
vivencias **de la skill**, de qué se hace con ella y con qué ha interactuado de
manera significativa. Viven en el directorio de la skill dentro de la factoría
y no se versionan.

```
skills/<nombre>/vivencias/
  ajustes.json            # se lee siempre. Preferencias de este usuario
  INDICE.md               # se lee siempre. Una línea por entrada
  registro/<fecha>-<tema>.md   # se abre sólo si el índice lo señala
```

**`ajustes.json`** guarda lo que este usuario quiere que la skill respete en cada
corrida: valores por defecto, umbrales, preferencias, contexto propio. Se lee
entero siempre, así que se mantiene corto; si crece hasta volverse un método
distinto, ya no es un ajuste y corresponde una skill nueva. Las claves las define
cada skill en su `SKILL.md`.

```json
{
  "skill": "chatarrero",
  "version": "1.3.4",
  "actualizado": "2026-08-29",
  "ajustes": { "pausa_entre_pedidos_s": 8, "navegador_por_defecto": "nodriver" },
  "notas": ["Los sitios con WAF agresivo exigen UA de navegador real"]
}
```

Los ajustes pueden cambiar valores por defecto, umbrales y preferencias, y
agregar contexto. **No** pueden anular las comprobaciones de seguridad de la
skill ni el principio de no destruir sus vivencias.

**`INDICE.md`** es la puerta de entrada al registro: una línea por entrada, con
formato fijo y sin prosa (`fecha | tema | resultado en pocas palabras |
archivo`). Se lee entero porque es barato, y sirve para decidir qué vale la pena
abrir. **`registro/`** guarda una entrada por hecho y nunca se lee completo.
Cuando el índice se vuelve largo, se consolida: varias entradas que dicen lo
mismo se funden en una sola regla. Sin poda, las vivencias se degradan hasta
volverse caras e inútiles a la vez.

**Qué guardan las `vivencias/` y qué no.** `vivencias/` no es donde una
skill guarda los datos de su propio dominio de trabajo — eso puede vivir en
cualquier otro lugar, con el nombre y la visibilidad que le convengan a
quien lo va a revisar. `biblio-rata` indexa las bibliotecas que arma en
`<corpus>/.biblio-rata/`, junto a los PDFs: es memoria de las bibliotecas, no
de la skill. `prueba-y-error` corre experimentos en `experimentos/`, fuera de
`skills/`, a propósito: esas corridas generan mucho volumen y alguien
necesita poder revisarlas y manipularlas a mano sin entrar a una skill.

`vivencias/` es para lo otro: lo que una skill aprende sobre sí misma y
sobre con quién se relaciona, pero sólo si fue significativo — no cualquier
interacción amerita una entrada, sólo la que dejó una marca. Hallazgos
resumidos de sus interacciones con otras skills, con el usuario o con su
entorno, casos curiosos que informan el desarrollo de la skill misma —su
historia, su familia según "El ecosistema", sus decisiones—. Si una entrada
describe un dato de dominio en vez de un hallazgo sobre la skill o sus
relaciones, no va en `vivencias/`.

### Del uso propio al conocimiento compartido

Todo aprendizaje **nace** en `vivencias/`, local y sin versionar. Sólo lo que se
confirma en varios usos **y** puede describirse sin datos propios —por tipo de
caso, no por nombre— se **promueve** a `references/` y pasa a versionarse.

Esa es la diferencia entre las dos cosas que hoy conviven en `references/`: las
**facultades** (técnicas e instrucciones que la skill puede necesitar) y el
**conocimiento destilado** (hallazgos medidos que le sirven a cualquiera). Con el
ciclo de promoción, lo versionado deja de ser lo que midió una persona y pasa a
ser lo que sobrevivió a la prueba de servirle a otro.

### Dónde se escribe

La copia instalada en `.claude/skills/<nombre>/` es la que los agentes leen, pero
`install` la reemplaza entera en cada corrida: lo que se escriba ahí se pierde.
**Las vivencias se escriben siempre en la fuente**, `skills/<nombre>/vivencias/`,
y `install` las propaga a la copia.

Para que una skill instalada sepa volver a su fuente —sobre todo si se instaló
con `--global` y se la usa desde otro proyecto— `skillcheck install` deja en la
copia un archivo `.factoria-origen` con la ruta absoluta del directorio de
origen.

## Instalación

La única skill que es un requisito para este repo es **forjador**. Por allí pasa
la lógica general de todo el ecosistema, por lo que es la primera skill que hay
que instalar. Luego puedes instalar otras a tu gusto.

```sh
cargo build --release
cargo run --quiet -- install forjador
```

Ese segundo comando copia `skills/forjador` a `.claude/skills/forjador`, una
ubicación compatible tanto con Claude Code como con otros *arneses de IA*. A
partir de ahí `forjador` queda disponible para los agentes. El resto de los
comandos está en `CLAUDE.md`.

## Estructura

| Ruta | Qué es | ¿Versionado? |
|---|---|---|
| `skills/<nombre>/SKILL.md` | Código fuente de cada skill | Sí |
| `skills/<nombre>/references/`, `scripts/`, `assets/` | Facultades, herramientas y recursos de esa skill | Sí |
| `skills/<nombre>/vivencias/` | Ajustes, índice y registro de uso de este usuario | No |
| `.claude/skills/<nombre>/` | Copia instalada: lo que los agentes realmente leen | Sólo `forjador`, como bootstrap |
| `src/main.rs` | El binario `skillcheck` | Sí |
| `docs/experimentos/` | Análisis y conclusiones de los experimentos de las skills principales | Sí |
| `experimentos/` | Corridas crudas y ledgers de `prueba-y-error` | No |
| `docs/literatura/` | Corpus local de PDFs para ser leídos por `biblio-rata`, subdirectorios por skill o por tema de interés para el desarrollo de este repositorio | No |
| `CHANGELOG.md` | Historia del ecosistema | Sí |

La distinción entre `docs/experimentos/` y `experimentos/` es el principio 2
hecho estructura de directorios: la conclusión de un experimento es del
repositorio, los datos crudos son de quien lo corrió.

**Fuente contra copia instalada:** si editas una skill en `skills/`, hay que
volver a correr `install` para que el cambio se refleje. Hasta entonces no tiene
ningún efecto sobre las conversaciones. Es el error más frecuente del repo.

## Jerarquía documental

Cuatro documentos, cuatro responsabilidades que no deben solaparse:

- **`README.md`** (este archivo) — el sentido: qué es una skill acá, cómo se
  relacionan, qué se acepta y qué no.
- **`CLAUDE.md`** — la ejecución: comandos, arquitectura de `skillcheck` y las
  reglas que aplica el validador.
- **`skills/<nombre>/SKILL.md`** — el contrato de cada skill: cuándo se dispara,
  qué hace, de qué depende.
- **`CHANGELOG.md`** — la historia: qué cambió, cuándo y por qué.

Las skills usan el frontmatter estándar `name` y `description`; `skillcheck`
valida además las restricciones de nombres y longitud que exige OpenCode, con el
detalle en `CLAUDE.md`.

Ante una contradicción entre este documento y una skill, manda este documento y
la skill se corrige. Si lo que está mal es el criterio y no la skill, se cambia
acá primero y después se propaga.

## Lenguajes del repo

Las skills que se documentan aquí pueden ser para cualquier lenguaje o stack —
eso es independiente. Los lenguajes más usados de este repo son Rust y Python,
pero cualquier lenguaje es bienvenido.