// FmtProbe: what .NET Framework 4 (x86, the game's runtime) prints for the number formats the game uses.
// C# 5. Build and run: tools\cs2rs\tests\framework\build.cmd  ->  fmt_probe.out (read by the unit tests of rt::fmt).
//
// One line per (value, format): kind TAB value TAB format TAB en-US output TAB invariant output.
// Floating-point values are written as the hexadecimal bit pattern so that the reader gets the same number.
using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Text;

static class FmtProbe
{
    static readonly CultureInfo En = new CultureInfo("en-US", false);
    static readonly CultureInfo Inv = CultureInfo.InvariantCulture;

    static string Esc(string s)
    {
        return s.Replace("\\", "\\\\").Replace("\t", "\\t").Replace("\n", "\\n");
    }

    static int Main(string[] args)
    {
        var sb = new StringBuilder();
        // every format string of the game (tools\cs2rs\DESIGN.md "Format strings") plus the neighbours that
        // exercise the same code paths
        string[] intFormats = { "", "D", "D2", "D4", "X", "X2", "X8", "x4", "N0", "N", "N2", "F2", "P0", "P1", "0", "00", "##", "0.0", "+0;-#", "#,0", "#,##0", "0.0%", "000.00", "G", "0;(0);zero", "'x'0", "\\#0" };
        string[] floatFormats = { "", "F0", "F1", "F2", "F3", "F4", "N0", "N1", "N2", "P", "P0", "P1", "P2", "0", "00", "0.0", "0.00", "0.0%", "##", "#.##", "0.###", "+0;-#", "#,0.0", "G", "R", "0;(0);zero", "0.0;-0.0", ".00", "0.", "#,#" };
        long[] ints = { 0, 1, -1, 5, 9, 10, 12, 99, 100, 255, 1000, 1234, 12345, 999999, 1000000, 1234567, -5, -12, -1234, -1234567, 65535, int.MaxValue, int.MinValue };
        double[] doubles = { 0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 1.5, 2.5, 0.05, 0.005, 0.015, 0.025, 0.125, 0.135, 0.145, 0.995, 0.9995, 1.005, 2.675, 1.45, 12.5, 99.95, 99.995, 100.0, 123.456, 1234.5, 1234.5678, 12345.6789, 0.001, 0.0001, 0.00001, 1e-7, -0.001, -0.004, -0.005, -1234.5678, 1e15, 1e16, 123456789012345678.0, 0.1, 0.2, 0.3, 0.7, 1.0 / 3.0, 2.0 / 3.0, 3.14159265358979, 0.999, 0.0999, 0.33, 0.666, 59.94, 60.0, 16.6667, 0.1234567890123456, 9.9999999999999995, double.NaN, double.PositiveInfinity, double.NegativeInfinity };
        float[] floats = { 0f, 1f, -1f, 0.5f, 1.5f, 2.5f, 0.05f, 0.125f, 0.15f, 0.25f, 0.35f, 0.1f, 0.2f, 0.3f, 0.7f, 1f / 3f, 2f / 3f, 0.995f, 1.005f, 2.675f, 12.5f, 99.95f, 123.456f, 1234.5f, 1234.5678f, 0.001f, -0.004f, -0.005f, 16777216f, 1e10f, 3.1415927f, 0.999f, 59.94f, 16.6667f, float.NaN, float.PositiveInfinity };
        foreach (long v in ints)
        {
            foreach (string f in intFormats)
            {
                if (v >= int.MinValue && v <= int.MaxValue) sb.Append("i32\t").Append(v).Append('\t').Append(f).Append('\t').Append(Esc(((int)v).ToString(f, En))).Append('\t').Append(Esc(((int)v).ToString(f, Inv))).Append('\n');
                sb.Append("i64\t").Append(v).Append('\t').Append(f).Append('\t').Append(Esc(v.ToString(f, En))).Append('\t').Append(Esc(v.ToString(f, Inv))).Append('\n');
                if (v >= 0 && v <= 255) sb.Append("u8\t").Append(v).Append('\t').Append(f).Append('\t').Append(Esc(((byte)v).ToString(f, En))).Append('\t').Append(Esc(((byte)v).ToString(f, Inv))).Append('\n');
                if (v >= short.MinValue && v <= short.MaxValue) sb.Append("i16\t").Append(v).Append('\t').Append(f).Append('\t').Append(Esc(((short)v).ToString(f, En))).Append('\t').Append(Esc(((short)v).ToString(f, Inv))).Append('\n');
                if (v >= 0) sb.Append("u32\t").Append(v).Append('\t').Append(f).Append('\t').Append(Esc(((uint)v).ToString(f, En))).Append('\t').Append(Esc(((uint)v).ToString(f, Inv))).Append('\n');
            }
        }
        foreach (double v in doubles)
            foreach (string f in floatFormats)
                sb.Append("f64\t").Append(BitConverter.DoubleToInt64Bits(v).ToString("X16")).Append('\t').Append(f).Append('\t').Append(Esc(v.ToString(f, En))).Append('\t').Append(Esc(v.ToString(f, Inv))).Append('\n');
        foreach (float v in floats)
            foreach (string f in floatFormats)
                sb.Append("f32\t").Append(BitConverter.ToInt32(BitConverter.GetBytes(v), 0).ToString("X8")).Append('\t').Append(f).Append('\t').Append(Esc(v.ToString(f, En))).Append('\t').Append(Esc(v.ToString(f, Inv))).Append('\n');
        // composite formatting: alignment, escapes, a format on a string argument, null
        var comp = new List<KeyValuePair<string, object[]>>();
        comp.Add(new KeyValuePair<string, object[]>("{0}", new object[] { 5 }));
        comp.Add(new KeyValuePair<string, object[]>("a{0}b{1}c{0}", new object[] { 1, "x" }));
        comp.Add(new KeyValuePair<string, object[]>("[{0,5}][{0,-5}][{1,2}][{1,-2}]", new object[] { 42, "abc" }));
        comp.Add(new KeyValuePair<string, object[]>("{{0}} {{{0}}} }}{{", new object[] { 7 }));
        comp.Add(new KeyValuePair<string, object[]>("{0:0.0%} {1,7:P2} {2,3:00} {3,5:F2}", new object[] { 0.256, 0.5, 7, 3.14159 }));
        comp.Add(new KeyValuePair<string, object[]>("{0:X2}{1:X2}{2:X2}", new object[] { (byte)255, (byte)0, (byte)17 }));
        comp.Add(new KeyValuePair<string, object[]>("{0:+0;-#} {1:+0;-#} {2:+0;-#}", new object[] { 3, -4, 0 }));
        comp.Add(new KeyValuePair<string, object[]>("<{0}> <{1}> <{2:F1}>", new object[] { null, "", "str" }));
        comp.Add(new KeyValuePair<string, object[]>("{0} {1} {2} {3}", new object[] { true, 'c', 1.5f, 2.25 }));
        comp.Add(new KeyValuePair<string, object[]>("{0 , 4 }|{1,-3 :D2}|", new object[] { 1, 2 }));
        comp.Add(new KeyValuePair<string, object[]>("{0:0.0}{1:0}", new object[] { 9.96, 0.5 }));
        foreach (var c in comp)
        {
            var kinds = new List<string>();
            foreach (object o in c.Value)
            {
                if (o == null) kinds.Add("null");
                else if (o is int) kinds.Add("i32:" + o);
                else if (o is byte) kinds.Add("u8:" + o);
                else if (o is string) kinds.Add("str:" + Esc((string)o));
                else if (o is bool) kinds.Add("bool:" + ((bool)o ? "1" : "0"));
                else if (o is char) kinds.Add("char:" + (int)(char)o);
                else if (o is float) kinds.Add("f32:" + BitConverter.ToInt32(BitConverter.GetBytes((float)o), 0).ToString("X8"));
                else if (o is double) kinds.Add("f64:" + BitConverter.DoubleToInt64Bits((double)o).ToString("X16"));
            }
            sb.Append("fmt\t").Append(string.Join(" ", kinds.ToArray())).Append('\t').Append(Esc(c.Key)).Append('\t').Append(Esc(string.Format(En, c.Key, c.Value))).Append('\t').Append(Esc(string.Format(Inv, c.Key, c.Value))).Append('\n');
        }
        // TimeSpan custom format used by the character list (UICharacterListItem.cs)
        long[] spans = { 0, 10000000L * 59, 10000000L * 3600, 10000000L * (3600 * 27 + 62), 10000000L * (86400 * 3 + 3600 * 5 + 61) };
        foreach (long t in spans)
        {
            TimeSpan ts = new TimeSpan(t);
            sb.Append("span\t").Append(t).Append('\t').Append("\\:mm\\:ss").Append('\t').Append(Esc(ts.ToString("\\:mm\\:ss"))).Append('\t').Append(Esc(ts.ToString())).Append('\n');
        }
        File.WriteAllBytes(args[0], new UTF8Encoding(false).GetBytes(sb.ToString()));
        Console.WriteLine("wrote " + args[0] + " (" + Environment.Version + ", " + (IntPtr.Size * 8) + "-bit)");
        return 0;
    }
}
