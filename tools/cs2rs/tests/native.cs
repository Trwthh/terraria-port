// Natively mapped library types (NATIVE_SCHEMA.md): a class (System.IO streams, BinaryReader, BinaryWriter:
// native.d\rt_io.json -> rt::io) and a struct (System.TimeSpan: native.d\rt_time.json -> rt::time).
using System;
using System.Collections.Generic;
using System.IO;

// The worked class example: BinaryReader over a byte buffer.
public static class NativeBinaryReader
{
    static int Sum(Stream s)                    // a MemoryStream passed as its base class
    {
        int sum = 0;
        int b;
        while ((b = s.ReadByte()) != -1) sum += b;
        return sum;
    }

    static string Kind(object o)                // type tests against native classes
    {
        if (o is BinaryReader) return "reader";
        if (o is MemoryStream) return "memory";
        if (o is Stream) return "stream";
        return o == null ? "null" : "other";
    }

    public static string Run()
    {
        byte[] data = { 0x78, 0x56, 0x34, 0x12, 1, 0, 0, 0xC0, 0x3F, 5, (byte)'h', (byte)'e', (byte)'l', (byte)'l', (byte)'o', 0xFF, 0xFE, 9, 8, 7 };
        MemoryStream ms = new MemoryStream(data);
        BinaryReader r = new BinaryReader(ms);
        string s = r.ReadInt32().ToString() + "," + r.ReadBoolean() + "," + r.ReadSingle() + "," + r.ReadString() + "," + r.ReadInt16();
        s += ";" + r.BaseStream.Position + "/" + ms.Length;         // Position is declared on Stream, Length is read through MemoryStream
        byte[] rest = r.ReadBytes(10);                              // fewer bytes at the end: no exception
        s += ";" + rest.Length + ":" + rest[0] + rest[1] + rest[2];
        try { r.ReadByte(); s += ";no exception"; }
        catch (EndOfStreamException) { s += ";EndOfStream"; }       // thrown by native code, caught by translated code
        ms.Position = 15;
        s += ";" + Sum(ms);
        object o = r;
        BinaryReader again = (BinaryReader)o;                       // checked downcast from object
        s += ";" + Kind(o) + "," + Kind(ms) + "," + Kind("x") + "," + Kind(null) + "," + (again == r) + "," + ((o as MemoryStream) == null);
        try { MemoryStream bad = (MemoryStream)o; s += ";cast ok" + (bad == null); }
        catch (InvalidCastException) { s += ";InvalidCast"; }
        using (BinaryReader r2 = new BinaryReader(new MemoryStream(new byte[] { 200, 1 })))
        {
            s += ";" + r2.ReadUInt16();
        }
        r.Close();
        s += ";" + ms.CanRead;                                      // closing the reader closes the stream
        return s;
    }
}

public static class NativeBinaryWriter
{
    public static string Run()
    {
        MemoryStream ms = new MemoryStream();
        BinaryWriter w = new BinaryWriter(ms);
        w.Write(-2);
        w.Write((byte)7);
        w.Write((short)-3);
        w.Write(true);
        w.Write(2.5f);
        w.Write(1.0 / 3.0);
        w.Write("café");
        w.Write(new byte[] { 1, 2, 3 });
        w.Write(9000000000L);
        w.Flush();
        long len = ms.Length;
        ms.Position = 0;
        BinaryReader r = new BinaryReader(ms);
        string s = len + ":" + r.ReadInt32() + "," + r.ReadByte() + "," + r.ReadInt16() + "," + r.ReadBoolean() + "," + r.ReadSingle() + "," + (r.ReadDouble() * 3.0) + "," + r.ReadString().Length;
        byte[] three = r.ReadBytes(3);
        s += "," + (three[0] + three[1] + three[2]) + "," + r.ReadInt64();
        byte[] all = ms.ToArray();
        s += ";" + all.Length + ":" + all[0] + "," + all[3] + "," + all[4];
        w.Seek(0, SeekOrigin.Begin);
        w.Write(5);
        ms.Seek(-8, SeekOrigin.End);
        s += ";" + r.ReadInt64() + "," + ms.ToArray()[0];
        return s;
    }
}

// The worked struct example: TimeSpan.
public class Lap
{
    public TimeSpan best;                       // a native struct as a field: zeroed is TimeSpan.Zero
    public TimeSpan[] splits = new TimeSpan[2];
}

public static class NativeTimeSpan
{
    static void Extend(ref TimeSpan t, out TimeSpan before) { before = t; t += TimeSpan.FromSeconds(1.5); }

