// RefDumpLiquid: runs the real game's liquid simulation (Terraria.Liquid, LiquidBuffer, WorldGen.WaterCheck, WorldFile.LoadWorld's settle)
// headless over a real world file and dumps the result. Pure reflection over work\refserver\TerrariaServer.exe, the same technique as
// tools\refdump\RefDump.cs + RefDumpFramed.cs (their game initialisation is repeated here so this file stands alone). C# 5, .NET Framework 4, x86.
//
//   RefDumpLiquid.exe <world.wld> <outdir> <scenario>        scenario = settle | lake_drain | surface_fill | shimmer_drain | honey_drain | lava_open
//
// Every run: (1) the same initialisation as RefDump.InitGame (Program.SavePath, dedServ, language, new Main(), Lang, Main.rand, main.Initialize());
// (2) Liquid.cycles = 7 and Liquid.curMaxLiquid = 25000: the values Main.DoUpdate leaves in the statics during the menu frames that
// precede any world load with Main.gfxQuality = 1 (Main.cs:17289-17290; the headless harness runs no DoUpdate, and the statics' initialisers are
// cycles = 10, curMaxLiquid = 0, with which LoadWorld buffers every liquid and settles nothing); (3) a scratch copy of the world is loaded with the
// game's own WorldFile.LoadWorld(), which runs Liquid.QuickWater, WorldGen.WaterCheck and the quickSettle UpdateLiquid loop.
//
//   scenario settle:   writes <outdir>\settle.liquid.bin (the liquid state of every tile after LoadWorld, layout of refdata\framed\*.liquid.bin:
//                      8-byte records i32 x*height+y | u8 liquid | u8 bTileHeader&0x60 | u8 bTileHeader3&0x18 | u8 0, for every tile that has any
//                      of them non-zero) and <outdir>\settle.json (Liquid.numLiquid, numLiquidBuffer, wetCounter, stuckCount, ..., next Main.rand value).
//   other scenarios:  after the load: Main.netMode = 0, WorldGen.generatingWorld = false, Main.rand = new UnifiedRandom(SeedDynamics); the scenario's
//                      operations change tiles (deactivate a block = WorldGen.KillTile's effect on the tile bits without its side effects, or fill
//                      air with liquid 255); then WorldGen.TileFrame's reaction is imitated: Liquid.AddWater(x, y) for every tile with liquid > 0 in
//                      the rectangle of the operations grown by 1 (x outer, y inner, the order of WorldGen.SquareTileFrame); then Liquid.UpdateLiquid()
//                      is called 600 times (Main.DoUpdate -> WorldGen.UpdateWorld calls it every second tick with Main.SwapRandom("UpdateLiquid");
//                      here Main.rand is the seeded generator and every call is made). Before the first call and after every 60 calls the region around
//                      the operations is written to <outdir>\<scenario>.bin.
//
// <scenario>.bin (little endian): "LIQD" | i32 version=1 | i32 x0,y0,w,h (region) | i32 snapshots | u32 seed | then per snapshot:
//   i32 callsDone | i32 numLiquid | i32 numLiquidBuffer | i32 wetCounter | u64 FNV-1a of the first numLiquid Main.liquid entries (i32 x,y,kill,delay each) |
//   w*h records (x outer, y inner) of 8 bytes: u16 type | u8 liquid | u8 bTileHeader&0x60 | u8 bTileHeader3&0x18 | u16 sTileHeader | u8 0
//   and at the end i32 next Main.rand.Next() value.
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

static class RefDumpLiquid
{
    static Assembly G, Xna;
    static string GameDir, OutDir, SaveDir;
    static readonly Dictionary<string, Assembly> Embedded = new Dictionary<string, Assembly>();
    const BindingFlags AllStatic = BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic;
    const BindingFlags AllInst = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;
    static readonly List<string> Log = new List<string>();

    const int SeedDynamics = 90210;
    const int SeedSettle = 12345;
    const int Calls = 600;
    const int SnapshotEvery = 60;
    const int Margin = 20;

