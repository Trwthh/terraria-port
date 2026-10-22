// RefDumpTicks: runs the real dedicated server's per-tick simulation (Main.Update, the method the server loop calls) headless, with no network,
// on a scratch copy of ref_small.wld, and dumps compact per-tick records.
//
//   RefDumpTicks.exe world  <outdir> [ticks=3600]    scenario 1: zero players (what the server runs once a client has joined, minus the client)
//   RefDumpTicks.exe player <outdir> [ticks=600]     scenario 2: one scripted remote player (Main.player[0]) driven through its control flags
//
// Call sequence (every call is the game's own code, by reflection; source line references are to decompile\Terraria\Terraria):
//   Program.cs:219,223   new Main(); Lang.InitializeLegacyLocalization()
//   LaunchInitializer.cs:173-178 / Main.cs:5308   `-world <existing file>`: WorldFile.GetAllMetadata(path, false).SetAsActive()   (before Initialize, as the real server)
//   Main.cs:5375,5392-5394  Main.dedServ = true; Initialize()  (Initialize ends with netMode = 2 for dedServ [Main.cs:6640-6642] and creates Main._rngs [6653])
//   Main.cs:5853 / WorldGen.cs:6903-6906,6840  Task.Factory.StartNew(WorldGen.serverLoadWorldCallBack) -> WorldFile.LoadWorld()   (as the real server: thread-pool thread)
//   Netplay.cs:249-263 InitializeServer WITHOUT its socket part (no TcpListener, no StartListening, no UPnP, no Netplay.StartServer, no ServerLoop thread):
//                        Main.myPlayer = 255; Main.netMode = 2; Netplay.Disconnect = false; Netplay.Clients[i] = new RemoteClient{Id=i, ReadBuffer=new byte[1024]} (Reset())
//   Main.cs:5905-5906   Netplay.StartServer(); Main.gameMenu = false   (StartServer is NOT called here)
//   pins                Main.rand = new UnifiedRandom(RandSeed); every other unseeded UnifiedRandom reachable from the statics is re-seeded (see RngPins)
//   per tick            Main.Update(new GameTime())   [Main.cs:5923 is the server loop's call; Update = Main.cs:16985 -> DoUpdate 17039 -> DoUpdateInWorld 17942]
// The real loop calls Update only while Netplay.HasFullyConnectedClients [Main.cs:5920-5923]; with no client it only calls Netplay.UpdateInMainThread() and nothing else
// advances, so this harness stands for "a server with a client that has fully joined".
using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

public static partial class WG
{
    const int RandSeed = 424242;      // Main.rand (the object Main.cs:16000+ uses outside SwapRandom: weather, ...)
    const int LoadSeed = 1357;        // Main.rand of the thread that runs LoadWorld (thread-static!)
    const int PinSeedBase = 9000;     // other unseeded UnifiedRandom instances found by the scan
    const string WorldName = "ref_small";
    static MethodInfo UpdateM;
    static Type GameTimeT;
    static string TOut;
    static StreamWriter RecLog, RandLog, PlayerLog;
    static BinaryWriter ColLog;
    static Snap Cur, Initial;
    static List<string> Notes = new List<string>();
    static List<string> RngPins = new List<string>();
    static Dictionary<string, string> Main0;
    static FieldInfo[] WGPrim, NpcPrim, MainPrim;
    static List<KeyValuePair<string, FieldInfo>> MainSel;
    static List<KeyValuePair<string, FieldInfo>> LiquidSel;
    static int RecCount = 0, windResets = 0;

    [STAThread]
    static int Main(string[] args)
    {
        if (args.Length < 2) { Console.Error.WriteLine("usage: RefDumpTicks.exe world|player <outdir> [ticks]"); return 2; }
        Root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(Root, "work", "refserver");
        string mode = args[0];
        TOut = Dir(Path.GetFullPath(args[1]));
        int ticks = args.Length > 2 ? int.Parse(args[2]) : (mode == "world" ? 3600 : 600);
        string scratch = Dir(Path.Combine(Root, "work", "ticks-scratch", mode + "_" + new DirectoryInfo(TOut).Name));   // one scratch folder per output folder, so runs can go in parallel
        SaveDir = Dir(Path.Combine(scratch, "save"));
        Watchdog(480);
        int rc = 0;
        try { Run(mode, ticks, scratch); }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }

