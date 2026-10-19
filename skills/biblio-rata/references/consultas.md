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
- **NFKC**: antes de indexar, y también sobre la consulta, el texto pasa por la
  normalización NFKC de Unicode. Las ligaduras de los PDF (`ﬁ`, `ﬀ`, `ﬂ`)
  quedan como letras sueltas, y lo mismo las letras matemáticas en cursiva,
  `ℓ` y `µ`. Sin eso `classification` no encontraba `classiﬁcation`. El precio
  es que `pagina.py` muestra el texto normalizado: `x²` sale `x2` y `½` sale
  `1⁄2`. Ten presente que una cifra con exponente o fracción puede haber
  perdido su forma antes de citarla textual.
- **`slug` y `pag` van sin indexar**: no se puede filtrar por documento con la
  sintaxis de columnas de FTS5. Para eso está `--doc` en `buscar.py`, que
  acepta el slug entero, su último componente o un prefijo inequívoco de
  cualquiera de los dos.

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

Regla práctica en inglés: **no uses prefijos**. El *stemming* ya cubre plurales
y derivados, que es para lo que uno los usaría. Si aun así los necesitas, corta
corto. En otros idiomas es al revés (ver "Idioma").

**2. `NEAR` no admite `^` adentro.** `NEAR(^one, two)` es error de sintaxis.
Si una consulta da error, el script lo dice y no rompe nada: reformula.

Los caracteres `-`, `.` y `:` sueltos pueden ser interpretados como sintaxis de
FTS5. Si aparecen en un término literal y SQLite devuelve un error de sintaxis,
quita el signo, reformula la consulta o pon la frase literal entre comillas.
Los operadores solo deben escribirse a propósito; el buscador no modifica la
consulta automáticamente.

Al indexar se pegan las palabras cortadas al final de línea con guion o con
guion blando, U+00AD (`succes-` / `sive` queda `successive`). El precio es que un compuesto legítimo
cortado justo ahí pierde el guion: `state-of-the-` / `art` queda
`state-of-theart`. Si un término compuesto no aparece, prueba también la forma
pegada o sus partes con `OR`.

## Recetas para este corpus

```sh
# concepto de dos palabras, en cualquier orden y sin ser contiguas
buscar.py "successive halving"

# frase literal, cuando el término técnico es una unidad
buscar.py '"failure-inducing input"'

# dos ideas que deben aparecer cerca, no solo en la misma página
buscar.py "NEAR(budget candidates, 8)"

# acotar a un documento: slug entero, último componente o prefijo inequívoco
buscar.py "regret bound" --doc lattimore
buscar.py "regret bound" --doc bandits/lattimore-2020

# ampliar el fragmento cuando la línea queda cortada
buscar.py "value of information" --tokens 40

# alternativas de vocabulario: la literatura no usa siempre la misma palabra
buscar.py "(holdout OR validation) adaptive"
```

## Idioma

**Se consulta en el idioma de cada documento**, porque el índice guarda las
palabras tal como están escritas y no traduce nada. Al indexar se estima el
idioma de cada documento contando palabras vacías de inglés, español,
portugués, francés y alemán. `--listar` lo muestra por documento, con el
reparto al pie. Si hay más de uno, `buscar.py` lo agrega a su última línea:

```
— 8 de 120 páginas con coincidencia · idiomas del corpus: en 14, es 3
```

Con ese aviso, repite la consulta en cada idioma, con los términos de esa
literatura. `?` quiere decir que ninguna lista llegó al 3 % de las palabras del
documento, porque está en otro idioma o tiene muy poco texto.

El *stemming* de `porter` implementa reglas del **inglés**. En español une el
plural en `-s` y `-es` (`indicadores` encuentra `indicador`, `políticas`
encuentra `política`), pero no el género (`indicador` no encuentra
`indicadora`, ni `público` a `pública`). En portugués no une `-ção` con `-ções`
(`avaliação` no encuentra `avaliações`). Ahí sirve lo que en inglés conviene
evitar, un prefijo corto: `indicad*`, `public*` y `avalia*` encuentran todas las
formas. Los acentos dan igual también en el prefijo.

## Cómo leer la salida

```
li-2018-hyperband p.6 · -6.94 · …3.1 [Successive] [Halving] Hyperband extends…
└── slug          └ pág └ BM25   └ fragmento con los aciertos entre corchetes
```

El slug es la ruta del PDF relativa al corpus sin la extensión. Si el PDF está
en una subcarpeta, sale con ella: `bandits/li-2018-hyperband p.6`.

BM25 es negativo por convención: **más negativo es mejor**, y los resultados ya
vienen ordenados. La última línea dice cuántas páginas coincidieron en total; si
son muchas, conviene afinar antes de leer. Si el corpus mezcla idiomas, también
dice cuáles (ver "Idioma").

`--tokens` controla el largo del fragmento. La documentación de SQLite declara
el rango 1–64; valores mayores funcionan en esta build pero no son portables, así
que conviene quedarse dentro. Cada resultado cuesta unos 40–60 tokens de
contexto con el valor por defecto (24).
