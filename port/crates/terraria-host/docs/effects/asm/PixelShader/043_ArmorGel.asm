// PixelShader.Technique1[43] ArmorGel
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor           c12  default (0, 0, 0, 0)
//   sampler2D uImage0          s0
//   sampler2D uImage1          s1
//   float2    uImageSize0      c15  default (0, 0, 0, 0)
//   float3    uSecondaryColor  c13  default (0, 0, 0, 0)
//   float4    uSourceRect      c14  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uColor           c0  default (0, 0, 0, 0)
//   float2 uImageSize0      c5  default (0, 0, 0, 0)
//   float  uRotation        c2  default (0, 0, 0, 0)
//   float3 uSecondaryColor  c1  default (0, 0, 0, 0)
//   float4 uSourceRect      c4  default (0, 0, 0, 0)
//   float  uTime            c3  default (0, 0, 0, 0)
//
//     add r0.xy, c4.zw, c4.xy
//     add r1.xy, (-1), r0.xy
//     rcp r0.x, c5.x
//     rcp r0.y, c5.y
//     mul oc7.xy, r1.xy, r0.xy
//     add r0.zw, (1), c4.xy
//     mul oc6.xy, r0.xy, r0.zw
//     add r1.xy, r0.xy, r0.xy
//     mul oc0.xy, (0.5), c4.zw
//     mul oc3.x, c3.x, (0.025)
//     neg r0.x, c2.x
//     add r1.z, r0.x, (-1.57075)
//     mul r0.x, r1.z, (0.15915494309189535)
//     add r1.z, r0.x, (0.5)
//     frc r0.x, r1.z
//     mul r1.z, r0.x, (6.283185307179586)
//     add r0.x, r1.z, (-3.141592653589793)
//     cos r2.x, r0.x
//     sin r2.y, r0.x
//     mul r0.xy, r1.xy, r2.xy
//     mov oc4.x, r1.x
//     mul oc5.xy, (3.5), r0.xy
//     mul oc8.xy, (3), r0.xy
//     add oc9.xy, r0.xy, r0.xy
//     mov oc10.xy, r0.xy
//     neg r0.xyz, c1.xyz
//     add oc11.xyz, r0.xyz, c0.xyz
//     mul r0.x, c2.x, (0.15915494309189535)
//     add r1.x, r0.x, (0.5)
//     frc r0.x, r1.x
//     mul r1.x, r0.x, (6.283185307179586)
//     add r0.x, r1.x, (-3.141592653589793)
//     cos r1.x, r0.x
//     sin r1.y, r0.x
//     mov oc1.x, r1.x
//     mov oc2.x, r1.y
//
    ps_2_0
    def c16, -0, 0.0025, 0.33333334, 4
    def c17, 0.5, 0.25, 1.5, 2
    def c18, 10, -2, 3, 0.75
    def c19, -0.45, 1, 0.7, -0.2
    def c20, 0.1875, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    texld r0, t0, s0
    add r1.xy, t0, c5
    max r2.xy, r1, c6
    min r1.xy, c7, r2
    add r1.zw, t0.wzyx, c8.wzyx
    max r2.xy, r1.wzyx, c6
    min r3.xy, c7, r2
    add r1.zw, t0.wzyx, c9.wzyx
    max r2.xy, r1.wzyx, c6
    min r4.xy, c7, r2
    add r1.zw, t0.wzyx, c10.wzyx
    max r2.xy, r1.wzyx, c6
    min r5.xy, c7, r2
    mov r2.xy, c15
    mad r1.zw, t0.wzyx, r2.wzyx, -c14.wzyx
    add r1.zw, r1, -c0.wzyx
    mul r2.x, r1.z, c2.x
    mad r2.x, r1.w, c1.x, -r2.x
    mul r1.z, r1.z, c1.x
    mad r2.y, r1.w, c2.x, r1.z
    add r1.zw, r2.wzyx, c0.wzyx
    mov r2.x, c16.x
    mov r2.y, -c3.x
    mad r1.zw, r1, c16.y, r2.wzyx
    mov r2.y, -c16.x
    add r2.z, r0.y, r0.x
    add r2.z, r0.z, r2.z
    mul r2.w, r2.z, c16.z
    mul r3.z, r2.w, c4.x
    mul r2.x, r3.z, c16.w
    add r2.xy, r1.wzyx, r2
    texld r1, r1, s0
    texld r3, r3, s0
    texld r4, r4, s0
    texld r5, r5, s0
    texld r6, r2, s1
    max r1.x, r5.w, r0.w
    max r2.x, r4.w, r1.x
    max r1.y, r3.w, r2.x
    max r2.y, r1.w, r1.y
    mul r1.y, r1.y, c17.x
    mad r1.y, r2.y, c17.y, r1.y
    mad r1.y, r2.x, c17.z, r1.y
    mad r1.x, r1.x, c17.w, r1.y
    mul r0.xyz, r2.w, c12
    mul r1.x, r6.x, r1.x
    mad r1.y, r1.x, c17.x, r0.w
    mul r1.x, r1.x, c17.x
    mad r1.x, r0.w, c17.x, r1.x
    add r1.x, r1.x, c19.w
    mul_sat r1.x, r1.x, c18.x
    add r1.y, r1.y, c19.x
    cmp r1.y, r1.y, c19.y, c19.z
    mul r1.y, r6.x, r1.y
    mov r3.xyz, c11
    mad r3.xyz, r1.y, r3, c13
    mad r1.y, r1.x, c18.y, c18.z
    mul r1.x, r1.x, r1.x
    mul r1.x, r1.y, r1.x
    mov r3.w, c19.y
    mul r1, r3, r1.x
    mul r1, r2.y, r1
    mul r2.x, r1.w, c18.w
    mad r2.x, r2.z, -c20.x, r2.x
    mad r1, r1, c18.w, -r0
    mad r3, r2.x, r1, r0
    mul r0.x, r2.x, r1.w
    cmp r3.w, r0.x, r3.w, r0.w
    mul r0, r3, v0
    mov oC0, r0

// 70 instructions