    static string Describe(object o) { return o is TimeSpan ? "span " + ((TimeSpan)o).Ticks : "?"; }

    public static string Run()
    {
        TimeSpan a = TimeSpan.FromSeconds(2.5);
        TimeSpan b = new TimeSpan(1, 2, 3);
        TimeSpan c = a;                         // copy
        c = c.Add(TimeSpan.FromMilliseconds(250));
        Lap lap = new Lap();
        string s = a.Ticks + "," + a.TotalSeconds + "," + c.TotalMilliseconds + "," + b.Hours + ":" + b.Minutes + ":" + b.Seconds + "," + b.TotalMinutes;
        s += ";" + (lap.best == TimeSpan.Zero) + "," + (a < b) + "," + (b - a > a) + "," + a.CompareTo(c) + "," + a.Equals(TimeSpan.FromMilliseconds(2500));
        lap.splits[1] = b;
        TimeSpan before;
        Extend(ref lap.splits[1], out before);  // ref to an array element of a native struct
        Extend(ref lap.best, out before);
        s += ";" + lap.splits[1].TotalSeconds + "," + lap.best.Ticks + "," + before.Ticks;
        object boxed = a;                       // boxing with the struct's own type id
        TimeSpan? maybe = null;
        s += ";" + Describe(boxed) + "," + Describe(5) + "," + maybe.HasValue;
        maybe = b;
        s += "," + maybe.Value.Days;
        Dictionary<TimeSpan, string> names = new Dictionary<TimeSpan, string>();
        names[a] = "a";
        names[TimeSpan.FromSeconds(2.5)] = "same key";
        List<TimeSpan> list = new List<TimeSpan> { b, a };
        s += ";" + names.Count + "," + names[a] + "," + list.IndexOf(a) + "," + list.Contains(c);
        try { TimeSpan huge = new TimeSpan(long.MaxValue) + a; s += ";" + huge.Ticks; }
        catch (OverflowException) { s += ";Overflow"; }
        return s;
    }
}

// P/Invoke: a `static extern` method is a host member (HOST_API.md lists it under its class). A void one that the
// host does not implement is recorded and execution continues; a lambda with discard parameters rides along.
public static class NativeExtern
{
    [System.Runtime.InteropServices.DllImport("kernel32.dll")]
    static extern void SetLastError(uint code);

    [System.Runtime.InteropServices.DllImport("kernel32.dll")]
    static extern uint GetTickCount();

    public static string Run()
    {
        Func<int, int, int> pick = (_, __) => 7;
        SetLastError(0);
        uint t = 1;
        if (pick(1, 2) != 7) t = GetTickCount();        // never taken: a value-returning extern without a mapping panics
        return "after extern " + pick(3, 4) + "," + t;
    }
}

// A translated class deriving from a natively implemented class: the native value is embedded as the first field
// (rt::NativeSlot), so the object is also a valid List<int>.
public class Bag : List<int>
{
    public string label;
    public Bag(string label) { this.label = label; }
    public int Total() { int t = 0; for (int i = 0; i < Count; i++) t += this[i]; return t; }
    public void AddTwice(int v) { Add(v); Add(v); }
}
public class BigBag : Bag
{
    public BigBag() : base("big") { }
    public int Twice() { return Total() * 2; }
}
public static class NativeBaseList
{
    static int Sum(List<int> l) { int t = 0; foreach (int v in l) t += v; return t; }
    static int CountOf(IEnumerable<int> e) { int n = 0; foreach (int v in e) n++; return n; }
    public static string Run()
    {
        Bag b = new Bag("small") { 1, 2 };
        b.AddTwice(5);
        b.Add(7);
        string r = b.label + ":" + b.Count + ":" + b.Total() + ":" + b[0] + ";";
        List<int> asList = b;
        asList.Add(100);
        r += Sum(b) + "," + CountOf(b) + "," + b.Contains(100) + "," + (asList == b) + ";";
        object o = b;
        r += (o is Bag) + "," + (o is BigBag) + "," + (((Bag)asList).label) + ";";
        BigBag big = new BigBag();
        big.AddTwice(4);
        Bag viaBase = big;
        r += big.Twice() + "," + viaBase.Total() + "," + (viaBase is BigBag) + ";";
        int n = 0;
        foreach (int v in b) n += v;
        b.RemoveAt(0);
        r += n + "," + b.Count + "," + b.IndexOf(7);
        return r;
    }
}
