// RefDumpWorldgen "x87" mode: reference table of System.Math results as computed by this 32-bit .NET Framework process
// (the same JIT and the same FPU code the 32-bit game uses). Calls System.Math directly; no game code is loaded.
//
//   RefDumpWorldgen.exe x87 <outdir>
//
// One file per function, <outdir>\<Function>.bin (little endian):
//   magic "X87T" | u32 version = 1 | u32 nargs (1 or 2) | u32 count | u32 0
//   then count records of (nargs + 1) IEEE-754 binary64 values: arg0 [, arg1], result.   Raw bit patterns (NaN payloads/signs are as produced).
// Inputs come from a deterministic xorshift64* generator (seed 0x9E3779B97F4A7C15), documented in README.md; the list of special inputs
// comes first, then random draws until 20000 records.
// <outdir>\x87_env.json records IntPtr.Size, the x87 control word (via msvcrt _controlfp), CLR version and a few probe values.
using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

static partial class WG
{
    [DllImport("msvcrt.dll", CallingConvention = CallingConvention.Cdecl)]
    static extern uint _controlfp(uint newValue, uint mask);

    class XRng
    {
        ulong s = 0x9E3779B97F4A7C15UL;
        public ulong Next() { s ^= s >> 12; s ^= s << 25; s ^= s >> 27; return s * 0x2545F4914F6CDD1DUL; }
        public double U01() { return (double)(Next() >> 11) * (1.0 / 9007199254740992.0); }          // [0,1)
        public double Range(double lo, double hi) { return lo + (hi - lo) * U01(); }
        public int Pick(int n) { return (int)(Next() % (ulong)n); }
    }

    static double F32(double d) { return (double)(float)d; }
    static double NegZero { get { return BitConverter.Int64BitsToDouble(unchecked((long)0x8000000000000000UL)); } }
    static readonly double Denorm = BitConverter.Int64BitsToDouble(1);            // 4.94e-324
    static readonly double[] Specials = {
        0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 2.0, 0.1, 1e-300, 2.2250738585072014e-308, 4.94065645841247e-324, -4.94065645841247e-324, 1e-310,
        double.PositiveInfinity, double.NegativeInfinity, double.NaN, double.MaxValue, double.MinValue, double.Epsilon,
        1e6, -1e6, 1e22, 1e300, 4503599627370496.0, 9007199254740992.0, 0.49999999999999994, 1.5, 2.5, -1.5, -2.5, 3.5, 1e15 + 0.5,
        Math.PI, -Math.PI, Math.E, 10.0, 100.0
    };