    static void Run(string mode, int ticks, string scratch)
    {
        var total = Stopwatch.StartNew();
        string world = Path.Combine(Dir(Path.Combine(scratch, "Worlds")), WorldName + ".wld");
        File.Copy(Path.Combine(Root, "work", "refsave", "Worlds", WorldName + ".wld"), world, true);
        string srcSha = Sha256(File.ReadAllBytes(world));

        InitGame(() =>
        {
            Step("-world <existing>: WorldFile.GetAllMetadata(path, false).SetAsActive()   [LaunchInitializer.cs:175-178, Main.cs:5308-5311]", () =>
            {
                object wfd = null;
                foreach (MethodInfo m in TWorldFile.GetMethods(AllStatic))
                    if (m.Name == "GetAllMetadata" && m.GetParameters().Length == 2) wfd = Unwrap(m, null, new object[] { world, false });
                if (wfd == null) throw new Exception("GetAllMetadata returned null");
                wfd.GetType().GetMethod("SetAsActive").Invoke(wfd, null);
            });
        }, false);
        Console.WriteLine("netMode after Initialize = " + SF(TMain, "netMode") + ", dedServ = " + SF(TMain, "dedServ") + ", gameMenu = " + SF(TMain, "gameMenu") +
            ", Liquid.cycles/curMaxLiquid before load = " + SF(T("Terraria.Liquid"), "cycles") + "/" + SF(T("Terraria.Liquid"), "curMaxLiquid"));

        Step("Task.Factory.StartNew(WorldGen.serverLoadWorldCallBack)  -> WorldFile.LoadWorld()   [WorldGen.cs:6903-6906, 6840]", () =>
        {
            Exception err = null;
            // Main.rand is [ThreadStatic] [Main.cs:1367]: the thread-pool thread that runs the load starts with Main.rand == null, so LoadWorld creates
            // `new UnifiedRandom((int)DateTime.Now.Ticks)` for it [WorldFile.cs:652-655] and everything the load randomises (NPC.setFireFlyChance
            // [WorldFile.cs:774], weather counters, Main.cloudBGActive, ...) differs on every real server start. Pinned here.
            Task.Factory.StartNew(() => { try { SSet(TMain, "rand", NewRand(LoadSeed)); CallStatic(TWorldGen, "serverLoadWorldCallBack"); } catch (Exception e) { err = e; } }).Wait();
            if (err != null) throw new Exception("load failed: " + err);
            if ((bool)SF(TWorldGen, "loadFailed")) throw new Exception("WorldGen.loadFailed after serverLoadWorldCallBack");
        });
        Type liquidT = T("Terraria.Liquid"), lbT = T("Terraria.LiquidBuffer");
        Console.WriteLine("loaded " + SF(TMain, "maxTilesX") + "x" + SF(TMain, "maxTilesY") + " seed=" + IF(SF(TMain, "ActiveWorldFileData"), "Seed") + " time=" + SF(TMain, "time") + " dayTime=" + SF(TMain, "dayTime") +
            " Liquid.numLiquid=" + SF(liquidT, "numLiquid") + " cycles/curMaxLiquid after load = " + SF(liquidT, "cycles") + "/" + SF(liquidT, "curMaxLiquid"));

        // ---- server state without the socket part of Netplay.InitializeServer [Netplay.cs:249-263]
        Step("server state: myPlayer = 255; netMode = 2; Netplay.Disconnect = false; Netplay.Clients[0..255] = new RemoteClient   [Netplay.cs:254-263]", () =>
        {
            Type netplay = T("Terraria.Netplay"), rc = T("Terraria.RemoteClient");
            SSet(TMain, "myPlayer", 255); SSet(TMain, "netMode", 2); SSet(netplay, "Disconnect", false);
            Array clients = (Array)SF(netplay, "Clients");
            for (int i = 0; i < 256; i++)
            {
                object c = Activator.CreateInstance(rc);
                rc.GetMethod("Reset").Invoke(c, null);
                rc.GetField("Id").SetValue(c, i);
                rc.GetField("ReadBuffer").SetValue(c, new byte[1024]);
                clients.SetValue(c, i);
            }
        });
        Step("Main.gameMenu = false   [Main.cs:5906]", () => SSet(TMain, "gameMenu", false));

        foreach (MethodInfo m in TMain.GetMethods(AllInst | BindingFlags.DeclaredOnly))
            if (m.Name == "Update" && m.GetParameters().Length == 1 && m.GetParameters()[0].ParameterType.Name == "GameTime") { UpdateM = m; GameTimeT = m.GetParameters()[0].ParameterType; }
        if (UpdateM == null) throw new Exception("Main.Update(GameTime) not found");

        // ---- pin every random generator we can find
        SSet(TMain, "rand", NewRand(RandSeed));
        PinRandoms();
        // GUID-seeded statics: Main.UnpausedUpdateSeed = (ulong)Guid.NewGuid().GetHashCode() [Main.cs:6539, advanced once per tick at 17443, read nowhere else],
        // Main.TileFrameSeed = (ulong)Guid.NewGuid().GetHashCode() [Main.cs:420, only read by tile drawing / flame particles; advanced in Draw]. Neither reaches the tiles or entities.
        SSet(TMain, "UnpausedUpdateSeed", 123456789UL);
        SSet(TMain, "TileFrameSeed", 987654321UL);
        RngPins.Add("Main.rand on the LoadWorld thread-pool thread = new UnifiedRandom(" + LoadSeed + ") (was DateTime.Now.Ticks, WorldFile.cs:652-655; Main.rand is [ThreadStatic])");
        // Main.ResetWindCounter [Main.cs:59674-59681] draws windCounter / extremeWindCounter from FastRandom.CreateWithRandomSeed() = new FastRandom(Guid.NewGuid().GetHashCode())
        // [FastRandom.cs:37-40]. It runs in WorldGen.clearWorld during the load [WorldGen.cs:6926] and again inside UpdateWeather whenever windCounter reaches 0 [Main.cs:59842-59918].
        // There is no seed to set, so the values are overwritten: after the load, and after any tick in which windCounter went up instead of down.
        SSet(TMain, "windCounter", 1500); SSet(TMain, "extremeWindCounter", 25);
        RngPins.Add("Main.windCounter = 1500, Main.extremeWindCounter = 25 after the load (were FastRandom(Guid.NewGuid().GetHashCode()) draws, Main.ResetWindCounter)");
        RngPins.Add("Main.windCounter after each ResetWindCounter inside a tick = 900 + (n * 773) % 1801 (n-th reset; the game draws FastRandom(Guid) 900..2700)");
        RngPins.Add("Terraria.Main.UnpausedUpdateSeed = 123456789 (was Guid.NewGuid().GetHashCode())");
        RngPins.Add("Terraria.Main.TileFrameSeed = 987654321 (was Guid.NewGuid().GetHashCode())");
        InitSelectors2();

        RecLog = new StreamWriter(Path.Combine(TOut, mode + ".ticks.jsonl"), false, new UTF8Encoding(false)) { NewLine = "\n" };
        RandLog = new StreamWriter(Path.Combine(TOut, mode + ".rand_per_tick.jsonl"), false, new UTF8Encoding(false)) { NewLine = "\n" };
        ColLog = new BinaryWriter(new FileStream(Path.Combine(TOut, mode + ".columns.bin"), FileMode.Create));
        if (mode == "player") PlayerLog = new StreamWriter(Path.Combine(TOut, "player.per_tick.jsonl"), false, new UTF8Encoding(false)) { NewLine = "\n" };

        Initial = TakeSnapshot(null);
        Cur = TakeSnapshot(null);
        Main0 = MainStatics();
        if (mode == "player") SetupPlayer();

        WriteRecord(0, true);   // state right after load, before any tick
        var tickClock = Stopwatch.StartNew();
        Exception tickErr = null;
        for (int t = 1; t <= ticks; t++)
        {
            if (mode == "player") ScriptPlayer(t);
            int windBefore = (int)SF(TMain, "windCounter");
            try { UpdateM.Invoke(MainInst, new object[] { Activator.CreateInstance(GameTimeT) }); }
            catch (TargetInvocationException e) { tickErr = e.InnerException; Console.WriteLine("TICK " + t + " THREW " + e.InnerException); break; }
            int windAfter = (int)SF(TMain, "windCounter");
            if (windAfter > windBefore)
            {
                int pinned = 900 + (windResets++ * 773) % 1801;
                SSet(TMain, "windCounter", pinned);
                Notes.Add("tick " + t + ": Main.ResetWindCounter ran (the game drew windCounter from FastRandom(Guid)), windCounter replaced by " + pinned);
            }
            if (t <= 120) WriteRandRecord(t);
            if (mode == "player") WritePlayerRecord(t);
            if (t == 1 || t == 2 || t == 3 || t == 10 || t % 60 == 0 || t == ticks) WriteRecord(t, t <= 3 || t == ticks);
            if (t % 600 == 0) Console.WriteLine("tick " + t + "  (" + tickClock.ElapsedMilliseconds + " ms)");
        }
        RecLog.Close(); RandLog.Close(); ColLog.Close(); if (PlayerLog != null) PlayerLog.Close();
        long tickMs = tickClock.ElapsedMilliseconds;

        // ---- final tile diff vs the loaded world, and statics diff
        Cur = TakeSnapshot(Cur);
        WriteTileChanges(Path.Combine(TOut, mode + ".tile_changes.bin"));
        var main1 = MainStatics();
        var diffs = new StringBuilder("{");
        bool first = true;
        foreach (var kv in main1)
        {
            string old;
            if (!Main0.TryGetValue(kv.Key, out old) || old != kv.Value)
            {
                if (!first) diffs.Append(','); first = false;
                diffs.Append(Q(kv.Key) + ":[" + Q(old) + "," + Q(kv.Value) + "]");
            }
        }
        diffs.Append("}");
        File.WriteAllText(Path.Combine(TOut, mode + ".main_statics_changed.json"), diffs.ToString() + "\n", new UTF8Encoding(false));

        var sj = new StringBuilder("{\n");
        sj.Append("\"generator\":\"tools/refdump_ticks (RefDumpTicks.exe " + mode + ")\",\n");
        sj.Append("\"mode\":" + Q(mode) + ",\"ticks_requested\":" + ticks + ",\"tick_error\":" + (tickErr == null ? "null" : Q(tickErr.ToString())) + ",\n");
        sj.Append("\"world_sha256\":" + Q(srcSha) + ",\"records\":" + RecCount + ",\n");
        sj.Append("\"main_rand_seed\":" + RandSeed + ",\"world_seed\":" + IF(SF(TMain, "ActiveWorldFileData"), "Seed") + ",\n");
        sj.Append("\"rng_pins\":[" + string.Join(",", RngPins.Select(Q).ToArray()) + "],\n");
        sj.Append("\"notes\":[" + string.Join(",", Notes.Select(Q).ToArray()) + "],\n");
        sj.Append("\"intptr_size\":" + IntPtr.Size + ",\"main_halloween\":" + ((bool)SF(TMain, "halloween") ? "true" : "false") + ",\"main_xmas\":" + ((bool)SF(TMain, "xMas") ? "true" : "false") + ",\n");
        sj.Append("\"ticks_run\":" + (tickErr == null ? ticks : -1) + "\n}\n");   // wall-clock timings go to <mode>.timing.json so that the dump files are byte-identical between runs
        File.WriteAllText(Path.Combine(TOut, mode + ".timing.json"), "{\"tick_ms_total\":" + tickMs + ",\"total_ms\":" + total.ElapsedMilliseconds + "}\n", new UTF8Encoding(false));
        File.WriteAllText(Path.Combine(TOut, mode + ".summary.json"), sj.ToString(), new UTF8Encoding(false));
        Console.WriteLine(sj.ToString());
    }

