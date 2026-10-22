// Constructs added for the scale-up: boxing, interfaces, using, events, heap cells, lifted operators, enum names,
// game exceptions, collections, generic code over object, long else-if chains, 3-D arrays.
using System;
using System.Collections.Generic;
using System.Linq;

public enum Fruit { Apple, Pear = 5, Plum }

[Flags]
public enum Sides { None = 0, Left = 1, Right = 2, Top = 4, All = 7 }

public interface IShape2
{
    int Area();
    string Name { get; }
}

public class Sq : IShape2
{
    public int side;
    public Sq(int s) { side = s; }
    public int Area() { return side * side; }
    public string Name { get { return "sq"; } }
}

public struct Rc : IShape2
{
    public int w, h;
    public Rc(int w, int h) { this.w = w; this.h = h; }
    public int Area() { return w * h; }
    public string Name { get { return "rc"; } }
}

public class GameError : Exception
{
    public int code;
    public GameError(string message, int code) : base(message) { this.code = code; }
}

public class Res : IDisposable
{
    public static string log = "";
    public string tag;
    public Res(string tag) { this.tag = tag; log += "open " + tag + ";"; }
    public void Dispose() { log += "close " + tag + ";"; }
}

public class Bell
{
    public event Action<int> Rang;
    public static event Action Tick;
    public string heard = "";
    public void OnA(int n) { heard += "a" + n; }
    public void OnB(int n) { heard += "b" + n; }
    public void Ring(int n) { if (Rang != null) Rang(n); }
    public static void DoTick() { if (Tick != null) Tick(); }
    public static int ticks;
    public static void CountTick() { ticks++; }
}

public class Holder3
{
    public List<int> items = new List<int>();
    public Rc box;
    public int n;
}

public static class Boxing
{
    public static string Run()
    {
        object a = 5;
        object b = 5;
        object s = "text";
        object f = 2.5f;
        int? none = null;
        int? some = 7;
        object bn = none;
        object bs = some;
        string r = ((int)a + 1) + "," + (a == b) + "," + (a == a) + ";";
        r += (a is int) + "," + (a is float) + "," + (s is string) + "," + (f is float) + "," + (bn == null) + "," + (bs is int) + "," + (int)bs + ";";
        string cast = "";
        try { float bad = (float)a; cast = "no throw " + bad; }
        catch (InvalidCastException) { cast = "InvalidCast"; }
        try { object nul = null; int x = (int)nul; cast += " no throw " + x; }
        catch (NullReferenceException) { cast += ",NullRef"; }
        string t = s as string;
        string u = a as string;
        r += cast + ";" + t + "," + (u == null) + ";";
        if (s is string str && a is int num) r += str.Length + num;
        object[] mixed = new object[] { 1, "two", 3.5, (short)4, true, 'c', null };
        int ints = 0;
        foreach (object o in mixed) { if (o is int) ints++; else if (o is short) ints += 10; else if (o is bool) ints += 100; else if (o is string) ints += 1000; else if (o == null) ints += 10000; }
        return r + ";" + ints;
    }
}

public static class Interfaces
{
    static int Twice(IShape2 s) { return s.Area() * 2; }

    public static string Run()
    {
        IShape2 a = new Sq(3);
        IShape2 b = new Rc(2, 5);            // boxed struct behind the interface
        Rc local = new Rc(4, 4);
        IShape2 c = local;                   // boxing copies
        local.w = 100;
        object o = a;
        IShape2[] all = { a, b, c };
        int sum = 0;
        string names = "";
        foreach (IShape2 s in all) { sum += s.Area(); names += s.Name; }
        string r = sum + "," + names + "," + Twice(a) + "," + Twice(b) + "," + c.Area() + ";";
        r += (o is IShape2) + "," + (o is Sq) + "," + ((object)local is IShape2) + "," + ((object)5 is IShape2) + ";";
        IShape2 back = o as IShape2;
        Sq sq = a as Sq;
        r += back.Area() + "," + sq.side + "," + (b is Rc) + "," + ((Rc)b).h;
        return r;
    }
}

