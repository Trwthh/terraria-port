// PixelShader.Technique1[7] ArmorBrightnessColored
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor   c0  default (0, 0, 0, 0)
//   sampler2D uImage0  s0
//
    ps_2_0
    def c1, 0.33333334, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    add r1.w, r0.y, r0.x
    add r1.x, r0.z, r1.w
    mul r1.x, r1.x, c1.x
    mul r1.xyz, r1.x, c0
    mul r0.xyz, r0.w, r1
    mul r0, r0, v0
    mov oC0, r0

// 8 instructions
