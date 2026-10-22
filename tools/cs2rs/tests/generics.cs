// Generic classes and generic methods of the translated program (DESIGN.md section 8a): one Rust type per closed
// instantiation, statics per instantiation, generic methods per (method, type arguments).
using System;
using System.Collections.Generic;

public class GBox<T>
{
    public static int made;
    public static T last;
    public T value;
    public GBox(T v) { value = v; made++; last = v; }
    public T Get() { return value; }
    public void Set(T v) { value = v; }
    public bool IsDefault() { return EqualityComparer<T>.Default.Equals(value, default(T)); }
}

public class GPair<A, B>
{
    public A first;
    public B second;
    public GPair(A a, B b) { first = a; second = b; }
    public GPair<B, A> Swap() { return new GPair<B, A>(second, first); }
}

/// uses instantiations that appear nowhere else in the program: GBox<List<T>> and GPair<T, int>
public class GWrap<T>
{
    public GBox<List<T>> items = new GBox<List<T>>(new List<T>());
    public GPair<T, int> Add(T v)
    {
        items.Get().Add(v);
        return new GPair<T, int>(v, items.Get().Count);
    }
}

public struct GCell<T>
{
    public T item;
    public int hits;
    public void Put(T v) { item = v; hits++; }
}

public static class GCache<T>
{
    public static int Id = GCacheIds.Next();
    public static List<T> Store = new List<T>();
}
public static class GCacheIds
{
    static int n;
    public static int Next() { n += 10; return n; }
}

public interface IGShape<T>
{
    T Measure();
    void Grow(T by);
}
public class GSquare : IGShape<int>
{
    public int side;
    public int Measure() { return side * side; }
    public void Grow(int by) { side += by; }
}
public struct GLine : IGShape<int>
{
    public int len;
    public int Measure() { return len; }
    public void Grow(int by) { len += by; }
}
public class GText : IGShape<string>
{
    public string text = "";
    public string Measure() { return text; }
    public void Grow(string by) { text += by; }
}

public abstract class GNode
{
    public abstract string Describe();
    public virtual int Rank() { return 0; }
}
public class GLeaf<T> : GNode
{
    public T payload;
    public static int described;
    public GLeaf(T p) { payload = p; }
    public override string Describe() { described++; return "leaf(" + Generics.Show(payload) + ")"; }
}
public class GRankedLeaf<T> : GLeaf<T>
{
    public int rank;
    public GRankedLeaf(T p, int rank) : base(p) { this.rank = rank; }
    public override int Rank() { return rank; }
    public override string Describe() { return "ranked:" + base.Describe(); }
}

public abstract class GBase<T>
{
    public abstract T Make(int n);
    public virtual string Tag() { return "base"; }
    public string Both(int n) { return Tag() + ":" + Generics.Show(Make(n)); }
}
public class GInts : GBase<int>
{
    public override int Make(int n) { return n * 2; }
}
public class GStrs : GBase<string>
{
    public override string Make(int n) { return new string('x', n); }
    public override string Tag() { return "strs"; }
}

public delegate R GFn<A, R>(A a);

public class GEntity
{
    public int id;
    public static int created;
    public GEntity() { created++; id = created; }
    public virtual string Kind() { return "entity"; }
}
public class GMonster : GEntity
{
    public override string Kind() { return "monster"; }
}

public class GVisitor
{
    public virtual string Visit<U>(U u) { return "v:" + Generics.Show(u); }
}
public class GLoudVisitor : GVisitor
{
    public override string Visit<U>(U u) { return "V!" + Generics.Show(u); }
}

public static class Generics
{
    public static string Show<T>(T v)
    {
        object o = v;
        if (o == null) return "null";
        if (o is int) return "i" + (int)o;
        if (o is string) return "s" + (string)o;
        if (o is bool) return "b" + ((bool)o ? 1 : 0);
        return "?";
    }
    public static T Create<T>() where T : GEntity, new()
    {
        T t = new T();
        t.id += 100;
        return t;
    }
    public static string KindOf<T>(T e) where T : GEntity { return e.Kind() + e.id; }
    public static bool IsNull<T>(T v) { return v == null; }
    public static T OrDefault<T>(bool take, T v) { return take ? v : default(T); }
    public static int GrowTwice<S>(ref S shape) where S : IGShape<int>
    {
        shape.Grow(1);
        shape.Grow(2);
        return shape.Measure();
    }
    public static string TypeName<T>() { return typeof(T).Name; }
    public static R Apply<A, R>(GFn<A, R> f, A a) { return f(a); }
    public static T Max<T>(T a, T b) where T : IComparable<T> { return a.CompareTo(b) >= 0 ? a : b; }
    public static void Swap<T>(ref T a, ref T b) { T t = a; a = b; b = t; }

