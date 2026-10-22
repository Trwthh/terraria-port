// PixelShader.Technique1[40] ArmorMidnightRainbow
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0      s0
//   float2    uImageSize0  c5  default (0, 0, 0, 0)
//   float4    uSourceRect  c4  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize0  c2  default (0, 0, 0, 0)
//   float4 uSourceRect  c1  default (0, 0, 0, 0)
//   float  uTime        c0  default (0, 0, 0, 0)
//
//     rcp oc2.x, c1.z
//     mul oc3.x, c0.x, (0.4)
//     rcp r0.x, c2.x
//     rcp r0.y, c2.y
//     add r1.xy, r0.xy, r0.xy
//     mov oc0.x, r1.y
//     mov oc1.x, r1.x
//
    ps_2_0
    def c6, 0, 0.33333334, 4, 0.4
    def c7, -2, 3, 0.06666667, -1
    def c8, 0, 0.666, 0.333, 0.9
    dcl v0
    dcl t0.xy
    dcl_2d s0
    mov r0.x, -c1.x
    mov r0.y, c6.x
    add r0.xy, r0, t0
    mov r1.x, t0.x
    add r1.y, t0.y, -c0.x
    mov r2.x, t0.x
    add r2.y, t0.y, c0.x
    add r3.x, t0.x, c1.x
    mov r3.y, t0.y
    texld r0, r0, s0
    texld r4, t0, s0
    texld r1, r1, s0
    texld r2, r2, s0
    texld r3, r3, s0
    add r0.xyz, r0, -r4
    add r1.w, r0.y, r0.x
    add r1.w, r0.z, r1.w
    add r0.xyz, -r4, r1
    add r2.w, r0.y, r0.x
    add r2.w, r0.z, r2.w
    mul r2.w, r2.w, c6.y
    mad r2.w, r1.w, c6.y, r2.w
    add r0.xyz, -r4, r2
    add r3.w, r0.y, r0.x
    add r3.w, r0.z, r3.w
    mad r3.w, r3.w, c6.y, r2.w
    add r0.xyz, -r4, r3
    add r0.x, r0.y, r0.x
    add r0.x, r0.z, r0.x
    mad r0.x, r0.x, c6.y, r3.w
    add r0.y, r4.y, r4.x
    add r0.y, r4.z, r0.y
    mul r0.z, r0.y, c6.y
    mul r0.x, r0.x, r0.z
    mul r0.x, r0.x, c6.z
    mov r1.x, c5.x
    mad r0.z, t0.x, r1.x, -c4.x
    mul_sat r0.z, r0.z, c2.x
    mad r0.w, r0.z, c7.x, c7.y
    mul r0.z, r0.z, r0.z
    mul r0.z, r0.w, r0.z
    mov r0.w, c6.w
    mad r0.z, r0.z, r0.w, c3.x
    mad r0.y, r0.y, c7.z, r0.z
    add r0.yzw, r0.y, c8.wzyx
    abs r1.xyz, r0.wzyx
    frc r1.xyz, r1
    cmp r0.yzw, r0, r1.wzyx, -r1.wzyx
    mad r0.yzw, r0, c7.y, c7.w
    abs r0.yzw, r0
    add r0.yzw, -r0, c8.w
    mul r0.xyz, r0.x, r0.wzyx
    mov r0.w, -c7.w
    mul r0, r4.w, r0
    mul r0, r0, v0
    mov oC0, r0

// 56 instructions
