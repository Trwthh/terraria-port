// ScreenShader.Technique1[3] FilterInvert
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0   s0
//   float     uOpacity  c0  default (0, 0, 0, 0)
//
    ps_2_0
    def c1, -1, -1, -1, 1
    def c2, 1, 1, 1, 0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mov r1, c1
    mad r1, r0, r1, c2
    lrp r2, c0.x, r1, r0
    mov oC0, r2

// 5 instructions
