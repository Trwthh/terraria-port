// PixelShader.Technique1[38] ArmorHallowBoss
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0  s0
//   sampler2D uImage1  s1
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float uTime  c0  default (0, 0, 0, 0)
//
//     mul oc0.x, c0.x, (0.5)
//
    ps_2_0
    def c1, 0.5, 0.8, 0.2, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    texld r0, t0, s0
    max r1.w, r0.y, r0.z
    max r2.w, r0.x, r1.w
    min r1.x, r0.z, r0.y
    min r2.x, r1.x, r0.x
    add r1.x, r2.w, r2.x
    mov r2.x, c1.x
    mad r1.x, r1.x, r2.x, c0.x
    abs r1.y, r1.x
    frc r1.y, r1.y
    cmp r1.x, r1.x, r1.y, -r1.y
    mov r1.y, c1.x
    texld r1, r1, s1
    mul r1, r1, c1.y
    mad r1, r0, c1.z, r1
    mul r1, r1, v0
    mul r0, r0.w, r1
    mov oC0, r0

// 18 instructions
