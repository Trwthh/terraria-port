// PixelShader.Technique1[25] ArmorTwilight
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor       c3  default (0, 0, 0, 0)
//   sampler2D uImage0      s0
//   sampler2D uImage1      s1
//   float2    uImageSize0  c5  default (0, 0, 0, 0)
//   float4    uSourceRect  c4  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float  uDirection       c2  default (0, 0, 0, 0)
//   float2 uImageSize1      c3  default (0, 0, 0, 0)
//   float4 uSourceRect      c0  default (0, 0, 0, 0)
//   float2 uTargetPosition  c1  default (0, 0, 0, 0)
//
//     rcp oc0.x, c3.x
//     rcp oc0.y, c3.y
//     mul r0.x, c1.x, c2.x
//     rcp r0.y, c0.z
//     mul r1.x, r0.x, r0.y
//     rcp r0.x, c0.w
//     mul r1.y, r0.x, c1.y
//     mul oc1.xy, (0.004), r1.xy
//     mul oc2.xy, (0.02), r1.xy
//
    ps_2_0
    def c6, 0.125, -0.6, 5, 20
    def c7, 0.25, -0.7, 0.35, 0.05
    def c8, -2, 3, -0.4, 2.5
    def c9, 0.033333335, 1, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    mov r0.xy, c5
    mad r0.xy, t0, r0, -c4
    mul r0.zw, r0.wzyx, c0.wzyx
    mov r1.xy, c0
    mad r0.xy, r0, r1, c2
    mov r1.x, c6.x
    mad r1.xy, r0.wzyx, r1.x, c1
    texld r0, r0, s1
    texld r1, r1, s1
    texld r2, t0, s0
    add r0.x, r0.x, c6.y
    mul_sat r0.x, r0.x, c6.z
    mad r0.w, r0.x, c8.x, c8.y
    mul r0.x, r0.x, r0.x
    mul r0.x, r0.w, r0.x
    mul r0.x, r0.x, c6.w
    mad r0.x, r1.x, c7.z, r0.x
    add r0.x, r0.x, c7.w
    add r0.y, r0.y, c8.z
    add r0.z, r0.z, c7.y
    mul_sat r0.z, r0.z, c6.z
    mul_sat r0.y, r0.y, c8.w
    mad r0.w, r0.y, c8.x, c8.y
    mul r0.y, r0.y, r0.y
    mul r0.y, r0.w, r0.y
    mul r0.y, r0.y, c7.x
    mad r1.xyz, r0.x, c3, r0.y
    mad r1.w, r0.z, c8.x, c8.y
    mul r0.x, r0.z, r0.z
    mul r1.w, r1.w, r0.x
    mad r0.xyz, r1.w, -c8.x, r1
    add r0.w, r2.y, r2.x
    add r0.w, r2.z, r0.w
    mad r0.xyz, r0.w, c9.x, r0
    mov r0.w, c9.y
    mul r0, r2.w, r0
    mul r0, r0, v0.w
    mov oC0, r0

// 38 instructions