    // ---------------------------------------------------------------- scenarios (mirrored in port/crates/terraria-sim/tests/liquid_dynamics.rs)
    // kind 'D': deactivate tiles x0..x1, y0..y1;  kind 'F': fill air x0..x1, y0..y1 with liquid 255 of type t
    class Op { public char Kind; public int X0, X1, Y0, Y1, T; public Op(char k, int x0, int x1, int y0, int y1, int t) { Kind = k; X0 = x0; X1 = x1; Y0 = y0; Y1 = y1; T = t; } }
    class Scenario { public string Name; public Op[] Ops; public Scenario(string n, params Op[] ops) { Name = n; Ops = ops; } }
    static readonly Scenario[] Scenarios = new Scenario[]
    {
        // a pond in a bowl at x 2466..2523, y 232..261 (surface, water): a 4 x 9 shaft opens under its bottom
        new Scenario("lake_drain", new Op('D', 2472, 2475, 262, 270, 0)),
        // air above flat ground at x 2090..2095, y 222..231 (surface): a 4 x 5 block of water appears in mid air
        new Scenario("surface_fill", new Op('F', 2091, 2094, 222, 226, 0)),
        // the shimmer lake at x 190..246, y 449..463: a 4 x 6 shaft opens under its bottom
        new Scenario("shimmer_drain", new Op('D', 215, 218, 463, 468, 0)),
        // the honey pool at x 290..340, y 640..649: a 4 x 6 shaft opens under its bottom
        new Scenario("honey_drain", new Op('D', 309, 312, 650, 655, 0)),
        // the lava lake at x 2798..2896, y 1079..1195: a 3 x 5 block of its rim is removed
        new Scenario("lava_open", new Op('D', 2813, 2815, 1185, 1189, 0)),
    };

