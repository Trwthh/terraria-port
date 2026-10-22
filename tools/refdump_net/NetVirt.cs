// NetVirt: virtual connections for the net reference run (tools\refdump_net, RefDumpNet.exe server and RefDumpDraw.exe net).
//
// The game talks to the network through Terraria.Net.Sockets.ISocket; its only implementation used here is TcpSocket
// (Net/Sockets/TcpSocket.cs). NetVirt replaces TcpSocket's ISocket members (Detour.cs) so that no OS socket is ever
// opened, connected or listened on: a TcpSocket becomes a virtual connection when the game calls Connect on it.
//
//   Connect(address)     the connection exists at once (TcpSocket's own field _remoteAddress = address, as the original)
//   IsConnected()        true until Close()
//   AsyncSend(data, offset, size, callback, state)
//                        the bytes data[0 .. size) are appended to the connection's output (TcpSocket.AsyncSend on Windows
//                        writes from offset 0, TcpSocket.cs: `GetStream().BeginWrite(data, 0, size, ..)`), then the callback
//                        runs with its state (the write completed)
//   IsDataAvailable()    false: the game never reads from the socket. What the other side sent goes in through the call
//                        the read callbacks make, NetMessage.ReceiveBytes(bytes, length, bufferIndex) (RemoteClient.cs
//                        ServerReadCallBack, RemoteServer.cs ClientReadCallBack), by the harness at a fixed point of a tick
//   AsyncReceive         never reached (IsDataAvailable is false); throws
//   Close()              the connection is closed (_remoteAddress = null, as the original)
//   StartListening       false: no listener exists in this run (the harness accepts its client itself)
//
// Port: terraria-host sys/net.rs "virtual connections" (set_virtual): the same behaviour under the game's real TcpSocket
// code (TcpClient.Connect gives a virtual connection; writes are recorded; no data is ever available to read).
//
// NetPipe: the lock-step link between the two harness processes (a named pipe of this machine, no network).
using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Pipes;
using System.Reflection;

static class NetVirt
{
    public class Conn
    {
        public int Id;
        public bool Closed;
        public MemoryStream Out = new MemoryStream();
        public long Sent;
    }

    static readonly object Lock = new object();
    static readonly Dictionary<object, Conn> BySocket = new Dictionary<object, Conn>(new RefEq());
    public static readonly List<Conn> All = new List<Conn>();
    static FieldInfo RemoteAddressField;

    class RefEq : IEqualityComparer<object>
    {
        public new bool Equals(object a, object b) { return ReferenceEquals(a, b); }
        public int GetHashCode(object o) { return System.Runtime.CompilerServices.RuntimeHelpers.GetHashCode(o); }
    }

    public static void Install(Assembly game)
    {
        Type ts = game.GetType("Terraria.Net.Sockets.TcpSocket", true);
        RemoteAddressField = ts.GetField("_remoteAddress", BindingFlags.Instance | BindingFlags.NonPublic);
        const BindingFlags I = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;
        const BindingFlags S = BindingFlags.Static | BindingFlags.NonPublic;
        Func<string, MethodInfo> member = n =>
        {
            MethodInfo m = ts.GetMethod("Terraria.Net.Sockets.ISocket." + n, I);
            if (m == null) throw new Exception("TcpSocket has no ISocket." + n);
            return m;
        };
        Detour.Install(member("Connect"), typeof(NetVirt).GetMethod("VConnect", S));
        Detour.Install(member("IsConnected"), typeof(NetVirt).GetMethod("VIsConnected", S));
        Detour.Install(member("AsyncSend"), typeof(NetVirt).GetMethod("VAsyncSend", S));
        Detour.Install(member("AsyncReceive"), typeof(NetVirt).GetMethod("VAsyncReceive", S));
        Detour.Install(member("IsDataAvailable"), typeof(NetVirt).GetMethod("VIsDataAvailable", S));
        Detour.Install(member("Close"), typeof(NetVirt).GetMethod("VClose", S));
        Detour.Install(member("StartListening"), typeof(NetVirt).GetMethod("VStartListening", S));
    }

    static Conn Of(object self) { lock (Lock) { Conn c; return BySocket.TryGetValue(self, out c) ? c : null; } }

    static void VConnect(object self, object address)
    {
        lock (Lock)
        {
            var c = new Conn { Id = All.Count };
            BySocket[self] = c;
            All.Add(c);
        }
        RemoteAddressField.SetValue(self, address);
    }
    static bool VIsConnected(object self) { Conn c = Of(self); return c != null && !c.Closed; }
    static void VAsyncSend(object self, byte[] data, int offset, int size, Delegate callback, object state)
    {
        Conn c = Of(self);
        if (c == null || c.Closed) throw new IOException("NetVirt: send on a socket that is not connected");
        lock (Lock) { c.Out.Write(data, 0, size); c.Sent += size; }
        if (callback != null) callback.DynamicInvoke(state);
    }
    static void VAsyncReceive(object self, byte[] data, int offset, int size, Delegate callback, object state)
    {
        throw new InvalidOperationException("NetVirt: AsyncReceive (the harness delivers through NetMessage.ReceiveBytes)");
    }
    static bool VIsDataAvailable(object self) { return false; }
    static void VClose(object self)
    {
        Conn c = Of(self);
        if (c != null) c.Closed = true;
        RemoteAddressField.SetValue(self, null);
    }
    static bool VStartListening(object self, Delegate callback) { return false; }

