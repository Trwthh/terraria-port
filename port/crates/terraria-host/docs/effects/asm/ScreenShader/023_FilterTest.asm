// ScreenShader.Technique1[23] FilterTest
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0  s0
//
    ps_2_0
    def c0, -0.5, 0.2, 0.5, 0
    dcl t0.xy
    dcl_2d s0
    add r0.xy, t0, c0.x
    mad r0.xy, r0, c0.y, c0.z
    texld r0, r0, s0
    mov oC0, r0

// 4 instructions
