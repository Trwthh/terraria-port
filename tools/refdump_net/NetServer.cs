// RefDumpNet: the multiplayer reference run's SERVER: the real TerrariaServer.exe (work\refserver copy) in-process, a
// dedicated server's per-tick loop with one client connected through a virtual connection (NetVirt.cs), in lock step
// with the original CLIENT (RefDumpDraw.exe <dir> net, tools\refdump_draw\NetClient.cs) over a named pipe.
//
//   RefDumpNet.exe server <outdir> <pipe name> [watchdog seconds]
//
// No socket is opened: no TcpListener, no Netplay.StartServer (whose ServerLoop thread would also start the LAN
// broadcast, Netplay.cs:291-302, 786-832), no UPnP. The process is the dedicated server's main loop, tick by tick:
//   load            as tools\refdump_ticks\Ticks.cs: InitGame (Main.dedServ = true, Initialize), `-world <scratch copy of
//                   ref_small.wld>` SetAsActive, WorldGen.serverLoadWorldCallBack on a pool thread with its Main.rand pinned
//   server state    Netplay.InitializeServer without its socket part (Netplay.cs:246-271): Main.myPlayer = 255, netMode = 2,
//                   Netplay.Disconnect = false, Netplay.Clients[i] = new RemoteClient { Id = i, ReadBuffer = byte[1024] }
//                   (Reset), Main.gameMenu = false (Main.cs:5906); the pins of Ticks.cs (Main.rand, UnpausedUpdateSeed,
//                   TileFrameSeed, the wind counters)
//   accept          when the client's first frame arrives: a TcpSocket connected to 127.0.0.1:7777 (virtual) handed to
//                   Netplay.OnConnectionAccepted (Netplay.cs:181-195), what TcpSocket.ListenLoop does with a new client
//   per tick k      1. the client's frame k (its bytes) arrives over the pipe and is queued for buffer 0
//                   2. delivered: NetMessage.ReceiveBytes(bytes, n, 0) with n = what fits the read buffer
//                      (MessageBuffer.readBufferMax - totalData); the rest waits for the next tick
//                   3. Netplay.UpdateConnectedClients() (what the server loop thread does, Netplay.cs:304-335)
//                   4. the main loop's body (Main.cs:5920-5939): HasFullyConnectedClients ? (DetailedFPS.StartNextFrame();
//                      Main.Update(new GameTime())) : Netplay.UpdateInMainThread()
//                   5. the server's own script for this tick (ServerScript), relative to the tick the client is in the
//                      world (Clients[0].State == 10)
//                   6. what the server sent this tick (the virtual connection's output) goes back over the pipe
// Pins (DateTime.Now = 2026-06-15 12:00, Environment.TickCount = 1000000, Guid.NewGuid() = Guid.Empty on the main
// thread: the same instants as the client's harness) are detours (Detour.cs).
//
// Output (<outdir>): server_in.stream / server_out.stream (per tick: tick, length, bytes; NetVirt.cs NetStreamLog),
// server.state (per tick: the state line, NetStateLine), server.summary.json (messages by type and direction, ticks).
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
    const int NetRandSeed = 424242, NetLoadSeed = 1357;
    public const int NetTickCount = 1000000;
    public static readonly long NetClockTicks = new DateTime(2026, 6, 15, 12, 0, 0).Ticks;
    static int NetMainThread;
    static MethodInfo NetUpdateM;
    static Type NetGameTimeT;
    static readonly StringBuilder NetState = new StringBuilder();
    static readonly List<string> NetNotes = new List<string>();
    static int NetWindResets;

    static int NetFakeTickCount() { return NetTickCount; }
    static DateTime NetFakeNow() { return new DateTime(NetClockTicks, DateTimeKind.Local); }
    static DateTime NetFakeUtcNow() { return new DateTime(NetClockTicks, DateTimeKind.Utc); }
    [System.Runtime.InteropServices.DllImport("ole32.dll")] static extern int CoCreateGuid(out Guid g);
    static Guid NetFakeNewGuid()
    {
        if (Thread.CurrentThread.ManagedThreadId == NetMainThread) return Guid.Empty;
        Guid g; CoCreateGuid(out g); return g;
    }

    public static Action NetAfterRun;

    [STAThread]
    static int Main(string[] args) { return NetServerMain(args); }

    public static int NetServerMain(string[] args)
    {
        if (args.Length < 3 || args[0] != "server") { Console.Error.WriteLine("usage: RefDumpNet.exe server <outdir> <pipe name> [watchdog seconds]"); return 2; }
        Root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(Root, "work", "refserver");
        string outDir = Dir(Path.GetFullPath(args[1]));
        string pipe = args[2];
        Watchdog(args.Length > 3 ? int.Parse(args[3]) : 3600);
        string scratch = Dir(Path.Combine(Root, "work", "net-scratch", "server_" + new DirectoryInfo(outDir).Name + "_" + DateTime.Now.ToString("yyyyMMdd-HHmmss")));
        SaveDir = Dir(Path.Combine(scratch, "save"));
        int rc = 0;
        try { RunNetServer(outDir, pipe, scratch); }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        if (NetAfterRun != null && rc == 0) NetAfterRun();
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }

    static void RunNetServer(string outDir, string pipeName, string scratch)
    {
        var total = Stopwatch.StartNew();
        NetMainThread = Thread.CurrentThread.ManagedThreadId;
        string world = Path.Combine(Dir(Path.Combine(scratch, "Worlds")), "ref_small.wld");
        File.Copy(Path.Combine(Root, "work", "refsave", "Worlds", "ref_small.wld"), world, true);
        // the clock, the tick counter, the main thread's GUIDs
        BindingFlags S = BindingFlags.Static | BindingFlags.NonPublic;
        Detour.Install(typeof(Environment).GetProperty("TickCount").GetGetMethod(), typeof(WG).GetMethod("NetFakeTickCount", S));
        Detour.Install(typeof(DateTime).GetProperty("Now").GetGetMethod(), typeof(WG).GetMethod("NetFakeNow", S));
        Detour.Install(typeof(DateTime).GetProperty("UtcNow").GetGetMethod(), typeof(WG).GetMethod("NetFakeUtcNow", S));
        Detour.Install(typeof(Guid).GetMethod("NewGuid"), typeof(WG).GetMethod("NetFakeNewGuid", S));

        InitGame(() =>
        {
            Step("-world <existing>: WorldFile.GetAllMetadata(path, false).SetAsActive()   [LaunchInitializer.cs:175-178, Main.cs:5308-5311]", () =>
            {
                object wfd = null;
                foreach (MethodInfo m in TWorldFile.GetMethods(AllStatic))
                    if (m.Name == "GetAllMetadata" && m.GetParameters().Length == 2) wfd = Unwrap(m, null, new object[] { world, false });
                wfd.GetType().GetMethod("SetAsActive").Invoke(wfd, null);
            });
        }, false);
        NetVirt.Install(G);
        Step("Task.Factory.StartNew(WorldGen.serverLoadWorldCallBack)  -> WorldFile.LoadWorld()   [WorldGen.cs:6903-6906, 6840]", () =>
        {
            Exception err = null;
            Task.Factory.StartNew(() => { try { SSet(TMain, "rand", NetNewRand(NetLoadSeed)); CallStatic(TWorldGen, "serverLoadWorldCallBack"); } catch (Exception e) { err = e; } }).Wait();
            if (err != null) throw new Exception("load failed: " + err);
            if ((bool)SF(TWorldGen, "loadFailed")) throw new Exception("WorldGen.loadFailed after serverLoadWorldCallBack");
        });
        Type netplay = T("Terraria.Netplay"), rc = T("Terraria.RemoteClient");
        Step("server state: myPlayer = 255; netMode = 2; Netplay.Disconnect = false; Netplay.Clients[0..255] = new RemoteClient   [Netplay.cs:254-263]", () =>
        {
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
            if (m.Name == "Update" && m.GetParameters().Length == 1 && m.GetParameters()[0].ParameterType.Name == "GameTime") { NetUpdateM = m; NetGameTimeT = m.GetParameters()[0].ParameterType; }
        // the pins of Ticks.cs (its walk that re-seeds every other reachable UnifiedRandom is not needed here: their
        // seeds come from Environment.TickCount, which is pinned; the port's test pins the same)
        SSet(TMain, "rand", NetNewRand(NetRandSeed));
        SSet(TMain, "UnpausedUpdateSeed", 123456789UL);
        SSet(TMain, "TileFrameSeed", 987654321UL);
        SSet(TMain, "windCounter", 1500); SSet(TMain, "extremeWindCounter", 25);

        var inLog = new NetStreamLog(Path.Combine(outDir, "server_in.stream"));
        var outLog = new NetStreamLog(Path.Combine(outDir, "server_out.stream"));
        Console.WriteLine("waiting for the client on pipe " + pipeName);
        NetPipe pipe = NetPipe.Serve(pipeName, 1800 * 1000);
        Console.WriteLine("client harness connected");
        var pending = new List<byte>();
        int connectedAt = -1, tick = 0;
        var inCounts = new SortedDictionary<int, int>();
        var outCounts = new SortedDictionary<int, int>();
        var inPartial = new List<byte>();
        var outPartial = new List<byte>();
        Type nm = T("Terraria.NetMessage");
        MethodInfo receive = nm.GetMethod("ReceiveBytes", AllStatic);
        Array buffers = (Array)SF(nm, "buffer");
        MethodInfo updateClients = netplay.GetMethod("UpdateConnectedClients", AllStatic);
        MethodInfo updateMain = netplay.GetMethod("UpdateInMainThread", AllStatic);
        MethodInfo nextFrame = T("Terraria.Testing.DetailedFPS").GetMethod("StartNextFrame", AllStatic);
        while (true)
        {
            tick++;
            byte[] fromClient;
            try { fromClient = pipe.ReceiveAny(out tick); }
            catch (EndOfStreamException) { Console.WriteLine("the client harness closed the pipe after tick " + (tick - 1)); break; }
            if (fromClient == null) { Console.WriteLine("the client ended the run at tick " + tick); break; }
            inLog.Write(tick, fromClient);
            NetVirt.Count(fromClient, inPartial, inCounts);
            if (tick == 1)
            {
                // TcpSocket.ListenLoop's work for a new client (Netplay.cs:181-195 via TcpSocket.cs ListenLoop)
                object sock = Activator.CreateInstance(T("Terraria.Net.Sockets.TcpSocket"));
                object addr = Activator.CreateInstance(T("Terraria.Net.TcpAddress"), new object[] { System.Net.IPAddress.Loopback, 7777 });
                T("Terraria.Net.Sockets.ISocket").GetMethod("Connect").Invoke(sock, new object[] { addr });
                netplay.GetMethod("OnConnectionAccepted", AllStatic).Invoke(null, new object[] { sock });
            }
            pending.AddRange(fromClient);
            NetVirt.Deliver(receive, buffers, pending, 0);
            updateClients.Invoke(null, null);
            if ((bool)SF(netplay, "HasFullyConnectedClients"))
            {
                nextFrame.Invoke(null, null);
                int windBefore = (int)SF(TMain, "windCounter");
                NetUpdateM.Invoke(MainInst, new object[] { Activator.CreateInstance(NetGameTimeT) });
                if ((int)SF(TMain, "windCounter") > windBefore)
                {
                    int pinned = 900 + (NetWindResets++ * 773) % 1801;
                    SSet(TMain, "windCounter", pinned);
                    NetNotes.Add("tick " + tick + ": Main.ResetWindCounter ran, windCounter replaced by " + pinned);
                }
            }
            else updateMain.Invoke(null, null);
            object client0 = ((Array)SF(netplay, "Clients")).GetValue(0);
            if (connectedAt < 0 && (int)IF(client0, "State") == 10) { connectedAt = tick; Console.WriteLine("client in the world at tick " + tick); }
            if (connectedAt >= 0) ServerScript(tick - connectedAt);
            byte[] toClient = NetVirt.Take(0);
            outLog.Write(tick, toClient);
            NetVirt.Count(toClient, outPartial, outCounts);
            NetState.Append(NetStateLine(tick, fromClient, toClient));
            pipe.Send(tick, toClient);
            if (tick % 300 == 0) Console.WriteLine("tick " + tick + " (" + total.Elapsed.TotalSeconds.ToString("0", Inv) + " s), state " + IF(client0, "State"));
        }
        inLog.Close(); outLog.Close(); pipe.Close();
        File.WriteAllText(Path.Combine(outDir, "server.state"), NetState.ToString(), new UTF8Encoding(false));
        var sj = new StringBuilder("{\n");
        sj.Append("\"generator\":\"tools/refdump_net (RefDumpNet.exe server)\",\n\"ticks\":" + tick + ",\"client_in_world_at\":" + connectedAt + ",\n");
        sj.Append("\"messages_received\":{" + string.Join(",", inCounts.Select(kv => "\"" + kv.Key + "\":" + kv.Value).ToArray()) + "},\n");
        sj.Append("\"messages_sent\":{" + string.Join(",", outCounts.Select(kv => "\"" + kv.Key + "\":" + kv.Value).ToArray()) + "},\n");
        sj.Append("\"notes\":[" + string.Join(",", NetNotes.Select(n => "\"" + n.Replace("\"", "'") + "\"").ToArray()) + "]\n}\n");
        File.WriteAllText(Path.Combine(outDir, "server.summary.json"), sj.ToString(), new UTF8Encoding(false));
        File.WriteAllText(Path.Combine(outDir, "server.timing.json"), "{\"total_ms\":" + total.ElapsedMilliseconds + "}\n", new UTF8Encoding(false));
        Console.WriteLine("done: " + tick + " ticks, in the world at " + connectedAt + "; received types " + inCounts.Count + ", sent types " + outCounts.Count);
    }

    /// <summary>The server's state after a tick (port: tests/net.rs, the same bytes): Main.rand, FNV-1a 64 of every active
    /// player (index, position, velocity, life, mana, selected item, controls), NPC (index, type, position, velocity, life,
    /// ai[]), projectile (index, type, owner, position, velocity, ai[], timeLeft) and world item (index, type,
    /// stack, position, velocity); Main.time; the bytes in and out.</summary>
    static string NetStateLine(int tick, byte[] inb, byte[] outb)
    {
        Func<ulong, int, ulong> i4 = (h, v) => { for (int k = 0; k < 4; k++) { h ^= (byte)(v >> (8 * k)); h *= 1099511628211UL; } return h; };
        Func<float, int> fb = f => BitConverter.ToInt32(BitConverter.GetBytes(f), 0);
        Func<object, string, Microsoft.Xna.Framework.Vector2> v2 = (o, n) => (Microsoft.Xna.Framework.Vector2)IF(o, n);
        uint inext; int[] sa;
        RandState(SF(TMain, "rand"), out inext, out sa);
        ulong rh = FnvBasis; foreach (int v in sa) rh = i4(rh, v);
        ulong ph = FnvBasis, nh = FnvBasis, prh = FnvBasis, ih = FnvBasis;
        int np = 0, nn = 0, npr = 0, ni = 0;
        Array players = (Array)SF(TMain, "player");
        for (int i = 0; i < 255; i++)
        {
            object p = players.GetValue(i);
            if (!(bool)IF(p, "active")) continue;
            np++;
            var pos = v2(p, "position"); var vel = v2(p, "velocity");
            ph = i4(i4(i4(i4(i4(i4(i4(i4(ph, i), fb(pos.X)), fb(pos.Y)), fb(vel.X)), fb(vel.Y)), (int)IF(p, "statLife")), (int)IF(p, "statMana")), (int)IF(p, "selectedItem"));
            ph = i4(ph, ((bool)IF(p, "controlLeft") ? 1 : 0) | ((bool)IF(p, "controlRight") ? 2 : 0) | ((bool)IF(p, "controlJump") ? 4 : 0) | ((bool)IF(p, "controlUseItem") ? 8 : 0));
        }
        Array npcs = (Array)SF(TMain, "npc");
        for (int i = 0; i < npcs.Length; i++)
        {
            object n = npcs.GetValue(i);
            if (!(bool)IF(n, "active")) continue;
            nn++;
            var pos = v2(n, "position"); var vel = v2(n, "velocity");
            nh = i4(i4(i4(i4(i4(i4(i4(nh, i), (int)IF(n, "type")), fb(pos.X)), fb(pos.Y)), fb(vel.X)), fb(vel.Y)), (int)IF(n, "life"));
            foreach (float a in (float[])IF(n, "ai")) nh = i4(nh, fb(a));
        }
        Array projs = (Array)SF(TMain, "projectile");
        for (int i = 0; i < projs.Length; i++)
        {
            object n = projs.GetValue(i);
            if (!(bool)IF(n, "active")) continue;
            npr++;
            var pos = v2(n, "position"); var vel = v2(n, "velocity");
            prh = i4(i4(i4(i4(i4(i4(i4(prh, i), (int)IF(n, "type")), (int)IF(n, "owner")), fb(pos.X)), fb(pos.Y)), fb(vel.X)), fb(vel.Y));
            foreach (float a in (float[])IF(n, "ai")) prh = i4(prh, fb(a));
            prh = i4(prh, (int)IF(n, "timeLeft"));
        }
        Array items = (Array)SF(TMain, "item");
        for (int i = 0; i < items.Length; i++)
        {
            object n = items.GetValue(i);
            if (!(bool)IF(n, "active")) continue;
            ni++;
            var pos = v2(n, "position"); var vel = v2(n, "velocity");
            ih = i4(i4(i4(i4(i4(i4(i4(ih, i), (int)IF(n, "type")), (int)IF(n, "stack")), fb(pos.X)), fb(pos.Y)), fb(vel.X)), fb(vel.Y));
        }
        double time = (double)SF(TMain, "time");
        return "tick " + tick + " in " + inb.Length + ":" + Fnv(inb, 0, inb.Length, FnvBasis).ToString("x16") + " out " + outb.Length + ":" + Fnv(outb, 0, outb.Length, FnvBasis).ToString("x16")
            + " rand " + inext + " " + rh.ToString("x16") + " players " + np + ":" + ph.ToString("x16") + " npc " + nn + ":" + nh.ToString("x16") + " proj " + npr + ":" + prh.ToString("x16")
            + " item " + ni + ":" + ih.ToString("x16") + " time " + BitConverter.DoubleToInt64Bits(time).ToString("x16") + " day " + ((bool)SF(TMain, "dayTime") ? 1 : 0) + "\n";
    }

    static object NetNewRand(int seed) { return Activator.CreateInstance(T("Terraria.Utilities.UnifiedRandom"), new object[] { seed }); }

    /// <summary>The server's own actions, by ticks since the client is in the world (port: tests/net.rs server_script).</summary>
    static void ServerScript(int t)
    {
        object p0 = ((Array)SF(TMain, "player")).GetValue(0);
        var pos = (Microsoft.Xna.Framework.Vector2)IF(p0, "position");
        Type nm = T("Terraria.NetMessage");
        MethodInfo send = nm.GetMethods(AllStatic).First(m => m.Name == "SendData" && m.GetParameters().Length == 11);
        object src = Activator.CreateInstance(T("Terraria.DataStructures.EntitySource_DebugCommand"));
        switch (t)
        {
            case 600:
                // a blue slime near the player (NPC.NewNPC syncs it: 23)
                T("Terraria.NPC").GetMethods(AllStatic).First(m => m.Name == "NewNPC" && m.GetParameters().Length == 10)
                    .Invoke(null, new object[] { src, (int)pos.X + 200, (int)pos.Y, 1, 0, 0f, 0f, 0f, 0f, 255 });
                break;
            case 650:
                // rain (Main.StartRain; the world data message 7 carries it)
                CallStatic(TMain, "StartRain", false, null, false);
                send.Invoke(null, new object[] { 7, -1, -1, null, 0, 0f, 0f, 0f, 0, 0, 0 });
                break;
            case 700:
                // an item dropped by the server near the player (21)
                T("Terraria.Item").GetMethods(AllStatic).First(m => m.Name == "NewItem" && m.GetParameters().Length == 12)
                    .Invoke(null, new object[] { src, (int)pos.X - 64, (int)pos.Y, 16, 16, 73, 3, false, 0, Enum.ToObject(T("Terraria.Item").GetMethods(AllStatic).First(m => m.Name == "NewItem" && m.GetParameters().Length == 12).GetParameters()[9].ParameterType, 0), null, null });
                break;
            case 750:
                // a server chat line (NetTextModule broadcast)
                object text = T("Terraria.Localization.NetworkText").GetMethod("FromLiteral", AllStatic).Invoke(null, new object[] { "hello from the server" });
                CallStatic(T("Terraria.Chat.ChatHelper"), "BroadcastChatMessage", text, Activator.CreateInstance(typeof(Microsoft.Xna.Framework.Color), new object[] { 255, 240, 20 }), -1);
                break;
            case 800:
                // a tile the server breaks next to the player, and its square (17, 20)
                {
                    int tx = (int)((pos.X + 8f) / 16f) - 3, ty = (int)((pos.Y + 42f) / 16f) + 1;
                    CallStatic(TWorldGen, "KillTile", tx, ty, false, false, false);
                    send.Invoke(null, new object[] { 17, -1, -1, null, 0, (float)tx, (float)ty, 0f, 0, 0, 0 });
                }
                break;
            case 850:
                // night, then the time message (18)
                SSet(TMain, "dayTime", false); SSet(TMain, "time", 0.0);
                send.Invoke(null, new object[] { 18, -1, -1, null, 0, 0f, 0f, 0f, 0, 0, 0 });
                break;
        }
    }
}