    static void Trig(XRng r, List<double[]> o, int n, bool unit)
    {
        foreach (double s in Specials) o.Add(new[] { s });
        for (int k = -16; k <= 16; k++) { o.Add(new[] { k * (Math.PI / 2) }); o.Add(new[] { F32(k * (Math.PI / 2)) }); }   // exact multiples of pi/2 as doubles / as floats
        o.Add(new[] { (double)(float)Math.PI });
        while (o.Count < n)
        {
            int m = r.Pick(10);
            double x;
            if (m < 4) x = r.Range(-10, 10);
            else if (m < 6) x = F32(r.Range(-10, 10));                  // float-valued arguments, as the game passes (float) values
            else if (m < 7) x = r.Range(-1e6, 1e6);
            else if (m < 8) x = r.Range(-100, 100);
            else if (m < 9) x = r.Range(-1, 1);
            else x = r.Range(-2 * Math.PI, 2 * Math.PI);
            if (unit) x = Math.Max(-1.0, Math.Min(1.0, x / (m < 6 ? 10.0 : 1.0)));    // asin/acos domain
            o.Add(new[] { x });
        }
    }
    static void Sqr(XRng r, List<double[]> o, int n)
    {
        foreach (double s in Specials) o.Add(new[] { s });
        while (o.Count < n)
        {
            int m = r.Pick(8);
            double x;
            if (m < 3) x = r.Range(0, 1e6);
            else if (m < 4) x = F32(r.Range(0, 1e6));
            else if (m < 5) x = r.Range(0, 1);
            else if (m < 6) x = r.Range(0, 1e12);
            else if (m < 7) x = F32(r.Range(0, 1e4) * r.Range(0, 1e4));  // float product
            else x = r.Range(0, 1e3);
            o.Add(new[] { x });
        }
    }
    static void Lg(XRng r, List<double[]> o, int n)
    {
        foreach (double s in Specials) o.Add(new[] { s });
        while (o.Count < n)
        {
            int m = r.Pick(6);
            double x;
            if (m < 2) x = r.Range(0, 1e6);
            else if (m < 3) x = r.Range(0, 1);
            else if (m < 4) x = F32(r.Range(0, 1e4));
            else if (m < 5) x = r.Range(0, 100);
            else x = Math.Exp(r.Range(-700, 700));
            o.Add(new[] { x });
        }
    }
    static void Ex(XRng r, List<double[]> o, int n)
    {
        foreach (double s in Specials) o.Add(new[] { s });
        foreach (double s in new[] { 709.0, 709.78, 710.0, -708.0, -745.0, -746.0, 88.0, -88.0, 700.0 }) o.Add(new[] { s });
        while (o.Count < n)
        {
            int m = r.Pick(5);
            double x;
            if (m < 2) x = r.Range(-5, 5);
            else if (m < 3) x = r.Range(-50, 50);
            else if (m < 4) x = F32(r.Range(-20, 20));
            else x = r.Range(-700, 700);
            o.Add(new[] { x });
        }
    }
    static void At(XRng r, List<double[]> o, int n)
    {
        foreach (double s in Specials) o.Add(new[] { s });
        while (o.Count < n)
        {
            int m = r.Pick(6);
            double x;
            if (m < 2) x = r.Range(-10, 10);
            else if (m < 3) x = F32(r.Range(-10, 10));
            else if (m < 4) x = r.Range(-1e6, 1e6);
            else if (m < 5) x = r.Range(-1, 1);
            else x = r.Range(-1000, 1000);
            o.Add(new[] { x });
        }
    }
    static void At2(XRng r, List<double[]> o, int n)
    {
        double[] z = { 0.0, -0.0, 1.0, -1.0, double.PositiveInfinity, double.NegativeInfinity, double.NaN };
        foreach (double y in z) foreach (double x in z) o.Add(new[] { y, x });
        while (o.Count < n)
        {
            int m = r.Pick(6);
            double y, x;
            if (m < 2) { y = r.Range(-1000, 1000); x = r.Range(-1000, 1000); }
            else if (m < 4) { y = F32(r.Range(-1000, 1000)); x = F32(r.Range(-1000, 1000)); }
            else if (m < 5) { y = r.Range(-1, 1); x = r.Range(-1, 1); }
            else { y = r.Range(-1e6, 1e6); x = r.Range(-1e6, 1e6); }
            o.Add(new[] { y, x });
        }
    }
    static void Pw(XRng r, List<double[]> o, int n)
    {
        double[] ys = { 0.5, 2.0, 3.0, 1.5, 1.0 / 3.0, F32(1.0 / 3.0), 0.25, 1.25, 0.75, 4.0, 10.0, -1.0, -0.5, -2.0, 0.0, -0.0, 1.0, 0.1, 7.0, 0.3 };
        double[] xs = { 0.0, -0.0, 1.0, -1.0, 2.0, 10.0, 0.5, double.PositiveInfinity, double.NegativeInfinity, double.NaN, -8.0, 100.0, 1e-300, 1e300, 0.99999999999999989 };
        foreach (double x in xs) foreach (double y in ys) o.Add(new[] { x, y });
        foreach (double y in new[] { double.PositiveInfinity, double.NegativeInfinity, double.NaN, 1e300, -1e300, 0.5, 2.0 }) foreach (double x in new[] { 0.5, 1.0, 2.0, -2.0, 0.0 }) o.Add(new[] { x, y });
        while (o.Count < n)
        {
            int m = r.Pick(8);
            double x, y;
            if (m < 3) { x = r.Range(0, 100); y = ys[r.Pick(ys.Length)]; }
            else if (m < 4) { x = F32(r.Range(0, 100)); y = ys[r.Pick(5)]; }
            else if (m < 5) { x = r.Range(0, 100); y = r.Range(-5, 5); }
            else if (m < 6) { x = r.Range(0, 1); y = r.Range(-5, 5); }
            else if (m < 7) { x = r.Range(-20, 20); y = (double)r.Pick(9) - 4; }   // negative bases, integer exponents
            else { x = r.Range(0, 1e4); y = ys[r.Pick(8)]; }
            o.Add(new[] { x, y });
        }
    }
    static void Rnd(XRng r, List<double[]> o, int n)
    {
        foreach (double s in Specials) o.Add(new[] { s });
        for (int k = -8; k <= 8; k++) { o.Add(new[] { k + 0.5 }); o.Add(new[] { k + 0.49999999999999994 }); o.Add(new[] { k - 0.5 }); }
        while (o.Count < n)
        {
            int m = r.Pick(6);
            double x;
            if (m < 2) x = r.Range(-1e6, 1e6);
            else if (m < 3) x = Math.Floor(r.Range(-1000, 1000)) + 0.5;                // exact halves
            else if (m < 4) x = F32(r.Range(-5000, 5000));
            else if (m < 5) x = r.Range(-10, 10);
            else x = r.Range(-1e15, 1e15);
            o.Add(new[] { x });
        }
    }

    static void WriteX87(string outdir, string name, List<double[]> recs, int nargs, Func<double[], double> f)
    {
        using (var bw = new BinaryWriter(new FileStream(Path.Combine(outdir, name + ".bin"), FileMode.Create)))
        {
            bw.Write(new byte[] { (byte)'X', (byte)'8', (byte)'7', (byte)'T' });
            bw.Write(1u); bw.Write((uint)nargs); bw.Write((uint)recs.Count); bw.Write(0u);
            double[] res = new double[1];
            foreach (double[] a in recs)
            {
                double[] cell = res;
                cell[0] = f(a);                           // stored through a double[] element: rounded to 53 bits like any store in the game
                foreach (double v in a) bw.Write(BitConverter.DoubleToInt64Bits(v));
                bw.Write(BitConverter.DoubleToInt64Bits(cell[0]));
            }
        }
        Console.WriteLine("x87 " + name + ": " + recs.Count + " records");
    }

