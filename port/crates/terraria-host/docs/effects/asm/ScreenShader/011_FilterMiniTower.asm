// ScreenShader.Technique1[11] FilterMiniTower
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor    c0  default (0, 0, 0, 0)
//   sampler2D uImage0   s0
//   float     uOpacity  c1  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uColor  c0  default (0, 0, 0, 0)
//
//     neg r0.xyz, c0.xyz
//     add oc2.xyz, (1), r0.xyz
//
    ps_2_0
    def c3, 0.5, -0.5, -1, 0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    max r1.w, r0.y, r0.z
    max r2.w, r0.x, r1.w
    min r1.x, r0.z, r0.y
    min r2.x, r1.x, r0.x
    add r1.x, r2.w, r2.x
    add r1.y, r1.x, c3.z
    mov r2.xyz, c0
    mad r1.yzw, r1.y, c2.wzyx, r2.wzyx
    mad r2.x, r1.x, c3.x, c3.y
    mul r2.yzw, r1.x, c0.wzyx
    cmp r1.xyz, r2.x, r1.wzyx, r2.wzyx
    mad r1.xyz, r1, r0.w, -r0
    mad r0.xyz, c1.x, r1, r0
    mov oC0, r0

// 15 instructions
