// RefDump: loads the real TerrariaServer.exe by reflection, runs the game's own
// headless initialisation, and dumps the resulting static data to JSON.
// Pure reflection: no compile-time reference to the game. C# 5, .NET Framework 4, x86.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Threading;

static partial class RefDump
{
    static Assembly G;
    static string GameDir, OutDir, SaveDir;
    static readonly CultureInfo Inv = CultureInfo.InvariantCulture;
    static readonly Dictionary<string, Assembly> Embedded = new Dictionary<string, Assembly>();
    const BindingFlags AllStatic = BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic;
    const BindingFlags AllInst = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;

    // ---- manifest bookkeeping ----
    class FileEntry
    {
        public string File, Call, RowsMeaning;
        public int Rows;
        public string Sha;
        public List<string> Skipped = new List<string>();
        public List<string> Notes = new List<string>();
    }
    static List<FileEntry> Files = new List<FileEntry>();
    static List<string> InitLog = new List<string>();
    static List<string> Failures = new List<string>();
    static FileEntry Cur;   // file currently being produced (receives skip notes)

    static string Ctx = "";
    static void Skip(string what) { what = Ctx + what; if (Cur != null && !Cur.Skipped.Contains(what)) Cur.Skipped.Add(what); }
    static void Note(string what) { what = Ctx + what; if (Cur != null && !Cur.Notes.Contains(what)) Cur.Notes.Add(what); }

    [STAThread]
    static int Main(string[] args)
    {
        if (args.Length > 0 && args[0] == "framed") return MainFramed(args);
        string root = args.Length > 0 ? args[0] : null;
        if (root == null) { Console.Error.WriteLine("usage: RefDump <terraria-project-root>"); return 2; }
        GameDir = Path.Combine(root, "work", "refserver");
        OutDir = Path.Combine(root, "refdata");
        SaveDir = Path.Combine(root, "work", "refdump-save");
        Directory.CreateDirectory(OutDir);
        Directory.CreateDirectory(SaveDir);

        var wd = new Thread(() => { Thread.Sleep(280000); Console.Error.WriteLine("WATCHDOG: timeout"); Environment.Exit(3); });
        wd.IsBackground = true; wd.Start();

        int rc = 0;
        try { Run(); }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }

    // ---------------------------------------------------------------- assembly loading
    static Assembly OnResolve(object sender, ResolveEventArgs a)
    {
        string simple = new AssemblyName(a.Name).Name;
        lock (Embedded)
        {
            Assembly cached;
            if (Embedded.TryGetValue(simple, out cached)) return cached;
            // 1) same lookup the game's WindowsLaunch.Main does: embedded resource ending in "<name>.dll"
            if (G != null)
            {
                string res = Array.Find(G.GetManifestResourceNames(), e => e.EndsWith(simple + ".dll"));
                if (res != null)
                {
                    using (Stream s = G.GetManifestResourceStream(res))
                    {
                        byte[] b = new byte[s.Length];
                        int off = 0; while (off < b.Length) { int n = s.Read(b, off, b.Length - off); if (n <= 0) break; off += n; }
                        Assembly asm = Assembly.Load(b);
                        Embedded[simple] = asm;
                        return asm;
                    }
                }
            }
            // 2) loose dll next to the private server copy (XNA etc.)
            string p = Path.Combine(GameDir, simple + ".dll");
            if (File.Exists(p)) { Assembly asm = Assembly.LoadFrom(p); Embedded[simple] = asm; return asm; }
        }
        return null;
    }

    static Type T(string name)
    {
        Type t = G.GetType(name, false);
        if (t == null) throw new Exception("type not found: " + name);
        return t;
    }
    static object SF(Type t, string name)
    {
        FieldInfo f = t.GetField(name, AllStatic);
        if (f != null) return f.GetValue(null);
        PropertyInfo p = t.GetProperty(name, AllStatic);
        if (p != null) return p.GetValue(null, null);
        throw new Exception("static member not found: " + t.FullName + "." + name);
    }
    static void SSet(Type t, string name, object v)
    {
        FieldInfo f = t.GetField(name, AllStatic);
        if (f != null) { f.SetValue(null, v); return; }
        PropertyInfo p = t.GetProperty(name, AllStatic);
        if (p != null) { p.SetValue(null, v, null); return; }
        throw new Exception("static member not found: " + t.FullName + "." + name);
    }
    static object CallStatic(Type t, string name, params object[] args)
    {
        foreach (MethodInfo m in t.GetMethods(AllStatic))
            if (m.Name == name && m.GetParameters().Length == args.Length) return Unwrap(m, null, args);
        throw new Exception("static method not found: " + t.FullName + "." + name);
    }
    static object Unwrap(MethodInfo m, object o, object[] args)
    {
        try { return m.Invoke(o, args); }
        catch (TargetInvocationException e) { throw new Exception(m.DeclaringType.FullName + "." + m.Name + " threw " + e.InnerException.GetType().Name + ": " + e.InnerException.Message, e.InnerException); }
    }
    // Calls <name>(first, <optional defaults...>) on obj, mirroring a C# call that omits the optional parameters.
    static void CallWithDefaults(object obj, string name, object first)
    {
        MethodInfo pick = null;
        foreach (MethodInfo m in obj.GetType().GetMethods(BindingFlags.Instance | BindingFlags.Public))
        {
            if (m.Name != name) continue;
            ParameterInfo[] ps = m.GetParameters();
            if (ps.Length == 0 || ps[0].ParameterType != first.GetType()) continue;
            bool ok = true;
            for (int i = 1; i < ps.Length; i++) if (!ps[i].IsOptional) ok = false;
            if (ok) { pick = m; break; }
        }
        if (pick == null) throw new Exception("no matching " + name + " on " + obj.GetType().FullName);
        ParameterInfo[] pp = pick.GetParameters();
        object[] a = new object[pp.Length];
        a[0] = first;
        for (int i = 1; i < pp.Length; i++)
        {
            object d = pp[i].DefaultValue;
            if (d == null || d is DBNull || d == Missing.Value)
                d = pp[i].ParameterType.IsValueType ? Activator.CreateInstance(pp[i].ParameterType) : null;
            a[i] = d;
        }
        Unwrap(pick, obj, a);
    }

