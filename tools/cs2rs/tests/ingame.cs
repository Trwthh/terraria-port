// Constructs first met when the client entered a world: a `List<T>.Enumerator` held in a local
// (Main.DrawInterface), `ArraySegment<T>` (ItemSlot), array covariance (`NetworkText[]` as `object[]`,
// `string[]` as `object[]`), `++` on a property of a receiver with side effects (AchievementsHelper).
using System;
using System.Collections.Generic;

public class IgLayer
{
    public string Name;
    public bool Keep;
    public static int Drawn;
    public bool Draw() { Drawn++; return Keep; }
}

public class IgCounter
{
    public static int Lookups;
    int _value;
    public int Value { get { return _value; } set { _value = value; } }
    public float Ratio { get; set; }
}

public class IgBase { public virtual string Tag() { return "base"; } public override string ToString() { return "B:" + Tag(); } }
public class IgDerived : IgBase { public string N; public override string Tag() { return "derived" + N; } }

public static class InGameConstructs
{
    static Dictionary<string, IgCounter> _conditions = new Dictionary<string, IgCounter>();

    static IgCounter GetCondition(string name)
    {
        IgCounter.Lookups++;
        if (!_conditions.TryGetValue(name, out var c)) { c = new IgCounter(); _conditions[name] = c; }
        return c;
    }

    static string Layers()
    {
        List<IgLayer> layers = new List<IgLayer>
        {
            new IgLayer { Name = "a", Keep = true }, new IgLayer { Name = "b", Keep = true },
            new IgLayer { Name = "c", Keep = false }, new IgLayer { Name = "d", Keep = true }
        };
        string last = "";
        using (List<IgLayer>.Enumerator enumerator = layers.GetEnumerator())
        {
            while (enumerator.MoveNext() && enumerator.Current.Draw())
            {
                last = enumerator.Current.Name;
            }
        }
        return IgLayer.Drawn + last;
    }

    static int Sum(ArraySegment<int> seg)
    {
        int s = 0;
        for (int i = 0; i < seg.Count; i++) s += seg.Array[i + seg.Offset];
        return s;
    }

    static ArraySegment<int> Middle(int[] a) { return new ArraySegment<int>(a, 2, 3); }

    static string Segments()
    {
        int[] a = { 1, 2, 3, 4, 5, 6, 7 };
        ArraySegment<int> m = Middle(a);
        a[3] = 40;                                       // the segment is a view
        string s = Sum(m) + "," + m.Offset + "," + m.Count + "," + Sum(new ArraySegment<int>(a));
        try { new ArraySegment<int>(a, 5, 3); s += ",ok"; }
        catch (ArgumentException) { s += ",ArgumentException"; }
        return s;
    }

    static string Join(object[] parts)
    {
        string s = "";
        for (int i = 0; i < parts.Length; i++) s += (i > 0 ? "+" : "") + parts[i];
        return s;
    }

    static string Covariance()
    {
        IgDerived[] ds = { new IgDerived { N = "1" }, new IgDerived { N = "2" } };
        IgBase[] bs = ds;
        object[] os = ds;
        string[] names = { "x", null, "z" };
        object[] subs = names;
        string s = Join(os) + ";" + bs[1].Tag() + ";" + string.Format("{0}-{2}", subs) + ";" + (os.Length + subs.Length) + ";" + (bs[0] == os[0]);
        ds[0] = new IgDerived { N = "9" };               // one array under three types
        s += ";" + bs[0].Tag();
        return s;
    }

    static string Counters()
    {
        GetCondition("pick").Value++;
        ((IgCounter)GetCondition("pick")).Value++;
        int was = GetCondition("pick").Value++;
        int now = ++GetCondition("pick").Value;
        GetCondition("run").Ratio += 2.5f;
        GetCondition("run").Ratio--;
        return was + "," + now + "," + GetCondition("pick").Value + "," + GetCondition("run").Ratio + "," + IgCounter.Lookups;
    }

    public static string Run()
    {
        return Layers() + "|" + Segments() + "|" + Covariance() + "|" + Counters();
    }
}