    // ------------------------------------------------------------ RNG pinning
    // Walks the object graph reachable from every static field of every Terraria type (depth <= 3, no arrays/collections) and re-seeds each UnifiedRandom
    // found (SetSeed is the game's own method). Main.rand is set separately; the Main._rngs streams are seeded by the game from the world seed (Main.cs:3393).
    static void PinRandoms()
    {
        Type ur = T("Terraria.Utilities.UnifiedRandom"), fr = T("Terraria.Utilities.FastRandom");
        var seen = new HashSet<object>(new RefEq());
        int n = 0;
        object mainRand = SF(TMain, "rand");
        object rngs = TMain.GetField("_rngs", AllStatic).GetValue(null);
        Action<object, string, int> visit = null;
        visit = (o, path, depth) =>
        {
            if (o == null || depth > 3) return;
            Type t = o.GetType();
            if (t.IsPrimitive || t.IsEnum || o is string || o is Delegate || o is Array || o is IEnumerable) return;
            if (!seen.Add(o)) return;
            if (t == ur)
            {
                if (ReferenceEquals(o, mainRand)) return;
                if (rngs != null && ((IDictionary)rngs).Values.Cast<object>().Any(v => ReferenceEquals(v, o))) return;
                int seed = PinSeedBase + n++;
                t.GetMethod("SetSeed").Invoke(o, new object[] { seed });
                RngPins.Add(path + " = UnifiedRandom pinned with SetSeed(" + seed + ")");
                return;
            }
            if (!(t.Namespace ?? "").StartsWith("Terraria") && !(t.IsValueType)) return;
            foreach (FieldInfo f in FieldsOf(t))
            {
                if (f.FieldType.IsPrimitive || f.FieldType == typeof(string)) continue;
                object v; try { v = f.GetValue(o); } catch { continue; }
                visit(v, path + "." + f.Name, depth + 1);
            }
        };
        foreach (Type t in G.GetTypes())
        {
            if (!(t.Namespace ?? "").StartsWith("Terraria")) continue;
            if (t.IsGenericTypeDefinition) continue;
            foreach (FieldInfo f in t.GetFields(AllStatic | BindingFlags.DeclaredOnly))
            {
                if (f.IsLiteral || f.FieldType.IsPrimitive || f.FieldType.IsEnum) continue;
                if (f.FieldType.IsArray || (typeof(IEnumerable).IsAssignableFrom(f.FieldType) && f.FieldType != typeof(string))) continue;
                object v; try { v = f.GetValue(null); } catch { continue; }
                visit(v, t.FullName + "." + f.Name, 0);
            }
        }
        // FastRandom instances are structs / created per use; list the static holders only
        Console.WriteLine("RNG pins: " + RngPins.Count);
        foreach (string s in RngPins) Console.WriteLine("  " + s);
    }
    class RefEq : IEqualityComparer<object>
    {
        public new bool Equals(object a, object b) { return ReferenceEquals(a, b); }
        public int GetHashCode(object o) { return System.Runtime.CompilerServices.RuntimeHelpers.GetHashCode(o); }
    }

