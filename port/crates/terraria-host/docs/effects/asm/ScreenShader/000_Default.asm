// ScreenShader.Technique1[0] Default
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0  s0
//
    ps_2_0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mul r0, r0, v0
    mov oC0, r0

// 3 instructions
