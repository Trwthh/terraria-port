// PixelShader.Technique1[62] LensFlare
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0  s0
//   sampler2D uImage1  s1
//
    ps_2_0
    def c0, 0.5, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    texld r0, t0, s0
    mul r0, r0, v0
    mov r1.xy, c0.x
    texld r1, r1, s1
    mul r0, r0, r1.w
    mov oC0, r0

// 6 instructions
