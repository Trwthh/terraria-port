// RefDumpX87Ext "x87raw" / "x87dense" / "x87more" modes: more System.Math reference data from the 32-bit .NET Framework 4 process.
// Separate program from RefDumpWorldgen.exe (build_ext.cmd) so that the original harness and its outputs stay byte-identical.
//
//   RefDumpX87Ext.exe x87raw   <outdir>   raw x87 instruction results at the full 80-bit register width (fsin, fcos, fptan, fpatan, fyl2x),
//                                         obtained by calling 12..20-byte machine-code thunks; next to each the System.Math result.
//   RefDumpX87Ext.exe x87dense <outdir>   dense System.Math tables (results only; the inputs are regenerated from seed/lo/hi, see below)
//   RefDumpX87Ext.exe x87more  <outdir>   20000-record tables in the X87T format of X87.cs for functions the first dump did not cover,
//                                         float-cast variants and "used without a store" variants
//
// Formats (little endian):
//   x87raw:   "X87R" | u32 version=1 | u32 nargs | u32 count | u32 0 | count x { nargs x f64 args | 10 bytes x87 extended result | f64 System.Math result }
//   x87dense: "X87D" | u32 version=1 | u32 nargs | u32 count | u32 mode | u64 seed | f64 lo0 | f64 hi0 | f64 lo1 | f64 hi1 | u64 fnv1a64(input bits) | count x f64 result
//             inputs: xorshift64* (s ^= s>>12; s ^= s<<25; s ^= s>>27; out = s * 0x2545F4914F6CDD1D), u = (out>>11) * 2^-53,
//             arg_k = lo_k + (hi_k - lo_k) * u (one draw per argument, argument 0 first); mode 1: every argument narrowed to float and widened again.
//   x87more:  X87T (see X87.cs); float tables "X87F": "X87F" | u32 version=1 | u32 nargs | u32 count | u32 0 | count x { nargs x f32 | f32 result }
using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;

static class XE
{
    [DllImport("kernel32.dll")] static extern IntPtr VirtualAlloc(IntPtr addr, UIntPtr size, uint type, uint protect);
    [DllImport("kernel32.dll")] static extern bool VirtualFree(IntPtr addr, UIntPtr size, uint type);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate void Thunk(double[] args, byte[] out10);

    public class Rng
    {
        ulong s;
        public Rng(ulong seed) { s = seed; }
        public ulong Next() { s ^= s >> 12; s ^= s << 25; s ^= s >> 27; return s * 0x2545F4914F6CDD1DUL; }
        public double U01() { return (double)(Next() >> 11) * (1.0 / 9007199254740992.0); }
        public double Range(double lo, double hi) { return lo + (hi - lo) * U01(); }
        public int Pick(int n) { return (int)(Next() % (ulong)n); }
    }
    static readonly float[] fcell = new float[1];
    static double F32(double d) { fcell[0] = (float)d; return (double)fcell[0]; }

    // ---------------------------------------------------------------- raw x87 instructions
    // void thunk(double* args, byte* out10)   cdecl
    static readonly byte[] PRE1 = { 0x8B, 0x44, 0x24, 0x04, 0xDD, 0x00 };                          // mov eax,[esp+4]; fld qword [eax]
    static readonly byte[] PRE2 = { 0x8B, 0x44, 0x24, 0x04, 0xDD, 0x00, 0xDD, 0x40, 0x08 };        // ...; fld qword [eax+8]     (ST1 = arg0, ST0 = arg1)
    static readonly byte[] POST = { 0x8B, 0x44, 0x24, 0x08, 0xDB, 0x38, 0xC3 };                    // mov eax,[esp+8]; fstp tbyte [eax]; ret
    static byte[] Cat(params byte[][] p) { var l = new List<byte>(); foreach (var b in p) l.AddRange(b); return l.ToArray(); }

