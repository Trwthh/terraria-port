// PixelShader.Technique1[37] ArmorFog
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor           c4  default (0, 0, 0, 0)
//   sampler2D uImage0          s0
//   sampler2D uImage1          s1
//   float2    uImageSize0      c7  default (0, 0, 0, 0)
//   float3    uSecondaryColor  c5  default (0, 0, 0, 0)
//   float4    uSourceRect      c6  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize1  c2  default (0, 0, 0, 0)
//   float4 uSourceRect  c1  default (0, 0, 0, 0)
//   float  uTime        c0  default (0, 0, 0, 0)
//
//     rcp oc0.x, c1.w
//     mul oc1.x, c0.x, (2.5)
//     mul oc3.x, c0.x, (0.04)
//     rcp oc2.x, c2.x
//     rcp oc2.y, c2.y
//
    ps_2_0
    def c8, 10, 0.15915494, 0.5, 0.013333334
    def c9, 6.2831855, -3.1415927, 0.06666667, 0.8
    def c10, 0.004, 1.8, 0.18333334, 0.3
    def c11, -0.1, 1.25, -2, 3
    def c12, -0.2, 1.6666666, 0.85, 0.15
    def c13, 0.9, 0, 0, 0
    def c14, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c15, -0.020833334, -0.125, 1, 0.5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    texld r0, t0, s0
    mov r1.xy, c7
    mad r1.z, t0.y, r1.y, -c6.y
    mul r1.z, r1.z, c0.x
    mov r1.w, c8.x
    mad r1.z, r1.z, r1.w, c1.x
    mad r1.z, r1.z, c8.y, c8.z
    frc r1.z, r1.z
    mad r1.z, r1.z, c9.x, c9.y
    sincos r2.y, r1.z, c14, c15
    mad r1.z, r2.y, c8.z, c8.z
    add r1.w, r0.y, r0.x
    add r1.w, r0.z, r1.w
    mul r2.x, r1.w, c8.w
    mad r1.z, r1.z, c10.x, r2.x
    mad r1.xy, t0, r1, -c6
    mul r1.xy, r1, c10.y
    mad r2.y, r1.y, c2.y, r1.z
    mov r3.w, c2.x
    mad r2.x, r1.x, r3.w, c3.x
    texld r2, r2, s1
    mul r1.x, r1.w, c10.z
    mad r1.y, r1.w, c9.z, c9.w
    mul r1.yzw, r1.y, c5.wzyx
    mul r1.yzw, r1, v0.wzyx
    mad r1.x, r2.x, c10.w, r1.x
    add r2.x, r1.x, c11.x
    add r1.x, r1.x, c12.x
    mul_sat r1.x, r1.x, c12.y
    mul_sat r2.x, r2.x, c11.y
    mad r2.y, r2.x, c11.z, c11.w
    mul r2.x, r2.x, r2.x
    mul r2.x, r2.y, r2.x
    mul r2.yzw, v0.wzyx, c12.z
    mad r2.yzw, v0.w, c12.w, r2
    mad r2.yzw, c4.wzyx, r2, -r1
    mad r1.yzw, r2.x, r2, r1
    mul r2.xyz, v0, c8.z
    mad r2.xyz, v0.w, c8.z, r2
    mul r3.xyz, r0, r2
    mul r3.xyz, r3, -c11.x
    mad r1.yzw, r1, c13.x, r3.wzyx
    mad r0.xyz, r0, r2, -r1.wzyx
    mad r2.x, r1.x, c11.z, c11.w
    mul r1.x, r1.x, r1.x
    mul r1.x, r2.x, r1.x
    mad r1.xyz, r1.x, r0, r1.wzyx
    mov r1.w, v0.w
    mul r0, r0.w, r1
    mov oC0, r0

// 50 instructions
