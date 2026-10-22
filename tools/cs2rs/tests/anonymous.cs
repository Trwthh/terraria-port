// Anonymous types (DESIGN.md section 8e): creation, property reads, value equality, ToString, as elements of LINQ
// projections, and as the argument of a "format with the properties of this object" helper (the shape of
// Terraria.Utils.FormatWith / Language.GetTextValueWith, here with the lookup written out by position).
using System;
using System.Collections.Generic;
using System.Linq;

public static class Anonymous
{
    static string Describe(object o)
    {
        return o == null ? "null" : o.ToString();
    }

    public static string Run()
    {
        string r = "";
        int count = 3;
        string name = "slime";
        var a = new { Count = count, Name = name };
        var b = new { Count = 3, Name = "slime" };
        var c = new { Count = 4, Name = "slime" };
        r += a.Count + "," + a.Name + ";";
        r += a.Equals(b) + "," + a.Equals(c) + "," + (a.GetHashCode() == b.GetHashCode()) + ";";
        r += Describe(a) + ";";
        // projection initialisers: the property takes the member's name
        List<int> list = new List<int> { 5, 6, 7 };
        var d = new { list.Count, Half = 0.5f, Flag = true, Nothing = (string)null };
        r += d.Count + "," + d.Half + "," + d.Flag + "," + (d.Nothing == null) + ";";
        r += Describe(d) + ";";
        // inside LINQ: elements of an anonymous type
        var pairs = list.Select((v, i) => new { index = i, value = v * 2 }).ToList();
        foreach (var p in pairs) r += p.index + ":" + p.value + ",";
        r += pairs.Count(p => p.value > 10) + ";";
        // evaluation order of the initialisers is source order
        int n = 0;
        var e = new { First = ++n, Second = ++n, Third = n * 10 };
        r += e.First + "" + e.Second + "" + e.Third + ";";
        // nested
        var outer = new { Inner = new { X = 1, Y = 2 }, Tag = "t" };
        r += outer.Inner.X + outer.Inner.Y + outer.Tag + ";" + Describe(outer);
        return r;
    }
}
