// PixelShader.Technique1[10] ArmorBrightnessGradient
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor       c2  default (0, 0, 0, 0)
//   sampler2D uImage0      s0
//   float2    uImageSize0  c4  default (0, 0, 0, 0)
//   float4    uSourceRect  c3  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uColor           c0  default (0, 0, 0, 0)
//   float3 uSecondaryColor  c1  default (0, 0, 0, 0)
//   float4 uSourceRect      c2  default (0, 0, 0, 0)
//
//     rcp oc0.x, c2.z
//     neg r0.xyz, c0.xyz
//     add oc1.xyz, r0.xyz, c1.xyz
//
    ps_2_0
    def c5, 0.5, -2, 3, 0
    def c6, 1.8, -0.4, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mov r1.w, c4.x
    mad r1.x, t0.x, r1.w, -c3.x
    mul_sat r1.x, r1.x, c0.x
    mad r1.y, r1.x, c5.y, c5.z
    mul r1.x, r1.x, r1.x
    mul r1.x, r1.y, r1.x
    mad r1.x, r1.x, c6.x, c6.y
    mov r2.xyz, c1
    mad r1.xyz, r1.x, r2, c2
    add r1.w, r0.y, r0.x
    add r1.w, r0.z, r1.w
    mul r1.w, r1.w, c5.x
    mul r1.xyz, r1, r1.w
    mul r0.xyz, r0.w, r1
    mul r0, r0, v0
    mov oC0, r0

// 17 instructions
