// Control flow: switch / goto, loops, labels, exceptions, evaluation order.

public static class SwitchGoto
{
    static string Classify(int n)
    {
        string r = "";
        switch (n)
        {
            case 0:
                r += "zero";
                break;
            case 1:
            case 2:
                r += "small";
                goto case 5;
            case 5:
                r += "+five";
                goto default;
            case 7:
                r += "seven";
                goto case 0;
            case -1:
                return "neg";
            default:
                r += "+default";
                break;
        }
        return r;
    }

    static string Words(string s)
    {
        switch (s)
        {
            case "a": return "A";
            case "bb":
            case "cc": return "BC";
            default: return "?";
        }
    }

    public static string Run()
    {
        string r = "";
        for (int i = -1; i <= 8; i++) r += Classify(i) + ";";
        // break / continue inside a switch inside a loop
        int sum = 0;
        for (int i = 0; i < 10; i++)
        {
            switch (i % 4)
            {
                case 0: continue;          // continues the for loop
                case 1: sum += 1; break;   // leaves the switch only
                case 2: sum += 10; break;
                default:
                    if (i > 6) goto done;
                    sum += 100;
                    break;
            }
            sum += 1000;
        }
    done:
        r += sum + ";" + Words("a") + Words("cc") + Words("zz") + Words(null);
        return r;
    }
}

public static class LabeledGoto
{
    public static string Run()
    {
        string r = "";
        int i = 0;
    again:
        r += i;
        i++;
        if (i < 3) goto again;             // backward goto = loop
        r += ";";
        for (int a = 0; a < 5; a++)
        {
            for (int b = 0; b < 5; b++)
            {
                if (a * b == 6) goto found;    // leave two loops
                if (b > a) continue;
                r += a * 10 + b;
                r += ",";
            }
        }
        r += "none";
    found:
        r += ";found";
        int n = 0;
        if (n == 0) goto skip;                 // forward goto over a declaration
        int never = 5;
        r += never;
    skip:
        r += ";end";
        return r;
    }
}

public static class Loops
{
    public static string Run()
    {
        string r = "";
        int n = 6;
        for (int i = 0; i < n; i++)            // bound and index both change in the body
        {
            if (i == 1) n = 4;
            if (i == 2) { i++; continue; }
            r += i + ",";
        }
        r += ";";
        int j = 0;
        do
        {
            j++;
            if (j == 2) continue;              // continue jumps to the condition
            r += j + ",";
        } while (j < 4);
        r += ";";
        int k = 10;
        while (true)
        {
            k -= 3;
            if (k < 0) break;
            r += k + ",";
        }
        r += ";";
        int[] arr = new int[] { 3, 1, 2 };
        int total = 0;
        foreach (int v in arr) { if (v == 1) continue; total += v; }
        r += total;
        for (int x = 0, y = 10; x < y; x += 3, y--) r += ";" + x + "/" + y;
        return r;
    }
}

public static class EvalOrder
{
    static string log = "";
    static int[] data = new int[4];
    static int L(string name, int v) { log += name; return v; }
    static int Three(int a, int b, int c) { return a * 100 + b * 10 + c; }

    public static string Run()
    {
        log = "";
        int r1 = L("a", 1) + L("b", 2) * L("c", 3);              // operands left to right: abc
        log += ";";
        int r2 = Three(L("x", 1), L("y", 2), L("z", 3));          // arguments left to right: xyz
        log += ";";
        int r3 = Three(c: L("p", 3), a: L("q", 1), b: L("r", 2));   // named arguments: source order pqr
        log += ";";
        data[L("i", 1)] = L("v", 7);                               // index before value: iv
        log += ";";
        data[L("j", 2)] += L("w", 5);                              // jw
        log += ";";
        int i = 0;
        data[i++] = i;                                             // data[0] = 1: the index is taken before the value
        int w = 3;
        data[w] = w++ * 2;                                         // data[3] = 6: the index is the old w
        Body holder = new Body(1), other = new Body(2);
        holder.id = (holder = other).id + 40;                      // writes the OLD holder: 42; other keeps 2
        int k = 1;
        k = k++ + k++;                                             // 1 + 2 = 3
        int m = 5;
        int n = m++ * 2 + ++m;                                     // 5*2 + 7 = 17
        int p = 3;
        p += p++;                                                  // 3 + 3 = 6
        int q, s;
        q = s = L("t", 4);                                         // chained assignment evaluates once
        return log + ";" + r1 + ";" + r2 + ";" + r3 + ";" + data[0] + data[1] + data[2] + data[3] + w + ";" + holder.id + other.id + ";" + k + ";" + m + ";" + n + ";" + p + ";" + q + s;
    }
}

public static class ShortCircuit
{
    static string log = "";
    static bool T(string n) { log += n; return true; }
    static bool F(string n) { log += n; return false; }
    public static string Run()
    {
        log = "";
        bool a = F("a") && T("b");          // b not evaluated
        bool b = T("c") || T("d");          // d not evaluated
        bool c = F("e") & T("f");           // both evaluated
        bool d = T("g") | F("h");           // both evaluated
        bool e = T("i") ? F("j") : T("k");  // only one branch
        string s = null;
        string t = s ?? "fallback";
        string u = t ?? "unused";
        int len = (s == null || s.Length == 0) ? -1 : s.Length;   // guards the null dereference
        return log + ";" + a + b + c + d + e + ";" + t + ";" + u + ";" + len;
    }
}

public static class TryCatch
{
    static string log = "";
    static int[] arr = new int[3];

    static int Get(int i)
    {
        try
        {
            return arr[i];                  // out of range is an exception, used as a bounds check
        }
        catch
        {
            return -1;
        }
    }

    static int Finally(int x)
    {
        try
        {
            log += "t";
            if (x == 0) return 10;          // return through finally
            if (x == 1) throw new System.InvalidOperationException("boom");
            log += "n";
        }
        catch (System.InvalidOperationException e)
        {
            log += "c(" + e.Message + ")";
            return 20;
        }
        finally
        {
            log += "f";
        }
        return 30;
    }

    static int Loop()
    {
        int n = 0;
        for (int i = 0; i < 6; i++)
        {
            try
            {
                if (i == 1) continue;        // continue out of a try
                if (i == 4) break;           // break out of a try
                n += arr[i];                 // throws at i == 3
                n += 1;
            }
            catch (System.IndexOutOfRangeException)
            {
                n += 100;
            }
            finally
            {
                n += 10;
            }
        }
        return n;
    }

    static string Nested()
    {
        string r = "";
        try
        {
            try
            {
                string s = null;
                r += s.Length;               // NullReferenceException
            }
            catch (System.IndexOutOfRangeException)
            {
                r += "wrong";
            }
            finally
            {
                r += "inner-finally;";
            }
        }
        catch (System.NullReferenceException)
        {
            r += "outer-null";
        }
        try
        {
            try { throw new System.Exception("a"); }
            catch (System.Exception) { r += ";rethrow"; throw; }
        }
        catch (System.Exception e) { r += ";" + e.Message; }
        return r;
    }

    public static string Run()
    {
        log = "";
        arr[0] = 5; arr[1] = 6; arr[2] = 7;
        string r = Get(1) + ";" + Get(3) + ";" + Get(-1) + ";";
        r += Finally(0) + ";" + Finally(1) + ";" + Finally(2) + ";" + log + ";";
        r += Loop() + ";" + Nested();
        return r;
    }
}
