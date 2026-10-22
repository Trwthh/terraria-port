// PixelShader.Technique1[47] ForceField
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor   c4  default (0, 0, 0, 0)
//   sampler2D uImage0  s0
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize0  c2  default (0, 0, 0, 0)
//   float4 uSourceRect  c1  default (0, 0, 0, 0)
//   float  uTime        c0  default (0, 0, 0, 0)
//
//     rcp r0.x, c2.x
//     rcp r0.y, c2.y
//     mul oc0.xy, r0.xy, c1.xy
//     mul r1.xy, r0.xy, c1.zw
//     rcp oc1.x, r1.x
//     rcp oc1.y, r1.y
//     mul oc2.xy, (0.5), r1.xy
//     mul oc3.x, c0.x, (5)
//
    ps_2_0
    def c5, -0.5, 1.5, 1, 0
    def c6, 2.8, -1, 20, 30
    def c7, -0.3, 0.75, 0, 0
    def c8, 12.566368, -6.283184, 1.9999996, 0.5
    def c9, -2, 3, 0.07957749, -0.0000014980282
    def c10, 6.2831855, -3.1415927, -0.5, 0.5
    def c11, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c12, -0.020833334, -0.125, 1, 0.5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    add r0.xy, t0, -c0
    mov r1.w, c5.x
    mad r0.xy, r0, c1, r1.w
    mov r0.w, c6.w
    mad r0.z, r0.y, r0.w, c3.x
    mul r0.xy, r0, c5.yzxw
    dp2add r0.x, r0, r0, c5.w
    rsq r0.x, r0.x
    rcp r0.x, r0.x
    mul r0.y, r0.z, c9.z
    abs r0.y, r0.y
    frc r0.y, r0.y
    cmp r0.y, r0.z, r0.y, -r0.y
    mad r0.z, r0.y, c8.z, c8.w
    mad r0.y, r0.y, c8.x, c8.y
    frc r0.z, r0.z
    mad r0.z, r0.z, c10.x, c10.y
    cmp r0.y, r0.y, c9.w, r0.z
    sincos r1.x, r0.y, c11, c12
    mad r0.y, r1.x, c10.z, c10.w
    mul r0.z, r0.x, c6.x
    mad r0.x, r0.x, c6.x, c6.y
    mul_sat r0.x, r0.x, c6.z
    rsq r0.w, r0.z
    rcp r0.w, r0.w
    add r1.xy, t0, -c2
    mad r1.xy, r1, r0.w, -c5.x
    texld r1, r1, s0
    mad r0.w, r1.x, r0.y, c7.x
    mul_sat r0.w, r0.w, c6.z
    mad r1.y, r0.w, c9.x, c9.y
    mul r0.w, r0.w, r0.w
    mul r0.w, r1.y, r0.w
    rcp r1.y, r1.x
    mul r1.x, r1.x, r1.x
    lrp r2.w, r0.w, r1.y, c5.z
    mul r0.w, r1.x, r2.w
    mad r1.x, r0.x, c9.x, c9.y
    mul r0.x, r0.x, r0.x
    mad r0.x, r1.x, -r0.x, c5.z
    mul r0.y, r0.y, r0.x
    mul r1.w, r0.w, r0.y
    mul r1.xyz, r1.w, c4
    mul r0.y, r0.z, r0.z
    mul r0.x, r0.x, r0.y
    mul r0.w, r0.z, r0.x
    mul r0.xyz, r0.w, c4
    mad r0, r0, c7.y, r1
    mul r0, r0, v0
    mov oC0, r0

// 50 instructions
