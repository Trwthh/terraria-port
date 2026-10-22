// ScreenShader.Technique1[20] FilterCrystalDestructionColor
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor   c2  default (0, 0, 0, 0)
//   sampler2D uImage0  s0
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uColor      c0  default (0, 0, 0, 0)
//   float  uIntensity  c2  default (0, 0, 0, 0)
//   float  uOpacity    c1  default (0, 0, 0, 0)
//
//     neg r0.xyz, c0.xyz
//     add oc0.xyz, (1), r0.xyz
//     mul oc1.x, c1.x, c2.x
//
    ps_2_0
    def c3, 0.33333334, -2, 3, 1
    def c4, 0, 0, 0, 0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    add r0.w, r0.y, r0.x
    add r0.w, r0.z, r0.w
    mul_sat r0.w, r0.w, c3.x
    mad r1.w, r0.w, c3.y, c3.z
    mul r0.w, r0.w, r0.w
    mul r0.w, r1.w, r0.w
    mul r0.w, r0.w, r0.w
    mad r1.x, r0.w, -c3.y, -c3.w
    add r0.w, r0.w, r0.w
    mov_sat r0.w, r0.w
    mul r1.yzw, r1.x, c0.wzyx
    cmp r1.xyz, r1.x, r1.wzyx, c4.x
    mad r1.xyz, r0.w, c2, r1
    lrp r2.xyz, c1.x, r1, r0
    mov r2.w, c3.w
    mov oC0, r2

// 17 instructions
