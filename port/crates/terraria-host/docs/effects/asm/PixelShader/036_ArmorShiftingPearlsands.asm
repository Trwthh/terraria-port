// PixelShader.Technique1[36] ArmorShiftingPearlsands
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
//     mul oc3.x, c0.x, (-0.1)
//     rcp oc2.x, c2.x
//     rcp oc2.y, c2.y
//
    ps_2_0
    def c8, 1, 0.4, 0.85, 0
    def c9, 10, 0.15915494, 0.5, 0.013333334
    def c10, 6.2831855, -3.1415927, 0.06666667, 0.8
    def c11, 0.02, 0.18333334, 0.3, -0.3
    def c12, 2.5, -2, 3, 0.13333334
    def c13, -0.7, 0.85, 0.15, -0.2
    def c14, 33.333332, 0.15, 0.1, -0.5
    def c15, 0.7, 0.75, 0, 0
    def c16, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c17, -0.020833334, -0.125, 1, 0.5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    texld r0, t0, s0
    mov r1.xy, c7
    mad r1.z, t0.y, r1.y, -c6.y
    mul r1.z, r1.z, c0.x
    mov r1.w, c9.x
    mad r1.z, r1.z, r1.w, c1.x
    mad r1.z, r1.z, c9.y, c9.z
    frc r1.z, r1.z
    mad r1.z, r1.z, c10.x, c10.y
    sincos r2.y, r1.z, c16, c17
    mad r1.z, r2.y, c9.z, c9.z
    add r1.w, r0.y, r0.x
    add r1.w, r0.z, r1.w
    mul r0.x, r1.w, c9.w
    mad r0.x, r1.z, c11.x, r0.x
    mad r0.yz, t0.zxyw, r1.zxyw, -c6.zxyw
    add r0.yz, r0, r0
    mad r0.x, r0.y, c2.x, r0.x
    mov r1.y, c2.y
    mad r0.y, r0.z, r1.y, c3.x
    mul r1.xy, r0, c10.w
    texld r2, r0, s1
    texld r3, r1, s1
    mul r3.w, r1.w, c11.y
    mad r3.w, r2.x, c11.z, r3.w
    add r3.w, r3.w, c11.w
    mul_sat r3.w, r3.w, c12.x
    mad r0.x, r3.w, c12.y, c12.z
    mul r3.w, r3.w, r3.w
    mul r3.w, r0.x, r3.w
    mul r0.xyz, v0, c13.y
    mad r0.xyz, v0.w, c13.z, r0
    mad r1.x, r1.w, c10.z, c10.w
    mad r1.y, r1.w, c12.w, r3.x
    add r1.y, r1.y, c13.x
    mul_sat r1.y, r1.y, c9.x
    mul r2.xyz, r1.x, c5
    mul r2.xyz, r2, v0
    mad r0.xyz, c4, r0, -r2
    mad r0.xyz, r3.w, r0, r2
    add r1.x, r3.y, c13.w
    mad r1.z, r3.z, c15.x, c15.y
    mul_sat r1.x, r1.x, c14.x
    mad r1.w, r1.x, c12.y, c12.z
    mul r1.x, r1.x, r1.x
    mul r1.x, r1.w, r1.x
    mov r2.yzw, c14
    mad r2.xyz, r1.x, r2.wzyx, c8
    mul r2.xyz, r1.z, r2
    mul r3.xyz, v0, -c13.x
    mad r3.xyz, v0.w, c11.z, r3
    mad r2.xyz, r2, r3, -r0
    mad r2.w, r1.y, c12.y, c12.z
    mul r1.x, r1.y, r1.y
    mul r2.w, r2.w, r1.x
    mad r1.xyz, r2.w, r2, r0
    mov r1.w, v0.w
    mul r0, r0.w, r1
    mov oC0, r0

// 59 instructions
