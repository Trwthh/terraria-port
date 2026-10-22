"""The ORIGINAL dedicated server's console, scripted, for comparison with the browser port's server console
(web/STATUS.md 2026-10-14 "Server console: every command"; run-host-console-mp.sh is the port's side).

    python web/www/test/tools/orig_console.py <steps file> [--limit 600] [--log work/logs/orig_console.txt]

Server: work/refserver/TerrariaServer.exe (private copy of the install's server; the install is not touched), no
window, working directory and -savedirectory under work/console-orig (a previous one is moved to
work/port-save-runs/_old), -world <copy of work/refsave/Worlds/ref_small.wld> -port 47801 -ip 127.0.0.1 -noupnp
-password "" and NO -autoshutdown (so its console input thread runs). stdin is a pipe this script writes, in
UTF-16LE (Program.InitializeConsoleOutput sets Console.InputEncoding = Encoding.Unicode on Windows: UTF-8 lines are
never split, the whole input arrives as one line at end of file).
The original announces itself on the LAN (UDP broadcast, Netplay.BroadcastThread) while it runs: keep runs short.
Joiners: the port's native client (port/target-app/client/terraria.exe), hidden, muted, loopback only, scripted.

Steps file, one per line ('#' comment):
  send <line>                the line on the server's stdin (a console command); <sp> is a space (a trailing one)
  expect <seconds> <text>    wait until the server's output after the last expect shows <text> (fails otherwise)
  sleep <seconds>
  join <input script>        start a native client with that TERRARIA_INPUT_SCRIPT (path relative to the project)
  joined <seconds>           wait for the last started client to exit (fails on a time-out or a non-zero exit)
  file <name> <text>         fail unless <scratch>/<name> exists and contains <text>
Every send is written to the transcript as "> <line>", the server's output as it comes; ends with "exit-nosave"
if the steps did not stop the server; the server and every client are killed at the end in any case.
"""
import os
import shutil
import subprocess
import sys
import threading
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..', '..'))
args = sys.argv[1:]
STEPS = args[0]
LIMIT = float(args[args.index('--limit') + 1]) if '--limit' in args else 600
LOG = os.path.join(ROOT, args[args.index('--log') + 1]) if '--log' in args else os.path.join(ROOT, 'work', 'logs', 'orig_console.txt')
SERVER = os.path.join(ROOT, 'work', 'refserver', 'TerrariaServer.exe')
CLIENT = os.path.join(ROOT, 'port', 'target-app', 'client', 'terraria.exe')
SCRATCH = os.path.join(ROOT, 'work', 'console-orig')
OLD = os.path.join(ROOT, 'work', 'port-save-runs', '_old')
STAMP = time.strftime('%Y%m%d-%H%M%S')
NOWIN = 0x08000000  # CREATE_NO_WINDOW (it needs a console: Console.Title; DETACHED_PROCESS crashes there)

os.makedirs(OLD, exist_ok=True)
if os.path.isdir(SCRATCH):
    os.rename(SCRATCH, os.path.join(OLD, f'console-orig-{STAMP}'))
os.makedirs(os.path.join(SCRATCH, 'save'))
os.makedirs(os.path.join(SCRATCH, 'Worlds'))
shutil.copy(os.path.join(ROOT, 'work', 'refsave', 'Worlds', 'ref_small.wld'), os.path.join(SCRATCH, 'Worlds', 'ref_small.wld'))

out_lock = threading.Lock()
output = []  # the transcript (str pieces)
server_text = ['']  # the server's output only
log = open(LOG, 'w', encoding='utf-8', newline='\n')


def note(s):
    with out_lock:
        output.append(s)
        log.write(s)
        log.flush()


srv = subprocess.Popen([SERVER, '-world', os.path.join(SCRATCH, 'Worlds', 'ref_small.wld'), '-port', '47801', '-ip', '127.0.0.1',
                        '-noupnp', '-password', '', '-savedirectory', os.path.join(SCRATCH, 'save')],
                       cwd=SCRATCH, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, creationflags=NOWIN)
print(f'orig_console: server pid {srv.pid}, scratch {SCRATCH}, transcript {LOG}')