    [STAThread]
    static int Main(string[] args)
    {
        if (args.Length < 3) { Console.Error.WriteLine("usage: RefDumpLiquid.exe <world.wld> <outdir> <scenario>"); return 2; }
        string root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(root, "work", "refserver");
        SaveDir = Path.Combine(root, "work", "refdump-save");
        OutDir = Path.GetFullPath(args[1]);
        Directory.CreateDirectory(SaveDir);
        Directory.CreateDirectory(OutDir);
        var wd = new Thread(() => { Thread.Sleep(240000); Console.Error.WriteLine("WATCHDOG: timeout"); Environment.Exit(3); });
        wd.IsBackground = true; wd.Start();
        int rc = 0;
        try { Run(root, Path.GetFullPath(args[0]), args[2]); }
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
    static FieldInfo fType, fLiquid, fS, fB, fB3;
    static Array TileArr;
    static int W, H;

    static void InitTileAccess()
    {
        TileT = T("Terraria.Tile");
        fType = TileT.GetField("type"); fLiquid = TileT.GetField("liquid");
        fS = TileT.GetField("sTileHeader"); fB = TileT.GetField("bTileHeader"); fB3 = TileT.GetField("bTileHeader3");
        TileArr = (Array)SF(T("Terraria.Main"), "tile");
    }

    static void Put16(byte[] b, int o, int v) { b[o] = (byte)v; b[o + 1] = (byte)(v >> 8); }
    static void Put32(byte[] b, int o, int v) { Put16(b, o, v); Put16(b, o + 2, v >> 16); }

    static byte[] TileRecord(int x, int y)
    {
        byte[] r = new byte[8];
        object t = TileArr.GetValue(x, y);
        Put16(r, 0, (ushort)fType.GetValue(t));
        r[2] = (byte)fLiquid.GetValue(t);
        r[3] = (byte)((byte)fB.GetValue(t) & 0x60);
        r[4] = (byte)((byte)fB3.GetValue(t) & 0x18);
        Put16(r, 5, (ushort)fS.GetValue(t));
        r[7] = 0;
        return r;
    }

    static void Run(string root, string worldSrc, string scenarioName)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        Scenario sc = null;
        if (scenarioName != "settle" && scenarioName != "settle_trace")
        {
            sc = Scenarios.FirstOrDefault(s => s.Name == scenarioName);
            if (sc == null) throw new Exception("unknown scenario " + scenarioName);
        }
        InitGame();
        Type main = T("Terraria.Main"), worldGen = T("Terraria.WorldGen"), worldFile = T("Terraria.IO.WorldFile"), liquid = T("Terraria.Liquid"), liquidBuffer = T("Terraria.LiquidBuffer");

        Step("Liquid.cycles = 7; Liquid.curMaxLiquid = 25000   (Main.cs:17289-17290 with gfxQuality 1)", () => { SSet(liquid, "cycles", 7); SSet(liquid, "curMaxLiquid", 25000); });

        string scratchDir = Path.Combine(root, "work", "liquid-scratch");
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
        if (scenarioName == "settle_trace") { RunSettleTrace(root, scratch, worldSrc, true); return; }
        if (sc != null)
        {
            // the dynamics scenarios start from the settle of RunSettleTrace (fixed Main.rand), which equals the game's LoadWorld result
            // (settle_trace.liquid.bin is byte-identical to settle.liquid.bin) but does not depend on the clock-seeded generator
            RunSettleTrace(root, scratch, worldSrc, false);
        }
        else
        {
            Step("WorldFile.LoadWorld()   [WorldFile.cs:650]", () =>
            {
                CallStatic(worldFile, "LoadWorld");
                if ((bool)SF(worldGen, "loadFailed"))
                {
                    object ex = null;
                    try { ex = SF(worldFile, "LastThrownLoadException"); } catch { }
                    throw new Exception("WorldGen.loadFailed after LoadWorld: " + ex);
                }
            });
            W = (int)SF(main, "maxTilesX"); H = (int)SF(main, "maxTilesY");
            InitTileAccess();
        }
        Console.WriteLine("loaded " + W + " x " + H + " in " + sw.ElapsedMilliseconds + " ms");

        if (sc == null) { DumpSettle(liquid, liquidBuffer, main); return; }

        Step("Main.netMode = 0; WorldGen.generatingWorld = false; Main.rand = new UnifiedRandom(" + SeedDynamics + ")", () =>
        {
            SSet(main, "netMode", 0); SSet(worldGen, "generatingWorld", false);
            SSet(main, "rand", Activator.CreateInstance(T("Terraria.Utilities.UnifiedRandom"), new object[] { SeedDynamics }));
        });

        // the operations
        int minx = int.MaxValue, miny = int.MaxValue, maxx = int.MinValue, maxy = int.MinValue;
        foreach (Op op in sc.Ops)
        {
            minx = Math.Min(minx, op.X0); maxx = Math.Max(maxx, op.X1); miny = Math.Min(miny, op.Y0); maxy = Math.Max(maxy, op.Y1);
            for (int x = op.X0; x <= op.X1; x++)
                for (int y = op.Y0; y <= op.Y1; y++)
                {
                    object t = TileArr.GetValue(x, y);
                    if (op.Kind == 'D')
                        fS.SetValue(t, (ushort)((ushort)fS.GetValue(t) & 0xFFDF));            // Tile.active(false)
                    else
                    {
                        fLiquid.SetValue(t, (byte)255);
                        fB.SetValue(t, (byte)(((byte)fB.GetValue(t) & 0x9F) | (op.T << 5)));      // Tile.liquidType(op.T)
                    }
                }
        }
        // WorldGen.TileFrame's `if (tile.liquid > 0 && Main.netMode != 1 && !generatingWorld) Liquid.AddWater(i, j)` over the grown rectangle
        for (int x = minx - 1; x <= maxx + 1; x++)
            for (int y = miny - 1; y <= maxy + 1; y++)
                if ((byte)fLiquid.GetValue(TileArr.GetValue(x, y)) > 0)
                    CallStatic(liquid, "AddWater", x, y);

        int rx0 = minx - Margin, ry0 = miny - Margin, rx1 = maxx + Margin, ry1 = maxy + Margin;
        if (rx0 < 0) rx0 = 0; if (ry0 < 0) ry0 = 0; if (rx1 > W - 1) rx1 = W - 1; if (ry1 > H - 1) ry1 = H - 1;
        int rw = rx1 - rx0 + 1, rh = ry1 - ry0 + 1;
        int snapshots = Calls / SnapshotEvery + 1;
        using (var fs = new FileStream(Path.Combine(OutDir, sc.Name + ".bin"), FileMode.Create))
        {
            byte[] head = new byte[40];
            head[0] = (byte)'L'; head[1] = (byte)'I'; head[2] = (byte)'Q'; head[3] = (byte)'D';
            Put32(head, 4, 1); Put32(head, 8, rx0); Put32(head, 12, ry0); Put32(head, 16, rw); Put32(head, 20, rh); Put32(head, 24, snapshots); Put32(head, 28, SeedDynamics);
            fs.Write(head, 0, 32);
            Snapshot(fs, 0, rx0, ry0, rw, rh, liquid, liquidBuffer, main);
            for (int call = 1; call <= Calls; call++)
            {
                CallStatic(liquid, "UpdateLiquid");
                if (call % SnapshotEvery == 0) Snapshot(fs, call, rx0, ry0, rw, rh, liquid, liquidBuffer, main);
            }
            object rand = SF(main, "rand");
            int next = (int)rand.GetType().GetMethod("Next", Type.EmptyTypes).Invoke(rand, null);
            byte[] tail = new byte[4]; Put32(tail, 0, next);
            fs.Write(tail, 0, 4);
        }
        Console.WriteLine("scenario " + sc.Name + ": region x " + rx0 + ".." + rx1 + " y " + ry0 + ".." + ry1 + ", " + snapshots + " snapshots, " + sw.ElapsedMilliseconds + " ms");
    }

