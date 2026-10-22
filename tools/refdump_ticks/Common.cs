// Shared plumbing of RefDumpWorldgen: loads the real TerrariaServer.exe by reflection (no compile-time reference to the game), the same
// technique as tools\refdump\RefDump.cs (InitGame is a copy of RefDump.InitGame, steps and source line references unchanged), plus a
// tile snapshotter (Expression-tree compiled, so 5 M tiles are read in about a quarter of a second), FNV-1a 64, a zlib writer and a
// reflection JSON walker. C# 5, .NET Framework 4, x86.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Linq.Expressions;
using System.Reflection;
using System.Text;
using System.Threading;

static partial class WG
{
    public static Assembly G;
    public static string GameDir, OutDir, SaveDir, Root;
    public static readonly CultureInfo Inv = CultureInfo.InvariantCulture;
    static readonly Dictionary<string, Assembly> Embedded = new Dictionary<string, Assembly>();
    public const BindingFlags AllStatic = BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic;
    public const BindingFlags AllInst = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;
    public static object MainInst;
    public static Type TMain, TWorldGen, TWorldFile;

    // ------------------------------------------------------------ assembly loading (copy of RefDump.OnResolve)
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

    public static Type T(string name)
    {
        Type t = G.GetType(name, false);
        if (t == null) throw new Exception("type not found: " + name);
        return t;
    }
    public static object SF(Type t, string name)
    {
        FieldInfo f = t.GetField(name, AllStatic);
        if (f != null) return f.GetValue(null);
        PropertyInfo p = t.GetProperty(name, AllStatic);
        if (p != null) return p.GetValue(null, null);
        throw new Exception("static member not found: " + t.FullName + "." + name);
    }
    public static void SSet(Type t, string name, object v)
    {
        FieldInfo f = t.GetField(name, AllStatic);
        if (f != null) { f.SetValue(null, v); return; }
        PropertyInfo p = t.GetProperty(name, AllStatic);
        if (p != null) { p.SetValue(null, v, null); return; }
        throw new Exception("static member not found: " + t.FullName + "." + name);
    }
    public static object IF(object o, string name)
    {
        for (Type t = o.GetType(); t != null; t = t.BaseType)
        {
            FieldInfo f = t.GetField(name, AllInst | BindingFlags.DeclaredOnly);
            if (f != null) return f.GetValue(o);
        }
        PropertyInfo pi = o.GetType().GetProperty(name, AllInst);
        if (pi != null) return pi.GetValue(o, null);
        throw new Exception("instance member not found: " + o.GetType().FullName + "." + name);
    }
    public static void ISet(object o, string name, object v)
    {
        for (Type t = o.GetType(); t != null; t = t.BaseType)
        {
            FieldInfo f = t.GetField(name, AllInst | BindingFlags.DeclaredOnly);
            if (f != null) { f.SetValue(o, v); return; }
        }
        throw new Exception("instance field not found: " + o.GetType().FullName + "." + name);
    }
    public static object CallStatic(Type t, string name, params object[] args)
    {
        foreach (MethodInfo m in t.GetMethods(AllStatic))
            if (m.Name == name && m.GetParameters().Length == args.Length) return Unwrap(m, null, args);
        throw new Exception("static method not found: " + t.FullName + "." + name);
    }
    public static object Unwrap(MethodInfo m, object o, object[] args)
    {
        try { return m.Invoke(o, args); }
        catch (TargetInvocationException e) { throw new Exception(m.DeclaringType.FullName + "." + m.Name + " threw " + e.InnerException.GetType().Name + ": " + e.InnerException.Message, e.InnerException); }
    }

    public static void Step(string desc, Action a)
    {
        try { a(); Console.WriteLine("init ok   " + desc); }
        catch (Exception e) { Console.WriteLine("init FAIL " + desc + " -> " + e.GetType().Name + ": " + e.Message); throw; }
    }

