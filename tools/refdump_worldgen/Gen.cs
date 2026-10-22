// RefDumpWorldgen "gen" mode: generates the ref_small world with the real game's code and dumps one record per generation pass.
//
//   RefDumpWorldgen.exe gen <outdir> <tag> [passes|real] [task|main] [nosnap]
//
// The parameters are those of the original run `TerrariaServer.exe -autocreate 1 -worldname ref_small -seed 12345 -world <path>`
// (work\refserver.log): size small 4200x1200, GameMode 0, WorldGenParam_Evil -1 (random), seed text "12345" -> int seed 12345.
// The world is saved to work\worldgen-scratch\<tag>\Worlds\ref_small.wld by the game's own WorldFile.SaveNewWorld.
//
// mode "real":   the dedicated server's exact path: Task.Factory.StartNew(WorldGen.serverLoadWorldCallBack) [WorldGen.cs:6903-6906, 6840]
//                -> WorldFile.LoadWorld() [WorldFile.cs:650] -> autogen block [666-709] -> WorldGen.GenerateWorld(Main.AutogenProgress)
//                -> WorldFile.SaveNewWorld() [862] -> and then LoadWorld's normal load of the file it just wrote. No instrumentation at all.
// mode "passes": the autogen block of LoadWorld replayed step by step by this harness (same calls, same order, see Gen.cs:RunPasses) but
//                GenerateWorld gets a WorldGenerator.Controller whose public OnPassesLoaded callback [WorldGenerator.cs:30,109-111] wraps
//                every PassLegacy's private _method delegate so that a callback runs right after the pass body and before RunPass's own
//                bookkeeping [WorldGenerator.cs:488-517]. The game's own WorldGenerator.GenerateWorld loop [421-466] runs unchanged.
// thread:        task = a thread-pool thread via Task.Factory.StartNew (like the real server); main = the process main thread.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Linq.Expressions;
using System.Reflection;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

static partial class WG
{
    const string SeedText = "12345";
    const string WorldNameConst = "ref_small";

    static StreamWriter PassLog;
    static BinaryWriter ColLog;
    static string GenOut;
    static bool DoSnap = true;
    static int PassIndex = 0, PassTotal = 0, RowCount = 0;
    static List<string> PassNames = new List<string>();
    static List<string> PassTypes = new List<string>();
    static List<string> HookErrors = new List<string>();
    static List<int> PassNextRand = new List<int>();
    static Snap SnapBuf;
    static List<string> SnapFiles = new List<string>();
    static readonly List<string> TraceLines = new List<string>();
    static Stopwatch GenClock;
    static FieldInfo[] WorldGenPrimStatics;
    static List<KeyValuePair<string, FieldInfo>> MainSel;
    static Type TGenVars;

    class Capture : TraceListener
    {
        public override void Write(string m) { lock (TraceLines) TraceLines.Add(m); }
        public override void WriteLine(string m) { lock (TraceLines) TraceLines.Add(m); }
    }

    static int MainGen(string[] args)
    {
        // args: gen <outdir> <tag> [mode] [thread] [nosnap]
        string outdir = Path.GetFullPath(args[1]);
        string tag = args[2];
        string mode = args.Length > 3 ? args[3] : "passes";
        string thread = args.Length > 4 ? args[4] : "task";
        DoSnap = !(args.Length > 5 && args[5] == "nosnap");
        Root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(Root, "work", "refserver");
        string scratch = Dir(Path.Combine(Root, "work", "worldgen-scratch", tag));
        SaveDir = Dir(Path.Combine(scratch, "save"));
        GenOut = Dir(outdir);
        string wldPath = Path.Combine(Dir(Path.Combine(scratch, "Worlds")), WorldNameConst + ".wld");
        if (File.Exists(wldPath)) File.Delete(wldPath);
        Watchdog(420);
        Trace.Listeners.Add(new Capture());
        int rc = 0;
        try { RunGen(tag, mode, thread, wldPath, scratch); }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }

