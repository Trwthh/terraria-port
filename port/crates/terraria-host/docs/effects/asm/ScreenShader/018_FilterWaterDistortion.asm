// ScreenShader.Technique1[18] FilterWaterDistortion
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0            s0
//   sampler2D uImage1            s1
//   sampler2D uImage2            s2
//   sampler2D uImage3            s3
//   float2    uImageOffset       c11  default (0, 0, 0, 0)
//   float     uIntensity         c10  default (0, 0, 0, 0)
//   float2    uScreenPosition    c8  default (0, 0, 0, 0)
//   float2    uScreenResolution  c7  default (0, 0, 0, 0)
//   float2    uTargetPosition    c9  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize1        c3  default (0, 0, 0, 0)
//   float2 uImageSize2        c4  default (0, 0, 0, 0)
//   float2 uImageSize3        c5  default (0, 0, 0, 0)
//   float  uOpacity           c1  default (0, 0, 0, 0)
//   float  uProgress          c2  default (0, 0, 0, 0)
//   float2 uScreenResolution  c0  default (0, 0, 0, 0)
//
//     rcp oc0.x, c3.x
//     rcp oc0.y, c3.y
//     mul oc1.x, c2.x, (0.04)
//     mul oc2.x, c2.x, (-0.04)
//     rcp oc3.x, c4.x
//     rcp oc3.y, c4.y
//     mul r0.x, c1.x, (32)
//     rcp r1.x, c0.x
//     rcp r1.y, c0.y
//     mul oc5.xy, r0.x, r1.xy
//     rcp oc6.x, c5.x
//     rcp oc6.y, c5.y
//
    ps_2_0
    def c12, 0.1, -0.5, 0.2, 0.25
    def c13, 0.4, 1, -0.3, 0.3
    def c14, 0.1, 1, 100, 0.001
    def c15, 500, -2, 3, 2
    def c16, 0, 0.33333334, 0, 0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    dcl_2d s2
    dcl_2d s3
    mov r0.xy, c7
    mad r0.zw, t0.wzyx, r0.wzyx, c8.wzyx
    mul r0.zw, r0, c0.wzyx
    mul r1.x, r0.w, c12.x
    mov r2.xz, c12
    mad r1.y, r0.z, r2.x, c1.x
    mul r2.x, r0.w, c12.z
    mad r2.y, r0.z, r2.z, c2.x
    mad r0.zw, t0.wzyx, r0.wzyx, c11.wzyx
    mul r0.zw, r0, c12.w
    mul r3.xy, r0.wzyx, c3
    texld r1, r1, s1
    texld r2, r2, s1
    texld r3, r3, s2
    add r0.zw, r1.wzyx, c12.y
    add r1.xy, r2, c12.y
    add r0.zw, r0, -r1.wzyx
    mul r1.xy, r0.wzyx, c12.x
    add r0.z, r3.x, c12.y
    mad r0.w, r3.z, -c13.x, c13.y
    mul r0.z, r0.z, r0.w
    max r1.z, -r0.z, c13.z
    min r0.w, r1.z, c13.w
    mad r2.y, r1.y, c10.x, r0.w
    mul r2.x, r1.x, c10.x
    dp2add r0.w, r1, r1, c16.x
    rsq r0.w, r0.w
    rcp r0.w, r0.w
    mad r0.z, r0.z, c13.w, r0.w
    mul r1.xy, r2, c14
    mov r0.w, c14.w
    mad r0.w, r1.y, c5.y, r0.w
    mad r1.xy, r1, c5, t0
    mul_sat r0.w, r0.w, c15.x
    mad r1.z, r0.w, c15.y, c15.z
    mul r0.w, r0.w, r0.w
    mul r0.w, r1.z, r0.w
    mad r1.zw, r1.wzyx, r0.wzyx, c9.wzyx
    mul r2.xy, r1.wzyx, c6
    mad r0.xy, t0, r0, c9
    mul r0.xy, r0, c6
    texld r1, r1, s0
    texld r2, r2, s3
    texld r3, r0, s3
    texld r4, t0, s0
    mul r0.x, r2.w, c14.z
    min r2.x, r0.x, c13.y
    mul r0.x, r3.w, c14.z
    min r2.y, r0.x, c13.y
    add r0.x, r2.x, -r2.y
    add r0.y, r2.x, r2.y
    mul r0.y, r0.y, c14.z
    min r2.y, r0.y, c13.y
    mad r0.y, r0.w, c15.w, r0.x
    abs r0.x, r0.x
    add r0.y, r0.y, -c13.y
    abs r0.y, r0.y
    add r0.y, -r0.y, c13.y
    mul_sat r0.x, r0.x, r0.y
    add r0.x, -r0.x, c13.y
    mul r0.y, r2.y, r0.x
    lrp r3, r0.y, r1, r4
    add r0.y, r3.y, r3.x
    add r0.y, r3.z, r0.y
    mul r0.y, r0.z, r0.y
    mul r0.y, r2.x, r0.y
    mul r0.x, r0.x, r0.y
    mad r3.xyz, r0.x, c16.y, r3
    mov oC0, r3

// 69 instructions