    static IntPtr mem; static int memUsed;
    static Thunk Make(byte[] code)
    {
        if (mem == IntPtr.Zero) mem = VirtualAlloc(IntPtr.Zero, (UIntPtr)4096, 0x3000, 0x40);
        IntPtr p = mem + memUsed;
        Marshal.Copy(code, 0, p, code.Length);
        memUsed += (code.Length + 15) & ~15;
        return (Thunk)Marshal.GetDelegateForFunctionPointer(p, typeof(Thunk));
    }

    static void WriteRaw(string outdir, string name, List<double[]> recs, int nargs, Thunk t, Func<double[], double> clr)
    {
        using (var bw = new BinaryWriter(new FileStream(Path.Combine(outdir, name + ".bin"), FileMode.Create)))
        {
            bw.Write(Encoding.ASCII.GetBytes("X87R")); bw.Write(1u); bw.Write((uint)nargs); bw.Write((uint)recs.Count); bw.Write(0u);
            byte[] o = new byte[10]; double[] cell = new double[1];
            foreach (double[] a in recs)
            {
                t(a, o);
                cell[0] = clr(a);
                foreach (double v in a) bw.Write(BitConverter.DoubleToInt64Bits(v));
                bw.Write(o);
                bw.Write(BitConverter.DoubleToInt64Bits(cell[0]));
            }
        }
        Console.WriteLine("x87raw " + name + ": " + recs.Count);
    }

    static List<double[]> ReadArgs(string path, int nargs)
    {
        var l = new List<double[]>();
        if (!File.Exists(path)) return l;
        byte[] b = File.ReadAllBytes(path);
        int n = BitConverter.ToInt32(b, 12);
        for (int i = 0; i < n; i++)
        {
            double[] a = new double[nargs];
            for (int k = 0; k < nargs; k++) a[k] = BitConverter.ToDouble(b, 20 + i * 8 * (nargs + 1) + 8 * k);
            l.Add(a);
        }
        return l;
    }

    static void TrigRaw(Rng r, List<double[]> o)
    {
        for (int i = 0; i < 100000; i++) o.Add(new[] { r.Range(-10, 10) });
        for (int i = 0; i < 40000; i++) o.Add(new[] { r.Range(-1000, 1000) });
        for (int i = 0; i < 30000; i++) { double m = Math.Pow(2.0, r.Range(-60, 64)) ; o.Add(new[] { r.Pick(2) == 0 ? m : -m }); }      // every magnitude up to past 2^63
        for (int i = 0; i < 30000; i++) { int k = 1 + r.Pick(1000); double x = k * (Math.PI / 2) * (1 + (r.U01() - 0.5) * Math.Pow(2.0, -r.Pick(50))); o.Add(new[] { x }); }   // near multiples of pi/2
        for (int i = 0; i < 20000; i++) o.Add(new[] { F32(r.Range(-10, 10)) });
        for (int i = 0; i < 2000; i++) { ulong bits = 0x43D0000000000000UL + (ulong)(i - 1000) ; o.Add(new[] { BitConverter.Int64BitsToDouble((long)bits) }); }   // around 2^62
        for (int i = 0; i < 2000; i++) { ulong bits = 0x43E0000000000000UL + (ulong)(i - 1000) ; o.Add(new[] { BitConverter.Int64BitsToDouble((long)bits) }); }   // around 2^63
    }

