// RefDumpSky: runs the real game's Main.SetBackColor / UpdateAtmosphereTransparencyToSkyColor / ApplyColorOfTheSkiesToTiles
// (plain CPU code) headless, for a grid of times of day, moon phases, biome influences and cloud alpha, and writes one CSV row per case.
// Pure reflection over the reference server (work/refserver/TerrariaServer.exe), the same technique as tools/refdump/RefDump.cs (its game initialisation is
// repeated here so this file stands alone). C# 5, .NET Framework 4, x86. Headless: no window, no network, nothing written to the game folder.
//
//   RefDumpSky.exe <world.wld> <outdir>      writes <outdir>\sky_colors.csv

using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;
using System.Threading;


static class RefDumpSky
{
    static Assembly G, Xna;
    static string GameDir, OutDir, SaveDir;
    static readonly Dictionary<string, Assembly> Embedded = new Dictionary<string, Assembly>();
    const BindingFlags AllStatic = BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic;
    const BindingFlags AllInst = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;
    static readonly List<string> Log = new List<string>();

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

    [STAThread]
    static int Main(string[] args)
    {
        if (args.Length < 2) { Console.Error.WriteLine("usage: RefDumpSky.exe <world.wld> <outdir>"); return 2; }
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

    static MethodInfo FindMethod(Type t, string name, int nargs)
    {
        foreach (MethodInfo m in t.GetMethods(AllStatic))
            if (m.Name == name && m.GetParameters().Length == nargs) return m;
        throw new Exception("static method not found: " + t.FullName + "." + name);
    }

    static string Rgba(object color, Type colorT)
    {
        uint packed = (uint)colorT.GetProperty("PackedValue").GetValue(color, null);
        return (packed & 255) + "," + ((packed >> 8) & 255) + "," + ((packed >> 16) & 255) + "," + (packed >> 24);
    }

    static void Run(string root, string worldSrc)
    {
        InitGame();
        Type main = T("Terraria.Main"), worldGen = T("Terraria.WorldGen"), worldFile = T("Terraria.IO.WorldFile");
        Type colorT = T("Microsoft.Xna.Framework.Color");
        string scratchDir = Path.Combine(root, "work", "sky-scratch");
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
        int maxX = (int)SF(main, "maxTilesX");
        double worldSurface = (double)SF(main, "worldSurface");
        Console.WriteLine("world " + maxX + " x " + (int)SF(main, "maxTilesY") + " worldSurface " + worldSurface);

        Type infoT = main.GetNestedType("InfoToSetBackColor", BindingFlags.Public | BindingFlags.NonPublic);
        MethodInfo setBackColor = FindMethod(main, "SetBackColor", 3);
        MethodInfo updateAtmo = FindMethod(main, "UpdateAtmosphereTransparencyToSkyColor", 1);
        MethodInfo applyTiles = FindMethod(main, "ApplyColorOfTheSkiesToTiles", 0);

        // the state the Rust port sets: no menu, single player, no remix / eclipse / lightning, full-colour lighting
        SSet(main, "gameMenu", false);
        SSet(main, "netMode", 0);
        SSet(main, "remixWorld", false);
        SSet(main, "eclipse", false);
        SSet(main, "lightning", 0f);
        SSet(main, "eclipseLight", 0f);
        SSet(main, "dontStarveWorld", false);

        // scenarios: name, corruption, crimson, jungle, mushroom, graveyard, cloudAlpha, atmo y (in tiles)
        object[][] scenarios = new object[][] {
            new object[] { "plain", 0f, 0f, 0f, 0f, 0f, 0f, 250.0 },
            new object[] { "plain_low_atmo", 0f, 0f, 0f, 0f, 0f, 0f, 100.0 },
            new object[] { "corruption", 0.5f, 0f, 0f, 0f, 0f, 0f, 250.0 },
            new object[] { "crimson", 0f, 0.7f, 0f, 0f, 0f, 0f, 250.0 },
            new object[] { "jungle", 0f, 0f, 1.2f, 0f, 0f, 0f, 250.0 },
            new object[] { "mushroom", 0f, 0f, 0f, 0.6f, 0f, 0f, 250.0 },
            new object[] { "graveyard", 0f, 0f, 0f, 0f, 0.5f, 0f, 250.0 },
            new object[] { "cloudy", 0f, 0f, 0f, 0f, 0f, 0.5f, 250.0 },
            new object[] { "everything", 1.5f, 0.4f, 0.3f, 0.2f, 0.2f, 0.3f, 180.0 },
        };
        int[] moonPhases = new int[] { 0, 3, 4 };
        var sb = new StringBuilder();
        sb.AppendLine("scenario,dayTime,time,moonPhase,bloodMoon,cloudAlpha,atmoTileY,mushroom,corruption,crimson,jungle,graveyard,sunR,sunG,sunB,sunA,moonR,moonG,moonB,moonA,skyR,skyG,skyB,skyA,tileR,tileG,tileB,tileA,atmo,eclipseLight");
        int rows = 0;
        foreach (object[] sc in scenarios)
        {
            foreach (int moonPhase in moonPhases)
            {
                for (int phase = 0; phase < 4; phase++)
                {
                    bool day = phase < 2;
                    bool blood = phase == 3;
                    if (phase == 1) continue; // phases: 0 day, 2 night, 3 night with a blood moon
                    double max = day ? 54000.0 : 32400.0;
                    for (double t = 0; t <= max; t += 600.0)
                    {
                        SSet(main, "dayTime", day);
                        SSet(main, "time", t);
                        SSet(main, "moonPhase", moonPhase);
                        SSet(main, "bloodMoon", blood);
                        SSet(main, "cloudAlpha", (float)sc[6]);
                        SSet(main, "SmoothedMushroomLightInfluence", (float)sc[4]);
                        object info = Activator.CreateInstance(infoT);
                        infoT.GetField("isInGameMenuOrIsServer").SetValue(info, false);
                        infoT.GetField("CorruptionBiomeInfluence").SetValue(info, (float)sc[1]);
                        infoT.GetField("CrimsonBiomeInfluence").SetValue(info, (float)sc[2]);
                        infoT.GetField("JungleBiomeInfluence").SetValue(info, (float)sc[3]);
                        infoT.GetField("MushroomBiomeInfluence").SetValue(info, (float)sc[4]);
                        infoT.GetField("GraveyardInfluence").SetValue(info, (float)sc[5]);
                        infoT.GetField("BloodMoonActive").SetValue(info, blood);
                        infoT.GetField("LanternNightActive").SetValue(info, false);
                        // atmo is read by SetBackColor (cloud darkening) and by the horizon light: set it as the previous frame left it
                        SSet(main, "atmo", 1f);
                        updateAtmo.Invoke(null, new object[] { (float)((double)sc[7] * 16.0) });
                        object[] a = new object[] { info, null, null };
                        Unwrap(setBackColor, null, a);
                        // UpdateAtmosphere... runs after SetBackColor in DoDraw, then ApplyColorOfTheSkiesToTiles runs before it: DoDraw order is
                        // SetBackColor, ApplyColorOfTheSkiesToTiles, UpdateAtmosphereTransparencyToSkyColor
                        Unwrap(applyTiles, null, new object[0]);
                        updateAtmo.Invoke(null, new object[] { (float)((double)sc[7] * 16.0) });
                        sb.AppendLine(string.Join(",", new string[] {
                            (string)sc[0], day ? "1" : "0", t.ToString("R", CultureInfo.InvariantCulture), moonPhase.ToString(), blood ? "1" : "0",
                            ((float)sc[6]).ToString("R", CultureInfo.InvariantCulture), ((double)sc[7]).ToString("R", CultureInfo.InvariantCulture),
                            ((float)sc[4]).ToString("R", CultureInfo.InvariantCulture), ((float)sc[1]).ToString("R", CultureInfo.InvariantCulture),
                            ((float)sc[2]).ToString("R", CultureInfo.InvariantCulture), ((float)sc[3]).ToString("R", CultureInfo.InvariantCulture),
                            ((float)sc[5]).ToString("R", CultureInfo.InvariantCulture),
                            Rgba(a[1], colorT), Rgba(a[2], colorT), Rgba(SF(main, "ColorOfTheSkies"), colorT), Rgba(SF(main, "tileColor"), colorT),
                            ((float)SF(main, "atmo")).ToString("R", CultureInfo.InvariantCulture), ((float)SF(main, "eclipseLight")).ToString("R", CultureInfo.InvariantCulture) }));
                        rows++;
                    }
                }
            }
        }
        File.WriteAllText(Path.Combine(OutDir, "sky_colors.csv"), sb.ToString());
        Console.WriteLine("wrote " + rows + " rows");
    }
}