    // ---------------------------------------------------------------- game initialisation
    static object MainInst;
    static void Step(string desc, Action a)
    {
        try { a(); InitLog.Add("ok   " + desc); Console.WriteLine("init ok   " + desc); }
        catch (Exception e)
        {
            string msg = "FAIL " + desc + " -> " + e.GetType().Name + ": " + e.Message;
            InitLog.Add(msg); Failures.Add(msg); Console.WriteLine("init " + msg);
            throw;
        }
    }

    static void InitGame()
    {
        AppDomain.CurrentDomain.AssemblyResolve += OnResolve;
        Environment.CurrentDirectory = GameDir;
        // native deps (ReLogic.Native.dll) resolve through PATH
        Environment.SetEnvironmentVariable("PATH", GameDir + ";" + Environment.GetEnvironmentVariable("PATH"));
        G = Assembly.LoadFrom(Path.Combine(GameDir, "TerrariaServer.exe"));

        Type program = T("Terraria.Program"), main = T("Terraria.Main");
        // Program.LaunchGame: SavePath = -savedirectory (Program.cs:187)
        Step("Program.SavePath = work\\refdump-save   [Program.cs:187]", () => SSet(program, "SavePath", SaveDir));
        // DedServ(): dedServ = true; showSplash = false (Main.cs:5392-5393)
        Step("Main.dedServ = true; Main.showSplash = false   [Main.cs:5392-5393]", () => { SSet(main, "dedServ", true); SSet(main, "showSplash", false); });
        // Program.RunGame: LanguageManager.Instance.SetLanguage(GameCulture.DefaultCulture) (Program.cs:197)
        Step("LanguageManager.Instance.SetLanguage(GameCulture.DefaultCulture)   [Program.cs:197]", () =>
        {
            object lm = SF(T("Terraria.Localization.LanguageManager"), "Instance");
            object gc = SF(T("Terraria.Localization.GameCulture"), "DefaultCulture");
            MethodInfo sl = lm.GetType().GetMethods().First(m => m.Name == "SetLanguage" && m.GetParameters().Length == 1 && m.GetParameters()[0].ParameterType == gc.GetType());
            Unwrap(sl, lm, new object[] { gc });
        });
        // Program.RunGame: using (Main main = new Main()) (Program.cs:219)
        Step("main = new Main()   [Program.cs:219]", () => MainInst = Activator.CreateInstance(main));
        // Program.RunGame: Lang.InitializeLegacyLocalization() (Program.cs:223)
        Step("Lang.InitializeLegacyLocalization()   [Program.cs:223]", () => CallStatic(T("Terraria.Lang"), "InitializeLegacyLocalization"));
        // Main.DedServ: rand = new UnifiedRandom() (Main.cs:5375). Fixed seed 0 so the dump is repeatable.
        Step("Main.rand = new UnifiedRandom(0)   [Main.cs:5375, seeded]", () =>
        {
            Type ur = T("Terraria.Utilities.UnifiedRandom");
            SSet(main, "rand", Activator.CreateInstance(ur, new object[] { 0 }));
        });
        // Main.DedServ: Initialize() (Main.cs:5394) -> dedServ branch ends in Initialize_AlmostEverything() (Main.cs:6687)
        Step("main.Initialize()   [Main.cs:5394 -> 6635-6693 -> Initialize_AlmostEverything 6726-6866]", () =>
        {
            MethodInfo init = main.GetMethod("Initialize", AllInst, null, Type.EmptyTypes, null);
            Unwrap(init, MainInst, new object[0]);
        });
    }

    // ---------------------------------------------------------------- JSON serialiser (reflection)
    static string Q(string s)
    {
        if (s == null) return "null";
        var sb = new StringBuilder(s.Length + 2);
        sb.Append('"');
        foreach (char c in s)
        {
            switch (c)
            {
                case '"': sb.Append("\\\""); break;
                case '\\': sb.Append("\\\\"); break;
                case '\n': sb.Append("\\n"); break;
                case '\r': sb.Append("\\r"); break;
                case '\t': sb.Append("\\t"); break;
                default:
                    if (c < 0x20 || (c >= 0xD800 && c <= 0xDFFF)) sb.Append("\\u").Append(((int)c).ToString("x4"));
                    else sb.Append(c);
                    break;
            }
        }
        sb.Append('"');
        return sb.ToString();
    }

    static bool IsSimple(Type t)
    {
        return t.IsPrimitive && t != typeof(IntPtr) && t != typeof(UIntPtr) || t == typeof(string) || t.IsEnum || t == typeof(decimal);
    }
    static bool IsSimpleOrXna(Type t)
    {
        if (IsSimple(t)) return true;
        string n = t.FullName;
        return n == "Microsoft.Xna.Framework.Color" || n == "Microsoft.Xna.Framework.Vector2" || n == "Microsoft.Xna.Framework.Rectangle" || n == "Microsoft.Xna.Framework.Point";
    }

    static string Prim(object v)
    {
        Type t = v.GetType();
        if (t.IsEnum) v = Convert.ChangeType(v, Enum.GetUnderlyingType(t), Inv);
        if (v is bool) return (bool)v ? "true" : "false";
        if (v is float) { float f = (float)v; return float.IsNaN(f) || float.IsInfinity(f) ? Q(f.ToString(Inv)) : f.ToString("R", Inv); }
        if (v is double) { double d = (double)v; return double.IsNaN(d) || double.IsInfinity(d) ? Q(d.ToString(Inv)) : d.ToString("R", Inv); }
        if (v is char) return Q(((char)v).ToString());
        return ((IFormattable)v).ToString(null, Inv);
    }

    static List<object> Stack = new List<object>();
    const int MaxDepth = 7;

