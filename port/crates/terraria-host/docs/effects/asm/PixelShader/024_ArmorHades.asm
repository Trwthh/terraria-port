// PixelShader.Technique1[24] ArmorHades
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor           c8  default (0, 0, 0, 0)
//   sampler2D uImage0          s0
//   float2    uImageSize0      c11  default (0, 0, 0, 0)
//   float3    uSecondaryColor  c9  default (0, 0, 0, 0)
//   float4    uSourceRect      c10  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize0  c3  default (0, 0, 0, 0)
//   float  uRotation    c0  default (0, 0, 0, 0)
//   float4 uSourceRect  c2  default (0, 0, 0, 0)
//   float  uTime        c1  default (0, 0, 0, 0)
//
//     add r0.x, c2.w, c2.y
//     rcp r0.y, c3.y
//     mul oc2.x, r0.x, r0.y
//     mul oc3.x, r0.y, c2.y
//     mov r1.y, r0.y
//     rcp r1.x, c3.x
//     add r0.xy, r1.xy, r1.xy
//     neg r0.z, c0.x
//     add r1.x, r0.z, (1.57075)
//     mul r0.z, r1.x, (0.15915494309189535)
//     add r1.x, r0.z, (0.5)
//     frc r0.z, r1.x
//     mul r1.x, r0.z, (6.283185307179586)
//     add r0.z, r1.x, (-3.141592653589793)
//     cos r1.x, r0.z
//     sin r1.y, r0.z
//     mul r2.xy, r0.xy, r1.xy
//     mul oc0.xy, (3), r2.xy
//     mov oc1.xy, r2.xy
//     mul oc4.xy, (0.5), c2.zw
//     mul oc7.x, c1.x, (10)
//     mul r0.x, c0.x, (0.15915494309189535)
//     add r1.x, r0.x, (0.5)
//     frc r0.x, r1.x
//     mul r1.x, r0.x, (6.283185307179586)
//     add r0.x, r1.x, (-3.141592653589793)
//     cos r1.x, r0.x
//     sin r1.y, r0.x
//     mov oc5.x, r1.x
//     mov oc6.x, r1.y
//
    ps_2_0
    def c12, 6.2831855, -3.1415927, 1.6666666, 1
    def c13, 0, 0.15915494, 0.5, 0.33333334
    def c14, 0.1, 1, 1.5, 1.4
    def c15, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c16, -0.020833334, -0.125, 1, 0.5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    mov r0.xy, c11
    mad r0.xy, t0, r0, -c10
    add r0.xy, r0, -c4
    mul r0.z, r0.y, c6.x
    mad r1.x, r0.x, c5.x, -r0.z
    mul r0.y, r0.y, c5.x
    mad r1.y, r0.x, c6.x, r0.y
    add r0.xy, r1, c4
    add r0.y, r0.y, c7.x
    mad r0.x, r0.x, c13.y, c13.z
    frc r0.x, r0.x
    mad r0.x, r0.x, c12.x, c12.y
    sincos r1.y, r0.x, c15, c16
    mad r0.x, r0.y, c13.y, c13.z
    frc r0.x, r0.x
    mad r0.x, r0.x, c12.x, c12.y
    sincos r2.y, r0.x, c15, c16
    add r0.xy, t0, c1
    add r0.z, r0.y, -c3.x
    add r3.xy, t0, c0
    texld r4, r0, s0
    texld r5, r3, s0
    texld r6, t0, s0
    add r0.x, -r0.y, c2.x
    cmp r0.y, r0.z, r4.w, c13.x
    cmp r0.x, r0.x, r0.y, c13.x
    mad_sat r1.w, r1.y, r2.y, r0.x
    mul r1.xyz, r1.w, c8
    add r0.y, r3.y, -c3.x
    add r0.z, -r3.y, c2.x
    cmp r0.y, r0.y, r5.w, c13.x
    cmp r0.y, r0.z, r0.y, c13.x
    max r2.x, r0.y, r0.x
    add r0.x, r6.y, r6.x
    add r0.x, r6.z, r0.x
    mad r0.y, r0.x, -c12.z, r2.x
    mul r0.x, r0.x, c13.w
    mul r2.xyz, r0.x, c9
    mul r1, r1, r0.y
    mad r0.x, r6.w, -c14.x, c14.y
    mul r1, r1, r0.x
    mul r1, r1, c14.z
    cmp r0, r0.y, r1, c13.x
    mov r2.w, c12.w
    mad r0, r2, r6.w, r0
    mul r0, r0, c14.w
    min r1, r0, c12.w
    max r0.xyz, v0, v0.w
    lrp r2.xyz, c13.z, r0, v0
    mov r2.w, v0.w
    mul r0, r1, r2
    mov oC0, r0

// 52 instructions
