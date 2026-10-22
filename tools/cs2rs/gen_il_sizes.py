"""il_sizes.tsv: IL byte size of every method of the game's assemblies, from db/terraria.sqlite (port_methods).

The translator needs it for the float mode "jit32" (DESIGN.md section 5): the 32-bit JIT compiles a method without
register allocation when it is too large ("MinOpts", about 20,000 IL instructions), and inlines only small methods.
Columns: CLR type name (nested with '+'), method name, parameter count, largest IL size among the overloads with
that parameter count. Deterministic: sorted, no timestamp. Run by run.sh.
"""
import os
import sqlite3

here = os.path.dirname(os.path.abspath(__file__))
db = os.path.join(here, '..', '..', 'db', 'terraria.sqlite')
out = os.path.join(here, 'il_sizes.tsv')
con = sqlite3.connect('file:' + db.replace('\\', '/') + '?mode=ro', uri=True)
rows = con.execute(
    "select type_name, name, param_count, max(il_size) from port_methods "
    "where has_body = 1 and il_size is not null group by 1, 2, 3 order by 1, 2, 3").fetchall()
text = ''.join('%s\t%s\t%d\t%d\n' % r for r in rows)
old = open(out, encoding='utf-8', newline='').read() if os.path.exists(out) else None
if old != text:
    open(out, 'w', encoding='utf-8', newline='').write(text)
print('il_sizes.tsv: %d methods (%s)' % (len(rows), 'unchanged' if old == text else 'written'))

# type_order.tsv: the types of the game's executable in metadata (TypeDef token) order, which is the order of
# `Assembly.GetTypes()`: `Program.ForceStaticInitializers` runs the static constructors in that order.
# Columns: CLR full name (nested with '+'), generic arity of the type itself, 1 when compiler generated.
out2 = os.path.join(here, 'type_order.tsv')
rows2 = con.execute(
    "select full_name, coalesce(generic_arity, 0), compiler_generated from port_types where assembly = 'Terraria' order by token").fetchall()
text2 = ''.join('%s\t%d\t%d\n' % r for r in rows2)
old2 = open(out2, encoding='utf-8', newline='').read() if os.path.exists(out2) else None
if old2 != text2:
    open(out2, 'w', encoding='utf-8', newline='').write(text2)
print('type_order.tsv: %d types (%s)' % (len(rows2), 'unchanged' if old2 == text2 else 'written'))
