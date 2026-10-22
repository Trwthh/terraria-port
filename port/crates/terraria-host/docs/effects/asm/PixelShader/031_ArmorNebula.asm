// PixelShader.Technique1[31] ArmorNebula
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor           c4  default (0, 0, 0, 0)
//   sampler2D uImage0          s0
//   sampler2D uImage1          s1
//   float     uSaturation      c6  default (0, 0, 0, 0)
//   float3    uSecondaryColor  c5  default (0, 0, 0, 0)
//   float     uTime            c7  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize0  c1  default (0, 0, 0, 0)
//   float4 uSourceRect  c0  default (0, 0, 0, 0)
//
//     rcp r0.x, c1.x
//     rcp r0.y, c1.y
//     add r1.xy, r0.xy, r0.xy
//     rcp oc0.x, r1.x
//     rcp oc0.y, r1.y
//     mov oc1.xy, r1.xy
//     mul oc2.xy, r0.xy, c0.xy
//     mul r1.xy, r0.xy, c0.zw
//     rcp oc3.x, r1.x
//     rcp oc3.y, r1.y
//
    ps_2_0
    def c8, 0.33333334, 0.1, -0.4, 5
    def c9, 0, 1, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    texld r0, t0, s0
    mul r1.xy, t0, c0
    frc r1.zw, r1.wzyx
    add r1.xy, r1, -r1.wzyx
    mov r2.xy, c1
    mad r1.xy, r1, r2, -c2
    mov r2.y, c3.y
    mad r1.y, r1.y, r2.y, c7.x
    mul r1.x, r1.x, c3.x
    add r1.x, r1.x, r1.x
    mul r2.y, r1.y, c8.y
    add r1.y, r0.y, r0.x
    add r1.y, r0.z, r1.y
    mul r1.y, r1.y, c8.x
    mad r2.x, r1.y, c1.x, r1.x
    texld r2, r2, s1
    add r1.x, r2.x, c8.z
    mul r1.z, r1.y, r1.x
    mad r2.xyz, c4, r1.y, -r0
    mad r0.xyz, c6.x, r2, r0
    mul r1.y, r1.z, c8.w
    mul r1.yzw, r1.y, c5.wzyx
    cmp r1.xyz, r1.x, r1.wzyx, c9.x
    add r1.xyz, r0, r1
    mov r1.w, c9.y
    mul r0, r0.w, r1
    mul r0, r0, v0
    mov oC0, r0

// 28 instructions
