# Auditar la lógica con la-quinta-pata

**Cuándo aplica.** La skill recién escrita tiene lógica no trivial que puede
fallar en silencio: una compuerta con criterios de decisión, una cascada de
niveles, coordinación de subagentes, o un contrato que otras skills van a
consumir. Para una skill simple, de instrucciones lineales sin ramas de
decisión, este paso no aporta — no lo ejecutes.

**Cómo.** Invoca la-quinta-pata sobre el SKILL.md recién escrito para que
busque fallas de lógica antes de que el usuario la use: supuestos no
verificados, casos donde la compuerta no cubre el camino feliz, condiciones de
parada ambiguas.

**Qué hacer con los hallazgos.** Resuelve los `confirmado` o `plausible` de
impacto alto antes de mostrarle nada al usuario. Los de impacto bajo pueden
mencionarse en la ronda de iteración sin bloquearla.
