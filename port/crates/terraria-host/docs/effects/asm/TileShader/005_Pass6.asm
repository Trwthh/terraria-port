// TileShader.Technique1[5] Pass6
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D s0  s0
//
    ps_2_0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    max r1.x, r0.x, r0.y
    min r1.y, r0.y, r0.x
    add r1.w, -r0.z, r1.x
    mov r1.z, r0.z
    cmp r2.x, r1.w, r1.x, r1.z
    cmp r2.y, r1.w, r1.z, r1.x
    min r0.xz, r2.y, r1.y
    mov r0.y, r2.x
    mul r0, r0, v0
    mov oC0, r0

// 11 instructions