    // Returns JSON text, or null when the value cannot be represented (caller records a skip).
    static string S(object v, int depth)
    {
        if (v == null) return "null";
        Type t = v.GetType();
        if (v is string) return Q((string)v);
        if (IsSimple(t)) return Prim(v);
        if (t == typeof(IntPtr) || t == typeof(UIntPtr)) return null;
        if (v is Delegate)
        {
            Delegate d = (Delegate)v;
            return Q("<delegate:" + string.Join(";", d.GetInvocationList().Select(x => (x.Method.DeclaringType != null ? x.Method.DeclaringType.FullName : "?") + "." + x.Method.Name).ToArray()) + ">");
        }
        if (v is Type || v is MemberInfo || v is Assembly || v is Stream || v is Module) return null;
        string fn = t.FullName ?? t.Name;
        if (fn.StartsWith("Microsoft.Xna.Framework.Graphics.") || fn.StartsWith("ReLogic.Content.") || fn.StartsWith("System.Threading") || fn.StartsWith("System.Reflection")) return null;
        if (fn == "Microsoft.Xna.Framework.Color")
        {
            Func<string, string> g = n => Prim(t.GetProperty(n).GetValue(v, null));
            return "{\"a\":" + g("A") + ",\"b\":" + g("B") + ",\"g\":" + g("G") + ",\"r\":" + g("R") + "}";
        }
        if (fn == "Terraria.Localization.LocalizedText")
        {
            try { return "{\"key\":" + Q((string)t.GetProperty("Key").GetValue(v, null)) + ",\"value\":" + Q((string)t.GetProperty("Value").GetValue(v, null)) + "}"; }
            catch { return null; }
        }
        if (fn == "Terraria.Item" && depth > 0)
        {
            return "{\"stack\":" + Prim(GetMember(v, "stack")) + ",\"type\":" + Prim(GetMember(v, "type")) + "}";
        }
        if (fn == "Terraria.RecipeGroup")
        {
            string name;
            try { name = Q((string)((Delegate)GetMember(v, "GetText")).DynamicInvoke()); } catch { name = "null"; }
            return "{\"decraftItemId\":" + S(GetMember(v, "DecraftItemId"), depth + 1) + ",\"items\":" + S(GetMember(v, "Items"), depth + 1)
                + ",\"name\":" + name + ",\"registeredId\":" + S(GetMember(v, "RegisteredId"), depth + 1)
                + ",\"validItems\":" + S(GetMember(v, "ValidItems"), depth + 1) + "}";
        }
        if (depth > MaxDepth) return null;

        Array arr = v as Array;
        if (arr != null)
        {
            if (arr.Rank == 1)
            {
                var sb = new StringBuilder("[");
                bool bad = false;
                int i = 0;
                foreach (object e in arr)
                {
                    if (i++ > 0) sb.Append(',');
                    string s = S(e, depth + 1);
                    if (s == null) { bad = true; s = "null"; }
                    sb.Append(s);
                }
                sb.Append(']');
                if (bad) Skip("element(s) of " + t.Name + " not serialisable");
                return sb.ToString();
            }
            return SMulti(arr, 0, new int[arr.Rank], depth);
        }
        IDictionary dict = v as IDictionary;
        if (dict != null)
        {
            var items = new List<KeyValuePair<object, object>>();
            foreach (DictionaryEntry de in dict) items.Add(new KeyValuePair<object, object>(de.Key, de.Value));
            bool allNum = items.All(kv => kv.Key != null && (kv.Key.GetType().IsEnum || (kv.Key.GetType().IsPrimitive && !(kv.Key is bool) && !(kv.Key is char) && !(kv.Key is float) && !(kv.Key is double))));
            Func<object, string> ks = k => k == null ? "null" : (k.GetType().IsEnum ? Prim(k) : (k is string ? (string)k : (k is float || k is double || k is bool || k is char ? Prim(k).Trim('"') : (IsSimple(k.GetType()) ? Prim(k) : k.ToString()))));
            if (allNum) items.Sort((a, b) => Convert.ToDecimal(Convert.ChangeType(a.Key.GetType().IsEnum ? Convert.ChangeType(a.Key, Enum.GetUnderlyingType(a.Key.GetType()), Inv) : a.Key, typeof(decimal), Inv), Inv).CompareTo(Convert.ToDecimal(Convert.ChangeType(b.Key.GetType().IsEnum ? Convert.ChangeType(b.Key, Enum.GetUnderlyingType(b.Key.GetType()), Inv) : b.Key, typeof(decimal), Inv), Inv)));
            else items.Sort((a, b) => string.CompareOrdinal(ks(a.Key), ks(b.Key)));
            var sb = new StringBuilder("{");
            int n = 0; bool bad = false;
            foreach (var kv in items)
            {
                if (n++ > 0) sb.Append(',');
                string s = S(kv.Value, depth + 1);
                if (s == null) { bad = true; s = "null"; }
                sb.Append(Q(ks(kv.Key))).Append(':').Append(s);
            }
            sb.Append('}');
            if (bad) Skip("value(s) of " + t.Name + " not serialisable");
            return sb.ToString();
        }
        IEnumerable en = v as IEnumerable;
        if (en != null)
        {
            var list = new List<object>();
            foreach (object e in en) list.Add(e);
            if (t.Name.StartsWith("HashSet") && list.Count > 0 && list.All(e => e != null && IsSimple(e.GetType()) && !(e is string)))
                list = list.OrderBy(e => Convert.ToDecimal(Convert.ChangeType(e.GetType().IsEnum ? Convert.ChangeType(e, Enum.GetUnderlyingType(e.GetType()), Inv) : e, typeof(decimal), Inv), Inv)).ToList();
            var sb = new StringBuilder("[");
            bool bad = false;
            for (int i = 0; i < list.Count; i++)
            {
                if (i > 0) sb.Append(',');
                string s = S(list[i], depth + 1);
                if (s == null) { bad = true; s = "null"; }
                sb.Append(s);
            }
            sb.Append(']');
            if (bad) Skip("element(s) of " + t.Name + " not serialisable");
            return sb.ToString();
        }

        // plain struct / class: public instance fields, else simple public properties
        if (!t.IsValueType)
        {
            foreach (object o in Stack) if (ReferenceEquals(o, v)) return null;
            Stack.Add(v);
        }
        try
        {
            string body = Members(v, depth, 0);
            return body;
        }
        finally { if (!t.IsValueType) Stack.RemoveAt(Stack.Count - 1); }
    }

