// ScreenShader.Technique1[19] WaterDebugDraw
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0  s0
//
    ps_2_0
    def c0, -0.5, 0.4, 1, 0
    def c1, 2, -2, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    add r0.x, r0.x, c0.x
    mad r0.y, r0.z, -c0.y, c0.z
    mul r0.x, r0.x, r0.y
    mul r0.x, r0.x, v0.w
    mul r0.xyz, r0.x, c1
    abs r0.w, r0.x
    mov oC0, r0

// 8 instructions