    // ---------------------------------------------------------------- settle_trace
    // The body of WorldFile.LoadWorld (WorldFile.cs:711-773) written out step by step, so that the state can be recorded after every UpdateLiquid call:
    // the file is read with WorldFile.LoadWorld_Version2 (no settle), then CheckSavedOreTiers / ConvertOldTileEntities / ClearTempTiles, then the settle
    // statements of WorldFile.cs:742-769, each by reflection. Writes <outdir>\settle_trace.txt: one line per phase / iteration with
    // "iter numLiquid numLiquidBuffer wetCounter stuckCount listHash regionHash(x 2460..2530, y 225..270: liquid, liquid type, flags)".
    static void RunSettleTrace(string root, string scratch, string worldSrc, bool writeOutputs)
    {
        Type main = T("Terraria.Main"), worldGen = T("Terraria.WorldGen"), worldFile = T("Terraria.IO.WorldFile"), liquid = T("Terraria.Liquid"), liquidBuffer = T("Terraria.LiquidBuffer");
        Type genVars = T("Terraria.WorldBuilding.GenVars");
        var lines = new List<string>();
        Step("read the file with WorldFile.LoadWorld_Version2 (WorldFile.cs:721)", () =>
        {
            using (var ms = new MemoryStream(File.ReadAllBytes(scratch)))
            using (var br = new BinaryReader(ms))
            {
                SSet(worldGen, "loadFailed", false);
                int version = br.ReadInt32();
                SSet(worldFile, "_versionNumber", version);
                object status = CallStatic(worldFile, "LoadWorld_Version2", br);
                Console.WriteLine("LoadWorld_Version2 status " + status);
            }
        });
        W = (int)SF(main, "maxTilesX"); H = (int)SF(main, "maxTilesY");
        InitTileAccess();
        Step("CheckSavedOreTiers; ConvertOldTileEntities; ClearTempTiles", () =>
        {
            CallStatic(worldFile, "CheckSavedOreTiers"); CallStatic(worldFile, "ConvertOldTileEntities"); CallStatic(worldFile, "ClearTempTiles");
        });
        Func<string, string> line = tag =>
        {
            ulong rh = 14695981039346656037UL;
            for (int x = 2460; x <= 2530; x++)
                for (int y = 225; y <= 270; y++)
                {
                    object t = TileArr.GetValue(x, y);
                    byte[] v = new byte[] { (byte)fLiquid.GetValue(t), (byte)((byte)fB.GetValue(t) & 0x60), (byte)((byte)fB3.GetValue(t) & 0x18) };
                    foreach (byte b in v) { rh ^= b; rh *= 1099511628211UL; }
                }
            return tag + " " + SF(liquid, "numLiquid") + " " + SF(liquidBuffer, "numLiquidBuffer") + " " + SF(liquid, "wetCounter") + " " + SF(liquid, "stuckCount") + " " + ListHash(liquid, main) + " " + rh;
        };
        // Main.rand at this point is not reproducible (the game seeds it from the clock somewhere in Initialize / the load), so it is set to a
        // fixed generator: the settle draws from it (Liquid.Update's `num == 254 && genRand.Next(30) == 0`)
        Step("Main.rand = new UnifiedRandom(" + SeedSettle + ")", () => SSet(main, "rand", Activator.CreateInstance(T("Terraria.Utilities.UnifiedRandom"), new object[] { SeedSettle })));
        Step("WorldGen.isGeneratingOrLoadingWorld = true; GenVars.waterLine = Main.maxTilesY; Liquid.QuickWater(2); WorldGen.WaterCheck()", () =>
        {
            SSet(worldGen, "isGeneratingOrLoadingWorld", true);
            SSet(genVars, "waterLine", H);
            lines.Add(line("start"));
            CallStatic(liquid, "QuickWater", 2, -1, -1);
            lines.Add(line("quickwater"));
            CallStatic(worldGen, "WaterCheck");
            lines.Add(line("watercheck"));
        });
        int n = 0;
        SSet(liquid, "quickSettle", true);
        while ((int)SF(liquid, "numLiquid") > 0 && n < 100000)
        {
            n++;
            CallStatic(liquid, "UpdateLiquid");
            lines.Add(line(n.ToString()));
            // LIQUID_TRACE_LIST=<iteration>: also write the whole Main.liquid list after that iteration
            string dumpIter = Environment.GetEnvironmentVariable("LIQUID_TRACE_LIST");
            if (dumpIter != null && dumpIter == n.ToString())
            {
                int cnt = (int)SF(liquid, "numLiquid");
                Array list = (Array)SF(main, "liquid");
                Type lt2 = list.GetType().GetElementType();
                var ls = new List<string>();
                for (int i = 0; i < cnt; i++)
                {
                    object e = list.GetValue(i);
                    ls.Add(lt2.GetField("x").GetValue(e) + " " + lt2.GetField("y").GetValue(e) + " " + lt2.GetField("kill").GetValue(e) + " " + lt2.GetField("delay").GetValue(e));
                }
                File.WriteAllLines(Path.Combine(OutDir, "settle_list_" + n + ".txt"), ls.ToArray());
            }
        }
        SSet(liquid, "quickSettle", false);
        CallStatic(worldGen, "WaterCheck");
        SSet(worldGen, "isGeneratingOrLoadingWorld", false);
        lines.Add(line("end"));
        if (!writeOutputs) return;
        File.WriteAllLines(Path.Combine(OutDir, "settle_trace.txt"), lines.ToArray());
        DumpSettle(liquid, liquidBuffer, main, "settle_trace");
        Console.WriteLine("settle_trace: " + n + " iterations");
    }

