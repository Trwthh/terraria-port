// RefDumpWorldgenX "genx" mode: the real game generates a world from ARGUMENTS (seed text, size, game mode, evil, special-seed
// flags) and dumps one record per generation pass, in the row format of Gen.cs (refdata\worldgen\README.md section 3).
//
//   RefDumpWorldgenX.exe genx <outdir> <tag> <seedtext> <small|medium|large> <classic|expert|master|journey>
//                             <random|corruption|crimson> <flags|-> <passes|real> <snaps> [watchdog seconds]
//
//   <tag>     world name and file-name prefix: <outdir>\<tag>.passes.jsonl, .columns.bin, .summary.json, .wld, .<snapshot>.tfrm.z
//   <flags>   "-" or a comma list of server-config seed flags (AWorldGenerationOption.ServerConfigName): notthebees, drunk,
//             celebration, theconstant, fortheworthy, notraps, remix, zenith, skyblock; and secret seeds (1.4.5) as
//             "secret.<name>" (<name> as in the localization key SecretSeedDescription.<name>, e.g. secret.errorWorld):
//             WorldGen.SecretSeed.Enable(seed, false), the call the server console makes for an entered secret seed
//   <snaps>   none | default (rows before pass 0, passes 0, 1, every 10th, the last, after GenerateWorld) | a comma list of pass
//             indices (-1 = before pass 0, the pass count = after GenerateWorld)
//
// This is a separate program from RefDumpWorldgen.exe (Program.cs / Gen.cs are not compiled into it and were not changed): the
// files of the "12345" / small scenario stay as they are. Shared code: Common.cs (game loading, hashing, the JSON walker).
//
// What the arguments stand for in the real dedicated server (decompile\Terraria\Terraria\...):
//   size      `-autocreate 1|2|3`                 -> Main.autoCreate(string)                    [Main.cs:5055-5078; LaunchInitializer.cs:160-163]
//   tag       `-worldname <tag>`, `-world <path>` -> Main.SetWorldName; new WorldFileData(path, false).SetAsActive();
//                                                    Main.autoGenFileLocation = path           [LaunchInitializer.cs:127-130,173-183; Main.cs:5313-5316]
//   mode      server config `difficulty=0..3`     -> Main.GameMode = n (a property: it writes Main.ActiveWorldFileData.GameMode, the
//                                                    WorldFileData of `-world`)                 [Main.cs:5241-5244,2785-2802; LoadDedConfig runs after -world: LaunchInitializer.cs:198-201]
//   flags     server config `seed_<name>=1`       -> WorldGenerationOptions.TryEnablingFlagFrom(line): option.AutoGenEnabled = true
//                                                    [Main.cs:5275; WorldGenerationOptions.cs:92-110]; LoadWorld turns AutoGenEnabled
//                                                    into Enabled                              [WorldFile.cs:690-696]
//   seedtext  `-seed <text>`                      -> Main.AutogenSeedName                       [LaunchInitializer.cs:203-206]
//   evil      NO server parameter exists. The client's world creation page writes WorldGen.WorldGenParam_Evil = -1 | 0 | 1
//             (UIWorldCreation.cs:133-143); the harness writes the same static. -1 (random) is the server's value.
//
// Why special seeds are given as FLAGS and not as their seed text ("not the bees", "5162020"): for a seed text that names an
// option the game does NOT use the text as the seed. WorldFile.LoadWorld (WorldFile.cs:697-700) and UIWorldCreation
// (.cs:984-987) both call SetSeedToRandomWithCurrentEvents(): `new UnifiedRandom().Next()` (tick seeded), so such a world cannot
// be reproduced from its text. The server-config flag selects the same option (Enabled = true) and keeps the `-seed` text as the
// seed, through the game's own code. This mode refuses a seed text that maps to an option or is empty, for that reason.
//
// mode "passes": the autogen block of WorldFile.LoadWorld (WorldFile.cs:679-709) replayed call by call, with a
//                WorldGenerator.Controller whose OnPassesLoaded wraps every pass (as Gen.cs). mode "real": the server's own
//                WorldGen.serverLoadWorldCallBack(), no instrumentation; only <tag>.wld and the summary are written.
// Generation runs on a thread-pool thread, as in the server. No socket, no window, no console input; nothing is written below
// work\refserver or the game folder: the save root is work\worldgen-scratch\<tag>\save, the world work\worldgen-scratch\<tag>\Worlds.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Linq.Expressions;
using System.Reflection;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

static partial class WG
{
    static string XTag, XSeedText, XOut;
    static StreamWriter XPassLog;
    static BinaryWriter XColLog;
    static bool XSnapDefault;
    static HashSet<int> XSnapSet = new HashSet<int>();
    static int XPassTotal, XRowCount;
    static List<string> XPassNames = new List<string>(), XPassTypes = new List<string>(), XHookErrors = new List<string>(), XSnapFiles = new List<string>();
    static List<bool> XPassEnabled = new List<bool>();
    static List<int> XPassNextRand = new List<int>();
    static readonly List<string> XTraceLines = new List<string>();
    static FieldInfo[] XWorldGenPrimStatics;
    static List<KeyValuePair<string, FieldInfo>> XMainSel;
    static Type XGenVars;
    static long XPeakWorkingSet;

    class XCapture : TraceListener
    {
        public override void Write(string m) { lock (XTraceLines) XTraceLines.Add(m); }
        public override void WriteLine(string m) { lock (XTraceLines) XTraceLines.Add(m); }
    }

    static readonly string[] XSizes = { "small", "medium", "large" };
    static readonly string[] XModes = { "classic", "expert", "master", "journey" };
    static readonly string[] XEvils = { "random", "corruption", "crimson" };

