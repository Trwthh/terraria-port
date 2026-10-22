// The net run's CLIENT (`RefDumpDraw.exe <dir> net <pipe name>`, refdata\net; port: tests/draw.rs DRAW_MODE=net,
// tests/draw/net.rs): the real Terraria.exe joins the original server of tools\refdump_net (RefDumpNet.exe server, in
// lock step over the named pipe) and plays: the draw harness's frames, with the client's network through NetVirt.cs.
//
// The client's network thread. Netplay.StartTcpClient starts a thread running Netplay.TcpClientLoop (ClientLoopSetup,
// TcpConnectLoop, InnerClientLoop: a loop that sleeps 1 ms per iteration, Netplay.cs:390-602). Here that thread is
// started by the harness (Netplay.StartTcpClient replaced: the same thread body) and runs only when the main thread
// lets it: after every Main.Update, one "slice" (to its next Thread.Sleep), the asset loader's share, a second slice.
// That is the port's schedule: its threads are fibers resumed by run_pending_work at the end of every Update, which
// runs twice there (terraria-host game.rs end_update, sys/threading.rs run_pending_work_and_dispatchers). Thread.Sleep
// on that thread is the gate (a detour); every other thread sleeps as before (kernel32 Sleep).
//
// Per frame (NetPre / NetPost around Tick): the server's bytes of the previous tick are delivered into buffer 256 (as
// much as fits, NetMessage.ReceiveBytes: what RemoteServer.ClientReadCallBack does); after the frame what the client
// sent goes to the server over the pipe, and the server's reply comes back for the next frame.
//
// Output: client_out.stream / client_in.stream (per frame: frame, length, bytes), the scenes and digests of the draw
// harness, scenes.state (StateLine at every recorded scene), client.summary.json (messages by type and direction).
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

public static partial class WG
{
    static bool NetMode;
    static string NetPipeName;
    static NetPipe NetLink;
    static bool NetActive;
    static int NetFrameNo;
    static readonly List<byte> NetPending = new List<byte>();
    static NetStreamLog NetOutLog, NetInLog;
    static readonly SortedDictionary<int, int> NetSentCounts = new SortedDictionary<int, int>(), NetRecvCounts = new SortedDictionary<int, int>();
    static readonly List<byte> NetSentPartial = new List<byte>(), NetRecvPartial = new List<byte>();
    static MethodInfo NetReceive;
    static Array NetBuffers;

    // ---------------------------------------------------------------- the client thread and its gate
    static Thread NetClientThread;
    static AutoResetEvent NetGo, NetParked;
    static volatile bool NetThreadDone;
    static int NetSlices;
    [DllImport("kernel32.dll")] static extern void Sleep(uint ms);

    /// <summary>Netplay.StartTcpClient (Netplay.cs:624-630) with the gate: the same thread body, Netplay.TcpClientLoop.</summary>
    static void NetStartTcpClient()
    {
        NetGo = new AutoResetEvent(false);
        NetParked = new AutoResetEvent(false);
        MethodInfo loop = T("Terraria.Netplay").GetMethod("TcpClientLoop", AllStatic);
        NetClientThread = new Thread(() =>
        {
            NetGo.WaitOne();
            try { loop.Invoke(null, null); }
            catch (Exception e) { Console.Error.WriteLine("client thread: " + e); }
            finally { NetThreadDone = true; NetParked.Set(); }
        });
        NetClientThread.Name = "TCP Client Thread";
        NetClientThread.IsBackground = true;
        NetClientThread.Start();
    }
    /// <summary>Thread.Sleep(int): on the client thread the end of a slice; elsewhere the OS sleep.</summary>
    static void NetSleep(int ms)
    {
        if (NetClientThread != null && Thread.CurrentThread == NetClientThread)
        {
            NetParked.Set();
            NetGo.WaitOne();
            return;
        }
        Sleep(ms < 0 ? 0xFFFFFFFF : (uint)ms);
    }
    /// <summary>One slice of the client thread (no-op without one).</summary>
    public static void NetSlice()
    {
        if (NetClientThread == null || NetThreadDone) return;
        NetSlices++;
        NetGo.Set();
        if (!NetParked.WaitOne(120000)) throw new Exception("the client thread did not reach its Thread.Sleep within 120 s (inlined Sleep? a wait?)");
    }

    static void NetInstall()
    {
        NetNudge();
        NetVirt.Install(G);
        BindingFlags S = BindingFlags.Static | BindingFlags.NonPublic;
        Detour.Install(typeof(Thread).GetMethod("Sleep", new[] { typeof(int) }), typeof(WG).GetMethod("NetSleep", S));
        Detour.Install(T("Terraria.Netplay").GetMethod("StartTcpClient", AllStatic), typeof(WG).GetMethod("NetStartTcpClient", S));
        Type nm = T("Terraria.NetMessage");
        NetReceive = nm.GetMethod("ReceiveBytes", AllStatic);
        NetBuffers = (Array)SF(nm, "buffer");
    }
    // (keeps the compiler from dropping the kernel32 import before the detour needs it)
    static void NetNudge() { }