    static ulong ListHash(Type liquid, Type main)
    {
        int n = (int)SF(liquid, "numLiquid");
        Array list = (Array)SF(main, "liquid");
        Type lt = list.GetType().GetElementType();
        FieldInfo fx = lt.GetField("x"), fy = lt.GetField("y"), fk = lt.GetField("kill"), fd = lt.GetField("delay");
        ulong hash = 14695981039346656037UL;
        for (int i = 0; i < n; i++)
        {
            object e = list.GetValue(i);
            foreach (FieldInfo f in new FieldInfo[] { fx, fy, fk, fd })
            {
                int v = (int)f.GetValue(e);
                for (int b = 0; b < 4; b++) { hash ^= (byte)(v >> (8 * b)); hash *= 1099511628211UL; }
            }
        }
        return hash;
    }

    static void Snapshot(FileStream fs, int calls, int rx0, int ry0, int rw, int rh, Type liquid, Type liquidBuffer, Type main)
    {
        byte[] rec = new byte[24];
        Put32(rec, 0, calls);
        Put32(rec, 4, (int)SF(liquid, "numLiquid"));
        Put32(rec, 8, (int)SF(liquidBuffer, "numLiquidBuffer"));
        Put32(rec, 12, (int)SF(liquid, "wetCounter"));
        ulong h = ListHash(liquid, main);
        for (int b = 0; b < 8; b++) rec[16 + b] = (byte)(h >> (8 * b));
        fs.Write(rec, 0, 24);
        for (int x = rx0; x < rx0 + rw; x++)
            for (int y = ry0; y < ry0 + rh; y++)
            {
                byte[] r = TileRecord(x, y);
                fs.Write(r, 0, 8);
            }
    }

