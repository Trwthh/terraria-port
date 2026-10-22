// RefDumpWorldgenX "jitdump" mode: the machine code the real game's 32-bit JIT produces for given methods, read in-process.
//
//   RefDumpWorldgenX.exe jitdump <outdir> <listfile>
//
// <listfile>: one method per line, `Full.Type.Name::Method` (every overload of that name), `...::Method/N` (the overloads with
//             N parameters) or `...::Method#K` (the K-th overload in metadata order); `#` starts a comment. Nested types with `+`.
// Output:    <outdir>\jit_code.bin   "JITC" | u32 version = 1 | u32 count | per method: u32 name length | name (ASCII) |
//                                    u32 address | u32 nbytes | bytes        (the format of float_probes2\probes2_code.bin)
//            <outdir>\jit_code.json  per method: name, metadata token, IL size, address, nbytes, parameters (name, type) and
//                                    IL locals (index, type) in order
//
// Technique (ExtProbes.cs, float_model\disasm.py): RuntimeHelpers.PrepareMethod forces the JIT, the function pointer is followed
// through its precode stub to the native entry point, and the memory there is read with ReadProcessMemory (the method is shorter
// than nbytes: a disassembler stops at the last reachable ret). The game assembly is loaded from work\refserver and nothing of it
// is executed apart from what the JIT itself triggers; no game initialisation, no window, no socket.
// The addresses differ from run to run; the instructions do not.
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;

