// RefDumpLight: runs the real game's lighting engine (Terraria.Lighting, LightingEngine, TileLightScanner, LightMap)
// headless over a real world file and dumps the light colour of every tile of a few areas.
// Pure reflection over work\refserver\TerrariaServer.exe, the same technique as tools\refdump\RefDump.cs
// (its game initialisation is repeated here so this file stands alone). C# 5, .NET Framework 4, x86.
//
//   RefDumpLight.exe <world.wld> <outdir>
//
// What it does, in order (every step is the game's own code, called by reflection):
//   1. Initialisation as RefDump.InitGame: Program.SavePath, Main.dedServ = true, language, new Main(), Lang, Main.rand,
//      main.Initialize() (which runs Lighting.Initialize(): Lighting.Mode = Color, NewEngine rebuilt).
//   2. Copies <world.wld> to work\light-scratch\ and loads it with WorldFile.LoadWorld() (Main.tile, Main.worldSurface, ...).
//   3. For every entry of Areas below:
//        a. optional "scramble": overwrites the tiles of the processed area with a deterministic pseudo-random pattern
//           (every tile type, wall, liquid, paint, frame, slope) so that all branches of TileLightScanner are reached;
//        b. dumps the tiles of the processed area plus a one tile margin (<name>.tiles.bin);
//        c. sets the documented Main state (see the header written to <name>.light.bin) and calls Lighting.Initialize();
//        d. seeds the three FastRandom fields the engine keeps (TileLightScanner._random, both LightMap._random: the
//           game seeds them with a GUID) and queues three Lighting.AddLight calls;
//        e. calls Lighting.LightTiles(area). LightingEngine.ProcessArea is a state machine that advances one step per call:
//           MinimapUpdate -> ExportMetrics -> Scan -> Blur -> (repeat). ExportMetrics calls Main.UpdateSceneMetrics, which needs
//           the camera and so a graphics device and throws headless; the harness therefore sets
//           SceneMetrics.PerspectivePlayer itself and starts each cycle at Scan (see Run1). It runs 2 cycles = Scan, Blur,
//           Scan, Blur, so both light maps, the Present swaps and the per-frame light swap are exercised;
//        f. writes <name>.light.bin: for every tile of the processed area (area inflated by 28, LightingEngine.AREA_PADDING),
//           x outer, y inner, the engine's raw Vector3 (NewEngine.GetColor) and Lighting.GetColor(x, y).PackedValue.
//
// Output (little endian), <name>.light.bin:
//   "LGHT" | i32 version=2 | i32 area x,y,w,h | i32 processed x,y,w,h | u64 seedScanner, seedActive, seedWorking | i32 cycles |
//   u32 gameUpdateCount | u8 tileColor r,g,b,a | u8 mouseTextColor,0,0,0 | f64 timeForVisualEffects | f32 globalTimeWrappedHourly |
//   i32 waterStyle | u8 dayTime,0,0,0 | f64 time | i32 nLights | nLights * (i32 x, i32 y, f32 r, f32 g, f32 b) | f32 demonTorch | i32 DiscoR,G,B | i32 sundialCooldown, moondialCooldown | f32 shimmerDarken | i32 cageFrames | u8 raining,0,0,0 | i32 myPlayer | i32 mapDelay | i32 n, n*f32 liquidAlpha | i32 rows, cols, rows*cols bytes jellyfishCageMode | f32 airLightDecay, solidLightDecay |
//   then w*h records of (f32 X, f32 Y, f32 Z, u32 packed)
// <name>.tiles.bin: "TILE" | i32 version=1 | i32 x,y,w,h | w*h records of 14 bytes (same layout as refdata\framed\*.post.bin:
//   u16 type | u16 wall | u8 liquid | u16 sTileHeader | u8 bTileHeader | u8 bTileHeader2 | u8 bTileHeader3 | i16 frameX | i16 frameY)
//   x outer, y inner, clipped to the world.
// refdata\light\MANIFEST.txt: one line per area plus the init log and timings.
//
// Headless: no window, no network, nothing written to the game folder; time-boxed by a watchdog (240 s).
using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;
using System.Threading;

static class RefDumpLight
{
    static Assembly G, Xna;
    static string GameDir, OutDir, SaveDir;
    static readonly Dictionary<string, Assembly> Embedded = new Dictionary<string, Assembly>();
    const BindingFlags AllStatic = BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic;
    const BindingFlags AllInst = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;
    static readonly List<string> Log = new List<string>();

