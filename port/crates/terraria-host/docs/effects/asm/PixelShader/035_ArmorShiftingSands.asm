// PixelShader.Technique1[35] ArmorShiftingSands
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
//     mul oc3.x, c0.x, (-0.2)
//     rcp oc2.x, c2.x
//     rcp oc2.y, c2.y
//
    ps_2_0
    def c8, 10, 0.15915494, 0.5, 0.013333334
    def c9, 6.2831855, -3.1415927, 0.06666667, 0.8
    def c10, 0.04, 0.18333334, 0.3, -0.3
    def c11, 2.5, -2, 3, 0.13333334
    def c12, 0.85, 0.15, 0.7, 0
    def c13, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c14, -0.020833334, -0.125, 1, 0.5
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
    sincos r2.y, r1.z, c13, c14
    mad r1.z, r2.y, c8.z, c8.z
    add r1.w, r0.y, r0.x
    add r1.w, r0.z, r1.w
    mul r0.x, r1.w, c8.w
    mad r0.x, r1.z, c10.x, r0.x
    mad r0.yz, t0.zxyw, r1.zxyw, -c6.zxyw
    add r0.yz, r0, r0
    mad r0.x, r0.y, c2.x, r0.x
    mov r1.y, c2.y
    mad r0.y, r0.z, r1.y, c3.x
    texld r2, r0, s1
    mul r0.x, r1.w, c10.y
    mad r0.x, r2.x, c10.z, r0.x
    mad r0.y, r1.w, c11.w, r2.x
    mad r0.z, r1.w, c9.z, c9.w
    mul r1.xyz, r0.z, c5
    mul r1.xyz, r1, v0
    add r1.w, r0.y, -c9.w
    mul_sat r1.w, r1.w, c8.x
    add r0.x, r0.x, c10.w
    mul_sat r0.x, r0.x, c11.x
    mad r0.y, r0.x, c11.y, c11.z
    mul r0.x, r0.x, r0.x
    mul r0.x, r0.y, r0.x
    mul r2.xyz, v0, c12.x
    mad r2.xyz, v0.w, c12.y, r2
    mad r2.xyz, c4, r2, -r1
    mad r0.xyz, r0.x, r2, r1
    mad r1.x, r1.w, c11.y, c11.z
    mul r1.y, r1.w, r1.w
    mul r1.x, r1.x, r1.y
    mul r1.yzw, v0.wzyx, c12.z
    mad r1.yzw, v0.w, c10.z, r1
    lrp r2.xyz, r1.x, r1.wzyx, r0
    mov r2.w, v0.w
    mul r0, r0.w, r2
    mov oC0, r0

// 47 instructions
