// `unsafe` code: `fixed` on an array element / an array / a field, pointer arithmetic, `p[i]`, `p->f`, `*p`,
// `sizeof`, pointer casts (a byte buffer read as structs), pointer difference and comparison; `ref` locals that
// are re-pointed; a MemoryStream that wraps the caller's array (rt::io).
// The shapes are the ones of LiquidRenderer.InternalPrepareDraw, TileBatch.FillVertexBuffer, CaptureCamera
// .DrawBytesToBuffer, MapHelper.LoadMapVersionCompressed and StateSnapshot.BackupRange.
using System;
using System.IO;

public struct PtCell
{
    public float Level;
    public bool Has;
    public byte Type;
    public PtPair Offset;
}

public struct PtPair
{
    public int X;
    public int Y;
}

public struct PtMapTile
{
    public ushort Type;
    public byte Light;
    public byte Extra;
}

public class PtHolder
{
    public ushort type;
    public ushort wall;
    public byte liquid;
    public PtCell[] cache = new PtCell[9];
}

public static class PointerBasics
{
    static unsafe string Cells()
    {
        PtCell[] cache = new PtCell[7];
        string s = "";
        fixed (PtCell* ptr = &cache[1])
        {
            PtCell* p = ptr;
            for (int i = 0; i < 5; i++)
            {
                p->Level = (float)i / 4f;
                p->Has = i % 2 == 0 && !p[-1].Has;
                p->Type = (byte)(p[-1].Type + 3);
                p->Offset.X = i * 10;
                p->Offset.Y = p[-1].Offset.X;
                p++;
            }
            p = ptr;
            p += 2;
            PtCell copy = p[1];
            PtCell before = p[-2];
            s += copy.Level + "," + copy.Has + "," + copy.Type + "," + copy.Offset.Y + ";" + before.Type + ";";
            p -= 1;
            s += p->Offset.X + "," + (p + 3)->Type + "," + (*p).Level + ";";
            long d = (ptr + 4) - p;
            s += d + "," + (p > ptr) + "," + (p == ptr + 1) + "," + (p != ptr) + ";";
            PtCell* q = p++;
            PtCell* r = ++p;
            s += (q == ptr + 1) + "," + (r == ptr + 3) + "," + (int)(r - q) + ";";
            *q = *r;
            s += cache[2].Offset.X + "," + cache[2].Type;
        }
        return s;
    }

    static unsafe string Bytes()
    {
        byte[] src = new byte[16];
        byte[] dst = new byte[16];
        for (int i = 0; i < 16; i++) src[i] = (byte)(i * 3 + 1);
        fixed (byte* ptr = &dst[0])
        {
            fixed (byte* ptr2 = &src[0])
            {
                byte* a = ptr2;
                byte* b = ptr + (1 << 2);
                for (int i = 0; i < 3; i++)
                {
                    b[2] = *a;
                    b[1] = a[1];
                    *b = a[2];
                    b[3] = a[3];
                    a += 4;
                    b += 4;
                }
            }
        }
        string s = "";
        for (int i = 0; i < 16; i++) s += dst[i] + (i < 15 ? " " : "");
        return s;
    }

    static unsafe string Reinterpret()
    {
        byte[] raw = new byte[sizeof(PtMapTile) * 4];
        for (int i = 0; i < raw.Length; i++) raw[i] = (byte)(i + 1);
        string s = sizeof(PtMapTile) + "," + sizeof(PtPair) + "," + sizeof(int) + ";";
        fixed (byte* ptr = raw)
        {
            PtMapTile* t = (PtMapTile*)ptr;
            for (int n = 0; n < 4; n++)
            {
                t[n].Type = (ushort)(t[n].Type + 1);
                s += t[n].Type + "/" + t[n].Light + "/" + t[n].Extra + " ";
                Bump(ref t[n]);
            }
        }
        s += ";" + raw[0] + "," + raw[2] + "," + raw[14];
        byte[] none = new byte[0];
        byte[] nothing = null;
        fixed (byte* e = none) { s += ";" + (e == null); }
        fixed (byte* e2 = nothing) { s += "," + (e2 == null); }
        return s;
    }