    static int MainGenX(string[] args)
    {
        if (args.Length < 10) { Console.Error.WriteLine("usage: RefDumpWorldgenX.exe genx <outdir> <tag> <seedtext> <small|medium|large> <classic|expert|master|journey> <random|corruption|crimson> <flags|-> <passes|real> <none|default|i,j,..> [watchdog seconds]"); return 2; }
        string outdir = Path.GetFullPath(args[1]);
        XTag = args[2]; XSeedText = args[3];
        int size = Array.IndexOf(XSizes, args[4]), gameMode = Array.IndexOf(XModes, args[5]), evil = Array.IndexOf(XEvils, args[6]) - 1;
        if (size < 0 || gameMode < 0 || evil < -1) { Console.Error.WriteLine("unknown size / game mode / evil"); return 2; }
        string[] flags = args[7] == "-" ? new string[0] : args[7].Split(',');
        string mode = args[8];
        if (mode != "passes" && mode != "real") { Console.Error.WriteLine("mode is passes or real"); return 2; }
        if (args[9] == "default") XSnapDefault = true;
        else if (args[9] != "none") foreach (string s in args[9].Split(',')) XSnapSet.Add(int.Parse(s, Inv));
        int watchdog = args.Length > 10 ? int.Parse(args[10], Inv) : 1800;
        if (XTag.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0 || XTag.Length == 0) { Console.Error.WriteLine("the tag must be a file name"); return 2; }

        Root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(Root, "work", "refserver");
        string scratch = Dir(Path.Combine(Root, "work", "worldgen-scratch", XTag));
        SaveDir = Dir(Path.Combine(scratch, "save"));
        XOut = Dir(outdir);
        // never below the private server copy or the game folder
        foreach (string p in new[] { XOut, scratch })
            if (Path.GetFullPath(p).StartsWith(Path.GetFullPath(GameDir), StringComparison.OrdinalIgnoreCase) || p.IndexOf("\\Documents\\Games\\", StringComparison.OrdinalIgnoreCase) >= 0)
            { Console.Error.WriteLine("refusing to write below the game folder: " + p); return 2; }
        string wldPath = Path.Combine(Dir(Path.Combine(scratch, "Worlds")), XTag + ".wld");
        if (File.Exists(wldPath)) File.Delete(wldPath);
        Watchdog(watchdog);
        Trace.Listeners.Add(new XCapture());
        var mem = new Thread(() => { for (;;) { try { long ws = Process.GetCurrentProcess().WorkingSet64; if (ws > XPeakWorkingSet) XPeakWorkingSet = ws; } catch { } Thread.Sleep(500); } });
        mem.IsBackground = true; mem.Start();
        int rc = 0;
        try { RunGenX(mode, size, gameMode, evil, flags, wldPath); }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }

    static void RunGenX(string mode, int size, int gameMode, int evil, string[] flags, string wldPath)
    {
        var total = Stopwatch.StartNew();
        Type opts = null;
        InitGame(() =>
        {
            opts = T("Terraria.WorldBuilding.WorldGenerationOptions");
            Step("main.autoCreate(\"" + (size + 1) + "\")   [Main.cs:5055-5078; LaunchInitializer.cs:160-163]", () => TMain.GetMethod("autoCreate", AllInst).Invoke(MainInst, new object[] { (size + 1).ToString(Inv) }));
            Step("main.SetWorldName(\"" + XTag + "\")   [LaunchInitializer.cs:127-130; Main.cs:5313-5316]", () => TMain.GetMethod("SetWorldName", AllInst).Invoke(MainInst, new object[] { XTag }));
            Step("new WorldFileData(path, false).SetAsActive(); Main.autoGenFileLocation = path   [LaunchInitializer.cs:173-183]", () =>
            {
                object wfd = Activator.CreateInstance(T("Terraria.IO.WorldFileData"), new object[] { wldPath, false });
                wfd.GetType().GetMethod("SetAsActive").Invoke(wfd, null);
                SSet(TMain, "autoGenFileLocation", wldPath);
            });
            // server config (LoadDedConfig runs after -world: LaunchInitializer.cs:198-201)
            Step("Main.GameMode = " + gameMode + "   [server config difficulty=: Main.cs:5241-5244 and the switch after the loop; property Main.cs:2785-2802]", () => TMain.GetProperty("GameMode", AllStatic).SetValue(null, gameMode, null));
            foreach (string f in flags)
            {
                if (f.StartsWith("secret.", StringComparison.Ordinal))
                {
                    // a secret seed (WorldGen.SecretSeed, 1.4.5): the dedicated server's console takes its text and calls
                    // SecretSeed.Enable(seed) [Main.cs:5649-5651]; the texts are stored as hashes only (SecretSeed.Check), so
                    // the seed is named by its localization key "SecretSeedDescription.<name>" and enabled through the same
                    // call, without its sound (a server plays none)
                    string key = "SecretSeedDescription." + f.Substring(7);
                    Step("WorldGen.SecretSeed.Enable(<" + key + ">, playSound: false)   [Main.cs:5649-5651; WorldGen.cs:520-533]", () =>
                    {
                        Type ss = T("Terraria.WorldGen+SecretSeed");
                        object seed = null;
                        foreach (object s in (IEnumerable)SF(ss, "AllSecretSeeds"))
                            if ((string)s.GetType().GetField("Localization").GetValue(s) == key) seed = s;
                        if (seed == null) throw new Exception("no secret seed has the localization key " + key);
                        ss.GetMethod("Enable", AllStatic).Invoke(null, new object[] { seed, false });
                        if (!(bool)XProp(seed, "Enabled")) throw new Exception(key + " is not enabled");
                    });
                    continue;
                }
                string line = "seed_" + f + "=1";
                Step("WorldGenerationOptions.TryEnablingFlagFrom(\"" + line + "\")   [Main.cs:5275; WorldGenerationOptions.cs:92-110]", () =>
                {
                    CallStatic(opts, "TryEnablingFlagFrom", line);
                    bool found = false;
                    foreach (object o in (IEnumerable)XProp(opts, "Options"))
                        if ((string)XProp(o, "ServerConfigName") == f) found = (bool)GetMX(o, "AutoGenEnabled");
                    if (!found) throw new Exception("no world generation option has the server config name " + f);
                });
            }
            Step("Main.AutogenSeedName = " + Q(XSeedText) + "   [LaunchInitializer.cs:203-206]", () => SSet(TMain, "AutogenSeedName", XSeedText));
            // no server parameter: the static the client's world creation page writes (UIWorldCreation.cs:133-143)
            Step("WorldGen.WorldGenParam_Evil = " + evil + "   [UIWorldCreation.cs:141; -1 is the default]", () => SSet(TWorldGen, "WorldGenParam_Evil", evil));
        }, false);
        Console.WriteLine("state: maxTilesX=" + SF(TMain, "maxTilesX") + " maxTilesY=" + SF(TMain, "maxTilesY") + " netMode=" + SF(TMain, "netMode") +
            " GameMode=" + XProp(TMain, "GameMode") + " WorldGenParam_Evil=" + SF(TWorldGen, "WorldGenParam_Evil") + " IntPtr.Size=" + IntPtr.Size + " worldPathName=" + SF(TMain, "worldPathName"));

        if (mode == "passes")
        {
            XPassLog = new StreamWriter(Path.Combine(XOut, XTag + ".passes.jsonl"), false, new UTF8Encoding(false)) { NewLine = "\n" };
            XColLog = new BinaryWriter(new FileStream(Path.Combine(XOut, XTag + ".columns.bin"), FileMode.Create));
        }
        Exception err = null;
        long genMs = 0;
        Task tk = Task.Factory.StartNew(() =>
        {
            try
            {
                var sw = Stopwatch.StartNew();
                if (mode == "passes") RunPassesX(opts);
                else CallStatic(TWorldGen, "serverLoadWorldCallBack");
                genMs = sw.ElapsedMilliseconds;
            }
            catch (Exception e) { err = e; }
        });
        tk.Wait();
        if (err != null) throw new Exception("generation failed: " + err.Message, err);
        if (XHookErrors.Count > 0) throw new Exception("hook errors: " + XHookErrors.Count + ": " + XHookErrors[0]);
        if (XPassLog != null) XPassLog.Close();
        if (XColLog != null) XColLog.Close();

        byte[] wld = File.ReadAllBytes(wldPath);
        File.WriteAllBytes(Path.Combine(XOut, XTag + ".wld"), wld);
        object ad = SF(TMain, "ActiveWorldFileData");
        var sj = new StringBuilder("{\n");
        sj.Append("\"generator\":\"tools/refdump_worldgen (RefDumpWorldgenX.exe genx)\",\n");
        sj.Append("\"tag\":" + Q(XTag) + ",\"mode\":" + Q(mode) + ",\"thread\":\"task\",\n");
        sj.Append("\"seed_text\":" + Q(XSeedText) + ",\"world_name\":" + Q(XTag) + ",\"size\":" + Q(XSizes[size]) + ",\"game_mode_name\":" + Q(XModes[gameMode]) + ",\"evil\":" + Q(XEvils[evil + 1]) + ",\n");
        sj.Append("\"flags\":[" + string.Join(",", flags.Select(Q).ToArray()) + "],\n");
        sj.Append("\"file_seed_text\":" + Q((string)XProp(ad, "SeedText")) + ",\"seed_int\":" + XProp(ad, "Seed") + ",\"world_id\":" + GetMX(ad, "WorldId") + ",\n");
        sj.Append("\"max_tiles_x\":" + SF(TMain, "maxTilesX") + ",\"max_tiles_y\":" + SF(TMain, "maxTilesY") + ",\"game_mode\":" + XProp(TMain, "GameMode") + ",\n");
        sj.Append("\"worldgen_param_evil\":" + SF(TWorldGen, "WorldGenParam_Evil") + ",\"crimson\":" + ((bool)SF(TWorldGen, "crimson") ? "true" : "false") + ",\n");
        var en = new List<string>();
        foreach (object o in (IEnumerable)XProp(opts, "Options")) if ((bool)XProp(o, "Enabled")) en.Add(o.GetType().Name);
        sj.Append("\"options_enabled_after_generation\":[" + string.Join(",", en.Select(Q).ToArray()) + "],\n");
        var secrets = new List<string>();
        foreach (object s in (IEnumerable)SF(T("Terraria.WorldGen+SecretSeed"), "AllSecretSeeds")) if ((bool)XProp(s, "Enabled")) secrets.Add((string)s.GetType().GetField("Localization").GetValue(s));
        sj.Append("\"secret_seeds_enabled_after_generation\":[" + string.Join(",", secrets.Select(Q).ToArray()) + "],\n");
        sj.Append("\"world_flags\":{");
        string[] wf = { "drunkWorld", "getGoodWorld", "notTheBeesWorld", "noTrapsWorld", "tenthAnniversaryWorld", "dontStarveWorld", "remixWorld", "zenithWorld", "skyblockWorld" };
        for (int i = 0; i < wf.Length; i++) { FieldInfo f = TMain.GetField(wf[i], AllStatic); sj.Append((i > 0 ? "," : "") + Q(wf[i]) + ":" + (f == null ? "null" : ((bool)f.GetValue(null) ? "true" : "false"))); }
        sj.Append("},\n");
        sj.Append("\"intptr_size\":" + IntPtr.Size + ",\"run_date\":" + Q(DateTime.Now.ToString("yyyy-MM-dd")) + ",\"main_halloween\":" + ((bool)SF(TMain, "halloween") ? "true" : "false") + ",\"main_xmas\":" + ((bool)SF(TMain, "xMas") ? "true" : "false") + ",\n");
        sj.Append("\"world_file\":" + Q(XTag + ".wld") + ",\"world_file_size\":" + wld.Length + ",\"world_file_sha256\":" + Q(Sha256(wld)) + ",\n");
        sj.Append("\"pass_count\":" + XPassTotal + ",\"rows\":" + XRowCount + ",\n");
        sj.Append("\"pass_names\":[" + string.Join(",", XPassNames.Select(Q).ToArray()) + "],\n");
        sj.Append("\"pass_classes\":[" + string.Join(",", XPassTypes.Select(Q).ToArray()) + "],\n");
        sj.Append("\"pass_enabled\":[" + string.Join(",", XPassEnabled.Select(b => b ? "true" : "false").ToArray()) + "],\n");
        sj.Append("\"snapshots\":[" + string.Join(",", XSnapFiles.Select(Q).ToArray()) + "],\n");
        var manifest = new StringBuilder();
        int mism = 0, mcount = 0;
        if (mode == "passes")
        {
            IList results = (IList)GetMX(SF(TWorldGen, "Manifest"), "GenPassResults");
            mcount = results.Count;
            int ran = 0;
            for (int i = 0; i < results.Count; i++)
            {
                object r = results[i];
                int rn = (int)XProp(r, "RandNext");
                bool skipped = (bool)XProp(r, "Skipped");
                if (!skipped) { if (ran < XPassNextRand.Count && rn != XPassNextRand[ran]) mism++; ran++; }
                object hash = XProp(r, "Hash");
                manifest.Append((i > 0 ? "," : "") + "{\"name\":" + Q((string)XProp(r, "Name")) + ",\"skipped\":" + (skipped ? "true" : "false") + ",\"randNext\":" + rn + ",\"game_hash\":" + (hash == null ? "null" : Convert.ToString(hash, Inv)) + "}");
            }
        }
        sj.Append("\"manifest_pass_results\":" + mcount + ",\"manifest_randnext_mismatches_vs_harness_preview\":" + mism + ",\n");
        sj.Append("\"manifest\":[" + manifest + "],\n");
        sj.Append("\"trace_lines\":[" + string.Join(",", XTraceLines.Where(l => l != null && l.Trim().Length > 0).Take(400).Select(Q).ToArray()) + "],\n");
        sj.Append("\"generation_ms\":" + genMs + ",\"peak_working_set_mb\":" + (XPeakWorkingSet >> 20) + ",\"total_ms\":" + total.ElapsedMilliseconds + "\n}\n");
        File.WriteAllBytes(Path.Combine(XOut, XTag + ".summary.json"), new UTF8Encoding(false).GetBytes(sj.ToString()));
        Console.WriteLine("world file " + wld.Length + " bytes sha256 " + Sha256(wld));
        Console.WriteLine("generation " + genMs + " ms, peak working set " + (XPeakWorkingSet >> 20) + " MB, manifest RandNext mismatches vs preview: " + mism);
        if (mism != 0) throw new Exception("the game's manifest and the harness's rows disagree on RandNext");
    }