    /// <summary>Queued bytes into the game's read buffer of `index`: as many as fit (MessageBuffer.readBufferMax 131070 -
    /// totalData), through NetMessage.ReceiveBytes, the call the socket read callbacks make.</summary>
    public static void Deliver(MethodInfo receive, Array buffers, List<byte> pending, int index)
    {
        if (pending.Count == 0) return;
        object buf = buffers.GetValue(index);
        int room = 131070 - (int)buf.GetType().GetField("totalData").GetValue(buf);
        int n = Math.Min(room, pending.Count);
        if (n <= 0) return;
        byte[] chunk = pending.GetRange(0, n).ToArray();
        pending.RemoveRange(0, n);
        receive.Invoke(null, new object[] { chunk, n, index });
    }

    /// <summary>Message counts by type of a stream (messages may span ticks: `partial` keeps the incomplete tail).</summary>
    public static void Count(byte[] data, List<byte> partial, SortedDictionary<int, int> counts)
    {
        partial.AddRange(data);
        int o = 0;
        while (partial.Count - o >= 3)
        {
            int len = partial[o] | (partial[o + 1] << 8);
            if (len < 3 || partial.Count - o < len) break;
            int type = partial[o + 2];
            int c; counts.TryGetValue(type, out c); counts[type] = c + 1;
            o += len;
        }
        partial.RemoveRange(0, o);
    }

    /// <summary>What connection k sent since the last call.</summary>
    public static byte[] Take(int k)
    {
        lock (Lock)
        {
            if (k >= All.Count) return new byte[0];
            byte[] b = All[k].Out.ToArray();
            All[k].Out.SetLength(0);
            return b;
        }
    }
}

/// <summary>The lock-step link: frames of (tick, bytes) over a named pipe of this machine.</summary>
class NetPipe
{
    readonly Stream _s;
    readonly BinaryReader _r;
    readonly BinaryWriter _w;
    NetPipe(Stream s) { _s = s; _r = new BinaryReader(s); _w = new BinaryWriter(s); }

    public static NetPipe Serve(string name, int timeoutMs)
    {
        var s = new NamedPipeServerStream(name, PipeDirection.InOut, 1, PipeTransmissionMode.Byte, PipeOptions.Asynchronous);
        IAsyncResult ar = s.BeginWaitForConnection(null, null);
        if (!ar.AsyncWaitHandle.WaitOne(timeoutMs)) throw new TimeoutException("NetPipe: no client connected to " + name);
        s.EndWaitForConnection(ar);
        return new NetPipe(s);
    }
    public static NetPipe Connect(string name, int timeoutMs)
    {
        var s = new NamedPipeClientStream(".", name, PipeDirection.InOut);
        s.Connect(timeoutMs);
        return new NetPipe(s);
    }
    public void Send(int tick, byte[] data)
    {
        _w.Write(tick);
        _w.Write(data.Length);
        _w.Write(data);
        _w.Flush();
    }
    /// <summary>The next frame: its tick, and its bytes (null: the other side ended the run, `End`).</summary>
    public byte[] ReceiveAny(out int tick)
    {
        tick = _r.ReadInt32();
        int n = _r.ReadInt32();
        if (n < 0) return null;
        return n == 0 ? new byte[0] : _r.ReadBytes(n);
    }
    /// <summary>The run ends after this tick.</summary>
    public void End(int tick)
    {
        _w.Write(tick);
        _w.Write(-1);
        _w.Flush();
    }
    public byte[] Receive(int tick)
    {
        int t = _r.ReadInt32();
        if (t != tick) throw new Exception("NetPipe: expected tick " + tick + ", got " + t);
        int n = _r.ReadInt32();
        return n == 0 ? new byte[0] : _r.ReadBytes(n);
    }
    public void Close() { try { _s.Close(); } catch (Exception) { } }
}

/// <summary>A recorded stream: per tick (tick, length, bytes), little endian (refdata\net\*.stream).</summary>
class NetStreamLog
{
    readonly BinaryWriter _w;
    public NetStreamLog(string path) { _w = new BinaryWriter(new FileStream(path, FileMode.Create)); }
    public void Write(int tick, byte[] data) { _w.Write(tick); _w.Write(data.Length); _w.Write(data); }
    public void Close() { _w.Close(); }
}
