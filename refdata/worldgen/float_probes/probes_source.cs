    class Holder { public float F; }
    static float SF;
    static Holder H = new Holder();
    [MethodImpl(MethodImplOptions.NoInlining)] static float Id(float f) { return f; }

    // p01: product-sum stored in a float local, then reused
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P01_mul_add_store_sub(float x, float y, float z, int n) { float a = x * y + z; return a - x; }
    // p02: product-sum returned directly (no store before the return)
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P02_mul_add_return(float x, float y, float z, int n) { return x * y + z; }
    // p03: compound assignments on float locals
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P03_compound_locals(float x, float y, float z, int n) { float a = x; a += y * z; a *= x; a -= y; return a; }
    // p04: compound assignments on a static float field
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P04_compound_static_field(float x, float y, float z, int n) { SF = x; SF += y * z; SF *= x; SF -= y; return SF; }
    // p05: compound assignments on an instance float field
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P05_compound_instance_field(float x, float y, float z, int n) { H.F = x; H.F += y * z; H.F *= x; H.F -= y; return H.F; }
    // p06: truncating cast of a float product (position * velocity style)
    [MethodImpl(MethodImplOptions.NoInlining)] public static int P06_int_cast_mul(float x, float y, float z, int n) { return (int)(x * y); }
    // p07: truncating cast of a product-sum
    [MethodImpl(MethodImplOptions.NoInlining)] public static int P07_int_cast_mul_add(float x, float y, float z, int n) { return (int)(x * y + z); }
    // p08: a stored product compared with the same expression recomputed
    [MethodImpl(MethodImplOptions.NoInlining)] public static int P08_cmp_store_vs_expr(float x, float y, float z, int n) { float p = x * y; return p == x * y ? 1 : 0; }
    // p09: comparisons of products and sums (bit0: x*y > z*z, bit1: x*y == y*x, bit2: (x+y)*z == x*z+y*z, bit3: x*y + z == z + y*x)
    [MethodImpl(MethodImplOptions.NoInlining)] public static int P09_cmp_products(float x, float y, float z, int n) { return (x * y > z * z ? 1 : 0) | (x * y == y * x ? 2 : 0) | ((x + y) * z == x * z + y * z ? 4 : 0) | (x * y + z == z + y * x ? 8 : 0); }
    // p10: float accumulator in a loop, trip count from n
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P10_loop_accum(float x, float y, float z, int n) { float s = 0f; for (int i = 0; i < (n & 31) + 1; i++) s += x * 0.1f; return s; }
    // p11: float accumulator with a multiply-add recurrence (damping)
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P11_loop_recurrence(float x, float y, float z, int n) { float s = 1f; for (int i = 0; i < 8; i++) s = s * 0.97f + x * 0.5f - z * 0.01f; return s; }
    // p12: Vector2 fields updated in place (real XNA Vector2)
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P12_vector2_fields(float x, float y, float z, int n) { Vector2 v = new Vector2(x, y); v.X += v.Y * z; v.Y = v.X * z - v.Y; return v.X + v.Y; }
    // p13: Vector2 operators (+, - and * float)
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P13_vector2_ops(float x, float y, float z, int n) { Vector2 a = new Vector2(x, y), b = new Vector2(z, x); Vector2 c = a + b * 0.5f - a * z; return c.X + c.Y * 2f; }
    // p14: Vector2.Length (float)Math.Sqrt(X*X + Y*Y)
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P14_vector2_length(float x, float y, float z, int n) { return new Vector2(x, y).Length(); }
    // p15: Vector2.Normalize
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P15_vector2_normalize(float x, float y, float z, int n) { Vector2 v = new Vector2(x, y); v.Normalize(); return v.X + v.Y * 3f; }
    // p16: a float expression passed through a method call and added to
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P16_call_arg(float x, float y, float z, int n) { return Id(x * y) + z; }
    // p17: float product widened to double by assignment, then used
    [MethodImpl(MethodImplOptions.NoInlining)] public static double P17_double_from_float_product(float x, float y, float z, int n) { double d = x * y; return d * z; }
    // p18: whole float expression assigned to a double
    [MethodImpl(MethodImplOptions.NoInlining)] public static double P18_double_from_float_expr(float x, float y, float z, int n) { double d = x * y + z; return d; }
    // p19: Math.Sqrt of a float product, cast back to float
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P19_sqrt_float_product(float x, float y, float z, int n) { float r = (float)Math.Sqrt(Math.Abs(x * y)); return r; }
    // p20: Math.Sqrt of a sum of float squares
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P20_sqrt_sum_squares(float x, float y, float z, int n) { return (float)Math.Sqrt(x * x + y * y); }
    // p21: mixed int / float arithmetic with integer division and modulo
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P21_mixed_int_float(float x, float y, float z, int n) { return n / 7 * x + (float)n / 3 * y + n % 5 / 2f; }
    // p22: product chain
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P22_product_chain(float x, float y, float z, int n) { return x * y * z; }
    // p23: associativity residue (a nonzero result shows that sums are rounded to float at each step or not)
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P23_assoc_residue(float x, float y, float z, int n) { return (x + y) + z - (x + (y + z)); }
    // p24: tile coordinate from a pixel coordinate (two casts)
    [MethodImpl(MethodImplOptions.NoInlining)] public static int P24_tile_coord(float x, float y, float z, int n) { return (int)(x * 0.0625f) + (int)((x + y) / 16f) * 1000; }
    // p25: trigonometry cast to float, then multiplied
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P25_trig_cast(float x, float y, float z, int n) { return (float)Math.Cos(x) * y + (float)Math.Sin(x) * z; }
    // p26: Math.Atan2 cast to float
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P26_atan2_cast(float x, float y, float z, int n) { return (float)Math.Atan2(y, x) * z; }
    // p27: Math.Max / Math.Min (float overloads: the arguments are rounded to float at the call)
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P27_minmax(float x, float y, float z, int n) { return Math.Max(x * y, z * z) - Math.Min(x * y, z); }
    // p28: division by a float constant and multiplication by its reciprocal
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P28_div_const(float x, float y, float z, int n) { return x / 3f * y + x * (1f / 3f) * z; }
    // p29: int to float conversions in an expression
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P29_int_to_float(float x, float y, float z, int n) { return (float)n * x + y - n; }
    // p30: Math.Pow with a float base and exponent 1.5 (cast back to float)
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P30_pow_cast(float x, float y, float z, int n) { return (float)Math.Pow(Math.Abs(x), 1.5) * y; }
    [MethodImpl(MethodImplOptions.NoInlining)] static void Touch(ref float f) { f = f; }
    // p31: ten float locals, all still live at the end (register pressure: some must live in memory)
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P31_ten_locals(float x, float y, float z, int n) { float a0 = x * y; float a1 = a0 + z; float a2 = a1 * x; float a3 = a2 - y; float a4 = a3 * z; float a5 = a4 + a0; float a6 = a5 * a1; float a7 = a6 - a2; float a8 = a7 + a3 * 0.5f; float a9 = a8 * a4 + a5; return a9 + a0 + a1 + a2 + a3 + a4 + a5 + a6 + a7 + a8; }
    // p32: a float local that is live across a method call
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P32_local_across_call(float x, float y, float z, int n) { float a = x * y + z; float b = Id(x); return a - b; }
    // p33: a float accumulator with a method call in every loop iteration
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P33_loop_with_call(float x, float y, float z, int n) { float s = 0f; for (int i = 0; i < (n & 7) + 1; i++) { s += x * 0.1f; Id(1f); } return s; }
    // p34: a float local passed by ref (address taken)
    [MethodImpl(MethodImplOptions.NoInlining)] public static float P34_local_address_taken(float x, float y, float z, int n) { float a = x * y + z; Touch(ref a); return a - x; }
    
