// Object model: references alias, structs copy, ref/out, statics, inheritance, collections, delegates.
using System.Collections.Generic;
using System.Linq;

public struct Vec
{
    public int X;
    public int Y;
    public Vec(int x, int y) { X = x; Y = y; }
    public int Sum() { return X + Y; }
    public void Grow(int by) { X += by; Y += by; }      // mutates the receiver in place
}

public class Body
{
    public int id;
    public Vec pos;                 // struct field of a class: mutated in place
    public Body next;               // reference: aliases
    public int[] slots = new int[3];
    public string name = "body";
    public Body() { id = -1; }
    public Body(int id) { this.id = id; }
}

public static class StructVsClass
{
    static void ByValue(Vec v) { v.X = 99; }
    static void ByRef(Body b) { b.pos.X = 99; }

    public static string Run()
    {
        Vec a = new Vec(1, 2);
        Vec b = a;                  // copy
        b.X = 10;
        Body p = new Body(1);
        Body q = p;                 // alias
        q.id = 7;
        q.pos.X = 5;                // in-place mutation of a struct field, seen through p
        p.pos.Grow(1);
        Vec snap = p.pos;           // copy out
        snap.Y = 50;
        ByValue(a);                 // callee changes its own copy
        ByRef(p);
        Body[] bodies = new Body[2];        // elements start null
        Vec[] vecs = new Vec[2];            // elements start zeroed
        vecs[1].X = 3;                      // element mutated in place
        Vec copy = vecs[1];
        copy.X = 4;
        bodies[0] = p;
        bodies[0].next = new Body();
        bodies[0].next.pos = a;             // struct assignment copies into the object
        a.X = -1;
        string r = a.X + "," + b.X + ";" + p.id + "," + p.pos.X + "," + p.pos.Y + ";" + snap.X + "," + snap.Y + ";";
        r += (bodies[1] == null) + "," + vecs[0].X + "," + vecs[1].X + "," + copy.X + ";" + p.next.pos.X + "," + p.next.id + "," + (p == q) + "," + (p == p.next) + ";" + p.name + "," + p.slots.Length + "," + new Vec(3, 4).Sum();
        return r;
    }
}

public static class RefOut
{
    static int counter;
    static void Swap(ref int a, ref int b) { int t = a; a = b; b = t; }
    static void AddTwice(ref int target, ref int alias) { target += 1; alias += 1; }   // both may be the same variable
    static bool TryHalf(int v, out int half) { half = v / 2; return v % 2 == 0; }
    static void Bump(ref Vec v) { v.X++; }
    static void Fill(out Vec v) { v = new Vec(8, 9); }

    public static string Run()
    {
        int x = 1, y = 2;
        Swap(ref x, ref y);
        int[] arr = new int[] { 10, 20, 30 };
        Swap(ref arr[0], ref arr[2]);               // refs to array elements
        Body b = new Body(5);
        Swap(ref b.id, ref arr[1]);                 // ref to a field of an object and to an element
        int z = 0;
        AddTwice(ref z, ref z);                     // aliasing: z == 2
        counter = 40;
        AddTwice(ref counter, ref x);               // ref to a static
        int half;
        bool even = TryHalf(9, out half);
        bool even2 = TryHalf(8, out int half2);
        Bump(ref b.pos);                            // ref to a struct field of an object
        Vec[] vs = new Vec[1];
        Bump(ref vs[0]);
        Fill(out Vec made);
        return x + "," + y + ";" + arr[0] + "," + arr[1] + "," + arr[2] + ";" + b.id + ";" + z + ";" + counter + ";" + even + half + even2 + half2 + ";" + b.pos.X + "," + vs[0].X + "," + made.Y;
    }
}

public static class StaticsA
{
    public static string log = "";
    public static int first = Note("A.first", 1);
    public static int fromB = StaticsB.value + 1;      // forces B's initialiser in the middle of A's
    public static int last = Note("A.last", 3);
    public static int Note(string s, int v) { log += s + ">"; return v; }
}

public static class StaticsB
{
    public static int value = StaticsA.Note("B.value", 10);
    public static int seesA = StaticsA.first * 100 + StaticsA.last;   // A.first is set, A.last is still 0
}

public class Counter
{
    public static int created;
    public static readonly int Step;
    public int n = created * 10;        // field initialiser runs before the constructor body
    static Counter() { Step = 5; StaticsA.log += "Counter.cctor>"; }
    public Counter() { created++; n += Step; }
}

public static class StaticInit
{
    public static string Run()
    {
        string r = StaticsA.fromB + ";" + StaticsB.seesA + ";" + StaticsA.last + ";";
        Counter a = new Counter();
        Counter b = new Counter();
        r += a.n + "," + b.n + "," + Counter.created + ";" + StaticsA.log;
        return r;
    }
}