    /// <summary>Before a frame: the server's pending bytes into buffer 256.</summary>
    static void NetPre()
    {
        if (!NetActive) return;
        NetVirt.Deliver(NetReceive, NetBuffers, NetPending, 256);
    }
    /// <summary>After a frame: the client's bytes to the server, the server's reply queued.</summary>
    static void NetPost()
    {
        if (!NetActive) return;
        NetFrameNo++;
        byte[] c = NetVirt.Take(0);
        NetOutLog.Write(NetFrameNo, c);
        NetVirt.Count(c, NetSentPartial, NetSentCounts);
        NetLink.Send(NetFrameNo, c);
        byte[] s = NetLink.Receive(NetFrameNo);
        NetInLog.Write(NetFrameNo, s);
        NetVirt.Count(s, NetRecvPartial, NetRecvCounts);
        NetPending.AddRange(s);
    }

    static object NetConnection { get { return SF(T("Terraria.Netplay"), "Connection"); } }

    /// <summary>The net run after the character creation scene: the player created and selected, the join, play.</summary>
    static void NetScenario()
    {
        Type pfdT = T("Terraria.IO.PlayerFileData");
        object player = SF(TMain, "PendingPlayer");
        ISet(player, "name", "Ref");
        object created = CallStatic(pfdT, "CreateAndSave", player);
        object pfd = CallStatic(T("Terraria.Player"), "GetFileData", (string)IF(created, "Path"), false);
        SSet(TMain, "menuMultiplayer", true);
        CallStatic(TMain, "SelectPlayer", pfd);
        Note("selected: menuMode " + SF(TMain, "menuMode"));
        // "Join via IP": 127.0.0.1, port 7777 (Main.OnSubmitServerIP / OnSubmitServerPort, Main.cs:52924-52939, with
        // Netplay.SetRemoteIP for SetRemoteIPAsync: no DNS lookup for an address)
        SSet(TMain, "getIP", "127.0.0.1");
        SSet(T("Terraria.Netplay"), "ListenPort", 7777);
        SSet(TMain, "autoPass", false);
        CallStatic(T("Terraria.Netplay"), "SetRemoteIP", "127.0.0.1");
        NetOutLog = new NetStreamLog(Path.Combine(OutRoot, "client_out.stream"));
        NetInLog = new NetStreamLog(Path.Combine(OutRoot, "client_in.stream"));
        NetLink = NetPipe.Connect(NetPipeName, 600000);
        NetActive = true;
        CallStatic(TMain, "StartClientGameplay");
        int joined = -1;
        for (int i = 0; i < 6000; i++)
        {
            Frames(1);
            if ((int)SF(TMain, "netMode") == 1 && !(bool)SF(TMain, "gameMenu") && (int)IF(NetConnection, "State") == 10) { joined = Frame; break; }
        }
        Note("joined at frame " + joined + " (state " + IF(NetConnection, "State") + ", netMode " + SF(TMain, "netMode") + ", gameMenu " + SF(TMain, "gameMenu") + ", slices " + NetSlices + ")");
        if (joined < 0) throw new Exception("the client did not get into the world");
        Frames(60);
        Record("net_joined", 2);
        NetClientScript();
        Record("net_end", 2);
        NetLink.End(NetFrameNo + 1);
        NetActive = false;
        NetOutLog.Close(); NetInLog.Close(); NetLink.Close();
        var sj = new StringBuilder("{\n\"generator\":\"tools/refdump_draw (RefDumpDraw.exe net)\",\n\"frames_linked\":" + NetFrameNo + ",\"joined_at_frame\":" + joined + ",\"client_thread_slices\":" + NetSlices + ",\n");
        sj.Append("\"messages_sent\":{" + string.Join(",", NetSentCounts.Select(kv => "\"" + kv.Key + "\":" + kv.Value).ToArray()) + "},\n");
        sj.Append("\"messages_received\":{" + string.Join(",", NetRecvCounts.Select(kv => "\"" + kv.Key + "\":" + kv.Value).ToArray()) + "}\n}\n");
        File.WriteAllText(Path.Combine(OutRoot, "client.summary.json"), sj.ToString(), new UTF8Encoding(false));
    }

