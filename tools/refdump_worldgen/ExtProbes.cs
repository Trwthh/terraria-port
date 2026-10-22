// RefDumpX87Ext "floatprobe2" mode: the second set of float probes (ExtProbesGen.cs, generated from float_model/probes.py), run in this
// 32-bit .NET Framework process, plus a dump of the machine code the JIT produced for every probe.
//
//   RefDumpX87Ext.exe floatprobe2 <outdir>
//
// <outdir>\probes2.bin (little endian), same layout as float_probes\probes.bin with magic "FPR2":
//   "FPR2" | u32 version = 1 | u32 nprobes | u32 ninputs (= 2000) | u32 0
//   ninputs records f32 x | f32 y | f32 z | i32 n         (the SAME inputs as the first probe set: same generator, same seed)
//   nprobes blocks of ninputs u64, probe order = probes2.json (ordinal name order); 'f' = f32 bits, 'i' = int32 sign-extended
// <outdir>\probes2.json        names and kinds
// <outdir>\probes2_source.cs   the C# of the probes (the text between the PROBES2 markers of ExtProbesGen.cs)
// <outdir>\probes2_code.bin    "JITC" | u32 version = 1 | u32 nprobes | per probe: u32 name length | name (ASCII) | u32 address | u32 nbytes | bytes
//                              (nbytes bytes of memory starting at the method's native entry point, read after the probe ran; the method ends
//                              before nbytes: disassemble up to the last reachable ret, see float_model/disasm.py)
using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;