    // Replays WorldFile.cs:666-709 (the autogen block of LoadWorld) with a controller. Every call is the game's own.
    static void RunPasses(string wldPath)
    {
        Type wfd = T("Terraria.IO.WorldFileData"), gen = T("Terraria.WorldGen"), opts = T("Terraria.WorldBuilding.WorldGenerationOptions");
        if (opts == null) throw new Exception("no options type");
        // WorldFile.cs:666-678 : Utils.TryCreatingDirectory(<dir of worldPathName>) (done by the caller already)
        // WorldFile.cs:679
        object md = CallStatic(TWorldFile, "CreateMetadata", (string)SF(TMain, "worldName"), false, (int)SF(TMain, "GameMode"));
        SSet(TMain, "ActiveWorldFileData", md);
        // WorldFile.cs:680-705
        string text = ((string)SF(TMain, "AutogenSeedName") ?? "").Trim();
        object[] ta = new object[] { text, true, null, null, null };
        MethodInfo tryApply = wfd.GetMethod("TryApplyingCopiedSeed", BindingFlags.Public | BindingFlags.Static);
        bool applied = (bool)Unwrap(tryApply, null, ta);
        if (applied) throw new Exception("seed text unexpectedly parsed as a copied seed");
        text = (string)CallStatic(T("Terraria.Utils"), "TrimUserString", text, (int)SF(wfd, "MAX_USER_SEED_TEXT_LENGTH"));
        object opt = CallStatic(opts, "GetOptionFromSeedText", text);
        CallStatic(opts, "SelectOption", opt);
        foreach (object o in (IEnumerable)SF(opts, "Options"))
            if ((bool)GetM(o, "AutoGenEnabled")) SetM(o, "Enabled", true);
        if (opt != null || string.IsNullOrWhiteSpace(text)) throw new Exception("seed text mapped to an option / empty: not this scenario");
        md.GetType().GetMethod("SetSeed").Invoke(md, new object[] { text });
        Console.WriteLine("ActiveWorldFileData: Seed=" + md.GetType().GetProperty("Seed").GetValue(md, null) + " SeedText=" + md.GetType().GetProperty("SeedText").GetValue(md, null) + " GameMode=" + SF(TMain, "GameMode"));
        // WorldFile.cs:706-709, with the controller
        Type ctrlT = T("Terraria.WorldBuilding.WorldGenerator+Controller");
        object ctrl = Activator.CreateInstance(ctrlT, new object[] { null });
        Type actT = typeof(Action<>).MakeGenericType(ctrlT);
        ParameterExpression pc = Expression.Parameter(ctrlT, "c");
        Delegate cb = Expression.Lambda(actT, Expression.Call(typeof(WG).GetMethod("OnPassesLoaded", BindingFlags.Static | BindingFlags.NonPublic), Expression.Convert(pc, typeof(object))), pc).Compile();
        ctrlT.GetField("OnPassesLoaded").SetValue(ctrl, cb);
        MethodInfo gw = gen.GetMethods(BindingFlags.Public | BindingFlags.Static).First(m => m.Name == "GenerateWorld");
        GenClock = Stopwatch.StartNew();
        bool ok = (bool)Unwrap(gw, null, new object[] { SF(TMain, "AutogenProgress"), ctrl });
        Console.WriteLine("WorldGen.GenerateWorld returned " + ok + " after " + GenClock.ElapsedMilliseconds + " ms");
        if (!ok) throw new Exception("GenerateWorld returned false");
        // record the state after Finish() / RestoreTemporaryStateChanges() as an extra record, then WorldFile.cs:708
        WritePassRecord(PassTotal, "<GenerateWorld returned>", true, true);
        CallStatic(TWorldFile, "SaveNewWorld");
    }

