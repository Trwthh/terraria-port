// PixelShader.Technique1[14] ArmorInvert
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0  s0
//
    ps_2_0
    def c0, 1, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mov r1.w, c0.x
    add r1.xyz, -r0, c0.x
    mul r0, r0.w, r1
    mul r0, r0, v0
    mov oC0, r0

// 6 instructions