public static class UsingDispose
{
    static int Early(int n)
    {
        using (Res r = new Res("e" + n))
        {
            if (n > 0) return n * 2;
            Res.log += "body;";
        }
        return -1;
    }

    public static string Run()
    {
        Res.log = "";
        using (Res a = new Res("a"))
        using (Res b = new Res("b"))
        {
            Res.log += "in;";
        }
        int x = Early(3);
        int y = Early(0);
        try
        {
            using (Res t = new Res("t")) { throw new InvalidOperationException("boom"); }
        }
        catch (InvalidOperationException e) { Res.log += "caught " + e.Message + ";"; }
        using (Res n = null) { Res.log += "null ok;"; }
        return Res.log + x + "," + y;
    }
}

public static class Events
{
    public static string Run()
    {
        Bell bell = new Bell();
        bell.Ring(0);                         // no subscribers: nothing
        bell.Rang += bell.OnA;
        bell.Rang += bell.OnB;
        bell.Rang += bell.OnA;
        bell.Ring(1);                         // a1 b1 a1
        bell.Rang -= bell.OnA;                // removes the LAST matching subscription
        bell.Ring(2);                         // a2 b2
        bell.Rang -= bell.OnB;
        bell.Rang -= bell.OnB;                // not subscribed any more: no effect
        bell.Ring(3);                         // a3
        bell.Rang -= bell.OnA;
        bell.Ring(4);                         // nothing: the event is null again
        int local = 0;
        Action bump = () => local += 10;
        Bell.Tick += Bell.CountTick;
        Bell.Tick += bump;
        Bell.DoTick();
        Bell.DoTick();
        Bell.Tick -= bump;
        Bell.DoTick();
        Bell.Tick -= Bell.CountTick;
        Bell.DoTick();
        Action<int> both = bell.OnA;
        both += bell.OnB;
        both(9);
        return bell.heard + ";" + Bell.ticks + "," + local;
    }
}

public static class Captures
{
    public static string Run()
    {
        int counter = 0;                      // captured and reassigned by both sides: one shared variable
        Func<int> next = () => ++counter;
        Action<int> add = n => counter += n;
        next(); next();
        counter += 100;
        add(5);
        int seen = counter;                   // 107
        var fs = new List<Func<int>>();
        for (int i = 0; i < 3; i++)
        {
            int copy = i * 10;                // a fresh variable per iteration
            fs.Add(() => copy++);
        }
        string r = seen + ":";
        foreach (var f in fs) r += f() + "," + f() + ";";
        int total = 0;
        int[] data = { 1, 2, 3, 4 };
        foreach (int v in data) { Action acc = () => total += v; acc(); }
        string label = "sum";
        label += "!";                         // reassigned before the capture, read inside
        Func<string> show = () => label + total;
        return r + show();
    }
}

public static class LiftedNullable
{
    public static string Run()
    {
        int? a = 6, b = 3, n = null;
        float? f = 5f, fn = null;
        string r = (a + b) + "," + (a / b) + "," + ((a + n) == null) + "," + ((a * n).HasValue) + ";";
        r += (a > b) + "," + (a < n) + "," + (n >= n) + "," + (a == b) + "," + (n == null) + "," + (a != n) + ";";
        r += (f / 2f) + "," + ((fn / 2f).HasValue) + "," + (-a) + "," + ((-n).HasValue) + ";";
        float? scaled = a;                    // int? -> float?
        long? wide = n;
        r += scaled + "," + (wide == null) + "," + (a ?? 0) + "," + (n ?? -1) + "," + a.GetValueOrDefault() + "," + n.GetValueOrDefault(9);
        return r;
    }
}

public static class EnumNames
{
    public static string Run()
    {
        Fruit p = Fruit.Pear;
        Fruit unknown = (Fruit)42;
        Sides lr = Sides.Left | Sides.Right;
        Sides odd = (Sides)9;
        string r = p.ToString() + "," + Fruit.Apple + "," + unknown + "," + (Fruit)6 + ";";
        r += lr + "," + Sides.All + "," + Sides.None + "," + (Sides.Left | Sides.Top) + "," + odd + ";";
        r += "x" + Fruit.Plum + (int)Fruit.Plum;
        return r;
    }
}

