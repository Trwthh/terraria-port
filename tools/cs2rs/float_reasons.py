#!/usr/bin/env python3
"""Why each float method without a reference (float_uncovered.tsv, last column '-') has none: one reason per method,
determined from the binary, the call graph and the reference runs' machine code. Run after float_coverage.py.

    python tools/cs2rs/float_reasons.py

Adds two columns to float_uncovered.tsv (reason code, evidence) for the uncovered rows ('' for covered rows) and writes
the per-reason summary between the markers of FLOAT_COVERAGE.md. Reason codes, in the order they are decided:

  dead        no path from the program: no call / callvirt / newobj / ldftn of it anywhere in Terraria.exe (IL call
              graph, db port_calls), it is not virtual (no dispatch can reach it), not a static constructor, or every
              method that calls it is itself dead (the chain is listed)
  inlined     every covered caller that calls it compiled no call instruction to it in any reference process that ran
              that caller (its IL is part of that caller's machine code, whose facts the tables hold); evidence: the
              callers and the dumps
  platform    only on a platform or service the port does not run (Steam, WeGame, Xbox / Windows store, the RGB
              keyboard SDKs, the dedicated server's console), with the guard that selects it
  notrun      reachable, no reference run executes it; evidence: the callers and, for a covered caller, the guard of
              the call site (decompiled statement and the nearest enclosing condition)
"""
import collections
import glob
import json
import os
import re
import sqlite3
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..'))
sys.path.insert(0, os.path.join(HERE, '..', 'refdump_worldgen'))
sys.path.insert(0, HERE)
import jit_calls  # noqa: E402
import float_coverage as fc  # noqa: E402

TSV = os.path.join(HERE, 'float_uncovered.tsv')
MD = os.path.join(HERE, 'FLOAT_COVERAGE.md')
SEQ = os.path.join(ROOT, 'work', 'pdb', 'pdb_seq.tsv')

PRIM = {'Single': 'float', 'Double': 'double', 'Int32': 'int', 'Boolean': 'bool', 'Byte': 'byte', 'Int16': 'short',
        'UInt16': 'ushort', 'Int64': 'long', 'UInt32': 'uint', 'String': 'string', 'Object': 'object', 'Char': 'char',
        'SByte': 'sbyte', 'UInt64': 'ulong', 'Void': 'void'}

# platform guards (decompiled source, checked by hand; the port runs SocialMode.None, no store, no RGB SDK, a client)
PLATFORM = [
    ('Terraria.Social.Steam', 'Steam only: SocialAPI.Initialize creates the Steam modules only for SocialMode.Steam (Social/SocialAPI.cs, `case SocialMode.Steam`); the port starts with SocialMode.None'),
    ('Terraria.Social.WeGame', 'WeGame (China) only: SocialAPI.Initialize, `case SocialMode.WeGame` (Social/SocialAPI.cs); the port starts with SocialMode.None'),
    ('Terraria.GameContent.UI.States.AWorkshopPublishInfoState', 'Steam Workshop publishing: opened only from the workshop menu, which needs SocialAPI.Workshop (null unless SocialMode.Steam, Social/SocialAPI.cs)'),
    ('Terraria.GameContent.UI.States.WorkshopPublish', 'Steam Workshop publishing (SocialAPI.Workshop, Steam only)'),
    # (a trailing '*': every type whose full name starts with the rest)
    ('Terraria.GameContent.UI.States.UIWorkshop*', 'Steam Workshop menus: the hub (UIWorkshopHub) is created only by the title menu under `if (SocialAPI.Workshop != null)` (Main.cs:49799-49808), and every other UIWorkshop* state and list item only from the hub; SocialAPI.Workshop is null unless SocialMode.Steam (Social/SocialAPI.cs)'),
    ('Terraria.GameContent.UI.Elements.UIWorkshop*', 'Steam Workshop list items: created only by the UIWorkshop* states, reached only from UIWorkshopHub (SocialAPI.Workshop != null, Steam only: Main.cs:49799-49808)'),
    ('Terraria.GameContent.RGB', 'RGB keyboard / mouse lighting (Chroma, Logitech, Corsair, Razer SDKs): ChromaInitializer registers these shaders only when the device SDK is present (Initializers/ChromaInitializer.cs); the port has no RGB device'),
]


