// PixelShader.Technique1[23] ArmorBasicColor
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor    c0  default (0, 0, 0, 0)
//   sampler2D uImage0   s0
//   float     uOpacity  c1  default (0, 0, 0, 0)
//
    ps_2_0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mul r0.xyz, r0, c0
    mul r0, r0, v0
    mul r0, r0, c1.x
    mov oC0, r0

// 5 instructions
