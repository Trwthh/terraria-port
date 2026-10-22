// PixelShader.Technique1[45] ArmorForceField
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor       c3  default (0, 0, 0, 0)
//   sampler2D uImage0      s0
//   float2    uImageSize0  c5  default (0, 0, 0, 0)
//   float4    uSourceRect  c4  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize0  c1  default (0, 0, 0, 0)
//   float  uTime        c0  default (0, 0, 0, 0)
//
//     rcp r0.x, c1.y
//     add oc1.x, r0.x, r0.x
//     mul oc0.x, r0.x, (4)
//     mul oc2.x, c0.x, (10)
//
    ps_2_0
    def c6, 0.15915494, 0.5, 6.2831855, -3.1415927
    def c7, 1, 0.33333334, 2.5, 0.16666667
    def c8, 0, 0, 0, 0
    def c9, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c10, -0.020833334, -0.125, 1, 0.5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    mov r0.x, t0.x
    add r0.y, t0.y, c1.x
    mov r1.x, t0.x
    add r1.y, t0.y, c0.x
    texld r0, r0, s0
    texld r1, r1, s0
    texld r2, t0, s0
    mov r0.xy, c5
    mad r0.xy, t0, r0, -c4
    add r0.z, r0.y, -c2.x
    mad r0.z, r0.z, c6.x, c6.y
    frc r0.z, r0.z
    mad r0.z, r0.z, c6.z, c6.w
    sincos r3.y, r0.z, c9, c10
    mul r0.z, r0.w, r3.y
    add r0.y, r0.y, c2.x
    mad r0.x, r0.x, c6.x, c6.y
    frc r0.x, r0.x
    mad r0.x, r0.x, c6.z, c6.w
    sincos r3.y, r0.x, c9, c10
    mad r0.x, r0.y, c6.x, c6.y
    frc r0.x, r0.x
    mad r0.x, r0.x, c6.z, c6.w
    sincos r4.y, r0.x, c9, c10
    mad r0.x, r3.y, r4.y, r0.z
    min r3.w, r0.x, c7.x
    mul r3.xyz, r3.w, c3
    mul r0, r3, c7.z
    min r3, r0, c7.x
    add r0.x, r2.y, r2.x
    add r0.x, r2.z, r0.x
    mad r0.y, r0.x, -c7.w, r1.w
    mul r0.x, r0.x, c7.y
    mul r1.xyz, r0.x, c3
    mul r3, r3, r0.y
    cmp_sat r0, r0.y, r3, c8.x
    mov r1.w, c7.x
    mul r1, r2.w, r1
    mul r1, r1, v0
    mad r0, r0, c6.y, r1
    mov oC0, r0

// 41 instructions
