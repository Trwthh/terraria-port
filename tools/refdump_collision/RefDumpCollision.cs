// RefDumpCollision: runs the real game's Terraria.Collision methods (TerrariaServer.exe, loaded by reflection, headless)
// on thousands of generated inputs over the real reference world and records inputs + outputs for the Rust test.
//
//   RefDumpCollision.exe [outdir]          (default: <root>\refdata\collision)
//
// Pure reflection, C# 5, .NET Framework 4, x86 (same technique as tools\refdump\RefDump.cs; the plumbing below is copied
// from there on purpose so that tool stays untouched).
//
// Sequence (every step is the game's own code):
//   1. InitGame: Program.SavePath, dedServ, language, new Main(), Lang, Main.rand, main.Initialize() (as RefDump.cs).
//   2. Copies work\refsave\Worlds\ref_small.wld to work\collision-scratch\ (the original is never opened for writing),
//      Main.ActiveWorldFileData = WorldFile.GetAllMetadata(copy), WorldFile.LoadWorld().
//   3. Stamps (Stamps.cs): a deterministic garden of tiles is written into an empty piece of sky so that every kind of tile
//      the collision code distinguishes (slopes, half bricks, platforms, actuated tiles, switches, doors, spikes, liquids...)
//      occurs. The changed tiles are written to stamps.bin; the Rust test applies the same tiles.
//   4. Snapshots Main.tile (all fields) and writes world_columns.bin (FNV-1a 64 per column, same record layout as
//      tools\refdump\RefDumpFramed.cs) so the Rust test can prove it starts from the same tiles.
//   5. Cases.cs: for every family (one Collision method / overload) generates N inputs from the seeded generator, calls the
//      method through reflection and records inputs and outputs in <family>.bin (format in Cases.cs).
//
// Output files in outdir: <family>.bin, stamps.bin, world_columns.bin, collision.json (summary).
using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Linq.Expressions;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Threading;

static partial class RefDumpCollision
{
    static Assembly G;
    static string GameDir, OutDir, SaveDir, Root;
    static readonly CultureInfo Inv = CultureInfo.InvariantCulture;
    static readonly Dictionary<string, Assembly> Embedded = new Dictionary<string, Assembly>();
    const BindingFlags AllStatic = BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic;
    const BindingFlags AllInst = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;

    [STAThread]
    static int Main(string[] args)
    {
        Root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(Root, "work", "refserver");
        SaveDir = Path.Combine(Root, "work", "refdump-save");
        OutDir = args.Length > 0 ? Path.GetFullPath(args[0]) : Path.Combine(Root, "refdata", "collision");
        Directory.CreateDirectory(SaveDir);
        Directory.CreateDirectory(OutDir);

        var wd = new Thread(() => { Thread.Sleep(560000); Console.Error.WriteLine("WATCHDOG: timeout"); Environment.Exit(3); });
        wd.IsBackground = true; wd.Start();

        int rc = 0;
        try { Run(); }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }

    // ---------------------------------------------------------------- plumbing (copied from tools\refdump\RefDump.cs)
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
    static object CallStatic(Type t, string name, params object[] args)
    {
        foreach (MethodInfo m in t.GetMethods(AllStatic))
            if (m.Name == name && m.GetParameters().Length == args.Length) return Unwrap(m, null, args);
        throw new Exception("static method not found: " + t.FullName + "." + name);
    }
    static object Unwrap(MethodInfo m, object o, object[] args)
    {
        try { return m.Invoke(o, args); }
        catch (TargetInvocationException e) { throw new Exception(m.DeclaringType.FullName + "." + m.Name + " threw " + e.InnerException.GetType().Name + ": " + e.InnerException.Message, e.InnerException); }
    }

    static object MainInst;
    static void Step(string desc, Action a)
    {
        try { a(); Console.WriteLine("ok   " + desc); }
        catch (Exception e) { Console.WriteLine("FAIL " + desc + " -> " + e.GetType().Name + ": " + e.Message); throw; }
    }

