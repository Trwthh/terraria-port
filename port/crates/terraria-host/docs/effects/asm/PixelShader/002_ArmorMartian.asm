// PixelShader.Technique1[2] ArmorMartian
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0  s0
//
    ps_2_0
    def c0, 0, 2, 3, 0.5
    def c1, 1, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    add r1.w, r0.y, r0.x
    add r1.x, r0.z, r1.w
    max r1.y, r0.y, r0.z
    max r2.w, r0.x, r1.y
    mul r0.w, r0.w, v0.w
    add r1.x, r1.x, -r2.w
    mul r1.x, r1.x, v0.w
    mul r1.y, r1.x, c0.w
    mad r1.x, r1.x, -c0.w, r2.w
    mul r2.xyz, r0.w, c0
    mad r1.yzw, r1.x, r2.wzyx, r1.y
    add r1.x, -r1.x, c1.x
    add r2.xyz, -v0, c1.x
    mad r2.xyz, r2, -r1.x, c1.x
    mul r0.xyz, r1.wzyx, r2
    mov oC0, r0

// 17 instructions