    const int Cycles = 2;   // scan+blur pairs; the Rust side runs 4 ProcessArea calls per cycle (the first two are no-ops there)
    const ulong SeedScanner = 0x1234567UL, SeedActive = 0xABCDEF01UL, SeedWorking = 0x5EED5EEDUL;

    class AreaSpec
    {
        public string Name;
        public int X, Y, W, H;       // the rectangle handed to Lighting.LightTiles
        public bool Scramble;
        public int WaterStyle;
        public bool Day;
        public double Time;
        public bool Lights;
        public float Gtwh = 12.5f;   // Main.GlobalTimeWrappedHourly
        public bool Shimmer;   // scrambled area in which a third of the tiles are configured to call LiquidRenderer.GetShimmerBaseColor
    }

    [STAThread]
    static int Main(string[] args)
    {
        if (args.Length < 2) { Console.Error.WriteLine("usage: RefDumpLight.exe <world.wld> <outdir>"); return 2; }
        string root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(root, "work", "refserver");
        SaveDir = Path.Combine(root, "work", "refdump-save");
        OutDir = Path.GetFullPath(args[1]);
        Directory.CreateDirectory(SaveDir);
        Directory.CreateDirectory(OutDir);
        var wd = new Thread(() => { Thread.Sleep(240000); Console.Error.WriteLine("WATCHDOG: timeout"); Environment.Exit(3); });
        wd.IsBackground = true; wd.Start();
        int rc = 0;
        try { Run(root, Path.GetFullPath(args[0])); }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }

    // ---------------------------------------------------------------- reflection helpers (as RefDump.cs)
    static Assembly OnResolve(object sender, ResolveEventArgs a)
    {
        string simple = new AssemblyName(a.Name).Name;
        lock (Embedded)
        {
            Assembly cached;
            if (Embedded.TryGetValue(simple, out cached)) return cached;
            if (G != null)
            {
                string res = Array.Find(G.GetManifestResourceNames(), e => e.EndsWith(simple + ".dll"));
                if (res != null)
                {
                    using (Stream s = G.GetManifestResourceStream(res))
                    {
                        byte[] b = new byte[s.Length];
                        int off = 0; while (off < b.Length) { int n = s.Read(b, off, b.Length - off); if (n <= 0) break; off += n; }
                        Assembly asm = Assembly.Load(b);
                        Embedded[simple] = asm;
                        return asm;
                    }
                }
            }
            string p = Path.Combine(GameDir, simple + ".dll");
            if (File.Exists(p)) { Assembly asm = Assembly.LoadFrom(p); Embedded[simple] = asm; return asm; }
        }
        return null;
    }
    static Type T(string name)
    {
        Type t = G.GetType(name, false);
        if (t == null && name.StartsWith("Microsoft.Xna.Framework."))
        {
            if (Xna == null) Xna = Assembly.LoadFrom(Path.Combine(GameDir, "Microsoft.Xna.Framework.dll"));
            t = Xna.GetType(name, false);
        }
        if (t == null) throw new Exception("type not found: " + name);
        return t;
    }
    static object SF(Type t, string name)
    {
        FieldInfo f = t.GetField(name, AllStatic);
        if (f != null) return f.GetValue(null);
        PropertyInfo p = t.GetProperty(name, AllStatic);
        if (p != null) return p.GetValue(null, null);
        throw new Exception("static member not found: " + t.FullName + "." + name);
    }
    static void SSet(Type t, string name, object v)
    {
        FieldInfo f = t.GetField(name, AllStatic);
        if (f != null) { f.SetValue(null, v); return; }
        PropertyInfo p = t.GetProperty(name, AllStatic);
        if (p != null) { p.SetValue(null, v, null); return; }
        throw new Exception("static member not found: " + t.FullName + "." + name);
    }
    static object Unwrap(MethodInfo m, object o, object[] args)
    {
        try { return m.Invoke(o, args); }
        catch (TargetInvocationException e) { throw new Exception(m.DeclaringType.FullName + "." + m.Name + " threw " + e.InnerException.GetType().Name + ": " + e.InnerException.Message, e.InnerException); }
    }
    static object CallStatic(Type t, string name, params object[] args)
    {
        foreach (MethodInfo m in t.GetMethods(AllStatic))
            if (m.Name == name && m.GetParameters().Length == args.Length) return Unwrap(m, null, args);
        throw new Exception("static method not found: " + t.FullName + "." + name);
    }
    static void Step(string desc, Action a)
    {
        try { a(); Log.Add("ok   " + desc); Console.WriteLine("ok   " + desc); }
        catch (Exception e) { Log.Add("FAIL " + desc + " -> " + e.GetType().Name + ": " + e.Message); Console.WriteLine("FAIL " + desc + " -> " + e); throw; }
    }

