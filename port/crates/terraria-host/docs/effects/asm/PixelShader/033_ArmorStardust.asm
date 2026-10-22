// PixelShader.Technique1[33] ArmorStardust
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor           c3  default (0, 0, 0, 0)
//   sampler2D uImage0          s0
//   sampler2D uImage1          s1
//   float2    uImageSize0      c6  default (0, 0, 0, 0)
//   float3    uSecondaryColor  c4  default (0, 0, 0, 0)
//   float4    uSourceRect      c5  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize0  c1  default (0, 0, 0, 0)
//   float  uTime        c0  default (0, 0, 0, 0)
//
//     rcp r0.x, c1.x
//     rcp r0.y, c1.y
//     add r1.xy, r0.xy, r0.xy
//     rcp oc0.x, r1.x
//     rcp oc0.y, r1.y
//     mov oc1.xy, r1.xy
//     mul oc2.x, c0.x, (0.2)
//
    ps_2_0
    def c7, 0.0078125, 0.333, 0.666, 0
    def c8, 3, -1, 1, 0.6666667
    def c9, 0.5, -0.1, 8, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    mul r0.xy, t0, c0
    frc r0.zw, r0.wzyx
    add r0.xy, r0, -r0.wzyx
    mul r0.xy, r0, c1
    mov r1.xy, c6
    mad r0.xy, r0, r1, -c5
    mov r1.x, c7.x
    mad r0.z, r0.x, r1.x, c2.x
    mad r0.z, r0.y, c7.x, r0.z
    mul r0.xy, r0, c7.x
    texld r1, r0, s1
    texld r2, t0, s0
    add r0.xyz, r0.z, c7.wzyx
    abs r3.xyz, r0
    frc r3.xyz, r3
    cmp r0.xyz, r0, r3, -r3
    mad r0.xyz, r0, c8.x, c8.y
    abs r0.xyz, r0
    add r0.xyz, -r0, c8.z
    mul r0.xyz, r1, r0
    mul r0.xy, r0, r0
    add r0.x, r0.y, r0.x
    mad r0.x, r0.z, r0.z, r0.x
    mad_sat r0.x, r0.x, c9.x, c9.y
    mul r0.xyz, r0.x, c4
    mul r0.xyz, r2.w, r0
    mul r0.xyz, r0, c9.z
    add r0.w, r2.y, r2.x
    add r0.w, r2.z, r0.w
    mul r0.w, r0.w, c8.w
    mul r1.xyz, r0.w, c3
    mad r2.xyz, r1, r2.w, r0
    mul r0, r2, v0
    mov oC0, r0

// 34 instructions