    static object XProp(object o, string n)
    {
        Type t = o as Type ?? o.GetType();
        object target = o is Type ? null : o;
        for (Type c = t; c != null; c = c.BaseType)
        {
            PropertyInfo p = c.GetProperty(n, BindingFlags.Instance | BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.DeclaredOnly);
            if (p != null) return p.GetValue(target, null);
        }
        throw new Exception("property not found: " + t.FullName + "." + n);
    }
    static object GetMX(object o, string n)
    {
        for (Type c = o.GetType(); c != null; c = c.BaseType)
        {
            FieldInfo f = c.GetField(n, AllInst | BindingFlags.DeclaredOnly); if (f != null) return f.GetValue(o);
        }
        return XProp(o, n);
    }

    // WorldFile.cs:679-709 (the autogen block of LoadWorld) with a controller. Every call is the game's own.
    static void RunPassesX(Type opts)
    {
        Type wfd = T("Terraria.IO.WorldFileData");
        // WorldFile.cs:679
        string wn = (string)SF(TMain, "worldName");
        object md = CallStatic(TWorldFile, "CreateMetadata", wn == "" ? "World" : wn, false, (int)XProp(TMain, "GameMode"));
        SSet(TMain, "ActiveWorldFileData", md);
        // WorldFile.cs:680-705
        string text = ((string)SF(TMain, "AutogenSeedName") ?? "").Trim();
        object[] ta = new object[] { text, true, null, null, null };
        MethodInfo tryApply = wfd.GetMethod("TryApplyingCopiedSeed", BindingFlags.Public | BindingFlags.Static);
        if ((bool)Unwrap(tryApply, null, ta))
        {
            // WorldFile.cs:683-685: a copied seed sets size / mode / evil / options itself
            md.GetType().GetMethod("SetSeed").Invoke(md, new object[] { ta[3] });
            Console.WriteLine("seed text is a copied seed: size, game mode, evil and options come from it");
        }
        else
        {
            text = (string)CallStatic(T("Terraria.Utils"), "TrimUserString", text, (int)SF(wfd, "MAX_USER_SEED_TEXT_LENGTH"));
            object opt = CallStatic(opts, "GetOptionFromSeedText", text);
            CallStatic(opts, "SelectOption", opt);
            foreach (object o in (IEnumerable)XProp(opts, "Options"))
                if ((bool)GetMX(o, "AutoGenEnabled")) o.GetType().GetProperty("Enabled").SetValue(o, true, null);
            if (opt != null || string.IsNullOrWhiteSpace(text))
                throw new Exception("the seed text names a special-seed option (" + (opt == null ? "empty" : opt.GetType().Name) + "): the game would draw a random seed here (WorldFile.cs:697-700). Give the option as a flag and a plain seed text.");
            md.GetType().GetMethod("SetSeed").Invoke(md, new object[] { text });
        }
        Console.WriteLine("ActiveWorldFileData: Seed=" + XProp(md, "Seed") + " SeedText=" + XProp(md, "SeedText") + " GameMode=" + XProp(TMain, "GameMode") + " maxTilesX=" + SF(TMain, "maxTilesX"));
        // WorldFile.cs:706-709, with the controller
        Type ctrlT = T("Terraria.WorldBuilding.WorldGenerator+Controller");
        object ctrl = Activator.CreateInstance(ctrlT, new object[] { null });
        Type actT = typeof(Action<>).MakeGenericType(ctrlT);
        ParameterExpression pc = Expression.Parameter(ctrlT, "c");
        Delegate cb = Expression.Lambda(actT, Expression.Call(typeof(WG).GetMethod("OnPassesLoadedX", BindingFlags.Static | BindingFlags.NonPublic), Expression.Convert(pc, typeof(object))), pc).Compile();
        ctrlT.GetField("OnPassesLoaded").SetValue(ctrl, cb);
        MethodInfo gw = TWorldGen.GetMethods(BindingFlags.Public | BindingFlags.Static).First(m => m.Name == "GenerateWorld");
        var clock = Stopwatch.StartNew();
        bool ok = (bool)Unwrap(gw, null, new object[] { SF(TMain, "AutogenProgress"), ctrl });
        Console.WriteLine("WorldGen.GenerateWorld returned " + ok + " after " + clock.ElapsedMilliseconds + " ms");
        if (!ok) throw new Exception("GenerateWorld returned false");
        WritePassRecordX(XPassTotal, "<GenerateWorld returned>", true);
        CallStatic(TWorldFile, "SaveNewWorld");
    }

    // The wrapper pass type of Gen.cs (MakeHookType), calling this file's callbacks.
    static Type _hookTypeX;
    static Type MakeHookTypeX()
    {
        Type gp = T("Terraria.WorldBuilding.GenPass"), prog = T("Terraria.WorldBuilding.GenerationProgress");
        Type cfg = T("Terraria.IO.GameConfiguration");
        var ab = AppDomain.CurrentDomain.DefineDynamicAssembly(new AssemblyName("WGHookX"), System.Reflection.Emit.AssemblyBuilderAccess.Run);
        var mb = ab.DefineDynamicModule("WGHookX");
        var tb = mb.DefineType("HookPassX", TypeAttributes.Public | TypeAttributes.Class, gp);
        var fInner = tb.DefineField("inner", gp, FieldAttributes.Public);
        var fIdx = tb.DefineField("idx", typeof(int), FieldAttributes.Public);
        var ctor = tb.DefineConstructor(MethodAttributes.Public, CallingConventions.Standard, new[] { typeof(string), typeof(double), gp, typeof(int) });
        var il = ctor.GetILGenerator();
        il.Emit(System.Reflection.Emit.OpCodes.Ldarg_0); il.Emit(System.Reflection.Emit.OpCodes.Ldarg_1); il.Emit(System.Reflection.Emit.OpCodes.Ldarg_2);
        il.Emit(System.Reflection.Emit.OpCodes.Call, gp.GetConstructor(new[] { typeof(string), typeof(double) }));
        il.Emit(System.Reflection.Emit.OpCodes.Ldarg_0); il.Emit(System.Reflection.Emit.OpCodes.Ldarg_3); il.Emit(System.Reflection.Emit.OpCodes.Stfld, fInner);
        il.Emit(System.Reflection.Emit.OpCodes.Ldarg_0); il.Emit(System.Reflection.Emit.OpCodes.Ldarg, (short)4); il.Emit(System.Reflection.Emit.OpCodes.Stfld, fIdx);
        il.Emit(System.Reflection.Emit.OpCodes.Ret);
        MethodInfo baseApplyPass = gp.GetMethod("ApplyPass", AllInst);
        var m = tb.DefineMethod("ApplyPass", MethodAttributes.Family | MethodAttributes.Virtual | MethodAttributes.HideBySig, typeof(void), new[] { prog, cfg });
        il = m.GetILGenerator();
        il.Emit(System.Reflection.Emit.OpCodes.Ldarg_0); il.Emit(System.Reflection.Emit.OpCodes.Ldfld, fInner);
        il.Emit(System.Reflection.Emit.OpCodes.Ldarg_1); il.Emit(System.Reflection.Emit.OpCodes.Ldarg_2);
        il.Emit(System.Reflection.Emit.OpCodes.Callvirt, gp.GetMethod("Apply"));
        il.Emit(System.Reflection.Emit.OpCodes.Ldarg_0); il.Emit(System.Reflection.Emit.OpCodes.Ldfld, fIdx);
        il.Emit(System.Reflection.Emit.OpCodes.Call, typeof(WG).GetMethod("AfterPassX", BindingFlags.Static | BindingFlags.Public));
        il.Emit(System.Reflection.Emit.OpCodes.Ret);
        tb.DefineMethodOverride(m, baseApplyPass);
        return tb.CreateType();
    }