    // ------------------------------------------------------------ selectors
    static void InitSelectors2()
    {
        WGPrim = TWorldGen.GetFields(AllStatic | BindingFlags.DeclaredOnly).Where(f => !f.IsLiteral && !f.IsDefined(typeof(ThreadStaticAttribute), false) && (f.FieldType.IsPrimitive || f.FieldType.IsEnum)).OrderBy(f => f.Name, StringComparer.Ordinal).ToArray();
        NpcPrim = T("Terraria.NPC").GetFields(AllStatic | BindingFlags.DeclaredOnly).Where(f => !f.IsLiteral && (f.FieldType.IsPrimitive || f.FieldType.IsEnum)).OrderBy(f => f.Name, StringComparer.Ordinal).ToArray();
        MainPrim = TMain.GetFields(AllStatic | BindingFlags.DeclaredOnly).Where(f => !f.IsLiteral && !f.IsDefined(typeof(ThreadStaticAttribute), false) && (f.FieldType.IsPrimitive || f.FieldType.IsEnum)).OrderBy(f => f.Name, StringComparer.Ordinal).ToArray();
        string[] mn = { "time", "dayTime", "moonPhase", "moonType", "bloodMoon", "eclipse", "raining", "rainTime", "maxRaining", "cloudAlpha", "windSpeedCurrent", "windSpeedTarget",
            "numClouds", "windCounter", "extremeWindCounter", "weatherCounter", "cloudBGActive", "slimeRain", "slimeRainTime", "slimeWarningTime", "slimeWarningDelay", "invasionType", "invasionSize", "invasionDelay", "invasionProgress", "invasionProgressWave",
            "invasionX", "invasionWarn", "hardMode", "checkForSpawns", "dayRate", "sundialCooldown", "moondialCooldown", "fastForwardTimeToDawn", "fastForwardTimeToDusk",
            "worldSurface", "rockLayer", "spawnTileX", "spawnTileY", "dungeonX", "dungeonY", "netPlayCounter", "ambientCounter", "_gameUpdateCount", "cloudLimit", "halloween", "xMas",
            "bgDelay", "bgStyle", "caveBackX", "caveBackStyle", "treeX", "treeStyle", "iceBackStyle", "jungleBackStyle", "hellBackStyle", "snowBackStyle", "desertBackStyle", "oceanBackStyle", "mushroomBackStyle" };
        MainSel = new List<KeyValuePair<string, FieldInfo>>();
        foreach (string n in mn)
        {
            FieldInfo f = TMain.GetField(n, AllStatic);
            if (f == null) { Notes.Add("Main." + n + " is not a static field; not in the per-record list"); continue; }
            MainSel.Add(new KeyValuePair<string, FieldInfo>(n, f));
        }
        Type liquidT = T("Terraria.Liquid"), lbT = T("Terraria.LiquidBuffer");
        LiquidSel = new List<KeyValuePair<string, FieldInfo>>();
        foreach (string n in new[] { "numLiquid", "wetCounter", "panicCounter", "stuckCount", "stuckAmount", "quickSettle", "quickFall", "cycles", "curMaxLiquid", "skipCount", "stuck", "panicMode", "panicY" })
        {
            FieldInfo f = liquidT.GetField(n, AllStatic);
            if (f != null) LiquidSel.Add(new KeyValuePair<string, FieldInfo>("Liquid." + n, f)); else Notes.Add("Liquid." + n + " missing");
        }
        FieldInfo nlb = lbT.GetField("numLiquidBuffer", AllStatic);
        if (nlb != null) LiquidSel.Add(new KeyValuePair<string, FieldInfo>("LiquidBuffer.numLiquidBuffer", nlb));
    }

