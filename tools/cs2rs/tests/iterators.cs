// `yield` iterator methods (DESIGN.md section 8d): laziness, re-enumeration of IEnumerable<T>, IEnumerator objects
// stepped by hand, nested iterators (the shape of Main.LoadContent_Deferred / LoadMusic_InSteps), `yield break`,
// loops and switches around a yield, an exception leaving through MoveNext, instance iterators reading fields.
using System;
using System.Collections;
using System.Collections.Generic;

public class ItCounter
{
    public int start;
    public List<string> log = new List<string>();

    public IEnumerable<int> Up(int n)
    {
        log.Add("begin" + n);
        for (int i = 0; i < n; i++)
        {
            if (i == 5) yield break;
            yield return start + i;
        }
        log.Add("end" + n);
    }

    public IEnumerator Steps()
    {
        log.Add("s1");
        yield return null;
        log.Add("s2");
        IEnumerator sub = Sub(2);
        while (sub.MoveNext())
        {
            yield return sub.Current;
        }
        log.Add("s3");
        yield return "last";
    }

    IEnumerator Sub(int n)
    {
        for (int i = 0; i < n; i++)
        {
            log.Add("sub" + i);
            yield return i;
        }
    }
}

public static class Iterators
{
    static IEnumerable<string> Words(string text, bool upper)
    {
        string cur = "";
        foreach (char c in text)
        {
            switch (c)
            {
                case ' ':
                    if (cur.Length > 0) { yield return upper ? cur.ToUpper() : cur; cur = ""; }
                    break;
                default:
                    cur += c;
                    break;
            }
        }
        if (cur.Length > 0) yield return cur;
    }

    static IEnumerable<int> Evens(IEnumerable<int> src)
    {
        foreach (int v in src)
            if (v % 2 == 0) yield return v;
    }

    static IEnumerator<int> Throws()
    {
        yield return 1;
        int[] a = new int[1];
        yield return a[3];
        yield return 3;
    }

    static IEnumerable<KeyValuePair<string, int>> Pairs(Dictionary<string, int> d)
    {
        foreach (KeyValuePair<string, int> kv in d)
        {
            yield return new KeyValuePair<string, int>(kv.Key + "!", kv.Value * 2);
        }
    }

    public static string Run()
    {
        string r = "";
        ItCounter c = new ItCounter();
        c.start = 10;
        IEnumerable<int> up = c.Up(3);
        r += c.log.Count + ":";                       // lazy: nothing ran
        foreach (int v in up) r += v + ",";
        c.start = 20;
        foreach (int v in up) r += v + ",";           // a second enumeration starts the body again
        foreach (int v in c.Up(9)) r += v + ",";      // yield break at 5: "end9" is not logged
        r += string.Join("|", c.log) + ";";

        c.log.Clear();
        IEnumerator st = c.Steps();
        r += c.log.Count + ":";
        while (st.MoveNext())
        {
            object cur = st.Current;
            r += (cur == null ? "null" : cur.ToString()) + "@" + c.log.Count + ",";
        }
        r += st.MoveNext() + "," + string.Join("|", c.log) + ";";

        foreach (string w in Words("the quick  brown fox", true)) r += w + ",";
        r += ";";
        foreach (int v in Evens(new List<int> { 1, 2, 3, 4, 6 })) r += v + ",";
        foreach (int v in Evens(Evens(new int[] { 8, 9, 10 }))) r += v + ",";
        r += ";";

        IEnumerator<int> th = Throws();
        r += th.MoveNext() + "" + th.Current;
        try { th.MoveNext(); r += "no"; }
        catch (IndexOutOfRangeException) { r += "caught"; }
        r += th.MoveNext() + ";";

        Dictionary<string, int> d = new Dictionary<string, int>();
        d["a"] = 1; d["b"] = 2;
        foreach (KeyValuePair<string, int> kv in Pairs(d)) r += kv.Key + kv.Value + ",";
        return r;
    }
}
