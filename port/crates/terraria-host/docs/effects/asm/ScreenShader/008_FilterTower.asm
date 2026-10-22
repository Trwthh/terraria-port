// ScreenShader.Technique1[8] FilterTower
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor             c5  default (0, 0, 0, 0)
//   sampler2D uImage0            s0
//   float     uIntensity         c10  default (0, 0, 0, 0)
//   float     uOpacity           c9  default (0, 0, 0, 0)
//   float2    uScreenPosition    c7  default (0, 0, 0, 0)
//   float2    uScreenResolution  c6  default (0, 0, 0, 0)
//   float2    uTargetPosition    c8  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uColor             c0  default (0, 0, 0, 0)
//   float  uProgress          c3  default (0, 0, 0, 0)
//   float2 uScreenResolution  c1  default (0, 0, 0, 0)
//   float  uTime              c2  default (0, 0, 0, 0)
//
//     mul r0.x, c3.x, (1000)
//     add oc0.x, r0.x, (180)
//     neg r0.x, c3.x
//     add oc1.x, r0.x, (1)
//     mul oc2.x, c2.x, (-5)
//     rcp oc3.x, c1.x
//     rcp oc3.y, c1.y
//     neg r0.xyz, c0.xyz
//     add oc4.xyz, (1), r0.xyz
//
    ps_2_0
    def c11, 1.5, 1, 0, 50
    def c12, 5, 0.2, 0.15915494, 0.5
    def c13, 6.2831855, -3.1415927, 2.25, 3.5
    def c14, 0.5, -0.5, 0, 0
    def c15, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c16, -0.020833334, -0.125, 1, 0.5
    dcl t0.xy
    dcl_2d s0
    mov r0.xy, c7
    mad r0.xy, c6, t0, r0
    add r0.xy, r0, -c8
    mul r1.xy, r0, c11
    dp2add r0.z, r1, r1, c11.z
    rsq r0.z, r0.z
    rcp r0.w, r0.z
    mul r0.xy, r0, r0.z
    add r0.z, r0.w, -c0.x
    rcp r0.z, -r0.z
    mul r0.z, r0.z, c11.w
    abs r0.z, r0.z
    min r1.x, r0.z, c12.x
    mul r0.z, r1.x, c10.x
    mul r1.x, r0.w, c12.y
    mov r2.w, c1.x
    mad r1.x, r1.x, r2.w, c2.x
    mad r1.x, r1.x, c12.z, c12.w
    frc r1.x, r1.x
    mad r1.x, r1.x, c13.x, c13.y
    sincos r2.y, r1.x, c15, c16
    mad r0.z, r2.y, r0.z, r0.w
    mad r0.xy, r0, r0.z, c8
    add r0.xy, r0, -c7
    mad r0.zw, r0.wzyx, c3.wzyx, -t0.wzyx
    mul r0.xy, r0, c3
    mad r1.xy, r0.wzyx, c13.w, t0
    mad r2.xy, r0.wzyx, c13.z, t0
    texld r0, r0, s0
    texld r2, r2, s0
    texld r1, r1, s0
    max r0.x, r1.y, r1.z
    max r2.x, r1.x, r0.x
    min r0.x, r1.z, r1.y
    min r2.z, r0.x, r1.x
    add r0.x, r2.x, r2.z
    add r0.y, r0.x, -c11.y
    mov r3.xyz, c5
    mad r3.xyz, r0.y, c4, r3
    mad r3.w, r0.x, c14.x, c14.y
    mul r4.xyz, r0.x, c5
    cmp r3.xyz, r3.w, r3, r4
    mad r3.xyz, r3, r1.w, -r1
    mad r3.xyz, c9.x, r3, r1
    add r0.x, r0.z, -r1.z
    add r3.z, r3.z, r0.x
    add r0.x, r2.y, -r1.y
    mov r3.w, r1.w
    add r3.y, r3.y, r0.x
    mov oC0, r3

// 50 instructions