    static string SMulti(Array a, int dim, int[] idx, int depth)
    {
        var sb = new StringBuilder("[");
        int lo = a.GetLowerBound(dim), hi = a.GetUpperBound(dim);
        for (int i = lo; i <= hi; i++)
        {
            if (i > lo) sb.Append(',');
            idx[dim] = i;
            if (dim == a.Rank - 1) { string s = S(a.GetValue(idx), depth + 1); sb.Append(s ?? "null"); }
            else sb.Append(SMulti(a, dim + 1, idx, depth));
        }
        sb.Append(']');
        return sb.ToString();
    }

    static object GetMember(object o, string name)
    {
        Type t = o.GetType();
        FieldInfo f = t.GetField(name, BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic);
        if (f != null) return f.GetValue(o);
        PropertyInfo p = t.GetProperty(name, BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic);
        if (p != null) return p.GetValue(o, null);
        throw new Exception("member " + name + " not found on " + t.FullName);
    }

    // propsMode 0: public instance fields (falls back to simple props if there are none)
    //            1: fields + simple public properties (primitive/string/enum/Color/Vector2/Rectangle/Point)
    //            2: fields + every readable public property
    static Dictionary<Type, FieldInfo[]> FieldCache = new Dictionary<Type, FieldInfo[]>();
    static Dictionary<Type, PropertyInfo[]> PropCache = new Dictionary<Type, PropertyInfo[]>();
    static FieldInfo[] FieldsOf(Type t)
    {
        FieldInfo[] r;
        if (!FieldCache.TryGetValue(t, out r))
        {
            r = t.GetFields(BindingFlags.Instance | BindingFlags.Public).Where(f => !f.IsStatic).OrderBy(f => f.Name, StringComparer.Ordinal).ToArray();
            FieldCache[t] = r;
        }
        return r;
    }
    static PropertyInfo[] PropsOf(Type t)
    {
        PropertyInfo[] r;
        if (!PropCache.TryGetValue(t, out r))
        {
            r = t.GetProperties(BindingFlags.Instance | BindingFlags.Public).Where(p => p.CanRead && p.GetIndexParameters().Length == 0 && p.GetGetMethod(false) != null).OrderBy(p => p.Name, StringComparer.Ordinal).ToArray();
            PropCache[t] = r;
        }
        return r;
    }

    static string Members(object v, int depth, int propsMode)
    {
        Type t = v.GetType();
        var parts = new List<KeyValuePair<string, string>>();
        FieldInfo[] fs = FieldsOf(t);
        foreach (FieldInfo f in fs)
        {
            object fv;
            try { fv = f.GetValue(v); } catch (Exception e) { Skip(t.Name + "." + f.Name + " (" + f.FieldType.Name + "): getter threw " + e.GetType().Name); continue; }
            string s = S(fv, depth + 1);
            if (s == null) { Skip(t.Name + "." + f.Name + " (" + f.FieldType.Name + ")"); continue; }
            parts.Add(new KeyValuePair<string, string>(f.Name, s));
        }
        if (propsMode > 0 || fs.Length == 0)
        {
            int mode = propsMode == 0 ? 1 : propsMode;
            foreach (PropertyInfo p in PropsOf(t))
            {
                if (mode == 1 && !IsSimpleOrXna(p.PropertyType)) continue;
                if (parts.Any(x => x.Key == p.Name)) continue;
                object pv;
                try { pv = p.GetValue(v, null); }
                catch (Exception e) { Skip(t.Name + "." + p.Name + " (" + p.PropertyType.Name + "): getter threw " + (e.InnerException ?? e).GetType().Name); continue; }
                string s = S(pv, depth + 1);
                if (s == null) { Skip(t.Name + "." + p.Name + " (" + p.PropertyType.Name + ")"); continue; }
                parts.Add(new KeyValuePair<string, string>(p.Name, s));
            }
        }
        if (parts.Count == 0 && !t.IsPrimitive)
        {
            // no public data at all (e.g. a struct wrapping a private array): read the private instance fields instead
            foreach (FieldInfo f in t.GetFields(BindingFlags.Instance | BindingFlags.NonPublic).Where(f => f.Name.IndexOf('<') < 0).OrderBy(f => f.Name, StringComparer.Ordinal))
            {
                string s = S(f.GetValue(v), depth + 1);
                if (s == null) { Skip(t.Name + "." + f.Name + " (" + f.FieldType.Name + ")"); continue; }
                parts.Add(new KeyValuePair<string, string>(f.Name, s));
            }
            if (parts.Count > 0) Note("type " + t.FullName + " has no public data; private instance fields were read by reflection");
        }
        if (parts.Count == 0) return null;
        parts.Sort((a, b) => string.CompareOrdinal(a.Key, b.Key));
        var sb = new StringBuilder("{");
        for (int i = 0; i < parts.Count; i++)
        {
            if (i > 0) sb.Append(',');
            sb.Append(Q(parts[i].Key)).Append(':').Append(parts[i].Value);
        }
        sb.Append('}');
        return sb.ToString();
    }

    // ---------------------------------------------------------------- output helpers
    static FileEntry Begin(string file, string call, string rowsMeaning)
    {
        var fe = new FileEntry { File = file, Call = call, RowsMeaning = rowsMeaning };
        Files.Add(fe); Cur = fe;
        return fe;
    }
    // entries are already-serialised "key":value pairs; one per line
    static void End(FileEntry fe, List<KeyValuePair<string, string>> top, int rows)
    {
        var sb = new StringBuilder("{\n");
        for (int i = 0; i < top.Count; i++)
        {
            sb.Append(Q(top[i].Key)).Append(':').Append(top[i].Value);
            sb.Append(i + 1 < top.Count ? ",\n" : "\n");
        }
        sb.Append("}\n");
        byte[] bytes = new UTF8Encoding(false).GetBytes(sb.ToString());
        File.WriteAllBytes(Path.Combine(OutDir, fe.File), bytes);
        fe.Rows = rows;
        fe.Sha = Hex(SHA256.Create().ComputeHash(bytes));
        Cur = null;
        Console.WriteLine("wrote " + fe.File + " rows=" + rows + " bytes=" + bytes.Length + " skipped=" + fe.Skipped.Count);
    }
    static string Hex(byte[] b) { var sb = new StringBuilder(); foreach (byte x in b) sb.Append(x.ToString("x2")); return sb.ToString(); }
    static KeyValuePair<string, string> KV(string k, string v) { return new KeyValuePair<string, string>(k, v); }

