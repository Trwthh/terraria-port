# VAEL - TERRARIA

Reference database for a Rust / Bevy rewrite of Terraria 1.4.5.8. Plan: `plan.md`. Latest check: `logs\parity-report.md`.

The game is read from `C:\Users\user\Documents\Games\Terraria` and never written to.

## Rebuild

```
python tools\pipeline.py                      # decompile (skips if unchanged), metadata, index, port tables, parity
python tools\pipeline.py --steps extend,parity
python tools\pipeline.py --force-decompile    # the old tree is moved to _archive\, not deleted
```

Needs: Python 3.12, .NET 8 SDK, and in `VAEL - GAMELAB\tools`: `gamedb`, `ilspycmd`, `xna40`.

## Query with gamedb

```
set G="..\VAEL - GAMELAB\tools\gamedb\target\release\gamedb.exe"
%G% search -r decompile --db db\terraria.sqlite AI_007
%G% read   -r decompile --db db\terraria.sqlite StartCachingType --path Terraria/Item.cs
%G% graph  -r decompile --db db\terraria.sqlite UpdateEquips --direction callers
%G% modules -r decompile --db db\terraria.sqlite --rules tools\modules.txt
```

gamedb's parser is heuristic: it has 14,916 of the 17,394 hand-written method bodies and its call edges are
name matches. For exhaustive answers use the `port_*` tables below.

## Tables

gamedb's own: `files`, `functions`, `symbols`, `strings`, `edges`, `module_status`.

Added by `tools\port_extend.py` (rebuilt on every run, same input gives the same rows):

| Table | One row per | Key columns |
|---|---|---|
| `port_assemblies` | indexed binary | name, sha256, version, refs |
| `port_files` | file under `decompile\` | path, kind, sha256, `content` (zlib of the exact bytes), `gamedb_file_id` |
| `port_types` | type in the binary | assembly + token, full_name, kind, base, file_id, start_line, end_line |
| `port_methods` | method in the binary | assembly + token, type_name, name, params, ret, il_size, il_sha256, `src_match`, file_id, start_line, end_line, src_sha256, origin_token, gamedb_function_id |
| `port_fields` | field or enum member | type_name, name, type, `constant` (enum and const values), file_id, line |
| `port_props` | property or event | getter_token, setter_token, source span |
| `port_calls` | call site in the IL | src_token, opcode, dst_assembly, dst_token, dst_type, dst_name |
| `port_field_refs` | field access in the IL | src_token, access (read / write / address), field_token |
| `port_strings` | string literal in the IL | method_token, text |
| `port_resources`, `port_embedded` | embedded resource / library | name, size, sha256 |
| `port_src_decls` | declaration in the C# text | kind, container, name, span, matched token |
| `port_callgraph` (view) | caller, callee pair | names on both sides, call-site count |
| `port_runs` | pipeline run | time, gamedb commit, counts |

`src_match` on a method: `exact` / `by-count` (has its own source span), `implicit` (default constructor or field
initialisers; span is the type), `runtime` (delegate methods, no IL), `origin*` (lambda / iterator; code is inside
`origin_token`), `synthetic` (compiler helper types).

## Examples

```sql
-- who writes Player.statLife
SELECT m.type_name, m.name, COUNT(*) FROM port_field_refs r
JOIN port_methods m ON m.assembly = r.assembly AND m.token = r.src_token
WHERE r.field_type = 'Terraria.Player' AND r.field_name = 'statLife' AND r.access = 'write' GROUP BY 1, 2;

-- the exact source of a method (lines start_line..end_line of the stored file)
SELECT f.path, m.start_line, m.end_line FROM port_methods m JOIN port_files f ON f.id = m.file_id
WHERE m.type_name = 'Terraria.Player' AND m.name = 'Hurt';

-- every XNA call the port has to replace, most used first
SELECT dst_type, dst_name, COUNT(*) FROM port_calls
WHERE dst_assembly IS NULL AND dst_type LIKE 'Microsoft.Xna.%' GROUP BY 1, 2 ORDER BY 3 DESC;
```

Read a stored file back in Python: `zlib.decompress(content)`.