# Reachable methods whose condition no reference run can meet, each with the reason and its source (checked by hand).
# (type, method names or None for every method of the type and its nested types, reason code, evidence)
OVERRIDES = [
    ('Terraria.GameContent.UI.States.UIWorldGenDebug', None, 'dead',
     'only with DebugOptions.enableDebugCommands, which is false and never set (Testing/DebugOptions.cs:8; read at WorldBuilding/WorldGenerator.cs:372-379, UIWorldCreation.cs:999): the world generation debug view cannot open in this build'),
    ('Terraria.Testing.ChatCommands', None, 'dead',
     'debug chat commands: DebugCommandProcessor runs them only with DebugOptions.enableDebugCommands (Testing/ChatCommands/DebugCommandProcessor.cs:30), false and never set (Testing/DebugOptions.cs:8)'),
    ('Terraria.Main', ['DedServ', 'startDedInput', 'startDedInputCallBack'], 'unrunnable',
     'the frame loop of the dedicated server process and console reader (Program.cs:229 main.DedServ(); Main.cs:5900 startDedInput): DedServ paces Main.Update by the wall clock (Stopwatch, Thread.Sleep) and startDedInputCallBack blocks on Console.ReadLine on its own thread; a JIT reference run cannot pin either (the server reference runs call Main.Update tick by tick instead). The console commands themselves are compared with the original another way: the unmodified TerrariaServer.exe and the port given the same commands give the same transcript (web/www/test/tools/console_diff.py: 0 differences, VERIFICATION.md / web/STATUS.md 2026-10-14), but that process is not a JIT dump'),
    ('Terraria.IO.WorldFile', ['LoadWorld_Version1_Old_BeforeRelease88'], 'unrunnable',
     'needs a world file of release 87 or older (WorldFile.cs:721); this build writes release 326 only and the install has no such file'),
    ('Terraria.Testing.DetailedFPS', None, 'unrunnable',
     'the F7 diagnostics page 1 (Main.drawDiag == 1): it shows what WindowsPerformanceDiagnostics reads from this machine (user32 EnumDisplaySettings, the registry, process times; Testing/WindowsPerformanceDiagnostics.cs:340-380); a translated build has no such values to give (Marshal.SizeOf of the Win32 DEVMODE, the P/Invoke) and the numbers belong to the machine, not to the game'),
    ('Terraria.Testing.WindowsPerformanceDiagnostics', None, 'unrunnable',
     'reads this machine through Win32 (EnumDisplaySettings, the registry, process affinity): Testing/WindowsPerformanceDiagnostics.cs; machine state, not game state'),
    ('Terraria.TimeLogger', ['DrawTimes', 'DrawExtras'], 'unrunnable',
     'the F7 diagnostics page 2 (Main.drawDiag == 2, Main.cs:63500) draws the times TimeLogger measured with its own Stopwatch objects: two runs of the same scenario differ there (the coverage run tried it: its JIT twin diag_2 frame differed from the reference run), so no reference can be compared bit for bit'),
    ('Terraria.WorldGen', ['playWorldCallBack'], 'unrunnable',
     'the world loading thread (WorldGen.playWorld queues it on the thread pool, WorldGen.cs:6823): its only float statements are the map loading percentage inside `while (Main.loadMapLock)` (WorldGen.cs:6800-6808), which spins while another thread loads the map, and it ends in Main.RunOnMainThread(..).Wait() (6815-6820), which blocks until the main thread runs the queued action; the reference harness runs the same statements on the main thread instead (tools/refdump_draw/RefDumpDraw.cs EnterWorld, the comment "WorldGen.playWorldCallBack (WorldGen.cs:6732), on this thread"), and the port enters the world the same way, so the timing of the thread is never part of a run'),
    ('Terraria.Main', ['EndDraw'], 'unrunnable',
     'the override of Game.EndDraw that XNA calls after each Draw: it paces the frame by the wall clock (Stopwatch timestamps into UpdateTimeAccumulator, the subtle frame skip loop) and reads DetailedFPS frame times before presenting (Main.cs:62661-62700); the reference harness and the port present each frame themselves after Draw (tools/refdump_draw/RefDumpDraw.cs:311-313 XnaStandIn.Host.EndDraw, tests/draw.rs tick: hg::end_draw), so no run calls it, and what it decides depends on the time the frame took'),
    ('Terraria.Main', ['SetDisplayModeAsBorderless'], 'unrunnable',
     'sizes the borderless window to the monitor it is on: System.Windows.Forms.Screen.FromPoint(form.Location).Bounds (Main.cs:53197), and only while Main.screenBorderless with resizes pending and not full screen (Main.cs:53194); the result is the bounds of the display of this machine, not game state, and the reference runs have no window on a monitor'),
    ('Terraria.GameContent.UI.States.UICharacterCreation', ['Click_PastePlayerTemplate'], 'unrunnable',
     'reads the operating system clipboard: Platform.Get<IClipboard>().Value (GameContent/UI/States/UICharacterCreation.cs:1426); the text there belongs to the desktop session of the user, and the reference runs must not read or replace it'),
    ('Terraria.Map.MapHelper', ['LoadMapVersion1', 'LoadMapVersionCompressed'], 'unrunnable',
     'needs a map file of release 91 or older, or one with the 0x8000 "compressed" flag (Map/WorldMap.cs:197-208); this build writes release 326 without the flag (Map/MapHelper.cs:3067), so only map files of old releases reach it'),
]