    static void DumpSettle(Type liquid, Type liquidBuffer, Type main) { DumpSettle(liquid, liquidBuffer, main, "settle"); }

    static void DumpSettle(Type liquid, Type liquidBuffer, Type main, string name)
    {
        long n = 0;
        using (var fs = new FileStream(Path.Combine(OutDir, name + ".liquid.bin"), FileMode.Create))
            for (int x = 0; x < W; x++)
                for (int y = 0; y < H; y++)
                {
                    object t = TileArr.GetValue(x, y);
                    byte liq = (byte)fLiquid.GetValue(t), lt = (byte)((byte)fB.GetValue(t) & 0x60), fl = (byte)((byte)fB3.GetValue(t) & 0x18);
                    if (liq == 0 && lt == 0 && fl == 0) continue;
                    fs.Write(BitConverter.GetBytes(x * H + y), 0, 4);
                    fs.Write(new byte[] { liq, lt, fl, 0 }, 0, 4);
                    n++;
                }
        object rand = SF(main, "rand");
        int next = (int)rand.GetType().GetMethod("Next", Type.EmptyTypes).Invoke(rand, null);
        var json = new StringBuilder("{\n");
        json.Append("\"generator\":\"tools/refdump_liquid/RefDumpLiquid.cs (settle)\",\n");
        json.Append("\"statics\":\"Liquid.cycles=7, Liquid.curMaxLiquid=25000 before LoadWorld\",\n");
        json.Append("\"records\":" + n + ",\n");
        json.Append("\"numLiquid\":" + SF(liquid, "numLiquid") + ",\"numLiquidBuffer\":" + SF(liquidBuffer, "numLiquidBuffer") + ",\n");
        json.Append("\"wetCounter\":" + SF(liquid, "wetCounter") + ",\"stuckCount\":" + SF(liquid, "stuckCount") + ",\"stuckAmount\":" + SF(liquid, "stuckAmount") + ",\"skipCount\":" + SF(liquid, "skipCount") + ",\n");
        json.Append("\"curMaxLiquid\":" + SF(liquid, "curMaxLiquid") + ",\"cycles\":" + SF(liquid, "cycles") + ",\n");
        json.Append("\"listHash\":" + ListHash(liquid, main) + ",\n");
        json.Append("\"nextRandAfterLoad\":" + next.ToString(CultureInfo.InvariantCulture) + "\n}\n");
        File.WriteAllBytes(Path.Combine(OutDir, name + ".json"), new UTF8Encoding(false).GetBytes(json.ToString()));
        Console.WriteLine(json.ToString());
    }
}
