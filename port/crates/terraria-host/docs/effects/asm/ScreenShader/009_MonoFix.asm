// ScreenShader.Technique1[9] MonoFix
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uColor             c0  default (0, 0, 0, 0)
//   float  uOpacity           c5  default (0, 0, 0, 0)
//   float2 uScreenPosition    c3  default (0, 0, 0, 0)
//   float2 uScreenResolution  c2  default (0, 0, 0, 0)
//   float3 uSecondaryColor    c1  default (0, 0, 0, 0)
//   float2 uTargetPosition    c4  default (0, 0, 0, 0)
//   float  uTime              c6  default (0, 0, 0, 0)
//
//     mul r0.xyz, c0.xyz, c1.xyz
//     mul r1.xy, r0.xy, c2.xy
//     mul r0.xy, r1.xy, c3.xy
//     mul oc0.xy, r0.xy, c4.xy
//     mov r0.x, c5.x
//     mul r1.x, r0.x, c5.x
//     mul r2.x, r0.z, r1.x
//     mul oc0.z, r2.x, c5.x
//     mov oc0.w, c6.x
//
    ps_2_0
    mov oC0, c0

// 1 instructions
