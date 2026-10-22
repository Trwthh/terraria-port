// TileShader.Technique1[44] Pass45
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D s0  s0
//
    ps_2_0
    def c0, 6, 2, 4, 0.16666667
    def c1, 1, 0, 0, 0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    min r1.w, r0.z, r0.y
    min r2.w, r1.w, r0.x
    max r1.x, r0.y, r0.z
    max r3.z, r0.x, r1.x
    add r1.x, -r2.w, r3.z
    rcp r1.y, r1.x
    add r2.xyz, -r0.zxyw, r3.z
    mul r4.xyz, r1.y, r2
    mad r2.xyz, r2, r1.y, -r4.zxyw
    add r1.yzw, r2.wzyx, c0.wzyx
    add r0.xyz, r0, -r3.z
    mov r3.w, r0.w
    cmp r0.xyz, r0, c1.x, c1.y
    mul r0.xyz, r1.wzyx, r0
    max r1.y, r0.z, r0.x
    max r2.x, r0.y, r1.y
    mul r0.x, r2.x, c0.w
    frc r0.x, r0.x
    rcp r0.z, r3.z
    mul r0.y, r1.x, r0.z
    mul r0.z, r1.x, r1.x
    cmp r3.xy, -r0.z, c1.y, r0
    mov oC0, r3

// 24 instructions