    static Dictionary<string, string> MainStatics()
    {
        var d = new Dictionary<string, string>();
        foreach (FieldInfo f in MainPrim)
        {
            object v; try { v = f.GetValue(null); } catch { continue; }
            d[f.Name] = JStr(v);
        }
        return d;
    }

    // ------------------------------------------------------------ records
    static readonly string[] NpcFields = { "type", "netID", "position", "velocity", "ai", "localAI", "life", "lifeMax", "direction", "directionY", "target", "homeTileX", "homeTileY", "homeless",
        "spriteDirection", "rotation", "timeLeft", "wet", "lavaWet", "honeyWet", "width", "height", "aiStyle", "friendly", "townNPC", "shimmerTransparency", "oldTarget", "knockBackResist" };
    static readonly string[] ProjFields = { "type", "position", "velocity", "ai", "localAI", "owner", "timeLeft", "damage", "penetrate", "aiStyle", "friendly", "hostile", "wet", "rotation" };
    static readonly string[] ItemFields = { "type", "stack", "position", "velocity", "timeSinceItemSpawned", "playerIndexTheItemIsReservedFor", "wet", "shimmered" };
    static readonly string[] PlayerFields = { "position", "velocity", "oldPosition", "oldVelocity", "direction", "jump", "wingTime", "rocketTime", "fallStart", "fallStart2", "gravDir", "wet", "lavaWet", "honeyWet",
        "statLife", "statMana", "controlLeft", "controlRight", "controlUp", "controlDown", "controlJump", "releaseJump", "canJumpAgain", "justJumped", "sliding", "slideDir", "fullRotation", "headPosition",
        "bodyFrame", "legFrame", "breath", "grappling", "mount", "oldDirection", "maxFallSpeed", "moveSpeed", "jumpSpeed", "runAcceleration", "maxRunSpeed", "runSlowdown", "gravity", "dead", "active", "ghost", "pulley",
        "isSolidTileUnderPlayer", "autoJump", "velocityPlayerCollision" };