    static int IntConst(Type t, string name) { FieldInfo f = t.GetField(name, BindingFlags.Static | BindingFlags.Public); return Convert.ToInt32(f.IsLiteral ? f.GetRawConstantValue() : f.GetValue(null)); }

    // ---------------------------------------------------------------- dumps
    static List<FieldInfo> StaticPublicFields(Type t)
    {
        return t.GetFields(BindingFlags.Static | BindingFlags.Public).Where(f => !f.IsLiteral).OrderBy(f => f.Name, StringComparer.Ordinal).ToList();
    }

    static string ArraysOfLength(Type main, int len, out int count)
    {
        var parts = new List<string>();
        foreach (FieldInfo f in StaticPublicFields(main))
        {
            Array a = f.GetValue(null) as Array;
            if (a == null || a.Rank != 1 || a.Length != len) continue;
            if (!IsSimple(f.FieldType.GetElementType())) continue;
            string s = S(a, 0);
            if (s == null) { Skip("Main." + f.Name); continue; }
            parts.Add(Q(f.Name) + ":" + s);
        }
        count = parts.Count;
        return "{\n" + string.Join(",\n", parts.ToArray()) + "\n}";
    }

    static void DumpTileArrays()
    {
        Type main = T("Terraria.Main");
        int tc = IntConst(T("Terraria.ID.TileID"), "Count"), wc = IntConst(T("Terraria.ID.WallID"), "Count");
        FileEntry fe = Begin("tile_arrays.json", "reflection over public static primitive arrays of Terraria.Main with Length == TileID.Count / WallID.Count; Main.tileMerge (bool[][])", "number of arrays (tile + wall) + tileMerge pair entries are not counted");
        int nt, nw;
        string tiles = ArraysOfLength(main, tc, out nt), walls = ArraysOfLength(main, wc, out nw);
        var pairs = new StringBuilder("[");
        bool[][] tm = (bool[][])SF(main, "tileMerge");
        int np = 0;
        for (int a = 0; a < tm.Length; a++)
            if (tm[a] != null)
                for (int b = 0; b < tm[a].Length; b++)
                    if (tm[a][b]) { if (np++ > 0) pairs.Append(','); pairs.Append('[').Append(a).Append(',').Append(b).Append(']'); }
        pairs.Append(']');
        fe.Notes.Add("tileMerge pairs: " + np + " (tileMerge dimensions " + tm.Length + " x " + (tm.Length > 0 && tm[0] != null ? tm[0].Length : 0) + ")");
        fe.Notes.Add("tile arrays: " + nt + ", wall arrays: " + nw + ", tileMerge true pairs: " + np);
        End(fe, new List<KeyValuePair<string, string>> {
            KV("tileCount", tc.ToString(Inv)), KV("wallCount", wc.ToString(Inv)),
            KV("tile", tiles), KV("wall", walls), KV("tileMerge", pairs.ToString()) }, nt + nw);
    }

    static void DumpIdSets()
    {
        FileEntry fe = Begin("id_sets.json", "reflection over every public static non-const field of every Terraria.ID.*+Sets(+Nested) type", "number of Class.Sets.Field entries");
        var entries = new List<KeyValuePair<string, string>>();
        var types = G.GetTypes().Where(t => t.FullName != null && t.FullName.StartsWith("Terraria.ID.") && t.FullName.IndexOf('<') < 0 && t.FullName.Split('+').Skip(1).Contains("Sets"))
            .OrderBy(t => t.FullName, StringComparer.Ordinal).ToList();
        int nTypes = 0;
        foreach (Type t in types)
        {
            nTypes++;
            string cls = t.FullName.Substring("Terraria.ID.".Length).Replace('+', '.');
            foreach (FieldInfo f in StaticPublicFields(t))
            {
                string key = cls + "." + f.Name;
                if (f.FieldType.Name == "SetFactory") { Skip(key + " (Terraria.ID.SetFactory: helper object that creates sets, not data)"); continue; }
                object val;
                try { val = f.GetValue(null); } catch (Exception e) { Skip(key + " (" + f.FieldType.FullName + "): " + (e.InnerException ?? e).GetType().Name); continue; }
                string s;
                Ctx = key + ": ";
                try { Stack.Clear(); s = S(val, 0); } catch (Exception e) { s = null; Skip(key + " (" + f.FieldType.FullName + "): " + e.GetType().Name); continue; }
                if (s == null) { Skip(key + " (" + (val == null ? f.FieldType.FullName : val.GetType().FullName) + ")"); continue; }
                entries.Add(KV(key, s));
            }
        }
        Ctx = "";
        fe.Notes.Add("Sets types visited: " + nTypes);
        entries.Sort((a, b) => string.CompareOrdinal(a.Key, b.Key));
        End(fe, entries, entries.Count);
    }

