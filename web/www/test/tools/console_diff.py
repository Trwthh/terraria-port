"""Compares the browser port's server console with the ORIGINAL dedicated server's, command by command
(web/STATUS.md 2026-10-14 "Server console: every command").

    python web/www/test/tools/console_diff.py [orig transcript] [port dump ...]
    defaults: work/logs/orig_console.txt (test/tools/orig_console.py)  work/logs/hpcm_console_1.txt hpcm_console_2.txt
              (test/run-host-console-mp.sh: the page's #sc-out before and after `clear`)

The original's transcript has each command as "> <command>" on a line of its own (orig_console.py writes it). The
page echoes the typed line where the cursor is (after the ": " prompt, or alone on a line when the server printed
something after its prompt) and ends it with a bare LF, while every line the server writes ends with CR LF
(Console.WriteLine, Windows newline): that tells the echoes apart. A command's output is what follows up to the
next command. Lines of other threads, which come at their own time (connections, joins, saves, fps lines, the end of
settling), are compared as sets. Peer ports, the listening port and fps numbers are normalized.
Prints a table; exit 1 when a command's output differs.
"""
import re
import sys

ROOT = __file__.replace('\\', '/').rsplit('/web/www/test/tools/', 1)[0]
args = sys.argv[1:]
orig_path = args[0] if args else f'{ROOT}/work/logs/orig_console.txt'
port_paths = args[1:] or [f'{ROOT}/work/logs/hpcm_console_1.txt', f'{ROOT}/work/logs/hpcm_console_2.txt']

ASYNC = re.compile(r'is connecting\.\.\.$|has joined\.$|has left\.$|^Saving world data|^Validating world save|^Backing up world file'
                   r'|^Water has been settled\.$|^\d+  \([\d.]+\)$|^Server started$'
                   r'|was booted: Incorrect password$|was booted: You are banned from this server\.$')  # refused at connect


def norm(line):
    line = re.sub(r'127\.0\.0\.1:\d+', '127.0.0.1:*', line)
    line = re.sub(r'^Port: \d+$', 'Port: *', line)
    line = re.sub(r'^\d+  \([\d.]+\)$', 'N  (N)', line)
    return line


def strip_prompts(s):
    while s.startswith(': '):
        s = s[2:]
    return s


def split(text, page):
    cmds, asyncs = [], []
    cur = None
    lines = text.split('\n')
    for i, raw in enumerate(lines):
        if page:
            body = strip_prompts(raw)
            is_cmd = i < len(lines) - 1 and not raw.endswith('\r') and body.strip() != ''
        else:
            is_cmd = raw.startswith('> ')
            body = raw[2:]
        if is_cmd:
            cur = (body, [])
            cmds.append(cur)
            continue
        line = strip_prompts(raw.rstrip('\r')).rstrip()
        if not line or line == ':':
            continue
        if ASYNC.search(line):
            asyncs.append(norm(line))
        elif cur is not None:
            cur[1].append(norm(line))
    return cmds, asyncs


orig_text = open(orig_path, encoding='utf-8', errors='replace', newline='').read().replace('\r\n', '\n')
orig_text = orig_text[orig_text.find('Server started'):]
orig_cmds, orig_async = split(orig_text, page=False)
port_cmds, port_async = [], []
for p in port_paths:
    c, a = split(open(p, encoding='utf-8', errors='replace', newline='').read(), page=True)
    port_cmds += c
    port_async += a

by_cmd = {}  # the n-th occurrence of a command on each side are paired
for side, cmds in (('orig', orig_cmds), ('port', port_cmds)):
    for c, out in cmds:
        by_cmd.setdefault(c, {'orig': [], 'port': []})[side].append(out)
diffs = 0
show = lambda v: '-' if v is None else (' | '.join(v) if v else '(nothing)')
for c in by_cmd:
    o, p = by_cmd[c]['orig'], by_cmd[c]['port']
    for i in range(max(len(o), len(p))):
        a = o[i] if i < len(o) else None
        b = p[i] if i < len(p) else None
        if a is None or b is None:
            status = 'only ' + ('port' if a is None else 'original')
        elif a == b:
            status = 'same'
        elif c == 'playing':
            # the page's server always has its hosting player "Hoster" too (Host & Play); the original's has only
            # the joiner: compared without that line, the count as the original would print it
            rest = [x for x in b if not x.startswith('Hoster (')]
            n = sum(1 for x in rest if x.endswith(')'))
            count = {0: 'No players connected.', 1: '1 player connected.'}.get(n, f'{n} players connected.')
            same = rest[:-1] + [count] == a
            status = 'same without the hosting player' if same else 'DIFFERENT'
            diffs += 0 if same else 1
        else:
            status = 'DIFFERENT'
            diffs += 1
        if status == 'same':
            print(f'{c!r:32} #{i + 1}  same: {show(a)[:150]}')
        else:
            print(f'{c!r:32} #{i + 1}  {status}\n      original: {show(a)[:400]}\n      port:     {show(b)[:400]}')
oa, pa = sorted(set(orig_async)), sorted(set(port_async))
print('\nlines of other threads (as sets):')
print('  in both:          ' + ' | '.join(x for x in oa if x in pa))
print('  only original:    ' + ' | '.join(x for x in oa if x not in pa))
print('  only port:        ' + ' | '.join(x for x in pa if x not in oa))
print(f'\n{diffs} command output(s) differ')
sys.exit(1 if diffs else 0)
