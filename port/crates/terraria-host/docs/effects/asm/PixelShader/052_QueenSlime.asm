// PixelShader.Technique1[52] QueenSlime
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0      s0
//   sampler2D uImage1      s1
//   sampler2D uImage2      s2
//   float2    uImageSize0  c7  default (0, 0, 0, 0)
//   float4    uSourceRect  c6  default (0, 0, 0, 0)
//   float     uTime        c5  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float4 uSourceRect  c1  default (0, 0, 0, 0)
//   float  uTime        c0  default (0, 0, 0, 0)
//
//     rcp oc0.x, c1.z
//     rcp oc1.x, c1.w
//     mul r0.x, c0.x, (0.15915494309189535)
//     add r1.x, r0.x, (0.5)
//     frc r0.x, r1.x
//     mul r1.x, r0.x, (6.283185307179586)
//     add r0.x, r1.x, (-3.141592653589793)
//     cos r1.x, r0.x
//     sin r1.y, r0.x
//     mul r0.x, r1.x, (0.5)
//     mul r0.y, r1.y, (0.3)
//     mul r0.z, c0.x, (0.2)
//     add oc4.x, r0.x, r0.z
//     add r1.x, r0.y, r0.z
//     neg r0.x, r1.x
//     add oc3.x, r0.x, (1)
//
    ps_2_0
    def c8, 1, -1, 0, 0
    def c9, 0.4, 0.3, 0.2, 1
    def c10, -2, 3, 0.24191551, 0.5
    def c11, 6.2831855, -3.1415927, 0.3771972, 0.5
    def c12, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c13, -0.020833334, -0.125, 1, 0.5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    dcl_2d s2
    mov r0.xy, c7
    mad r0.y, t0.y, r0.y, -c6.y
    mul_sat r0.y, r0.y, c1.x
    mad r0.z, r0.y, c10.x, c10.y
    mul r0.y, r0.y, r0.y
    mul r0.w, r0.z, r0.y
    mad r0.w, r0.w, c11.z, c11.w
    frc r0.w, r0.w
    mad r0.w, r0.w, c11.x, c11.y
    sincos r1.y, r0.w, c12, c13
    mov r2.xy, c9
    mad r0.w, r1.y, r2.x, c3.x
    mad r1.y, r0.z, r0.y, r0.w
    mad r0.x, t0.x, r0.x, -c6.x
    mul_sat r0.x, r0.x, c0.x
    mad r0.y, r0.x, c10.x, c10.y
    mul r0.x, r0.x, r0.x
    mul r0.x, r0.y, r0.x
    mad r0.z, r0.x, c10.z, c10.w
    frc r0.z, r0.z
    mad r0.z, r0.z, c11.x, c11.y
    sincos r3.x, r0.z, c12, c13
    mad r1.x, r3.x, r2.y, c4.x
    mov r0.y, c9.z
    add r0.xy, r1, r0
    mul r0.xy, r0, c10.w
    texld r0, r0, s2
    texld r1, t0, s0
    add r0.y, r1.x, c5.x
    add r0.x, -r0.x, r0.y
    add r0.x, r0.x, c9.w
    cmp r0.y, r0.x, c8.x, c8.y
    mul r0.x, r0.x, r0.y
    frc r0.x, r0.x
    mul r0.x, r0.y, r0.x
    mov r0.y, c8.z
    texld r0, r0, s1
    mov r2.w, c9.w
    mov r2.xyz, r1.x
    add r3.xyz, r0, -r2.z
    add r3.w, r0.w, -r2.w
    mad r0, r1.y, r3, r2
    mul r0.w, r1.w, r0.w
    mul r0, r0, v0
    mov oC0, r0

// 45 instructions
