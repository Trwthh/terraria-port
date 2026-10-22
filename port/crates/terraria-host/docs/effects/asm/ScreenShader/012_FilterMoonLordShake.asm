// ScreenShader.Technique1[12] FilterMoonLordShake
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0     s0
//   float     uIntensity  c0  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float uIntensity  c1  default (0, 0, 0, 0)
//   float uTime       c0  default (0, 0, 0, 0)
//
//     mul r0.x, c0.x, (3.183098861837907)
//     add r1.x, r0.x, (0.5)
//     frc r0.x, r1.x
//     mul r1.x, r0.x, (6.283185307179586)
//     add r0.x, r1.x, (-3.141592653589793)
//     sin r1.y, r0.x
//     mul r0.x, r1.y, c1.x
//     mul oc1.x, r0.x, (0.01)
//
    ps_2_0
    def c2, -0.5, 0, 1, 0.15
    def c3, 0.7, 1.3, 1, 0
    dcl t0.xy
    dcl_2d s0
    add r0.xy, t0, c2.x
    dp2add r0.x, r0, r0, c2.y
    rsq r0.x, r0.x
    rcp r0.x, r0.x
    mov r0.z, c2.z
    mad r0.y, c0.x, -r0.x, r0.z
    mul r0.x, r0.x, c0.x
    mad r0.x, r0.x, -c3.y, c3.z
    mul r1.x, r0.y, c1.x
    mov r1.y, c2.y
    add r2.xy, r1, t0
    add r1.xy, -r1, t0
    texld r1, r1, s0
    texld r2, r2, s0
    texld r3, t0, s0
    mul r2, r2, c2.w
    mad r2, r3, c3.x, r2
    mad r1, r1, c2.w, r2
    mul r0, r0.x, r1
    mov oC0, r0

// 20 instructions