    // ------------------------------------------------------------ game initialisation (RefDump.InitGame, same steps)
    public static void InitGame(Action beforeInit = null, bool tickSeededRand = false)
    {
        AppDomain.CurrentDomain.AssemblyResolve += OnResolve;
        Environment.CurrentDirectory = GameDir;
        Environment.SetEnvironmentVariable("PATH", GameDir + ";" + Environment.GetEnvironmentVariable("PATH"));
        G = Assembly.LoadFrom(Path.Combine(GameDir, "TerrariaServer.exe"));
        TMain = T("Terraria.Main"); TWorldGen = T("Terraria.WorldGen"); TWorldFile = T("Terraria.IO.WorldFile");
        Type program = T("Terraria.Program");
        Step("Program.SavePath = " + SaveDir + "   [Program.cs:187]", () => SSet(program, "SavePath", SaveDir));
        Step("Main.dedServ = true; Main.showSplash = false   [Main.cs:5392-5393]", () => { SSet(TMain, "dedServ", true); SSet(TMain, "showSplash", false); });
        Step("LanguageManager.Instance.SetLanguage(GameCulture.DefaultCulture)   [Program.cs:197]", () =>
        {
            object lm = SF(T("Terraria.Localization.LanguageManager"), "Instance");
            object gc = SF(T("Terraria.Localization.GameCulture"), "DefaultCulture");
            MethodInfo sl = lm.GetType().GetMethods().First(m => m.Name == "SetLanguage" && m.GetParameters().Length == 1 && m.GetParameters()[0].ParameterType == gc.GetType());
            Unwrap(sl, lm, new object[] { gc });
        });
        Step("main = new Main()   [Program.cs:219]", () => MainInst = Activator.CreateInstance(TMain));
        Step("Lang.InitializeLegacyLocalization()   [Program.cs:223]", () => CallStatic(T("Terraria.Lang"), "InitializeLegacyLocalization"));
        // Program.RunGame order [Program.cs:219-229]: new Main(); Lang.InitializeLegacyLocalization(); LaunchInitializer.LoadParameters(main) (command line); main.DedServ().
        // beforeInit stands for LoadParameters: it must run before Initialize() because Initialize reads Main.maxTilesX (WorldGen.RandomizeCaveBackgrounds, Main.cs:6791).
        if (beforeInit != null) beforeInit();
        if (tickSeededRand) Step("Main.rand = new UnifiedRandom()   [Main.cs:5375, Environment.TickCount seed, as the real server]", () => SSet(TMain, "rand", Activator.CreateInstance(T("Terraria.Utilities.UnifiedRandom"))));
        else Step("Main.rand = new UnifiedRandom(0)   [Main.cs:5375, seeded for repeatability]", () => SSet(TMain, "rand", Activator.CreateInstance(T("Terraria.Utilities.UnifiedRandom"), new object[] { 0 })));
        Step("main.Initialize()   [Main.cs:5394]", () =>
        {
            MethodInfo init = TMain.GetMethod("Initialize", AllInst, null, Type.EmptyTypes, null);
            Unwrap(init, MainInst, new object[0]);
        });
    }

    public static void Watchdog(int seconds)
    {
        var wd = new Thread(() => { Thread.Sleep(seconds * 1000); Console.Error.WriteLine("WATCHDOG: timeout"); Environment.Exit(3); });
        wd.IsBackground = true; wd.Start();
    }

    // ------------------------------------------------------------ hashing / zlib
    public const ulong FnvBasis = 14695981039346656037UL, FnvPrime = 1099511628211UL;
    public static ulong Fnv(byte[] b, int off, int len, ulong h)
    {
        for (int i = off, e = off + len; i < e; i++) { h ^= b[i]; h *= FnvPrime; }
        return h;
    }
    public static ulong FnvU64(ulong[] a)
    {
        ulong h = FnvBasis;
        foreach (ulong v in a) for (int k = 0; k < 8; k++) { h ^= (byte)(v >> (8 * k)); h *= FnvPrime; }
        return h;
    }
    public static string Hex(ulong v) { return v.ToString("x16", Inv); }
    public static string Hex(byte[] b) { var sb = new StringBuilder(); foreach (byte x in b) sb.Append(x.ToString("x2")); return sb.ToString(); }
    public static string Sha256(byte[] b) { using (var s = System.Security.Cryptography.SHA256.Create()) return Hex(s.ComputeHash(b)); }

    // RFC 1950 zlib stream (header 78 9C, deflate, big-endian Adler-32) of header+body.
    public static void WriteZlib(string path, byte[] head, byte[] body)
    {
        uint a = 1, b = 0;
        Action<byte[]> adler = buf =>
        {
            int i = 0;
            while (i < buf.Length)
            {
                int n = Math.Min(buf.Length - i, 5552);
                for (int k = 0; k < n; k++) { a += buf[i + k]; b += a; }
                a %= 65521; b %= 65521; i += n;
            }
        };
        adler(head); adler(body);
        using (var fs = new FileStream(path, FileMode.Create))
        {
            fs.WriteByte(0x78); fs.WriteByte(0x9C);
            using (var ds = new DeflateStream(fs, CompressionLevel.Optimal, true))
            {
                ds.Write(head, 0, head.Length);
                ds.Write(body, 0, body.Length);
            }
            uint ad = (b << 16) | a;
            fs.WriteByte((byte)(ad >> 24)); fs.WriteByte((byte)(ad >> 16)); fs.WriteByte((byte)(ad >> 8)); fs.WriteByte((byte)ad);
        }
    }

