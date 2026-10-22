"""Real-window test driver for the translated client: a visible, presenting window driven by window messages only.

Built on the sanctioned helper C:\\Users\\user\\.claude\\skills\\bg-window-control\\bgwin.py (its PostMessage / PNG
code is imported). Nothing here calls SendInput, SetCursorPos, SetForegroundWindow or anything else that touches the
real mouse, the keyboard focus or the screen; the window is identified by the PID this script launched.

    python rw.py launch [--volume 0.03] [--name tag] [--size WxH] [KEY=VALUE ...]   start the client (no activation, off screen)
    python rw.py do "<cmd>; <cmd>; ..."                                            run driver commands (below)
    python rw.py kill                                                               end the process by PID, verify
    python rw.py status                                                             pid, exit code, window rect, foreground exe

Driver commands (coordinates are CLIENT pixels = game pixels):
    move X Y | down [left|right|middle] | up [left|right|middle] | click X Y [right] [HOLD_S] | dclick X Y
    drag X1 Y1 X2 Y2 | wheel CLICKS [X Y] | leave
    key CHORD [HOLD_S]      e.g. key enter / key shift+a / key numenter / key num5 / key alt+enter
    keydown K | keyup K     one message each (no repeat)
    hold K SECONDS          key down, the OS auto-repeat (first after 500 ms, then every 33 ms, lParam bit 30), key up
    text STRING             every character as WM_KEYDOWN + WM_KEYUP of its key (TranslateMessage makes the WM_CHAR)
    char STRING             bare WM_CHAR messages (what bgwin `type` sends); \\uXXXX escapes are decoded
    unichar STRING          WM_UNICHAR messages (UTF-32; DefWindowProc turns them into WM_CHAR)
    focus on|off            the messages of an activation / deactivation, in the system's order: WM_ACTIVATEAPP,
                            WM_NCACTIVATE, WM_SETFOCUS / WM_NCACTIVATE, WM_ACTIVATEAPP, WM_KILLFOCUS (posted; the OS
                            focus is not touched). XNA's Game.IsActive follows WM_ACTIVATEAPP alone
    focusonly on|off        only WM_NCACTIVATE + WM_SETFOCUS / WM_KILLFOCUS + WM_NCACTIVATE (no WM_ACTIVATEAPP)
    activateapp on|off      only WM_ACTIVATEAPP
    style                   the window's style bits (GetWindowLong) and rectangles, decoded
    minimize | restore      ShowWindow(SW_SHOWMINNOACTIVE / SW_SHOWNOACTIVATE)
    syscommand minimize     WM_SYSCOMMAND SC_MINIMIZE as a message
    resize W H              SetWindowPos (client size W x H, no activation, no move)
    volume X                output gain 0 .. 0.03 of a client launched with --volume-file (TERRARIA_VOLUME_FILE)
    wait SECONDS | shot NAME [SCALE] | px X Y | waitpx X Y R G B [TOL] [TIMEOUT] | rect | fg | cursor
    dump WORDS...           `dump ...` / `aim ...` through TERRARIA_COMMAND_FILE; prints the JSON line(s)
"""
import ctypes
import ctypes.wintypes as wt
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, r"C:\Users\user\.claude\skills\bg-window-control")
import bgwin  # noqa: E402  (PostMessage helpers, PNG writer; sets the process DPI aware)

u32, k32, g32 = bgwin.u32, bgwin.k32, bgwin.g32
HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.abspath(os.path.join(HERE, "..", "..", "..", "..", ".."))
WORK = os.path.join(PROJECT, "work")
# RW_SAVE: another save root below work\\ (a fresh one for the window-mode and game-pad sessions)
SAVE = os.environ.get("RW_SAVE") or os.path.join(WORK, "port-save-realwin")
SHOTS = os.path.join(WORK, "shots")
LOGS = os.path.join(WORK, "logs")
STATE = os.path.join(WORK, "realwin", "run.json")
CMDFILE = os.path.join(WORK, "realwin", "command.txt")
VOLFILE = os.path.join(WORK, "realwin", "volume.txt")
STATELOG = os.path.join(LOGS, "rw_state.jsonl")
# RW_EXE: another build of the client (a staged copy, so that a rebuild can replace the real one meanwhile)
EXE = os.environ.get("RW_EXE") or os.path.join(PROJECT, "port", "target-app", "client-opt", "terraria.exe")

