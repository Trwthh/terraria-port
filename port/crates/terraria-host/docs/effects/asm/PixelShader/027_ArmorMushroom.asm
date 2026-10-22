// PixelShader.Technique1[27] ArmorMushroom
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor   c0  default (0, 0, 0, 0)
//   sampler2D uImage0  s0
//
    ps_2_0
    def c1, 0.33333334, -0.3, 1.6666666, 1
    def c2, -2, 3, 0.25, 0
    def c3, 0.5, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    add r1.w, r0.y, r0.x
    add r1.x, r0.z, r1.w
    mad r1.x, r1.x, c1.x, c1.y
    mul_sat r1.y, r1.x, c1.z
    mad r1.z, r1.y, c2.x, c2.y
    mul r1.y, r1.y, r1.y
    mad r1.y, r1.z, -r1.y, c1.w
    mul r0.xyz, r0, c2.z
    mad r2.xyz, c0, r1.y, -r0
    mad r0.xyz, r1.y, r2, r0
    cmp r1.xyz, r1.x, r0, c2.w
    mov r1.w, c1.w
    mul r0, r0.w, r1
    min r1, r0, c1.w
    max r0.xyz, v0, v0.w
    lrp r2.xyz, c3.x, r0, v0
    mov r2.w, v0.w
    mul r0, r1, r2
    mov oC0, r0

// 20 instructions