    static object MainInst;
    static void InitGame()
    {
        AppDomain.CurrentDomain.AssemblyResolve += OnResolve;
        Environment.CurrentDirectory = GameDir;
        Environment.SetEnvironmentVariable("PATH", GameDir + ";" + Environment.GetEnvironmentVariable("PATH"));
        G = Assembly.LoadFrom(Path.Combine(GameDir, "TerrariaServer.exe"));
        Type program = T("Terraria.Program"), main = T("Terraria.Main");
        Step("Program.SavePath = work\\refdump-save", () => SSet(program, "SavePath", SaveDir));
        Step("Main.dedServ = true; Main.showSplash = false", () => { SSet(main, "dedServ", true); SSet(main, "showSplash", false); });
        Step("LanguageManager.Instance.SetLanguage(GameCulture.DefaultCulture)", () =>
        {
            object lm = SF(T("Terraria.Localization.LanguageManager"), "Instance");
            object gc = SF(T("Terraria.Localization.GameCulture"), "DefaultCulture");
            MethodInfo sl = lm.GetType().GetMethods().First(m => m.Name == "SetLanguage" && m.GetParameters().Length == 1 && m.GetParameters()[0].ParameterType == gc.GetType());
            Unwrap(sl, lm, new object[] { gc });
        });
        Step("main = new Main()", () => MainInst = Activator.CreateInstance(main));
        Step("Lang.InitializeLegacyLocalization()", () => CallStatic(T("Terraria.Lang"), "InitializeLegacyLocalization"));
        Step("Main.rand = new UnifiedRandom(0)", () => SSet(main, "rand", Activator.CreateInstance(T("Terraria.Utilities.UnifiedRandom"), new object[] { 0 })));
        Step("main.Initialize()", () =>
        {
            MethodInfo init = main.GetMethod("Initialize", AllInst, null, Type.EmptyTypes, null);
            Unwrap(init, MainInst, new object[0]);
        });
    }

    // ---------------------------------------------------------------- tiles
    static Type TileT;
    static FieldInfo fType, fWall, fLiquid, fS, fB, fB2, fB3, fFx, fFy;
    static Array TileArr;
    static int W, H;

    static void InitTileAccess()
    {
        TileT = T("Terraria.Tile");
        fType = TileT.GetField("type"); fWall = TileT.GetField("wall"); fLiquid = TileT.GetField("liquid");
        fS = TileT.GetField("sTileHeader"); fB = TileT.GetField("bTileHeader"); fB2 = TileT.GetField("bTileHeader2");
        fB3 = TileT.GetField("bTileHeader3"); fFx = TileT.GetField("frameX"); fFy = TileT.GetField("frameY");
        TileArr = (Array)SF(T("Terraria.Main"), "tile");
    }

    static void Put16(byte[] b, int o, int v) { b[o] = (byte)v; b[o + 1] = (byte)(v >> 8); }

    static byte[] TileRecord(int x, int y)
    {
        byte[] r = new byte[14];
        object t = TileArr.GetValue(x, y);
        Put16(r, 0, (ushort)fType.GetValue(t));
        Put16(r, 2, (ushort)fWall.GetValue(t));
        r[4] = (byte)fLiquid.GetValue(t);
        Put16(r, 5, (ushort)fS.GetValue(t));
        r[7] = (byte)fB.GetValue(t);
        r[8] = (byte)fB2.GetValue(t);
        r[9] = (byte)fB3.GetValue(t);
        Put16(r, 10, (short)fFx.GetValue(t));
        Put16(r, 12, (short)fFy.GetValue(t));
        return r;
    }

    // Deterministic xorshift draw per tile so the pattern does not depend on the visiting order.
    static uint Hash(int x, int y, uint salt)
    {
        uint h = (uint)x * 73856093u ^ (uint)y * 19349663u ^ salt * 83492791u ^ 0x9E3779B9u;
        h ^= h >> 16; h *= 0x7feb352du; h ^= h >> 15; h *= 0x846ca68bu; h ^= h >> 16;
        return h;
    }

    static readonly int[] FrameUnits = new int[] { 18, 22, 36, 54 };