    static void DumpIdConstants()
    {
        FileEntry fe = Begin("id_constants.json", "FieldInfo.IsLiteral fields (and enum members) of every top-level type in namespace Terraria.ID", "number of ID classes");
        var entries = new List<KeyValuePair<string, string>>();
        int total = 0;
        var types = G.GetTypes().Where(t => t.Namespace == "Terraria.ID" && !t.IsNested && t.FullName.IndexOf('<') < 0).OrderBy(t => t.Name, StringComparer.Ordinal).ToList();
        foreach (Type t in types)
        {
            var parts = new List<KeyValuePair<string, object>>();
            if (t.IsEnum)
            {
                foreach (FieldInfo f in t.GetFields(BindingFlags.Static | BindingFlags.Public))
                    parts.Add(new KeyValuePair<string, object>(f.Name, f.GetRawConstantValue()));
            }
            else
            {
                foreach (FieldInfo f in t.GetFields(BindingFlags.Static | BindingFlags.Public))
                {
                    // true consts, plus static readonly ints such as TileID.Count (compiled as initonly, not literal)
                    if (!f.IsLiteral && !(f.IsInitOnly && IsSimple(f.FieldType) && f.FieldType != typeof(string))) continue;
                    object v = f.IsLiteral ? f.GetRawConstantValue() : f.GetValue(null);
                    if (v == null || !(IsSimple(v.GetType()))) { Skip(t.Name + "." + f.Name + " (" + f.FieldType.Name + ")"); continue; }
                    parts.Add(new KeyValuePair<string, object>(f.Name, v));
                }
            }
            if (parts.Count == 0) continue;
            parts.Sort((a, b) => string.CompareOrdinal(a.Key, b.Key));
            total += parts.Count;
            entries.Add(KV(t.Name, "{" + string.Join(",", parts.Select(p => Q(p.Key) + ":" + S(p.Value, 0)).ToArray()) + "}"));
        }
        fe.Notes.Add("total constants: " + total + (types.Count > 0 ? "; includes the requested TileID WallID ItemID NPCID ProjectileID BuffID PrefixID SoundID DustID GoreID MountID AmmoID MessageID PaintID plus every other Terraria.ID class that has constants" : ""));
        End(fe, entries, entries.Count);
    }

    static string RecordList(string file, string call, int from, int to, Func<int, string> make, out int n)
    {
        var sb = new StringBuilder("[\n");
        n = 0;
        for (int id = from; id <= to; id++)
        {
            string rec;
            try { rec = make(id); }
            catch (Exception e) { Skip("id " + id + ": " + (e.InnerException ?? e).GetType().Name + ": " + (e.InnerException ?? e).Message); continue; }
            if (rec == null) { Skip("id " + id + ": no members"); continue; }
            if (n++ > 0) sb.Append(",\n");
            sb.Append(rec);
        }
        sb.Append("\n]");
        return sb.ToString();
    }

    static string WithId(int id, string members) { return "{\"id\":" + id + "," + members.Substring(1); }

    static void DumpEntities()
    {
        Type itemT = T("Terraria.Item"), npcT = T("Terraria.NPC"), projT = T("Terraria.Projectile");
        int ic = IntConst(T("Terraria.ID.ItemID"), "Count"), nc = IntConst(T("Terraria.ID.NPCID"), "Count"), nneg = IntConst(T("Terraria.ID.NPCID"), "NegativeIDCount"), pc = IntConst(T("Terraria.ID.ProjectileID"), "Count");
        int n;
        FileEntry fe = Begin("items.json", "for id in 1..ItemID.Count-1: new Item(); Item.SetDefaults(id, null); reflect public instance fields + simple public properties", "number of items (ItemID.Count - 1 expected)");
        string s = RecordList(fe.File, fe.Call, 1, ic - 1, id => { object o = Activator.CreateInstance(itemT); CallWithDefaults(o, "SetDefaults", id); Stack.Clear(); return WithId(id, Members(o, 0, 1)); }, out n);
        End(fe, new List<KeyValuePair<string, string>> { KV("itemCount", ic.ToString(Inv)), KV("items", s) }, n);

        fe = Begin("npcs.json", "for id in NPCID.NegativeIDCount+1..NPCID.Count-1 (NegativeIDCount = -66 is the sentinel below the lowest netID -65): new NPC(); NPC.SetDefaults(id, default(NPCSpawnParams)); reflect public instance fields + simple public properties", "number of NPC records (negative ids + 0..Count-1)");
        s = RecordList(fe.File, fe.Call, nneg + 1, nc - 1, id => { object o = Activator.CreateInstance(npcT); CallWithDefaults(o, "SetDefaults", id); Stack.Clear(); return WithId(id, Members(o, 0, 1)); }, out n);
        End(fe, new List<KeyValuePair<string, string>> { KV("npcCount", nc.ToString(Inv)), KV("negativeIdCount", nneg.ToString(Inv)), KV("npcs", s) }, n);

        fe = Begin("projectiles.json", "for id in 1..ProjectileID.Count-1: new Projectile(); Projectile.SetDefaults(id); reflect public instance fields + simple public properties", "number of projectiles (ProjectileID.Count - 1 expected)");
        s = RecordList(fe.File, fe.Call, 1, pc - 1, id => { object o = Activator.CreateInstance(projT); CallWithDefaults(o, "SetDefaults", id); Stack.Clear(); return WithId(id, Members(o, 0, 1)); }, out n);
        End(fe, new List<KeyValuePair<string, string>> { KV("projectileCount", pc.ToString(Inv)), KV("projectiles", s) }, n);
    }

    static void DumpBuffs()
    {
        Type main = T("Terraria.Main");
        int bc = IntConst(T("Terraria.ID.BuffID"), "Count");
        FileEntry fe = Begin("buffs.json", "reflection over public static primitive arrays of Terraria.Main with Length == BuffID.Count", "number of arrays");
        int na;
        string arrays = ArraysOfLength(main, bc, out na);
        End(fe, new List<KeyValuePair<string, string>> { KV("buffCount", bc.ToString(Inv)), KV("arrays", arrays) }, na);
    }