    public static int Raw(string outdir, string x87dir)
    {
        Directory.CreateDirectory(outdir);
        Thunk fsin = Make(Cat(PRE1, new byte[] { 0xD9, 0xFE }, POST));
        Thunk fcos = Make(Cat(PRE1, new byte[] { 0xD9, 0xFF }, POST));
        Thunk fptan = Make(Cat(PRE1, new byte[] { 0xD9, 0xF2, 0xDD, 0xD8 }, POST));          // fptan; fstp st(0)
        Thunk fpatan = Make(Cat(PRE2, new byte[] { 0xD9, 0xF3 }, POST));                     // atan(ST1/ST0) = atan2(arg0, arg1)
        Thunk fpatan1 = Make(Cat(PRE1, new byte[] { 0xD9, 0xE8, 0xD9, 0xF3 }, POST));        // fld1; fpatan
        Thunk fyl2x = Make(Cat(PRE2, new byte[] { 0xD9, 0xF1 }, POST));                      // arg0 * log2(arg1)
        Thunk fsqrt = Make(Cat(PRE1, new byte[] { 0xD9, 0xFA }, POST));
        Thunk fld = Make(Cat(PRE1, POST));                                                   // identity: checks the thunk plumbing

        byte[] o = new byte[10];
        fld(new[] { 1.5 }, o);
        if (!(o[9] == 0x3F && o[8] == 0xFF && o[7] == 0xC0)) throw new Exception("thunk self-test failed");

        List<double[]> l;
        Rng r = new Rng(0xA5A5A5A55A5A5A5AUL);
        l = ReadArgs(Path.Combine(x87dir, "Sin.bin"), 1); TrigRaw(r, l); WriteRaw(outdir, "Fsin", l, 1, fsin, a => Math.Sin(a[0]));
        l = ReadArgs(Path.Combine(x87dir, "Cos.bin"), 1); TrigRaw(r, l); WriteRaw(outdir, "Fcos", l, 1, fcos, a => Math.Cos(a[0]));
        l = ReadArgs(Path.Combine(x87dir, "Tan.bin"), 1); TrigRaw(r, l); WriteRaw(outdir, "Fptan", l, 1, fptan, a => Math.Tan(a[0]));
        l = ReadArgs(Path.Combine(x87dir, "Atan.bin"), 1);
        for (int i = 0; i < 100000; i++) l.Add(new[] { r.Range(-10, 10) });
        for (int i = 0; i < 50000; i++) { double m = Math.Pow(2.0, r.Range(-80, 80)); l.Add(new[] { r.Pick(2) == 0 ? m : -m }); }
        WriteRaw(outdir, "Fpatan1", l, 1, fpatan1, a => Math.Atan(a[0]));
        l = ReadArgs(Path.Combine(x87dir, "Atan2.bin"), 2);
        for (int i = 0; i < 80000; i++) l.Add(new[] { r.Range(-1000, 1000), r.Range(-1000, 1000) });
        for (int i = 0; i < 60000; i++) l.Add(new[] { F32(r.Range(-1000, 1000)), F32(r.Range(-1000, 1000)) });
        for (int i = 0; i < 30000; i++) { double a = Math.Pow(2.0, r.Range(-40, 40)), b = Math.Pow(2.0, r.Range(-40, 40)); l.Add(new[] { r.Pick(2) == 0 ? a : -a, r.Pick(2) == 0 ? b : -b }); }
        for (int i = 0; i < 30000; i++) l.Add(new[] { (double)(r.Pick(65) - 32), (double)(r.Pick(65) - 32) });                 // small integers (tile deltas)
        WriteRaw(outdir, "Fpatan", l, 2, fpatan, a => Math.Atan2(a[0], a[1]));
        l = new List<double[]>();
        for (int i = 0; i < 50000; i++) l.Add(new[] { r.Range(-5, 5), r.Range(0, 100) });
        WriteRaw(outdir, "Fyl2x", l, 2, fyl2x, a => a[0] * Math.Log(a[1], 2.0));
        l = ReadArgs(Path.Combine(x87dir, "Sqrt.bin"), 1);
        WriteRaw(outdir, "Fsqrt", l, 1, fsqrt, a => Math.Sqrt(a[0]));
        return 0;
    }

    // ---------------------------------------------------------------- dense System.Math tables
    static ulong Fnv(ulong h, ulong v) { for (int i = 0; i < 8; i++) { h ^= (v >> (8 * i)) & 0xFF; h *= 0x100000001b3UL; } return h; }