u32.GetForegroundWindow.restype = wt.HWND
u32.GetClientRect.argtypes = [wt.HWND, ctypes.POINTER(wt.RECT)]
u32.ClientToScreen.argtypes = [wt.HWND, ctypes.POINTER(wt.POINT)]
u32.SetWindowPos.argtypes = [wt.HWND, wt.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wt.UINT]
u32.ShowWindow.argtypes = [wt.HWND, ctypes.c_int]
u32.GetWindowLongW.argtypes = [wt.HWND, ctypes.c_int]
u32.AdjustWindowRectEx.argtypes = [ctypes.POINTER(wt.RECT), wt.DWORD, wt.BOOL, wt.DWORD]
u32.PrintWindow.argtypes = [wt.HWND, wt.HDC, wt.UINT]
u32.VkKeyScanW.argtypes = [wt.WCHAR]
u32.VkKeyScanW.restype = ctypes.c_short
u32.MapVirtualKeyW.argtypes = [wt.UINT, wt.UINT]
u32.GetDC.restype = wt.HDC
u32.GetDC.argtypes = [wt.HWND]
u32.ReleaseDC.argtypes = [wt.HWND, wt.HDC]
g32.CreateCompatibleDC.restype = wt.HDC
g32.CreateCompatibleDC.argtypes = [wt.HDC]
g32.CreateCompatibleBitmap.restype = wt.HBITMAP
g32.CreateCompatibleBitmap.argtypes = [wt.HDC, ctypes.c_int, ctypes.c_int]
g32.SelectObject.argtypes = [wt.HDC, wt.HGDIOBJ]
g32.SelectObject.restype = wt.HGDIOBJ
g32.GetDIBits.argtypes = [wt.HDC, wt.HBITMAP, wt.UINT, wt.UINT, ctypes.c_void_p, ctypes.c_void_p, wt.UINT]
g32.DeleteObject.argtypes = [wt.HGDIOBJ]
g32.DeleteDC.argtypes = [wt.HDC]


class CURSORINFO(ctypes.Structure):
    _fields_ = [("cbSize", wt.DWORD), ("flags", wt.DWORD), ("hCursor", wt.HANDLE), ("pt", wt.POINT)]


# ------------------------------------------------------------------ process / window
def load():
    return json.load(open(STATE, encoding="utf-8"))


def exit_code(pid):
    h = k32.OpenProcess(0x1000, False, pid)
    if not h:
        return "gone"
    code = wt.DWORD()
    k32.GetExitCodeProcess(h, ctypes.byref(code))
    k32.CloseHandle(h)
    return None if code.value == 259 else code.value


def find_window(pid):
    found = []

    def cb(h, _):
        p = wt.DWORD()
        u32.GetWindowThreadProcessId(h, ctypes.byref(p))
        if p.value == pid and u32.IsWindowVisible(h) or (p.value == pid and u32.IsIconic(h)):
            cls = bgwin.text_of(h, u32.GetClassNameW)
            if "IME" not in cls and "MSCTF" not in cls:
                r = wt.RECT()
                u32.GetWindowRect(h, ctypes.byref(r))
                found.append((h, (r.right - r.left) * (r.bottom - r.top)))
        return True

    u32.EnumWindows(bgwin.WNDENUMPROC(cb), 0)
    found.sort(key=lambda f: -f[1])
    return found[0][0] if found else None


def windows_of(pid):
    """Every visible top-level window of the process: (hwnd, (left, top, right, bottom))."""
    out = []

    def cb(h, _):
        p = wt.DWORD()
        u32.GetWindowThreadProcessId(h, ctypes.byref(p))
        if p.value == pid and u32.IsWindowVisible(h):
            r = wt.RECT()
            u32.GetWindowRect(h, ctypes.byref(r))
            out.append((int(h), (r.left, r.top, r.right, r.bottom)))
        return True

    u32.EnumWindows(bgwin.WNDENUMPROC(cb), 0)
    return out


def fg_info():
    h = u32.GetForegroundWindow()
    pid = wt.DWORD()
    u32.GetWindowThreadProcessId(h, ctypes.byref(pid))
    return {"hwnd": int(h or 0), "pid": pid.value, "exe": bgwin.exe_of(h) if h else ""}


def rects(h):
    wr, cr, pt = wt.RECT(), wt.RECT(), wt.POINT(0, 0)
    u32.GetWindowRect(h, ctypes.byref(wr))
    u32.GetClientRect(h, ctypes.byref(cr))
    u32.ClientToScreen(h, ctypes.byref(pt))
    return {"window": [wr.left, wr.top, wr.right - wr.left, wr.bottom - wr.top], "client_origin": [pt.x, pt.y], "client": [cr.right, cr.bottom],
            "iconic": bool(u32.IsIconic(h)), "visible": bool(u32.IsWindowVisible(h))}


