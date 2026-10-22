// Integer and floating point semantics that differ between C# and naive Rust.
// Each class has `public static string Run()`; the result must be identical in C# and in the translation.

public static class IntOverflow
{
    static int Max() { return int.MaxValue; }
    static int Min() { return int.MinValue; }
    public static string Run()
    {
        int a = Max();
        int b = a + 1;                 // wraps to MinValue
        int c = a * 2;                 // wraps to -2
        int d = -Min();                // stays MinValue
        int e = Min() - 1;             // wraps to MaxValue
        uint u = 0;
        u = u - 1;                     // 4294967295
        long l = a;
        l = l * l;                     // no wrap in long
        long m = long.MaxValue;
        m = m + 1;
        int h = 17;
        for (int i = 0; i < 40; i++) h = h * 31 + i;   // hash-style accumulation
        return b + ";" + c + ";" + d + ";" + e + ";" + u + ";" + l + ";" + m + ";" + h;
    }
}

public static class NarrowingCasts
{
    public static string Run()
    {
        int big = 300, neg = -1, wide = 70000, twoHundred = 200;
        long l = 0x1_2345_6789L;
        byte b = (byte)big;            // 44
        sbyte sb = (sbyte)twoHundred;  // -56
        short s = (short)wide;         // 4464
        ushort us = (ushort)neg;       // 65535
        int i = (int)l;                // 0x23456789
        uint ui = (uint)neg;           // 4294967295
        char ch = (char)(65 + 1);      // 'B'
        int fromChar = ch + 1;         // 67
        long back = (long)ui;          // 4294967295
        ulong ul = (ulong)neg;         // 18446744073709551615
        return b + ";" + sb + ";" + s + ";" + us + ";" + i + ";" + ui + ";" + ch + ";" + fromChar + ";" + back + ";" + ul;
    }
}

public static class FloatToInt
{
    public static string Run()
    {
        float f = 2.9f, g = -2.9f, h = 0.999999f;
        double d = 1234567.89, big = 1e10, nan = double.NaN;
        float fbig = 1e10f;
        int a = (int)f;                // 2: truncates toward zero
        int b = (int)g;                // -2
        int c = (int)h;                // 0
        int e = (int)d;                // 1234567
        int o1 = (int)big;             // x86 and x64: int.MinValue
        int o2 = (int)nan;             // int.MinValue
        int o3 = (int)fbig;            // int.MinValue
        long l = (long)d;
        short s = (short)f;
        byte y = (byte)f;
        float back = (float)a + 0.5f;  // 2.5
        double w = (double)f;          // widening is exact; then * 0.6 in double
        int k = (int)((double)13 * 0.6);   // 7  (Item.SetDefaults: useTime * 0.6)
        int k2 = (int)((double)15 * 0.6);  // 9
        return a + ";" + b + ";" + c + ";" + e + ";" + o1 + ";" + o2 + ";" + o3 + ";" + l + ";" + s + ";" + y + ";" + back + ";" + (w > 2.8999 && w < 2.9001) + ";" + k + ";" + k2;
    }
}

public static class CompoundNarrowing
{
    public static string Run()
    {
        byte b = 250;
        b += 10;                       // 4: (byte)(b + 10)
        byte b2 = 5;
        b2 -= 10;                      // 251
        sbyte sb = 127;
        sb++;                          // -128
        short s = 32767;
        s += 1;                        // -32768
        ushort us = 0;
        us--;                          // 65535
        byte m = 16;
        m *= 20;                       // 64
        byte sh = 0x81;
        sh <<= 1;                      // 2
        int i = 10;
        i /= 3;                        // 3
        i %= 2;                        // 1
        float f = 1f;
        f += 1;                        // 2
        f *= 1.5f;                     // 3
        int fromFloat = 7;
        fromFloat = (int)(fromFloat * 0.5f);   // 3
        char c = 'a';
        c += (char)2;                  // 'c'
        return b + ";" + b2 + ";" + sb + ";" + s + ";" + us + ";" + m + ";" + sh + ";" + i + ";" + f + ";" + fromFloat + ";" + c;
    }
}

public static class DivMod
{
    static int Zero() { return 0; }
    public static string Run()
    {
        int a = -7, b = 2;
        string r = (a / b) + ";" + (a % b) + ";" + (7 / -2) + ";" + (7 % -2) + ";" + (-7 / -2) + ";" + (-7 % -2);
        float f = -7.5f;
        r += ";" + (f % 2f) + ";" + (7.5f % -2f);
        uint u = 7;
        r += ";" + (u / 2) + ";" + (u % 4);
        long l = -9000000000L;
        r += ";" + (l / 7) + ";" + (l % 7);
        try
        {
            int z = 10 / Zero();
            r += ";no exception " + z;
        }
        catch (System.DivideByZeroException)
        {
            r += ";div by zero";
        }
        r += ";" + (1 << 33) + ";" + (-16 >> 2) + ";" + ((uint)0x80000000 >> 31) + ";" + (1L << 40) + ";" + (5 & 3) + ";" + (5 | 3) + ";" + (5 ^ 3) + ";" + (~5);
        return r;
    }
}

// The game's runtime formats float with 7 significant digits and double with 15; .NET Core 3.0+ prints the
// shortest round-trip form instead. The stated result is the .NET Framework one.
// framework-expect NumberFormatting: 1;2.5;-0.25;0.3333333;0.6666667;1E+10;1.234568E+07;0.1;0.3;0.333333333333333;100;1E-05;123456.8;True;False;c;7
public static class NumberFormatting
{
    public static string Run()
    {
        float one = 1f, third = 1f / 3f, big = 1e10f;
        double dthird = 1.0 / 3.0;
        bool t = true;
        char c = 'c';
        byte seven = 7;
        return one + ";" + 2.5f + ";" + (-0.25f) + ";" + third + ";" + (2f / 3f) + ";" + big + ";" + 12345678f + ";" + 0.1 + ";" + (0.1 + 0.2) + ";" + dthird + ";" + 100.0 + ";" + 0.00001f + ";" + 123456.789f + ";" + t + ";" + (!t) + ";" + c + ";" + seven;
    }
}