    static void Dense(string outdir, string name, int nargs, int count, int mode, ulong seed, double lo0, double hi0, double lo1, double hi1, Func<double, double, double> f)
    {
        Rng r = new Rng(seed);
        double[] res = new double[count];
        ulong h = 0xcbf29ce484222325UL;
        for (int i = 0; i < count; i++)
        {
            double a = r.Range(lo0, hi0), b = 0;
            if (nargs == 2) b = r.Range(lo1, hi1);
            if (mode == 1) { a = F32(a); if (nargs == 2) b = F32(b); }
            h = Fnv(h, (ulong)BitConverter.DoubleToInt64Bits(a));
            if (nargs == 2) h = Fnv(h, (ulong)BitConverter.DoubleToInt64Bits(b));
            res[i] = f(a, b);
        }
        using (var bw = new BinaryWriter(new FileStream(Path.Combine(outdir, name + ".bin"), FileMode.Create)))
        {
            bw.Write(Encoding.ASCII.GetBytes("X87D")); bw.Write(1u); bw.Write((uint)nargs); bw.Write((uint)count); bw.Write((uint)mode);
            bw.Write(seed); bw.Write(lo0); bw.Write(hi0); bw.Write(lo1); bw.Write(hi1); bw.Write(h);
            byte[] buf = new byte[count * 8];
            Buffer.BlockCopy(res, 0, buf, 0, buf.Length);
            bw.Write(buf);
        }
        Console.WriteLine("x87dense " + name + ": " + count);
    }

    public static int DenseAll(string outdir)
    {
        Directory.CreateDirectory(outdir);
        const int M = 1000000, Q = 250000;
        Dense(outdir, "Sin_m10_10", 1, M, 0, 0x1111111111111111UL, -10, 10, 0, 0, (a, b) => Math.Sin(a));
        Dense(outdir, "Sin_0_1000", 1, M, 0, 0x2222222222222222UL, 0, 1000, 0, 0, (a, b) => Math.Sin(a));
        Dense(outdir, "Sin_f32_m7_7", 1, Q, 1, 0x3333333333333333UL, -7, 7, 0, 0, (a, b) => Math.Sin(a));
        Dense(outdir, "Cos_m10_10", 1, M, 0, 0x4444444444444444UL, -10, 10, 0, 0, (a, b) => Math.Cos(a));
        Dense(outdir, "Cos_0_1000", 1, M, 0, 0x5555555555555555UL, 0, 1000, 0, 0, (a, b) => Math.Cos(a));
        Dense(outdir, "Cos_f32_m7_7", 1, Q, 1, 0x6666666666666666UL, -7, 7, 0, 0, (a, b) => Math.Cos(a));
        Dense(outdir, "Tan_m10_10", 1, Q, 0, 0x7777777777777777UL, -10, 10, 0, 0, (a, b) => Math.Tan(a));
        Dense(outdir, "Atan_m100_100", 1, Q, 0, 0x8888888888888888UL, -100, 100, 0, 0, (a, b) => Math.Atan(a));
        Dense(outdir, "Atan2_m1000_1000", 2, Q, 0, 0x9999999999999999UL, -1000, 1000, -1000, 1000, (a, b) => Math.Atan2(a, b));
        Dense(outdir, "Atan2_f32_m1000_1000", 2, Q, 1, 0xAAAAAAAAAAAAAAAAUL, -1000, 1000, -1000, 1000, (a, b) => Math.Atan2(a, b));
        Dense(outdir, "Pow_0_100_m5_5", 2, Q, 0, 0xBBBBBBBBBBBBBBBBUL, 0, 100, -5, 5, (a, b) => Math.Pow(a, b));
        Dense(outdir, "Pow_f32_0_10_0_4", 2, Q, 1, 0xCCCCCCCCCCCCCCCCUL, 0, 10, 0, 4, (a, b) => Math.Pow(a, b));
        Dense(outdir, "Log_0_1000", 1, Q, 0, 0xDDDDDDDDDDDDDDDDUL, 0, 1000, 0, 0, (a, b) => Math.Log(a));
        Dense(outdir, "Log10_0_1000", 1, Q, 0, 0xEEEEEEEEEEEEEEEEUL, 0, 1000, 0, 0, (a, b) => Math.Log10(a));
        Dense(outdir, "Exp_m20_20", 1, Q, 0, 0x1212121212121212UL, -20, 20, 0, 0, (a, b) => Math.Exp(a));
        Dense(outdir, "Acos_m1_1", 1, Q, 0, 0x2323232323232323UL, -1, 1, 0, 0, (a, b) => Math.Acos(a));
        Dense(outdir, "Asin_m1_1", 1, Q, 0, 0x3434343434343434UL, -1, 1, 0, 0, (a, b) => Math.Asin(a));
        Dense(outdir, "Sinh_m10_10", 1, Q, 0, 0x4545454545454545UL, -10, 10, 0, 0, (a, b) => Math.Sinh(a));
        Dense(outdir, "Cosh_m10_10", 1, Q, 0, 0x5656565656565656UL, -10, 10, 0, 0, (a, b) => Math.Cosh(a));
        Dense(outdir, "Tanh_m10_10", 1, Q, 0, 0x6767676767676767UL, -10, 10, 0, 0, (a, b) => Math.Tanh(a));
        return 0;
    }

