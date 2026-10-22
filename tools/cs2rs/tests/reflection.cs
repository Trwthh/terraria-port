// Reflection over translated types (gen::meta), the concurrent dictionary, and the LINQ operators of rt::lq.
// Run on .NET, translated, outputs compared (Snippets.cs).
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;

[AttributeUsage(AttributeTargets.Class | AttributeTargets.Interface, Inherited = false)]
public sealed class RfTagAttribute : Attribute
{
    public readonly string Name;
    public int Level;
    public RfTagAttribute(string name) { Name = name; }
}

public sealed class RfOtherAttribute : Attribute
{
    public Type T;
    public int N { get; set; }
    public RfOtherAttribute(int n, Type t) { N = n; T = t; }
}

[RfTag("alpha", Level = 3)]
public class RfA { }

[RfTag("beta")]
[RfOther(7, typeof(RfA), N = 9)]
public class RfB { }

public class RfNone { }

public enum RfE { One = 1, Five = 5, Three = 3 }

public class RfBase
{
    public int id = 4;
    protected int hidden = 5;
    public static int Shared = 11;
}

public class RfThing : RfBase
{
    public string name = "n";
    public float f = 1.5f;
    private int secret = 9;
    public RfBase other;
    public int[] arr = { 1, 2 };
    public int Twice { get { return id * 2 + secret; } }
}

public struct RfPoint
{
    public int X;
    public int Y;
}

public static class Reflection
{
    static T Attr<T>(Type t) where T : Attribute
    {
        return t.GetCustomAttributes(false).OfType<T>().SingleOrDefault();
    }

    public static string Run()
    {
        string r = "";
        // attributes: constructor arguments, named field / property arguments, a typeof argument
        RfTagAttribute a = Attr<RfTagAttribute>(typeof(RfA));
        r += a.Name + a.Level + "," + (Attr<RfTagAttribute>(typeof(RfNone)) == null) + ",";
        RfOtherAttribute o = Attr<RfOtherAttribute>(typeof(RfB));
        r += o.N + o.T.Name + "," + typeof(RfB).GetCustomAttributes(false).Length + "," + typeof(RfB).GetCustomAttributes(typeof(RfTagAttribute), false).Length + ",";
        r += typeof(RfNone).GetCustomAttributes(false).Length + ",";
        // every call makes new attribute objects
        r += ReferenceEquals(Attr<RfTagAttribute>(typeof(RfA)), Attr<RfTagAttribute>(typeof(RfA))) + ";";

        // instance fields: own level first, then inherited public ones
        RfThing t = new RfThing();
        FieldInfo[] fs = typeof(RfThing).GetFields(BindingFlags.Instance | BindingFlags.Public);
        foreach (FieldInfo f in fs)
        {
            r += f.Name + (f.IsStatic ? "S" : "I") + ",";
        }
        r += typeof(RfThing).GetField("name").GetValue(t) + "," + typeof(RfThing).GetField("id").GetValue(t) + "," + (typeof(RfThing).GetField("secret") == null) + ",";
        typeof(RfThing).GetField("name").SetValue(t, "z");
        typeof(RfThing).GetField("id").SetValue(t, 40);
        typeof(RfThing).GetField("f").SetValue(t, 2.5f);
        typeof(RfThing).GetField("other").SetValue(t, new RfBase());
        r += t.name + t.id + "," + t.f + "," + (t.other != null) + "," + t.Twice + ",";
        typeof(RfBase).GetField("Shared").SetValue(null, 12);
        r += RfBase.Shared + "," + typeof(RfBase).GetField("Shared").IsStatic + ",";
        // a boxed struct is read through its box
        object boxed = new RfPoint { X = 3, Y = 4 };
        r += typeof(RfPoint).GetField("Y").GetValue(boxed) + "," + typeof(RfPoint).GetFields().Length + ";";

        // ConcurrentDictionary (one thread)
        ConcurrentDictionary<string, int> cd = new ConcurrentDictionary<string, int>();
        cd["a"] = 1;
        cd["b"] = 2;
        cd["a"] = 5;
        int v;
        bool hasB = cd.TryGetValue("b", out v);
        int sum = 0;
        foreach (KeyValuePair<string, int> kv in cd.ToArray())
        {
            sum += kv.Value;
        }
        r += cd["a"] + "," + hasB + v + "," + cd.ContainsKey("c") + "," + sum + ",";
        cd.Clear();
        r += cd.ContainsKey("a") + ";";
        return r;
    }
}

public static class LinqMore
{
    static string J(IEnumerable<int> s)
    {
        string r = "";
        foreach (int x in s)
        {
            r += x + " ";
        }
        return r;
    }