    static void ScrambleTile(int x, int y, bool shimmerArea)
    {
        object t = TileArr.GetValue(x, y);
        uint a = Hash(x, y, 1), b = Hash(x, y, 2), c = Hash(x, y, 3), d = Hash(x, y, 4), e = Hash(x, y, 5);
        bool active = (a % 10) < 7;
        ushort type = (ushort)(active ? (b % 754) : 0);
        ushort wall = (ushort)((c % 2 == 0) ? 0 : ((c >> 4) % 367));
        int liquid = (d % 5 < 2) ? 0 : (int)((d >> 8) % 256);
        int liquidType = (int)((d >> 20) % 4);
        int unit = FrameUnits[(e >> 3) % 4];
        short frameX = (short)(((e >> 6) % 220) * unit);
        short frameY = (short)(((e >> 14) % 170) * FrameUnits[(e >> 28) % 4]);
        // The original throws IndexOutOfRangeException for frames that no real world has: tiles 316..318 index
        // Main.jellyfishCageMode with ((x - frameX / 18) / 3) * ((y - frameY / 18) / 3) % cageFrames (negative for a large
        // frameY near the top of the world) and tile 171 reads Main.tile[x - frameX, y - frameY] when frameX < 10. Keep those
        // frames small so the scrambled areas stay inside what the game itself can process.
        if (type == 171) { frameX = (short)((e >> 3) % 12); frameY = (short)((e >> 14) % 8); }
        if (type >= 316 && type <= 318) { frameX = (short)((e >> 3) % 54); frameY = (short)((e >> 14) % 54); }
        // LiquidRenderer.GetShimmerBaseColor evaluates a float expression whose result goes through Math.Sin at an argument
        // of around 1000 radians; the x86 JIT (x87) keeps excess precision in that expression, so the result differs from
        // strict single precision by ~1e-5 and the difference is spread by the blur. The ordinary scrambled areas therefore
        // avoid the frames that call it (tiles 659, 667, 708 and the sub-case 43 of 93, 33, 100/173, 34, 42), and the
        // dedicated "scramble_shimmer" area puts them in on purpose; that area is compared with a tolerance.
        if (type == 659 || type == 667 || type == 708) { if (!shimmerArea) type = 0; }
        int shimStep = type == 93 || type == 34 ? 54 : type == 33 ? 22 : (type == 100 || type == 173 || type == 42) ? 36 : 0;
        if (shimStep != 0)
        {
            bool isShimmer = false;
            if (type == 93) isShimmer = frameX == 0 && frameY / 54 == 43;
            if (type == 33) isShimmer = frameX == 0 && frameY / 22 == 43;
            if (type == 100 || type == 173) isShimmer = frameX < 36 && frameY / 36 == 43;
            if (type == 34) isShimmer = frameX % 108 < 54 && frameY / 54 + 37 * (frameX / 108) == 43;
            if (type == 42) isShimmer = frameX == 0 && frameY / 36 == 43;
            if (isShimmer && !shimmerArea) frameY = (short)(frameY + shimStep);
        }
        if (shimmerArea && active && (e >> 24) % 3 == 0)
        {
            int[] shimTypes = new int[] { 659, 667, 708, 93, 33, 100, 173, 34, 42 };
            type = (ushort)shimTypes[(e >> 20) % 9];
            frameX = 0;
            frameY = (short)(type == 93 || type == 34 ? 43 * 54 : type == 33 ? 43 * 22 : (type == 100 || type == 173 || type == 42) ? 43 * 36 : 0);
        }
        int color = (int)((a >> 8) % 32);
        if (a % 3 != 0) color = 0;
        int wallColor = (int)((b >> 8) % 32);
        if (b % 3 != 0) wallColor = 0;
        int slope = (a >> 20) % 5 == 0 ? (int)((a >> 24) % 5) : 0;
        bool half = (a >> 28) % 6 == 0;
        bool inactive = (b >> 28) % 5 == 0;
        bool invisBlock = (c >> 28) % 7 == 0;
        bool invisWall = (d >> 28) % 7 == 0;
        int s = (color & 0x1F) | (active ? 0x20 : 0) | (inactive ? 0x40 : 0) | (half ? 0x400 : 0) | (slope << 12);
        int bh = (wallColor & 0x1F) | (liquidType << 5);
        int bh3 = (invisBlock ? 0x20 : 0) | (invisWall ? 0x40 : 0);
        fType.SetValue(t, type); fWall.SetValue(t, wall); fLiquid.SetValue(t, (byte)liquid);
        fS.SetValue(t, (ushort)s); fB.SetValue(t, (byte)bh); fB2.SetValue(t, (byte)0); fB3.SetValue(t, (byte)bh3);
        fFx.SetValue(t, frameX); fFy.SetValue(t, frameY);
    }