    static void DumpRecipes()
    {
        Type recipeT = T("Terraria.Recipe"), main = T("Terraria.Main");
        FileEntry fe = Begin("recipes.json", "Main.recipe[0..Recipe.numRecipes-1] after Recipe.SetupRecipes(); RecipeGroup.recipeGroups; Recipe.TileUsedInRecipes / TileCountsAs", "number of recipes (Recipe.numRecipes)");
        int num = (int)SF(recipeT, "numRecipes");
        Array rec = (Array)SF(main, "recipe");
        var sb = new StringBuilder("[\n");
        int n = 0;
        for (int i = 0; i < num; i++)
        {
            object r = rec.GetValue(i);
            Stack.Clear();
            // fields; trailing empty slots of requiredItem / requiredItemQuickLookup are trimmed (they are fixed-size 15 arrays)
            var parts = new List<KeyValuePair<string, string>>();
            foreach (FieldInfo f in FieldsOf(recipeT))
            {
                object v = f.GetValue(r);
                string s;
                if (f.Name == "requiredItem")
                {
                    Array a = (Array)v; int last = -1;
                    for (int k = 0; k < a.Length; k++) { object it = a.GetValue(k); if (it != null && (Convert.ToInt32(GetMember(it, "type")) != 0 || Convert.ToInt32(GetMember(it, "stack")) != 0)) last = k; }
                    var l = new List<string>();
                    for (int k = 0; k <= last; k++) l.Add(S(a.GetValue(k), 1) ?? "null");
                    s = "[" + string.Join(",", l.ToArray()) + "]";
                }
                else if (f.Name == "requiredItemQuickLookup")
                {
                    Array a = (Array)v; int last = -1;
                    for (int k = 0; k < a.Length; k++) { object e = a.GetValue(k); if (Convert.ToInt32(GetMember(e, "itemIdOrRecipeGroup")) != 0 || Convert.ToInt32(GetMember(e, "stack")) != 0) last = k; }
                    var l = new List<string>();
                    for (int k = 0; k <= last; k++) l.Add(S(a.GetValue(k), 1) ?? "null");
                    s = "[" + string.Join(",", l.ToArray()) + "]";
                }
                else s = S(v, 1);
                if (s == null) { Skip("Recipe." + f.Name + " (" + f.FieldType.Name + ")"); continue; }
                parts.Add(KV(f.Name, s));
            }
            if (n++ > 0) sb.Append(",\n");
            sb.Append("{\"index\":").Append(i);
            foreach (var p in parts) sb.Append(',').Append(Q(p.Key)).Append(':').Append(p.Value);
            sb.Append('}');
        }
        sb.Append("\n]");
        Stack.Clear();
        string groups = S(SF(T("Terraria.RecipeGroup"), "recipeGroups"), 0);
        string used = S(SF(recipeT, "TileUsedInRecipes"), 0);
        string counts = S(SF(recipeT, "TileCountsAs"), 0);
        fe.Notes.Add("requiredItem and requiredItemQuickLookup have trailing empty slots trimmed (fixed 15-slot arrays); all other fields are raw");
        fe.Notes.Add("Recipe.requiredTile in this build is a single int (-1 = none), not a list");
        End(fe, new List<KeyValuePair<string, string>> {
            KV("numRecipes", num.ToString(Inv)), KV("recipes", sb.ToString()),
            KV("recipeGroups", groups ?? "null"), KV("tileUsedInRecipes", used ?? "null"), KV("tileCountsAs", counts ?? "null") }, n);
    }

    static void DumpTileObjectData()
    {
        Type tod = T("Terraria.ObjectData.TileObjectData");
        int tc = IntConst(T("Terraria.ID.TileID"), "Count");
        FileEntry fe = Begin("tile_object_data.json", "TileObjectData._data (private static List) after TileObjectData.Initialize(); per tile: base + SubTiles (private prop) + Alternates (private prop); values read through the public properties (parent fallback applied)", "number of tile types with object data");
        fe.Notes.Add("non-public access: TileObjectData._data (static field), TileObjectData.SubTiles and .Alternates (private properties) read by reflection");
        IList data = (IList)tod.GetField("_data", AllStatic).GetValue(null);
        PropertyInfo subP = tod.GetProperty("SubTiles", AllInst), altP = tod.GetProperty("Alternates", AllInst);
        Func<object, string> props = o => { Stack.Clear(); return Members(o, 0, 2); };
        Func<object, string> alts = o =>
        {
            IList al = (IList)altP.GetValue(o, null);
            if (al == null) return "[]";
            var l = new List<string>();
            for (int a = 0; a < al.Count; a++) l.Add(al[a] == null ? "null" : "{\"alternate\":" + (a + 1) + ",\"data\":" + (props(al[a]) ?? "null") + "}");
            return "[" + string.Join(",", l.ToArray()) + "]";
        };
        var sb = new StringBuilder("[\n");
        int n = 0;
        for (int type = 0; type < data.Count; type++)
        {
            object d = data[type];
            if (d == null) continue;
            IList subs = (IList)subP.GetValue(d, null);
            var st = new List<string>();
            if (subs != null)
                for (int s = 0; s < subs.Count; s++)
                {
                    object sd = subs[s];
                    bool own = sd != null;
                    object eff = own ? sd : d;   // TileObjectData.GetTileData(type, style): falls back to the base when the sub-tile slot is null
                    st.Add("{\"style\":" + s + ",\"explicit\":" + (own ? "true" : "false") + ",\"data\":" + (props(eff) ?? "null") + ",\"alternates\":" + alts(eff) + "}");
                }
            if (n++ > 0) sb.Append(",\n");
            sb.Append("{\"tile\":").Append(type).Append(",\"base\":").Append(props(d) ?? "null").Append(",\"alternates\":").Append(alts(d)).Append(",\"styles\":[").Append(string.Join(",", st.ToArray())).Append("]}");
        }
        sb.Append("\n]");
        fe.Notes.Add("_data list length " + data.Count + " (TileID.Count " + tc + ")");
        End(fe, new List<KeyValuePair<string, string>> { KV("tileCount", tc.ToString(Inv)), KV("tileObjectData", sb.ToString()) }, n);
    }

    static void DumpFraming()
    {
        Type fr = T("Terraria.Framing");
        FileEntry fe = Begin("framing_tables.json", "reflection over every static non-const field (public and private) of Terraria.Framing after Framing.Initialize()", "number of static fields");
        var entries = new List<KeyValuePair<string, string>>();
        foreach (FieldInfo f in fr.GetFields(AllStatic).Where(f => !f.IsLiteral && f.Name.IndexOf('<') < 0).OrderBy(f => f.Name, StringComparer.Ordinal))
        {
            Stack.Clear();
            string s = S(f.GetValue(null), 0);
            if (s == null) { Skip("Framing." + f.Name + " (" + f.FieldType.Name + ")"); continue; }
            entries.Add(KV(f.Name + (f.IsPublic ? "" : ""), s));
            if (!f.IsPublic) fe.Notes.Add("non-public: " + f.Name);
        }
        End(fe, entries, entries.Count);
    }