# Classes no live code can construct (checked by hand: no `new` of them that runs, no reflection that creates them):
# every method of the type is dead, its virtual overrides included. The script checks that each constructor has no
# caller outside dead code and the type itself, and lists the constructors' callers in the evidence.
DEAD_TYPES = [
    ('Terraria.Cinematics.DD2Film', 'no `new DD2Film` anywhere in Terraria.exe: the film is never created (CinematicManager plays only what is added to it)'),
    ('Terraria.GameContent.UI.States.UITextWrappingTest', 'a test screen: no `new UITextWrappingTest` anywhere in Terraria.exe'),
    ('Terraria.GameContent.UI.States.UITextWrappingTest+TestElement', 'created only by UITextWrappingTest, which is never created'),
    ('Terraria.GameContent.UI.LegacyMultiplayerClosePlayersOverlay', 'never created: Main uses NewMultiplayerClosePlayersOverlay (no `new LegacyMultiplayerClosePlayersOverlay` in Terraria.exe)'),
    ('Terraria.GameContent.UI.Chat.LegacyChatMonitor', 'never created: Main.chatMonitor is a RemadeChatMonitor (Main.cs:2380); no `new LegacyChatMonitor` in Terraria.exe'),
    ('Terraria.GameContent.Drawing.EmptyHorizonRenderer', 'created only by Main.ToggleHorizonRenderer (Main.cs:17739), which nothing calls'),
    ('Terraria.UI.InGamePopups+PlayerWantsToJoinGamePopup', 'created only by InGameNotificationsTracker.AddJoinRequest for ServerJoinRequestsManager.OnRequestAdded, raised only by ServerJoinRequestsManager.Add (Social/Base/ServerJoinRequestsManager.cs:33), which nothing in Terraria.exe calls', ['UI.InGameNotificationsTracker.AddJoinRequest']),
    ('Terraria.GameContent.UI.Elements.UIDebugCommandItem', 'created only by UIDebugCommandsList, opened only by the debug chat command of ToolkitDebugCommands (Testing/ChatCommands/ToolkitDebugCommands.cs:45), which runs only with DebugOptions.enableDebugCommands (false, never set)'),
    ('Terraria.GameContent.Bestiary.UnlockProgressDisplayBestiaryInfoElement', 'no `new UnlockProgressDisplayBestiaryInfoElement` anywhere in Terraria.exe (the type is named only in its own file): never created'),
    ('Terraria.GameContent.Bestiary.UnlockProgressDisplayBestiaryInfoElement+<>c__DisplayClass5_0', 'a closure of UnlockProgressDisplayBestiaryInfoElement.AddDynamicResize, which is never created'),
    ('Terraria.WorldBuilding.Shapes+HalfCircle', 'created only by DungeonUtils.GenerateShimmerPool (GameContent/Generation/Dungeon/DungeonUtils.cs:269-270), which nothing in Terraria.exe calls'),
    ('Terraria.GameContent.Bestiary.ItemFromCatchingNPCBestiaryInfoElement', 'no `new ItemFromCatchingNPCBestiaryInfoElement` anywhere in Terraria.exe (the type is named only in its own file): never created'),
    ('Terraria.GameContent.UI.Elements.UIBestiaryInfoLine`1', 'created only by ItemFromCatchingNPCBestiaryInfoElement.ProvideUIElement (GameContent/Bestiary/ItemFromCatchingNPCBestiaryInfoElement.cs:22), a class that is never created'),
    ('Terraria.GameContent.UI.Elements.UISelectableTextPanel`1', 'no `new UISelectableTextPanel<T>` anywhere in Terraria.exe (no instantiation of the generic type, no subclass): never created'),
    ('Terraria.GameContent.UI.Elements.UIDebugLoadoutItem', 'created only by UIDebugLoadoutsList, opened only by the debug chat command of ToolkitDebugCommands (Testing/ChatCommands/ToolkitDebugCommands.cs:887), which runs only with DebugOptions.enableDebugCommands (false, never set)'),
]
# Single methods unreachable in this build for a reason the call graph cannot see (a flag that is never set),
# checked by hand: (type, method, evidence)
DEAD_METHODS = [
    ('Terraria.Main', 'DrawDebugSectionsOnMap', 'called only under `if (DebugOptions.ShowSections)` (Main.cs:57229); DebugOptions.ShowSections is set only by a debug chat command (Testing/ChatCommands/ToolkitDebugCommands.cs:476), which runs only with DebugOptions.enableDebugCommands (false, never set)'),
    ('Terraria.Main', 'DrawUnbreakableWallScansOnMap', 'called only under `if (DebugOptions.ShowUnbreakableWall)` (Main.cs:57233); set only by a debug chat command (Testing/ChatCommands/ToolkitDebugCommands.cs:645), which runs only with DebugOptions.enableDebugCommands (false, never set)'),
    ('Terraria.Testing.DebugUtils', 'GetTileDescription', 'called only by Main.DrawMap under `if (UIWorldGenDebug.ActiveInstance != null ...` (Main.cs:56718); the world generation debug view opens only with DebugOptions.enableDebugCommands (false, never set)'),
    ('Terraria.Graphics.Light.LegacyLighting', 'doColors_Mode0_Swipe', 'LegacyLighting.DoColors picks it for Mode 0 (Graphics/Light/LegacyLighting.cs:1077-1093); the only writer of LegacyLighting.Mode is the setter of Lighting.Mode (Lighting.cs:37-57), which sets Mode 0 only together with _activeEngine = NewEngine (LightMode.Color), and the legacy engine lights only as the active engine: it never processes with Mode 0'),
    ('Terraria.GameContent.Generation.Dungeon.Rooms.BiomeSquareDungeonRoom', 'BiomeRoom', 'a BiomeSquareDungeonRoom is created only for DungeonRoomType.BiomeSquare (GameContent/Generation/Dungeon/DungeonCrawler.cs:887-888), and no room ever has that type: MakeDungeon_GetRoomSettings is called with the BiomeRoomType of a style (BiomeRugged or BiomeStructured in every style, DungeonGenerationStyles.cs:103-718), DualDungeonLayout_GetGeneralRoomType (Legacy, Regular, Wormlike or a GenShape type, LayoutProviders/DualDungeonLayoutProvider.cs:572-591) or LivingTree (:750, :852), and the other rooms are built from Legacy / Regular settings directly'),
    ('Terraria.Graphics.Renderers.LittleFlyingCritterParticle', 'Draw_ButterFly', 'called only under `case FlyType.ButterFly` (Graphics/Renderers/LittleFlyingCritterParticle.cs:131); _type is set only by Prepare, and every Prepare call in Terraria.exe passes FlyType.RegularFly (GameContent/Drawing/ParticleOrchestrator.cs:1331 Spawn_NatureFly, :2332 Spawn_PooFly): no particle is ever a butterfly'),
    ('Terraria.WorldBuilding.WorldGenerator+Controller', 'ForceUpdateProgress', 'called only by Controller.OnPaused (WorldBuilding/WorldGenerator.cs:118) and the world generation debug view (UIWorldGenDebug.cs:702, 714); OnPaused runs only while Controller.Paused (WorldGenerator.cs:440-442), which is set only by UIWorldCreation under DebugOptions.enableDebugCommands (UIWorldCreation.cs:999), by a pass hash mismatch with PauseOnHashMismatch (WorldGenerator.cs:197-200; PauseOnHashMismatch is set only by UIWorldGenDebug, UIWorldGenDebug.cs:994 and 1583, or copied from it by QuickLoad), or by the regeneration of QuickLoad from the debug view (Testing/QuickLoad.cs:197); DebugOptions.enableDebugCommands is false and never set (Testing/DebugOptions.cs:8)'),
    ('Terraria.GameContent.Drawing.HorizonHelper', 'Test_DrawSmallTarget', 'called only under `if (DebugSunVisibility)` (GameContent/Drawing/HorizonHelper.cs:62); HorizonHelper.DebugSunVisibility is initialised false (line 9) and never assigned anywhere in Terraria.exe'),
]