def style_of(h):
    """Style bits (GetWindowLong), rectangles, and whether the window touches the real desktop."""
    style = u32.GetWindowLongW(h, -16) & 0xFFFFFFFF
    ex = u32.GetWindowLongW(h, -20) & 0xFFFFFFFF
    bits = {"WS_CAPTION": 0x00C00000, "WS_THICKFRAME": 0x00040000, "WS_SYSMENU": 0x00080000, "WS_MAXIMIZE": 0x01000000, "WS_MINIMIZE": 0x20000000,
            "WS_POPUP": 0x80000000, "WS_VISIBLE": 0x10000000, "WS_MAXIMIZEBOX": 0x00010000, "WS_MINIMIZEBOX": 0x00020000}
    exbits = {"WS_EX_TOPMOST": 0x8, "WS_EX_APPWINDOW": 0x40000, "WS_EX_TOOLWINDOW": 0x80, "WS_EX_NOACTIVATE": 0x08000000, "WS_EX_WINDOWEDGE": 0x100}
    r = rects(h)
    vs = (u32.GetSystemMetrics(76), u32.GetSystemMetrics(77), u32.GetSystemMetrics(78), u32.GetSystemMetrics(79))
    w = r["window"]
    on_desktop = w[0] + w[2] > vs[0] and w[0] < vs[0] + vs[2] and w[1] + w[3] > vs[1] and w[1] < vs[1] + vs[3]
    return {"style": f"{style:#010x}", "set": [k for k, v in bits.items() if style & v == v], "ex": [k for k, v in exbits.items() if ex & v == v],
            "style_borderless": not (style & 0x00C00000 == 0x00C00000 or style & 0x00040000), "borderless": (w[2], w[3]) == tuple(r["client"]) and not r["iconic"], "window": w, "client": r["client"], "client_origin": r["client_origin"],
            "iconic": r["iconic"], "touches_desktop": bool(on_desktop and not r["iconic"]), "foreground_is_client": fg_info()["hwnd"] == int(h)}


def launch(args):
    env = dict(os.environ)
    name = "run"
    volume = None
    size = None
    extra = {}
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--volume":
            volume = args[i + 1]
            i += 1
        elif a == "--volume-file":  # the device is opened silent; `volume X` lets sound out later (at most 0.03)
            volume = "file"
        elif a == "--name":
            name = args[i + 1]
            i += 1
        elif "=" in a:
            k, v = a.split("=", 1)
            extra[k] = v
        i += 1
    for d in (SAVE, os.path.join(SAVE, "Worlds"), SHOTS, LOGS, os.path.dirname(STATE)):
        os.makedirs(d, exist_ok=True)
    # what run-terraria.cmd sets ...
    for k in ("TERRARIA_HIDDEN", "TERRARIA_MUTE", "TERRARIA_VOLUME", "TERRARIA_INPUT_SCRIPT", "TERRARIA_FIXED_DT", "TERRARIA_SCREENSHOT", "TERRARIA_DISPLAY",
              "TERRARIA_KEEP_GOING", "TERRARIA_STRICT", "TERRARIA_GC_FULL", "TERRARIA_GC_VERIFY", "TERRARIA_LOAD_SLICE", "TERRARIA_AUDIO_PCM16"):
        env.pop(k, None)
    env.setdefault("TERRARIA_DIR", r"C:\Users\user\Documents\Games\Terraria")
    env["TERRARIA_SAVE_DIR"] = SAVE
    env["TERRARIA_PROJECT_DIR"] = PROJECT
    env["TERRARIA_FRAME_TIMES"] = "1"
    # ... plus the real-window test switches: no activation, outside the desktop, muted, no listeners
    vs = (u32.GetSystemMetrics(76), u32.GetSystemMetrics(77), u32.GetSystemMetrics(78), u32.GetSystemMetrics(79))
    pos = (vs[0] + vs[2] + 240, vs[1] + vs[3] + 160)
    env["TERRARIA_NOACTIVATE"] = "1"
    env["TERRARIA_WINDOW_POS"] = f"{pos[0]},{pos[1]}"
    env["TERRARIA_NET_LOOPBACK_ONLY"] = "1"
    env["TERRARIA_HOST_REPORT"] = "1"
    env["TERRARIA_FRAME_LOG"] = os.path.join(LOGS, f"rw_{name}.frames.tsv")
    env["TERRARIA_COMMAND_FILE"] = CMDFILE
    env["TERRARIA_STATE_LOG"] = STATELOG
    if volume is None:
        env["TERRARIA_MUTE"] = "1"
    elif volume == "file":
        open(VOLFILE, "w").write("0")
        env["TERRARIA_VOLUME_FILE"] = VOLFILE
    else:
        if float(volume) > 0.03:
            sys.exit("refused: at most 0.03 on the real device")
        env["TERRARIA_VOLUME"] = volume
    env.update(extra)
    if os.path.exists(CMDFILE):
        os.remove(CMDFILE)
    before = fg_info()
    log = open(os.path.join(SAVE, "last-run.log"), "wb")
    si = subprocess.STARTUPINFO()
    si.dwFlags = subprocess.STARTF_USESHOWWINDOW
    si.wShowWindow = 4  # SW_SHOWNOACTIVATE
    p = subprocess.Popen([EXE], env=env, cwd=PROJECT, stdin=subprocess.DEVNULL, stdout=log, stderr=log, startupinfo=si,
                         creationflags=subprocess.CREATE_NO_WINDOW | subprocess.CREATE_NEW_PROCESS_GROUP)
    state = {"pid": p.pid, "name": name, "virtual_screen": vs, "asked_pos": pos, "started": time.time(), "fg_before": before}
    json.dump(state, open(STATE, "w", encoding="utf-8"))
    # the foreground window, sampled for the whole run by a detached watcher (exe names only, no titles)
    subprocess.Popen([sys.executable, os.path.abspath(__file__), "fgwatch", str(p.pid), name], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                     creationflags=subprocess.CREATE_NO_WINDOW | subprocess.CREATE_NEW_PROCESS_GROUP)
    # guard: for the first 40 s every window of the process is looked at every 15 ms; one that is visible inside
    # the virtual screen, or the process becoming the foreground window, ends the process at once
    h = None
    end = time.time() + 40
    seen = []
    while time.time() < end:
        if p.poll() is not None:
            sys.exit(f"the client ended at once with exit code {p.returncode}; see {log.name}")
        bad = None
        for hw, r in windows_of(p.pid):
            if r not in seen:
                seen.append(r)
            if r[2] > vs[0] and r[0] < vs[0] + vs[2] and r[3] > vs[1] and r[1] < vs[1] + vs[3] and (r[2] - r[0]) > 8:
                bad = f"window {hw} at {r} is inside the virtual screen {vs}"
        if fg_info()["pid"] == p.pid:
            bad = "the client became the foreground window"
        if bad:
            subprocess.run(["taskkill", "/PID", str(p.pid), "/F"], capture_output=True)
            sys.exit(f"GUARD: {bad}: process killed. window rects seen: {seen}")
        h = find_window(p.pid)
        time.sleep(0.015)
    after = fg_info()
    state["rects_seen_first_40s"] = seen
    state.update({"hwnd": int(h or 0), "fg_after": after})
    json.dump(state, open(STATE, "w", encoding="utf-8"))
    print(json.dumps({"pid": p.pid, "hwnd": int(h or 0), "fg_before": before, "fg_after": after, "rect": rects(h) if h else None, "virtual_screen": vs}))


