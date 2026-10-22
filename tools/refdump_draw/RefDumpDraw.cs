// RefDumpDraw: what the ORIGINAL client draws, call by call, for fixed scenes.
//
//   RefDumpDraw.exe <outdir> [scene ...]
//
// The real Terraria.exe (a copy in work\refclient; the install is only read: Content) runs in-process on the XNA
// stand-in of tools\refdump_draw\xna (bin\xna: Microsoft.Xna.Framework.Graphics.dll / .Game.dll with XNA's identity,
// no graphics device, every graphics call recorded by xna\Graphics\Rec.cs). The real Microsoft.Xna.Framework.dll
// (maths, content pipeline, input, audio) and Xact.dll are used as they are. No window is shown: the game's form is a
// hidden WinForms window on a desktop of its own (CreateDesktop; never switched to), checked after every frame.
//
// What is pinned so that a run is repeatable (the port's tests/draw.rs pins the same, step for step):
//   - input: Mouse.GetState() = (640, 360), no button, wheel 0; Keyboard.GetState() = no key; GamePad.GetState() = not
//     connected (detours of the real XNA members); Mouse.SetPosition does nothing (the user's cursor is never moved)
//   - Environment.TickCount = TickCount (the seed of every `new UnifiedRandom()`); DateTime.Now / UtcNow = 2026-06-15 12:00
//     (no seasonal event); the game thread's stopwatch clock stands still (Stopwatch.GetTimestamp); the game thread's Guid.NewGuid()
//     = Guid.Empty (hash code 0, the port's configured Guid.GetHashCode: FastRandom.CreateWithRandomSeed seeds with it)
//   - the asset loader thread (ReLogic AsyncActionDispatcher) never starts: its queue is served on the game thread,
//     at most 64 actions after every Update (port: terraria_host::sys::threading::dispatcher_budget)
//   - content files: TitleContainer.OpenStream opens <install>\<name> read-only (and names the asset for Rec)
//   - the frame loop of terraria-host's driver protocol (game.rs; mirrored in xna\Game\Game.cs Host): window
//     1280 x 720, active, display 1920 x 1080, 1/60 s per tick
//   - config.json in the scratch save folder: RGB devices off, music / sound / ambient volume 0 ("VolumeMusic" ..: quick splash;
//     no sound reaches the speakers), Program.SkipAssemblyLoad (no force-load thread)
// Scenes: Scenario() below; each one names the frames it records. Output per scene: <scene>.trace.zlib (the recorded
// frames' lines, `frame N` before each) and <scene>.digest (every frame of the run: frame, lines, FNV-1a 64 of its
// lines), summary.txt.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;

public static partial class WG
{
    const int Width = 1280, Height = 720, DisplayW = 1920, DisplayH = 1080, DispatcherBudget = 64;
    public const int TickCount = 1000000;
    static string ClientDir, XnaDir, InstallDir, OutRoot;
    static IntPtr FormHandle;
    static Form TheForm;
    static object TheMain;
    static int Frame;
    static StreamWriter DigestOut;
    // the audio run: its calls (every frame, "frame N" before each) and the per-frame digest of them
    static bool AudioMode, ScenesMode;
    static StreamWriter AudioDigest;
    static TextWriter AudioOut;
    static readonly List<string> Summary = new List<string>();