    static void InitGame()
    {
        AppDomain.CurrentDomain.AssemblyResolve += OnResolve;
        Environment.CurrentDirectory = GameDir;
        Environment.SetEnvironmentVariable("PATH", GameDir + ";" + Environment.GetEnvironmentVariable("PATH"));
        G = Assembly.LoadFrom(Path.Combine(GameDir, "TerrariaServer.exe"));

        Type program = T("Terraria.Program"), main = T("Terraria.Main");
        Step("Program.SavePath = work\\refdump-save   [Program.cs:187]", () => SSet(program, "SavePath", SaveDir));
        Step("Main.dedServ = true; Main.showSplash = false   [Main.cs:5392-5393]", () => { SSet(main, "dedServ", true); SSet(main, "showSplash", false); });
        Step("LanguageManager.Instance.SetLanguage(GameCulture.DefaultCulture)   [Program.cs:197]", () =>
        {
            object lm = SF(T("Terraria.Localization.LanguageManager"), "Instance");
            object gc = SF(T("Terraria.Localization.GameCulture"), "DefaultCulture");
            MethodInfo sl = lm.GetType().GetMethods().First(m => m.Name == "SetLanguage" && m.GetParameters().Length == 1 && m.GetParameters()[0].ParameterType == gc.GetType());
            Unwrap(sl, lm, new object[] { gc });
        });
        Step("main = new Main()   [Program.cs:219]", () => MainInst = Activator.CreateInstance(main));
        Step("Lang.InitializeLegacyLocalization()   [Program.cs:223]", () => CallStatic(T("Terraria.Lang"), "InitializeLegacyLocalization"));
        Step("Main.rand = new UnifiedRandom(0)   [Main.cs:5375, seeded]", () =>
        {
            Type ur = T("Terraria.Utilities.UnifiedRandom");
            SSet(main, "rand", Activator.CreateInstance(ur, new object[] { 0 }));
        });
        Step("main.Initialize()   [Main.cs:5394]", () =>
        {
            MethodInfo init = main.GetMethod("Initialize", AllInst, null, Type.EmptyTypes, null);
            Unwrap(init, MainInst, new object[0]);
        });
    }

    static string Hex(byte[] b) { var sb = new StringBuilder(); foreach (byte x in b) sb.Append(x.ToString("x2")); return sb.ToString(); }
    static string Q(string s)
    {
        var sb = new StringBuilder("\"");
        foreach (char c in s) { if (c == '"' || c == '\\') sb.Append('\\'); sb.Append(c); }
        return sb.Append('"').ToString();
    }

    // ---------------------------------------------------------------- tile access (compiled field readers, as RefDumpFramed.cs)
    const int RecordSize = 14;
    static Type TileT, MainT, CollisionT;
    static int W, H;
    static Array TileArr;
    static Func<Array, int, int, ushort> RdType, RdWall, RdS;
    static Func<Array, int, int, byte> RdLiquid, RdB, RdB2, RdB3;
    static Func<Array, int, int, short> RdFx, RdFy;

    static void MakeReaders()
    {
        Type arrT = TileT.MakeArrayType(2);
        ParameterExpression pa = Expression.Parameter(typeof(Array), "a"), px = Expression.Parameter(typeof(int), "x"), py = Expression.Parameter(typeof(int), "y");
        Expression tile = Expression.ArrayAccess(Expression.Convert(pa, arrT), px, py);
        RdType = Expression.Lambda<Func<Array, int, int, ushort>>(Expression.Field(tile, "type"), pa, px, py).Compile();
        RdWall = Expression.Lambda<Func<Array, int, int, ushort>>(Expression.Field(tile, "wall"), pa, px, py).Compile();
        RdS = Expression.Lambda<Func<Array, int, int, ushort>>(Expression.Field(tile, "sTileHeader"), pa, px, py).Compile();
        RdLiquid = Expression.Lambda<Func<Array, int, int, byte>>(Expression.Field(tile, "liquid"), pa, px, py).Compile();
        RdB = Expression.Lambda<Func<Array, int, int, byte>>(Expression.Field(tile, "bTileHeader"), pa, px, py).Compile();
        RdB2 = Expression.Lambda<Func<Array, int, int, byte>>(Expression.Field(tile, "bTileHeader2"), pa, px, py).Compile();
        RdB3 = Expression.Lambda<Func<Array, int, int, byte>>(Expression.Field(tile, "bTileHeader3"), pa, px, py).Compile();
        RdFx = Expression.Lambda<Func<Array, int, int, short>>(Expression.Field(tile, "frameX"), pa, px, py).Compile();
        RdFy = Expression.Lambda<Func<Array, int, int, short>>(Expression.Field(tile, "frameY"), pa, px, py).Compile();
    }

    static void Put16(byte[] b, int o, int v) { b[o] = (byte)v; b[o + 1] = (byte)(v >> 8); }
    static void Put32(byte[] b, int o, int v) { Put16(b, o, v); Put16(b, o + 2, v >> 16); }