    // ---------------------------------------------------------------- more 20000-record tables
    static readonly double[] Specials = {
        0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 2.0, 0.1, 1e-300, 2.2250738585072014e-308, 4.94065645841247e-324, -4.94065645841247e-324, 1e-310,
        double.PositiveInfinity, double.NegativeInfinity, double.NaN, double.MaxValue, double.MinValue,
        1e6, -1e6, 1e22, 1e300, 4503599627370496.0, 9007199254740992.0, 0.49999999999999994, 1.5, 2.5, -1.5, -2.5, 3.5, 1e15 + 0.5,
        Math.PI, -Math.PI, Math.E, 10.0, 100.0, 709.0, 710.0, 711.0, -709.0, -745.0, -746.0, 19.0, 20.0, 22.0, 23.0, 1e-9, -1e-9, 3e-8, 1e-5
    };
    static void WriteT(string outdir, string name, List<double[]> recs, int nargs, Func<double[], double> f)
    {
        using (var bw = new BinaryWriter(new FileStream(Path.Combine(outdir, name + ".bin"), FileMode.Create)))
        {
            bw.Write(Encoding.ASCII.GetBytes("X87T")); bw.Write(1u); bw.Write((uint)nargs); bw.Write((uint)recs.Count); bw.Write(0u);
            double[] cell = new double[1];
            foreach (double[] a in recs)
            {
                cell[0] = f(a);
                foreach (double v in a) bw.Write(BitConverter.DoubleToInt64Bits(v));
                bw.Write(BitConverter.DoubleToInt64Bits(cell[0]));
            }
        }
        Console.WriteLine("x87more " + name + ": " + recs.Count);
    }
    static void WriteF(string outdir, string name, List<float[]> recs, int nargs, Func<float[], float> f)
    {
        using (var bw = new BinaryWriter(new FileStream(Path.Combine(outdir, name + ".bin"), FileMode.Create)))
        {
            bw.Write(Encoding.ASCII.GetBytes("X87F")); bw.Write(1u); bw.Write((uint)nargs); bw.Write((uint)recs.Count); bw.Write(0u);
            float[] cell = new float[1];
            foreach (float[] a in recs)
            {
                cell[0] = f(a);
                foreach (float v in a) bw.Write(v);
                bw.Write(cell[0]);
            }
        }
        Console.WriteLine("x87more " + name + ": " + recs.Count);
    }
    static List<double[]> One(Rng r, int n, Func<Rng, double> g)
    {
        var l = new List<double[]>();
        foreach (double s in Specials) l.Add(new[] { s });
        while (l.Count < n) l.Add(new[] { g(r) });
        return l;
    }
    static List<double[]> Two(Rng r, int n, double[] sp, Func<Rng, double[]> g)
    {
        var l = new List<double[]>();
        foreach (double a in sp) foreach (double b in sp) l.Add(new[] { a, b });
        while (l.Count < n) l.Add(g(r));
        return l;
    }
    static double Mixed(Rng r)
    {
        int m = r.Pick(6);
        if (m < 2) return r.Range(-1e6, 1e6);
        if (m < 3) return Math.Floor(r.Range(-1000, 1000)) + 0.5;
        if (m < 4) return F32(r.Range(-5000, 5000));
        if (m < 5) return r.Range(-10, 10);
        return r.Range(-1e15, 1e15);
    }
    static int SignOrCode(double d) { try { return Math.Sign(d); } catch (ArithmeticException) { return -99; } }

