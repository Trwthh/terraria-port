// ScreenShader.Technique1[4] FilterMoonLord
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0            s0
//   float     uOpacity           c3  default (0, 0, 0, 0)
//   float2    uScreenPosition    c1  default (0, 0, 0, 0)
//   float2    uScreenResolution  c0  default (0, 0, 0, 0)
//   float2    uTargetPosition    c2  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float uTime  c0  default (0, 0, 0, 0)
//
//     mul r0.x, c0.x, (3.183098861837907)
//     add r1.x, r0.x, (0.5)
//     frc r0.x, r1.x
//     mul r1.x, r0.x, (6.283185307179586)
//     add r0.x, r1.x, (-3.141592653589793)
//     sin r1.y, r0.x
//     mul oc4.x, r1.y, (0.01)
//
    ps_2_0
    def c5, 0, 0.0005, 1, 0.15
    def c6, 0.7, -0.5, -0.6, 0.5
    def c7, 2, -1, 0.3, 3
    def c8, -2, 3, 0, 0
    def c9, 0.48, -4000, 0.00025, 0
    def c10, 0.0005, -0.4, 0.00065, 1
    def c11, 1, 0.8333333, 1.25, 1
    dcl t0.xy
    dcl_2d s0
    mov r0.xy, c0
    mad r0.xy, r0, t0, c1
    add r0.xy, -r0, c2
    dp2add r0.x, r0, r0, c5.x
    rsq r0.x, r0.x
    rcp r0.x, r0.x
    mul r0.y, r0.x, c5.y
    min r1.w, r0.y, c5.z
    mul r1.x, r1.w, c4.x
    mov r1.y, c5.x
    add r2.xy, -r1, t0
    add r1.xy, r1, t0
    texld r1, r1, s0
    texld r2, r2, s0
    texld r3, t0, s0
    mul r0.yzw, r1.wzyx, c5.w
    mad r0.yzw, r3.wzyx, c6.x, r0
    mad r0.yzw, r2.wzyx, c5.w, r0
    add r1.xy, -r0.wzyx, c5.z
    mad r1.xy, r1, -c11.wzyx, c11.x
    add r2.xy, r0.wzyx, c6.yzxw
    mul r1.zw, r0, c11.wzyx
    cmp r1.xy, r2, r1, r1.wzyx
    mad r1.xy, r1, c7.x, c7.y
    cmp r1.zw, -r1.wzyx, c5.x, c5.z
    cmp r2.xy, r1, -c5.x, -c5.z
    abs r1.xy, r1
    add r1.zw, r1, r2.wzyx
    mul r2.xy, r1, r1
    mad r2.xy, r2, -r1, r2
    add r1.xy, r1, r2
    mad r1.xy, r1, r1.wzyx, c5.z
    mul r1.xy, r1, c6.w
    add r2.xyz, r0.wzyx, c7.z
    mul r2.w, r0.y, c7.w
    mul r0.yzw, r2.wzyx, r2.w
    mul r0.yzw, r0, c6.w
    max r2.xy, r0.wzyx, r1
    mov r2.z, r0.y
    mad r2.w, r0.x, c10.x, c10.y
    mad r0.y, r0.x, -c10.z, c10.w
    add r0.x, r0.x, c9.y
    mul_sat r0.x, r0.x, c9.z
    cmp r2.w, r2.w, c9.x, r0.y
    mad r0.yzw, r2.wzyx, -r2.w, r3.wzyx
    mul r1.xyz, r2, r2.w
    mad r1.w, r0.x, c8.x, c8.y
    mul r0.x, r0.x, r0.x
    mul r1.w, r1.w, r0.x
    mad r0.xyz, r1.w, r0.wzyx, r1
    mad r0.xyz, r0, r3.w, -r3
    mad r3.xyz, c3.x, r0, r3
    mov oC0, r3

// 53 instructions