    static string EntityJson(object o, string[] fields)
    {
        var sb = new StringBuilder("{");
        bool first = true;
        foreach (string n in fields)
        {
            object v;
            try { v = IF(o, n); } catch { continue; }
            if (!first) sb.Append(','); first = false;
            sb.Append(Q(n)).Append(':'); J(sb, v, 1);
        }
        return sb.Append('}').ToString();
    }

    static string EntitiesJson(string arrName, string[] fields, out ulong hash, out int count, bool full, Func<object, bool> active)
    {
        Array a = (Array)SF(TMain, arrName);
        var all = new StringBuilder();
        var list = new StringBuilder("[");
        count = 0;
        for (int i = 0; i < a.Length; i++)
        {
            object o = a.GetValue(i);
            if (o == null || !active(o)) continue;
            string js = EntityJson(o, fields);
            all.Append(i).Append(':').Append(js).Append('\n');
            if (count > 0) list.Append(',');
            list.Append("{\"index\":" + i + ",\"v\":" + js + "}");
            count++;
        }
        list.Append(']');
        byte[] b = Encoding.UTF8.GetBytes(all.ToString());
        hash = Fnv(b, 0, b.Length, FnvBasis);
        return full ? list.ToString() : "null";
    }

    static string RandAll()
    {
        var sb = new StringBuilder("{\"main_rand\":" + RandJson(SF(TMain, "rand")) + ",\"rngs\":{");
        object rngs = TMain.GetField("_rngs", AllStatic).GetValue(null);
        bool first = true;
        foreach (string k in ((IDictionary)rngs).Keys.Cast<string>().OrderBy(x => x, StringComparer.Ordinal))
        {
            if (!first) sb.Append(','); first = false;
            sb.Append(Q(k)).Append(':').Append(RandJson(((IDictionary)rngs)[k]));
        }
        return sb.Append("}}").ToString();
    }

