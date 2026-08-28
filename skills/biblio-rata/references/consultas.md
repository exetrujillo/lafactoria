# Sintaxis de consulta (SQLite FTS5)

Cómo está indexado el corpus, y por qué importa:

```sql
CREATE VIRTUAL TABLE pagina USING fts5(
  slug UNINDEXED, pag UNINDEXED, texto,
  tokenize = "porter unicode61 remove_diacritics 2");
```

- **`porter`**: hay *stemming*. `halve` encuentra `halving` y `halves`;
  `experiment` encuentra `experiments` y `experimental`. No hace falta pluralizar
  ni conjugar a mano.
- **`remove_diacritics 2`**: los acentos se ignoran. `medicion` encuentra
  `medición`. Sirve para no pelear con el teclado.
- **`slug` y `pag` van sin indexar**: no se puede filtrar por documento con la
  sintaxis de columnas de FTS5. Para eso está `--doc` en `buscar.py`.

## Operadores

| Forma | Qué hace |
|---|---|
| `a b` | AND implícito: ambas en la misma página. Es lo que más se usa. |
| `a AND b`, `a OR b`, `a NOT b` | Explícitos. **Van en mayúsculas**; en minúsculas se tratan como palabras. |
| `(a OR b) c` | Paréntesis para agrupar. |
| `"frase exacta"` | Frase literal. Las comillas van dentro de la cadena que se pasa al script. |
| `term*` | Prefijo. El asterisco va **fuera** de las comillas. |
| `^term` | La página empieza con ese término. |
| `NEAR(a b, 5)` | Las dos a menos de 5 tokens. Sin el número, la distancia por defecto es **10**. |

Precedencia, de más fuerte a más débil: AND implícito → `NOT` → `AND` → `OR`.
Ante la duda, paréntesis.

## Las dos trampas

**1. Los prefijos pelean con el *stemming*.** El asterisco se aplica sobre la
raíz, no sobre la palabra escrita. Verificado en este corpus:

```
candidat*   -> no encuentra nada   (la raíz indexada es "candid")
candid*     -> encuentra
halv*       -> encuentra
```

Regla práctica: **no uses prefijos**. El *stemming* ya cubre plurales y
derivados, que es para lo que uno los usaría. Si aun así los necesitas, corta
corto.

**2. `NEAR` no admite `^` adentro.** `NEAR(^one, two)` es error de sintaxis.
Si una consulta da error, el script lo dice y no rompe nada: reformula.

Los caracteres `-`, `.` y `:` sueltos pueden ser interpretados como sintaxis de
FTS5. Si aparecen en un término literal y SQLite devuelve un error de sintaxis,
quita el signo, reformula la consulta o pon la frase literal entre comillas.
Los operadores solo deben escribirse a propósito; el buscador no modifica la
consulta automáticamente.

## Recetas para este corpus

```sh
# concepto de dos palabras, en cualquier orden y sin ser contiguas
buscar.py "successive halving"

# frase literal, cuando el término técnico es una unidad
buscar.py '"failure-inducing input"'

# dos ideas que deben aparecer cerca, no solo en la misma página
buscar.py "NEAR(budget candidates, 8)"

# acotar a un documento (admite prefijo del slug si es inequívoco)
buscar.py "regret bound" --doc lattimore

# ampliar el fragmento cuando la línea queda cortada
buscar.py "value of information" --tokens 40

# alternativas de vocabulario: la literatura no usa siempre la misma palabra
buscar.py "(holdout OR validation) adaptive"
```

## Idioma

**Se consulta en el idioma del corpus**, porque el índice guarda las palabras
tal como están escritas y no traduce nada. Si el corpus está en inglés, buscar
en español no devuelve nada, salvo por las notas que hayas escrito tú.

`--listar` muestra los títulos, que suelen bastar para saber en qué idioma está.
Cuando el corpus no está ni en inglés ni en español, el `SKILL.md` manda
preguntarle al usuario en qué idioma consultar antes de seguir.

Un detalle del *stemming*: `porter` implementa reglas del **inglés**. En otros
idiomas no rompe nada —se aplica igual al indexar y al consultar, así que las
coincidencias exactas siguen funcionando— pero no unifica singular con plural ni
las formas conjugadas. En un corpus que no sea inglés conviene probar con más de
una forma de la palabra, o usar un prefijo corto.

## Cómo leer la salida

```
li-2018-hyperband p.6 · -6.94 · …3.1 [Successive] [Halving] Hyperband extends…
└── slug          └ pág └ BM25   └ fragmento con los aciertos entre corchetes
```

BM25 es negativo por convención: **más negativo es mejor**, y los resultados ya
vienen ordenados. La última línea dice cuántas páginas coincidieron en total; si
son muchas, conviene afinar antes de leer.

`--tokens` controla el largo del fragmento. La documentación de SQLite declara
el rango 1–64; valores mayores funcionan en esta build pero no son portables, así
que conviene quedarse dentro. Cada resultado cuesta unos 40–60 tokens de
contexto con el valor por defecto (24).
