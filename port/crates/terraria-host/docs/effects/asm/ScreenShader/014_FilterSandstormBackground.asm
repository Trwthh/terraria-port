// ScreenShader.Technique1[14] FilterSandstormBackground
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor             c4  default (0, 0, 0, 0)
//   sampler2D uImage1            s1
//   float     uOpacity           c9  default (0, 0, 0, 0)
//   float2    uScreenPosition    c7  default (0, 0, 0, 0)
//   float2    uScreenResolution  c6  default (0, 0, 0, 0)
//   float3    uSecondaryColor    c5  default (0, 0, 0, 0)
//   float2    uTargetPosition    c8  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uDirection       c1  default (0, 0, 0, 0)
//   float2 uImageSize1      c2  default (0, 0, 0, 0)
//   float2 uTargetPosition  c0  default (0, 0, 0, 0)
//
//     rcp oc0.x, c2.x
//     rcp oc0.y, c2.y
//     mul oc1.x, c1.x, (0.01)
//     mul oc2.xy, (1.2), c0.xy
//     mul oc3.xy, (1.4), c0.xy
//
    ps_2_0
    def c10, 0.40743667, 0.25, 6.2831855, -3.1415927
    def c11, -0.0000002523985, 0.0000247609, -0.0013888397, 0.04166664
    def c12, -0.5, 1, 0.3, 0.085
    def c13, 2.5, -2, 3, -0.52
    def c14, 10, 1.4, 0.2, 0
    def c15, 0.05, 1, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s1
    mov r0.xy, c6
    mad r0.xy, t0, r0, c7
    mul r0.xy, r0, c0
    mad r1.x, r0.y, c10.x, c10.y
    mad r1.y, r0.x, c10.x, c10.y
    frc r0.zw, r1.wzyx
    mad r0.zw, r0, c10.z, c10.w
    mul r0.zw, r0, r0
    mad r1.xy, r0.wzyx, c11.x, c11.y
    mad r1.xy, r0.wzyx, r1, c11.z
    mad r1.xy, r0.wzyx, r1, c11.w
    mad r1.xy, r0.wzyx, r1, c12.x
    mad r0.zw, r0, r1.wzyx, c12.y
    mad r0.xy, r0.wzyx, c1.x, r0
    add r1.xy, r0, c2
    add r2.xy, r0, c8
    add r0.xy, r0, c3
    texld r1, r1, s1
    texld r0, r0, s1
    texld r2, r2, s1
    mad r0.x, r1.y, c12.z, c12.w
    mul_sat r0.x, r0.x, c13.x
    mad r0.y, r0.x, c13.y, c13.z
    mul r0.x, r0.x, r0.x
    mul r0.x, r0.y, r0.x
    add r0.y, r0.z, c13.w
    mul_sat r0.y, r0.y, c14.x
    mul r1.xyz, r2.x, c5
    mov r3.xyz, c5
    mad r3.xyz, r3, -r2.x, c4
    mov r1.w, r2.x
    add r3.w, -r1.w, c12.y
    mad r1, r0.x, r3, r1
    mad r0.x, r0.y, c13.y, c13.z
    mul r0.y, r0.y, r0.y
    mul r0.x, r0.x, r0.y
    lrp r2, r0.x, c14.y, r1
    mul r0, r2, c9.x
    mul r0, r0, c14.z
    lrp r1, c15.x, c15.y, v0
    mul r0, r0, r1
    mov oC0, r0

// 42 instructions
