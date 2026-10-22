// ScreenShader.Technique1[15] FilterBlizzardForeground
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor             c5  default (0, 0, 0, 0)
//   sampler2D uImage0            s0
//   sampler2D uImage1            s1
//   float     uIntensity         c11  default (0, 0, 0, 0)
//   float     uOpacity           c10  default (0, 0, 0, 0)
//   float2    uScreenPosition    c8  default (0, 0, 0, 0)
//   float2    uScreenResolution  c7  default (0, 0, 0, 0)
//   float3    uSecondaryColor    c6  default (0, 0, 0, 0)
//   float2    uTargetPosition    c9  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uDirection       c1  default (0, 0, 0, 0)
//   float2 uImageSize1      c3  default (0, 0, 0, 0)
//   float  uIntensity       c2  default (0, 0, 0, 0)
//   float2 uTargetPosition  c0  default (0, 0, 0, 0)
//
//     rcp oc0.x, c3.x
//     rcp oc0.y, c3.y
//     mul oc1.x, c1.x, (0.06)
//     mul oc2.xy, (1.2), c0.xy
//     mul oc3.xy, (1.4), c0.xy
//     neg r0.x, c2.x
//     add oc4.x, r0.x, (1)
//
    ps_2_0
    def c12, -0.5, 1, 0.3, 0.085
    def c13, 0.81487334, 0.25, 6.2831855, -3.1415927
    def c14, -0.0000002523985, 0.0000247609, -0.0013888397, 0.04166664
    def c15, 2.5, -2, 3, -0.52
    def c16, 10, 1.2, 0.33333334, 0.6
    def c17, 0.4, 0, 4, 0
    dcl v0.xyz
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    mov r0.xy, c7
    mad r0.xy, t0, r0, c8
    mul r0.xy, r0, c0
    mad r1.x, r0.y, c13.x, c13.y
    mad r1.y, r0.x, c13.x, c13.y
    frc r0.zw, r1.wzyx
    mad r0.zw, r0, c13.z, c13.w
    mul r0.zw, r0, r0
    mad r1.xy, r0.wzyx, c14.x, c14.y
    mad r1.xy, r0.wzyx, r1, c14.z
    mad r1.xy, r0.wzyx, r1, c14.w
    mad r1.xy, r0.wzyx, r1, c12.x
    mad r0.zw, r0, r1.wzyx, c12.y
    mad r0.xy, r0.wzyx, c1.x, r0
    add r1.xy, r0, c2
    add r2.xy, r0, c9
    add r0.xy, r0, c3
    texld r1, r1, s1
    texld r0, r0, s1
    texld r2, r2, s1
    texld r3, t0, s0
    mad r0.x, r1.y, c12.z, c12.w
    mul_sat r0.x, r0.x, c15.x
    mad r0.y, r0.x, c15.y, c15.z
    mul r0.x, r0.x, r0.x
    mul r0.x, r0.y, r0.x
    add r0.y, r0.z, c15.w
    mul_sat r0.y, r0.y, c16.x
    mul r1.xyz, r2.x, c6
    mov r4.xyz, c6
    mad r4.xyz, r4, -r2.x, c5
    mov r1.w, r2.x
    add r4.w, -r1.w, c12.y
    mad r1, r0.x, r4, r1
    mad r0.x, r0.y, c15.y, c15.z
    mul r0.y, r0.y, r0.y
    mul r0.x, r0.x, r0.y
    lrp r2, r0.x, c16.y, r1
    add r0.x, r3.y, r3.x
    add r0.x, r3.z, r0.x
    mul r0.x, r0.x, c16.z
    mov r1.x, c11.x
    mad r0.x, r0.x, c4.x, r1.x
    mad r1, r2, r0.x, -r3
    mul r0.x, r2.w, r0.x
    mul r0.x, r0.x, c10.x
    mul r0.x, r0.x, c16.w
    mad r0, r0.x, r1, r3
    add r1.xy, t0, c12.x
    dp2add r1.x, r1, r1, c17.y
    rsq r1.x, r1.x
    rcp r1.x, r1.x
    min r2.x, r1.x, -c12.x
    mul r1.x, r2.x, c10.x
    mul r1.x, r1.x, c11.x
    mul r1.x, r1.x, c17.z
    mul r1.yzw, v0.wzyx, c6.wzyx
    mul r2.xyz, r1.wzyx, c17.x
    mov r2.w, c12.y
    lrp r3, r1.x, r2, r0
    mov oC0, r3

// 61 instructions