def fgwatch(pid, name):
    """One line per change of the foreground window while `pid` lives: time, hwnd, pid, exe. Flags our process."""
    path = os.path.join(LOGS, f"rw_{name}.foreground.log")
    last = None
    samples = 0
    ours = 0
    on_desktop = 0
    vs = (u32.GetSystemMetrics(76), u32.GetSystemMetrics(77), u32.GetSystemMetrics(78), u32.GetSystemMetrics(79))
    with open(path, "w", encoding="utf-8") as f:
        f.write(f"# foreground window while client pid {pid} runs; sampled every 50 ms; exe names only\n")
        f.write(f"# guard for the whole run: a visible window of the client inside the virtual screen {vs}, or the client in the foreground, ends the client\n")
        while exit_code(pid) is None:
            i = fg_info()
            samples += 1
            bad = None
            for hw, r in windows_of(pid):
                if r[2] > vs[0] and r[0] < vs[0] + vs[2] and r[3] > vs[1] and r[1] < vs[1] + vs[3] and (r[2] - r[0]) > 8 and not u32.IsIconic(hw):
                    bad = f"window {hw} at {r} is inside the virtual screen"
            if i["pid"] == pid:
                bad = "the client became the foreground window"
            if bad:
                on_desktop += 1
                subprocess.run(["taskkill", "/PID", str(pid), "/F"], capture_output=True)
                f.write(f"{time.strftime('%H:%M:%S')} GUARD: {bad}: process killed\n")
                f.flush()
            if i["pid"] == pid:
                ours += 1
            key = (i["hwnd"], i["pid"])
            if key != last:
                f.write(f"{time.strftime('%H:%M:%S')} hwnd={i['hwnd']} pid={i['pid']} exe={i['exe']}{'  <-- THE CLIENT IS THE FOREGROUND WINDOW' if i['pid'] == pid else ''}\n")
                f.flush()
                last = key
            time.sleep(0.05)
        f.write(f"# end: {samples} samples, the client was the foreground window in {ours} of them; the guard fired {on_desktop} times; exit code {exit_code(pid)}\n")