    public static string Run()
    {
        string r = "";
        // statics per instantiation
        var bi = new GBox<int>(5);
        var bi2 = new GBox<int>(6);
        var bs = new GBox<string>("a");
        r += GBox<int>.made + "," + GBox<string>.made + "," + GBox<int>.last + "," + GBox<string>.last + ";";
        bi.Set(bi.Get() + bi2.Get());
        r += bi.Get() + "," + bi.IsDefault() + "," + new GBox<int>(0).IsDefault() + "," + new GBox<string>(null).IsDefault() + ";";
        // type identity of instantiations
        object ob = bi;
        r += (ob is GBox<int>) + "," + (ob is GBox<string>) + "," + ((ob as GBox<string>) == null) + ";";
        // two type parameters, an instantiation created by generic code
        var p = new GPair<int, string>(1, "one").Swap();
        r += p.first + p.second + ";";
        var w = new GWrap<string>();
        w.Add("x");
        var wp = w.Add("y");
        r += wp.first + wp.second + "," + GBox<List<string>>.made + ";";
        // generic struct: value semantics
        GCell<int> c1 = new GCell<int>();
        c1.Put(4);
        GCell<int> c2 = c1;
        c2.Put(9);
        r += c1.item + "/" + c1.hits + "," + c2.item + "/" + c2.hits + ";";
        // static generic class as type-keyed storage; initialisers run once per instantiation, lazily
        GCache<string>.Store.Add("s");
        int idS = GCache<string>.Id, idI = GCache<int>.Id;
        GCache<int>.Store.Add(1); GCache<int>.Store.Add(2);
        r += idS + "," + idI + "," + GCache<int>.Id + "," + GCache<string>.Store.Count + "," + GCache<int>.Store.Count + ";";
        // generic interface: class, boxed struct, constrained call on a struct in place
        IGShape<int> sq = new GSquare { side = 2 };
        sq.Grow(1);
        IGShape<int> boxed = new GLine { len = 3 };
        boxed.Grow(4);
        GLine line = new GLine { len = 1 };
        int grown = GrowTwice(ref line);
        GSquare sq2 = new GSquare { side = 1 };
        int grown2 = GrowTwice(ref sq2);
        IGShape<string> tx = new GText();
        tx.Grow("ab"); tx.Grow("c");
        r += sq.Measure() + "," + boxed.Measure() + "," + grown + "," + line.len + "," + grown2 + "," + tx.Measure() + ";";
        // non-generic base dispatching to generic subclasses
        GNode[] nodes = { new GLeaf<int>(7), new GLeaf<string>("q"), new GRankedLeaf<int>(8, 3) };
        foreach (GNode n in nodes) r += n.Describe() + "#" + n.Rank() + ",";
        r += GLeaf<int>.described + "," + GLeaf<string>.described + ";";
        // generic base, closed subclasses
        GBase<int> gi = new GInts(); GBase<string> gs = new GStrs();
        r += gi.Both(4) + "," + gs.Both(2) + ";";
        // generic methods: new T(), constraint calls, default, null test, typeof
        GMonster m = Create<GMonster>();
        GEntity e = Create<GEntity>();
        r += KindOf(m) + "," + KindOf(e) + "," + KindOf<GEntity>(m) + "," + GEntity.created + ";";
        r += IsNull(5) + "," + IsNull("s") + "," + IsNull<string>(null) + "," + OrDefault(false, 3) + "," + (OrDefault(false, "s") == null) + "," + OrDefault(true, "t") + ";";
        r += TypeName<int>() + "," + TypeName<string>() + "," + TypeName<GMonster>() + ";";
        r += Max(3, 9) + "," + Max("pear", "apple") + "," + Max(2.5f, 1.5f) + ";";
        int a = 1, b = 2; string sa = "L", sb = "R";
        Swap(ref a, ref b); Swap(ref sa, ref sb);
        r += a + "" + b + sa + sb + ";";
        // generic delegate
        int bonus = 10;
        r += Apply<int, string>(x => "n" + (x + bonus), 5) + "," + Apply<string, int>(s => s.Length, "four") + ";";
        // virtual generic method
        GVisitor v1 = new GVisitor(), v2 = new GLoudVisitor();
        r += v1.Visit(1) + "," + v2.Visit(2) + "," + v2.Visit("z") + ";";
        return r;
    }
}