    // ---- WorldGenerator.Controller.OnPassesLoaded: replace every pass with a wrapper subclass of GenPass (emitted at run time) whose
    // ApplyPass calls inner.Apply(progress, configuration) [GenPass.cs:30-33] and then WG.AfterPass(index). Name, Weight and Enabled are copied.
    // (Only part of the passes are PassLegacy; e.g. "Terrain" is Terraria.GameContent.Biomes.TerrainPass, so PassLegacy._method cannot be wrapped alone.)
    static Type _hookType;
    static Type MakeHookType()
    {
        Type gp = T("Terraria.WorldBuilding.GenPass"), prog = T("Terraria.WorldBuilding.GenerationProgress");
        Type cfg = T("Terraria.IO.GameConfiguration");
        var ab = AppDomain.CurrentDomain.DefineDynamicAssembly(new AssemblyName("WGHook"), System.Reflection.Emit.AssemblyBuilderAccess.Run);
        var mb = ab.DefineDynamicModule("WGHook");
        var tb = mb.DefineType("HookPass", TypeAttributes.Public | TypeAttributes.Class, gp);
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
        il.Emit(System.Reflection.Emit.OpCodes.Ldarg_0); il.Emit(System.Reflection.Emit.OpCodes.Ldfld, fIdx);
        il.Emit(System.Reflection.Emit.OpCodes.Call, typeof(WG).GetMethod("BeforePass", BindingFlags.Static | BindingFlags.Public));
        il.Emit(System.Reflection.Emit.OpCodes.Ldarg_0); il.Emit(System.Reflection.Emit.OpCodes.Ldfld, fInner);
        il.Emit(System.Reflection.Emit.OpCodes.Ldarg_1); il.Emit(System.Reflection.Emit.OpCodes.Ldarg_2);
        il.Emit(System.Reflection.Emit.OpCodes.Callvirt, gp.GetMethod("Apply"));
        il.Emit(System.Reflection.Emit.OpCodes.Ldarg_0); il.Emit(System.Reflection.Emit.OpCodes.Ldfld, fIdx);
        il.Emit(System.Reflection.Emit.OpCodes.Call, typeof(WG).GetMethod("AfterPass", BindingFlags.Static | BindingFlags.Public));
        il.Emit(System.Reflection.Emit.OpCodes.Ret);
        tb.DefineMethodOverride(m, baseApplyPass);
        return tb.CreateType();
    }

    static void OnPassesLoaded(object controller)
    {
        IList passes = (IList)controller.GetType().GetProperty("Passes").GetValue(controller, null);
        PassTotal = passes.Count;
        if (_hookType == null) _hookType = MakeHookType();
        Type gp = T("Terraria.WorldBuilding.GenPass");
        int legacy = 0;
        for (int i = 0; i < passes.Count; i++)
        {
            object p = passes[i];
            string name = (string)gp.GetField("Name").GetValue(p);
            double weight = (double)gp.GetField("Weight").GetValue(p);
            bool en = (bool)gp.GetProperty("Enabled").GetValue(p, null);
            PassNames.Add(name);
            PassTypes.Add(p.GetType().Name);
            if (p.GetType().Name == "PassLegacy") legacy++;
            object w = Activator.CreateInstance(_hookType, new object[] { name, weight, p, i });
            if (!en) gp.GetMethod("Disable").Invoke(w, null);
            passes[i] = w;
        }
        // The generator calls clearWorld() and Reset() [WorldGen.cs:11092-11093] before it builds the pass list, and OnPassesLoaded runs at the start of
        // WorldGenerator.GenerateWorld [WorldGenerator.cs:424], so this is the state right before pass 0: the first record of the dump (row 0, i = -1).
        WritePassRecord(-1, "<before pass 0: after clearWorld + Reset>", true, true);
        Console.WriteLine("passes loaded: " + passes.Count + " (" + legacy + " PassLegacy, " + (passes.Count - legacy) + " other GenPass subclasses), each wrapped in a HookPass");
    }

    // Only for the date-dependence experiment (env REFDUMP_FORCE_DATE=halloween0 | xmas1): forces Main.halloween / Main.xMas before every pass.
    // Without the variable this does nothing, so the normal dump is unaffected.
    static string _forceDate = Environment.GetEnvironmentVariable("REFDUMP_FORCE_DATE");
    public static void BeforePass(int idx)
    {
        if (string.IsNullOrEmpty(_forceDate)) return;
        if (_forceDate == "halloween0") { SSet(TMain, "halloween", false); SSet(TMain, "xMas", false); }
        else if (_forceDate == "xmas1") { SSet(TMain, "halloween", false); SSet(TMain, "xMas", true); }
        else if (_forceDate == "halloween1") { SSet(TMain, "halloween", true); SSet(TMain, "xMas", false); }
    }

    public static void AfterPass(int idx)
    {
        PassIndex = idx;
        // RunPass catches exceptions thrown inside a pass [WorldGenerator.cs:501-508], so a failing record would be silent: remember it
        try { WritePassRecord(idx, PassNames[idx], false, false); }
        catch (Exception e) { HookErrors.Add("pass " + idx + ": " + e); Console.WriteLine("HOOK ERROR pass " + idx + ": " + e); }
    }

    static object GetM(object o, string n)
    {
        FieldInfo f = o.GetType().GetField(n, AllInst); if (f != null) return f.GetValue(o);
        return o.GetType().GetProperty(n, AllInst).GetValue(o, null);
    }
    static void SetM(object o, string n, object v)
    {
        FieldInfo f = o.GetType().GetField(n, AllInst); if (f != null) { f.SetValue(o, v); return; }
        o.GetType().GetProperty(n, AllInst).SetValue(o, v, null);
    }