    static void OnPassesLoadedX(object controller)
    {
        IList passes = (IList)controller.GetType().GetProperty("Passes").GetValue(controller, null);
        XPassTotal = passes.Count;
        if (_hookTypeX == null) _hookTypeX = MakeHookTypeX();
        Type gp = T("Terraria.WorldBuilding.GenPass");
        int disabled = 0;
        for (int i = 0; i < passes.Count; i++)
        {
            object p = passes[i];
            string name = (string)gp.GetField("Name").GetValue(p);
            double weight = (double)gp.GetField("Weight").GetValue(p);
            bool en = (bool)gp.GetProperty("Enabled").GetValue(p, null);
            XPassNames.Add(name); XPassTypes.Add(p.GetType().Name); XPassEnabled.Add(en);
            object w = Activator.CreateInstance(_hookTypeX, new object[] { name, weight, p, i });
            // a disabled pass (WorldGen.DisablePassesForSpecialSeeds ran before this callback) is not applied: RunPass
            // returns a Skipped result for it [WorldGenerator.cs:490-497] and no row is written
            if (!en) { gp.GetMethod("Disable").Invoke(w, null); disabled++; }
            passes[i] = w;
        }
        WritePassRecordX(-1, "<before pass 0: after clearWorld + Reset>", true);
        Console.WriteLine("passes loaded: " + passes.Count + " (" + disabled + " disabled), each wrapped in a HookPassX");
    }

    // REFDUMP_WATCH=<pass>:<x>,<y>;<x>,<y>;.. (diagnosis): while pass <pass> runs, a second thread polls those tiles
    // and, whenever one changes, suspends the generating thread and writes its stack to <tag>.watch.txt (which code
    // made the change; the frozen world's dual dungeon cells, 2026-10-16)
    static Thread XWatchThread;
    static volatile bool XWatchStop;
    static void WatchStart(Thread main, int pass)
    {
        string spec = Environment.GetEnvironmentVariable("REFDUMP_WATCH");
        if (string.IsNullOrEmpty(spec)) return;
        int colon = spec.IndexOf(':');
        if (int.Parse(spec.Substring(0, colon)) != pass) return;
        var cells = spec.Substring(colon + 1).Split(';').Select(c => c.Split(',')).Select(c => new int[] { int.Parse(c[0]), int.Parse(c[1]) }).ToArray();
        Array tile = (Array)SF(TMain, "tile");
        Type tt = tile.GetType().GetElementType();
        FieldInfo[] fs = new[] { "type", "wall", "liquid", "sTileHeader", "bTileHeader", "bTileHeader2", "bTileHeader3", "frameX", "frameY" }.Select(n => tt.GetField(n, BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic)).ToArray();
        Func<int[], string> read = c =>
        {
            object t = tile.GetValue(c[0], c[1]);
            if (t == null) return "null";
            return string.Join(" ", fs.Select(f => f == null ? "?" : Convert.ToString(f.GetValue(t), Inv)));
        };
        string[] last = cells.Select(read).ToArray();
        var log = new StreamWriter(Path.Combine(XOut, XTag + ".watch.txt"), false);
        log.WriteLine("watch pass " + pass + ": " + spec);
        for (int k = 0; k < cells.Length; k++) log.WriteLine("start " + cells[k][0] + "," + cells[k][1] + ": " + last[k]);
        log.Flush();
        XWatchStop = false;
        XWatchThread = new Thread(() =>
        {
            while (!XWatchStop)
            {
                for (int k = 0; k < cells.Length; k++)
                {
                    string now = read(cells[k]);
                    if (now == last[k]) continue;
#pragma warning disable 618
                    main.Suspend();
                    string st;
                    try { st = new StackTrace(main, false).ToString(); }
                    catch (Exception e) { st = "(stack: " + e.Message + ")"; }
                    now = read(cells[k]);
                    main.Resume();
#pragma warning restore 618
                    log.WriteLine("change " + cells[k][0] + "," + cells[k][1] + ": " + last[k] + " -> " + now);
                    log.WriteLine(st);
                    log.Flush();
                    last[k] = now;
                }
            }
            for (int k = 0; k < cells.Length; k++) log.WriteLine("end " + cells[k][0] + "," + cells[k][1] + ": " + read(cells[k]));
            log.Close();
        });
        XWatchThread.IsBackground = true;
        XWatchThread.Start();
    }