    static void Bump(ref PtMapTile t) { t.Light += 100; }

    // a struct copied out of / into the fields of a class object (StateSnapshot.BackupRange)
    static unsafe string Fields()
    {
        PtHolder h = new PtHolder { type = 513, wall = 7, liquid = 255 };
        ushort got;
        ushort got2;
        fixed (ushort* p = &h.type)
        {
            got = *p;
            *p = 99;
        }
        fixed (ushort* w = &h.wall) { got2 = w[0]; w[0]++; }
        int sum = 0;
        fixed (PtCell* c = &h.cache[0])
        {
            PtCell* k = c;
            for (int i = 0; i < h.cache.Length; i++) { k->Type = (byte)i; k++; }
            k = c;
            while (k < c + h.cache.Length) { sum += k->Type; k++; }
        }
        return got + "," + got2 + "," + h.type + "," + h.wall + "," + sum;
    }

    static string RefLocals()
    {
        int[] a = { 1, 2, 3, 4 };
        ref int r = ref a[0];
        r += 10;
        r = ref a[2];
        r += 100;
        int hits = 0;
        try { _ = ref a[7]; hits = 1; }
        catch (IndexOutOfRangeException) { hits = 2; }
        return a[0] + "," + a[1] + "," + a[2] + "," + a[3] + "," + hits;
    }

    public static string Run()
    {
        return Cells() + "|" + Bytes() + "|" + Reinterpret() + "|" + Fields() + "|" + RefLocals();
    }
}

// new MemoryStream(byte[]) wraps the array (Terraria.DataStructures.CachedBuffer, MessageBuffer)
public static class MemoryStreamWraps
{
    public static string Run()
    {
        byte[] data = new byte[8];
        MemoryStream ms = new MemoryStream(data);
        BinaryWriter w = new BinaryWriter(ms);
        BinaryReader r = new BinaryReader(ms);
        w.Write((short)0x0102);
        w.Write((byte)7);
        string s = data[0] + "," + data[1] + "," + data[2] + ";" + ms.Position + "/" + ms.Length + "/" + ms.Capacity + ";" + ms.CanWrite;
        ms.Position = 0L;
        data[1] = 9;                                   // the stream sees a change of the array
        s += ";" + r.ReadInt16();
        ms.Position = 6;
        try { w.Write(1); s += ";grew"; }
        catch (NotSupportedException) { s += ";NotSupported"; }
        try { ms.GetBuffer(); s += ";buffer"; }
        catch (UnauthorizedAccessException) { s += ";Unauthorized"; }
        ms.SetLength(2);
        s += ";" + ms.Length + "," + ms.ToArray().Length;
        ms.SetLength(4);
        s += "," + data[2] + data[3];

        byte[] part = { 1, 2, 3, 4, 5, 6 };
        MemoryStream ps = new MemoryStream(part, 2, 3, true, true);
        ps.WriteByte(30);
        s += ";" + part[2] + "," + ps.Length + "," + (ps.GetBuffer() == part) + "," + ps.ToArray()[1];
        MemoryStream ro = new MemoryStream(part, false);
        s += ";" + ro.CanWrite;
        try { ro.WriteByte(1); s += ",wrote"; }
        catch (NotSupportedException) { s += ",NotSupported"; }

        MemoryStream g = new MemoryStream(4);
        g.Position = 3;
        g.WriteByte(5);
        g.Position = 10;
        s += ";" + g.Length + "," + g.ReadByte() + "," + g.ToArray()[3] + g.ToArray()[0];
        byte[] live = g.GetBuffer();
        g.Position = 0;
        g.WriteByte(1);
        s += "," + live[0] + "," + (live.Length >= g.Length);
        g.Close();
        s += "," + g.ToArray().Length + "," + g.CanRead;
        return s;
    }
}