static partial class WG
{
    [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool ReadProcessMemory(IntPtr h, IntPtr addr, [Out] byte[] buf, IntPtr size, out IntPtr read);

    static IntPtr JitCodeAddress(MethodBase m)
    {
        RuntimeHelpers.PrepareMethod(m.MethodHandle);
        return EntryNoJit(m);
    }

    /// Where the method's entry point leads now, without compiling it (the precode jumps followed).
    static IntPtr EntryNoJit(MethodBase m)
    {
        bool unboxed;
        return EntryNoJit(m, out unboxed);
    }

    /// The same; `unboxed`: the way led through a value type's unboxing stub (`add ecx, 4; jmp [slot]`, the entry of
    /// a struct method as reflection gives it), whose slot holds the code of the method's other descriptor (the one
    /// eight bytes on, which the code header names).
    static IntPtr EntryNoJit(MethodBase m, out bool unboxed)
    {
        unboxed = false;
        IntPtr p = m.MethodHandle.GetFunctionPointer();
        for (int hop = 0; hop < 5; hop++)
        {
            byte b0 = Marshal.ReadByte(p);
            if (b0 == 0xE9) { int rel = Marshal.ReadInt32(p, 1); p = new IntPtr(p.ToInt32() + 5 + rel); continue; }
            if (b0 == 0xB8 && Marshal.ReadByte(p, 5) == 0x90 && Marshal.ReadByte(p, 6) == 0xE9)
            { int rel = Marshal.ReadInt32(p, 7); p = new IntPtr(p.ToInt32() + 11 + rel); continue; }
            if (!unboxed && b0 == 0x83 && Marshal.ReadByte(p, 1) == 0xC1 && Marshal.ReadByte(p, 2) == 0x04 && Marshal.ReadByte(p, 3) == 0xFF && Marshal.ReadByte(p, 4) == 0x25)
            { unboxed = true; p = new IntPtr(Marshal.ReadInt32(new IntPtr(Marshal.ReadInt32(p, 5)))); continue; }
            break;
        }
        return p;
    }

    /// <outdir>\jit_callees.json: every method the IL of the dumped methods names in a call / callvirt / newobj / ldftn, with
    /// the address its callers' machine code calls (the function pointer: the precode stub, or the code once jitted) and,
    /// when the stub is a jump, where it leads (not jitted here: nothing is forced). A machine `call` to one of these
    /// addresses is a call of that method; an IL call with no machine call to its callee was inlined (jit_calls.py).
    /// The IL is walked by opcode sizes, so only real call tokens are resolved.
    static void WriteCallees(string outdir, List<MethodBase> found)
    {
        var seen = new Dictionary<string, MethodBase>();
        var per = new StringBuilder("{\n");
        int pm = 0;
        foreach (MethodBase m in found)
        {
            var mine = new List<string>();
            byte[] il = m.GetMethodBody().GetILAsByteArray();
            Type[] targs = m.DeclaringType.IsGenericType ? m.DeclaringType.GetGenericArguments() : null;
            Type[] margs = m.IsGenericMethod ? m.GetGenericArguments() : null;
            for (int i = 0; i < il.Length;)
            {
                int op = il[i];
                int size;
                if (op == 0xFE) { int op2 = il[i + 1]; size = 2 + (op2 == 0x06 || op2 == 0x07 ? 4 : (op2 == 0x09 || op2 == 0x0A || op2 == 0x0B || op2 == 0x0C || op2 == 0x0D || op2 == 0x0E) ? 2 : (op2 == 0x12 || op2 == 0x19) ? 1 : (op2 == 0x15 || op2 == 0x16 || op2 == 0x1C) ? 4 : 0);
                    if (op2 == 0x06 || op2 == 0x07) Callee(m, BitConverter.ToInt32(il, i + 2), targs, margs, seen, mine, op2 == 0x06 ? "ldftn" : "ldvirtftn");
                    i += size; continue; }
                size = OperandSize(op, il, i);
                if (op == 0x28 || op == 0x6F || op == 0x73) Callee(m, BitConverter.ToInt32(il, i + 1), targs, margs, seen, mine, op == 0x28 ? "call" : op == 0x6F ? "callvirt" : "newobj");
                i += 1 + size;
            }
            string mname = m.DeclaringType.FullName + "::" + m.Name + "(" + string.Join(",", m.GetParameters().Select(q => JitTypeName(q.ParameterType)).ToArray()) + ")";
            per.Append((pm++ > 0 ? ",\n" : "") + Q(mname) + ":[" + string.Join(",", mine.Select(Q).ToArray()) + "]");
        }
        per.Append("\n}\n");
        File.WriteAllBytes(Path.Combine(outdir, "jit_method_callees.json"), new UTF8Encoding(false).GetBytes(per.ToString()));
        var js = new StringBuilder("[\n");
        int k = 0;
        foreach (var kv in seen)
        {
            MethodBase c = kv.Value;
            uint fp = 0, entry = 0;
            try
            {
                IntPtr p = c.MethodHandle.GetFunctionPointer();
                fp = (uint)p.ToInt32();
                for (int hop = 0; hop < 4; hop++)
                {
                    byte b0 = Marshal.ReadByte(p);
                    if (b0 == 0xE9) { int rel = Marshal.ReadInt32(p, 1); p = new IntPtr(p.ToInt32() + 5 + rel); continue; }
                    if (b0 == 0xB8 && Marshal.ReadByte(p, 5) == 0x90 && Marshal.ReadByte(p, 6) == 0xE9) { int rel = Marshal.ReadInt32(p, 7); p = new IntPtr(p.ToInt32() + 11 + rel); continue; }
                    break;
                }
                entry = (uint)p.ToInt32();
            }
            catch (Exception) { }
            int ilsize = -1;
            try { var b = c.GetMethodBody(); if (b != null) ilsize = b.GetILAsByteArray().Length; } catch (Exception) { }
            js.Append((k++ > 0 ? ",\n" : "") + "{\"name\":" + Q(kv.Key) + ",\"fp\":" + fp + ",\"entry\":" + entry + ",\"il_size\":" + ilsize + "}");
        }
        js.Append("\n]\n");
        File.WriteAllBytes(Path.Combine(outdir, "jit_callees.json"), new UTF8Encoding(false).GetBytes(js.ToString()));
        Console.WriteLine("jitdump: " + seen.Count + " callees named");
    }

    /// <outdir>\jit_cells.json: {cell address: the address in it} for every `call dword ptr [cell]` (FF 15 imm32) in the
    /// dumped code. The JIT calls a method that was not compiled yet through such a cell (its slot); the address in it
    /// is the method's function pointer (stub or code), which jit_callees.json names. Read in-process, now.
    static void WriteCells(string outdir)
    {
        byte[] bin = File.ReadAllBytes(Path.Combine(outdir, "jit_code.bin"));
        int n = BitConverter.ToInt32(bin, 8), o = 12;
        var cells = new SortedDictionary<uint, uint>();
        for (int k = 0; k < n; k++)
        {
            int ln = BitConverter.ToInt32(bin, o); o += 4 + ln;
            o += 4; int nb = BitConverter.ToInt32(bin, o); o += 4;
            for (int i = o; i + 6 <= o + nb; i++)
                if (bin[i] == 0xFF && bin[i + 1] == 0x15)
                {
                    uint cell = BitConverter.ToUInt32(bin, i + 2);
                    if (cells.ContainsKey(cell)) continue;
                    // ReadProcessMemory, not Marshal: a byte pattern that is no instruction may name unmapped memory
                    byte[] four = new byte[4]; IntPtr rd;
                    if (ReadProcessMemory(GetCurrentProcess(), new IntPtr((int)cell), four, new IntPtr(4), out rd) && rd.ToInt32() == 4)
                        cells[cell] = BitConverter.ToUInt32(four, 0);
                }
            o += nb;
        }
        var sb = new StringBuilder("{");
        int c = 0;
        foreach (var kv in cells) sb.Append((c++ > 0 ? "," : "") + "\"" + kv.Key + "\":" + kv.Value);
        sb.Append("}\n");
        File.WriteAllBytes(Path.Combine(outdir, "jit_cells.json"), new UTF8Encoding(false).GetBytes(sb.ToString()));
        Console.WriteLine("jitdump: " + cells.Count + " call cells read");
    }

    /// The non-generic methods with a body that the IL of `m` calls (call / newobj; callvirt of a non-virtual).
    public static List<MethodBase> IlCallees(MethodBase m)
    {
        var r = new List<MethodBase>();
        byte[] il;
        try { var b = m.GetMethodBody(); if (b == null) return r; il = b.GetILAsByteArray(); } catch (Exception) { return r; }
        Type[] targs = m.DeclaringType.IsGenericType ? m.DeclaringType.GetGenericArguments() : null;
        Type[] margs = m.IsGenericMethod ? m.GetGenericArguments() : null;
        for (int i = 0; i < il.Length;)
        {
            int op = il[i];
            if (op == 0xFE) { int op2 = il[i + 1]; i += 2 + (op2 == 0x06 || op2 == 0x07 || op2 == 0x15 || op2 == 0x16 || op2 == 0x1C ? 4 : (op2 >= 0x09 && op2 <= 0x0E) ? 2 : (op2 == 0x12 || op2 == 0x19) ? 1 : 0); continue; }
            int size = OperandSize(op, il, i);
            if (op == 0x28 || op == 0x6F || op == 0x73)
            {
                try
                {
                    MethodBase c = m.Module.ResolveMethod(BitConverter.ToInt32(il, i + 1), targs, margs);
                    // the game's own methods only: compiling library methods ahead (native-backed XNA graphics) crashed the
                    // process; mscorlib is precompiled (direct calls) anyway
                    if (c != null && c.Module == m.Module && !c.ContainsGenericParameters && !(op == 0x6F && c.IsVirtual && !c.IsFinal) && c.GetMethodBody() != null
                        && !(c.DeclaringType.FullName ?? "").StartsWith("Steamworks", StringComparison.Ordinal)
                        && !(c.DeclaringType.FullName ?? "").StartsWith("Terraria.Social", StringComparison.Ordinal))
                        r.Add(c);
                }
                catch (Exception) { }
            }
            i += 1 + size;
        }
        return r;
    }

    static void Callee(MethodBase from, int token, Type[] targs, Type[] margs, Dictionary<string, MethodBase> seen, List<string> mine, string how)
    {
        try
        {
            MethodBase c = from.Module.ResolveMethod(token, targs, margs);
            if (c == null || c.ContainsGenericParameters) return;
            string name = c.DeclaringType.FullName + "::" + c.Name + "(" + string.Join(",", c.GetParameters().Select(q => JitTypeName(q.ParameterType)).ToArray()) + ")";
            if (!seen.ContainsKey(name)) seen[name] = c;
            mine.Add(how + (c.IsVirtual && !c.IsFinal && how == "callvirt" ? "-virtual" : "") + " " + name);
        }
        catch (Exception) { }
    }

    /// Operand size of a one-byte IL opcode (ECMA-335 III); `switch` reads its count.
    static int OperandSize(int op, byte[] il, int i)
    {
        switch (op)
        {
            case 0x0E: case 0x0F: case 0x10: case 0x11: case 0x12: case 0x13: case 0x1F: case 0xDE:
            case 0x2B: case 0x2C: case 0x2D: case 0x2E: case 0x2F: case 0x30: case 0x31: case 0x32: case 0x33: case 0x34: case 0x35:
            case 0x36: case 0x37:
                return 1;
            case 0x20: case 0x22: case 0x28: case 0x29: case 0x27: case 0x38: case 0x39: case 0x3A: case 0x3B: case 0x3C: case 0x3D:
            case 0x3E: case 0x3F: case 0x40: case 0x41: case 0x42: case 0x43: case 0x44: case 0x6F: case 0x70: case 0x71: case 0x72:
            case 0x73: case 0x74: case 0x75: case 0x79: case 0x7B: case 0x7C: case 0x7D: case 0x7E: case 0x7F: case 0x80: case 0x81:
            case 0x8C: case 0x8D: case 0x8F: case 0xA3: case 0xA4: case 0xA5: case 0xC2: case 0xC6: case 0xD0: case 0xDD:
                return 4;
            case 0x21: case 0x23:
                return 8;
            case 0x45:
                return 4 + 4 * BitConverter.ToInt32(il, i + 1);
            default:
                return 0;
        }
    }

    static string JitTypeName(Type t)
    {
        if (t.IsByRef) return JitTypeName(t.GetElementType()) + "&";
        if (t.IsArray) return JitTypeName(t.GetElementType()) + "[" + new string(',', t.GetArrayRank() - 1) + "]";
        return t.FullName ?? t.Name;
    }

    static int MainJitDump(string[] args)
    {
        if (args.Length < 3) { Console.Error.WriteLine("usage: RefDumpWorldgenX.exe jitdump <outdir> <listfile>"); return 2; }
        string outdir = Dir(Path.GetFullPath(args[1]));
        Root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(Root, "work", "refserver");
        Watchdog(1800);
        AppDomain.CurrentDomain.AssemblyResolve += OnResolve;
        Environment.CurrentDirectory = GameDir;
        G = Assembly.LoadFrom(Path.Combine(GameDir, "TerrariaServer.exe"));
        const BindingFlags all = BindingFlags.Instance | BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.DeclaredOnly;
        var found = new List<MethodBase>();
        int missing = 0;
        foreach (string raw in File.ReadAllLines(args[2]))
        {
            string line = raw.Split('#')[0].Trim();
            int hash = raw.IndexOf('#');
            // `#K` selects an overload: only a `#` directly after the method name counts (a comment needs a space before it)
            string sel = null;
            if (hash > 0 && raw[hash - 1] != ' ' && raw[hash - 1] != '\t') { line = raw.Substring(0, hash).Trim(); sel = raw.Substring(hash + 1).Split(' ')[0].Trim(); }
            if (line.Length == 0) continue;
            int sep = line.IndexOf("::", StringComparison.Ordinal);
            if (sep < 0) { Console.Error.WriteLine("bad line: " + raw); missing++; continue; }
            string typeName = line.Substring(0, sep), method = line.Substring(sep + 2);
            int nparams = -1;
            int slash = method.IndexOf('/');
            if (slash >= 0) { nparams = int.Parse(method.Substring(slash + 1), Inv); method = method.Substring(0, slash); }
            // `*::*`: every non-generic method body of every non-generic type of the game assembly
            if (typeName == "*")
            {
                Type[] types;
                try { types = G.GetTypes(); } catch (ReflectionTypeLoadException e) { types = e.Types.Where(x => x != null).ToArray(); }
                // never the Steam / social layer (the project does not touch steam_api.dll or what wraps it)
                foreach (Type tt in types.Where(x => !x.ContainsGenericParameters && !(x.FullName ?? "").StartsWith("Steamworks", StringComparison.Ordinal) && !(x.FullName ?? "").StartsWith("Terraria.Social", StringComparison.Ordinal)).OrderBy(x => x.FullName, StringComparer.Ordinal))
                {
                    var ms = new List<MethodBase>();
                    try { ms.AddRange(tt.GetMethods(all)); ms.AddRange(tt.GetConstructors(all)); } catch (Exception) { continue; }
                    found.AddRange(ms.Where(m => { try { return m.GetMethodBody() != null && !m.ContainsGenericParameters; } catch (Exception) { return false; } }).OrderBy(m => m.MetadataToken));
                }
                continue;
            }
            Type t = G.GetType(typeName, false);
            if (t == null) { Console.Error.WriteLine("type not found: " + typeName); missing++; continue; }
            var cands = new List<MethodBase>();
            if (method == ".ctor" || method == ".cctor") cands.AddRange(t.GetConstructors(all).Where(c => c.Name == method));
            else cands.AddRange(t.GetMethods(all).Where(m => m.Name == method));
            cands = cands.Where(m => m.GetMethodBody() != null && !m.ContainsGenericParameters).OrderBy(m => m.MetadataToken).ToList();
            if (nparams >= 0) cands = cands.Where(m => m.GetParameters().Length == nparams).ToList();
            if (sel != null) { int k = int.Parse(sel, Inv); cands = k < cands.Count ? new List<MethodBase> { cands[k] } : new List<MethodBase>(); }
            if (cands.Count == 0) { Console.Error.WriteLine("method not found: " + raw.Trim()); missing++; continue; }
            found.AddRange(cands);
        }
        // callees before callers (depth first over the IL's call / newobj tokens): a method whose callee is not compiled
        // yet calls it through an indirection cell whose stub has no public name; compiled first, the call is direct
        // to the callee's function pointer, which jit_callees.json names
        var done = new HashSet<MethodBase>();
        foreach (MethodBase root in found)
        {
            var stack = new Stack<KeyValuePair<MethodBase, bool>>();
            stack.Push(new KeyValuePair<MethodBase, bool>(root, false));
            while (stack.Count > 0)
            {
                var top = stack.Pop();
                MethodBase m = top.Key;
                if (top.Value) { try { JitCodeAddress(m); } catch (Exception) { } continue; }
                if (!done.Add(m)) continue;
                stack.Push(new KeyValuePair<MethodBase, bool>(m, true));
                foreach (MethodBase c in IlCallees(m))
                    if (!done.Contains(c)) stack.Push(new KeyValuePair<MethodBase, bool>(c, false));
            }
        }
        Console.WriteLine("jitdump: " + done.Count + " methods prepared callees first");
        DumpJitted(outdir, found);
        Console.WriteLine("jitdump: " + missing + " list entries not found");
        Console.Out.Flush();
        Environment.Exit(missing == 0 ? 0 : 1);
        return 0;
    }

    // ------------------------------------------------------------------------------------------------------------
    // DumpRan: the code of every method of the translated assemblies that RAN in this process, read without compiling
    // anything. Used by the reference-run wrappers (RefJit*.cs, CollisionJit.cs, LightJit.cs) after the harness's own
    // sequence has finished.
    //
    // Which methods ran: the CLR (.NET Framework 4.8, x86) puts a code header right in front of every jitted method:
    // [entry-16] debug info, [entry-12] EH info, [entry-8] GC info, [entry-4] the MethodDesc (measured 2026-10-14 with a
    // probe program: the dword at entry-4 equals MethodHandle.Value for a jitted method; for one that never ran the
    // entry point is its precode, `E8 ...`, with no such header). The method's code size is the first value of its GC
    // info (x86 GC encoding: unsigned, 7 bits per byte, most significant group first, high bit = more), so the code is
    // read exactly, not up to a guessed length.
    static uint ReadU32(uint addr, out bool ok)
    {
        byte[] four = new byte[4]; IntPtr rd;
        ok = ReadProcessMemory(GetCurrentProcess(), new IntPtr((int)addr), four, new IntPtr(4), out rd) && rd.ToInt32() == 4;
        return ok ? BitConverter.ToUInt32(four, 0) : 0;
    }

    static byte[] ReadBytes(uint addr, int n)
    {
        if (n <= 0) return new byte[0];
        byte[] buf = new byte[n]; IntPtr rd;
        if (ReadProcessMemory(GetCurrentProcess(), new IntPtr((int)addr), buf, new IntPtr(n), out rd) && rd.ToInt32() == n) return buf;
        return null;
    }

    /// The jitted code of a method: (entry, size); entry 0 when the method has no jitted code in this process.
    static KeyValuePair<uint, int> JittedCode(uint entry, uint md)
    {
        bool ok;
        if (entry < 16) return new KeyValuePair<uint, int>(0, 0);
        uint hdr = ReadU32(entry - 4, out ok);
        if (!ok || hdr != md) return new KeyValuePair<uint, int>(0, 0);
        uint gc = ReadU32(entry - 8, out ok);
        if (!ok) return new KeyValuePair<uint, int>(0, 0);
        byte[] g = ReadBytes(gc, 5);
        if (g == null) return new KeyValuePair<uint, int>(0, 0);
        uint v = (uint)(g[0] & 0x7F); int k = 0;
        while ((g[k] & 0x80) != 0 && k < 4) { k++; v = (v << 7) | (uint)(g[k] & 0x7F); }
        return new KeyValuePair<uint, int>(entry, (int)v);
    }

    public static string JitName(MethodBase m)
    {
        string name = m.Name;
        // a closed instantiation of a generic method of the game: `Name<Arg1,Arg2>` (as for generic types)
        if (m.IsGenericMethod && !m.ContainsGenericParameters)
            name += "<" + string.Join(",", m.GetGenericArguments().Select(a => JitOwnerName(a)).ToArray()) + ">";
        return JitOwnerName(m.DeclaringType) + "::" + name + "(" + string.Join(",", m.GetParameters().Select(q => JitTypeName(q.ParameterType)).ToArray()) + ")";
    }

    /// The closed instantiations of the generic methods of assembly `a` that its own code calls (its MethodSpec table).
    static List<MethodBase> ClosedGenericMethods(Assembly a)
    {
        var r = new List<MethodBase>();
        var seen = new HashSet<MethodBase>();
        foreach (Module mod in a.GetModules())
        {
            int misses = 0;
            for (int row = 1; misses < 64; row++)
            {
                MethodBase m;
                try { m = mod.ResolveMethod(0x2B000000 | row); misses = 0; }
                catch (ArgumentOutOfRangeException) { misses++; continue; }
                catch (Exception) { misses = 0; continue; }
                if (m == null || !m.IsGenericMethod || m.ContainsGenericParameters || m.DeclaringType == null || m.DeclaringType.ContainsGenericParameters) continue;
                if (m.DeclaringType.Assembly != a && !(m.DeclaringType.IsGenericType && m.DeclaringType.GetGenericTypeDefinition().Assembly == a)) continue;
                if (seen.Add(m)) r.Add(m);
            }
        }
        return r.OrderBy(x => JitName(x), StringComparer.Ordinal).ToList();
    }

    /// The declaring type's name in a dump: its full name; a closed instantiation of a generic type of the game is
    /// `Definition`n<Arg1,Arg2>` (the arguments' full names without assembly), so that the coverage report can match
    /// it to the definition while the JIT facts (keyed by exact names) never apply one instantiation's code to another.
    static string JitOwnerName(Type t)
    {
        if (t.IsGenericType && !t.ContainsGenericParameters)
            return t.GetGenericTypeDefinition().FullName + "<" + string.Join(",", t.GetGenericArguments().Select(a => JitOwnerName(a)).ToArray()) + ">";
        return t.FullName ?? t.Name;
    }

    /// The closed instantiations of the generic types of assembly `a` that its own code names (its TypeSpec table):
    /// their methods get code of their own (value-type arguments) or share one (reference types), and run like any
    /// other method of the game.
    static List<Type> ClosedGenericTypes(Assembly a)
    {
        var r = new List<Type>();
        var seen = new HashSet<Type>();
        foreach (Module mod in a.GetModules())
        {
            int misses = 0;
            for (int row = 1; misses < 64; row++)
            {
                Type t;
                try { t = mod.ResolveType(0x1B000000 | row); misses = 0; }
                catch (ArgumentOutOfRangeException) { misses++; continue; }
                catch (Exception) { misses = 0; continue; }
                for (Type u = t; u != null; u = u.IsArray || u.IsByRef || u.IsPointer ? u.GetElementType() : null)
                {
                    if (!u.IsGenericType || u.ContainsGenericParameters || u.GetGenericTypeDefinition().Assembly != a) continue;
                    if (seen.Add(u)) r.Add(u);
                }
            }
        }
        return r.OrderBy(x => JitOwnerName(x), StringComparer.Ordinal).ToList();
    }

    /// Every method of the assemblies of this process whose simple name is in `names` (TerrariaServer, ReLogic,
    /// Microsoft.Xna.Framework): non-generic types and methods with a body. Steam / social types are never touched.
    public static List<MethodBase> AllMethods(params string[] names)
    {
        const BindingFlags all = BindingFlags.Instance | BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.DeclaredOnly;
        var r = new List<MethodBase>();
        foreach (string an in names)
        {
            Assembly a = AppDomain.CurrentDomain.GetAssemblies().FirstOrDefault(x => x.GetName().Name == an);
            if (a == null) { Console.Error.WriteLine("dumpran: assembly not loaded: " + an); continue; }
            Type[] types;
            try { types = a.GetTypes(); } catch (ReflectionTypeLoadException e) { types = e.Types.Where(x => x != null).ToArray(); }
            foreach (Type t in types.Where(x => !x.ContainsGenericParameters && !(x.FullName ?? "").StartsWith("Steamworks", StringComparison.Ordinal) && !(x.FullName ?? "").StartsWith("Terraria.Social", StringComparison.Ordinal)).OrderBy(x => x.FullName, StringComparer.Ordinal))
            {
                var ms = new List<MethodBase>();
                try { ms.AddRange(t.GetMethods(all)); ms.AddRange(t.GetConstructors(all)); } catch (Exception) { continue; }
                r.AddRange(ms.Where(m => { try { return !m.ContainsGenericParameters && m.GetMethodBody() != null; } catch (Exception) { return false; } }).OrderBy(m => m.MetadataToken));
            }
            foreach (Type t in ClosedGenericTypes(a))
            {
                var ms = new List<MethodBase>();
                try { ms.AddRange(t.GetMethods(all)); ms.AddRange(t.GetConstructors(all)); } catch (Exception) { continue; }
                r.AddRange(ms.Where(m => { try { return !m.ContainsGenericParameters && m.GetMethodBody() != null; } catch (Exception) { return false; } }).OrderBy(m => m.MetadataToken));
            }
            r.AddRange(ClosedGenericMethods(a).Where(m => { try { return m.GetMethodBody() != null; } catch (Exception) { return false; } }));
        }
        return r;
    }

    /// The reference-run wrappers' dump: every method of the game, ReLogic and XNA that ran in this process.
    public static void DumpRanGame(string outdir)
    {
        DumpRan(outdir, AllMethods("TerrariaServer", "ReLogic", "Microsoft.Xna.Framework"));
    }

    /// Writes jit_code.bin / jit_code.json (every method of `cands` that has jitted code now: "compiled_before" true,
    /// "exact_size" true), jit_callees.json, jit_method_callees.json, jit_cells.json and jit_targets.json. Compiles nothing.
    public static void DumpRan(string outdir, List<MethodBase> cands)
    {
        Directory.CreateDirectory(outdir);
        var ran = new List<MethodBase>();
        var code = new Dictionary<MethodBase, KeyValuePair<uint, int>>();
        var mdName = new Dictionary<uint, string>();
        var fpName = new Dictionary<uint, string>();
        foreach (MethodBase m in cands)
        {
            try
            {
                uint md = (uint)m.MethodHandle.Value.ToInt32();
                string nm = JitName(m);
                mdName[md] = nm;
                bool unboxed;
                uint e = (uint)EntryNoJit(m, out unboxed).ToInt32();
                fpName[(uint)m.MethodHandle.GetFunctionPointer().ToInt32()] = nm;
                var c = JittedCode(e, md);
                // a struct method reached through its unboxing stub: the code belongs to the descriptor 8 bytes on
                if (c.Key == 0 && unboxed) { c = JittedCode(e, md + 8); if (c.Key != 0) mdName[md + 8] = nm; }
                if (c.Key == 0) continue;
                fpName[e] = nm;
                ran.Add(m); code[m] = c;
            }
            catch (Exception) { }
        }
        AddSharedGenericCode(ran, code, mdName, fpName);
        Console.WriteLine("dumpran: " + ran.Count + " of " + cands.Count + " methods ran");
        var js = new StringBuilder("[\n");
        using (var bw = new BinaryWriter(new FileStream(Path.Combine(outdir, "jit_code.bin"), FileMode.Create)))
        {
            bw.Write(Encoding.ASCII.GetBytes("JITC")); bw.Write(1u); bw.Write((uint)ran.Count);
            for (int i = 0; i < ran.Count; i++)
            {
                MethodBase m = ran[i];
                MethodBody body = m.GetMethodBody();
                byte[] ilb = body.GetILAsByteArray();
                var c = code[m];
                byte[] bytes = ReadBytes(c.Key, c.Value) ?? new byte[0];
                string name = JitName(m);
                byte[] nm = Encoding.ASCII.GetBytes(name);
                bw.Write((uint)nm.Length); bw.Write(nm); bw.Write(c.Key); bw.Write((uint)bytes.Length); bw.Write(bytes);
                js.Append("{\"name\":" + Q(name) + ",\"compiled_before\":true,\"exact_size\":true,\"token\":" + m.MetadataToken + ",\"static\":" + (m.IsStatic ? "true" : "false") + ",\"il_size\":" + ilb.Length + ",\"address\":" + c.Key + ",\"nbytes\":" + bytes.Length + ",\"init_locals\":" + (body.InitLocals ? "true" : "false"));
                js.Append(",\"params\":[" + string.Join(",", m.GetParameters().Select(q => "{\"name\":" + Q(q.Name) + ",\"type\":" + Q(JitTypeName(q.ParameterType)) + "}").ToArray()) + "]");
                js.Append(",\"locals\":[" + string.Join(",", body.LocalVariables.OrderBy(l => l.LocalIndex).Select(l => "{\"index\":" + l.LocalIndex + ",\"type\":" + Q(JitTypeName(l.LocalType)) + ",\"pinned\":" + (l.IsPinned ? "true" : "false") + "}").ToArray()) + "]");
                js.Append(",\"il\":\"" + Hex(ilb) + "\"}" + (i + 1 < ran.Count ? ",\n" : "\n"));
            }
        }
        js.Append("]\n");
        File.WriteAllBytes(Path.Combine(outdir, "jit_code.json"), new UTF8Encoding(false).GetBytes(js.ToString()));
        // jit_method_callees.json also for the game methods the ran methods' IL calls (two levels) that have no code of
        // their own because they were always inlined (FastRandom.Next(int, int) in ApplyWallLight): the generators need
        // their IL callees to see that an inlined body still executes a call (jit_calls.call_answers). IL only.
        var withCallees = new List<MethodBase>(ran);
        var seenM = new HashSet<MethodBase>(ran);
        var level = new List<MethodBase>(ran);
        for (int lv = 0; lv < 2; lv++)
        {
            var next = new List<MethodBase>();
            foreach (MethodBase m in level)
                foreach (MethodBase c in IlCallees(m))
                    if (seenM.Add(c)) next.Add(c);
            withCallees.AddRange(next);
            level = next;
        }
        WriteCallees(outdir, withCallees);
        WriteCells(outdir);
        WriteTargets(outdir, mdName, fpName);
        WriteDebugInfo(outdir, ran, code);
        Console.WriteLine("dumpran: " + ran.Count + " methods written to " + outdir);
    }

    /// The shared code of the game's generic methods instantiated over reference types (`CreateSubcategoryButton<T>`
    /// with T a class: one body for every such T, its MethodDesc the instantiation over System.__Canon). Reflection's
    /// closed instantiations name the exact MethodDescs, which the callers pass as the hidden argument and whose own
    /// precode never runs, so no candidate leads to that code: it is found as a call target of the code that ran (the
    /// jumps of the target's precode followed, the MethodDesc in the code header) and named through the runtime's own
    /// MethodDesc -> MethodBase lookup (`RuntimeType.GetMethodBase(RuntimeType, RuntimeMethodHandleInternal)`). Only a generic method
    /// or a method of a generic type of the dumped assemblies is taken; repeated until no new code appears.
    static void AddSharedGenericCode(List<MethodBase> ran, Dictionary<MethodBase, KeyValuePair<uint, int>> code, Dictionary<uint, string> mdName, Dictionary<uint, string> fpName)
    {
        Type rmhi = typeof(RuntimeMethodHandle).Assembly.GetType("System.RuntimeMethodHandleInternal");
        Type rt = typeof(Type).Assembly.GetType("System.RuntimeType");
        // .NET Framework 4.8: RuntimeType.GetMethodBase(RuntimeType reflectedType, RuntimeMethodHandleInternal) (null: the declaring type)
        MethodInfo gmb = rmhi == null || rt == null ? null : rt.GetMethod("GetMethodBase", BindingFlags.NonPublic | BindingFlags.Static, null, new[] { rt, rmhi }, null);
        if (gmb == null) { Console.Error.WriteLine("dumpran: no RuntimeType.GetMethodBase(RuntimeType, RuntimeMethodHandleInternal): shared generic code not dumped"); return; }
        var asms = new HashSet<Assembly>(ran.Select(m => m.DeclaringType == null ? null : m.DeclaringType.Assembly).Where(a => a != null));
        // the MethodTables of the dumped assemblies' non-generic types: a call target's code header names a MethodDesc
        // only if its chunk (x86: 12 bytes before the first MethodDesc, `8 * chunk index` before this one; its first word
        // the MethodTable as a pointer relative to itself) belongs to one of them. Anything else (helpers, stubs: the
        // word before them is not a MethodDesc) is never handed to the runtime.
        var mts = new HashSet<uint>();
        foreach (Assembly a in asms)
        {
            Type[] ts;
            try { ts = a.GetTypes(); } catch (ReflectionTypeLoadException e) { ts = e.Types.Where(x => x != null).ToArray(); }
            foreach (Type t in ts) { try { if (!t.ContainsGenericParameters) mts.Add((uint)t.TypeHandle.Value.ToInt32()); } catch (Exception) { } }
        }
        var scanned = new HashSet<MethodBase>();
        int added = 0;
        for (bool more = true; more; )
        {
            more = false;
            foreach (MethodBase m in ran.ToList())
            {
                if (!scanned.Add(m)) continue;
                var c = code[m];
                byte[] b = ReadBytes(c.Key, c.Value);
                if (b == null) continue;
                for (int i = 0; i + 5 <= b.Length; i++)
                {
                    uint t;
                    bool ok;
                    if (b[i] == 0xE8) t = (uint)(c.Key + i + 5 + BitConverter.ToInt32(b, i + 1));
                    else if (b[i] == 0xFF && i + 6 <= b.Length && b[i + 1] == 0x15) { t = ReadU32(BitConverter.ToUInt32(b, i + 2), out ok); if (!ok) continue; }
                    else continue;
                    uint p = t;
                    for (int hop = 0; hop < 5; hop++)
                    {
                        byte[] h = ReadBytes(p, 12);
                        if (h == null) break;
                        if (h[0] == 0xE9) { p = (uint)(p + 5 + BitConverter.ToInt32(h, 1)); continue; }
                        if (h[0] == 0xB8 && h[5] == 0x90 && h[6] == 0xE9) { p = (uint)(p + 11 + BitConverter.ToInt32(h, 7)); continue; }
                        break;
                    }
                    uint md = ReadU32(p - 4, out ok);
                    if (!ok || md == 0 || mdName.ContainsKey(md)) continue;
                    var jc = JittedCode(p, md);
                    if (jc.Key == 0) continue;
                    byte[] mdb = ReadBytes(md, 4);
                    if (mdb == null) { mdName[md] = null; continue; }
                    uint chunk = md - 8u * mdb[2] - 12u;
                    uint rel = ReadU32(chunk, out ok);
                    if (!ok || !mts.Contains(chunk + rel)) { mdName[md] = null; continue; }
                    MethodBase g;
                    try
                    {
                        object hd = Activator.CreateInstance(rmhi, BindingFlags.NonPublic | BindingFlags.Public | BindingFlags.Instance, null, new object[] { new IntPtr((int)md) }, null);
                        g = (MethodBase)gmb.Invoke(null, new[] { null, hd });
                    }
                    catch (Exception) { mdName[md] = null; continue; }
                    if (g == null || g.DeclaringType == null || !asms.Contains(g.DeclaringType.Assembly) || !(g.IsGenericMethod || g.DeclaringType.IsGenericType))
                    { mdName[md] = null; continue; }
                    try { if (g.GetMethodBody() == null) { mdName[md] = null; continue; } } catch (Exception) { mdName[md] = null; continue; }
                    string nm = JitName(g);
                    mdName[md] = nm;
                    fpName[p] = nm;
                    ran.Add(g); code[g] = jc;
                    added++;
                    more = true;
                }
            }
        }
        // the lookups that named nothing are not names
        foreach (uint k in mdName.Where(kv => kv.Value == null).Select(kv => kv.Key).ToList()) mdName.Remove(k);
        Console.WriteLine("dumpran: shared generic code: " + added + " methods");
    }

    /// <outdir>\jit_debug.bin: "JDBG" | u32 version 1 | u32 count | per method: u32 name length | name | u32 debug info
    /// pointer (the code header's [entry-16]; 0: none) | u32 n | n bytes read there (the CLR's compressed bounds and
    /// variable locations, debuginfostore.cpp; the exact length from the blob's own header: two sizes, nibble-encoded).
    /// The variable locations say which IL local lives in an x87 register and which in a frame slot (float_alloc.py).
    static void WriteDebugInfo(string outdir, List<MethodBase> ran, Dictionary<MethodBase, KeyValuePair<uint, int>> code)
    {
        int with = 0;
        using (var bw = new BinaryWriter(new FileStream(Path.Combine(outdir, "jit_debug.bin"), FileMode.Create)))
        {
            bw.Write(Encoding.ASCII.GetBytes("JDBG")); bw.Write(1u); bw.Write((uint)ran.Count);
            foreach (MethodBase m in ran)
            {
                byte[] nm = Encoding.ASCII.GetBytes(JitName(m));
                bool ok;
                uint p = ReadU32(code[m].Key - 16, out ok);
                byte[] blob = null;
                if (ok && p != 0)
                {
                    // the header (two nibble-encoded sizes: bounds, vars) gives the exact length
                    byte[] head = null;
                    foreach (int n in new[] { 16, 8, 4 }) { head = ReadBytes(p, n); if (head != null) break; }
                    if (head != null)
                    {
                        int ni = 0;
                        Func<uint> u32 = () =>
                        {
                            uint v = 0; int nib;
                            do { int b = head[ni / 2]; nib = (ni % 2 == 0) ? (b & 0xF) : (b >> 4); ni++; v = (v << 3) | (uint)(nib & 7); } while ((nib & 8) != 0);
                            return v;
                        };
                        try
                        {
                            uint cb1 = u32(), cb2 = u32();
                            int total = (ni + 1) / 2 + (int)cb1 + (int)cb2;
                            if (total > 0 && total < (1 << 24)) blob = ReadBytes(p, total);
                        }
                        catch (IndexOutOfRangeException) { blob = null; }
                    }
                }
                if (blob == null) blob = new byte[0]; else with++;
                bw.Write((uint)nm.Length); bw.Write(nm); bw.Write(ok ? p : 0u); bw.Write((uint)blob.Length); bw.Write(blob);
            }
        }
        Console.WriteLine("dumpran: debug info for " + with + " of " + ran.Count + " methods");
    }

    /// <outdir>\jit_targets.json: {call target: method name} for the targets of every `call rel32` and every call cell
    /// of the dumped code that lead to a known method. A caller compiled before its callee calls the callee's PRECODE;
    /// once the callee is jitted that precode jumps to its code (`E9 rel32`, or `mov eax, MethodDesc; ...; jmp`), and the
    /// function pointer reported now is the code itself, so jit_callees.json alone does not name such a call. Resolved
    /// here: the target, the jumps of its precode followed, then the MethodDesc (precode immediate or code header).
    /// Targets that lead nowhere known (JIT helpers, stubs of generic instantiations) are not written.
    static void WriteTargets(string outdir, Dictionary<uint, string> mdName, Dictionary<uint, string> fpName)
    {
        // the callees named by jit_callees.json (mscorlib ...) too: one object per line, written by WriteCallees
        try
        {
            foreach (string line in File.ReadAllLines(Path.Combine(outdir, "jit_callees.json")))
            {
                int a = line.IndexOf("{\"name\":\"", StringComparison.Ordinal);
                int f = line.IndexOf("\",\"fp\":", StringComparison.Ordinal), en = line.IndexOf(",\"entry\":", StringComparison.Ordinal), il = line.IndexOf(",\"il_size\":", StringComparison.Ordinal);
                if (a < 0 || f < 0 || en < 0 || il < 0) continue;
                string nm = line.Substring(a + 9, f - a - 9).Replace("\\\"", "\"").Replace("\\\\", "\\");
                uint fp = uint.Parse(line.Substring(f + 7, en - f - 7), Inv), ent = uint.Parse(line.Substring(en + 9, il - en - 9), Inv);
                if (fp != 0 && !fpName.ContainsKey(fp)) fpName[fp] = nm;
                if (ent != 0 && !fpName.ContainsKey(ent)) fpName[ent] = nm;
            }
        }
        catch (Exception e) { Console.Error.WriteLine("dumpran: jit_callees.json not read: " + e.Message); }
        byte[] bin = File.ReadAllBytes(Path.Combine(outdir, "jit_code.bin"));
        int n = BitConverter.ToInt32(bin, 8), o = 12;
        var targets = new SortedDictionary<uint, string>();
        var tried = new HashSet<uint>();
        int unresolved = 0;
        for (int k = 0; k < n; k++)
        {
            int ln = BitConverter.ToInt32(bin, o); o += 4 + ln;
            uint addr = BitConverter.ToUInt32(bin, o); o += 4; int nb = BitConverter.ToInt32(bin, o); o += 4;
            for (int i = o; i + 5 <= o + nb; i++)
            {
                uint t;
                if (bin[i] == 0xE8) t = (uint)(addr + (i - o) + 5 + BitConverter.ToInt32(bin, i + 1));
                else if (bin[i] == 0xFF && i + 6 <= o + nb && bin[i + 1] == 0x15)
                {
                    bool ok; uint cell = BitConverter.ToUInt32(bin, i + 2);
                    t = ReadU32(cell, out ok); if (!ok) continue;
                    // the cell of a call to a method that was not compiled when the caller was: the MethodDesc's own slot,
                    // right behind the 8-byte MethodDesc (measured with a probe program: slot = MethodDesc + 8). While the
                    // callee never ran it holds a temporary entry point (`mov al, n; jmp ...`) that nothing else names.
                    string byCell;
                    if (!targets.ContainsKey(t) && mdName.TryGetValue(cell - 8, out byCell) && ResolveTarget(t, mdName, fpName) == null) { targets[t] = byCell; tried.Add(t); continue; }
                }
                else continue;
                if (!tried.Add(t)) continue;
                string nm = ResolveTarget(t, mdName, fpName);
                if (nm != null) targets[t] = nm; else unresolved++;
            }
            o += nb;
        }
        var sb = new StringBuilder("{\n");
        int c = 0;
        foreach (var kv in targets) sb.Append((c++ > 0 ? ",\n" : "") + "\"" + kv.Key + "\":" + Q(kv.Value));
        sb.Append("\n}\n");
        File.WriteAllBytes(Path.Combine(outdir, "jit_targets.json"), new UTF8Encoding(false).GetBytes(sb.ToString()));
        Console.WriteLine("dumpran: " + targets.Count + " call targets named, " + unresolved + " byte patterns / helpers not");
    }

    static string ResolveTarget(uint t, Dictionary<uint, string> mdName, Dictionary<uint, string> fpName)
    {
        uint p = t;
        for (int hop = 0; hop < 5; hop++)
        {
            string nm;
            if (fpName.TryGetValue(p, out nm)) return nm;
            bool ok;
            uint hdr = ReadU32(p - 4, out ok);
            if (ok && mdName.TryGetValue(hdr, out nm)) return nm;
            byte[] b = ReadBytes(p, 12);
            if (b == null) return null;
            if (b[0] == 0xE9) { p = (uint)(p + 5 + BitConverter.ToInt32(b, 1)); continue; }
            if (b[0] == 0xB8)
            {
                uint md = BitConverter.ToUInt32(b, 1);
                if (mdName.TryGetValue(md, out nm)) return nm;
                if (b[5] == 0x90 && b[6] == 0xE9) { p = (uint)(p + 11 + BitConverter.ToInt32(b, 7)); continue; }
            }
            return null;
        }
        return null;
    }

    /// Writes jit_code.bin / jit_code.json / jit_callees.json / jit_method_callees.json / jit_cells.json for `found`.
    /// A method that is compiled already (in this process, by whatever ran before) is read as it is; the others are
    /// compiled now. Used by `jitdump` and by the collision reference run (CollisionJit.cs: the code of the run that
    /// wrote refdata\collision).
    public static void DumpJitted(string outdir, List<MethodBase> found)
    {
        // before anything is compiled here: which methods already were (their entry point leads to the same code with
        // and without PrepareMethod; one that never ran leads to a prestub until it is compiled). jit_code.json
        // "compiled_before": the code of such a method is the code that ran; the others' was made for the dump.
        var before = new Dictionary<MethodBase, IntPtr>();
        foreach (MethodBase m in found)
            try { before[m] = EntryNoJit(m); } catch (Exception) { }
        // the JIT first, for all: a method it refuses (a type that does not load) is reported and left out
        found = found.Where(m =>
        {
            try { JitCodeAddress(m); return true; }
            catch (Exception e) { Console.Error.WriteLine("not jitted: " + m.DeclaringType.FullName + "::" + m.Name + ": " + e.GetType().Name); return false; }
        }).ToList();
        var js = new StringBuilder("[\n");
        using (var bw = new BinaryWriter(new FileStream(Path.Combine(outdir, "jit_code.bin"), FileMode.Create)))
        {
            bw.Write(Encoding.ASCII.GetBytes("JITC")); bw.Write(1u); bw.Write((uint)found.Count);
            for (int i = 0; i < found.Count; i++)
            {
                MethodBase m = found[i];
                MethodBody body = m.GetMethodBody();
                int il = body.GetILAsByteArray().Length;
                IntPtr p = JitCodeAddress(m);
                int nbytes = Math.Min(Math.Max(768, il * 8), 4000000);
                byte[] code = new byte[nbytes];
                int got = 0;
                while (got < nbytes)
                {
                    int chunk = Math.Min(nbytes - got, 4096 - (int)((uint)(p.ToInt32() + got) & 4095));
                    byte[] tmp = new byte[chunk]; IntPtr rd;
                    if (!ReadProcessMemory(GetCurrentProcess(), new IntPtr(p.ToInt32() + got), tmp, new IntPtr(chunk), out rd) || rd.ToInt32() != chunk) break;
                    Buffer.BlockCopy(tmp, 0, code, got, chunk); got += chunk;
                }
                string name = m.DeclaringType.FullName + "::" + m.Name + "(" + string.Join(",", m.GetParameters().Select(q => JitTypeName(q.ParameterType)).ToArray()) + ")";
                byte[] nm = Encoding.ASCII.GetBytes(name);
                bw.Write((uint)nm.Length); bw.Write(nm); bw.Write((uint)p.ToInt32()); bw.Write((uint)got); bw.Write(code, 0, got);
                IntPtr pb;
                bool compiledBefore = before.TryGetValue(m, out pb) && pb == p;
                js.Append("{\"name\":" + Q(name) + ",\"compiled_before\":" + (compiledBefore ? "true" : "false") + ",\"token\":" + m.MetadataToken + ",\"static\":" + (m.IsStatic ? "true" : "false") + ",\"il_size\":" + il + ",\"address\":" + (uint)p.ToInt32() + ",\"nbytes\":" + got + ",\"init_locals\":" + (body.InitLocals ? "true" : "false"));
                js.Append(",\"params\":[" + string.Join(",", m.GetParameters().Select(q => "{\"name\":" + Q(q.Name) + ",\"type\":" + Q(JitTypeName(q.ParameterType)) + "}").ToArray()) + "]");
                js.Append(",\"locals\":[" + string.Join(",", body.LocalVariables.OrderBy(l => l.LocalIndex).Select(l => "{\"index\":" + l.LocalIndex + ",\"type\":" + Q(JitTypeName(l.LocalType)) + ",\"pinned\":" + (l.IsPinned ? "true" : "false") + "}").ToArray()) + "]");
                js.Append(",\"il\":\"" + Hex(body.GetILAsByteArray()) + "\"}" + (i + 1 < found.Count ? ",\n" : "\n"));
                Console.WriteLine("jit " + name + "  il=" + il + "  @" + ((uint)p.ToInt32()).ToString("x8") + "  read " + got);
            }
        }
        js.Append("]\n");
        File.WriteAllBytes(Path.Combine(outdir, "jit_code.json"), new UTF8Encoding(false).GetBytes(js.ToString()));
        WriteCallees(outdir, found);
        WriteCells(outdir);
        Console.WriteLine("jitdump: " + found.Count + " methods");
    }
}
