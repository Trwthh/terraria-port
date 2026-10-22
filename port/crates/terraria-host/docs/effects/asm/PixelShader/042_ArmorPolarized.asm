// PixelShader.Technique1[42] ArmorPolarized
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0  s0
//
    ps_2_0
    def c0, 0.33333334, 0.6, 0.16666667, 0.5
    def c1, 1, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    add r0.x, r0.y, r0.x
    add r0.x, r0.z, r0.x
    mad r0.y, r0.x, -c0.x, c0.y
    mul r0.z, r0.x, c0.z
    mad r0.x, r0.x, c0.z, c0.w
    cmp r1.xyz, r0.y, r0.z, r0.x
    mov r1.w, c1.x
    mul r0, r0.w, r1
    mul r0, r0, v0
    mov oC0, r0

// 11 instructions
