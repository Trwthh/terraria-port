// ScreenShader.Technique1[5] FilterGraveyard
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0     s0
//   float     uIntensity  c1  default (0, 0, 0, 0)
//   float     uOpacity    c0  default (0, 0, 0, 0)
//   float     uProgress   c2  default (0, 0, 0, 0)
//
    ps_2_0
    def c3, 0.33333334, 0, 0, 0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    add r1.w, r0.y, r0.x
    add r1.x, r0.z, r1.w
    mul r1.x, r1.x, c1.x
    mad r1.xyz, r1.x, c3.x, -r0
    mad r1.xyz, c2.x, r1, r0
    mad r1.xyz, r1, r0.w, -r0
    mad r0.xyz, c0.x, r1, r0
    mov oC0, r0

// 9 instructions
