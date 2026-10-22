// PixelShader.Technique1[34] ArmorVoid
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0  s0
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize0  c1  default (0, 0, 0, 0)
//   float  uTime        c0  default (0, 0, 0, 0)
//
//     mul r0.x, c0.x, (3.183098861837907)
//     add r1.x, r0.x, (0.5)
//     frc r0.x, r1.x
//     mul r1.x, r0.x, (6.283185307179586)
//     add r0.x, r1.x, (-3.141592653589793)
//     sin r1.y, r0.x
//     rcp r0.x, c1.x
//     add r1.x, r0.x, r0.x
//     mul r0.x, r1.y, r1.x
//     mul oc0.x, r0.x, (1.5)
//
    ps_2_0
    def c1, -0, 0.25, 0.5, 0.3
    def c2, 3, 0.35, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    add r0.x, t0.x, c0.x
    mov r0.y, t0.y
    mov r1.x, -c0.x
    mov r1.y, c1.x
    add r1.xy, r1, t0
    texld r0, r0, s0
    texld r2, t0, s0
    texld r1, r1, s0
    mul r0, r0, c1.y
    mad r0, r2, c1.z, r0
    mad r0, r1, c1.y, r0
    add r1.xyz, r0, c1.w
    mul r1.w, r0.z, c2.x
    mul r1.xyz, r1, r1.w
    mul r0.xyz, r1, v0
    mov r1.xyz, c2.y
    mov r1.w, v0.w
    mul r0, r0, r1
    mov oC0, r0

// 19 instructions