public abstract class Shape
{
    public string tag = "shape";
    public abstract int Area();
    public virtual string Describe() { return tag + ":" + Area(); }
    public string Twice() { return Describe() + Describe(); }      // non-virtual method calling virtual ones
}

public class Square : Shape
{
    public int side;
    public Square(int s) { side = s; tag = "square"; }
    public override int Area() { return side * side; }
}

public class LabeledSquare : Square
{
    public LabeledSquare(int s) : base(s) { tag = "labeled"; }
    public override string Describe() { return "[" + base.Describe() + "]"; }
}

public class Circle : Shape
{
    public override int Area() { return 3; }
    public override string Describe() { return "circle"; }
}

public static class Inheritance
{
    public static string Run()
    {
        Shape[] shapes = new Shape[] { new Square(2), new LabeledSquare(3), new Circle() };
        string r = "";
        foreach (Shape s in shapes) r += s.Describe() + ";";
        r += shapes[1].Twice() + ";";
        Shape sh = shapes[1];
        Square sq = sh as Square;                       // LabeledSquare is a Square
        Circle no = sh as Circle;                       // null
        r += (sq != null) + "," + (no == null) + "," + (sh is LabeledSquare) + "," + (shapes[0] is LabeledSquare) + "," + ((Square)shapes[0]).side;
        try { Circle bad = (Circle)sh; r += bad.tag; }
        catch (System.InvalidCastException) { r += ",bad cast"; }
        return r;
    }
}

public delegate int Op(int a, int b);

public static class Collections
{
    static int Apply(Op op, int a, int b) { return op(a, b); }
    static int Add(int a, int b) { return a + b; }

    public static string Run()
    {
        List<int> list = new List<int>();
        for (int i = 0; i < 5; i++) list.Add(i * i);
        list.RemoveAt(1);
        list.Insert(0, 100);
        List<int> alias = list;                         // same list
        alias.Add(7);
        string r = list.Count + ":";
        foreach (int v in list) r += v + ",";
        r += ";" + list[2] + ";" + list.Contains(16) + list.IndexOf(9);

        Dictionary<int, string> d = new Dictionary<int, string>();
        d[5] = "five"; d[1] = "one"; d.Add(9, "nine");
        d.Remove(1);
        d[2] = "two";                                    // takes the freed slot: order 5, 2, 9
        r += ";";
        foreach (KeyValuePair<int, string> kv in d) r += kv.Key + "=" + kv.Value + ",";
        string got;
        r += ";" + d.ContainsKey(1) + d.TryGetValue(9, out got) + got + d.Count;
        Dictionary<string, int> byName = new Dictionary<string, int> { { "a", 1 }, { "b", 2 } };
        string key = "a";
        key += "";                                       // a different string object with the same content
        r += ";" + byName[key] + byName["b"];

        int bias = 3;
        Op lambda = (a, b) => a * b + bias;              // captures a local that is never reassigned
        r += ";" + Apply(lambda, 2, 5) + "," + Apply(Add, 2, 5);
        System.Func<int, bool> even = v => v % 2 == 0;
        IEnumerable<int> seq = list;
        r += ";" + seq.Any(even) + seq.Any(v => v > 1000) + seq.Count();
        try
        {
            foreach (int v in list) { if (v == 4) list.Add(1); }   // modifying during foreach throws
            r += ";no exception";
        }
        catch (System.InvalidOperationException) { r += ";modified"; }
        return r;
    }
}

public static class NullAndNullable
{
    public static string Run()
    {
        Body b = null;
        string r = "";
        try { r += b.id; }
        catch (System.NullReferenceException) { r += "null deref;"; }
        int? none = null;
        int? some = 5;
        r += none.HasValue + "," + some.HasValue + "," + some.Value + "," + (none ?? 9) + "," + (some ?? 9) + "," + none.GetValueOrDefault() + "," + (none == null) + "," + (some != null) + "," + (some == 5) + ";";
        try { r += none.Value; }
        catch (System.InvalidOperationException) { r += "no value;"; }
        string s = null;
        r += "[" + s + "]" + (s == null) + ("" == null) + (s + "x") + ";";
        int[] arr = null;
        try { r += arr.Length; }
        catch (System.NullReferenceException) { r += "null array;"; }
        int[,] grid = new int[2, 3];
        grid[1, 2] = 7;
        int[][] jag = new int[2][];
        jag[1] = new int[] { 4, 5 };
        r += grid[1, 2] + "," + grid.GetLength(0) + grid.GetLength(1) + grid.Length + "," + (jag[0] == null) + jag[1][1];
        try { grid[2, 0] = 1; }
        catch (System.IndexOutOfRangeException) { r += ",grid range"; }
        return r;
    }
}