    static void DumpLang()
    {
        Type lang = T("Terraria.Lang");
        FileEntry fe = Begin("lang_en.json", "Lang.GetItemNameValue / GetNPCNameValue / GetProjectileName(.Value) / GetBuffName / GetBuffDescription / Lang.prefix[].Value", "number of name tables");
        object lm = SF(T("Terraria.Localization.LanguageManager"), "Instance");
        object cult = GetMember(lm, "ActiveCulture");
        string cultName = Convert.ToString(GetMember(cult, "Name"), Inv);
        fe.Notes.Add("ActiveCulture.Name = " + cultName);
        int ic = IntConst(T("Terraria.ID.ItemID"), "Count"), nc = IntConst(T("Terraria.ID.NPCID"), "Count"), nneg = IntConst(T("Terraria.ID.NPCID"), "NegativeIDCount"),
            pc = IntConst(T("Terraria.ID.ProjectileID"), "Count"), bc = IntConst(T("Terraria.ID.BuffID"), "Count"), xc = IntConst(T("Terraria.ID.PrefixID"), "Count");
        Func<int, int, Func<int, string>, string> table = (from, to, get) =>
        {
            var l = new List<string>();
            for (int id = from; id <= to; id++)
            {
                string v;
                try { v = get(id); } catch (Exception e) { Skip("id " + id + ": " + (e.InnerException ?? e).GetType().Name); continue; }
                if (v == null) continue;
                l.Add(Q(id.ToString(Inv)) + ":" + Q(v));
            }
            return "{" + string.Join(",", l.ToArray()) + "}";
        };
        MethodInfo gi = lang.GetMethod("GetItemNameValue"), gn = lang.GetMethod("GetNPCNameValue"), gp = lang.GetMethod("GetProjectileName"), gb = lang.GetMethod("GetBuffName"), gd = lang.GetMethod("GetBuffDescription");
        Array pref = (Array)SF(lang, "prefix");
        Func<MethodInfo, int, object> call = (m, id) => Unwrap(m, null, new object[] { id });
        var entries = new List<KeyValuePair<string, string>> {
            KV("culture", Q(cultName)),
            KV("item", table(1, ic - 1, id => (string)call(gi, id))),
            KV("npc", table(nneg + 1, nc - 1, id => (string)call(gn, id))),
            KV("projectile", table(1, pc - 1, id => { object lt = call(gp, id); return (string)GetMember(lt, "Value"); })),
            KV("buffName", table(1, bc - 1, id => (string)call(gb, id))),
            KV("buffDescription", table(1, bc - 1, id => (string)call(gd, id))),
            KV("prefix", table(1, xc - 1, id => { object lt = pref.GetValue(id); return lt == null ? null : (string)GetMember(lt, "Value"); })),
        };
        End(fe, entries, entries.Count);
    }

    // ---------------------------------------------------------------- main run
    static void Run()
    {
        InitGame();
        Type main = T("Terraria.Main");
        Console.WriteLine("game initialised; Main.dedServ=" + SF(main, "dedServ") + " netMode=" + SF(main, "netMode"));

        // each dump is independent: a failure in one is recorded and the others still run
        var dumps = new List<KeyValuePair<string, Action>> {
            new KeyValuePair<string, Action>("tile_arrays", DumpTileArrays),
            new KeyValuePair<string, Action>("id_sets", DumpIdSets),
            new KeyValuePair<string, Action>("id_constants", DumpIdConstants),
            new KeyValuePair<string, Action>("entities", DumpEntities),
            new KeyValuePair<string, Action>("buffs", DumpBuffs),
            new KeyValuePair<string, Action>("recipes", DumpRecipes),
            new KeyValuePair<string, Action>("tile_object_data", DumpTileObjectData),
            new KeyValuePair<string, Action>("framing", DumpFraming),
            new KeyValuePair<string, Action>("lang", DumpLang),
        };
        foreach (var d in dumps)
        {
            try { d.Value(); }
            catch (Exception e)
            {
                Failures.Add("dump " + d.Key + " failed: " + e);
                Console.WriteLine("DUMP FAILED " + d.Key + ": " + e);
                if (Cur != null) { Files.Remove(Cur); Cur = null; }
            }
        }
        WriteManifest();
    }

    static void WriteManifest()
    {
        string exe = Path.Combine(GameDir, "TerrariaServer.exe");
        var sb = new StringBuilder("{\n");
        sb.Append("\"generator\":\"tools/refdump/RefDump.cs (reflection harness over the real TerrariaServer.exe)\",\n");
        sb.Append("\"gameAssembly\":{\"file\":\"work/refserver/TerrariaServer.exe\",\"sha256\":" + Q(Hex(SHA256.Create().ComputeHash(File.ReadAllBytes(exe)))) + ",\"version\":" + Q(G.GetName().Version.ToString()) + "},\n");
        sb.Append("\"initSequence\":[").Append(string.Join(",", InitLog.Select(x => Q(x)).ToArray())).Append("],\n");
        sb.Append("\"failures\":[").Append(string.Join(",", Failures.Select(x => Q(x)).ToArray())).Append("],\n");
        sb.Append("\"files\":[\n");
        for (int i = 0; i < Files.Count; i++)
        {
            FileEntry f = Files[i];
            sb.Append("{\"file\":").Append(Q(f.File)).Append(",\"rows\":").Append(f.Rows).Append(",\"rowsMeaning\":").Append(Q(f.RowsMeaning))
              .Append(",\"sha256\":").Append(Q(f.Sha)).Append(",\"gameCall\":").Append(Q(f.Call))
              .Append(",\"notes\":[").Append(string.Join(",", f.Notes.Select(x => Q(x)).ToArray())).Append("]")
              .Append(",\"skipped\":[").Append(string.Join(",", f.Skipped.OrderBy(x => x, StringComparer.Ordinal).Select(x => Q(x)).ToArray())).Append("]}");
            sb.Append(i + 1 < Files.Count ? ",\n" : "\n");
        }
        sb.Append("]\n}\n");
        File.WriteAllBytes(Path.Combine(OutDir, "MANIFEST.json"), new UTF8Encoding(false).GetBytes(sb.ToString()));
        Console.WriteLine("wrote MANIFEST.json");
    }
}
