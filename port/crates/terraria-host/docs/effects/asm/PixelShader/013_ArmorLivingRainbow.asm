// PixelShader.Technique1[13] ArmorLivingRainbow
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0      s0
//   float2    uImageSize0  c3  default (0, 0, 0, 0)
//   float4    uSourceRect  c2  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float4 uSourceRect  c1  default (0, 0, 0, 0)
//   float  uTime        c0  default (0, 0, 0, 0)
//
//     rcp oc0.x, c1.z
//     mul oc1.x, c0.x, (0.8)
//
    ps_2_0
    def c4, 0.15, 0.333, 0.666, 0
    def c5, 0.5, -2, 3, 0.4
    def c6, 3, -1, 1, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mov r1.w, c3.x
    mad r1.x, t0.x, r1.w, -c2.x
    mul_sat r1.x, r1.x, c0.x
    mad r1.y, r1.x, c5.y, c5.z
    mul r1.x, r1.x, r1.x
    mul r1.x, r1.y, r1.x
    mov r1.w, c5.w
    mad r1.x, r1.x, r1.w, c1.x
    add r1.y, r0.y, r0.x
    add r1.y, r0.z, r1.y
    mad r1.x, r1.y, c4.x, r1.x
    mul r1.y, r1.y, c5.x
    add r2.xyz, r1.x, c4.wzyx
    abs r3.xyz, r2
    frc r3.xyz, r3
    cmp r2.xyz, r2, r3, -r3
    mad r2.xyz, r2, c6.x, c6.y
    abs r2.xyz, r2
    add r2.xyz, -r2, c6.z
    mul r1.xyz, r1.y, r2
    mul r0.xyz, r0.w, r1
    min r1, r0, c6.z
    max r0.xyz, v0, v0.w
    lrp r2.xyz, c5.w, r0, v0
    mov r2.w, v0.w
    mul r0, r1, r2
    mov oC0, r0

// 28 instructions
