// Object equality as the default comparer of .NET sees it (DESIGN.md section 8c), the virtuals of System.Object
// through `object`, enum boxing, custom event accessors, and the LINQ / reflection members added for start-up.
using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;

public class EqName
{
    public string text;
    public int calls;
    public EqName(string t) { text = t; }
    public override bool Equals(object obj) { EqName o = obj as EqName; return o != null && o.text == text; }
    public override int GetHashCode() { return text.Length; }
    public override string ToString() { return "name:" + text; }
}
public class EqId : IEquatable<EqId>
{
    public int id;
    public EqId(int id) { this.id = id; }
    public bool Equals(EqId other) { return other != null && other.id / 10 == id / 10; }
    public override int GetHashCode() { return id / 10; }
}
public class EqPlain
{
    public int v;
    public EqPlain(int v) { this.v = v; }
}
public struct EqPoint
{
    public int x, y;
    public EqPoint(int x, int y) { this.x = x; this.y = y; }
    public override string ToString() { return "(" + x + "," + y + ")"; }
}
public struct EqMod : IEquatable<EqMod>
{
    public int v;
    public EqMod(int v) { this.v = v; }
    public bool Equals(EqMod o) { return o.v % 5 == v % 5; }
    public override bool Equals(object obj) { return obj is EqMod && Equals((EqMod)obj); }
    public override int GetHashCode() { return v % 5; }
}
public struct EqBare
{
    public int a;
    public string s;
}
public enum EqColor { Red, Green = 4, Blue }
[Flags]
public enum EqBits : byte { None = 0, A = 1, B = 2 }

public static class EqIds
{
    public const short First = 1;
    public const short Second = 2;
    public const int Other = 9;
    public static readonly short Count = 3;
    private static short hidden = 7;
    public static int Hidden() { return hidden; }
}

public interface IEqSource
{
    event Action<int> Changed;
    void Fire(int v);
}
public class EqSource : IEqSource
{
    private List<Action<int>> handlers = new List<Action<int>>();
    public string log = "";
    public event Action<int> Changed
    {
        add { handlers.Add(value); log += "+"; }
        remove { handlers.Remove(value); log += "-"; }
    }
    public void Fire(int v) { foreach (Action<int> h in handlers) h(v); }
}

public static class Equality
{
    static int seen;
    static void OnChanged(int v) { seen += v; }
    static string Need(string s) { return s ?? throw new ArgumentNullException("s"); }