    [DllImport("user32.dll", SetLastError = true)] static extern IntPtr CreateDesktop(string name, IntPtr device, IntPtr devmode, int flags, uint access, IntPtr sa);
    [DllImport("user32.dll", SetLastError = true)] static extern bool SetThreadDesktop(IntPtr desktop);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)] static extern IntPtr LoadLibrary(string path);
    [DllImport("msvcrt.dll", CallingConvention = CallingConvention.Cdecl)] static extern uint _controlfp(uint value, uint mask);

    /// <summary>The JIT-dumping twin (DrawJit.cs): runs after the scenario, before the process ends.</summary>
    public static Action AfterScenario;

    static int Main(string[] args)
    {
        string exeDir = Path.GetDirectoryName(Path.GetFullPath(Assembly.GetEntryAssembly().Location));
        Root = Path.GetFullPath(Path.Combine(exeDir, "..", "..", ".."));
        ClientDir = Path.Combine(Root, "work", "refclient");
        XnaDir = Path.Combine(exeDir, "xna");
        InstallDir = @"C:\Users\user\Documents\Games\Terraria";
        GameDir = InstallDir;
        OutRoot = Path.GetFullPath(args.Length > 0 ? args[0] : Path.Combine(Root, "refdata", "draw"));
        Directory.CreateDirectory(OutRoot);
        string stamp = DateTime.Now.ToString("yyyyMMdd-HHmmss");
        SaveDir = Path.Combine(Root, "work", "draw-scratch", stamp);
        Directory.CreateDirectory(Path.Combine(SaveDir, "Worlds"));
        // the watchdog: 30 minutes, REFDRAW_WATCHDOG=<seconds> (the gallery run is longer; a busy machine is slower)
        string wd = Environment.GetEnvironmentVariable("REFDRAW_WATCHDOG");
        Watchdog(string.IsNullOrEmpty(wd) ? 1800 : int.Parse(wd));
        // REFDRAW_STACK=<seconds>: print the game thread's stack every that many seconds (diagnosis of a hang)
        string every = Environment.GetEnvironmentVariable("REFDRAW_STACK");
        if (!string.IsNullOrEmpty(every))
        {
            Thread main = Thread.CurrentThread;
            int sec = int.Parse(every);
            var st = new Thread(() =>
            {
                while (true)
                {
                    Thread.Sleep(sec * 1000);
#pragma warning disable 0618
                    try { main.Suspend(); var trace = new StackTrace(main, false); main.Resume(); Console.Error.WriteLine("---- stack at frame " + Frame + ":" + Environment.NewLine + trace); }
                    catch (Exception e) { Console.Error.WriteLine("stack: " + e.Message); }
#pragma warning restore 0618
                }
            });
            st.IsBackground = true; st.Start();
        }
        // a desktop of our own for every window this thread creates (the game's form): never visible to the user
        IntPtr desk = CreateDesktop("VaelRefDraw", IntPtr.Zero, IntPtr.Zero, 0, 0x10000000u /* GENERIC_ALL */, IntPtr.Zero);
        if (desk == IntPtr.Zero || !SetThreadDesktop(desk)) { Console.Error.WriteLine("hidden desktop: " + Marshal.GetLastWin32Error()); return 2; }
        AppDomain.CurrentDomain.AssemblyResolve += Resolve;
        try { return Run(args.Skip(1).ToArray()); }
        catch (Exception e)
        {
            Console.Error.WriteLine("FAILED at frame " + Frame + ": " + e);
            for (Exception x = e; x != null; x = x.InnerException)
            {
                var tl = x as ReflectionTypeLoadException;
                if (tl != null) foreach (Exception le in tl.LoaderExceptions.Where(q => q != null).Take(10)) Console.Error.WriteLine("  loader: " + le.Message);
            }
            return 1;
        }
    }

    static readonly Dictionary<string, Assembly> Loaded = new Dictionary<string, Assembly>();
    static Assembly Resolve(object sender, ResolveEventArgs a)
    {
        string simple = new AssemblyName(a.Name).Name;
        lock (Loaded)
        {
            Assembly asm;
            if (Loaded.TryGetValue(simple, out asm)) return asm;
            string fake = Path.Combine(XnaDir, simple + ".dll");
            if (File.Exists(fake)) asm = Assembly.LoadFrom(fake);
            else if (G != null && Array.Find(G.GetManifestResourceNames(), e => e.EndsWith(simple + ".dll")) != null)
            {
                string res = Array.Find(G.GetManifestResourceNames(), e => e.EndsWith(simple + ".dll"));
                using (Stream s = G.GetManifestResourceStream(res)) { var b = new byte[s.Length]; int o = 0; while (o < b.Length) { int n = s.Read(b, o, b.Length - o); if (n <= 0) break; o += n; } asm = Assembly.Load(b); }
            }
            else if (File.Exists(Path.Combine(ClientDir, simple + ".dll"))) asm = Assembly.LoadFrom(Path.Combine(ClientDir, simple + ".dll"));
            if (asm != null) Loaded[simple] = asm;
            return asm;
        }
    }

    // ================================================================== detours (replacements)
    static int FakeTickCount() { return TickCount; }
    // Guid.GetHashCode: 0 (the port's configured value, game.json; FastRandom.CreateWithRandomSeed seeds with
    // Guid.NewGuid().GetHashCode(): the stars of the night sky, ...)
    // (Guid.GetHashCode is small enough for the JIT to inline it from mscorlib: the game thread's Guid.NewGuid() is
    // Guid.Empty instead, whose hash code is 0; other threads keep CoCreateGuid)
    [DllImport("ole32.dll")] static extern int CoCreateGuid(out Guid g);
    static Guid FakeNewGuid()
    {
        if (Thread.CurrentThread.ManagedThreadId == Microsoft.Xna.Framework.XnaStandIn.Rec.MainThread) return Guid.Empty;
        // the net run's client network thread too (its WorldGen.clearWorld spawns the stars: Star.SpawnStars with
        // FastRandom.CreateWithRandomSeed, NetClient.cs)
        if (NetClientThread != null && Thread.CurrentThread == NetClientThread) return Guid.Empty;
        Guid g; CoCreateGuid(out g); return g;
    }
    // the stopwatch clock of the game thread stands still (Main.DoDraw draws map sections for 5 ms of real time per
    // frame: with the clock still, until none is left); other threads keep the real one
    [DllImport("kernel32.dll")] static extern bool QueryPerformanceCounter(out long value);
    static long FakeTimestamp()
    {
        if (Thread.CurrentThread.ManagedThreadId == Microsoft.Xna.Framework.XnaStandIn.Rec.MainThread) return 1000000000L;
        long t; QueryPerformanceCounter(out t); return t;
    }
    // the clock: 2026-06-15 12:00:00, local and UTC alike (no seasonal event; the port pins the same instant)
    public static readonly long ClockTicks = new DateTime(2026, 6, 15, 12, 0, 0).Ticks;
    static DateTime FakeNow() { return new DateTime(ClockTicks, DateTimeKind.Local); }
    static DateTime FakeUtcNow() { return new DateTime(ClockTicks, DateTimeKind.Utc); }
    // the scenario's input (idle unless a scene sets it; port: tests/draw.rs `Input`)
    static int InMouseX = 640, InMouseY = 360;
    static bool InLeft;
    static Microsoft.Xna.Framework.Input.Keys[] InKeys;   // null: none (no XNA type in WG's static initialiser: it runs before the resolver)
    static Microsoft.Xna.Framework.Input.MouseState IdleMouse()
    {
        var r = Microsoft.Xna.Framework.Input.ButtonState.Released;
        var l = InLeft ? Microsoft.Xna.Framework.Input.ButtonState.Pressed : r;
        return new Microsoft.Xna.Framework.Input.MouseState(InMouseX, InMouseY, 0, l, r, r, r, r);
    }
    static void NoSetPosition(int x, int y) { }
    static Microsoft.Xna.Framework.Input.KeyboardState IdleKeyboard() { return InKeys == null ? new Microsoft.Xna.Framework.Input.KeyboardState() : new Microsoft.Xna.Framework.Input.KeyboardState(InKeys); }
    static Microsoft.Xna.Framework.Input.KeyboardState IdleKeyboardP(Microsoft.Xna.Framework.PlayerIndex p) { return IdleKeyboard(); }
    static Microsoft.Xna.Framework.Input.GamePadState NoPad(Microsoft.Xna.Framework.PlayerIndex p) { return new Microsoft.Xna.Framework.Input.GamePadState(); }
    static Microsoft.Xna.Framework.Input.GamePadState NoPadDz(Microsoft.Xna.Framework.PlayerIndex p, Microsoft.Xna.Framework.Input.GamePadDeadZone dz) { return new Microsoft.Xna.Framework.Input.GamePadState(); }
    static Microsoft.Xna.Framework.Input.GamePadCapabilities NoPadCaps(Microsoft.Xna.Framework.PlayerIndex p) { return new Microsoft.Xna.Framework.Input.GamePadCapabilities(); }
    static Stream OpenContent(string name)
    {
        string full = Path.GetFullPath(Path.Combine(InstallDir, name));
        Microsoft.Xna.Framework.XnaStandIn.Rec.CurrentAsset = Microsoft.Xna.Framework.XnaStandIn.Rec.AssetLabel(full);
        Microsoft.Xna.Framework.XnaStandIn.Rec.AssetObjects = 0;
        return new FileStream(full, FileMode.Open, FileAccess.Read, FileShare.Read);
    }
    // AsyncActionDispatcher: the loader "thread" runs on the game thread between frames
    static readonly List<object> Dispatchers = new List<object>();
    static readonly Dictionary<object, Queue<Action>> DispatcherQueues = new Dictionary<object, Queue<Action>>();
    static FieldInfo DispatcherRunning;
    static void DispatcherStart(object self)
    {
        if ((bool)DispatcherRunning.GetValue(self)) throw new InvalidOperationException("AsyncActionDispatcher is already started.");
        DispatcherRunning.SetValue(self, true);
        if (!Dispatchers.Contains(self)) Dispatchers.Add(self);
    }
    static void DispatcherStop(object self) { DispatcherRunning.SetValue(self, false); }
    static void DispatcherQueue(object self, Action action)
    {
        Queue<Action> q;
        if (!DispatcherQueues.TryGetValue(self, out q)) { q = new Queue<Action>(); DispatcherQueues[self] = q; }
        q.Enqueue(action);
    }
    static int DispatcherRemaining(object self) { Queue<Action> q; return DispatcherQueues.TryGetValue(self, out q) ? q.Count : 0; }
    static bool PumpDispatcherAction()
    {
        foreach (object d in Dispatchers)
        {
            if (!(bool)DispatcherRunning.GetValue(d)) continue;
            Queue<Action> q;
            if (DispatcherQueues.TryGetValue(d, out q) && q.Count > 0) { q.Dequeue()(); return true; }
        }
        return false;
    }

    // a message box would wait for a click that never comes (and must not be shown): the run stops with its text
    static void NoBox(string preparedMessage) { throw new Exception("the game wanted to show a message box: " + preparedMessage); }
    static DialogResult NoBoxCore(IWin32Window owner, string text, string caption, MessageBoxButtons buttons, MessageBoxIcon icon, MessageBoxDefaultButton def, MessageBoxOptions options, bool showHelp)
    { throw new Exception("the game wanted to show a message box: " + caption + ": " + text); }

    static void InstallDetours()
    {
        Type env = typeof(Environment);
        BindingFlags S = BindingFlags.Static | BindingFlags.NonPublic;
        Func<string, MethodInfo> mine = n => typeof(WG).GetMethod(n, S);
        Detour.Install(env.GetProperty("TickCount").GetGetMethod(), mine("FakeTickCount"));
        Detour.Install(typeof(DateTime).GetProperty("Now").GetGetMethod(), mine("FakeNow"));
        Detour.Install(typeof(DateTime).GetProperty("UtcNow").GetGetMethod(), mine("FakeUtcNow"));
        Detour.Install(typeof(Stopwatch).GetMethod("GetTimestamp"), mine("FakeTimestamp"));
        Detour.Install(typeof(Guid).GetMethod("NewGuid"), mine("FakeNewGuid"));
        Assembly fw = typeof(Microsoft.Xna.Framework.Input.Mouse).Assembly;
        Type mouse = fw.GetType("Microsoft.Xna.Framework.Input.Mouse"), kb = fw.GetType("Microsoft.Xna.Framework.Input.Keyboard"), pad = fw.GetType("Microsoft.Xna.Framework.Input.GamePad");
        Detour.Install(mouse.GetMethod("GetState", Type.EmptyTypes), mine("IdleMouse"));
        Detour.Install(mouse.GetMethod("SetPosition", new[] { typeof(int), typeof(int) }), mine("NoSetPosition"));
        Detour.Install(kb.GetMethod("GetState", Type.EmptyTypes), mine("IdleKeyboard"));
        Detour.Install(kb.GetMethod("GetState", new[] { typeof(Microsoft.Xna.Framework.PlayerIndex) }), mine("IdleKeyboardP"));
        Detour.Install(pad.GetMethod("GetState", new[] { typeof(Microsoft.Xna.Framework.PlayerIndex) }), mine("NoPad"));
        Detour.Install(pad.GetMethod("GetState", new[] { typeof(Microsoft.Xna.Framework.PlayerIndex), typeof(Microsoft.Xna.Framework.Input.GamePadDeadZone) }), mine("NoPadDz"));
        Detour.Install(pad.GetMethod("GetCapabilities", new[] { typeof(Microsoft.Xna.Framework.PlayerIndex) }), mine("NoPadCaps"));
        Detour.Install(fw.GetType("Microsoft.Xna.Framework.TitleContainer").GetMethod("OpenStream", new[] { typeof(string) }), mine("OpenContent"));
        Detour.Install(typeof(MessageBox).GetMethod("ShowCore", AllStatic, null, new[] { typeof(IWin32Window), typeof(string), typeof(string), typeof(MessageBoxButtons), typeof(MessageBoxIcon), typeof(MessageBoxDefaultButton), typeof(MessageBoxOptions), typeof(bool) }, null), mine("NoBoxCore"));
        Type disp = Resolve(null, new ResolveEventArgs("ReLogic")).GetType("ReLogic.Threading.AsyncActionDispatcher", true);
        DispatcherRunning = disp.GetField("_isRunning", AllInst);
        Detour.Install(disp.GetMethod("Start"), mine("DispatcherStart"));
        Detour.Install(disp.GetMethod("Stop"), mine("DispatcherStop"));
        Detour.Install(disp.GetMethod("Queue"), mine("DispatcherQueue"));
        Detour.Install(disp.GetProperty("ActionsRemaining").GetGetMethod(), mine("DispatcherRemaining"));
        // the audio engine: XNA's native audio layer replaced by AudioRec's model (no XAudio2 / XACT engine; nothing is
        // played); the calls are recorded in the audio mode
        AudioRec.Install(fw, Resolve(null, new ResolveEventArgs("Microsoft.Xna.Framework.Xact")));
    }

    // ================================================================== the game
    static Microsoft.Xna.Framework.Game Game { get { return (Microsoft.Xna.Framework.Game)TheMain; } }

    static void Launch()
    {
        // Program.LaunchGame / RunGame up to main.Run() (port: src/host/launch.rs launch_game), arguments: -savedirectory <scratch>
        Type program = T("Terraria.Program");
        Thread.CurrentThread.Name = "Main Thread";
        object args = CallStatic(T("Terraria.Utils"), "ParseArguements", new object[] { new string[] { "-savedirectory", SaveDir } });
        SSet(program, "LaunchParameters", args);
        SSet(program, "SavePath", SaveDir);
        // (after SavePath: compiling the game's error printer runs Main's static initialiser, which reads it)
        Detour.Install(T("Terraria.UI.FancyErrorPrinter").GetMethod("ShowTheBox", AllStatic), typeof(WG).GetMethod("NoBox", BindingFlags.Static | BindingFlags.NonPublic));
        ThreadPool.SetMinThreads(8, 8);
        CallStatic(program, "InitializeConsoleOutput");
        CallStatic(program, "SetupLogging");
        object lm = SF(T("Terraria.Localization.LanguageManager"), "Instance");
        object gc = SF(T("Terraria.Localization.GameCulture"), "DefaultCulture");
        lm.GetType().GetMethods().First(m => m.Name == "SetLanguage" && m.GetParameters().Length == 1 && m.GetParameters()[0].ParameterType == gc.GetType()).Invoke(lm, new[] { gc });
        TheMain = Activator.CreateInstance(TMain);
        CallStatic(T("Terraria.Lang"), "InitializeLegacyLocalization");
        Type social = T("Terraria.Social.SocialAPI");
        MethodInfo init = social.GetMethod("Initialize", AllStatic);
        init.Invoke(null, new object[] { Enum.ToObject(init.GetParameters()[0].ParameterType.GetGenericArguments()[0], 0) });
        CallStatic(T("Terraria.Initializers.LaunchInitializer"), "LoadParameters", TheMain);
        EventInfo preload = TMain.GetEvent("OnEnginePreload", AllStatic);
        preload.AddEventHandler(null, Delegate.CreateDelegate(preload.EventHandlerType, program.GetMethod("StartForceLoad", AllStatic)));
        // pinned: no force-load thread (Program.StartForceLoad then sets LoadedEverything at once)
        SSet(TMain, "SkipAssemblyLoad", true);
    }

    static void Initialize() { Game.StandInInitialize(); }

    // one tick of the driver protocol (port tests/client.rs frames_with, idle input)
    static void Tick()
    {
        NetPre();
        Frame++;
        var H = typeof(Microsoft.Xna.Framework.XnaStandIn.Host);
        Microsoft.Xna.Framework.XnaStandIn.Rec.FrameStart();
        // the audio engine's time (port: terraria_host::game::begin_frame -> audio::advance), then the frame's audio lines
        AudioRec.FrameStart();
        if (AudioOut != null) AudioOut.Write("frame " + Frame + "\n");
        AudioRec.Advance(166667);
        Microsoft.Xna.Framework.XnaStandIn.Host.BeginFrame(166667);
        Microsoft.Xna.Framework.XnaStandIn.Host.DispatchEvents();
        Microsoft.Xna.Framework.GameTime t;
        while ((t = Microsoft.Xna.Framework.XnaStandIn.Host.BeginUpdate()) != null)
        {
            Game.StandInUpdate(t);
            Microsoft.Xna.Framework.XnaStandIn.Host.EndUpdate();
            // run_pending_work_and_dispatchers: a slice of the network thread (net run), the loader's queue (at most the
            // budget), a second slice (NetClient.cs)
            NetSlice();
            for (int i = 0; i < DispatcherBudget; i++) if (!PumpDispatcherAction()) break;
            NetSlice();
        }
        var dt = Microsoft.Xna.Framework.XnaStandIn.Host.BeginDraw();
        if (dt != null) Game.StandInDraw(dt);
        Microsoft.Xna.Framework.XnaStandIn.Host.EndDraw(dt != null);
        DigestOut.Write(Frame + " " + Microsoft.Xna.Framework.XnaStandIn.Rec.FrameLines + " " + Microsoft.Xna.Framework.XnaStandIn.Rec.FrameHash.ToString("x16") + "\n");
        if (AudioDigest != null) AudioDigest.Write(Frame + " " + AudioRec.FrameLines + " " + AudioRec.FrameHash.ToString("x16") + "\n");
        // the guard: the game's window must never be visible or in front
        if (IsWindowVisible(FormHandle)) throw new Exception("GUARD: the game's window became visible at frame " + Frame);
        if (GetForegroundWindow() == FormHandle) throw new Exception("GUARD: the game's window is in front at frame " + Frame);
        FxTrace();
        NetPost();
    }

    // DRAW_FXTRACE=<first>,<last> (diagnosis; port: tests/draw.rs the same): after each of those frames, every active gore
    // (index, type, position, velocity, rotation, scale, alpha) and dust (index, type, position, velocity, rotation,
    // scale) with floats as bits, to fx.trace
    static int FxFirst = -2, FxLast = -1;
    static void FxTrace()
    {
        if (FxFirst == -2)
        {
            FxFirst = -1;
            string e = Environment.GetEnvironmentVariable("DRAW_FXTRACE");
            if (!string.IsNullOrEmpty(e)) { var p = e.Split(','); FxFirst = int.Parse(p[0]); FxLast = int.Parse(p[1]); }
        }
        if (Frame < FxFirst || Frame > FxLast) return;
        Func<float, string> fb = f => BitConverter.ToInt32(BitConverter.GetBytes(f), 0).ToString("x8");
        var sb = new StringBuilder("frame " + Frame + "\n");
        Array gores = (Array)SF(TMain, "gore");
        for (int i = 0; i < gores.Length; i++)
        {
            object g = gores.GetValue(i);
            if (g == null || !(bool)IF(g, "active")) continue;
            var p = (Microsoft.Xna.Framework.Vector2)IF(g, "position"); var v = (Microsoft.Xna.Framework.Vector2)IF(g, "velocity");
            sb.Append("gore " + i + " " + IF(g, "type") + " " + fb(p.X) + "," + fb(p.Y) + " " + fb(v.X) + "," + fb(v.Y) + " " + fb((float)IF(g, "rotation")) + " " + fb((float)IF(g, "scale")) + " " + IF(g, "alpha") + "\n");
        }
        Array dusts = (Array)SF(TMain, "dust");
        for (int i = 0; i < dusts.Length; i++)
        {
            object d = dusts.GetValue(i);
            if (d == null || !(bool)IF(d, "active")) continue;
            var p = (Microsoft.Xna.Framework.Vector2)IF(d, "position"); var v = (Microsoft.Xna.Framework.Vector2)IF(d, "velocity");
            var fr = (Microsoft.Xna.Framework.Rectangle)IF(d, "frame");
            sb.Append("dust " + i + " " + IF(d, "type") + " " + fb(p.X) + "," + fb(p.Y) + " " + fb(v.X) + "," + fb(v.Y) + " " + fb((float)IF(d, "rotation")) + " " + fb((float)IF(d, "scale"))
                + " g" + ((bool)IF(d, "noGravity") ? 1 : 0) + " f" + fb((float)IF(d, "fadeIn")) + " a" + IF(d, "alpha") + " fr" + fr.X + "," + fr.Y + " c" + ((Microsoft.Xna.Framework.Color)IF(d, "color")).PackedValue.ToString("x8") + "\n");
        }
        File.AppendAllText(Path.Combine(OutRoot, "fx.trace"), sb.ToString(), new UTF8Encoding(false));
    }
    // DRAW_EXTRA=<first>,<count> (diagnosis): the lines of those frames of Frames() go to extra.trace (port: the same variable)
    static void Frames(int n)
    {
        string e = Environment.GetEnvironmentVariable("DRAW_EXTRA");
        int first = -1, count = 0;
        if (!string.IsNullOrEmpty(e)) { var p = e.Split(','); first = int.Parse(p[0]); count = int.Parse(p[1]); }
        for (int i = 0; i < n; i++)
        {
            bool rec = Frame + 1 >= first && Frame + 1 < first + count;
            if (!rec) { Tick(); continue; }
            var w = new StreamWriter(Path.Combine(OutRoot, "extra.trace"), true, new UTF8Encoding(false));
            w.Write("frame " + (Frame + 1) + "\n");
            Microsoft.Xna.Framework.XnaStandIn.Rec.Out = w;
            Microsoft.Xna.Framework.XnaStandIn.Rec.Full = true;
            Tick();
            Microsoft.Xna.Framework.XnaStandIn.Rec.Full = false;
            w.Close();
        }
    }

    // ---------------------------------------------------------------- content labels (port: tests/draw.rs label_assets)
    static int LabelledAt = -1;
    static void LabelAssets()
    {
        object repo = SF(TMain, "Assets");
        int loaded = (int)repo.GetType().GetProperty("LoadedAssets").GetValue(repo, null);
        if (loaded == LabelledAt) return;
        LabelledAt = loaded;
        var dict = (IDictionary)IF(repo, "_assets");
        foreach (DictionaryEntry e in dict)
        {
            object asset = e.Value;
            object v = asset.GetType().GetProperty("Value").GetValue(asset, null);
            if (v == null) continue;
            string name = ((string)e.Key).Replace('\\', '/');
            var tex = v as Microsoft.Xna.Framework.Graphics.Texture;
            if (tex != null) { if (tex.RecLabel == null) tex.RecLabel = name; continue; }
            var fx = v as Microsoft.Xna.Framework.Graphics.Effect;
            if (fx != null) { if (fx.RecLabel == null) fx.RecLabel = name; continue; }
            if (v.GetType().FullName == "ReLogic.Graphics.DynamicSpriteFont")
            {
                var chars = (Array)IF(v, "_spriteCharacters");
                var seen = new List<object>();
                foreach (object c in chars)
                {
                    if (c == null) continue;
                    var t = (Microsoft.Xna.Framework.Graphics.Texture)IF(c, "Texture");
                    if (t == null || seen.Contains(t)) continue;
                    seen.Add(t);
                    if (t.RecLabel == null) t.RecLabel = name + "#" + (seen.Count - 1);
                }
            }
        }
    }
    static void Record(string scene, int n)
    {
        var R = typeof(Microsoft.Xna.Framework.XnaStandIn.Rec);
        var ms = new MemoryStream();
        var w = new StreamWriter(ms, new UTF8Encoding(false));
        Microsoft.Xna.Framework.XnaStandIn.Rec.Out = w;
        Microsoft.Xna.Framework.XnaStandIn.Rec.Full = true;
        long before = Microsoft.Xna.Framework.XnaStandIn.Rec.Lines;
        for (int i = 0; i < n; i++)
        {
            w.Write("frame " + (Frame + 1) + "\n");
            Tick();
        }
        Microsoft.Xna.Framework.XnaStandIn.Rec.Full = false;
        w.Flush();
        byte[] body = ms.ToArray();
        WriteZlib(Path.Combine(OutRoot, scene + ".trace.zlib"), new byte[0], body);
        string line = scene + ": frames " + (Frame - n + 1) + ".." + Frame + ", " + (Microsoft.Xna.Framework.XnaStandIn.Rec.Lines - before) + " calls, " + body.Length + " bytes";
        if (ScenesMode || GalleryMode || NetMode || InteractMode || CoverMode || ProbeMode) StateLine(scene);
        Summary.Add(line);
        Console.WriteLine(line);
    }

    static int Run(string[] only)
    {
        var total = Stopwatch.StartNew();
        // XNA's native helper (LZX decompression of the content files), from the game's own XNA redistributable (not
        // installed on this machine: no registry key); loaded by full path first so XNA's load by name finds it
        if (LoadLibrary(Path.Combine(ClientDir, "XnaNative.dll")) == IntPtr.Zero) throw new Exception("XnaNative.dll: " + Marshal.GetLastWin32Error());
        G = Assembly.LoadFrom(Path.Combine(ClientDir, "Terraria.exe"));
        TMain = T("Terraria.Main"); TWorldGen = T("Terraria.WorldGen"); TWorldFile = T("Terraria.IO.WorldFile");
        InstallDetours();
        if (Array.IndexOf(only, "net") >= 0) NetInstall();
        Environment.CurrentDirectory = InstallDir;
        // the audio run (`RefDumpDraw.exe <outdir> audio`): the same scenario with sound, music and ambience on (the
        // volumes below), menu clicks after the title scene and the audio scene at the end (Scenario)
        AudioMode = Array.IndexOf(only, "audio") >= 0;
        // the scenes run (`RefDumpDraw.exe <outdir> scenes`): ScenesMenus / ScenesWorld instead of the draw scenes
        ScenesMode = Array.IndexOf(only, "scenes") >= 0;
        // the gallery run (`RefDumpDraw.exe <outdir> gallery`, Gallery.cs): GalleryWorld after EnterWorld
        GalleryMode = Array.IndexOf(only, "gallery") >= 0;
        // the interaction run (`RefDumpDraw.exe <outdir> interact`, Interact.cs): InteractWorld after EnterWorld
        InteractMode = Array.IndexOf(only, "interact") >= 0;
        // the coverage run (`RefDumpDraw.exe <outdir> cover`, Cover.cs): CoverWorld after EnterWorld
        CoverMode = Array.IndexOf(only, "cover") >= 0;
        // the probe run (`RefDumpDraw.exe <outdir> probe`, Probe.cs): ProbeWorld after EnterWorld
        ProbeMode = Array.IndexOf(only, "probe") >= 0;
        // the net run (`RefDumpDraw.exe <outdir> net <pipe>`, NetClient.cs): the client joins RefDumpNet.exe's server
        NetMode = Array.IndexOf(only, "net") >= 0;
        if (NetMode) NetPipeName = only[Array.IndexOf(only, "net") + 1];
        string volumes = AudioMode ? "  \"VolumeMusic\": 0.75,\n  \"VolumeSound\": 1.0,\n  \"VolumeAmbient\": 0.5,\n" : "  \"VolumeMusic\": 0.0,\n  \"VolumeSound\": 0.0,\n  \"VolumeAmbient\": 0.0,\n";
        // the scratch save folder: config.json (scenario pins) and the world
        File.WriteAllText(Path.Combine(SaveDir, "config.json"),
            "{\n  \"LastLaunchedVersion\": 326,\n  \"Fullscreen\": false,\n  \"WindowMaximized\": false,\n  \"WindowBorderless\": false,\n  \"DisplayWidth\": 1280,\n  \"DisplayHeight\": 720,\n  \"ResetDefaultUIScale\": false,\n  \"UIScale\": 1.0,\n  \"Zoom\": 1.0,\n" + volumes + "  \"Language\": \"en-US\",\n  \"UseRazerRGB\": false,\n  \"UseCorsairRGB\": false,\n  \"UseLogitechRGB\": false,\n  \"UseSteelSeriesRGB\": false\n}\n");
        File.Copy(Path.Combine(Root, "work", "refsave", "Worlds", "ref_small.wld"), Path.Combine(SaveDir, "Worlds", "ref_small.wld"));
        // the probe run: input profiles saved by the game itself (PlayerInput.Load then loads them: PlayerInputProfile.Load)
        if (ProbeMode) File.Copy(Path.Combine(Root, "tools", "refdump_draw", "probe_input_profiles.json"), Path.Combine(SaveDir, "input profiles.json"));
        // the probe run's resource pack (a manifest only, not enabled): the resource pack menus list it (ProbeS.cs respacks)
        if (ProbeMode) { Directory.CreateDirectory(Path.Combine(SaveDir, "ResourcePacks", "ProbePack")); File.Copy(Path.Combine(Root, "tools", "refdump_draw", "probe_pack", "pack.json"), Path.Combine(SaveDir, "ResourcePacks", "ProbePack", "pack.json")); }
        TheForm = new Form { ShowInTaskbar = false, StartPosition = FormStartPosition.Manual, Location = new System.Drawing.Point(0, 0), ClientSize = new System.Drawing.Size(Width, Height) };
        FormHandle = TheForm.Handle;
        Microsoft.Xna.Framework.XnaStandIn.Rec.MainThread = Thread.CurrentThread.ManagedThreadId;
        Microsoft.Xna.Framework.XnaStandIn.Host.Init(Width, Height, FormHandle, DisplayW, DisplayH);
        DigestOut = new StreamWriter(Path.Combine(OutRoot, "run.digest"), false, new UTF8Encoding(false));
        Microsoft.Xna.Framework.XnaStandIn.Rec.Active = true;
        Microsoft.Xna.Framework.XnaStandIn.Rec.Missing = LabelAssets;
        MemoryStream audioBody = null;
        if (AudioMode)
        {
            AudioDigest = new StreamWriter(Path.Combine(OutRoot, "audio.digest"), false, new UTF8Encoding(false));
            audioBody = new MemoryStream();
            AudioOut = new StreamWriter(audioBody, new UTF8Encoding(false));
            AudioOut.Write("frame 0\n");
            AudioRec.Out = AudioOut;
            AudioRec.Active = true;
            AudioRec.Full = true;
        }
        // FastParallel.For on the calling thread (ReLogic's own switch; the port runs every range on the calling thread:
        // terraria-gen-rt fast_parallel_for)
        if (Environment.GetEnvironmentVariable("DRAW_PARALLEL") != "1")
            Resolve(null, new ResolveEventArgs("ReLogic")).GetType("ReLogic.Threading.FastParallel", true).GetProperty("ForceTasksOnCallingThread").SetValue(null, true, null);
        Launch();
        Initialize();
        // the start-up force-load (Program.ForceLoadThread: every method compiled, every static initialiser run) on this
        // thread, before the first frame, instead of on a thread of its own next to the splash screen
        // (the static initialisers only: ForceJITOnAssembly would compile every method, and the JIT twin's dump is of the
        // methods that ran; the port's force_load_assembly sets the progress counters and runs the initialisers the same)
        {
            Type program = T("Terraria.Program");
            int count = G.GetTypes().Length;
            SSet(program, "ThingsToLoad", count);
            SSet(program, "ThingsLoaded", count);
            CallStatic(program, "ForceStaticInitializers", G);
            SSet(program, "LoadedEverything", true);
        }
        if (Environment.GetEnvironmentVariable("DRAW_FPU") == "1")
        {
            Console.WriteLine("fpu main: " + _controlfp(0, 0).ToString("x8"));
            var done = new ManualResetEvent(false);
            ThreadPool.QueueUserWorkItem(_ => { Console.WriteLine("fpu pool: " + _controlfp(0, 0).ToString("x8")); done.Set(); });
            done.WaitOne();
        }
        Scenario(only);
        DigestOut.Close();
        if (ScenesMode || GalleryMode || NetMode || InteractMode || CoverMode || ProbeMode) File.WriteAllText(Path.Combine(OutRoot, "scenes.state"), StateLines.ToString(), new UTF8Encoding(false));
        if (AudioMode)
        {
            AudioOut.Flush();
            WriteZlib(Path.Combine(OutRoot, "run.audio.zlib"), new byte[0], audioBody.ToArray());
            AudioDigest.Close();
            Summary.Add("audio: " + AudioRec.Lines + " calls");
            Console.WriteLine("audio: " + AudioRec.Lines + " calls");
        }
        Summary.Add(Frame + " frames");
        if (AfterScenario != null) AfterScenario();
        File.WriteAllLines(Path.Combine(OutRoot, "summary.txt"), Summary);
        Console.WriteLine("done: " + Frame + " frames, " + total.Elapsed.TotalSeconds.ToString("0.0", Inv) + " s");
        return 0;
    }

    // ================================================================== the scenario (port: tests/draw.rs, same steps)
    static void Note(string s) { Console.WriteLine("[" + Frame + "] " + s); Summary.Add("[" + Frame + "] " + s); }

    static object V2(float x, float y) { return Activator.CreateInstance(typeof(Microsoft.Xna.Framework.Vector2), new object[] { x, y }); }
    static object LocalPlayer { get { return ((Array)SF(TMain, "player")).GetValue((int)SF(TMain, "myPlayer")); } }

    /// <summary>The player stands at tile (x, y) (its feet), not moving; the camera follows it.</summary>
    static void PlacePlayer(int tx, int ty)
    {
        object p = LocalPlayer;
        int h = (int)IF(p, "height"), w = (int)IF(p, "width");
        ISet(p, "position", V2(tx * 16 + 8 - w / 2, ty * 16 - h));
        ISet(p, "velocity", V2(0f, 0f));
        ISet(p, "fallStart", (int)(ty - h / 16));
    }

    static void PlaySound(int type)
    {
        T("Terraria.Audio.SoundEngine").GetMethods(AllStatic).First(m => m.Name == "PlaySound" && m.GetParameters().Length == 6 && m.GetParameters()[0].ParameterType == typeof(int))
            .Invoke(null, new object[] { type, -1, -1, 1, 1f, 0f });
    }

    static void EnterWorld()
    {
        // the character creation's end (UICharacterCreation: PlayerFileData.CreateAndSave), the player list's load of
        // the file and its selection (Main.SelectPlayer), the world list's PlayGame (UIWorldListItem.cs:360)
        Type pfdT = T("Terraria.IO.PlayerFileData");
        object player = SF(TMain, "PendingPlayer");
        ISet(player, "name", "Ref");
        object created = CallStatic(pfdT, "CreateAndSave", player);
        object pfd = CallStatic(T("Terraria.Player"), "GetFileData", (string)IF(created, "Path"), false);
        CallStatic(TMain, "SelectPlayer", pfd);
        CallStatic(TMain, "LoadWorlds");
        if (ScenesMode) ScenesWorldMenus();
        object wfd = CallStatic(TWorldFile, "GetAllMetadata", Path.Combine((string)SF(TMain, "WorldPath"), "ref_small.wld"), false);
        wfd.GetType().GetMethod("SetAsActive").Invoke(wfd, null);
        PlaySound(10);
        CallStatic(TMain, "clrInput");
        CallStatic(TMain, "GetInputText", "", false);
        SSet(TMain, "menuMode", 10);
        // WorldGen.playWorldCallBack (WorldGen.cs:6732), on this thread; RunOnMainThread(..).Wait() is the block it runs
        Array players = (Array)SF(TMain, "player");
        for (int i = 0; i < 255; i++) if (i != (int)SF(TMain, "myPlayer")) ISet(players.GetValue(i), "active", false);
        CallStatic(TMain, "ToggleGameplayUpdates", false);
        CallStatic(TWorldFile, "LoadWorld");
        if ((bool)SF(TWorldGen, "loadFailed")) throw new Exception("the world did not load");
        if ((bool)SF(TMain, "mapEnabled")) { object map = SF(TMain, "Map"); map.GetType().GetMethod("Load").Invoke(map, null); }
        if ((int)SF(TMain, "netMode") != 2) { object sm = SF(TMain, "sectionManager"); sm.GetType().GetMethod("SetAllSectionsLoaded").Invoke(sm, null); }
        if ((int)SF(TMain, "netMode") == 0 && ((IList)SF(TMain, "anglerWhoFinishedToday")).Contains(IF(LocalPlayer, "name"))) SSet(TMain, "anglerQuestFinished", true);
        SSet(TMain, "gameMenu", false);
        object ctx = Enum.Parse(T("Terraria.PlayerSpawnContext"), "SpawningIntoWorld");
        LocalPlayer.GetType().GetMethod("Spawn").Invoke(LocalPlayer, new object[] { ctx });
        CallStatic(TWorldFile, "SetOngoingToTemps");
        PlaySound(11);
        SSet(TMain, "resetClouds", true);
    }

    static void SetTime(bool day, double time) { SSet(TMain, "dayTime", day); SSet(TMain, "time", time); }

    static void Scenario(string[] only)
    {
        // 1. start-up: splash (quick: music volume 0, assembly load skipped) into the title menu; its first frames recorded
        Record("boot", 3);
        Frames(297);
        Note("after start-up: showSplash " + SF(TMain, "showSplash") + " gameMenu " + SF(TMain, "gameMenu") + " menuMode " + SF(TMain, "menuMode"));
        // 2. title menu
        Record("title", 3);
        if (Environment.GetEnvironmentVariable("REFDRAW_FINDMENU") == "1") { FindMenu(); return; }
        if (AudioMode) MenuClicks();
        if (ScenesMode) ScenesMenus();
        // 3. character creation: the player list opens (Main.LoadPlayers), then UICharacterSelect.NewCharacterClick
        CallStatic(TMain, "LoadPlayers");
        PlaySound(10);
        SSet(TMain, "PendingPlayer", Activator.CreateInstance(T("Terraria.Player")));
        SSet(TMain, "menuMode", 888);
        object ui = SF(TMain, "MenuUI");
        object cc = Activator.CreateInstance(T("Terraria.GameContent.UI.States.UICharacterCreation"), new object[] { SF(TMain, "PendingPlayer") });
        ui.GetType().GetMethod("SetState").Invoke(ui, new object[] { cc });
        Frames(30);
        Record("charcreate", 3);
        if (NetMode) { NetScenario(); return; }
        // 4. into the world (the new character, ref_small.wld), noon
        EnterWorld();
        Note("in the world: gameMenu " + SF(TMain, "gameMenu") + " spawn " + SF(TMain, "spawnTileX") + "," + SF(TMain, "spawnTileY"));
        if (Environment.GetEnvironmentVariable("REFDRAW_FIND") == "1") { FindPlaces(); return; }
        if (Environment.GetEnvironmentVariable("REFDRAW_FINDBIOMES") == "1") { FindBiomes(); return; }
        if (ScenesMode) { ScenesWorld(); return; }
        if (GalleryMode) { GalleryWorld(); return; }
        if (InteractMode) { InteractWorld(); Record("interact_end", 2); return; }
        if (CoverMode) { CoverWorld(); Record("cover_end", 2); return; }
        if (ProbeMode) { ProbeWorld(); Record("probe_end", 2); return; }
        SetTime(true, 27000.0);
        Frames(60);
        Record("world_day", 3);
        // 5. the same place at midnight (moon, stars)
        SetTime(false, 16200.0);
        Frames(60);
        Record("world_night", 3);
        // 6. on the sea floor of the ocean at the left edge of the world (water, the surface above), noon
        SetTime(true, 27000.0);
        PlacePlayer(150, 249);
        Frames(60);
        Record("ocean", 3);
        // 7. a cave below the rock layer (dark: the lighting engine)
        PlacePlayer(1999, 547);
        Frames(60);
        Record("cave", 3);
        // 8. the underworld (lava, its light)
        PlacePlayer(480, 1009);
        Frames(60);
        Record("underworld", 3);
        // 9. back at the spawn at night: the Eye of Cthulhu summoned (NPC.SpawnOnPlayer) and projectiles of the player
        SetTime(false, 16200.0);
        PlacePlayer((int)SF(TMain, "spawnTileX"), (int)SF(TMain, "spawnTileY"));
        Frames(30);
        CallStatic(T("Terraria.NPC"), "SpawnOnPlayer", (int)SF(TMain, "myPlayer"), 4, 0f, 0f, 0f, 0f);
        {
            MethodInfo newProj = T("Terraria.Projectile").GetMethods(AllStatic).First(m => m.Name == "NewProjectile" && m.GetParameters()[1].ParameterType.Name == "Vector2");
            object pl = LocalPlayer;
            var c = (Microsoft.Xna.Framework.Vector2)IF(pl, "position");
            int k = 0;
            foreach (int type in BossProjectiles)
            {
                newProj.Invoke(null, new object[] { Activator.CreateInstance(T("Terraria.DataStructures.EntitySource_DebugCommand")), V2(c.X + 10f, c.Y - 20f - 8f * k), V2(4f + k, -1.5f * k), type, 20, 2f, (int)SF(TMain, "myPlayer"), 0f, 0f, 0f, null });
                k++;
            }
        }
        Frames(20);
        Record("boss", 3);
        // 10. the inventory open (Player.ToggleInv)
        LocalPlayer.GetType().GetMethod("ToggleInv").Invoke(LocalPlayer, null);
        Frames(10);
        Record("inventory", 3);
        // 11. the full-screen map (Player.TryOpeningFullscreenMap; the minimap is in every scene above)
        LocalPlayer.GetType().GetMethod("TryOpeningFullscreenMap").Invoke(LocalPlayer, null);
        Frames(10);
        Record("map", 3);
        if (AudioMode) AudioScene();
    }

    // ================================================================== the audio run's own steps (port: tests/draw.rs, same steps)
    /// <summary>The title menu's items at x 640 (REFDRAW_FINDMENU=1 prints which item the cursor is on, per y).</summary>
    const int MenuSingleY = 240, MenuMultiY = 292, MenuSettingsY = 450;

    static void FindMenu()
    {
        for (int y = 200; y < 600; y += 8)
        {
            SSet(TMain, "menuMode", 11);
            InMouseY = 360;
            Frames(3);
            InMouseY = y;
            Frames(3);
            Click();
            Frames(2);
            Console.WriteLine("menu 11 click y " + y + " -> menuMode " + SF(TMain, "menuMode"));
        }
        InMouseY = 360;
    }

    /// <summary>A key pressed for one frame, then released.</summary>
    static void Press(Microsoft.Xna.Framework.Input.Keys k)
    {
        InKeys = new[] { k };
        Frames(1);
        InKeys = null;
        Frames(1);
    }
    /// <summary>A left click: the button down for one frame, then up.</summary>
    static void Click()
    {
        InLeft = true;
        Frames(1);
        InLeft = false;
        Frames(1);
    }

    /// <summary>Title menu: the cursor over "Single Player", "Multiplayer", "Settings" (a tick sound each), a click on
    /// "Settings" (the settings menu opens), then back to the title menu as its "Back" does (menuMode 0, sound 11).</summary>
    static void MenuClicks()
    {
        InMouseX = 640;
        foreach (int y in new[] { MenuSingleY, MenuMultiY, MenuSettingsY })
        {
            InMouseY = y;
            Frames(10);
        }
        Click();
        Frames(10);
        Note("settings: menuMode " + SF(TMain, "menuMode"));
        PlaySound(11);
        SSet(TMain, "menuMode", 0);
        Frames(10);
        Note("back: menuMode " + SF(TMain, "menuMode"));
        InMouseX = 640; InMouseY = 360;
        Frames(5);
    }

    /// <summary>The cursor on world position (x, y) (zoom 1, UI scale 1: screen = world - Main.screenPosition).</summary>
    static void Aim(float x, float y)
    {
        var sp = (Microsoft.Xna.Framework.Vector2)SF(TMain, "screenPosition");
        InMouseX = (int)(x - sp.X);
        InMouseY = (int)(y - sp.Y);
    }
    static int FeetTileX { get { object p = LocalPlayer; var pos = (Microsoft.Xna.Framework.Vector2)IF(p, "position"); return (int)((pos.X + (int)IF(p, "width") / 2) / 16f); } }
    static int FeetTileY { get { object p = LocalPlayer; var pos = (Microsoft.Xna.Framework.Vector2)IF(p, "position"); return (int)((pos.Y + (int)IF(p, "height")) / 16f); } }

    static void GiveItem(int slot, int type)
    {
        object item = ((Array)IF(LocalPlayer, "inventory")).GetValue(slot);
        item.GetType().GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int)).Invoke(item, new object[] { type, null });
    }

    /// <summary>The audio scene: walking on grass, mining, hitting an enemy, item use, music across day / night and biomes.</summary>
    static void AudioScene()
    {
        // the map and the inventory closed, the boss and the projectiles gone, noon at the spawn
        SSet(TMain, "mapFullscreen", false);
        SSet(TMain, "playerInventory", false);
        foreach (object n in (Array)SF(TMain, "npc")) ISet(n, "active", false);
        foreach (object pr in (Array)SF(TMain, "projectile")) ISet(pr, "active", false);
        SetTime(true, 27000.0);
        PlacePlayer((int)SF(TMain, "spawnTileX"), (int)SF(TMain, "spawnTileY"));
        // the hotbar: an iron broadsword (1), an iron pickaxe (2), a lesser healing potion (4) (Item.SetDefaults)
        GiveItem(0, 4);
        GiveItem(1, 1);
        GiveItem(3, 28);
        Frames(120);
        Note("audio: day at the spawn, curMusic " + SF(TMain, "curMusic"));
        // walking right on the grass (D held)
        InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.D };
        Frames(90);
        InKeys = null;
        Frames(30);
        Note("audio: walked to " + FeetTileX + "," + FeetTileY);
        // mining: the pickaxe (hotbar 2), the cursor on the ground one tile right of the feet, the button held
        Press(Microsoft.Xna.Framework.Input.Keys.D2);
        int gx = FeetTileX + 1, gy = FeetTileY;
        Aim(gx * 16 + 8, gy * 16 + 8);
        InLeft = true;
        for (int i = 0; i < 90; i++)
        {
            Frames(1);
            if (i < 4 && Environment.GetEnvironmentVariable("REFDRAW_DEBUG") == "1")
            {
                object tr = SF(T("Terraria.GameInput.PlayerInput"), "Triggers");
                object cur = IF(tr, "Current");
                Console.WriteLine("dbg mouse " + InMouseX + "," + InMouseY + " blockMouse " + SF(TMain, "blockMouse") + " mouseInterface " + IF(LocalPlayer, "mouseInterface") + " trig " + cur.GetType().GetProperty("MouseLeft").GetValue(cur, null) + " mouseLeft " + SF(TMain, "mouseLeft") + " use " + IF(LocalPlayer, "controlUseItem") + " anim " + IF(LocalPlayer, "itemAnimation") + " active " + Game.IsActive + " gamePaused " + SF(TMain, "gamePaused"));
            }
        }
        InLeft = false;
        Frames(10);
        Note("audio: mined at " + gx + "," + gy + ", selected " + IF(LocalPlayer, "selectedItem"));
        // an enemy: a blue slime three tiles right, the sword (hotbar 1) swung at it
        {
            MethodInfo newNpc = T("Terraria.NPC").GetMethods(AllStatic).First(m => m.Name == "NewNPC" && m.GetParameters().Length == 10);
            var pos = (Microsoft.Xna.Framework.Vector2)IF(LocalPlayer, "position");
            int slot = (int)newNpc.Invoke(null, new object[] { Activator.CreateInstance(T("Terraria.DataStructures.EntitySource_DebugCommand")), (int)pos.X + 36, (int)pos.Y + 20, 1, 0, 0f, 0f, 0f, 0f, 255 });
            Press(Microsoft.Xna.Framework.Input.Keys.D1);
            object slime = ((Array)SF(TMain, "npc")).GetValue(slot);
            for (int i = 0; i < 90; i++)
            {
                var sp = (Microsoft.Xna.Framework.Vector2)IF(slime, "position");
                Aim(sp.X + 8, sp.Y + 8);
                InLeft = true;
                Frames(1);
            }
            InLeft = false;
            Frames(30);
            Note("audio: slime " + slot + " active " + IF(slime, "active") + " life " + IF(slime, "life"));
        }
        // item use: the lesser healing potion (hotbar 4)
        {
            object item = ((Array)IF(LocalPlayer, "inventory")).GetValue(3);
            Press(Microsoft.Xna.Framework.Input.Keys.D4);
            InMouseX = 700; InMouseY = 360;
            Click();
            Frames(40);
            Note("audio: potion stack " + IF(item, "stack"));
        }
        InMouseX = 640; InMouseY = 360;
        // music: night, space, the ocean, the underworld
        SetTime(false, 16200.0);
        Frames(120);
        Note("audio: night, curMusic " + SF(TMain, "curMusic"));
        PlacePlayer((int)SF(TMain, "spawnTileX"), 60);
        Frames(120);
        Note("audio: space, curMusic " + SF(TMain, "curMusic"));
        SetTime(true, 27000.0);
        PlacePlayer(150, 249);
        Frames(120);
        Note("audio: ocean, curMusic " + SF(TMain, "curMusic"));
        PlacePlayer(480, 1009);
        Frames(120);
        Note("audio: underworld, curMusic " + SF(TMain, "curMusic"));
        Record("audio_end", 3);
    }

    /// <summary>The projectiles of the boss scene (ProjectileID): wooden arrow, bullet, ball of fire, water bolt, a magic missile, a laser.</summary>
    static readonly int[] BossProjectiles = { 1, 14, 15, 27, 16, 88 };

    // ================================================================== the scenes run (`RefDumpDraw.exe <dir> scenes`; port: tests/draw.rs DRAW_MODE=scenes)
    // More of the client than the draw run: menus, biomes, events, in-game UI screens, mounts / pets / minions, the smart
    // cursor, the other lighting modes and every boss; each scene recorded for 2 frames (and every frame's digest).
    // The order is the order of ScenesWorld; volumes 0 as in the draw run.

    /// <summary>ScenesMenus: the multiplayer menu and the character list, after the title scene (the world list and world
    /// creation need a selected player: ScenesWorldMenus, from EnterWorld).</summary>
    static void ScenesMenus()
    {
        SSet(TMain, "menuMode", 12);
        Frames(10);
        Record("menu_multiplayer", 2);
        SSet(TMain, "menuMode", 0);
        Frames(5);
        object ui = SF(TMain, "MenuUI");
        CallStatic(TMain, "LoadPlayers");
        CallStatic(TMain, "OpenCharacterSelectUI");
        Frames(20);
        Record("menu_characters", 2);
        ui.GetType().GetMethod("SetState").Invoke(ui, new object[] { null });
        SSet(TMain, "menuMode", 0);
        Frames(5);
    }

    /// <summary>The scenes run's state after a recorded scene (port: tests/draw.rs `state_line`, the same bytes):
    /// Main.rand (inext, FNV-1a 64 of the seed array), the local player (position, velocity, life, mana), and FNV-1a 64
    /// over every active NPC (index, type, position, velocity, life, ai[]) and projectile (index, type, position,
    /// velocity, ai[], timeLeft), all little endian, floats by their bits; "menu" while Main.gameMenu.</summary>
    static readonly StringBuilder StateLines = new StringBuilder();
    static void StateLine(string scene)
    {
        // in the menus there is no world state to compare (the draw lines are)
        if ((bool)SF(TMain, "gameMenu")) { StateLines.Append("state " + scene + " menu\n"); return; }
        Func<ulong, int, ulong> i4 = (h, v) => { for (int k = 0; k < 4; k++) { h ^= (byte)(v >> (8 * k)); h *= 1099511628211UL; } return h; };
        Func<float, int> fb = f => BitConverter.ToInt32(BitConverter.GetBytes(f), 0);
        uint inext; int[] sa;
        RandState(SF(TMain, "rand"), out inext, out sa);
        ulong rh = 14695981039346656037UL;
        foreach (int v in sa) rh = i4(rh, v);
        object pl = LocalPlayer;
        var pp = (Microsoft.Xna.Framework.Vector2)IF(pl, "position");
        var pv = (Microsoft.Xna.Framework.Vector2)IF(pl, "velocity");
        ulong nh = 14695981039346656037UL, ph = nh;
        int nn = 0, np = 0;
        Array npcs = (Array)SF(TMain, "npc");
        for (int i = 0; i < npcs.Length; i++)
        {
            object n = npcs.GetValue(i);
            if (!(bool)IF(n, "active")) continue;
            nn++;
            var p = (Microsoft.Xna.Framework.Vector2)IF(n, "position"); var v = (Microsoft.Xna.Framework.Vector2)IF(n, "velocity");
            nh = i4(i4(i4(i4(i4(i4(i4(nh, i), (int)IF(n, "type")), fb(p.X)), fb(p.Y)), fb(v.X)), fb(v.Y)), (int)IF(n, "life"));
            foreach (float a in (float[])IF(n, "ai")) nh = i4(nh, fb(a));
        }
        Array projs = (Array)SF(TMain, "projectile");
        for (int i = 0; i < projs.Length; i++)
        {
            object n = projs.GetValue(i);
            if (!(bool)IF(n, "active")) continue;
            np++;
            var p = (Microsoft.Xna.Framework.Vector2)IF(n, "position"); var v = (Microsoft.Xna.Framework.Vector2)IF(n, "velocity");
            ph = i4(i4(i4(i4(i4(i4(ph, i), (int)IF(n, "type")), fb(p.X)), fb(p.Y)), fb(v.X)), fb(v.Y));
            foreach (float a in (float[])IF(n, "ai")) ph = i4(ph, fb(a));
            ph = i4(ph, (int)IF(n, "timeLeft"));
        }
        StateLines.Append("state " + scene + " rand " + inext + " " + rh.ToString("x16") + " player " + fb(pp.X).ToString("x8") + "," + fb(pp.Y).ToString("x8") + ","
            + fb(pv.X).ToString("x8") + "," + fb(pv.Y).ToString("x8") + "," + IF(pl, "statLife") + "," + IF(pl, "statMana") + " npc " + nn + ":" + nh.ToString("x16") + " proj " + np + ":" + ph.ToString("x16") + "\n");
    }

    /// <summary>The world list and world creation, in EnterWorld after the player is selected and the worlds are loaded.</summary>
    static void ScenesWorldMenus()
    {
        CallStatic(TMain, "OpenWorldSelectUI");
        Frames(20);
        Record("menu_worlds", 2);
        object ui = SF(TMain, "MenuUI");
        ui.GetType().GetMethod("SetState").Invoke(ui, new object[] { Activator.CreateInstance(T("Terraria.GameContent.UI.States.UIWorldCreation")) });
        Frames(20);
        Record("menu_worldcreation", 2);
        ui.GetType().GetMethod("SetState").Invoke(ui, new object[] { null });
        SSet(TMain, "menuMode", 0);
        Frames(5);
    }

    /// <summary>Biomes of ref_small (REFDRAW_FINDBIOMES=1): name, the tile the player stands on (feet).</summary>
    static readonly object[][] Biomes =
    {
        new object[] { "crimson", 1408, 471 }, new object[] { "jungle", 309, 340 }, new object[] { "snow", 2499, 298 },
        new object[] { "desert", 1720, 352 }, new object[] { "mushroom", 151, 886 }, new object[] { "dungeon", 3352, 295 },
        new object[] { "temple", 790, 527 }, new object[] { "marble", 627, 859 }, new object[] { "granite", 2342, 446 },
    };
    /// <summary>The bosses (NPCID): name, type.</summary>
    static readonly object[][] Bosses =
    {
        new object[] { "kingslime", 50 }, new object[] { "eye", 4 }, new object[] { "brain", 266 }, new object[] { "eater", 13 },
        new object[] { "queenbee", 222 }, new object[] { "skeletron", 35 }, new object[] { "deerclops", 668 }, new object[] { "wall", 113 },
        new object[] { "queenslime", 657 }, new object[] { "twins", 125 }, new object[] { "destroyer", 134 }, new object[] { "prime", 127 },
        new object[] { "plantera", 262 }, new object[] { "golem", 245 }, new object[] { "fishron", 370 }, new object[] { "empress", 636 },
        new object[] { "cultist", 439 }, new object[] { "moonlord", 398 },
    };

    static void ClearEntities()
    {
        foreach (object n in (Array)SF(TMain, "npc")) ISet(n, "active", false);
        foreach (object pr in (Array)SF(TMain, "projectile")) ISet(pr, "active", false);
    }
    static int NewNpc(int x, int y, int type)
    {
        MethodInfo newNpc = T("Terraria.NPC").GetMethods(AllStatic).First(m => m.Name == "NewNPC" && m.GetParameters().Length == 10);
        return (int)newNpc.Invoke(null, new object[] { Activator.CreateInstance(T("Terraria.DataStructures.EntitySource_DebugCommand")), x, y, type, 0, 0f, 0f, 0f, 0f, 255 });
    }
    static int NewProj(float x, float y, float vx, float vy, int type)
    {
        MethodInfo newProj = T("Terraria.Projectile").GetMethods(AllStatic).First(m => m.Name == "NewProjectile" && m.GetParameters()[1].ParameterType.Name == "Vector2");
        return (int)newProj.Invoke(null, new object[] { Activator.CreateInstance(T("Terraria.DataStructures.EntitySource_DebugCommand")), V2(x, y), V2(vx, vy), type, 10, 1f, (int)SF(TMain, "myPlayer"), 0f, 0f, 0f, null });
    }
    static void Fancy(object state)
    {
        T("Terraria.UI.IngameFancyUI").GetMethods(AllStatic).First(m => m.Name == "OpenUIState" && m.GetParameters().Length == 2).Invoke(null, new object[] { state, true });
    }
    static void FancyClose() { T("Terraria.UI.IngameFancyUI").GetMethods(AllStatic).First(m => m.Name == "Close" && m.GetParameters().Length == 1).Invoke(null, new object[] { false }); }

    /// <summary>The scenes run in the world (after EnterWorld).</summary>
    static void ScenesWorld()
    {
        int sx = (int)SF(TMain, "spawnTileX"), sy = (int)SF(TMain, "spawnTileY");
        object pl = LocalPlayer;
        SetTime(true, 27000.0);
        PlacePlayer(sx, sy);
        GiveItem(0, 4);
        GiveItem(1, 1);
        GiveItem(2, 9);
        ((Array)IF(pl, "inventory")).GetValue(2).GetType().GetField("stack").SetValue(((Array)IF(pl, "inventory")).GetValue(2), 99);
        Frames(30);
        // biomes (backgrounds, music state, ambience, the scene metrics)
        foreach (object[] b in Biomes)
        {
            PlacePlayer((int)b[1], (int)b[2]);
            Frames(90);
            Record("biome_" + b[0], 2);
        }
        // events at the spawn
        PlacePlayer(sx, sy);
        Frames(30);
        CallStatic(TMain, "StartRain", false, null, false);
        Frames(60);
        Record("event_rain", 2);
        CallStatic(TMain, "StopRain", true);
        SetTime(false, 16200.0);
        SSet(TMain, "bloodMoon", true);
        Frames(60);
        Record("event_bloodmoon", 2);
        SSet(TMain, "bloodMoon", false);
        SetTime(true, 27000.0);
        SSet(TMain, "eclipse", true);
        Frames(60);
        Record("event_eclipse", 2);
        SSet(TMain, "eclipse", false);
        CallStatic(TMain, "StartSlimeRain", false);
        Frames(90);
        Record("event_slimerain", 2);
        CallStatic(TMain, "StopSlimeRain", false);
        ClearEntities();
        CallStatic(TMain, "StartInvasion", 1);
        Frames(120);
        Record("event_goblins", 2);
        SSet(TMain, "invasionType", 0);
        SSet(TMain, "invasionSize", 0);
        ClearEntities();
        PlacePlayer(1720, 352);
        T("Terraria.GameContent.Events.Sandstorm").GetMethod("StartSandstorm", AllStatic).Invoke(null, null);
        Frames(120);
        Record("event_sandstorm", 2);
        T("Terraria.GameContent.Events.Sandstorm").GetMethod("StopSandstorm", AllStatic).Invoke(null, null);
        PlacePlayer(sx, sy);
        Frames(30);
        // in-game UI
        Type opts = T("Terraria.IngameOptions");
        CallStatic(opts, "Open");
        for (int c = 0; c < 4; c++)
        {
            SSet(opts, "category", c);
            Frames(4);
            Record("ui_settings_" + c, 2);
        }
        opts.GetMethods(AllStatic).First(m => m.Name == "Close" && m.GetParameters().Length == 1).Invoke(null, new object[] { true });
        Frames(5);
        SSet(TMain, "playerInventory", true);
        CallStatic(T("Terraria.Recipe"), "UpdateRecipeList");
        Frames(10);
        Record("ui_crafting", 2);
        SSet(TMain, "EquipPage", 1);
        Frames(5);
        Record("ui_housing", 2);
        SSet(TMain, "EquipPage", 0);
        SSet(TMain, "playerInventory", false);
        var ppos = (Microsoft.Xna.Framework.Vector2)IF(pl, "position");
        int guide = NewNpc((int)ppos.X + 40, (int)ppos.Y + 40, 22);
        int merchant = NewNpc((int)ppos.X - 40, (int)ppos.Y + 40, 17);
        Frames(5);
        pl.GetType().GetMethod("SetTalkNPC").Invoke(pl, new object[] { guide });
        SSet(TMain, "npcChatText", ((Array)SF(TMain, "npc")).GetValue(guide).GetType().GetMethod("GetChat").Invoke(((Array)SF(TMain, "npc")).GetValue(guide), null));
        Frames(10);
        Record("ui_chat", 2);
        pl.GetType().GetMethod("SetTalkNPC").Invoke(pl, new object[] { merchant });
        SSet(TMain, "npcChatText", "");
        CallStatic(TMain, "SetNPCShopIndex", 1);
        SSet(TMain, "playerInventory", true);
        Frames(10);
        Record("ui_shop", 2);
        pl.GetType().GetMethod("SetTalkNPC").Invoke(pl, new object[] { -1 });
        CallStatic(TMain, "SetNPCShopIndex", 0);
        SSet(TMain, "playerInventory", false);
        Frames(5);
        Fancy(SF(TMain, "BestiaryUI"));
        Frames(20);
        Record("ui_bestiary", 2);
        FancyClose();
        Frames(5);
        Fancy(Activator.CreateInstance(T("Terraria.GameContent.UI.States.UIEmotesMenu")));
        Frames(10);
        Record("ui_emotes", 2);
        FancyClose();
        Frames(5);
        // a chest of the world, opened
        {
            Array chests = (Array)SF(TMain, "chest");
            int ci = -1;
            for (int i = 0; i < chests.Length && ci < 0; i++) { object c = chests.GetValue(i); if (c != null && (int)IF(c, "y") < 300) ci = i; }
            object ch = chests.GetValue(ci);
            PlacePlayer((int)IF(ch, "x"), (int)IF(ch, "y") + 2);
            Frames(10);
            ISet(pl, "chest", ci);
            SSet(TMain, "playerInventory", true);
            Frames(10);
            Record("ui_chest", 2);
            ISet(pl, "chest", -1);
            SSet(TMain, "playerInventory", false);
            PlacePlayer(sx, sy);
            Frames(10);
        }
        object cm = SF(T("Terraria.Graphics.Capture.CaptureManager"), "Instance");
        cm.GetType().GetProperty("Active").SetValue(cm, true, null);
        Frames(10);
        Record("ui_camera", 2);
        cm.GetType().GetProperty("Active").SetValue(cm, false, null);
        Frames(5);
        // the player's own things
        object mount = IF(pl, "mount");
        mount.GetType().GetMethod("SetMount").Invoke(mount, new object[] { 3, pl });
        Frames(30);
        Record("player_mount", 2);
        mount.GetType().GetMethods().First(m => m.Name == "Dismount").Invoke(mount, new object[] { pl, false });
        Frames(5);
        pl.GetType().GetMethod("AddBuff").Invoke(pl, new object[] { 40, 3600, false });
        pl.GetType().GetMethod("AddBuff").Invoke(pl, new object[] { 64, 3600, false });
        NewProj(ppos.X, ppos.Y - 40, 0f, 0f, 266);
        Frames(60);
        Record("player_pets", 2);
        SSet(TMain, "SmartCursorWanted_Mouse", true);
        Press(Microsoft.Xna.Framework.Input.Keys.D2);
        Aim(ppos.X + 40, ppos.Y + 56);
        Frames(10);
        Record("player_smartcursor", 2);
        SSet(TMain, "SmartCursorWanted_Mouse", false);
        InMouseX = 640; InMouseY = 360;
        Frames(5);
        PropertyInfo mode = T("Terraria.Lighting").GetProperty("Mode", AllStatic);
        Type lm = T("Terraria.Graphics.Light.LightMode");
        PlacePlayer(1999, 547);
        foreach (string m in new[] { "Retro", "Trippy", "White" })
        {
            mode.SetValue(null, Enum.Parse(lm, m), null);
            Frames(20);
            Record("light_" + m.ToLowerInvariant(), 2);
        }
        mode.SetValue(null, Enum.Parse(lm, "Color"), null);
        PlacePlayer(sx, sy);
        Frames(30);
        ScenesItems();
        // every boss, one after the other, at night near the spawn
        SetTime(false, 16200.0);
        foreach (object[] b in Bosses)
        {
            ClearEntities();
            var pos = (Microsoft.Xna.Framework.Vector2)IF(pl, "position");
            NewNpc((int)pos.X + 300, (int)pos.Y - 200, (int)b[1]);
            if ((int)b[1] == 125) NewNpc((int)pos.X - 300, (int)pos.Y - 200, 126);
            Frames(40);
            Record("boss_" + b[0], 2);
        }
        ClearEntities();
        Frames(10);
        Record("scenes_end", 2);
    }

    /// <summary>Scenes of the second round (the largest groups of float methods the first scenes left without a
    /// reference: tools/cs2rs/FLOAT_COVERAGE.md): an item tooltip, the stylist and clothes windows, swords with
    /// projectiles and particles, wings and Hermes boots, wires shown by a wrench, a boss bag opened, the aurora.</summary>
    static void ScenesItems()
    {
        int sx = (int)SF(TMain, "spawnTileX"), sy = (int)SF(TMain, "spawnTileY");
        object pl = LocalPlayer;
        SetTime(true, 27000.0);
        PlacePlayer(sx, sy);
        Frames(10);
        // the tooltip of the first hotbar item (the cursor over its slot)
        SSet(TMain, "playerInventory", true);
        InMouseX = 40; InMouseY = 40;
        Frames(5);
        Record("ui_tooltip", 2);
        InMouseX = 640; InMouseY = 360;
        SSet(TMain, "playerInventory", false);
        Frames(5);
        CallStatic(TMain, "OpenHairWindow");
        Frames(10);
        Record("ui_hair", 2);
        SSet(TMain, "hairWindow", false);
        Frames(5);
        CallStatic(TMain, "OpenClothesWindow");
        Frames(10);
        Record("ui_clothes", 2);
        SSet(TMain, "clothesWindow", false);
        Frames(5);
        // swords that shoot (Terra Blade, True Night's Edge, True Excalibur), swung to the right
        int[] swords = { 757, 675, 674 };
        for (int k = 0; k < swords.Length; k++)
        {
            GiveItem(0, swords[k]);
            Press(Microsoft.Xna.Framework.Input.Keys.D1);
            InMouseX = 900; InMouseY = 340;
            InLeft = true;
            Frames(30);
            InLeft = false;
            Record("item_sword_" + swords[k], 2);
            Frames(20);
        }
        InMouseX = 640; InMouseY = 360;
        // wings (Angel Wings) and Hermes Boots as accessories: flying up, then running
        ((Array)IF(pl, "armor")).GetValue(3).GetType().GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int)).Invoke(((Array)IF(pl, "armor")).GetValue(3), new object[] { 493, null });
        ((Array)IF(pl, "armor")).GetValue(4).GetType().GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int)).Invoke(((Array)IF(pl, "armor")).GetValue(4), new object[] { 54, null });
        Frames(5);
        InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.Space };
        Frames(40);
        Record("player_wings", 2);
        InKeys = null;
        Frames(60);
        InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.D };
        Frames(80);
        Record("player_boots", 2);
        InKeys = null;
        Frames(20);
        PlacePlayer(sx, sy);
        // a wrench held: wires are drawn
        GiveItem(0, 509);
        Press(Microsoft.Xna.Framework.Input.Keys.D1);
        Frames(10);
        Record("item_wrench", 2);
        // a boss bag (King Slime's) opened
        pl.GetType().GetMethod("OpenBossBag").Invoke(pl, new object[] { 3318 });
        Frames(30);
        Record("item_bossbag", 2);
        // the aurora: the snow biome at night
        SetTime(false, 16200.0);
        PlacePlayer(2499, 298);
        Frames(90);
        Record("biome_snow_night", 2);
        SetTime(true, 27000.0);
        PlacePlayer(sx, sy);
        ClearEntities();
        Frames(30);
    }

    // REFDRAW_FINDBIOMES=1: a standing place in each biome of ref_small (the densest window of the biome's tiles, then
    // the nearest spot with 3 open tiles above a solid one); printed, then hard-coded in the scenes run on both sides
    static void FindBiomes()
    {
        Snap s = TakeSnapshot(null);
        Func<int, int, int> at = (x, y) => (x * s.H + y) * RecSize;
        Func<int, int, bool> act = (x, y) => (s.Buf[at(x, y) + 5] & 0x20) != 0;
        Func<int, int, int> type = (x, y) => s.Buf[at(x, y)] | (s.Buf[at(x, y) + 1] << 8);
        Func<int, int, int> wall = (x, y) => s.Buf[at(x, y) + 2] | (s.Buf[at(x, y) + 3] << 8);
        Func<int, int, bool> solid = (x, y) => act(x, y) && type(x, y) != 5 && type(x, y) != 3 && type(x, y) != 4;
        var kinds = new Dictionary<string, int[]>
        {
            { "corruption", new[] { 23, 25, 163 } }, { "crimson", new[] { 199, 203, 200 } }, { "jungle", new[] { 60 } },
            { "snow", new[] { 147, 161 } }, { "desert", new[] { 53, 396, 397 } }, { "mushroom", new[] { 70 } },
            { "dungeon", new[] { 41, 43, 44 } }, { "temple", new[] { 226 } }, { "marble", new[] { 367 } }, { "granite", new[] { 368 } },
        };
        foreach (var k in kinds)
        {
            int best = 0, bx = 0, by = 0;
            for (int x = 60; x < s.W - 60; x += 6)
                for (int y = 60; y < s.H - 60; y += 6)
                {
                    int n = 0;
                    for (int dx = -20; dx <= 20; dx += 4) for (int dy = -12; dy <= 12; dy += 4) if (act(x + dx, y + dy) && Array.IndexOf(k.Value, type(x + dx, y + dy)) >= 0) n++;
                    if (n > best) { best = n; bx = x; by = y; }
                }
            // a standing spot near (bx, by): open x, y-1, y-2 (no solid tile, no liquid) over a solid y+1
            int sx = -1, sy = -1;
            for (int r = 0; r < 60 && sx < 0; r++)
                for (int dx = -r; dx <= r && sx < 0; dx++)
                    for (int dy = -r; dy <= r && sx < 0; dy++)
                    {
                        if (Math.Max(Math.Abs(dx), Math.Abs(dy)) != r) continue;
                        int x = bx + dx, y = by + dy;
                        if (x < 50 || y < 50 || x >= s.W - 50 || y >= s.H - 50) continue;
                        bool ok = !solid(x, y) && !solid(x, y - 1) && !solid(x, y - 2) && !solid(x + 1, y) && !solid(x + 1, y - 1) && !solid(x + 1, y - 2) && solid(x, y + 1)
                            && s.Buf[at(x, y) + 4] == 0 && s.Buf[at(x, y - 1) + 4] == 0;
                        if (ok) { sx = x; sy = y; }
                    }
            Console.WriteLine("biome " + k.Key + " density " + best + " at " + bx + "," + by + " stand " + sx + "," + sy + " wall " + (sx > 0 ? wall(sx, sy) : -1));
        }
    }

    // REFDRAW_FIND=1: where the world scenes look (printed, then hard-coded in the scenario on both sides)
    static void FindPlaces()
    {
        Snap s = TakeSnapshot(null);
        int sx = (int)SF(TMain, "spawnTileX"), sy = (int)SF(TMain, "spawnTileY");
        double surface = (double)SF(TMain, "worldSurface"), rock = (double)SF(TMain, "rockLayer");
        Console.WriteLine("size " + s.W + "x" + s.H + " worldSurface " + surface + " rockLayer " + rock + " spawn " + sx + "," + sy);
        Func<int, int, int> at = (x, y) => (x * s.H + y) * RecSize;
        Func<int, int, bool> solid = (x, y) => (s.Buf[at(x, y) + 5] & 0x20) != 0;
        Func<int, int, int> liq = (x, y) => s.Buf[at(x, y) + 4];
        Func<int, int, int> liqType = (x, y) => (s.Buf[at(x, y) + 7] >> 5) & 3;
        for (int kind = 0; kind < 2; kind++)
        {
            int best = 0, bx = 0, by = 0;
            for (int x = 60; x < s.W - 60; x += 2)
                for (int y = 60; y < s.H - 60; y += 2)
                {
                    if (liq(x, y) < 200 || liqType(x, y) != kind) continue;
                    int n = 0;
                    for (int dx = -20; dx <= 20; dx += 2) for (int dy = -12; dy <= 12; dy += 2) if (liq(x + dx, y + dy) > 0 && liqType(x + dx, y + dy) == kind) n++;
                    if (n > best) { best = n; bx = x; by = y; }
                }
            Console.WriteLine((kind == 0 ? "water" : "lava") + " best " + bx + "," + by + " (" + best + ")");
        }
        foreach (int x in new[] { 70, 90, 110, 130, 150 })
        {
            int top = -1, wat = -1;
            for (int y = 50; y < s.H - 50; y++) { if (wat < 0 && liq(x, y) > 0) wat = y; if (solid(x, y)) { top = y; break; } }
            Console.WriteLine("column " + x + ": first liquid " + wat + ", first solid " + top);
        }
        for (int x = 400; x < 520; x += 4)
            for (int y = 1000; y < s.H - 20; y++)
                if (solid(x, y) && !solid(x, y - 1) && !solid(x, y - 2) && !solid(x, y - 3) && liq(x, y - 1) == 0) { Console.WriteLine("hell ground " + x + "," + (y - 1)); x += 20; break; }
        // a cave: an open area below the rock layer near the spawn, with ground under its middle
        for (int y = (int)rock + 30; y < (int)rock + 250; y += 3)
            for (int x = sx - 300; x < sx + 300; x += 3)
            {
                bool open = true;
                for (int dx = -12; dx <= 12 && open; dx++) for (int dy = -8; dy <= 0 && open; dy++) if (solid(x + dx, y + dy) || liq(x + dx, y + dy) > 0) open = false;
                if (!open || !solid(x, y + 1)) continue;
                Console.WriteLine("cave " + x + "," + y);
                return;
            }
    }
}
