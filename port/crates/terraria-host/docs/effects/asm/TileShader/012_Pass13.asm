// TileShader.Technique1[12] Pass13
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D s0  s0
//
    ps_2_0
    def c0, 0.5, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    max r1.x, r0.x, r0.y
    min r1.y, r0.y, r0.x
    add r1.w, -r0.z, r1.x
    mov r1.z, r0.z
    cmp r0.x, r1.w, r1.x, r1.z
    cmp r0.y, r1.w, r1.z, r1.x
    min r2.w, r0.y, r1.y
    add r1.x, r0.x, r2.w
    mov r0.y, r2.w
    mul r0.z, r1.x, v0.z
    mov r1.xyw, v0
    mov r1.z, c0.x
    mul r0, r0, r1
    mov oC0, r0

// 15 instructions