    // REFDUMP_DUNGEON=<pass> (diagnosis): after pass <pass>, every dungeon of DungeonCrawler.dungeonData with its rooms,
    // halls, doors, platforms and features, one line each, to <tag>.dungeon.txt (the same lines as the port's
    // WORLDGEN_DUNGEON, tests/worldgen.rs): which room / hall the original was given (positions, sizes, seeds)
    static string DX(object v)
    {
        if (v == null) return "null";
        if (v is bool) return (bool)v ? "1" : "0";
        if (v is double) return BitConverter.DoubleToInt64Bits((double)v).ToString("x16");
        if (v is float) return BitConverter.ToInt32(BitConverter.GetBytes((float)v), 0).ToString("x8");
        Type t = v.GetType();
        if (t.IsEnum) return Convert.ToInt64(v, Inv).ToString(Inv);
        if (t.IsPrimitive) return Convert.ToString(v, Inv);
        if (t.FullName == "Microsoft.Xna.Framework.Point") return IF(v, "X") + "," + IF(v, "Y");
        if (t.FullName == "ReLogic.Utilities.Vector2D") return DX(IF(v, "X")) + "," + DX(IF(v, "Y"));
        if (t.FullName == "Microsoft.Xna.Framework.Rectangle") return IF(v, "X") + "," + IF(v, "Y") + "," + IF(v, "Width") + "," + IF(v, "Height");
        if (t.FullName == "Terraria.GameContent.Generation.Dungeon.DungeonBounds")
            return IF(v, "_boundsLeft") + "," + IF(v, "_boundsTop") + "," + IF(v, "_boundsRight") + "," + IF(v, "_boundsBottom") + "," + DX(IF(v, "_hitbox"));
        if (t.FullName == "Terraria.GameContent.Generation.Dungeon.DungeonGenerationStyleData") return DX(IF(v, "Style"));
        return "?" + t.Name;
    }
    static string DN(object o) { return o == null ? "null" : o.GetType().Name; }
    static string DF(object o, params string[] names)
    {
        var sb = new StringBuilder();
        foreach (string n in names) sb.Append(' ').Append(n).Append('=').Append(DX(IF(o, n)));
        return sb.ToString();
    }
    static string DShape(object s)
    {
        if (s == null) return "null";
        switch (s.GetType().Name)
        {
            case "CircleRoom": return "CircleRoom(" + IF(s, "_horizontalRadius") + "," + IF(s, "_verticalRadius") + ")";
            case "QuadCircleRoom": return "QuadCircleRoom(" + IF(s, "_radius") + "," + IF(s, "_distanceBetweenSpheres") + ")";
            case "MoundRoom": return "MoundRoom(" + IF(s, "_halfWidth") + "," + IF(s, "_height") + ")";
            case "HourglassRoom": return "HourglassRoom(" + IF(s, "_width") + "," + IF(s, "_height") + "," + DX(IF(s, "_percentileAddon")) + ")";
        }
        return s.GetType().Name;
    }
    // The pass clears DungeonCrawler.dungeonData when it ends (WorldGen.cs:14917), so the dump is taken DURING the pass: a
    // second thread waits for Main.statusText to become "WorldGeneration.DungeonLate" (DungeonCrawler.MakeDungeon sets it
    // after the layout, doors, platforms and every other global feature, before the late dual-dungeon features), suspends
    // the generating thread and writes dungeon GenVars.CurrentDungeon. The port takes it at the same status text.
    static Thread XDungeonThread;
    static volatile bool XDungeonStop;
    static void DungeonWatchStart(Thread main, int pass)
    {
        string spec = Environment.GetEnvironmentVariable("REFDUMP_DUNGEON");
        if (string.IsNullOrEmpty(spec) || int.Parse(spec) != pass) return;
        Type lang = T("Terraria.Localization.Language");
        string late = (string)lang.GetMethod("GetTextValue", new[] { typeof(string) }).Invoke(null, new object[] { "WorldGeneration.DungeonLate" });
        FieldInfo status = TMain.GetField("statusText", AllStatic);
        PropertyInfo cur = T("Terraria.WorldBuilding.GenVars").GetProperty("CurrentDungeon", AllStatic);
        var w = new StreamWriter(Path.Combine(XOut, XTag + ".dungeon.txt"), false);
        var done = new HashSet<int>();
        XDungeonStop = false;
        XDungeonThread = new Thread(() =>
        {
            while (!XDungeonStop)
            {
                if ((string)status.GetValue(null) != late || done.Contains((int)cur.GetValue(null, null))) continue;
#pragma warning disable 618
                main.Suspend();
                try
                {
                    int d = (int)cur.GetValue(null, null);
                    if (done.Add(d) && (string)status.GetValue(null) == late) { DungeonDumpOne(w, d); w.Flush(); Console.WriteLine("dungeon dump: dungeon " + d); }
                }
                catch (Exception e) { Console.WriteLine("DUNGEON DUMP ERROR: " + e); }
                main.Resume();
#pragma warning restore 618
            }
            w.Close();
        });
        XDungeonThread.IsBackground = true;
        XDungeonThread.Start();
    }
    static void DungeonDumpOne(StreamWriter w, int d)
    {
        IList all = (IList)SF(T("Terraria.GameContent.Generation.Dungeon.DungeonCrawler"), "dungeonData");
        {
            object dd = all[d];
            IList rooms = (IList)IF(dd, "dungeonRooms"), halls = (IList)IF(dd, "dungeonHalls"), feats = (IList)IF(dd, "dungeonFeatures");
            IList doors = (IList)IF(dd, "dungeonDoorData"), plats = (IList)IF(dd, "dungeonPlatformData"), prot = (IList)IF(dd, "protectedDungeonBounds");
            w.WriteLine("D " + d + DF(dd, "Type", "Iteration", "makeNextPitTrapFlooded", "useSkewedDungeonEntranceHalls", "createdDungeonEntranceOnSurface",
                "dungeonEntranceStrengthX", "dungeonEntranceStrengthY", "dungeonEntranceStrengthX2", "dungeonEntranceStrengthY2", "lastDungeonHall", "dungeonBounds",
                "globalFeatureScalar", "dungeonStepScalar", "hallStrengthScalar", "hallStepScalar", "hallInteriorToExteriorRatio", "hallSlantVariantScalar",
                "roomStrengthScalar", "roomStepScalar", "roomInteriorToExteriorRatio", "roomSlantVariantScalar")
                + " rooms=" + rooms.Count + " halls=" + halls.Count + " features=" + feats.Count + " doors=" + doors.Count + " platforms=" + plats.Count + " protected=" + prot.Count
                + " entrance=" + DN(IF(dd, "dungeonEntrance")));
            for (int k = 0; k < rooms.Count; k++)
            {
                object r = rooms[k], st = IF(r, "settings");
                string line = "R " + d + "." + k + " " + DN(r) + " " + DN(st) + DF(r, "calculated", "generated", "InnerBounds", "OuterBounds")
                    + DF(st, "RoomPosition", "RoomType", "RandomSeed", "StyleData", "ProgressionStage", "StartingRoom", "OverridePaintTile", "OverridePaintWall",
                        "ForceStyleForDoorsAndPlatforms", "OnCurvedLine", "Orientation", "HallwayPointAdjuster");
                switch (st.GetType().Name)
                {
                    case "GenShapeDungeonRoomSettings": line += DF(st, "ShapeType", "BoundingRadius") + " inner=" + DShape(IF(st, "InnerShape")) + " outer=" + DShape(IF(st, "OuterShape")) + DF(r, "_floodedTileCount"); break;
                    case "RegularDungeonRoomSettings": line += DF(st, "OverrideInnerBoundsSize", "OverrideOuterBoundsSize"); break;
                    case "WormlikeDungeonRoomSettings": line += DF(st, "FirstSideIterations", "SecondSideIterations"); break;
                    case "LivingTreeDungeonRoomSettings": line += DF(st, "InnerWidth", "InnerHeight", "Depth", "BoundingRadius"); break;
                    case "LegacyDungeonRoomSettings": line += DF(st, "OverrideStrength", "OverrideSteps", "OverrideStartPosition", "OverrideEndPosition", "OverrideVelocity", "OverrideInteriorToExteriorRatio", "IsEntranceRoom"); break;
                }
                w.WriteLine(line);
            }
            for (int k = 0; k < halls.Count; k++)
            {
                object h = halls[k], st = IF(h, "settings");
                string line = "H " + d + "." + k + " " + DN(h) + " " + DN(st) + DF(h, "calculated", "generated", "Bounds", "StartPosition", "EndPosition", "StartDirection", "EndDirection", "CrackedBrick")
                    + DF(st, "HallType", "RandomSeed", "StyleData", "OverridePaintTile", "OverridePaintWall", "CrackedBrickChance", "PlaceOverProtectedBricks", "ZigzagChance", "ForceStyleForDoorsAndPlatforms", "CarveOnly");
                switch (st.GetType().Name)
                {
                    case "RegularDungeonHallSettings": line += DF(st, "OverrideInnerBoundsSize", "OverrideOuterBoundsSize"); break;
                    case "SineDungeonHallSettings": line += DF(st, "Magnitude", "Iterations", "FlipSine"); break;
                    case "StairwellDungeonHallSettings": line += DF(st, "MaxDistFromLine", "PointVariance", "InnerBoundsSize", "OuterBoundsSize", "Gradient", "IsEntranceHall"); break;
                    case "LegacyDungeonHallSettings": line += DF(st, "OverrideStrength", "OverrideSteps", "ForceHorizontal", "OverrideInteriorToExteriorRatio"); break;
                    case "LegacyEntranceDungeonHallSettings": line += DF(st, "OverrideStrength", "OverrideSteps", "ForceHorizontal", "OverrideInteriorToExteriorRatio", "UsePrecalculatedEntrance"); break;
                }
                w.WriteLine(line);
            }
            for (int k = 0; k < doors.Count; k++) w.WriteLine("O " + d + "." + k + DF(doors[k], "Position", "OverrideStyle", "Direction", "InAHallway", "OverrideWidthFluff", "SkipOtherDoorsCheck", "SkipSpaceCheck", "AlwaysClearArea"));
            for (int k = 0; k < plats.Count; k++) w.WriteLine("P " + d + "." + k + DF(plats[k], "Position", "OverrideStyle", "OverrideMaxLengthAllowed", "OverrideHeightFluff", "InAHallway", "ForcePlacement", "SkipOtherPlatformsCheck", "SkipSpaceCheck", "PlaceBooksChance", "NoWaterbolt", "PlacePotsChance", "PlaceWaterCandlesChance", "PlacePotionBottlesChance"));
            for (int k = 0; k < prot.Count; k++) w.WriteLine("B " + d + "." + k + " " + DX(prot[k]));
            for (int k = 0; k < feats.Count; k++) w.WriteLine("F " + d + "." + k + " " + DN(feats[k]));
        }
    }

