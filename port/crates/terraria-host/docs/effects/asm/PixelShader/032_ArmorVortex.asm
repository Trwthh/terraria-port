// PixelShader.Technique1[32] ArmorVortex
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor           c4  default (0, 0, 0, 0)
//   sampler2D uImage0          s0
//   sampler2D uImage1          s1
//   float3    uSecondaryColor  c5  default (0, 0, 0, 0)
//   float     uTime            c6  default (0, 0, 0, 0)
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
    def c7, -0.5, 0.0208351, -0.085133, 0.180141
    def c8, -0.3302995, 0.999866, 0, 1
    def c9, -2, 1.5707964, -0, -3.1415927
    def c10, 0.1, -0.1, 0.33333334, 5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    mul r0.xy, t0, c0
    frc r0.zw, r0.wzyx
    add r0.xy, r0, -r0.wzyx
    mov r1.xy, c1
    mad r0.xy, r0, r1, -c2
    mov r1.x, c7.x
    mad r0.xy, r0, c3, r1.x
    abs r0.z, r0.x
    abs r0.w, r0.y
    max r1.x, r0.z, r0.w
    rcp r1.x, r1.x
    min r1.y, r0.w, r0.z
    add r0.z, -r0.z, r0.w
    cmp r0.z, r0.z, c8.z, c8.w
    mul r0.w, r1.x, r1.y
    mul r1.x, r0.w, r0.w
    mad r1.y, r1.x, c7.y, c7.z
    mad r1.y, r1.x, r1.y, c7.w
    mad r1.y, r1.x, r1.y, c8.x
    mad r1.x, r1.x, r1.y, c8.y
    mul r0.w, r0.w, r1.x
    mad r1.x, r0.w, c9.x, c9.y
    mad r0.z, r1.x, r0.z, r0.w
    cmp r0.w, r0.y, c9.z, c9.w
    add r0.z, r0.z, r0.w
    add r0.w, r0.z, r0.z
    max r1.x, r0.x, r0.y
    cmp r1.x, r1.x, c8.w, c8.z
    min r1.y, r0.y, r0.x
    cmp r1.x, r1.y, c8.z, r1.x
    mad r1.x, r1.x, -r0.w, r0.z
    mul r0.y, r0.y, r0.y
    mad r0.x, r0.x, r0.x, r0.y
    rsq r0.x, r0.x
    rcp r0.x, r0.x
    add r1.y, r0.x, c6.x
    add r0.x, r0.x, c10.x
    min r1.z, r0.x, c8.w
    mul r0.xy, r1, c10.x
    texld r0, r0, s1
    texld r2, t0, s0
    mad r0.x, r0.x, r1.z, c10.y
    add r0.y, r2.y, r2.x
    add r0.y, r2.z, r0.y
    mul r0.y, r0.y, c10.z
    mul r0.z, r0.x, r0.y
    mul r0.z, r0.z, c10.w
    mul r1.xyz, r0.z, c5
    cmp r1.xyz, r0.x, r1, c8.z
    mad r0.xyz, c4, r0.y, r1
    mov r0.w, c8.w
    mul r0, r2.w, r0
    mul r0, r0, v0
    mov oC0, r0

// 54 instructions