    static void WriteRandRecord(int tick)
    {
        RandLog.WriteLine("{\"tick\":" + tick + "," + RandAll().Substring(1));
    }

    static void WriteRecord(int tick, bool fullEntities)
    {
        Cur = TakeSnapshot(Cur);
        var sb = new StringBuilder(16384);
        sb.Append("{\"row\":" + RecCount + ",\"tick\":" + tick);
        sb.Append(",\"tiles_fnv64\":\"" + Hex(Cur.TilesFnv) + "\",\"cols_fnv64\":\"" + Hex(Cur.ColsFnv) + "\"");
        sb.Append(",\"n_active\":" + Cur.Active + ",\"n_wall\":" + Cur.Walls + ",\"n_liquid\":" + Cur.Liquid);
        // changed tiles vs the loaded world
        long changed = 0; int fx = -1, fy = -1;
        int h = Cur.H;
        for (long i = 0; i < (long)Cur.W * h; i++)
        {
            long o = i * RecSize;
            bool same = true;
            for (int k = 0; k < RecSize; k++) if (Cur.Buf[o + k] != Initial.Buf[o + k]) { same = false; break; }
            if (!same) { if (changed == 0) { fx = (int)(i / h); fy = (int)(i % h); } changed++; }
        }
        sb.Append(",\"tiles_changed_vs_load\":" + changed + ",\"first_changed\":" + (changed == 0 ? "null" : "[" + fx + "," + fy + "]"));
        sb.Append(",\"main\":{");
        bool first = true;
        foreach (var kv in MainSel) { if (!first) sb.Append(','); first = false; sb.Append(Q(kv.Key)).Append(':'); J(sb, kv.Value.GetValue(null), 0); }
        sb.Append("},\"liquid\":{");
        first = true;
        foreach (var kv in LiquidSel) { if (!first) sb.Append(','); first = false; sb.Append(Q(kv.Key)).Append(':'); J(sb, kv.Value.GetValue(null), 0); }
        sb.Append("},\"worldgen_static\":{");
        first = true;
        foreach (FieldInfo f in WGPrim) { if (!first) sb.Append(','); first = false; sb.Append(Q(f.Name)).Append(':'); J(sb, f.GetValue(null), 0); }
        sb.Append("},\"npc_static\":{");
        first = true;
        foreach (FieldInfo f in NpcPrim) { if (!first) sb.Append(','); first = false; sb.Append(Q(f.Name)).Append(':'); J(sb, f.GetValue(null), 0); }
        sb.Append("},\"rand\":" + RandAll());
        ulong hn, hi, hp; int cn, ci, cp;
        string npcs = EntitiesJson("npc", NpcFields, out hn, out cn, fullEntities, o => (bool)IF(o, "active"));
        string items = EntitiesJson("item", ItemFields, out hi, out ci, fullEntities, o => (bool)IF(o, "active"));
        string projs = EntitiesJson("projectile", ProjFields, out hp, out cp, fullEntities, o => (bool)IF(o, "active"));
        Array chests = (Array)SF(TMain, "chest");
        int nChest = 0; foreach (object c in chests) if (c != null) nChest++;
        object teById = SF(T("Terraria.DataStructures.TileEntity"), "ByID");
        sb.Append(",\"counts\":{\"npcs\":" + cn + ",\"items\":" + ci + ",\"projectiles\":" + cp + ",\"chests\":" + nChest + ",\"tile_entities\":" + ((IDictionary)teById).Count + "}");
        sb.Append(",\"npcs_fnv64\":\"" + Hex(hn) + "\",\"items_fnv64\":\"" + Hex(hi) + "\",\"projectiles_fnv64\":\"" + Hex(hp) + "\"");
        if (fullEntities) sb.Append(",\"npcs\":" + npcs + ",\"items\":" + items + ",\"projectiles\":" + projs);
        sb.Append("}");
        RecLog.WriteLine(sb.ToString());
        RecLog.Flush();
        foreach (ulong c in Cur.ColHash) ColLog.Write(c);
        ColLog.Flush();
        RecCount++;
    }

