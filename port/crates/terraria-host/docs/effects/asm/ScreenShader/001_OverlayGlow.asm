// ScreenShader.Technique1[1] OverlayGlow
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3 uColor             c0  default (0, 0, 0, 0)
//   float  uOpacity           c2  default (0, 0, 0, 0)
//   float2 uScreenResolution  c1  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uScreenResolution  c0  default (0, 0, 0, 0)
//
//     mul r0.xy, (0.5), c0.xy
//     add oc3.xy, (-200), r0.xy
//
    ps_2_0
    def c4, 0, 500, 0.002, 0
    dcl t0.xy
    mov r0.xy, c1
    mad r0.xy, r0, t0, -c3
    dp2add r0.x, r0, r0, c4.x
    rsq r0.x, r0.x
    rcp r0.x, r0.x
    add r0.x, -r0.x, c4.y
    mul r0.y, r0.x, c4.z
    cmp r0.x, r0.x, r0.y, c4.x
    mul r0.yzw, r0.x, c0.wzyx
    mul r1.w, r0.x, c2.x
    mul r1.xyz, r0.wzyx, c2.x
    mov oC0, r1

// 12 instructions