    public static void AfterPassX(int idx)
    {
        if (XWatchThread != null) { XWatchStop = true; XWatchThread.Join(); XWatchThread = null; }
        if (XDungeonThread != null) { XDungeonStop = true; XDungeonThread.Join(); XDungeonThread = null; }
        WatchStart(Thread.CurrentThread, idx + 1);
        DungeonWatchStart(Thread.CurrentThread, idx + 1);
        // RunPass catches exceptions thrown inside a pass [WorldGenerator.cs:501-508]: a failing record would be silent
        try { WritePassRecordX(idx, XPassNames[idx], false); }
        catch (Exception e) { XHookErrors.Add("pass " + idx + ": " + e); Console.WriteLine("HOOK ERROR pass " + idx + ": " + e); }
    }

    static void InitSelectorsX()
    {
        XGenVars = T("Terraria.WorldBuilding.GenVars");
        XWorldGenPrimStatics = TWorldGen.GetFields(AllStatic | BindingFlags.DeclaredOnly)
            .Where(f => !f.IsLiteral && !f.IsDefined(typeof(ThreadStaticAttribute), false) && (f.FieldType.IsPrimitive || f.FieldType.IsEnum)).OrderBy(f => f.Name, StringComparer.Ordinal).ToArray();
        // the list of Gen.cs:InitSelectors, unchanged
        string[] names = { "maxTilesX", "maxTilesY", "worldSurface", "worldSurfaceLow", "worldSurfaceHigh", "rockLayer", "rockLayerLow", "rockLayerHigh",
            "spawnTileX", "spawnTileY", "dungeonX", "dungeonY", "time", "dayTime", "moonPhase", "moonType", "GameMode", "hardMode",
            "treeX", "treeStyle", "caveBackX", "caveBackStyle", "iceBackStyle", "jungleBackStyle", "hellBackStyle", "snowBackStyle", "desertBackStyle",
            "oceanBackStyle", "mushroomBackStyle", "underworldBackStyle", "cloudBGActive", "slimeRainTime", "raining", "rainTime", "cloudAlpha", "maxRaining",
            "windSpeedCurrent", "windSpeedTarget", "numClouds", "bloodMoon", "eclipse", "halloween", "xMas", "netMode", "gameMenu", "dedServ",
            "drunkWorld", "getGoodWorld", "notTheBeesWorld", "noTrapsWorld", "tenthAnniversaryWorld", "dontStarveWorld", "remixWorld", "zenithWorld", "skyblockWorld",
            "worldName", "UnderworldLayer", "invasionType", "invasionSize", "numLavaBackground" };
        XMainSel = new List<KeyValuePair<string, FieldInfo>>();
        foreach (string n in names)
        {
            FieldInfo f = TMain.GetField(n, AllStatic);
            if (f == null) continue;
            XMainSel.Add(new KeyValuePair<string, FieldInfo>(n, f));
        }
    }

    // A zlib stream written piece by piece (the snapshot of a large world is 282 MB: it is never held in memory).
    class ZlibOut : IDisposable
    {
        FileStream fs; DeflateStream ds; uint a = 1, b = 0;
        public ZlibOut(string path) { fs = new FileStream(path, FileMode.Create); fs.WriteByte(0x78); fs.WriteByte(0x9C); ds = new DeflateStream(fs, CompressionLevel.Optimal, true); }
        public void Write(byte[] buf, int len)
        {
            int i = 0;
            while (i < len)
            {
                int n = Math.Min(len - i, 5552);
                for (int k = 0; k < n; k++) { a += buf[i + k]; b += a; }
                a %= 65521; b %= 65521; i += n;
            }
            ds.Write(buf, 0, len);
        }
        public void Dispose()
        {
            ds.Dispose();
            uint ad = (b << 16) | a;
            fs.WriteByte((byte)(ad >> 24)); fs.WriteByte((byte)(ad >> 16)); fs.WriteByte((byte)(ad >> 8)); fs.WriteByte((byte)ad);
            fs.Dispose();
        }
    }