    static void WriteTileChanges(string path)
    {
        var chg = new List<int>();
        int h = Cur.H;
        for (long i = 0; i < (long)Cur.W * h; i++)
        {
            long o = i * RecSize;
            for (int k = 0; k < RecSize; k++) if (Cur.Buf[o + k] != Initial.Buf[o + k]) { chg.Add((int)i); break; }
        }
        if (chg.Count > 2000000) { Notes.Add("tile change list not written: " + chg.Count + " tiles"); return; }
        using (var bw = new BinaryWriter(new FileStream(path, FileMode.Create)))
        {
            bw.Write(new byte[] { (byte)'T', (byte)'C', (byte)'H', (byte)'G' });
            bw.Write(1u); bw.Write(Cur.W); bw.Write(Cur.H); bw.Write(chg.Count);
            foreach (int i in chg)
            {
                bw.Write(i / h); bw.Write(i % h);
                bw.Write(Initial.Buf, i * RecSize, RecSize);
                bw.Write(Cur.Buf, i * RecSize, RecSize);
            }
        }
        Console.WriteLine("tile changes vs load: " + chg.Count);
        Notes.Add("tile changes vs load at the last tick: " + chg.Count);
    }

    // ------------------------------------------------------------ scenario 2: one scripted remote player
    static object P0;
    static void SetupPlayer()
    {
        Array players = (Array)SF(TMain, "player");
        P0 = players.GetValue(0);
        Type pt = P0.GetType();
        int sx = (int)SF(TMain, "spawnTileX"), sy = (int)SF(TMain, "spawnTileY");
        Notes.Add("world spawn tile = (" + sx + "," + sy + ")");
        // A remote player as the server sees it after its client has joined: active, alive, at the spawn, controls arriving per tick (message 13)
        IF_Set(P0, "active", true);
        IF_Set(P0, "name", "Ref");
        IF_Set(P0, "whoAmI", 0);
        IF_Set(P0, "statLifeMax", 100); IF_Set(P0, "statLifeMax2", 100); IF_Set(P0, "statLife", 100);
        IF_Set(P0, "statManaMax", 20); IF_Set(P0, "statManaMax2", 20); IF_Set(P0, "statMana", 20);
        try
        {
            Type ctx = T("Terraria.PlayerSpawnContext");
            object ev = Enum.ToObject(ctx, 0);
            foreach (object v in Enum.GetValues(ctx)) if (v.ToString() == "SpawningIntoWorld") ev = v;
            pt.GetMethod("Spawn").Invoke(P0, new object[] { ev });
            Notes.Add("Player.Spawn(SpawningIntoWorld) worked");
        }
        catch (Exception e)
        {
            Notes.Add("Player.Spawn failed (" + (e.InnerException ?? e).GetType().Name + ": " + (e.InnerException ?? e).Message + "); position set by hand");
            int w = (int)IF(P0, "width"), hh = (int)IF(P0, "height");
            object pos = Activator.CreateInstance(IF(P0, "position").GetType(), new object[] { (float)(sx * 16 + 8 - w / 2), (float)(sy * 16 - hh) });
            IF_Set(P0, "position", pos);
        }
        IF_Set(P0, "active", true);
        IF_Set(P0, "dead", false);
        Console.WriteLine("player 0 at " + JStr(IF(P0, "position")) + " size " + IF(P0, "width") + "x" + IF(P0, "height"));
    }
    static void IF_Set(object o, string name, object v)
    {
        for (Type t = o.GetType(); t != null; t = t.BaseType)
        {
            FieldInfo f = t.GetField(name, AllInst | BindingFlags.DeclaredOnly);
            if (f != null) { f.SetValue(o, f.FieldType.IsPrimitive ? Convert.ChangeType(v, f.FieldType) : v); return; }
            PropertyInfo p = t.GetProperty(name, AllInst | BindingFlags.DeclaredOnly);
            if (p != null && p.CanWrite) { p.SetValue(o, v, null); return; }
        }
        throw new Exception("no member " + name);
    }

    // Controls for tick t (1-based): right 1-120, right+jump held 121-150, idle 151-200, jump pulses at 201,211,221 (one tick each), left 241-360, left+jump held 361-420, idle after.
    static void ScriptPlayer(int t)
    {
        bool left = (t >= 241 && t <= 420), right = (t >= 1 && t <= 150);
        bool jump = (t >= 121 && t <= 150) || t == 201 || t == 211 || t == 221 || (t >= 361 && t <= 420);
        IF_Set(P0, "controlLeft", left); IF_Set(P0, "controlRight", right); IF_Set(P0, "controlJump", jump);
        IF_Set(P0, "controlUp", false); IF_Set(P0, "controlDown", false);
    }

    static void WritePlayerRecord(int t)
    {
        PlayerLog.WriteLine("{\"tick\":" + t + ",\"player\":" + EntityJson(P0, PlayerFields) + "}");
    }
}