    static bool Throws(Action a)
    {
        try { a(); }
        catch (InvalidOperationException) { return true; }
        catch (ArgumentOutOfRangeException) { return true; }
        catch (OverflowException) { return true; }
        return false;
    }

    public static string Run()
    {
        List<int> l = new List<int> { 3, 1, 4, 1, 5, 9, 2, 6 };
        int[] none = new int[0];
        string r = "";
        r += l.All(x => x > 0) + "," + l.All(x => x > 1) + "," + none.All(x => false) + "," + l.Count(x => x > 3) + "," + l.First(x => x > 3) + ",";
        r += none.FirstOrDefault() + "," + l.LastOrDefault() + "," + none.LastOrDefault() + "," + l.Single(x => x == 9) + "," + none.SingleOrDefault() + "," + l.ElementAt(2) + ";";
        r += Throws(() => l.First(x => x > 100)) + "," + Throws(() => l.Single(x => x == 1)) + "," + Throws(() => l.SingleOrDefault()) + "," + Throws(() => l.ElementAt(8)) + "," + Throws(() => none.Max()) + ";";
        r += J(l.Skip(5)) + "|" + J(l.Take(3)) + "|" + J(l.TakeWhile(x => x != 5)) + "|" + J(Enumerable.Reverse(l)) + "|" + J(l.Except(new int[] { 1, 9 })) + "|" + J(l.Intersect(new int[] { 9, 1, 7 })) + "|" + J(l.Zip(new int[] { 10, 20 }, (x, y) => x + y)) + ";";
        r += l.Sum() + "," + new long[] { 5, 6 }.Sum() + "," + l.Sum(x => x * 2) + "," + l.Sum(x => x / 2.0) + "," + l.Average(x => (double)x) + "," + l.Max() + "," + new double[] { 1.5, 0.5 }.Max() + ",";
        r += new string[] { "b", "c", "a" }.Max() + "," + l.Max(x => -x) + "," + l.Min(x => -x) + "," + Throws(() => new int[] { int.MaxValue, 1 }.Sum()) + ";";
        Dictionary<int, string> d = new string[] { "one", "three" }.ToDictionary(s => s.Length);
        r += d[3] + d[5] + "," + Enum.GetValues(typeof(RfE)).Cast<int>().Max() + ";";
        // deferred: the list is read when the result is enumerated
        IEnumerable<int> later = l.Skip(6);
        l.Add(7);
        r += J(later);
        return r;
    }
}

// Number formats, composite formatting and interpolation with a format / an alignment (rt::fmt). The formats whose
// result differs between .NET 8 (where this snippet runs) and .NET Framework 4 (the game) are NOT here: they are
// covered by rt::fmt's unit test against tools\cs2rs\tests\framework\fmt_probe.out, written by the real Framework.
public static class Formats
{
    public static string Run()
    {
        int n = 7;
        float f = 3.25f;
        double d = 1234.5;
        byte b = 255;
        string r = "";
        r += n.ToString("D3") + "," + b.ToString("X2") + "," + n.ToString("X8") + "," + f.ToString("F2") + "," + d.ToString("N1") + "," + d.ToString("0.00") + "," + n.ToString("00") + "," + n.ToString("##") + ";";
        r += string.Format("{0}-{1}", n, "s") + "," + string.Format("[{0,4}][{0,-4}]", n) + "," + string.Format("{0:F2} {1:+0;-#} {2:+0;-#}", f, n, -n) + "," + string.Format("{{{0}}}", n) + "," + string.Format("{0}{1}{2}", null, true, 'c') + ";";
        r += $"{n,3}|{n,-3}|{f:F3}|{d:0.0}|{b:X2}|{{x}}" + "," + $"a{n}b" + "," + string.Format("{0} {1} {2} {3}", 1, 2L, 3.5f, 4.25) + ";";
        object[] args = { 1, "two", 3.0 };
        r += string.Format("{2}/{1}/{0}", args) + ",";
        bool threw = false;
        try { string.Format("{3}", args); }
        catch (FormatException) { threw = true; }
        r += threw;
        // compound assignment on a property whose receiver is a call: the receiver is evaluated once
        RfCounter c = new RfCounter();
        RfCounter.Calls = 0;
        RfCounter.Get(c).Count += 5;
        int v = (RfCounter.Get(c).Count -= 2);
        r += "," + c.Count + v + RfCounter.Calls;
        return r;
    }
}

public class RfCounter
{
    public static int Calls;
    public int Count { get; set; }
    public static RfCounter Get(RfCounter c) { Calls++; return c; }
}