    // The row of Gen.cs:WritePassRecord (same keys, same order, same value rules), with the tile map read column by column.
    static void WritePassRecordX(int idx, string name, bool extra)
    {
        if (XWorldGenPrimStatics == null) InitSelectorsX();
        var sw = Stopwatch.StartNew();
        if (_fillColumn == null) _fillColumn = MakeFillColumn();
        int w = (int)SF(TMain, "maxTilesX"), h = (int)SF(TMain, "maxTilesY");
        Array tile = (Array)SF(TMain, "tile");
        bool snap = XSnapSet.Contains(idx) || (XSnapDefault && (extra || idx == 0 || idx == 1 || idx % 10 == 0 || idx == XPassTotal - 1));
        string snapFile = null;
        ZlibOut z = null;
        if (snap)
        {
            snapFile = XTag + (idx < 0 ? ".before_pass_000" : idx >= XPassTotal ? ".after_generate_world" : ".after_pass_" + idx.ToString("d3")) + ".tfrm.z";
            z = new ZlibOut(Path.Combine(XOut, snapFile));
            byte[] head = new byte[20];
            head[0] = (byte)'T'; head[1] = (byte)'F'; head[2] = (byte)'R'; head[3] = (byte)'M';
            BitConverter.GetBytes(1).CopyTo(head, 4); BitConverter.GetBytes(w).CopyTo(head, 8); BitConverter.GetBytes(h).CopyTo(head, 12); BitConverter.GetBytes(RecSize).CopyTo(head, 16);
            z.Write(head, head.Length);
        }
        byte[] col = new byte[h * RecSize];
        ulong[] colHash = new ulong[w];
        ulong all = FnvBasis;
        long active = 0, walls = 0, liq = 0;
        for (int x = 0; x < w; x++)
        {
            Array.Clear(col, 0, col.Length);
            _fillColumn(tile, x, h, col);
            colHash[x] = Fnv(col, 0, col.Length, FnvBasis);
            all = Fnv(col, 0, col.Length, all);
            for (int o = 0; o < col.Length; o += RecSize)
            {
                if ((col[o + 5] & 0x20) != 0) active++;
                if (col[o + 2] != 0 || col[o + 3] != 0) walls++;
                if (col[o + 4] != 0) liq++;
            }
            if (z != null) z.Write(col, col.Length);
        }
        if (z != null) { z.Dispose(); XSnapFiles.Add(snapFile); }
        var sb = new StringBuilder(8192);
        sb.Append("{\"row\":" + XRowCount + ",\"i\":" + idx + ",\"name\":" + Q(name) + ",\"extra\":" + (extra ? "true" : "false"));
        sb.Append(",\"tiles_fnv64\":\"" + Hex(all) + "\",\"cols_fnv64\":\"" + Hex(FnvU64(colHash)) + "\"");
        sb.Append(",\"n_active\":" + active + ",\"n_wall\":" + walls + ",\"n_liquid\":" + liq);
        object rand = SF(TMain, "rand");
        sb.Append(",\"rand\":" + RandJson(rand));
        if (!extra) XPassNextRand.Add(RandNextPreview(rand));
        sb.Append(",\"main\":{");
        bool first = true;
        foreach (var kv in XMainSel)
        {
            if (!first) sb.Append(','); first = false;
            sb.Append(Q(kv.Key)).Append(':'); J(sb, kv.Value.GetValue(null), 0);
        }
        sb.Append("},\"worldgen_static\":{");
        first = true;
        foreach (FieldInfo f in XWorldGenPrimStatics)
        {
            if (!first) sb.Append(','); first = false;
            sb.Append(Q(f.Name)).Append(':'); J(sb, f.GetValue(null), 0);
        }
        sb.Append("},\"genvars\":");
        sb.Append(StaticsJson(XGenVars, f => f.Name != "configuration" && f.Name != "tileSolid"));
        Array chests = (Array)SF(TMain, "chest"), signs = (Array)SF(TMain, "sign"), npcs = (Array)SF(TMain, "npc");
        int nChest = 0, nSign = 0, nNpc = 0, nItems = 0;
        var cs = new StringBuilder();
        foreach (object c in chests)
        {
            if (c == null) continue;
            nChest++;
            cs.Append(IF(c, "x")).Append(',').Append(IF(c, "y")).Append(',').Append(IF(c, "name")).Append('|');
            foreach (object it in (Array)IF(c, "item"))
            {
                if (it == null) { cs.Append("-;"); continue; }
                cs.Append(IF(it, "type")).Append(':').Append(IF(it, "stack")).Append(':').Append(IF(it, "prefix")).Append(';');
            }
            cs.Append('\n');
        }
        foreach (object sg in signs) if (sg != null) { nSign++; cs.Append("S").Append(IF(sg, "x")).Append(',').Append(IF(sg, "y")).Append(',').Append(IF(sg, "text")).Append('\n'); }
        foreach (object n in npcs)
            if (n != null && (bool)IF(n, "active")) { nNpc++; cs.Append("N").Append(IF(n, "type")).Append(',').Append(JStr(IF(n, "position"))).Append('\n'); }
        foreach (object it in (Array)SF(TMain, "item")) if (it != null && (bool)IF(it, "active")) nItems++;
        byte[] cb = Encoding.UTF8.GetBytes(cs.ToString());
        object teById = SF(T("Terraria.DataStructures.TileEntity"), "ByID"), teByPos = SF(T("Terraria.DataStructures.TileEntity"), "ByPosition");
        sb.Append(",\"counts\":{\"chests\":" + nChest + ",\"signs\":" + nSign + ",\"npcs_active\":" + nNpc + ",\"items_active\":" + nItems +
            ",\"tile_entities\":" + ((IDictionary)teById).Count + ",\"tile_entity_positions\":" + ((IDictionary)teByPos).Count + "}");
        sb.Append(",\"chests_signs_npcs_fnv64\":\"" + Hex(Fnv(cb, 0, cb.Length, FnvBasis)) + "\"");
        sb.Append("}");
        XPassLog.WriteLine(sb.ToString());
        XPassLog.Flush();
        foreach (ulong hh in colHash) XColLog.Write(hh);
        XColLog.Flush();
        XRowCount++;
        Console.WriteLine("pass " + idx.ToString("d3") + " " + name + "  tiles=" + Hex(all) + (snap ? "  [snapshot]" : "") + "  (record " + sw.ElapsedMilliseconds + " ms)");
    }
}