    /// <summary>What the client does in the world (port: tests/draw/net.rs net_client_script).</summary>
    static void NetClientScript()
    {
        // walking right (D held), then left, a jump
        InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.D };
        Frames(60);
        InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.A };
        Frames(30);
        InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.Space };
        Frames(20);
        InKeys = null;
        Frames(40);
        Record("net_walked", 2);
        object pl = LocalPlayer;
        // a chat line (ChatHelper.SendChatMessageFromClient: NetTextModule)
        CallStatic(T("Terraria.Chat.ChatHelper"), "SendChatMessageFromClient", Activator.CreateInstance(T("Terraria.Chat.ChatMessage"), new object[] { "hello from the client" }));
        Frames(30);
        Record("net_chat", 1);
        // mining with the copper pickaxe (hotbar 2) the ground one tile right of the feet
        Press(Microsoft.Xna.Framework.Input.Keys.D2);
        int gx = FeetTileX + 1, gy = FeetTileY;
        Aim(gx * 16 + 8, gy * 16 + 8);
        InLeft = true;
        Frames(90);
        InLeft = false;
        Frames(20);
        Record("net_mined", 1);
        // dirt blocks placed in the air two tiles right and up (hotbar 10)
        {
            object it = ((Array)IF(pl, "inventory")).GetValue(9);
            it.GetType().GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int)).Invoke(it, new object[] { 2, null });
            ISet(it, "stack", 50);
            Press(Microsoft.Xna.Framework.Input.Keys.D0);
            Aim((FeetTileX + 2) * 16 + 8, (FeetTileY - 2) * 16 + 8);
            InLeft = true;
            Frames(15);
            InLeft = false;
            Frames(20);
            Record("net_placed", 1);
            // one of them thrown out (Player.DropSelectedItem)
            pl.GetType().GetMethods().First(m => m.Name == "DropSelectedItem" && m.GetParameters().Length == 0).Invoke(pl, null);
            Frames(30);
            Record("net_dropped", 1);
        }
        // PvP on, a team (NetMessage 30, 45 as the player menu sends them)
        Type nm = T("Terraria.NetMessage");
        MethodInfo send = nm.GetMethods(AllStatic).First(m => m.Name == "SendData" && m.GetParameters().Length == 11);
        Action<int, int, float, float, float> sendData = (type, n, n2, n3, n4) => send.Invoke(null, new object[] { type, -1, -1, null, n, n2, n3, n4, 0, 0, 0 });
        int me = (int)SF(TMain, "myPlayer");
        ISet(pl, "hostile", true);
        sendData(30, me, 0f, 0f, 0f);
        ISet(pl, "team", 1);
        sendData(45, me, 0f, 0f, 0f);
        Frames(20);
        // an emote (EmoteBubble.MakeLocalPlayerEmote)
        CallStatic(T("Terraria.GameContent.UI.EmoteBubble"), "MakeLocalPlayerEmote", 0);
        Frames(30);
        Record("net_social", 1);
        // the client's own death and the respawn
        Type pdr = T("Terraria.DataStructures.PlayerDeathReason");
        pl.GetType().GetMethod("KillMe").Invoke(pl, new object[] { pdr.GetMethod("ByCustomReason", AllStatic).Invoke(null, new object[] { "the net run" }), 1000.0, 1, false });
        Frames(30);
        Record("net_dead", 1);
        for (int i = 0; i < 1200 && (bool)IF(pl, "dead"); i++) Frames(1);
        Frames(30);
        Record("net_respawned", 1);
        Frames(400);
        Record("net_server_events", 2);
        // quick stack to nearby chests as a client (QuickStacking.SendQuickStackToNearbyChests: NetMessage 5 for every
        // stackable slot, then 85; the server stacks)
        CallStatic(T("Terraria.GameContent.QuickStacking"), "QuickStackToNearbyChests", pl, false);
        Frames(30);
        Record("net_quickstack", 1);
        // two team mates, one on screen and one off it (NewMultiplayerClosePlayersOverlay: the name plates, the off-screen
        // distance, head and life bar): client-side player entries, as the client keeps the other players it was sent
        int plate = (int)SF(TMain, "teamNamePlateDistance");
        SSet(TMain, "teamNamePlateDistance", 400);
        Array players = (Array)SF(TMain, "player");
        var pp = (Microsoft.Xna.Framework.Vector2)IF(pl, "position");
        var saved = new List<object>();
        var offs = new[] { new Microsoft.Xna.Framework.Vector2(120f, -20f), new Microsoft.Xna.Framework.Vector2(1400f, -60f) };
        for (int k = 0; k < offs.Length; k++)
        {
            int slot = 200 + k;
            saved.Add(players.GetValue(slot));
            object o = Activator.CreateInstance(T("Terraria.Player"));
            ISet(o, "name", "Mate" + k);
            ISet(o, "whoAmI", slot);
            ISet(o, "team", 1);
            ISet(o, "active", true);
            ISet(o, "position", pp + offs[k]);
            players.SetValue(o, slot);
        }
        Frames(3);
        Record("net_teammates", 1);
        for (int k = 0; k < offs.Length; k++) players.SetValue(saved[k], 200 + k);
        SSet(TMain, "teamNamePlateDistance", plate);
        Frames(3);
    }
}