    // ------------------------------------------------------------ tile snapshot
    // 14-byte record, layout of refdata\framed\*.post.bin (RefDumpFramed.cs:28-29):
    //   u16 type | u16 wall | u8 liquid | u16 sTileHeader | u8 bTileHeader | u8 bTileHeader2 | u8 bTileHeader3 | i16 frameX | i16 frameY (little endian)
    public const int RecSize = 14;
    static Action<Array, int, int, byte[]> _fillColumn;

    static Action<Array, int, int, byte[]> MakeFillColumn()
    {
        Type tileT = T("Terraria.Tile");
        ParameterExpression pa = Expression.Parameter(typeof(Array), "a"), px = Expression.Parameter(typeof(int), "x"),
            ph = Expression.Parameter(typeof(int), "h"), pb = Expression.Parameter(typeof(byte[]), "buf");
        ParameterExpression arr = Expression.Variable(tileT.MakeArrayType(2), "arr"), y = Expression.Variable(typeof(int), "y"),
            o = Expression.Variable(typeof(int), "o"), t = Expression.Variable(tileT, "t");
        LabelTarget brk = Expression.Label("brk");
        var body = new List<Expression>();
        Func<int, Expression, Expression> put8 = (off, v) => Expression.Assign(Expression.ArrayAccess(pb, Expression.Add(o, Expression.Constant(off))), Expression.Convert(v, typeof(byte)));
        Func<int, Expression, Expression> put16 = (off, v) => Expression.Block(
            put8(off, Expression.And(Expression.Convert(v, typeof(int)), Expression.Constant(0xFF))),
            put8(off + 1, Expression.And(Expression.RightShift(Expression.Convert(v, typeof(int)), Expression.Constant(8)), Expression.Constant(0xFF))));
        Func<string, Expression> f = n => Expression.Field(t, n);
        Expression write = Expression.Block(
            put16(0, f("type")), put16(2, f("wall")), put8(4, f("liquid")), put16(5, f("sTileHeader")),
            put8(7, f("bTileHeader")), put8(8, f("bTileHeader2")), put8(9, f("bTileHeader3")), put16(10, f("frameX")), put16(12, f("frameY")));
        Expression loop = Expression.Loop(Expression.Block(
            Expression.IfThen(Expression.GreaterThanOrEqual(y, ph), Expression.Break(brk)),
            Expression.Assign(t, Expression.ArrayAccess(arr, px, y)),
            Expression.IfThen(Expression.NotEqual(t, Expression.Constant(null, tileT)), write),
            Expression.Assign(o, Expression.Add(o, Expression.Constant(RecSize))),
            Expression.Assign(y, Expression.Add(y, Expression.Constant(1)))), brk);
        var block = Expression.Block(new[] { arr, y, o, t },
            Expression.Assign(arr, Expression.Convert(pa, arr.Type)), Expression.Assign(y, Expression.Constant(0)), Expression.Assign(o, Expression.Constant(0)), loop);
        return Expression.Lambda<Action<Array, int, int, byte[]>>(block, pa, px, ph, pb).Compile();
    }

    public class Snap
    {
        public byte[] Buf;            // w*h*14 bytes, column-major
        public ulong[] ColHash;       // FNV-1a 64 of each column's h*14 bytes (offset basis restarted per column)
        public ulong TilesFnv;        // FNV-1a 64 over the whole buffer, no restart
        public ulong ColsFnv;         // FNV-1a 64 over the 8-byte little-endian ColHash values
        public long Active, Walls, Liquid;
        public int W, H;
    }

