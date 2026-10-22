// PixelShader.Technique1[44] ArmorFlow
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor   c0  default (0, 0, 0, 0)
//   sampler2D uImage0  s0
//   float     uTime    c1  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uColor           c0  default (0, 0, 0, 0)
//   float3 uSecondaryColor  c1  default (0, 0, 0, 0)
//
//     neg r0.xyz, c0.xyz
//     add oc2.xyz, r0.xyz, c1.xyz
//
    ps_2_0
    def c3, 0.33333334, 4, 0.15915494, 0.5
    def c4, 6.2831855, -3.1415927, 0, 0
    def c5, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c6, -0.020833334, -0.125, 1, 0.5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    add r1.w, r0.y, r0.x
    add r1.x, r0.z, r1.w
    mov r1.y, c3.y
    mad r1.y, r1.x, r1.y, c1.x
    mul r1.x, r1.x, c3.x
    mad r1.y, r1.y, c3.z, c3.w
    frc r1.y, r1.y
    mad r1.y, r1.y, c4.x, c4.y
    sincos r2.y, r1.y, c5, c6
    mad r1.y, r2.y, c3.w, c3.w
    mov r2.xyz, c2
    mad r1.yzw, r1.y, r2.wzyx, c0.wzyx
    mul r0.xyz, r1.x, r1.wzyx
    mul r0, r0, v0
    mov oC0, r0

// 16 instructions
