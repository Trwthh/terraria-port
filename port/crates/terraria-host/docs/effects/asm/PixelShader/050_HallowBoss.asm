// PixelShader.Technique1[50] HallowBoss
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0  s0
//   sampler2D uImage1  s1
//   float     uTime    c0  default (0, 0, 0, 0)
//
    ps_2_0
    def c1, 0.5, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    texld r0, t0, s0
    add r0.x, r0.x, c0.x
    abs r0.y, r0.x
    frc r0.y, r0.y
    cmp r0.x, r0.x, r0.y, -r0.y
    mov r0.y, c1.x
    texld r1, r0, s1
    mul r1, r1, v0
    mul r0, r0.w, r1
    mov oC0, r0

// 10 instructions