public static class GameExceptions
{
    static void Fail(int code) { throw new GameError("bad " + code, code); }

    public static string Run()
    {
        string r = "";
        try { Fail(3); }
        catch (GameError e) { r += "game " + e.code + " " + e.Message + ";"; }
        try { Fail(4); }
        catch (InvalidOperationException) { r += "wrong;"; }
        catch (Exception e) { r += "base " + e.Message + ";"; }
        try
        {
            try { throw new ArgumentException("arg"); }
            catch (GameError) { r += "wrong;"; }
            finally { r += "finally;"; }
        }
        catch (ArgumentException e) { r += "outer " + e.Message + ";"; }
        try { try { Fail(5); } catch (GameError) { r += "rethrow;"; throw; } }
        catch (Exception e) { r += (e is GameError) + "," + ((GameError)e).code; }
        return r;
    }
}

public static class MoreCollections
{
    public static string Run()
    {
        var set = new HashSet<int>();
        for (int i = 0; i < 6; i++) set.Add(i * 3);
        bool again = set.Add(6);
        set.Remove(3); set.Remove(9);
        set.Add(100); set.Add(200);            // refill the freed slots, last freed first
        string r = again + ":";
        foreach (int v in set) r += v + ",";
        var stack = new Stack<string>();
        stack.Push("a"); stack.Push("b"); stack.Push("c");
        r += ";" + stack.Pop() + stack.Peek() + stack.Count + ":";
        foreach (string s in stack) r += s;
        var dict = new Dictionary<string, int>();
        dict["one"] = 1; dict["two"] = 2; dict["three"] = 3;
        dict.Remove("two"); dict["four"] = 4;
        r += ";";
        foreach (string k in dict.Keys) r += k + " ";
        foreach (int v in dict.Values) r += v;
        var list = new List<int> { 5, 3, 9, 1, 7 };
        list.Sort();
        r += ";" + string.Join(",", list.Select(v => v.ToString()).ToArray());
        list.Sort((x, y) => y.CompareTo(x));
        list.Reverse();
        r += ";" + list[0] + list[4];
        var evens = list.Where(v => v % 2 == 1 && v > 2).Select(v => v * 10).ToList();
        r += ";" + evens.Count + ":" + evens[0] + ":" + Enumerable.Range(3, 3).Last() + ":" + list.Contains(9);
        var tuple = Tuple.Create(4, "four");
        r += ";" + tuple.Item1 + tuple.Item2;
        int[] src = { 1, 2, 3, 4, 5 };
        int[] dst = new int[7];
        Array.Copy(src, 1, dst, 2, 3);
        Array.Clear(src, 0, 2);
        Array.Resize(ref src, 3);
        r += ";" + string.Join("", dst.Select(v => v.ToString()).ToArray()) + ":" + src.Length + src[2];
        return r;
    }
}

public static class Strings2
{
    public static string Run()
    {
        string s = "  Hello, World  ";
        string t = s.Trim();
        string[] parts = "a,b,,c".Split(',');
        string r = t.Substring(7) + "|" + t.Substring(0, 5).ToLower() + "|" + t.ToUpper().Replace("L", "_") + "|" + parts.Length + parts[2].Length + parts[3];
        r += "|" + string.Join("-", parts) + "|" + t.StartsWith("Hell") + t.EndsWith("x") + t.Contains("lo, W") + t.IndexOf('W');
        r += "|" + string.IsNullOrWhiteSpace("  ") + string.IsNullOrEmpty(t) + "|" + int.Parse(" 42 ") + "|";
        int v;
        r += int.TryParse("x1", out v) + "" + v + int.TryParse("-17", out v) + v;
        foreach (char c in "abc") r += (int)c - 96;
        return r;
    }
}