# ------------------------------------------------------------------ capture (client area)
def grab(h):
    cr = wt.RECT()
    u32.GetClientRect(h, ctypes.byref(cr))
    w, hh = cr.right, cr.bottom
    if w <= 0 or hh <= 0:
        return 0, 0, b""
    hdc = u32.GetDC(None)
    mdc = g32.CreateCompatibleDC(hdc)
    bmp = g32.CreateCompatibleBitmap(hdc, w, hh)
    g32.SelectObject(mdc, bmp)
    u32.PrintWindow(h, mdc, 3)  # PW_CLIENTONLY | PW_RENDERFULLCONTENT
    bi = bgwin.BIH(ctypes.sizeof(bgwin.BIH), w, -hh, 1, 32, 0)
    buf = (ctypes.c_ubyte * (w * hh * 4))()
    g32.GetDIBits(mdc, bmp, 0, hh, buf, ctypes.byref(bi), 0)
    g32.DeleteObject(bmp)
    g32.DeleteDC(mdc)
    u32.ReleaseDC(None, hdc)
    return w, hh, bytes(buf)


def shot(h, name, scale=1):
    w, hh, data = grab(h)
    if not data:
        print(f"shot {name}: the client area is empty (minimised?)")
        return
    if scale > 1:
        nw, nh = w // scale, hh // scale
        out = bytearray(nw * nh * 4)
        for y in range(nh):
            row = data[(y * scale) * w * 4:(y * scale) * w * 4 + w * 4]
            out[y * nw * 4:(y + 1) * nw * 4] = b"".join(row[x * scale * 4:x * scale * 4 + 4] for x in range(nw))
        w, hh, data = nw, nh, bytes(out)
    path = name if os.path.isabs(name) else os.path.join(SHOTS, name if name.endswith(".png") else name + ".png")
    bgwin.write_png(path, w, hh, data)
    black = not any(data[i] for i in range(0, len(data), 997))
    print(f"shot {path} {w}x{hh}{' (ALL BLACK)' if black else ''}")


# ------------------------------------------------------------------ input (messages only)
WM_MOUSEMOVE, WM_MOUSELEAVE, WM_MOUSEWHEEL = 0x0200, 0x02A3, 0x020A
BTN = {"left": (0x0201, 0x0202, 0x0001), "right": (0x0204, 0x0205, 0x0002), "middle": (0x0207, 0x0208, 0x0010)}
VK = dict(bgwin.NAMED)
VK.update({"lshift": 0xA0, "rshift": 0xA1, "lctrl": 0xA2, "rctrl": 0xA3, "lalt": 0xA4, "ralt": 0xA5, "numenter": 0x0D, "capslock": 0x14,
           "num0": 0x60, "num1": 0x61, "num2": 0x62, "num3": 0x63, "num4": 0x64, "num5": 0x65, "num6": 0x66, "num7": 0x67, "num8": 0x68, "num9": 0x69,
           "nummul": 0x6A, "numadd": 0x6B, "numsub": 0x6D, "numdec": 0x6E, "numdiv": 0x6F})
EXT_NAMES = {"numenter", "rctrl", "ralt", "numdiv"}
# scan codes where MapVirtualKey of the generic VK is not the key meant
SCAN = {"shift": 0x2A, "lshift": 0x2A, "rshift": 0x36, "ctrl": 0x1D, "lctrl": 0x1D, "rctrl": 0x1D, "alt": 0x38, "lalt": 0x38, "ralt": 0x38}