def reader():
    while True:
        b = srv.stdout.read1(4096) if hasattr(srv.stdout, 'read1') else srv.stdout.read(1)
        if not b:
            break
        s = b.decode('utf-8', 'replace').replace('\r\n', '\n')
        with out_lock:
            server_text[0] += s
        note(s)


threading.Thread(target=reader, daemon=True).start()
clients = []
mark = 0
t0 = time.time()
fail = None


def wait_text(sec, text):
    global mark
    end = time.time() + sec
    while time.time() < end:
        with out_lock:
            at = server_text[0].find(text, mark)
        if at >= 0:
            mark = at + len(text)
            return True
        if srv.poll() is not None:
            time.sleep(0.5)
            with out_lock:
                at = server_text[0].find(text, mark)
            if at >= 0:
                mark = at + len(text)
                return True
            return False
        time.sleep(0.1)
    return False


try:
    for raw in open(STEPS, encoding='utf-8'):
        line = raw.rstrip('\n')
        if not line.strip() or line.lstrip().startswith('#'):
            continue
        if time.time() - t0 > LIMIT:
            fail = f'over the limit of {LIMIT} s'
            break
        cmd, _, rest = line.partition(' ')
        if cmd == 'send':
            rest = rest.replace('<sp>', ' ')
            note(f'\n> {rest}\n')
            srv.stdin.write((rest + '\r\n').encode('utf-16-le'))  # Program.InitializeConsoleOutput: InputEncoding = Unicode on Windows
            srv.stdin.flush()
        elif cmd == 'expect':
            sec, _, text = rest.partition(' ')
            ok = wait_text(float(sec), text)
            print(f'orig_console: expect "{text}": {"seen" if ok else "NOT seen"}')
            if not ok:
                fail = f'expect "{text}" not seen within {sec} s'
                break
        elif cmd == 'sleep':
            time.sleep(float(rest))
        elif cmd == 'join':
            n = len(clients) + 1
            save = os.path.join(SCRATCH, f'client{n}')
            os.makedirs(os.path.join(save, 'Worlds'), exist_ok=True)
            env = dict(os.environ, TERRARIA_NET_LOOPBACK_ONLY='1', TERRARIA_MUTE='1', TERRARIA_HIDDEN='1', TERRARIA_SAVE_DIR=save,
                       TERRARIA_INPUT_SCRIPT=os.path.join(ROOT, rest), TERRARIA_STATE_LOG=os.path.join(ROOT, 'work', 'logs', f'orig_console_client{n}.state.jsonl'))
            clog = open(os.path.join(ROOT, 'work', 'logs', f'orig_console_client{n}.log'), 'w')
            p = subprocess.Popen([CLIENT], cwd=os.path.join(ROOT, 'port'), env=env, stdout=clog, stderr=subprocess.STDOUT, creationflags=NOWIN)
            clients.append(p)
            print(f'orig_console: client {n} pid {p.pid} ({rest})')
        elif cmd == 'joined':
            p = clients[-1]
            try:
                r = p.wait(float(rest))
            except subprocess.TimeoutExpired:
                fail = f'client {len(clients)} still running after {rest} s'
                break
            print(f'orig_console: client {len(clients)} exit {r}')
            if r != 0:
                fail = f'client {len(clients)} exit {r} (work/logs/orig_console_client{len(clients)}.log)'
                break
        elif cmd == 'file':
            name, _, text = rest.partition(' ')
            path = os.path.join(SCRATCH, name)
            body = open(path, encoding='utf-8', errors='replace').read() if os.path.isfile(path) else None
            print(f'orig_console: file {name}: {body!r}')
            if body is None or text not in body:
                fail = f'file {name} does not contain "{text}"'
                break
        else:
            fail = f'unknown step {line!r}'
            break
finally:
    if srv.poll() is None and fail is None:
        try:
            note('\n> exit-nosave\n')
            srv.stdin.write('exit-nosave\r\n'.encode('utf-16-le'))
            srv.stdin.flush()
            srv.wait(30)
        except Exception:
            pass
    for p in clients:
        if p.poll() is None:
            p.kill()
    if srv.poll() is None:
        srv.kill()
    srv.wait(10)
    time.sleep(0.5)
    log.close()
print(f'orig_console: server exit {srv.returncode}, {time.time() - t0:.0f} s' + (f'; FAILED: {fail}' if fail else '; ok'))
sys.exit(1 if fail else 0)
