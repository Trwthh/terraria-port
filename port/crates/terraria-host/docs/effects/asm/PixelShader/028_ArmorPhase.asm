// PixelShader.Technique1[28] ArmorPhase
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor       c9  default (0, 0, 0, 0)
//   sampler2D uImage0      s0
//   sampler2D uImage1      s1
//   float2    uImageSize0  c12  default (0, 0, 0, 0)
//   float     uSaturation  c10  default (0, 0, 0, 0)
//   float4    uSourceRect  c11  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uColor       c0  default (0, 0, 0, 0)
//   float2 uImageSize0  c4  default (0, 0, 0, 0)
//   float2 uImageSize1  c5  default (0, 0, 0, 0)
//   float  uSaturation  c1  default (0, 0, 0, 0)
//   float4 uSourceRect  c3  default (0, 0, 0, 0)
//   float  uTime        c2  default (0, 0, 0, 0)
//
//     rcp r0.x, c4.x
//     rcp r0.z, c4.x
//     rcp r0.y, c4.y
//     rcp r0.w, c4.y
//     add r1.xy, r0.zw, r0.zw
//     mul r2.xyzw, r0.xyzw, c3.xyzw
//     rcp oc0.x, r1.x
//     rcp oc0.y, r1.y
//     mov oc1.xy, r1.xy
//     rcp oc2.x, c5.x
//     rcp oc2.y, c5.y
//     mul oc3.x, c2.x, (0.1)
//     mul r0.xy, (0.5), r2.zw
//     add oc4.xy, r2.xy, r0.xy
//     neg r0.xyz, c0.xyz
//     add r1.xyz, (1), r0.xyz
//     neg r0.xyz, r1.xyz
//     mov oc5.xyz, r1.xyz
//     add oc6.xyz, (1), r0.xyz
//     neg r0.x, c1.x
//     add r1.x, r0.x, (1)
//     rcp r0.x, c1.x
//     cmp r2.x, r1.x, (1), r0.x
//     neg r0.x, r2.x
//     mov oc7.x, r2.x
//     add oc8.x, r0.x, (1)
//
    ps_2_0
    def c13, 0.5, 2, 0.1, 0.8333333
    def c14, 0.90909094, -0.2, 1.25, 5
    def c15, -2, 3, 0.33333334, 0
    def c16, 0.5, 1.5, 1, 0
    def c17, 0.66, 0.33, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    texld r0, t0, s0
    max r1.w, r0.y, r0.z
    max r2.w, r0.x, r1.w
    min r1.x, r0.z, r0.y
    min r2.x, r1.x, r0.x
    add r1.x, r2.w, r2.x
    add r1.y, r2.w, -r2.x
    mad r1.z, r1.x, -c16.x, c16.y
    mov r1.w, c16.z
    mad r2.xyz, r1.z, -c6, r1.w
    mad r2.w, r1.x, -c13.x, c13.x
    mul r3.xyz, r1.x, c5
    cmp r2.xyz, r2.w, r3, r2
    mad r2.xyz, r1.x, -c13.x, r2
    mul r2.w, r1.x, c13.x
    mul r1.x, r1.y, c10.x
    mad r1.y, r1.y, c17.x, c17.y
    mov r3.x, c7.x
    mad r1.x, r1.x, r3.x, c8.x
    mad r2.xyz, r1.x, r2, r2.w
    mad r2.xyz, r2, -r0.w, c16.z
    mul r2.xyz, r0.w, r2
    mul r1.xyz, r1.y, r2
    mov r2.xy, c12
    mad r2.xy, t0, r2, -c11
    mul r2.xy, r2, c13.x
    frc r2.zw, r2.wzyx
    add r2.xy, r2, -r2.wzyx
    add r2.xy, r2, r2
    mul r2.xy, r2, c2
    mul r3.x, r2.x, c13.y
    mov r2.z, c13.z
    mad r3.y, r2.y, r2.z, c3.x
    mul r2.xy, t0, c0
    frc r2.zw, r2.wzyx
    add r2.xy, r2, -r2.wzyx
    mov r4.xy, c4
    mad r2.xy, r2, c1, -r4
    mul r4.x, r2.x, c13.w
    mov r2.x, c14.x
    mad r4.y, r2.y, r2.x, c1.y
    add r2.xy, r4, c4
    texld r3, r3, s1
    texld r2, r2, s0
    add r2.x, r3.x, c14.y
    mul_sat r2.x, r2.x, c14.z
    mad r2.y, r2.x, c15.x, c15.y
    mul r2.x, r2.x, r2.x
    mul r3.w, r2.y, r2.x
    mul r3.xyz, r3.w, c9
    mul r3, r3, c14.w
    add r0.x, r0.y, r0.x
    add r0.x, r0.z, r0.x
    mov r1.w, r0.w
    mad r0.x, r0.x, -c15.z, r2.w
    mul r2, r3, r0.x
    cmp r0, r0.x, r2, c15.w
    add r0, r1, r0
    mul r0, r0, v0.w
    mov oC0, r0

// 60 instructions
