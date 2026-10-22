// ScreenShader.Technique1[6] FilterSepia
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor             c3  default (0, 0, 0, 0)
//   sampler2D uImage0            s0
//   sampler2D uImage1            s1
//   float2    uImageSize0        c9  default (0, 0, 0, 0)
//   float     uIntensity         c8  default (0, 0, 0, 0)
//   float     uOpacity           c7  default (0, 0, 0, 0)
//   float2    uSceneOffset       c10  default (0, 0, 0, 0)
//   float2    uScreenPosition    c5  default (0, 0, 0, 0)
//   float2    uScreenResolution  c4  default (0, 0, 0, 0)
//   float2    uTargetPosition    c6  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize1  c1  default (0, 0, 0, 0)
//   float  uProgress    c0  default (0, 0, 0, 0)
//   float2 uSceneSize   c2  default (0, 0, 0, 0)
//
//     rcp oc0.x, c2.x
//     rcp oc0.y, c2.y
//     rcp oc1.x, c1.x
//     rcp oc1.y, c1.y
//     rcp oc2.x, c0.x
//
    ps_2_0
    def c11, 0.33333334, 0.9622505, 0.19245009, 0.19245009
    def c12, 2, -1, 0.2, 4
    def c13, 0.5, -0.5, 1.1, 1
    def c14, 1.2, 0.9, 1, 0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    mul r0.xy, t0, c4
    mov r1.w, c12.x
    mad r0.xy, r0, r1.w, c5
    mov r1.xy, c1
    mad r0.xy, r0, r1, c6
    texld r0, r0, s1
    texld r1, t0, s0
    mul r0.xyz, r0, r0
    nrm r2.xyz, r1
    dp3 r0.w, r2, c11.wzyx
    mad_sat r0.w, r0.w, c12.x, c12.y
    mul r0.w, r0.w, c12.z
    add r2.x, r1.y, r1.x
    add r2.x, r1.z, r2.x
    mad r2.yzw, r2.x, c11.x, -r1.wzyx
    mad r2.yzw, r0.w, r2, r1.wzyx
    mad r3.xyz, r2.x, c11.x, -r2.wzyx
    min r0.w, r1.z, r1.y
    min r3.w, r0.w, r1.x
    max r0.w, r1.y, r1.z
    max r4.w, r1.x, r0.w
    add r0.w, -r3.w, r4.w
    mad r3.w, r4.w, -c14.y, c14.z
    mul_sat r0.w, r0.w, c12.w
    add r0.w, -r0.w, -c12.y
    mad r2.yzw, r0.w, r3.wzyx, r2
    mov r0.w, c11.x
    mad r3.xyz, r2.x, r0.w, c3
    mul r0.w, r2.x, c11.x
    mul_sat r0.w, r0.w, c2.x
    add r0.w, -r0.w, -c12.y
    mad r0.w, r0.w, -r0.w, -c12.y
    mul r0.w, r0.w, r0.w
    lrp r4.xyz, c13.x, r3, r2.wzyx
    mad r0.xyz, r0, r4, -r4
    mov r2.xy, c9
    mad r2.xy, t0, r2, c10
    mov r4.w, c13.y
    mad r2.xy, r2, c0, r4.w
    mul r4.w, r2.y, r2.y
    mad r4.w, r2.x, r2.x, r4.w
    rsq r4.w, r4.w
    rcp r4.w, r4.w
    mad r4.w, r4.w, -c13.z, c13.w
    add r2.x, -r4.w, -c12.y
    mul r2.x, r2.x, r2.x
    mul r2.x, r2.x, c14.x
    mul r2.x, r3.w, r2.x
    mad r0.xyz, r2.x, r0, r4
    mul r0.xyz, r0.w, r0
    mul r0.xyz, r4.w, r0
    mul r0.xyz, r0, c8.x
    mad r0.xyz, r0, r1.w, -r1
    mad r1.xyz, c7.x, r0, r1
    mov oC0, r1

// 55 instructions