    // the float-cast shapes the game uses; NoInlining so that each is its own JIT unit
    [MethodImpl(MethodImplOptions.NoInlining)] static float SinF(float f) { return (float)Math.Sin(f); }
    [MethodImpl(MethodImplOptions.NoInlining)] static float CosF(float f) { return (float)Math.Cos(f); }
    [MethodImpl(MethodImplOptions.NoInlining)] static float SqrtF(float f) { return (float)Math.Sqrt(f); }
    [MethodImpl(MethodImplOptions.NoInlining)] static float Atan2F(float y, float x) { return (float)Math.Atan2(y, x); }
    [MethodImpl(MethodImplOptions.NoInlining)] static float PowF(float x, float y) { return (float)Math.Pow(x, y); }
    [MethodImpl(MethodImplOptions.NoInlining)] static float AbsF(float x, float y) { return Math.Abs(x * y); }
    // results used without a store in between (is the 80-bit result of the instruction visible to the next operation?)
    [MethodImpl(MethodImplOptions.NoInlining)] static double SinMul(double x, double y) { return Math.Sin(x) * y; }
    [MethodImpl(MethodImplOptions.NoInlining)] static double CosMul(double x, double y) { return Math.Cos(x) * y; }
    [MethodImpl(MethodImplOptions.NoInlining)] static double SinAdd(double x, double y) { return Math.Sin(x) + y; }
    [MethodImpl(MethodImplOptions.NoInlining)] static double Atan2Mul(double y, double x, double m) { return Math.Atan2(y, x) * m; }
    [MethodImpl(MethodImplOptions.NoInlining)] static double SqrtMul(double x, double y) { return Math.Sqrt(x) * y; }
    [MethodImpl(MethodImplOptions.NoInlining)] static double PowMul(double x, double y) { return Math.Pow(x, 1.5) * y; }
    [MethodImpl(MethodImplOptions.NoInlining)] static double SinLocalMul(double x, double y) { double s = Math.Sin(x); double c = Math.Cos(x); return s * y + c; }
    [MethodImpl(MethodImplOptions.NoInlining)] static double TanMul(double x, double y) { return Math.Tan(x) * y; }
    [MethodImpl(MethodImplOptions.NoInlining)] static double AtanMul(double x, double y) { return Math.Atan(x) * y; }
    // doubles: products / sums of doubles kept in locals vs stored (no extra precision expected at a 53-bit control word)
    static double SD;
    [MethodImpl(MethodImplOptions.NoInlining)] static double DblLocal(double x, double y) { double a = x * y + 0.1; double b = a / 3.0 - x; return a * b + y; }
    [MethodImpl(MethodImplOptions.NoInlining)] static double DblField(double x, double y) { SD = x * y + 0.1; double b = SD / 3.0 - x; return SD * b + y; }