    // ---------------------------------------------------------------- run
    static void Run(string root, string worldSrc)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        InitGame();
        Type main = T("Terraria.Main"), worldGen = T("Terraria.WorldGen"), worldFile = T("Terraria.IO.WorldFile");
        Type lighting = T("Terraria.Lighting");

        string scratchDir = Path.Combine(root, "work", "light-scratch");
        Directory.CreateDirectory(scratchDir);
        string scratch = Path.Combine(scratchDir, Path.GetFileName(worldSrc));
        File.Copy(worldSrc, scratch, true);
        Step("Main.ActiveWorldFileData = WorldFile.GetAllMetadata(scratch copy, false)", () =>
        {
            object wfd = null;
            foreach (MethodInfo m in worldFile.GetMethods(AllStatic))
                if (m.Name == "GetAllMetadata" && m.GetParameters().Length == 2) wfd = Unwrap(m, null, new object[] { scratch, false });
            if (wfd == null) throw new Exception("GetAllMetadata returned null (world unreadable)");
            SSet(main, "ActiveWorldFileData", wfd);
        });
        Step("WorldFile.LoadWorld()", () =>
        {
            CallStatic(worldFile, "LoadWorld");
            if ((bool)SF(worldGen, "loadFailed")) throw new Exception("WorldGen.loadFailed after LoadWorld");
        });
        W = (int)SF(main, "maxTilesX"); H = (int)SF(main, "maxTilesY");
        InitTileAccess();
        int spawnX = (int)SF(main, "spawnTileX"), spawnY = (int)SF(main, "spawnTileY");
        double worldSurface = (double)SF(main, "worldSurface"), rockLayer = (double)SF(main, "rockLayer");
        Log.Add("world " + W + " x " + H + " spawn " + spawnX + "," + spawnY + " worldSurface " + worldSurface + " rockLayer " + rockLayer);
        Console.WriteLine(Log[Log.Count - 1]);

        int underworld = H - 200;
        var areas = new List<AreaSpec>();
        areas.Add(new AreaSpec { Name = "spawn", X = spawnX - 50, Y = spawnY - 40, W = 100, H = 80, WaterStyle = 0, Day = true, Time = 13500.0, Lights = true });
        areas.Add(new AreaSpec { Name = "cavern", X = spawnX - 40, Y = (int)rockLayer + 60, W = 90, H = 70, WaterStyle = 2, Day = true, Time = 20000.0, Lights = true });
        areas.Add(new AreaSpec { Name = "hell", X = spawnX - 45, Y = underworld - 15, W = 90, H = 60, WaterStyle = 0, Day = false, Time = 8000.0, Lights = false });
        // GlobalTimeWrappedHourly = 0 makes (float)Math.Sin(t * 2f) exactly 0, so ApplyHellLight's num4 is the constant 0.55f and
        // the one excess-precision expression of the hell light disappears: the area must then agree to the last bit.
        areas.Add(new AreaSpec { Name = "hell_t0", X = spawnX - 45, Y = underworld - 15, W = 90, H = 60, WaterStyle = 0, Day = false, Time = 8000.0, Lights = false, Gtwh = 0f });
        areas.Add(new AreaSpec { Name = "edge_left", X = -40, Y = spawnY - 20, W = 100, H = 60, WaterStyle = 1, Day = true, Time = 13500.0, Lights = true });
        areas.Add(new AreaSpec { Name = "edge_bottom", X = spawnX + 600, Y = H - 40, W = 80, H = 60, WaterStyle = 0, Day = false, Time = 1000.0, Lights = false });
        areas.Add(new AreaSpec { Name = "scramble_surface", X = spawnX + 200, Y = (int)worldSurface - 40, W = 80, H = 70, Scramble = true, WaterStyle = 3, Day = true, Time = 13500.0, Lights = true });
        areas.Add(new AreaSpec { Name = "scramble_deep", X = spawnX + 330, Y = (int)rockLayer + 20, W = 80, H = 70, Scramble = true, WaterStyle = 9, Day = false, Time = 5000.0, Lights = true });
        areas.Add(new AreaSpec { Name = "scramble_hell", X = spawnX + 450, Y = underworld - 25, W = 80, H = 70, Scramble = true, WaterStyle = 13, Day = false, Time = 3000.0, Lights = false });