public static class XP
{
    [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll")] static extern bool ReadProcessMemory(IntPtr process, IntPtr address, [Out] byte[] buffer, IntPtr size, out IntPtr read);
    delegate float Ff(float x, float y, float z, int n);
    delegate int Fi(float x, float y, float z, int n);

    static ulong S = 0xD1B54A32D192ED03UL;
    static ulong Next() { S ^= S >> 12; S ^= S << 25; S ^= S >> 27; return S * 0x2545F4914F6CDD1DUL; }
    static double U01() { return (double)(Next() >> 11) * (1.0 / 9007199254740992.0); }
    static readonly float[] Typical = { 0.97f, 0.1f, 0.5f, 1f / 3f, 16f, 0.0625f, 0.99f, 1.5f, 2f, -1f, 0f, 0.01f };
    static float Comp()
    {
        if (Next() % 12 == 0) return Typical[Next() % (ulong)Typical.Length];
        int m = (int)(Next() % 8);
        double v = m < 3 ? -1 + 2 * U01() : m < 6 ? -100 + 200 * U01() : -5000 + 10000 * U01();
        return (float)v;
    }

    // native entry point of a jitted method: the function pointer is a precode stub "jmp rel32" (or "call"/"mov+jmp" fixups) into the code
    static IntPtr CodeAddress(MethodInfo m)
    {
        RuntimeHelpers.PrepareMethod(m.MethodHandle);
        IntPtr p = m.MethodHandle.GetFunctionPointer();
        for (int hop = 0; hop < 4; hop++)
        {
            byte b0 = Marshal.ReadByte(p);
            if (b0 == 0xE9) { int rel = Marshal.ReadInt32(p, 1); p = new IntPtr(p.ToInt32() + 5 + rel); continue; }                // jmp rel32
            if (b0 == 0xB8 && Marshal.ReadByte(p, 5) == 0x90 && Marshal.ReadByte(p, 6) == 0xE9)                                    // mov eax, imm32; nop; jmp rel32 (stub precode)
            { int rel = Marshal.ReadInt32(p, 7); p = new IntPtr(p.ToInt32() + 11 + rel); continue; }
            break;
        }
        return p;
    }

    public static int Run(string outdir)
    {
        Directory.CreateDirectory(outdir);
        const int N = 2000;
        float[] xs = new float[N], ys = new float[N], zs = new float[N]; int[] ns = new int[N];
        for (int i = 0; i < N; i++) { xs[i] = Comp(); ys[i] = Comp(); zs[i] = Comp(); ns[i] = (int)(Next() % 84) - 20; }
        var ms = new List<MethodInfo>();
        foreach (var m in typeof(FP2).GetMethods(BindingFlags.Public | BindingFlags.Static)) if (m.Name.StartsWith("Q_")) ms.Add(m);
        ms.Sort((a, b) => string.CompareOrdinal(a.Name, b.Name));
        var results = new List<ulong[]>(); var kinds = new List<char>();
        foreach (var m in ms)
        {
            ulong[] r = new ulong[N];
            if (m.ReturnType == typeof(float))
            {
                Ff f = (Ff)Delegate.CreateDelegate(typeof(Ff), m);
                float[] cell = new float[1];
                for (int i = 0; i < N; i++) { cell[0] = f(xs[i], ys[i], zs[i], ns[i]); r[i] = (uint)BitConverter.ToInt32(BitConverter.GetBytes(cell[0]), 0); }
                kinds.Add('f');
            }
            else
            {
                Fi f = (Fi)Delegate.CreateDelegate(typeof(Fi), m);
                for (int i = 0; i < N; i++) r[i] = (ulong)(long)f(xs[i], ys[i], zs[i], ns[i]);
                kinds.Add('i');
            }
            results.Add(r);
        }
        using (var bw = new BinaryWriter(new FileStream(Path.Combine(outdir, "probes2.bin"), FileMode.Create)))
        {
            bw.Write(Encoding.ASCII.GetBytes("FPR2")); bw.Write(1u); bw.Write((uint)ms.Count); bw.Write((uint)N); bw.Write(0u);
            for (int i = 0; i < N; i++) { bw.Write(xs[i]); bw.Write(ys[i]); bw.Write(zs[i]); bw.Write(ns[i]); }
            foreach (ulong[] r in results) foreach (ulong v in r) bw.Write(v);
        }
        var sb = new StringBuilder("{\n\"ninputs\":" + N + ",\n\"intptr_size\":" + IntPtr.Size + ",\n\"probes\":[\n");
        for (int j = 0; j < ms.Count; j++) sb.Append("  {\"name\":\"" + ms[j].Name + "\",\"kind\":\"" + kinds[j] + "\",\"il_bytes\":" + ms[j].GetMethodBody().GetILAsByteArray().Length + "}" + (j + 1 < ms.Count ? ",\n" : "\n"));
        sb.Append("]\n}\n");
        File.WriteAllText(Path.Combine(outdir, "probes2.json"), sb.ToString(), new UTF8Encoding(false));
        string src = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "ExtProbesGen.cs");
        if (File.Exists(src))
        {
            string all = File.ReadAllText(src);
            int a = all.IndexOf("// <<<PROBES2") + "// <<<PROBES2".Length, b = all.IndexOf("// >>>PROBES2");
            File.WriteAllText(Path.Combine(outdir, "probes2_source.cs"), all.Substring(a, b - a).Trim('\r', '\n') + "\n", new UTF8Encoding(false));
        }
        using (var bw = new BinaryWriter(new FileStream(Path.Combine(outdir, "probes2_code.bin"), FileMode.Create)))
        {
            bw.Write(Encoding.ASCII.GetBytes("JITC")); bw.Write(1u); bw.Write((uint)ms.Count);
            foreach (var m in ms)
            {
                IntPtr p = CodeAddress(m);
                int il = m.GetMethodBody().GetILAsByteArray().Length;
                int nbytes = Math.Min(Math.Max(768, il * 6), 400000);
                byte[] code = new byte[nbytes];
                // ReadProcessMemory instead of Marshal.Copy: the code heap may end before nbytes (page by page, stop at the first failure)
                int got = 0;
                while (got < nbytes)
                {
                    int chunk = Math.Min(nbytes - got, 4096 - (int)((uint)(p.ToInt32() + got) & 4095));
                    byte[] tmp = new byte[chunk]; IntPtr rd;
                    if (!ReadProcessMemory(GetCurrentProcess(), new IntPtr(p.ToInt32() + got), tmp, new IntPtr(chunk), out rd) || rd.ToInt32() != chunk) break;
                    Buffer.BlockCopy(tmp, 0, code, got, chunk); got += chunk;
                }
                nbytes = got;
                byte[] nm = Encoding.ASCII.GetBytes(m.Name);
                bw.Write((uint)nm.Length); bw.Write(nm); bw.Write((uint)p.ToInt32()); bw.Write((uint)nbytes); bw.Write(code, 0, nbytes);
            }
        }
        Console.WriteLine("float probes 2: " + ms.Count + " x " + N);
        return 0;
    }
}