    static void InitSelectors()
    {
        TGenVars = T("Terraria.WorldBuilding.GenVars");
        WorldGenPrimStatics = TWorldGen.GetFields(AllStatic | BindingFlags.DeclaredOnly)
            .Where(f => !f.IsLiteral && !f.IsDefined(typeof(ThreadStaticAttribute), false) && (f.FieldType.IsPrimitive || f.FieldType.IsEnum)).OrderBy(f => f.Name, StringComparer.Ordinal).ToArray();
        string[] names = { "maxTilesX", "maxTilesY", "worldSurface", "worldSurfaceLow", "worldSurfaceHigh", "rockLayer", "rockLayerLow", "rockLayerHigh",
            "spawnTileX", "spawnTileY", "dungeonX", "dungeonY", "time", "dayTime", "moonPhase", "moonType", "GameMode", "hardMode",
            "treeX", "treeStyle", "caveBackX", "caveBackStyle", "iceBackStyle", "jungleBackStyle", "hellBackStyle", "snowBackStyle", "desertBackStyle",
            "oceanBackStyle", "mushroomBackStyle", "underworldBackStyle", "cloudBGActive", "slimeRainTime", "raining", "rainTime", "cloudAlpha", "maxRaining",
            "windSpeedCurrent", "windSpeedTarget", "numClouds", "bloodMoon", "eclipse", "halloween", "xMas", "netMode", "gameMenu", "dedServ",
            "drunkWorld", "getGoodWorld", "notTheBeesWorld", "noTrapsWorld", "tenthAnniversaryWorld", "dontStarveWorld", "remixWorld", "zenithWorld", "skyblockWorld",
            "worldName", "UnderworldLayer", "invasionType", "invasionSize", "numLavaBackground" };
        MainSel = new List<KeyValuePair<string, FieldInfo>>();
        foreach (string n in names)
        {
            FieldInfo f = TMain.GetField(n, AllStatic);
            if (f == null) { Console.WriteLine("note: Main." + n + " is not a static field (skipped)"); continue; }
            MainSel.Add(new KeyValuePair<string, FieldInfo>(n, f));
        }
    }

    static int CountNonNull(Array a) { int n = 0; foreach (object o in a) if (o != null) n++; return n; }

    static void WritePassRecord(int idx, string name, bool extra, bool forceSnap)
    {
        if (WorldGenPrimStatics == null) InitSelectors();
        var sw = Stopwatch.StartNew();
        Snap s = SnapBuf = TakeSnapshot(SnapBuf);
        var sb = new StringBuilder(8192);
        sb.Append("{\"row\":" + RowCount + ",\"i\":" + idx + ",\"name\":" + Q(name) + ",\"extra\":" + (extra ? "true" : "false"));
        sb.Append(",\"tiles_fnv64\":\"" + Hex(s.TilesFnv) + "\",\"cols_fnv64\":\"" + Hex(s.ColsFnv) + "\"");
        sb.Append(",\"n_active\":" + s.Active + ",\"n_wall\":" + s.Walls + ",\"n_liquid\":" + s.Liquid);
        object rand = SF(TMain, "rand");
        sb.Append(",\"rand\":" + RandJson(rand));
        if (!extra) PassNextRand.Add(RandNextPreview(rand));
        // key statics of Main
        sb.Append(",\"main\":{");
        bool first = true;
        foreach (var kv in MainSel)
        {
            if (!first) sb.Append(','); first = false;
            sb.Append(Q(kv.Key)).Append(':'); J(sb, kv.Value.GetValue(null), 0);
        }
        sb.Append("},\"worldgen_static\":{");
        first = true;
        foreach (FieldInfo f in WorldGenPrimStatics)
        {
            if (!first) sb.Append(','); first = false;
            sb.Append(Q(f.Name)).Append(':'); J(sb, f.GetValue(null), 0);
        }
        sb.Append("},\"genvars\":");
        sb.Append(StaticsJson(TGenVars, f => f.Name != "configuration" && f.Name != "tileSolid"));
        // entity counts
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
        PassLog.WriteLine(sb.ToString());
        PassLog.Flush();
        foreach (ulong h in s.ColHash) ColLog.Write(h);
        ColLog.Flush();
        bool snap = DoSnap && (forceSnap || idx == 0 || idx == 1 || idx % 10 == 0 || idx == PassTotal - 1);
        RowCount++;
        if (snap)
        {
            string f = idx < 0 ? "ref_small.before_pass_000.tfrm.z" : idx >= PassTotal ? "ref_small.after_generate_world.tfrm.z" : "ref_small.after_pass_" + idx.ToString("d3") + ".tfrm.z";
            WriteTfrm(Path.Combine(GenOut, f), s);
            SnapFiles.Add(f);
        }
        Console.WriteLine("pass " + idx.ToString("d3") + " " + name + "  tiles=" + Hex(s.TilesFnv) + (snap ? "  [snapshot]" : "") + "  (record " + sw.ElapsedMilliseconds + " ms)");
    }