    public static Snap TakeSnapshot(Snap reuse)
    {
        if (_fillColumn == null) _fillColumn = MakeFillColumn();
        int w = (int)SF(TMain, "maxTilesX"), h = (int)SF(TMain, "maxTilesY");
        Array tile = (Array)SF(TMain, "tile");
        Snap s = reuse != null && reuse.W == w && reuse.H == h ? reuse : new Snap { W = w, H = h, Buf = new byte[(long)w * h * RecSize], ColHash = new ulong[w] };
        byte[] col = new byte[h * RecSize];
        ulong all = FnvBasis;
        long active = 0, walls = 0, liq = 0;
        for (int x = 0; x < w; x++)
        {
            Array.Clear(col, 0, col.Length);
            _fillColumn(tile, x, h, col);
            Buffer.BlockCopy(col, 0, s.Buf, x * col.Length, col.Length);
            s.ColHash[x] = Fnv(col, 0, col.Length, FnvBasis);
            all = Fnv(col, 0, col.Length, all);
            for (int o = 0; o < col.Length; o += RecSize)
            {
                if ((col[o + 5] & 0x20) != 0) active++;      // sTileHeader bit 5 = active
                if (col[o + 2] != 0 || col[o + 3] != 0) walls++;
                if (col[o + 4] != 0) liq++;
            }
        }
        s.TilesFnv = all; s.ColsFnv = FnvU64(s.ColHash); s.Active = active; s.Walls = walls; s.Liquid = liq;
        return s;
    }

    public static void WriteTfrm(string zpath, Snap s)
    {
        byte[] head = new byte[20];
        head[0] = (byte)'T'; head[1] = (byte)'F'; head[2] = (byte)'R'; head[3] = (byte)'M';
        BitConverter.GetBytes(1).CopyTo(head, 4); BitConverter.GetBytes(s.W).CopyTo(head, 8); BitConverter.GetBytes(s.H).CopyTo(head, 12); BitConverter.GetBytes(RecSize).CopyTo(head, 16);
        WriteZlib(zpath, head, s.Buf);
    }

    // ------------------------------------------------------------ UnifiedRandom
    public static object NewRand(int seed) { return Activator.CreateInstance(T("Terraria.Utilities.UnifiedRandom"), new object[] { seed }); }
    public static void RandState(object rand, out uint inext, out int[] seedArray)
    {
        inext = (uint)IF(rand, "inext");
        seedArray = (int[])((int[])IF(rand, "SeedArray")).Clone();
    }
    // the value rand.Next() would return, without consuming from the real generator (works on a copy)
    public static int RandNextPreview(object rand)
    {
        uint inext; int[] sa; RandState(rand, out inext, out sa);
        object c = NewRand(0); ISet(c, "inext", inext); ISet(c, "SeedArray", sa);
        return (int)c.GetType().GetMethod("Next", Type.EmptyTypes).Invoke(c, null);
    }
    public static string RandJson(object rand)
    {
        uint inext; int[] sa; RandState(rand, out inext, out sa);
        var sb = new StringBuilder("{\"inext\":" + inext + ",\"seed\":[");
        for (int i = 0; i < sa.Length; i++) { if (i > 0) sb.Append(','); sb.Append(sa[i]); }
        sb.Append("],\"next\":" + RandNextPreview(rand) + "}");
        return sb.ToString();
    }

    // ------------------------------------------------------------ JSON (reflection walker)
    public static string Q(string s)
    {
        if (s == null) return "null";
        var sb = new StringBuilder("\"");
        foreach (char c in s)
        {
            if (c == '"') sb.Append("\\\""); else if (c == '\\') sb.Append("\\\\");
            else if (c < 32 || c > 126) sb.Append("\\u" + ((int)c).ToString("x4")); else sb.Append(c);
        }
        return sb.Append('"').ToString();
    }
    public static string Num(double d)
    {
        if (double.IsNaN(d)) return "\"NaN\""; if (double.IsPositiveInfinity(d)) return "\"+Inf\""; if (double.IsNegativeInfinity(d)) return "\"-Inf\"";
        return d.ToString("R", Inv);
    }
    public const int ArrayFullMax = 64;

