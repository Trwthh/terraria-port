// PixelShader.Technique1[39] ArmorMirage
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0      s0
//   float2    uImageSize0  c4  default (0, 0, 0, 0)
//   float4    uSourceRect  c3  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize0  c2  default (0, 0, 0, 0)
//   float4 uSourceRect  c1  default (0, 0, 0, 0)
//   float  uTime        c0  default (0, 0, 0, 0)
//
//     rcp r0.x, c2.x
//     add oc2.x, r0.x, r0.x
//     rcp oc0.x, c1.w
//     mul oc1.x, c0.x, (5)
//
    ps_2_0
    def c5, -0.5, 30, 0.15915494, 0.5
    def c6, 6.2831855, -3.1415927, 0, 1
    def c7, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c8, -0.020833334, -0.125, 1, 0.5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    mov r0.w, c4.y
    mad r0.x, t0.y, r0.w, -c3.y
    mov r1.xy, c5
    mad r0.x, r0.x, c0.x, r1.x
    mad r0.x, r0.x, r1.y, c1.x
    mad r0.x, r0.x, c5.z, c5.w
    frc r0.x, r0.x
    mad r0.x, r0.x, c6.x, c6.y
    sincos r1.y, r0.x, c7, c8
    mul r0.x, r1.y, c2.x
    mov r1.x, -r0.x
    mov r1.y, c6.z
    add r1.xy, r1, t0
    mov r0.y, c6.z
    add r0.xy, r0, t0
    texld r1, r1, s0
    texld r0, r0, s0
    texld r2, t0, s0
    mov r0.x, r1.x
    mov r0.w, c6.w
    mov r0.y, r2.y
    mul r0, r2.w, r0
    mul r0, r0, v0
    mov oC0, r0

// 24 instructions