def platform_of(tname):
    """The PLATFORM reason of a type name, or None."""
    for pref, why in PLATFORM:
        if pref.endswith('*'):
            if tname.startswith(pref[:-1]):
                return why
            continue
        if tname == pref or tname.startswith(pref + '.') or tname.startswith(pref + '`') or tname.startswith(pref + '+') or (pref.endswith('State') and tname.startswith(pref)):
            return why
    return None


def override(tname, mname):
    for t, ms, code, why in OVERRIDES:
        if (tname == t or tname.startswith(t + '+') or tname.startswith(t + '.')) and (ms is None or mname in ms):
            return code, why
    return None


def il_fnv(il):
    return '%016x' % fc.fnv(fc.normalized(il))


def short_clr(t):
    arr = ''
    while t.endswith('[]'):
        arr += '[]'
        t = t[:-2]
    byref = t.endswith('&')
    t = t.rstrip('&')
    t = t.split('[[')[0].split('`')[0]
    t = re.split(r'[.+]', t)[-1]
    return ('ref ' if byref else '') + PRIM.get(t, t) + arr


def main():
    db = sqlite3.connect(fc.DB)
    pe = fc.Pe(fc.EXE)
    rows = [l.rstrip('\n').split('\t') for l in open(TSV, encoding='utf-8')]
    rows = [r[:6] for r in rows]
    meth = {}
    for tok, tname, name, params, rva, attrs, il_size in db.execute(
            "select token, type_name, name, params, rva, attrs, il_size from port_methods where assembly='Terraria'"):
        meth[tok] = dict(type=tname, name=name, params=params or '', rva=rva, attrs=attrs or 0)
    covered = {int(r[2]) for r in rows if r[5] != '-'}
    unc = [r for r in rows if r[5] == '-']
    unc_tok = {int(r[2]) for r in unc}
    # call graph
    callers = collections.defaultdict(list)       # dst -> [(src, il_offset, opcode)]
    by_name = collections.defaultdict(list)       # (type, name) -> tokens
    for tok, m in meth.items():
        by_name[(m['type'], m['name'])].append(tok)
    for src, off, op, dasm, dtok in db.execute("select src_token, il_offset, opcode, dst_assembly, dst_token from port_calls where assembly='Terraria'"):
        if dasm == 'Terraria' and dtok is not None:
            callers[dtok].append((src, off, op))
    types = {}
    for tok, full, base in db.execute("select token, full_name, base from port_types where assembly='Terraria'"):
        types[full] = base

    def virtual(tok):
        return bool(meth[tok]['attrs'] & 0x40)

    # dead: fixed point
    def rooted(tok):
        m = meth[tok]
        return virtual(tok) or m['name'] in ('.cctor', 'Main') or tok not in meth
    # seeds: the methods the hand-checked tables declare dead (OVERRIDES with 'dead', DEAD_TYPES, DEAD_METHODS); what
    # only they call is dead too (the fixed point below)
    def in_type(tname, t):
        return tname == t or tname.startswith(t + '+') or tname.startswith(t + '.')
    dead = set()
    for tok, m in meth.items():
        if any(code == 'dead' and in_type(m['type'], t) and (ms is None or m['name'] in ms) for t, ms, code, _ in OVERRIDES):
            dead.add(tok)
        elif any(m['type'] == e[0] for e in DEAD_TYPES) or any(m['type'] == t and m['name'] == mn for t, mn, _ in DEAD_METHODS):
            dead.add(tok)
    seeded = set(dead)
    changed = True
    while changed:
        changed = False
        for tok in meth:
            if tok in dead or rooted(tok):
                continue
            cs = callers.get(tok, [])
            if all(s in dead or s == tok for s, _, _ in cs):
                dead.add(tok)
                changed = True
    # unrunnable: the hand-checked OVERRIDES with 'unrunnable'; a method called only from those (and dead code) is
    # unrunnable for the same reason
    unrun = {tok for tok, m in meth.items() if any(code == 'unrunnable' and in_type(m['type'], t) and (ms is None or m['name'] in ms) for t, ms, code, _ in OVERRIDES)}
    unrun_seed = set(unrun)
    changed = True
    while changed:
        changed = False
        for tok in meth:
            if tok in unrun or tok in dead or rooted(tok):
                continue
            cs = callers.get(tok, [])
            if cs and all(s in unrun or s in dead or s == tok for s, _, _ in cs) and any(s in unrun for s, _, _ in cs):
                unrun.add(tok)
                changed = True

    # dumps: name -> (dir, info) for the covered callers; machine call targets per dump
    cov_dumps = collections.defaultdict(list)     # caller token -> [(dir, dump name)]
    key_tok = collections.defaultdict(list)
    for tok, m in meth.items():
        if tok in covered or tok in unc_tok:
            pass
        key_tok[(m['type'], m['name'])].append(tok)
    il_hash = {}

    def hash_of(tok):
        if tok not in il_hash:
            m = meth[tok]
            il_hash[tok] = il_fnv(pe.il(m['rva'])) if m['rva'] else None
        return il_hash[tok]
    # every method that ran in a reference process (any method, not only float ones): (type, name, IL hash)
    dirs = fc.dump_dirs()
    ran_keys = set()
    for d in dirs:
        for m in json.load(open(os.path.join(d, 'jit_code.json'), encoding='utf-8')):
            if m.get('il') and m.get('compiled_before') is not False:
                t, mn = m['name'].split('(')[0].rsplit('::', 1)
                ran_keys.add((t, mn, il_fnv(bytes.fromhex(m['il']))))
    for tok, m in meth.items():
        if m['rva'] and (m['type'], m['name'], hash_of(tok)) in ran_keys:
            covered.add(tok)
    need_callers = set()
    for r in unc:
        for s, _, _ in callers.get(int(r[2]), []):
            if s in covered:
                need_callers.add(s)
    want = collections.defaultdict(list)
    for s in need_callers:
        want[(meth[s]['type'], meth[s]['name'], hash_of(s))].append(s)
    calls_in = {}                                  # (dir, dump name) -> set of called names
    callees_of = {}
    for d in dirs:
        per = jit_calls.load_json(os.path.join(d, 'jit_method_callees.json'))
        jit_calls.load_cells(d)
        by_addr = jit_calls.names(d)
        for name, addr, code, info in jit_calls.methods(d):
            if not info or not info.get('il') or info.get('compiled_before') is False:
                continue
            t, mn = name.split('(')[0].rsplit('::', 1)
            h = il_fnv(bytes.fromhex(info['il']))
            toks = want.get((t, mn, h))
            if not toks:
                continue
            tgt = collections.Counter()
            unnamed = 0
            for x in jit_calls.machine_calls(addr, code, bool(info.get('exact_size'))):
                if x in by_addr:
                    tgt[by_addr[x]] += 1
                else:
                    unnamed += 1
            key = (d, name)
            calls_in[key] = (tgt, unnamed)
            callees_of[key] = [e.split(' ', 1)[1] for e in per.get(name, []) if not e.startswith('ld')]
            for s in toks:
                cov_dumps[s].append(key)

    def clr_matches(clr, tok):
        m = meth[tok]
        t, rest = clr.split('::', 1)
        if t != m['type'] or rest.split('(')[0] != m['name']:
            return False
        ps = rest.split('(', 1)[1].rstrip(')')
        ps = [short_clr(p) for p in ps.split(',')] if ps else []
        mine = [p.strip() for p in m['params'].split(',')] if m['params'] else []
        if len(ps) != len(mine):
            return False
        return all(a == b or a.split('[')[0] == b.split('[')[0].split('<')[0] for a, b in zip(ps, mine))

    # sequence points for call-site guards
    seq = collections.defaultdict(list)
    if os.path.exists(SEQ):
        for line in open(SEQ, encoding='utf-8'):
            t, mn, h, off, f, sl, sc, el, ec, ok = line.rstrip('\n').split('\t')
            seq[(t, mn, h)].append((int(off), f, int(sl)))
        for k in seq:
            seq[k].sort()
    src_cache = {}

    def guard(src_tok, off):
        m = meth[src_tok]
        sp = seq.get((m['type'], m['name'], hash_of(src_tok)))
        if not sp:
            return None
        cand = [x for x in sp if x[0] <= off]
        if not cand:
            return None
        _, f, line = cand[-1]
        path = os.path.join(ROOT, f)
        if path not in src_cache:
            try:
                src_cache[path] = open(path, encoding='utf-8-sig').read().split('\n')
            except OSError:
                src_cache[path] = []
        lines = src_cache[path]
        if not lines or line > len(lines):
            return None
        stmt = lines[line - 1].strip()
        ind = len(lines[line - 1]) - len(lines[line - 1].lstrip('\t '))
        cond = None
        for k in range(line - 2, max(0, line - 400), -1):
            l = lines[k]
            if not l.strip():
                continue
            i = len(l) - len(l.lstrip('\t '))
            if i < ind:
                s = l.strip()
                if s in ('{', '}'):
                    continue
                if re.match(r'(if|else|case|while|for|foreach|switch|default)\b', s) or re.match(r'(public|private|protected|internal|static)\b', s):
                    cond = s
                    if re.match(r'(public|private|protected|internal|static)\b', s):
                        cond = None
                    break
                ind = i
        rel = os.path.relpath(path, ROOT).replace('\\', '/')
        return '%s:%d `%s`%s' % (rel.replace('decompile/Terraria/', ''), line, stmt[:120], (' under `%s`' % cond[:140]) if cond else '')

    def ancestors(tok, depth=6):
        """The nearest covered methods above `tok` in the call graph, with the path and the guard of the call site."""
        seen = {tok}
        frontier = [(tok, [])]
        found = []
        for _ in range(depth):
            nxt = []
            for t, path in frontier:
                for s2, off, op in callers.get(t, []):
                    if s2 in seen:
                        continue
                    seen.add(s2)
                    if s2 in covered:
                        g = guard(s2, off)
                        found.append('%s -> %s%s' % (disp(s2), ' -> '.join(disp(x) for x in reversed([t] + path)), (' at ' + g) if g else ''))
                    else:
                        nxt.append((s2, [t] + path))
            if found:
                break
            frontier = nxt
        return '; '.join(found[:3])

    def disp(tok):
        m = meth[tok]
        return '%s.%s' % (m['type'].replace('Terraria.', ''), m['name'])

    out_rows = []
    counts = collections.Counter()
    il_by = collections.Counter()
    notrun = []
    for r in rows:
        if r[5] != '-':
            out_rows.append(r + ['', ''])
            continue
        tok = int(r[2])
        cs = callers.get(tok, [])
        tname = r[0]
        reason, ev = None, ''
        why = platform_of(tname)
        if why is not None:
            reason, ev = 'platform', why
        if reason is None and tok in dead:
            if not cs:
                reason, ev = 'dead', 'no call, callvirt, newobj or ldftn of it in Terraria.exe; not virtual, not a static constructor'
            else:
                reason, ev = 'dead', 'called only from dead code: ' + ', '.join(sorted({disp(s) for s, _, _ in cs}))[:300]
        if reason is None:
            cov_c = sorted({s for s, _, _ in cs if s in covered})
            unc_c = sorted({s for s, _, _ in cs if s not in covered})
            inl, called, unknown = [], [], []
            for s in cov_c:
                keys = cov_dumps.get(s, [])
                if not keys:
                    unknown.append(s)
                    continue
                any_call, any_known = False, False
                for k in keys:
                    tgt, unnamed = calls_in[k]
                    names = [c for c in callees_of[k] if clr_matches(c, tok)]
                    if not names:
                        continue
                    n = sum(tgt.get(c, 0) for c in set(names))
                    if n > 0:
                        any_call = True
                    elif unnamed == 0 or tname.startswith('Terraria.'):
                        any_known = True
                if any_call:
                    called.append(s)
                elif any_known:
                    inl.append(s)
                else:
                    unknown.append(s)
            if cov_c and inl and not called and not unknown:
                dumps = sorted({os.path.basename(k[0]) for s in inl for k in cov_dumps[s]})
                reason = 'inlined'
                ev = 'no call instruction to it in the machine code of its covered callers %s (dumps: %s)%s' % (
                    ', '.join(disp(s) for s in inl)[:300], ', '.join(dumps)[:200],
                    ('; also called by uncovered ' + ', '.join(disp(s) for s in unc_c)[:200]) if unc_c else '')
            else:
                reason = 'notrun'
                parts = []
                for s, off, op in cs:
                    if s in covered:
                        g = guard(s, off)
                        parts.append('%s %s%s' % ('delegate created (ldftn) in covered' if op.startswith('ld') else 'covered caller', disp(s), (' at ' + g) if g else ''))
                if not parts:
                    if virtual(tok):
                        ctors = by_name.get((tname, '.ctor'), [])
                        if ctors and not any(c in covered for c in ctors):
                            parts.append('virtual, reached by dispatch only; no instance of %s was created in any run (its constructor never ran)' % tname.replace('Terraria.', ''))
                            anc = ancestors(ctors[0])
                            if anc:
                                parts.append('created from ' + anc)
                        else:
                            parts.append('virtual, reached by dispatch only; no reference run dispatched to this override')
                    elif cs:
                        anc = ancestors(tok)
                        parts.append('callers not covered either: ' + ', '.join(sorted({disp(s) for s, _, _ in cs}))[:200] + ('; nearest covered: ' + anc if anc else ''))
                    else:
                        parts.append('static constructor' if r[1] == '.cctor' else 'no direct caller')
                ev = '; '.join(parts)[:900]
                notrun.append((int(r[3]), r))
        for entry in DEAD_TYPES:
            dt, why = entry[0], entry[1]
            allowed = entry[2] if len(entry) > 2 else []
            if tname == dt and reason == 'notrun':
                ctors = [c for c in by_name.get((tname, '.ctor'), [])]
                live = sorted({disp(s) for c in ctors for s, _, _ in callers.get(c, []) if s not in dead and meth[s]['type'] != tname and not meth[s]['type'].startswith(tname + '+')})
                ok_types = [e2[0] for e2 in DEAD_TYPES]
                live = [l for l in live if l not in allowed and not any(l.startswith(t2.replace('Terraria.', '') + '.') for t2 in ok_types)]
                if live:
                    raise SystemExit('DEAD_TYPES %s: constructor called from live code: %s' % (tname, ', '.join(live)))
                notrun = [x for x in notrun if x[1] is not r]
                cc = sorted({disp(s) for c in ctors for s, _, _ in callers.get(c, [])})
                reason, ev = 'dead', why + '; constructor callers: ' + (', '.join(cc) if cc else 'none') + '; ' + ev
        if reason == 'notrun' and cs and all(s2 in dead or s2 == tok or platform_of(meth[s2]['type']) is not None for s2, _, _ in cs) and any(platform_of(meth[s2]['type']) is not None for s2, _, _ in cs):
            notrun = [x for x in notrun if x[1] is not r]
            via = sorted({disp(s2) for s2, _, _ in cs if platform_of(meth[s2]['type']) is not None})
            reason, ev = 'platform', 'called only from platform code: ' + ', '.join(via)[:300] + ' (their reasons in this table); ' + ev
        if reason == 'notrun' and tok in unrun and tok not in unrun_seed:
            notrun = [x for x in notrun if x[1] is not r]
            via = sorted({disp(s2) for s2, _, _ in cs if s2 in unrun})
            reason, ev = 'unrunnable', 'called only from methods no reference run can execute: ' + ', '.join(via)[:300] + ' (their reasons in this table); ' + ev
        for dt, dm, why in DEAD_METHODS:
            if tname == dt and r[1] == dm and reason == 'notrun':
                notrun = [x for x in notrun if x[1] is not r]
                reason, ev = 'dead', why + '; ' + ev
        ov = override(tname, r[1])
        if ov and reason in ('notrun', 'dead'):
            if reason == 'notrun':
                notrun = [x for x in notrun if x[1] is not r]
            reason, ev = ov[0], ov[1] + '; ' + ev
        counts[reason] += 1
        il_by[reason] += int(r[3])
        out_rows.append(r + [reason, ev.replace('\t', ' ').replace('\n', ' ')])
    with open(TSV, 'w', encoding='utf-8', newline='\n') as f:
        for r in out_rows:
            f.write('\t'.join(r) + '\n')
    # summary
    names = {'dead': 'unreachable in this build (no caller in the IL call graph, or only dead callers)',
             'inlined': 'inlined into covered callers (no call instruction in their machine code: the facts are those callers\')',
             'platform': 'only on a platform / service the port does not run (Steam, WeGame, RGB devices)',
             'unrunnable': 'reachable, but no reference run can execute it (the reason in column 8)',
             'notrun': 'reachable, no reference run executes it yet (the condition: column 8)'}
    lines = ['<!-- reasons: generated by float_reasons.py -->', '## Why the uncovered methods have no reference (float_reasons.py)', '',
             'Column 7 of float_uncovered.tsv is the reason, column 8 the evidence (callers, dumps, guards of the call sites).', '',
             '| reason | methods | IL bytes |', '|---|---:|---:|']
    for k in ('inlined', 'dead', 'platform', 'unrunnable', 'notrun'):
        lines.append('| %s: %s | %d | %d |' % (k, names[k], counts[k], il_by[k]))
    lines.append('| total | %d | %d |' % (sum(counts.values()), sum(il_by.values())))
    lines.append('')
    lines.append('Reachable but not run, the 40 largest:')
    lines.append('')
    lines.append('| method | IL bytes | evidence |')
    lines.append('|---|---:|---|')
    for size, r in sorted(notrun, key=lambda x: -x[0])[:40]:
        ev = [x for x in out_rows if x[2] == r[2]][0][7]
        lines.append('| `%s.%s` | %d | %s |' % (r[0], r[1], size, ev[:300].replace('|', '\\|')))
    lines.append('<!-- /reasons -->')
    text = open(MD, encoding='utf-8').read()
    blk = '\n'.join(lines) + '\n'
    if '<!-- reasons: generated by float_reasons.py -->' in text:
        a = text.index('<!-- reasons: generated by float_reasons.py -->')
        b = text.index('<!-- /reasons -->') + len('<!-- /reasons -->\n')
        text = text[:a] + blk + text[b:]
    else:
        text = text + '\n' + blk
    open(MD, 'w', encoding='utf-8', newline='\n').write(text)
    print(' '.join('%s=%d (%d B)' % (k, counts[k], il_by[k]) for k in ('inlined', 'dead', 'platform', 'unrunnable', 'notrun')))


if __name__ == '__main__':
    main()