    // Appends a JSON value for v. Rules (documented in refdata\worldgen\README.md): primitives as JSON numbers/bools, enums as their integer,
    // float/double with round-trip format, structs and classes as objects of their instance fields (no properties), arrays/lists of at
    // most 64 elements in full, longer ones as {"len":N,"fnv64":hex} where fnv64 is FNV-1a 64 over the raw little-endian element bytes
    // (primitive element types) or over the UTF-8 bytes of the full JSON text of the elements (other element types).
    public static void J(StringBuilder sb, object v, int depth)
    {
        if (v == null) { sb.Append("null"); return; }
        Type t = v.GetType();
        if (v is bool) { sb.Append((bool)v ? "true" : "false"); return; }
        if (v is string) { sb.Append(Q((string)v)); return; }
        if (t.IsEnum) { sb.Append(Convert.ToInt64(v, Inv).ToString(Inv)); return; }
        if (v is double) { sb.Append(Num((double)v)); return; }
        if (v is float) { sb.Append(Num((double)(float)v)); return; }   // widened exactly; port compares f32 bit patterns after narrowing
        if (v is char) { sb.Append(((int)(char)v).ToString(Inv)); return; }
        if (t.IsPrimitive) { sb.Append(Convert.ToString(v, Inv)); return; }
        if (v is Delegate || v is Type || v is Thread || v is Assembly) { sb.Append("null"); return; }
        if (depth > 4) { sb.Append("\"...\""); return; }
        if (v is Array || (v is IList && t.IsGenericType))
        {
            IList l = (IList)v;
            int n = l.Count;
            Type et = v is Array ? t.GetElementType() : t.GetGenericArguments()[0];
            if (n <= ArrayFullMax)
            {
                sb.Append('[');
                for (int i = 0; i < n; i++) { if (i > 0) sb.Append(','); J(sb, l[i], depth + 1); }
                sb.Append(']');
            }
            else
            {
                ulong h = FnvBasis;
                if (et.IsPrimitive)
                {
                    foreach (object e in l) h = FnvPrimitive(e, h);
                }
                else
                {
                    var tmp = new StringBuilder();
                    for (int i = 0; i < n; i++) { if (i > 0) tmp.Append(','); J(tmp, l[i], depth + 1); }
                    byte[] bytes = Encoding.UTF8.GetBytes(tmp.ToString());
                    h = Fnv(bytes, 0, bytes.Length, h);
                }
                sb.Append("{\"len\":" + n + ",\"fnv64\":\"" + Hex(h) + "\"}");
            }
            return;
        }
        if (v is IDictionary || v is IEnumerable) { sb.Append("\"<collection " + t.Name + ">\""); return; }
        // object / struct
        sb.Append('{');
        bool first = true;
        foreach (FieldInfo f in FieldsOf(t))
        {
            if (!first) sb.Append(','); first = false;
            sb.Append(Q(f.Name)).Append(':');
            J(sb, f.GetValue(v), depth + 1);
        }
        sb.Append('}');
    }
    static ulong FnvPrimitive(object e, ulong h)
    {
        byte[] b;
        if (e is bool) b = new byte[] { (byte)((bool)e ? 1 : 0) };
        else if (e is byte) b = new byte[] { (byte)e };
        else if (e is sbyte) b = new byte[] { (byte)(sbyte)e };
        else if (e is short) b = BitConverter.GetBytes((short)e);
        else if (e is ushort) b = BitConverter.GetBytes((ushort)e);
        else if (e is int) b = BitConverter.GetBytes((int)e);
        else if (e is uint) b = BitConverter.GetBytes((uint)e);
        else if (e is long) b = BitConverter.GetBytes((long)e);
        else if (e is ulong) b = BitConverter.GetBytes((ulong)e);
        else if (e is float) b = BitConverter.GetBytes((float)e);
        else if (e is double) b = BitConverter.GetBytes((double)e);
        else b = Encoding.UTF8.GetBytes(Convert.ToString(e, Inv));
        return Fnv(b, 0, b.Length, h);
    }
    static readonly Dictionary<Type, FieldInfo[]> FieldCache = new Dictionary<Type, FieldInfo[]>();
    public static FieldInfo[] FieldsOf(Type t)
    {
        FieldInfo[] r;
        if (FieldCache.TryGetValue(t, out r)) return r;
        var l = new List<FieldInfo>();
        for (Type c = t; c != null && c != typeof(object) && c != typeof(ValueType); c = c.BaseType)
            foreach (FieldInfo f in c.GetFields(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.DeclaredOnly))
                if (!f.Name.Contains("<")) l.Add(f);
        r = l.ToArray();
        FieldCache[t] = r;
        return r;
    }
    public static string JStr(object v) { var sb = new StringBuilder(); J(sb, v, 0); return sb.ToString(); }

    // all static fields of t that are not const (IsLiteral), as a JSON object, skipping names in skip
    public static string StaticsJson(Type t, Func<FieldInfo, bool> filter)
    {
        var sb = new StringBuilder("{");
        bool first = true;
        foreach (FieldInfo f in t.GetFields(AllStatic | BindingFlags.DeclaredOnly).OrderBy(x => x.Name, StringComparer.Ordinal))
        {
            if (f.IsLiteral || f.IsDefined(typeof(ThreadStaticAttribute), false)) continue;
            if (filter != null && !filter(f)) continue;
            object v;
            try { v = f.GetValue(null); } catch { continue; }
            if (!first) sb.Append(','); first = false;
            sb.Append(Q(f.Name)).Append(':');
            J(sb, v, 0);
        }
        return sb.Append('}').ToString();
    }

    public static string Dir(string p) { Directory.CreateDirectory(p); return p; }
}
