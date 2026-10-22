// Named arguments written out of parameter order: C# evaluates them in SOURCE order, whatever the parameter order.
// Constructors (the shape of `new DrawData(position: .., texture: .., ..)` in PlayerDrawLayers), with an object
// initialiser after them, nested creations, and method calls.
using System;
using System.Collections.Generic;

public class NaPoint
{
    public int x, y, z;
    public string tag;
    public NaPoint(int x, int y, int z = 7)
    {
        this.x = x; this.y = y; this.z = z;
    }
    public override string ToString() { return x + "/" + y + "/" + z + (tag == null ? "" : "#" + tag); }
}

public struct NaPair
{
    public NaPoint first;
    public int second;
    public NaPair(NaPoint first, int second) { this.first = first; this.second = second; }
}

public static class NamedArgs
{
    static int n;
    static List<string> log = new List<string>();
    static int Next(string who) { log.Add(who); return ++n; }
    static string Join(string a, string b, string c) { return a + b + c; }

    public static string Run()
    {
        string r = "";
        // y is evaluated first (1), then x (2)
        NaPoint p = new NaPoint(y: Next("y"), x: Next("x"));
        r += p + ";";
        // with an initialiser whose value has a call of its own
        NaPoint q = new NaPoint(z: Next("z"), x: Next("x"), y: Next("y")) { tag = "t" + Next("tag") };
        r += q + ";";
        // nested creation as an out-of-order argument
        NaPair pair = new NaPair(second: Next("s"), first: new NaPoint(y: Next("iy"), x: Next("ix")));
        r += pair.first + "," + pair.second + ";";
        // a method call
        r += Join(c: "c" + Next("c"), a: "a" + Next("a"), b: "b" + Next("b")) + ";";
        // in order: nothing special
        NaPoint o = new NaPoint(x: Next("x"), y: Next("y"));
        r += o + ";" + string.Join(",", log);
        return r;
    }
}
