// TileShader.Technique1[42] Pass43
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D s0  s0
//
    ps_2_0
    def c0, 1, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    max r1.w, r0.x, r0.y
    max r2.w, r1.w, r0.z
    add r1.x, -r0.x, c0.x
    cmp r0.x, -r2.w, r0.x, r1.x
    add r1.x, -r0.y, c0.x
    cmp r0.y, -r2.w, r0.y, r1.x
    add r1.x, -r0.z, c0.x
    cmp r0.z, -r2.w, r0.z, r1.x
    mul r0, r0, v0
    mov oC0, r0

// 11 instructions
