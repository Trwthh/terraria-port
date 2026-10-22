// PixelShader.Technique1[56] MaskedFade
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0              s0
//   sampler2D uImage1              s1
//   float4    uShaderSpecificData  c0  default (0, 0, 0, 0)
//
    ps_2_0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    mov r0.x, t0.x
    mad r0.y, t0.y, c0.x, c0.y
    add r1.x, t0.x, c0.z
    add r1.y, t0.y, c0.w
    texld r0, r0, s1
    texld r1, r1, s0
    mul r1, r1, v0
    mul r0, r0.w, r1
    mov oC0, r0

// 9 instructions