        areas.Add(new AreaSpec { Name = "scramble_shimmer", X = spawnX + 560, Y = (int)worldSurface + 60, W = 80, H = 70, Scramble = true, Shimmer = true, WaterStyle = 0, Day = true, Time = 13500.0, Lights = false });
        var manifest = new StringBuilder();
        foreach (AreaSpec a in areas)
        {
            Run1(a, main, lighting);
            manifest.AppendLine("area " + a.Name + " " + a.X + " " + a.Y + " " + a.W + " " + a.H + " scramble=" + (a.Scramble ? 1 : 0) + " waterStyle=" + a.WaterStyle + " day=" + (a.Day ? 1 : 0));
        }
        DumpShimmerTable();
        manifest.AppendLine("cycles " + Cycles);
        manifest.AppendLine("world " + W + " x " + H);
        foreach (string l in Log) manifest.AppendLine("log " + l);
        manifest.AppendLine("elapsed_ms " + sw.ElapsedMilliseconds);
        File.WriteAllText(Path.Combine(OutDir, "MANIFEST.txt"), manifest.ToString());
        Console.WriteLine("done in " + sw.ElapsedMilliseconds + " ms");
    }

    // LiquidRenderer.GetShimmerBaseColor(float, float) for a grid of tiles, with Main.timeForVisualEffects as Run1 left it.
    static void DumpShimmerTable()
    {
        Type lr = T("Terraria.GameContent.Liquid.LiquidRenderer");
        MethodInfo gs = null;
        foreach (MethodInfo m in lr.GetMethods(AllStatic)) if (m.Name == "GetShimmerBaseColor" && m.GetParameters().Length == 2) gs = m;
        Type v4 = T("Microsoft.Xna.Framework.Vector4");
        FieldInfo[] f = new FieldInfo[] { v4.GetField("X"), v4.GetField("Y"), v4.GetField("Z"), v4.GetField("W") };
        var pts = new List<float[]>();
        for (int x = 2000; x < 2300; x += 13)
            for (int y = 200; y < 600; y += 11)
            {
                object v = Unwrap(gs, null, new object[] { (float)x, (float)y });
                pts.Add(new float[] { x, y, (float)f[0].GetValue(v), (float)f[1].GetValue(v), (float)f[2].GetValue(v), (float)f[3].GetValue(v) });
            }
        using (var bw = new BinaryWriter(File.Create(Path.Combine(OutDir, "shimmer.bin"))))
        {
            bw.Write(Encoding.ASCII.GetBytes("SHMR")); bw.Write(1);
            bw.Write(SF(T("Terraria.Main"), "timeForVisualEffects") is double ? (double)SF(T("Terraria.Main"), "timeForVisualEffects") : 0.0);
            bw.Write(pts.Count);
            foreach (float[] p in pts) foreach (float q in p) bw.Write(q);
        }
    }

    static object Rect(int x, int y, int w, int h)
    {
        return Activator.CreateInstance(T("Microsoft.Xna.Framework.Rectangle"), new object[] { x, y, w, h });
    }

    static object FastRandom(ulong seed)
    {
        return Activator.CreateInstance(T("Terraria.Utilities.FastRandom"), new object[] { seed });
    }

    static void Run1(AreaSpec a, Type main, Type lighting)
    {
        const int Pad = 28;
        int px = a.X - Pad, py = a.Y - Pad, pw = a.W + 2 * Pad, ph = a.H + 2 * Pad;

        if (a.Scramble)
            for (int x = Math.Max(0, px - 2); x < Math.Min(W, px + pw + 2); x++)
                for (int y = Math.Max(0, py - 2); y < Math.Min(H, py + ph + 2); y++)
                    ScrambleTile(x, y, a.Shimmer);

        // tile dump: processed area plus a one tile margin, clipped to the world
        int tx0 = Math.Max(0, px - 1), ty0 = Math.Max(0, py - 1), tx1 = Math.Min(W, px + pw + 1), ty1 = Math.Min(H, py + ph + 1);
        using (var bw = new BinaryWriter(File.Create(Path.Combine(OutDir, a.Name + ".tiles.bin"))))
        {
            bw.Write(Encoding.ASCII.GetBytes("TILE")); bw.Write(1);
            bw.Write(tx0); bw.Write(ty0); bw.Write(tx1 - tx0); bw.Write(ty1 - ty0);
            for (int x = tx0; x < tx1; x++)
                for (int y = ty0; y < ty1; y++) bw.Write(TileRecord(x, y));
        }

        // documented state
        byte tcR = 214, tcG = 196, tcB = 170, tcA = 255;
        byte mouseTextColor = 200;
        double tfve = 1234.5;
        float gtwh = a.Gtwh;
        uint gameUpdateCount = 1;
        object tileColor = Activator.CreateInstance(T("Microsoft.Xna.Framework.Color"), new object[] { (int)tcR, (int)tcG, (int)tcB, (int)tcA });
        SSet(main, "gameMenu", false);
        SSet(main, "netMode", 0);
        SSet(main, "gamePaused", false);
        SSet(main, "mapEnabled", false);
        SSet(main, "dayTime", a.Day);
        SSet(main, "time", a.Time);
        SSet(main, "tileColor", tileColor);
        SSet(main, "mouseTextColor", mouseTextColor);
        SSet(main, "timeForVisualEffects", tfve);
        SSet(main, "GlobalTimeWrappedHourly", gtwh);
        SSet(main, "waterStyle", a.WaterStyle);
        SSet(main, "_gameUpdateCount", gameUpdateCount);

        CallStatic(lighting, "Initialize");
        object engine = SF(lighting, "NewEngine");
        Type engT = engine.GetType();
        FieldInfo fScanner = engT.GetField("_tileScanner", AllInst), fActive = engT.GetField("_activeLightMap", AllInst), fWorking = engT.GetField("_workingLightMap", AllInst);
        object scanner = fScanner.GetValue(engine), mapA = fActive.GetValue(engine), mapW = fWorking.GetValue(engine);
        scanner.GetType().GetField("_random", AllInst).SetValue(scanner, FastRandom(SeedScanner));
        mapA.GetType().GetField("_random", AllInst).SetValue(mapA, FastRandom(SeedActive));
        mapW.GetType().GetField("_random", AllInst).SetValue(mapW, FastRandom(SeedWorking));

        var lights = new List<float[]>();
        if (a.Lights)
        {
            int cx = a.X + a.W / 2, cy = a.Y + a.H / 2;
            lights.Add(new float[] { cx, cy, 1.0f, 0.5f, 0.2f });
            lights.Add(new float[] { cx + 10, cy + 5, 0f, 0.8f, 1.2f });
            lights.Add(new float[] { cx - 12, cy - 3, 0.3f, 0.3f, 0.3f });
            lights.Add(new float[] { a.X + a.W + 5, cy, 0.9f, 0.9f, 0.9f });   // inside the padding, outside the area
            lights.Add(new float[] { a.X - 200, cy, 0.9f, 0.9f, 0.9f });      // outside the processed area: ignored
            MethodInfo add = null;
            foreach (MethodInfo m in lighting.GetMethods(AllStatic))
            {
                ParameterInfo[] ps = m.GetParameters();
                if (m.Name == "AddLight" && ps.Length == 5 && ps[0].ParameterType == typeof(int) && ps[2].ParameterType == typeof(float)) add = m;
            }
            if (add == null) throw new Exception("Lighting.AddLight(int, int, float, float, float) not found");
            foreach (float[] l in lights) Unwrap(add, null, new object[] { (int)l[0], (int)l[1], l[2], l[3], l[4] });
        }

        object rect = Rect(a.X, a.Y, a.W, a.H);
        MethodInfo lightTiles = null;
        foreach (MethodInfo m in lighting.GetMethods(AllStatic)) if (m.Name == "LightTiles" && m.GetParameters().Length == 1) lightTiles = m;
        // Main.SceneMetrics.PerspectivePlayer is only set by Main.UpdateSceneMetrics, which needs the camera (Main.Camera
        // -> GameViewMatrix) and therefore a graphics device: it throws NullReferenceException in Camera.get_ScaledPosition
        // when reached through the ExportMetrics state. So the harness sets PerspectivePlayer to Main.player[Main.myPlayer]
        // (what Scan would have done for a single local player) and starts every cycle at the Scan state, skipping
        // MinimapUpdate (no map: Main.mapEnabled = false anyway) and ExportMetrics.
        object sceneMetrics = SF(main, "SceneMetrics");
        Array players = (Array)SF(main, "player");
        object me = players.GetValue((int)SF(main, "myPlayer"));
        FieldInfo ppField = sceneMetrics.GetType().GetField("<PerspectivePlayer>k__BackingField", AllInst);
        if (ppField == null || me == null) throw new Exception("cannot set SceneMetrics.PerspectivePlayer");
        ppField.SetValue(sceneMetrics, me);
        FieldInfo stateField = engT.GetField("_state", AllInst);
        Type stateT = stateField.FieldType;
        for (int cycle = 0; cycle < Cycles; cycle++)
        {
            stateField.SetValue(engine, Enum.ToObject(stateT, 2));   // EngineState.Scan
            Unwrap(lightTiles, null, new object[] { rect });         // Scan
            Unwrap(lightTiles, null, new object[] { rect });         // Blur + Present
        }

        MethodInfo engGet = engT.GetMethod("GetColor", AllInst | BindingFlags.Public, null, new Type[] { typeof(int), typeof(int) }, null);
        MethodInfo gGet = null;
        foreach (MethodInfo m in lighting.GetMethods(AllStatic))
        {
            ParameterInfo[] ps = m.GetParameters();
            if (m.Name == "GetColor" && ps.Length == 2 && ps[0].ParameterType == typeof(int) && ps[1].ParameterType == typeof(int)) gGet = m;
        }
        Type v3 = T("Microsoft.Xna.Framework.Vector3"), colT = T("Microsoft.Xna.Framework.Color");
        FieldInfo vx = v3.GetField("X"), vy = v3.GetField("Y"), vz = v3.GetField("Z");
        PropertyInfo packed = colT.GetProperty("PackedValue");

        using (var bw = new BinaryWriter(File.Create(Path.Combine(OutDir, a.Name + ".light.bin"))))
        {
            bw.Write(Encoding.ASCII.GetBytes("LGHT")); bw.Write(2);
            bw.Write(a.X); bw.Write(a.Y); bw.Write(a.W); bw.Write(a.H);
            bw.Write(px); bw.Write(py); bw.Write(pw); bw.Write(ph);
            bw.Write(SeedScanner); bw.Write(SeedActive); bw.Write(SeedWorking);
            bw.Write(Cycles);
            bw.Write(gameUpdateCount);
            bw.Write(tcR); bw.Write(tcG); bw.Write(tcB); bw.Write(tcA);
            bw.Write(mouseTextColor); bw.Write((byte)0); bw.Write((byte)0); bw.Write((byte)0);
            bw.Write(tfve); bw.Write(gtwh);
            bw.Write(a.WaterStyle);
            bw.Write((byte)(a.Day ? 1 : 0)); bw.Write((byte)0); bw.Write((byte)0); bw.Write((byte)0);
            bw.Write(a.Time);
            bw.Write(lights.Count);
            foreach (float[] l in lights) { bw.Write((int)l[0]); bw.Write((int)l[1]); bw.Write(l[2]); bw.Write(l[3]); bw.Write(l[4]); }
            // Main statics the engine reads and that the harness did not set: whatever the game holds after its own
            // initialisation and LoadWorld (the Rust side is checked against its initial values and then set from these).
            bw.Write((float)SF(main, "demonTorch"));
            bw.Write((int)SF(main, "DiscoR")); bw.Write((int)SF(main, "DiscoG")); bw.Write((int)SF(main, "DiscoB"));
            bw.Write((int)SF(main, "sundialCooldown")); bw.Write((int)SF(main, "moondialCooldown"));
            bw.Write((float)SF(main, "shimmerDarken"));
            bw.Write((int)SF(main, "cageFrames"));
            bw.Write((byte)((bool)SF(main, "raining") ? 1 : 0)); bw.Write((byte)0); bw.Write((byte)0); bw.Write((byte)0);
            bw.Write((int)SF(main, "myPlayer"));
            bw.Write((int)SF(main, "mapDelay"));
            float[] la = (float[])SF(main, "liquidAlpha");
            bw.Write(la.Length); foreach (float f in la) bw.Write(f);
            byte[,] jc = (byte[,])SF(main, "jellyfishCageMode");
            bw.Write(jc.GetLength(0)); bw.Write(jc.GetLength(1));
            for (int i = 0; i < jc.GetLength(0); i++) for (int j = 0; j < jc.GetLength(1); j++) bw.Write(jc[i, j]);
            object sceneState = SF(main, "SceneState");
            bw.Write((float)sceneState.GetType().GetField("airLightDecay").GetValue(sceneState));
            bw.Write((float)sceneState.GetType().GetField("solidLightDecay").GetValue(sceneState));
            long nonZero = 0;
            for (int x = px; x < px + pw; x++)
                for (int y = py; y < py + ph; y++)
                {
                    object v = Unwrap(engGet, engine, new object[] { x, y });
                    float X = (float)vx.GetValue(v), Y = (float)vy.GetValue(v), Z = (float)vz.GetValue(v);
                    object c = Unwrap(gGet, null, new object[] { x, y });
                    uint pv = (uint)packed.GetValue(c, null);
                    bw.Write(X); bw.Write(Y); bw.Write(Z); bw.Write(pv);
                    if (X != 0f || Y != 0f || Z != 0f) nonZero++;
                }
            Console.WriteLine(a.Name + ": processed " + pw + "x" + ph + ", non-black tiles " + nonZero);
            Log.Add(a.Name + " non-black tiles " + nonZero + " of " + (pw * ph));
        }
    }
}