    public static int More(string outdir)
    {
        Directory.CreateDirectory(outdir);
        const int N = 20000;
        Rng r = new Rng(0x0F1E2D3C4B5A6978UL);
        WriteT(outdir, "Sinh", One(r, N, q => q.Pick(3) == 0 ? q.Range(-710, 710) : q.Pick(2) == 0 ? q.Range(-1, 1) : q.Range(-25, 25)), 1, a => Math.Sinh(a[0]));
        WriteT(outdir, "Cosh", One(r, N, q => q.Pick(3) == 0 ? q.Range(-710, 710) : q.Pick(2) == 0 ? q.Range(-1, 1) : q.Range(-25, 25)), 1, a => Math.Cosh(a[0]));
        WriteT(outdir, "Tanh", One(r, N, q => q.Pick(3) == 0 ? q.Range(-50, 50) : q.Pick(2) == 0 ? q.Range(-1, 1) : q.Range(-25, 25)), 1, a => Math.Tanh(a[0]));
        WriteT(outdir, "Truncate", One(r, N, Mixed), 1, a => Math.Truncate(a[0]));
        WriteT(outdir, "Sign", One(r, N, Mixed), 1, a => (double)SignOrCode(a[0]));
        for (int d = 0; d <= 15; d += (d < 4 ? 1 : 11))          // digits 0,1,2,3,4,15
        {
            int dd = d;
            WriteT(outdir, "Round" + dd, One(r, N, q => q.Pick(4) == 0 ? Math.Floor(q.Range(-100000, 100000)) / 1000.0 * 5 : q.Pick(3) == 0 ? F32(q.Range(-5000, 5000)) : Mixed(q)), 1, a => Math.Round(a[0], dd));
        }
        WriteT(outdir, "Round2Away", One(r, N, q => q.Pick(4) == 0 ? Math.Floor(q.Range(-100000, 100000)) / 1000.0 * 5 : Mixed(q)), 1, a => Math.Round(a[0], 2, MidpointRounding.AwayFromZero));
        double[] sp = { 0.0, -0.0, 1.0, -1.0, 0.5, 2.5, -2.5, 3.0, double.PositiveInfinity, double.NegativeInfinity, double.NaN, 1e300, 5e-324, Math.PI, 6.283185307179586, 360.0, 180.0 };
        Func<Rng, double[]> pair = q =>
        {
            int m = q.Pick(6);
            if (m < 2) return new[] { q.Range(-1000, 1000), q.Range(-1000, 1000) };
            if (m < 3) return new[] { F32(q.Range(-100, 100)), F32(q.Range(-100, 100)) };
            if (m < 4) return new[] { q.Range(-100, 100), 6.283185307179586 };                      // angle wrapping, as the game does
            if (m < 5) return new[] { (double)(q.Pick(2001) - 1000) * 0.5, (double)(q.Pick(41) - 20) };  // exact ties
            return new[] { q.Range(-1e6, 1e6), q.Range(-10, 10) };
        };
        WriteT(outdir, "IEEERemainder", Two(r, N, sp, pair), 2, a => Math.IEEERemainder(a[0], a[1]));
        WriteT(outdir, "Min", Two(r, N, sp, pair), 2, a => Math.Min(a[0], a[1]));
        WriteT(outdir, "Max", Two(r, N, sp, pair), 2, a => Math.Max(a[0], a[1]));
        WriteT(outdir, "Fmod", Two(r, N, sp, pair), 2, a => a[0] % a[1]);
        double[] psp = { 0.0, -0.0, 1.0, -1.0, 2.0, -2.0, 0.5, -0.5, 3.0, -3.0, 1.5, double.PositiveInfinity, double.NegativeInfinity, double.NaN, 1e300, -1e300, 5e-324, 1e-300,
            0.99999999999999989, 1.0000000000000002, 2.5, 1e-5, 1023.0, 1024.0, -1074.0, 1e17, 4.0, -4.0, 9007199254740993.0, -9007199254740992.0 };
        WriteT(outdir, "PowSpecial", Two(r, psp.Length * psp.Length, psp, q => null), 2, a => Math.Pow(a[0], a[1]));
        Func<Rng, double[]> trigpair = q => new[] { q.Pick(4) == 0 ? F32(q.Range(-10, 10)) : q.Range(-10, 10), q.Pick(3) == 0 ? F32(q.Range(-100, 100)) : q.Range(-100, 100) };
        double[] sp2 = { 0.0, 1.0, -1.0, 0.5, Math.PI, 3.0, 1e-5 };
        WriteT(outdir, "SinMul", Two(r, N, sp2, trigpair), 2, a => SinMul(a[0], a[1]));
        WriteT(outdir, "CosMul", Two(r, N, sp2, trigpair), 2, a => CosMul(a[0], a[1]));
        WriteT(outdir, "SinAdd", Two(r, N, sp2, q => new[] { q.Range(-10, 10), q.Range(-1, 1) }), 2, a => SinAdd(a[0], a[1]));
        WriteT(outdir, "SinLocalMul", Two(r, N, sp2, trigpair), 2, a => SinLocalMul(a[0], a[1]));
        WriteT(outdir, "TanMul", Two(r, N, sp2, trigpair), 2, a => TanMul(a[0], a[1]));
        WriteT(outdir, "AtanMul", Two(r, N, sp2, trigpair), 2, a => AtanMul(a[0], a[1]));
        WriteT(outdir, "SqrtMul", Two(r, N, sp2, q => new[] { q.Range(0, 1000), q.Range(-100, 100) }), 2, a => SqrtMul(a[0], a[1]));
        WriteT(outdir, "PowMul", Two(r, N, sp2, q => new[] { q.Range(0, 100), q.Range(-100, 100) }), 2, a => PowMul(a[0], a[1]));
        WriteT(outdir, "Atan2Mul3", Two(r, N, sp2, q => new[] { q.Range(-100, 100), q.Range(-100, 100) }), 2, a => Atan2Mul(a[0], a[1], 57.29577951308232));
        WriteT(outdir, "DblLocal", Two(r, N, sp2, q => new[] { q.Range(-100, 100), q.Range(-100, 100) }), 2, a => DblLocal(a[0], a[1]));
        WriteT(outdir, "DblField", Two(r, N, sp2, q => new[] { q.Range(-100, 100), q.Range(-100, 100) }), 2, a => DblField(a[0], a[1]));

        var fl = new List<float[]>();
        foreach (float s in new[] { 0f, -0f, 1f, -1f, 0.5f, 3.14159274f, 1.57079637f, 6.28318548f, float.NaN, float.PositiveInfinity, 1e-20f, 1e20f }) fl.Add(new[] { s });
        while (fl.Count < N) fl.Add(new[] { (float)r.Range(-10, 10) });
        WriteF(outdir, "SinF", fl, 1, a => SinF(a[0]));
        WriteF(outdir, "CosF", fl, 1, a => CosF(a[0]));
        fl = new List<float[]>();
        foreach (float s in new[] { 0f, -0f, 1f, 2f, 0.5f, float.NaN, float.PositiveInfinity, -1f, 1e-30f, 1e30f }) fl.Add(new[] { s });
        while (fl.Count < N) fl.Add(new[] { (float)(r.Pick(2) == 0 ? r.Range(0, 100) : r.Range(0, 1e6)) });
        WriteF(outdir, "SqrtF", fl, 1, a => SqrtF(a[0]));
        fl = new List<float[]>();
        while (fl.Count < N) fl.Add(new[] { (float)r.Range(-1000, 1000), (float)r.Range(-1000, 1000) });
        WriteF(outdir, "Atan2F", fl, 2, a => Atan2F(a[0], a[1]));
        WriteF(outdir, "AbsMulF", fl, 2, a => AbsF(a[0], a[1]));
        fl = new List<float[]>();
        while (fl.Count < N) fl.Add(new[] { (float)r.Range(0, 100), (float)(r.Pick(2) == 0 ? r.Range(-3, 3) : (r.Pick(9) - 2) * 0.5) });
        WriteF(outdir, "PowF", fl, 2, a => PowF(a[0], a[1]));
        return 0;
    }
}