    static void RunGen(string tag, string mode, string thread, string wldPath, string scratch)
    {
        var total = Stopwatch.StartNew();
        bool tickRand = Environment.GetEnvironmentVariable("REFDUMP_TICKRAND") == "1";
        // what LaunchInitializer / Main.autoCreate do for `-autocreate 1 -worldname ref_small -seed 12345 -world <path>` [LaunchInitializer.cs:127,160,173-183,203; Main.cs:5055-5078,5313]
        // They run after `new Main()` and before DedServ()/Initialize() in the real server, so they are passed as beforeInit.
        InitGame(() =>
        {
            Step("autoCreate(\"1\"): maxTilesX = 4200; maxTilesY = 1200; autoGen = true   [Main.cs:5062-5065]", () => { SSet(TMain, "maxTilesX", 4200); SSet(TMain, "maxTilesY", 1200); SSet(TMain, "autoGen", true); });
            Step("Main.worldName = \"ref_small\"   [Main.cs:5315]", () => SSet(TMain, "worldName", WorldNameConst));
            Step("new WorldFileData(path, false).SetAsActive(); Main.autoGenFileLocation = path   [LaunchInitializer.cs:181-182]", () =>
            {
                object wfd = Activator.CreateInstance(T("Terraria.IO.WorldFileData"), new object[] { wldPath, false });
                wfd.GetType().GetMethod("SetAsActive").Invoke(wfd, null);
                SSet(TMain, "autoGenFileLocation", wldPath);
            });
            Step("Main.AutogenSeedName = \"12345\"   [LaunchInitializer.cs:205]", () => SSet(TMain, "AutogenSeedName", SeedText));
        }, tickRand);
        Console.WriteLine("state: maxTilesX=" + SF(TMain, "maxTilesX") + " maxTilesY=" + SF(TMain, "maxTilesY") + " netMode=" + SF(TMain, "netMode") + " gameMenu=" + SF(TMain, "gameMenu") +
            " GameMode=" + SF(TMain, "GameMode") + " WorldGenParam_Evil=" + SF(TWorldGen, "WorldGenParam_Evil") + " IntPtr.Size=" + IntPtr.Size + " worldPathName=" + SF(TMain, "worldPathName"));

        string passesPath = Path.Combine(GenOut, "ref_small.passes.jsonl"), colsPath = Path.Combine(GenOut, "ref_small.columns.bin");
        if (mode == "passes")
        {
            PassLog = new StreamWriter(passesPath, false, new UTF8Encoding(false)) { NewLine = "\n" };
            ColLog = new BinaryWriter(new FileStream(colsPath, FileMode.Create));
        }
        Action body = () =>
        {
            if (mode == "passes") RunPasses(wldPath);
            else CallStatic(TWorldGen, "serverLoadWorldCallBack");
        };
        Exception err = null;
        if (thread == "task")
        {
            Task tk = Task.Factory.StartNew(() => { try { body(); } catch (Exception e) { err = e; } });
            tk.Wait();
        }
        else { try { body(); } catch (Exception e) { err = e; } }
        if (err != null) throw new Exception("generation failed: " + err.Message, err);
        if (HookErrors.Count > 0) throw new Exception("hook errors: " + HookErrors.Count + ": " + HookErrors[0]);
        if (PassLog != null) PassLog.Close();
        if (ColLog != null) ColLog.Close();

        // ---- summary
        byte[] wld = File.ReadAllBytes(wldPath);
        var sj = new StringBuilder("{\n");
        sj.Append("\"generator\":\"tools/refdump_worldgen (RefDumpWorldgen.exe gen)\",\n");
        sj.Append("\"tag\":" + Q(tag) + ",\"mode\":" + Q(mode) + ",\"thread\":" + Q(thread) + ",\n");
        sj.Append("\"seed_text\":" + Q(SeedText) + ",\"world_name\":" + Q(WorldNameConst) + ",\n");
        object ad = SF(TMain, "ActiveWorldFileData");
        sj.Append("\"seed_int\":" + IF(ad, "Seed") + ",\"world_id\":" + IF(ad, "WorldId") + ",\n");
        sj.Append("\"max_tiles_x\":" + SF(TMain, "maxTilesX") + ",\"max_tiles_y\":" + SF(TMain, "maxTilesY") + ",\"game_mode\":" + SF(TMain, "GameMode") + ",\n");
        sj.Append("\"worldgen_param_evil\":" + SF(TWorldGen, "WorldGenParam_Evil") + ",\"crimson\":" + ((bool)SF(TWorldGen, "crimson") ? "true" : "false") + ",\n");
        sj.Append("\"intptr_size\":" + IntPtr.Size + ",\n");
        sj.Append("\"run_date\":" + Q(DateTime.Now.ToString("yyyy-MM-dd")) + ",\"main_halloween\":" + ((bool)SF(TMain, "halloween") ? "true" : "false") + ",\"main_xmas\":" + ((bool)SF(TMain, "xMas") ? "true" : "false") + ",\"force_date_env\":" + Q(_forceDate) + ",\"tick_seeded_main_rand\":" + (Environment.GetEnvironmentVariable("REFDUMP_TICKRAND") == "1" ? "true" : "false") + ",\n");
        sj.Append("\"world_file\":" + Q(wldPath) + ",\"world_file_size\":" + wld.Length + ",\"world_file_sha256\":" + Q(Sha256(wld)) + ",\n");
        sj.Append("\"pass_count\":" + PassTotal + ",\n");
        sj.Append("\"pass_names\":[" + string.Join(",", PassNames.Select(Q).ToArray()) + "],\n");
        sj.Append("\"pass_classes\":[" + string.Join(",", PassTypes.Select(Q).ToArray()) + "],\n");
        sj.Append("\"snapshots\":[" + string.Join(",", SnapFiles.Select(Q).ToArray()) + "],\n");
        // the game's own manifest: per-pass RandNext written by WorldGenerator.RunPass [WorldGenerator.cs:515], Skipped flags
        var manifest = new StringBuilder();
        int mism = 0, mcount = 0;
        if (mode == "passes")
        {
            object man = SF(TWorldGen, "Manifest");
            IList results = (IList)IF(man, "GenPassResults");
            mcount = results.Count;
            for (int i = 0; i < results.Count; i++)
            {
                object r = results[i];
                int rn = (int)r.GetType().GetProperty("RandNext").GetValue(r, null);
                bool skipped = (bool)r.GetType().GetProperty("Skipped").GetValue(r, null);
                if (!skipped && i < PassNextRand.Count && rn != PassNextRand[i]) mism++;
                if (i > 0) manifest.Append(',');
                object hash = r.GetType().GetProperty("Hash").GetValue(r, null);
                manifest.Append("{\"name\":" + Q((string)r.GetType().GetProperty("Name").GetValue(r, null)) + ",\"skipped\":" + (skipped ? "true" : "false") + ",\"randNext\":" + rn + ",\"game_hash\":" + (hash == null ? "null" : Convert.ToString(hash, Inv)) + "}");
            }
        }
        sj.Append("\"manifest_pass_results\":" + mcount + ",\"manifest_randnext_mismatches_vs_harness_preview\":" + mism + ",\n");
        sj.Append("\"manifest\":[" + manifest + "],\n");
        sj.Append("\"trace_lines\":[" + string.Join(",", TraceLines.Where(l => l != null && l.Trim().Length > 0).Take(400).Select(Q).ToArray()) + "],\n");
        sj.Append("\"total_ms\":" + total.ElapsedMilliseconds + "\n}\n");
        File.WriteAllBytes(Path.Combine(GenOut, "ref_small.summary." + tag + ".json"), new UTF8Encoding(false).GetBytes(sj.ToString()));
        Console.WriteLine("world file " + wld.Length + " bytes sha256 " + Sha256(wld));
        Console.WriteLine("manifest RandNext mismatches vs preview: " + mism);
    }
}