public static class GenericObjects
{
    // the shape of SetFactory.CreateCustomSet<T>
    static T[] Fill<T>(int size, T defaultState, params object[] inputs)
    {
        T[] array = new T[size];
        for (int i = 0; i < array.Length; i++) array[i] = defaultState;
        for (int j = 0; j < inputs.Length; j += 2)
        {
            T val = (typeof(T).IsPrimitive ? ((T)inputs[j + 1]) : ((typeof(T).IsGenericType && typeof(T).GetGenericTypeDefinition() == typeof(Nullable<>)) ? ((T)inputs[j + 1]) : ((!typeof(T).IsClass) ? ((T)Convert.ChangeType(inputs[j + 1], typeof(T))) : ((T)inputs[j + 1]))));
            if (inputs[j] is ushort) array[(ushort)inputs[j]] = val;
            else if (inputs[j] is int) array[(int)inputs[j]] = val;
            else array[(short)inputs[j]] = val;
        }
        return array;
    }

    static T Clamp<T>(T value, T min, T max) where T : IComparable<T>
    {
        if (value.CompareTo(max) > 0) return max;
        if (value.CompareTo(min) < 0) return min;
        return value;
    }

    static object Box<T>(T value) { return value; }

    public static string Run()
    {
        bool[] flags = Fill<bool>(4, false, (short)1, true, 3, true);
        int[] nums = Fill<int>(4, -1, (ushort)0, 7, (short)2, 9);
        bool?[] maybe = Fill<bool?>(3, null, (short)1, true, (short)2, false);
        string[] names = Fill<string>(3, "none", 1, "one");
        float[] scales = Fill<float>(2, 1f, 0, 0.5f);
        string r = flags[0] + "" + flags[1] + flags[3] + ";" + nums[0] + nums[1] + nums[2] + ";";
        r += (maybe[0] == null) + "" + maybe[1] + maybe[2] + ";" + names[0] + names[1] + ";" + scales[0] + scales[1] + ";";
        r += Clamp(15, 0, 10) + "," + Clamp(-2.5f, -1f, 1f) + "," + Clamp(0.25, 0.0, 1.0) + "," + Clamp("m", "a", "f") + ";";
        r += (Box(3) is int) + "," + (Box("s") is string) + "," + (Box((int?)null) == null) + "," + (int)Box(41);
        return r;
    }
}

public static class LongChains
{
    static int Classify(int n, out int extra)
    {
        extra = 0;
        int r;
        if (n == 0) r = 100;
        else if (n == 1) { r = 101; extra = 1; }
        else if (n == 2) r = 102;
        else if (n < 0) { r = -1; if (n < -5) return -2; }
        else if (n == 4) r = 104;
        else if (n == 5) { r = 105; for (int i = 0; i < 3; i++) { if (i == 1) continue; extra += i; } }
        else if (int.TryParse(n.ToString(), out int parsed) && parsed > 50) r = parsed;
        else if (n == 7) r = 107;
        else r = 999;
        return r + extra;
    }

    public static string Run()
    {
        string r = "";
        int[] inputs = { 0, 1, 2, -1, -9, 4, 5, 60, 7, 8 };
        foreach (int n in inputs) { int e; r += Classify(n, out e) + ":" + e + ","; }
        return r;
    }
}

public static class ArraysAndInitialisers
{
    public static string Run()
    {
        int[,,] cube = new int[2, 3, 4];
        cube[1, 2, 3] = 5;
        cube[0, 1, 2] = cube[1, 2, 3] + 1;
        int[,,] lit = { { { 1, 2 }, { 3, 4 } }, { { 5, 6 }, { 7, 8 } } };
        Holder3 h = new Holder3 { n = 2, items = { 4, 5, 6 }, box = new Rc(2, 3) };
        Holder3 g = new Holder3 { items = { 9 } };
        string r = cube[1, 2, 3] + "" + cube[0, 1, 2] + cube.GetLength(2) + cube.Length + ";" + lit[1, 0, 1] + lit[0, 1, 0] + ";";
        r += h.items.Count + "" + h.items[2] + h.box.Area() + h.n + g.items[0] + ";";
        Rc rc = new Rc(3, 5);
        Func<int> area = rc.Area;             // delegate from a struct method: the struct is copied now
        rc.w = 100;
        Type ti = typeof(int);
        r += area() + ";" + (ti == typeof(int)) + (ti == typeof(float)) + ti.IsPrimitive + typeof(string).IsClass;
        return r;
    }
}