    static int MainX87(string[] args)
    {
        string outdir = Dir(Path.GetFullPath(args[1]));
        const int N = 20000;
        XRng r = new XRng();
        List<double[]> l;
        l = new List<double[]>(); Trig(r, l, N, false); WriteX87(outdir, "Sin", l, 1, a => Math.Sin(a[0]));
        l = new List<double[]>(); Trig(r, l, N, false); WriteX87(outdir, "Cos", l, 1, a => Math.Cos(a[0]));
        l = new List<double[]>(); Trig(r, l, N, false); WriteX87(outdir, "Tan", l, 1, a => Math.Tan(a[0]));
        l = new List<double[]>(); Trig(r, l, N, true); WriteX87(outdir, "Asin", l, 1, a => Math.Asin(a[0]));
        l = new List<double[]>(); Trig(r, l, N, true); WriteX87(outdir, "Acos", l, 1, a => Math.Acos(a[0]));
        l = new List<double[]>(); At(r, l, N); WriteX87(outdir, "Atan", l, 1, a => Math.Atan(a[0]));
        l = new List<double[]>(); At2(r, l, N); WriteX87(outdir, "Atan2", l, 2, a => Math.Atan2(a[0], a[1]));
        l = new List<double[]>(); Sqr(r, l, N); WriteX87(outdir, "Sqrt", l, 1, a => Math.Sqrt(a[0]));
        l = new List<double[]>(); Pw(r, l, N); WriteX87(outdir, "Pow", l, 2, a => Math.Pow(a[0], a[1]));
        l = new List<double[]>(); Ex(r, l, N); WriteX87(outdir, "Exp", l, 1, a => Math.Exp(a[0]));
        l = new List<double[]>(); Lg(r, l, N); WriteX87(outdir, "Log", l, 1, a => Math.Log(a[0]));
        l = new List<double[]>(); Lg(r, l, N); WriteX87(outdir, "Log10", l, 1, a => Math.Log10(a[0]));
        l = new List<double[]>(); Rnd(r, l, N); WriteX87(outdir, "Floor", l, 1, a => Math.Floor(a[0]));
        l = new List<double[]>(); Rnd(r, l, N); WriteX87(outdir, "Ceiling", l, 1, a => Math.Ceiling(a[0]));
        l = new List<double[]>(); Rnd(r, l, N); WriteX87(outdir, "Round", l, 1, a => Math.Round(a[0]));
        l = new List<double[]>(); Rnd(r, l, N); WriteX87(outdir, "RoundAway", l, 1, a => Math.Round(a[0], MidpointRounding.AwayFromZero));
        l = new List<double[]>(); Rnd(r, l, N); WriteX87(outdir, "Abs", l, 1, a => Math.Abs(a[0]));

        uint cw = _controlfp(0, 0);
        double sinPi = Math.Sin(Math.PI);
        var sb = new StringBuilder("{\n");
        sb.Append("\"intptr_size\":" + IntPtr.Size + ",\n");
        sb.Append("\"clr_version\":" + Q(Environment.Version.ToString()) + ",\n");
        sb.Append("\"os_64bit\":" + (Environment.Is64BitOperatingSystem ? "true" : "false") + ",\n");
        sb.Append("\"controlfp_word\":" + cw + ",\"controlfp_hex\":\"0x" + cw.ToString("x") + "\",\n");
        sb.Append("\"controlfp_precision_bits_0x30000\":" + (cw & 0x30000) + ",\"note_precision\":\"_controlfp encoding: 0x00000 = 64-bit mantissa, 0x10000 = 53-bit, 0x20000 = 24-bit\",\n");
        sb.Append("\"sin_pi_bits\":\"0x" + BitConverter.DoubleToInt64Bits(sinPi).ToString("x16") + "\",\"sin_pi\":" + Num(sinPi) + ",\n");
        sb.Append("\"sin_pi_correctly_rounded_libm\":1.2246467991473532e-16,\"sin_pi_is_x87_fsin_signature\":" + (sinPi.ToString("R", Inv) != "1.2246467991473532E-16" ? "true" : "false") + ",\n");
        sb.Append("\"sin_1e22\":" + Num(Math.Sin(1e22)) + ",\"sin_1e22_correctly_rounded_libm\":-0.8522008497671888,\n");
        sb.Append("\"cos_pi_over_2\":" + Num(Math.Cos(Math.PI / 2)) + ",\"cos_pi_over_2_correctly_rounded_libm\":6.123233995736766e-17,\n");
        sb.Append("\"count_per_function\":" + N + "\n}\n");
        File.WriteAllBytes(Path.Combine(outdir, "x87_env.json"), new UTF8Encoding(false).GetBytes(sb.ToString()));
        Console.WriteLine(sb.ToString());
        return 0;
    }
}
