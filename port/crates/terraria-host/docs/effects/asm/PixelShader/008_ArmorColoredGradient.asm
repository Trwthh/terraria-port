// PixelShader.Technique1[8] ArmorColoredGradient
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor       c4  default (0, 0, 0, 0)
//   sampler2D uImage0      s0
//   float2    uImageSize0  c7  default (0, 0, 0, 0)
//   float     uSaturation  c5  default (0, 0, 0, 0)
//   float4    uSourceRect  c6  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uColor           c0  default (0, 0, 0, 0)
//   float  uSaturation      c2  default (0, 0, 0, 0)
//   float3 uSecondaryColor  c1  default (0, 0, 0, 0)
//   float4 uSourceRect      c3  default (0, 0, 0, 0)
//
//     rcp oc0.x, c3.z
//     neg r0.xyz, c0.xyz
//     add oc1.xyz, r0.xyz, c1.xyz
//     neg r0.x, c2.x
//     add r1.x, r0.x, (1)
//     rcp r0.x, c2.x
//     cmp r2.x, r1.x, (1), r0.x
//     neg r0.x, r2.x
//     mov oc2.x, r2.x
//     add oc3.x, r0.x, (1)
//
    ps_2_0
    def c8, -2, 3, 1.8, -0.4
    def c9, 0.5, 1.5, 1, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mov r1.w, c7.x
    mad r1.x, t0.x, r1.w, -c6.x
    mul_sat r1.x, r1.x, c0.x
    mad r1.y, r1.x, c8.x, c8.y
    mul r1.x, r1.x, r1.x
    mul r1.x, r1.y, r1.x
    mad r1.x, r1.x, c8.z, c8.w
    mov r2.xyz, c1
    mad r1.xyz, r1.x, r2, c4
    add r2.xyz, -r1, c9.z
    max r1.w, r0.y, r0.z
    max r2.w, r0.x, r1.w
    min r1.w, r0.z, r0.y
    min r3.w, r1.w, r0.x
    add r1.w, r2.w, r3.w
    add r2.w, r2.w, -r3.w
    mul r2.w, r2.w, c5.x
    mov r0.x, c2.x
    mad r2.w, r2.w, r0.x, c3.x
    mad r0.x, r1.w, -c9.x, c9.y
    mad r0.xyz, r0.x, -r2, c9.z
    mul r1.xyz, r1, r1.w
    mad r2.x, r1.w, -c9.x, c9.x
    cmp r0.xyz, r2.x, r1, r0
    mad r0.xyz, r1.w, -c9.x, r0
    mul r1.x, r1.w, c9.x
    mad r1.xyz, r2.w, r0, r1.x
    mov r1.w, c9.z
    mul r0, r0.w, r1
    mul r0, r0, v0
    mov oC0, r0

// 32 instructions