    static void ReadTile(int x, int y, byte[] rec, int o)
    {
        Put16(rec, o, RdType(TileArr, x, y));
        Put16(rec, o + 2, RdWall(TileArr, x, y));
        rec[o + 4] = RdLiquid(TileArr, x, y);
        Put16(rec, o + 5, RdS(TileArr, x, y));
        rec[o + 7] = RdB(TileArr, x, y);
        rec[o + 8] = RdB2(TileArr, x, y);
        rec[o + 9] = RdB3(TileArr, x, y);
        Put16(rec, o + 10, RdFx(TileArr, x, y));
        Put16(rec, o + 12, RdFy(TileArr, x, y));
    }

    // FNV-1a 64 over the 14-byte records of each column
    static ulong[] ColumnHashes()
    {
        var res = new ulong[W];
        byte[] rec = new byte[RecordSize];
        for (int x = 0; x < W; x++)
        {
            ulong hash = 14695981039346656037UL;
            for (int y = 0; y < H; y++)
            {
                ReadTile(x, y, rec, 0);
                for (int k = 0; k < RecordSize; k++) { hash ^= rec[k]; hash *= 1099511628211UL; }
            }
            res[x] = hash;
        }
        return res;
    }

    static void Run()
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        InitGame();
        MainT = T("Terraria.Main"); TileT = T("Terraria.Tile"); CollisionT = T("Terraria.Collision");
        Type worldGen = T("Terraria.WorldGen"), worldFile = T("Terraria.IO.WorldFile");

        string worldSrc = Path.Combine(Root, "work", "refsave", "Worlds", "ref_small.wld");
        string scratchDir = Path.Combine(Root, "work", "collision-scratch");
        Directory.CreateDirectory(scratchDir);
        string scratch = Path.Combine(scratchDir, "ref_small.wld");
        File.Copy(worldSrc, scratch, true);
        string srcSha = Hex(SHA256.Create().ComputeHash(File.ReadAllBytes(scratch)));
        Step("Main.ActiveWorldFileData = WorldFile.GetAllMetadata(scratch copy, false)", () =>
        {
            object wfd = null;
            foreach (MethodInfo m in worldFile.GetMethods(AllStatic))
                if (m.Name == "GetAllMetadata" && m.GetParameters().Length == 2) wfd = Unwrap(m, null, new object[] { scratch, false });
            if (wfd == null) throw new Exception("GetAllMetadata returned null");
            SSet(MainT, "ActiveWorldFileData", wfd);
        });
        Step("WorldFile.LoadWorld()", () =>
        {
            CallStatic(worldFile, "LoadWorld");
            if ((bool)SF(worldGen, "loadFailed")) throw new Exception("WorldGen.loadFailed after LoadWorld");
        });
        W = (int)SF(MainT, "maxTilesX"); H = (int)SF(MainT, "maxTilesY");
        TileArr = (Array)SF(MainT, "tile");
        MakeReaders();
        Console.WriteLine("loaded " + W + " x " + H + " in " + sw.ElapsedMilliseconds + " ms");

        Stamps(out stampList);
        Console.WriteLine("stamped " + stampList.Count + " tiles at block " + StampX + "," + StampY);
        ulong[] cols = ColumnHashes();
        using (var fs = new FileStream(Path.Combine(OutDir, "world_columns.bin"), FileMode.Create))
            foreach (ulong c in cols) fs.Write(BitConverter.GetBytes(c), 0, 8);

        if (Environment.GetEnvironmentVariable("REFDUMP_PROBE") == "1")
        {
            Type wiring = T("Terraria.Wiring");
            foreach (int ty in new int[] { 135, 210, 443, 442 })
            {
                foreach (StampRec sr in stampList.FindAll(s => ((s.Rec[0] | (s.Rec[1] << 8)) == ty) && ((s.Rec[5] & 0x20) != 0)))
                {
                Console.WriteLine("probe: HitSwitch type " + ty + " at " + sr.X + "," + sr.Y); Console.Out.Flush();
                try { CallStatic(wiring, "HitSwitch", sr.X, sr.Y); Console.WriteLine("probe: ok"); } catch (Exception e) { Console.WriteLine("probe: threw " + e.Message); }
                Console.Out.Flush();
                }
            }
            return;
        }
        BuildHotspots();
        RunAllFamilies(srcSha);
        Console.WriteLine("done in " + sw.ElapsedMilliseconds + " ms");
    }
}
