// PixelShader.Technique1[46] ArmorLoki
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor       c7  default (0, 0, 0, 0)
//   sampler2D uImage0      s0
//   float2    uImageSize0  c9  default (0, 0, 0, 0)
//   float4    uSourceRect  c8  default (0, 0, 0, 0)
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
//     mul r1.x, r0.x, r0.y
//     neg r1.y, r0.y
//     mov r2.y, r0.y
//     add oc1.x, r1.x, r1.y
//     rcp r2.x, c3.x
//     add r0.xy, r2.xy, r2.xy
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
//     mov oc2.xy, r2.xy
//     mul oc3.xy, (0.5), c2.zw
//     mul oc6.x, c1.x, (10)
//     mul r0.x, c0.x, (0.15915494309189535)
//     add r1.x, r0.x, (0.5)
//     frc r0.x, r1.x
//     mul r1.x, r0.x, (6.283185307179586)
//     add r0.x, r1.x, (-3.141592653589793)
//     cos r1.x, r0.x
//     sin r1.y, r0.x
//     mov oc4.x, r1.x
//     mov oc5.x, r1.y
//
    ps_2_0
    def c10, 0.15915494, 0.5, 6.2831855, -3.1415927
    def c11, 1, 1.6666666, 0, 0.1
    def c12, 2.1, 0, 0, 0
    def c13, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c14, -0.020833334, -0.125, 1, 0.5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    mov r0.xy, c9
    mad r0.xy, t0, r0, -c8
    add r0.xy, r0, -c3
    mul r0.z, r0.y, c5.x
    mad r1.x, r0.x, c4.x, -r0.z
    mul r0.y, r0.y, c4.x
    mad r1.y, r0.x, c5.x, r0.y
    add r0.xy, r1, c3
    add r0.y, r0.y, c6.x
    mad r0.x, r0.x, c10.x, c10.y
    frc r0.x, r0.x
    mad r0.x, r0.x, c10.z, c10.w
    sincos r1.y, r0.x, c13, c14
    mad r0.x, r0.y, c10.x, c10.y
    frc r0.x, r0.x
    mad r0.x, r0.x, c10.z, c10.w
    sincos r2.y, r0.x, c13, c14
    add r0.xy, t0, c2
    min r3.y, c1.x, r0.y
    mov r3.x, r0.x
    add r0.xy, t0, c0
    min r4.y, c1.x, r0.y
    mov r4.x, r0.x
    texld r0, r3, s0
    texld r3, r4, s0
    texld r4, t0, s0
    mad r0.x, r1.y, r2.y, r0.w
    min r1.w, r0.x, c11.x
    mul r1.xyz, r1.w, c7
    max r2.x, r3.w, r0.w
    add r0.x, r4.y, r4.x
    add r0.x, r4.z, r0.x
    mad r0.x, r0.x, -c11.y, r2.x
    mul r1, r1, r0.x
    cmp r0, r0.x, r1, c11.z
    mad r1.x, r4.w, -c11.w, c11.x
    mul r1, r0, r1.x
    mul r1, r1, c12.x
    cmp r0, r0, r1, c11.z
    add r0, r4, r0
    min r1, r0, c11.x
    max r0.xyz, v0, v0.w
    lrp r2.xyz, c10.y, r0, v0
    mov r2.w, v0.w
    mul r0, r1, r2
    mov oC0, r0

// 46 instructions