class Win:
    def __init__(self, h):
        self.h = h
        self.pos = (0, 0)
        self.mk = 0  # MK_* flags of the buttons / modifiers held, as the OS puts them into wParam

    def lp(self, x, y):
        return ((y & 0xFFFF) << 16) | (x & 0xFFFF)

    def move(self, x, y):
        self.pos = (x, y)
        u32.PostMessageW(self.h, WM_MOUSEMOVE, self.mk, self.lp(x, y))

    def glide(self, x, y, steps=6, dt=0.012):
        x0, y0 = self.pos
        for i in range(1, steps + 1):
            self.move(x0 + (x - x0) * i // steps, y0 + (y - y0) * i // steps)
            time.sleep(dt)

    def down(self, b="left"):
        d, _, flag = BTN[b]
        self.mk |= flag
        u32.PostMessageW(self.h, d, self.mk, self.lp(*self.pos))

    def up(self, b="left"):
        _, u, flag = BTN[b]
        self.mk &= ~flag
        u32.PostMessageW(self.h, u, self.mk, self.lp(*self.pos))

    def click(self, x, y, b="left", hold=0.08):
        self.glide(x, y)
        time.sleep(0.05)
        self.down(b)
        time.sleep(hold)
        self.up(b)

    def wheel(self, clicks, x=None, y=None):
        if x is not None:
            self.glide(x, y)
            time.sleep(0.03)
        pt = wt.POINT(*self.pos)
        u32.ClientToScreen(self.h, ctypes.byref(pt))
        u32.PostMessageW(self.h, WM_MOUSEWHEEL, ((clicks * 120) & 0xFFFF) << 16 | self.mk, ((pt.y & 0xFFFF) << 16) | (pt.x & 0xFFFF))

    def vk(self, name):
        n = name.lower()
        return VK[n] if n in VK else bgwin.vk_of(n)

    def key_lp(self, name, up=False, repeat=False):
        n = name.lower()
        v = self.vk(n)
        scan = SCAN.get(n) or u32.MapVirtualKeyW(v, 0)
        ext = 1 if (v in bgwin.EXT or n in EXT_NAMES) else 0
        l = 1 | (scan << 16) | (ext << 24)
        if n in ("alt", "lalt", "ralt") or (self.alt and n not in ("alt", "lalt", "ralt")):
            l |= 1 << 29  # context code: Alt is down
        if up:
            l |= 3 << 30
        elif repeat:
            l |= 1 << 30
        return l

    alt = False

    def keydown(self, name, repeat=False):
        n = name.lower()
        if n in ("alt", "lalt", "ralt"):
            self.alt = True
        if n in ("shift", "lshift", "rshift"):
            self.mk |= 0x0004
        if n in ("ctrl", "lctrl", "rctrl"):
            self.mk |= 0x0008
        u32.PostMessageW(self.h, 0x0104 if self.alt else 0x0100, self.vk(n), self.key_lp(n, False, repeat))

    def keyup(self, name):
        n = name.lower()
        msg = 0x0105 if self.alt else 0x0101
        l = self.key_lp(n, True)
        if n in ("alt", "lalt", "ralt"):
            self.alt = False
            msg = 0x0101  # the Alt key itself comes up as WM_KEYUP unless another key was pressed meanwhile (close enough)
        if n in ("shift", "lshift", "rshift"):
            self.mk &= ~0x0004
        if n in ("ctrl", "lctrl", "rctrl"):
            self.mk &= ~0x0008
        u32.PostMessageW(self.h, msg, self.vk(n), l)

    def key(self, chord, hold=0.07):
        parts = chord.split("+")
        for p in parts:
            self.keydown(p)
            time.sleep(0.03)
        time.sleep(hold)
        for p in reversed(parts):
            self.keyup(p)
            time.sleep(0.03)

    def hold(self, name, seconds):
        """What the OS sends for a held key: down, a pause of 500 ms, then repeats at about 30 per second, up."""
        t0 = time.time()
        self.keydown(name)
        nxt = t0 + 0.5
        n = 0
        while time.time() - t0 < seconds:
            if time.time() >= nxt:
                self.keydown(name, repeat=True)
                n += 1
                nxt += 1 / 30
            time.sleep(0.004)
        self.keyup(name)
        return n

    def text(self, s, dt=0.06):
        for ch in s:
            r = u32.VkKeyScanW(ch)
            if r == -1:
                print(f"text: no key for {ch!r}")
                continue
            v, shift = r & 0xFF, (r >> 8) & 1
            name = {0x20: "space", 0x0D: "enter", 0x08: "backspace"}.get(v)
            if shift:
                self.keydown("shift")
                time.sleep(0.02)
            if name:
                self.keydown(name)
                time.sleep(0.03)
                self.keyup(name)
            else:
                scan = u32.MapVirtualKeyW(v, 0)
                u32.PostMessageW(self.h, 0x0100, v, 1 | (scan << 16))
                time.sleep(0.03)
                u32.PostMessageW(self.h, 0x0101, v, 1 | (scan << 16) | (3 << 30))
            if shift:
                time.sleep(0.02)
                self.keyup("shift")
            time.sleep(dt)

    def focus(self, on, app=True, window=True):
        # The shell's Game.IsActive follows WM_ACTIVATEAPP (XNA's rule); winit derives its own focus (key state,
        # modifiers) from WM_NCACTIVATE and WM_SETFOCUS / WM_KILLFOCUS. No WM_ACTIVATE: neither reads it, and
        # DefWindowProc answers a posted WM_ACTIVATE(WA_ACTIVE) on a window that is not really the active one by
        # taking the activation away again (measured: "focus lost" right after it).
        if on:
            if app:
                u32.PostMessageW(self.h, 0x001C, 1, 0)  # WM_ACTIVATEAPP
            if window:
                u32.PostMessageW(self.h, 0x0086, 1, 0)  # WM_NCACTIVATE
                u32.PostMessageW(self.h, 0x0007, 0, 0)  # WM_SETFOCUS
        else:
            if window:
                u32.PostMessageW(self.h, 0x0086, 0, 0)  # WM_NCACTIVATE
            if app:
                u32.PostMessageW(self.h, 0x001C, 0, 0)  # WM_ACTIVATEAPP
            if window:
                u32.PostMessageW(self.h, 0x0008, 0, 0)  # WM_KILLFOCUS

    def resize(self, w, hh):
        style = u32.GetWindowLongW(self.h, -16) & 0xFFFFFFFF
        ex = u32.GetWindowLongW(self.h, -20) & 0xFFFFFFFF
        r = wt.RECT(0, 0, w, hh)
        u32.AdjustWindowRectEx(ctypes.byref(r), style, False, ex)
        # SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE
        u32.SetWindowPos(self.h, None, 0, 0, r.right - r.left, r.bottom - r.top, 0x0002 | 0x0004 | 0x0010)


def dump(words, timeout=20):
    size = os.path.getsize(STATELOG) if os.path.exists(STATELOG) else 0
    tmp = CMDFILE + ".tmp"
    open(tmp, "w", encoding="utf-8").write(words + "\n")
    os.replace(tmp, CMDFILE)
    end = time.time() + timeout
    while time.time() < end:
        if not os.path.exists(CMDFILE) and os.path.exists(STATELOG):
            time.sleep(0.05)
            data = open(STATELOG, encoding="utf-8", errors="replace").read()
            if len(data.encode("utf-8", "replace")) < size:
                size = 0
            new = data.encode("utf-8", "replace")[size:].decode("utf-8", "replace")
            if new.strip():
                return new.strip()
        time.sleep(0.03)
    if os.path.exists(CMDFILE):
        os.remove(CMDFILE)
    return "(no answer)"


def run(win, line, quiet=False):
    a = line.split()
    if not a:
        return
    c = a[0]
    if c == "move":
        win.glide(int(a[1]), int(a[2]))
    elif c == "jump":  # one WM_MOUSEMOVE, no path
        win.move(int(a[1]), int(a[2]))
    elif c == "down":
        win.down(a[1] if len(a) > 1 else "left")
    elif c == "up":
        win.up(a[1] if len(a) > 1 else "left")
    elif c == "click":
        rest = a[3:]
        b = "right" if "right" in rest else "middle" if "middle" in rest else "left"
        hold = [float(v) for v in rest if v.replace(".", "").isdigit()]
        win.click(int(a[1]), int(a[2]), b, hold[0] if hold else 0.08)
    elif c == "dclick":
        win.click(int(a[1]), int(a[2]))
        time.sleep(0.09)
        win.down()
        time.sleep(0.06)
        win.up()
    elif c == "drag":
        win.glide(int(a[1]), int(a[2]))
        time.sleep(0.05)
        win.down()
        time.sleep(0.08)
        win.glide(int(a[3]), int(a[4]), steps=12, dt=0.02)
        time.sleep(0.08)
        win.up()
    elif c == "wheel":
        win.wheel(int(a[1]), *(int(v) for v in a[2:4]))
    elif c == "leave":
        u32.PostMessageW(win.h, WM_MOUSELEAVE, 0, 0)
    elif c == "key":
        win.key(a[1], float(a[2]) if len(a) > 2 else 0.07)
    elif c == "keydown":
        win.keydown(a[1])
    elif c == "keyup":
        win.keyup(a[1])
    elif c == "hold":
        n = win.hold(a[1], float(a[2]))
        if not quiet:
            print(f"hold {a[1]}: {n} repeats")
    elif c == "text":
        win.text(line.split(None, 1)[1])
    elif c == "char":
        text = line.split(None, 1)[1].encode("ascii", "backslashreplace").decode("unicode_escape")
        for unit in memoryview(text.encode("utf-16-le")).cast("H"):
            u32.PostMessageW(win.h, 0x0102, unit, 1)
            time.sleep(0.03)
    elif c == "unichar":
        text = line.split(None, 1)[1].encode("ascii", "backslashreplace").decode("unicode_escape")
        for ch in text:
            u32.PostMessageW(win.h, 0x0109, ord(ch), 1)
            time.sleep(0.03)
    elif c == "focus":
        win.focus(a[1] == "on")
    elif c == "focusonly":
        win.focus(a[1] == "on", app=False)
    elif c == "activateapp":
        win.focus(a[1] == "on", window=False)
    elif c == "style":
        print("style", json.dumps(style_of(win.h)))
    elif c == "post":  # post MSG WPARAM LPARAM (numbers; one raw message)
        u32.PostMessageW(win.h, int(a[1], 0), int(a[2], 0), int(a[3], 0))
    elif c == "minimize":
        u32.ShowWindow(win.h, 7)  # SW_SHOWMINNOACTIVE
    elif c == "restore":
        u32.ShowWindow(win.h, 4)  # SW_SHOWNOACTIVATE
    elif c == "syscommand":
        u32.PostMessageW(win.h, 0x0112, {"minimize": 0xF020}[a[1]], 0)
    elif c == "resize":
        win.resize(int(a[1]), int(a[2]))
    elif c == "wait":
        time.sleep(float(a[1]))
    elif c == "volume":  # output gain of a client started with --volume-file
        if float(a[1]) > 0.03:
            sys.exit("refused: at most 0.03 on the real device")
        open(VOLFILE, "w").write(a[1])
    elif c == "shot":
        shot(win.h, a[1], int(a[2]) if len(a) > 2 else 1)
    elif c == "crop":  # crop NAME X Y W H: a region of the client area at full resolution
        w, hh, data = grab(win.h)
        x0, y0, cw, ch = (int(v) for v in a[2:6])
        out = b"".join(data[((y0 + y) * w + x0) * 4:((y0 + y) * w + x0 + cw) * 4] for y in range(ch))
        path = os.path.join(SHOTS, a[1] + ".png")
        bgwin.write_png(path, cw, ch, out)
        print(f"crop {path} {cw}x{ch}")
    elif c == "px":
        w, hh, data = grab(win.h)
        print("px", a[1], a[2], *bgwin.px(data, w, int(a[1]), int(a[2])))
    elif c == "waitpx":
        x, y, r, g, b = (int(v) for v in a[1:6])
        tol = int(a[6]) if len(a) > 6 else 24
        end = time.time() + (float(a[7]) if len(a) > 7 else 30)
        ok = False
        while time.time() < end and not ok:
            w, hh, data = grab(win.h)
            p = bgwin.px(data, w, x, y)
            ok = all(abs(p[i] - (r, g, b)[i]) <= tol for i in range(3))
            if not ok:
                time.sleep(0.25)
        print("waitpx", "match" if ok else "TIMEOUT", *p)
    elif c == "rect":
        print("rect", json.dumps(rects(win.h)))
    elif c == "fg":
        print("fg", json.dumps(fg_info()))
    elif c == "cursor":
        ci = CURSORINFO()
        ci.cbSize = ctypes.sizeof(CURSORINFO)
        u32.GetCursorInfo(ctypes.byref(ci))
        print("cursor", json.dumps({"system_cursor_showing": bool(ci.flags & 1), "real_pointer": [ci.pt.x, ci.pt.y]}))
    elif c in ("dump", "aim"):
        print(dump(line))
    else:
        sys.exit(f"unknown driver command {c!r}")


def window_or_exit():
    st = load()
    code = exit_code(st["pid"])
    if code is not None:
        sys.exit(f"the client (pid {st['pid']}) is not running: exit code {code}")
    h = find_window(st["pid"])
    if not h:
        sys.exit(f"pid {st['pid']} has no window")
    return st, Win(h)


def main():
    a = sys.argv[1:]
    if not a or a[0] in ("-h", "--help"):
        print(__doc__)
    elif a[0] == "launch":
        launch(a[1:])
    elif a[0] == "fgwatch":
        fgwatch(int(a[1]), a[2])
    elif a[0] == "status":
        st = load()
        h = find_window(st["pid"])
        print(json.dumps({"pid": st["pid"], "exit_code": exit_code(st["pid"]), "rect": rects(h) if h else None, "fg": fg_info()}))
    elif a[0] == "kill":
        st = load()
        # a process with a large heap can take a while to go (seen: more than a minute after a session in a world):
        # ask again every 5 s and wait for it (up to 3 minutes) before reporting
        for i in range(1800):
            if i % 50 == 0:
                subprocess.run(["taskkill", "/PID", str(st["pid"]), "/F"], capture_output=True)
            if exit_code(st["pid"]) is not None:
                break
            time.sleep(0.1)
        # the exit code is set as soon as the termination starts; the process object stays in the task list
        # until its memory is released (seconds, with a 2 GB heap): wait for that too
        for _ in range(120):
            out = subprocess.run(["tasklist", "/FI", f"PID eq {st['pid']}"], capture_output=True, text=True).stdout
            if str(st["pid"]) not in out:
                break
            time.sleep(0.5)
        print("killed" if str(st["pid"]) not in out else "STILL RUNNING", "| tasklist:", out.strip().splitlines()[-1] if out.strip() else "")
        out = subprocess.run(["tasklist", "/FI", "IMAGENAME eq terraria.exe"], capture_output=True, text=True).stdout
        print("terraria.exe processes:", out.strip().splitlines()[-1] if out.strip() else "none")
    elif a[0] == "do":
        st, win = window_or_exit()
        # the last cursor position / button state of the previous call
        mem = os.path.join(os.path.dirname(STATE), "mouse.json")
        if os.path.exists(mem):
            m = json.load(open(mem))
            if m.get("pid") == st["pid"]:
                win.pos, win.mk = tuple(m["pos"]), m["mk"]
        for line in " ".join(a[1:]).split(";"):
            run(win, line.strip())
            time.sleep(0.03)
        json.dump({"pid": st["pid"], "pos": win.pos, "mk": win.mk}, open(mem, "w"))
    else:
        sys.exit("unknown command; --help")


if __name__ == "__main__":
    main()