    public static string Run()
    {
        string r = "";
        // a class that overrides Equals / GetHashCode is a key by value
        var d = new Dictionary<EqName, int>();
        d[new EqName("a")] = 1;
        d[new EqName("bb")] = 2;
        d[new EqName("a")] = 3;
        r += d.Count + "," + d[new EqName("a")] + "," + d.ContainsKey(new EqName("cc")) + ";";
        d.Remove(new EqName("a"));
        d[new EqName("ccc")] = 4;
        d[new EqName("dddd")] = 5;
        foreach (KeyValuePair<EqName, int> kv in d) r += kv.Key.text + kv.Value;
        r += ";";
        var names = new List<EqName> { new EqName("x"), new EqName("y"), new EqName("x") };
        r += names.Contains(new EqName("y")) + "," + names.IndexOf(new EqName("x")) + "," + names.Remove(new EqName("x")) + "," + names.Count + "," + names.Distinct().Count() + ";";
        var set = new HashSet<EqName>();
        r += set.Add(new EqName("q")) + "," + set.Add(new EqName("q")) + "," + set.Contains(new EqName("q")) + ";";
        // IEquatable<T> is what the default comparer calls; a class without either compares by identity
        var ids = new List<EqId> { new EqId(11), new EqId(25) };
        r += ids.Contains(new EqId(19)) + "," + ids.IndexOf(new EqId(20)) + "," + EqualityComparer<EqId>.Default.Equals(new EqId(31), new EqId(39)) + ";";
        var p1 = new EqPlain(1);
        var plain = new List<EqPlain> { p1 };
        r += plain.Contains(new EqPlain(1)) + "," + plain.Contains(p1) + "," + object.Equals(p1, p1) + "," + object.Equals(p1, null) + "," + object.Equals(null, null) + ";";
        // structs: field-wise by default, the override / IEquatable when present; boxed values compare by value
        var points = new HashSet<EqPoint> { new EqPoint(1, 2) };
        r += points.Contains(new EqPoint(1, 2)) + "," + points.Contains(new EqPoint(2, 1)) + ",";
        var mods = new Dictionary<EqMod, string>();
        mods[new EqMod(2)] = "two";
        r += mods.ContainsKey(new EqMod(7)) + "," + mods.ContainsKey(new EqMod(3)) + ",";
        EqBare b1 = new EqBare { a = 1, s = "s" }, b2 = new EqBare { a = 1, s = "s" }, b3 = new EqBare { a = 1, s = "t" };
        r += b1.Equals(b2) + "," + b1.Equals(b3) + ",";
        object o1 = 5, o2 = 5, o3 = 5L, os = "txt";
        r += o1.Equals(o2) + "," + o1.Equals(o3) + "," + os.Equals("txt") + "," + new EqPoint(3, 4).Equals(new EqPoint(3, 4)) + ";";
        // ToString: virtual through object, in a concatenation, of a struct, of a class without an override
        object named = new EqName("n");
        r += named + "|" + named.ToString() + "|" + new EqPoint(5, 6) + "|" + new EqPlain(1) + "|" + (object)7 + "|" + 2.5 + ";";
        // enums keep their identity when boxed
        object red = EqColor.Red, green = EqColor.Green, four = 4;
        r += green + "," + green.ToString() + "," + (green is EqColor) + "," + (four is EqColor) + "," + green.Equals(four) + "," + green.Equals(EqColor.Green) + "," + ((EqColor)four) + "," + (int)green + ",";
        r += Enum.GetName(typeof(EqColor), 5) + "," + (Enum.GetName(typeof(EqColor), 3) == null) + "," + (EqColor)Enum.Parse(typeof(EqColor), "Blue") + "," + string.Join("/", Enum.GetNames(typeof(EqColor))) + ",";
        int total = 0;
        foreach (EqColor c in (EqColor[])Enum.GetValues(typeof(EqColor))) total += (int)c;
        EqColor parsed;
        r += total + "," + Enum.TryParse("green", true, out parsed) + parsed + "," + Enum.IsDefined(typeof(EqColor), 4) + "," + (EqBits.A | EqBits.B) + "," + (object)EqBits.B + ";";
        // events with custom accessors, through the class and through the interface
        var src = new EqSource();
        src.Changed += OnChanged;
        IEqSource isrc = src;
        isrc.Changed += OnChanged;
        isrc.Fire(5);
        src.Changed -= OnChanged;
        src.Fire(1);
        r += seen + src.log + ";";
        // throw expression
        string caught = "";
        try { Need(null); } catch (ArgumentNullException) { caught = "null!"; }
        r += Need("ok") + caught + ";";
        // LINQ members of the start-up path
        object[] mixed = { 1, "two", new EqName("n"), "three", null };
        r += string.Join(",", mixed.OfType<string>()) + "|";
        var words = new List<string> { "pear", "fig", "apple", "kiwi", "plum" };
        foreach (IGrouping<int, string> g in words.GroupBy(w => w.Length)) r += g.Key + ":" + string.Join("+", g.ToArray()) + " ";
        r += "|" + string.Join(",", words.OrderBy(w => w.Length).ThenByDescending(w => w[0]).ToArray()) + "|" + string.Join(",", words.OrderByDescending(w => w.Length).ToList()) + "|";
        r += words.RemoveAll(w => w.Length == 4) + "" + words.Count + "|" + string.Join("", new[] { "ab", "cd" }.SelectMany(w => w.ToCharArray().Select(c => c.ToString())).ToArray()) + ";";
        var ll = new LinkedList<int>();
        ll.AddLast(2); ll.AddLast(3); ll.AddFirst(1);
        LinkedListNode<int> node = ll.First.Next;
        ll.Remove(node);
        foreach (int v in ll) r += v;
        r += ll.Count + "" + (ll.First.Next == ll.Last) + ";";
        // reflection over the static fields of a translated class
        List<FieldInfo> shorts = typeof(EqIds).GetFields(BindingFlags.Static | BindingFlags.Public).Where(f => f.FieldType == typeof(short)).ToList();
        shorts.ForEach(f => r += f.Name + "=" + Convert.ToInt32(f.GetValue(null)) + (f.IsLiteral ? "c" : "r") + " ");
        FieldInfo count = typeof(EqIds).GetFields().FirstOrDefault(f => f.Name == "Count");
        r += (count != null) + "," + typeof(EqIds).GetFields().Length + "," + typeof(EqIds).Name;
        return r;
    }
}
