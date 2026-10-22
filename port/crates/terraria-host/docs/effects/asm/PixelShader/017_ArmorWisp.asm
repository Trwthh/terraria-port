// PixelShader.Technique1[17] ArmorWisp
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor           c2  default (0, 0, 0, 0)
//   sampler2D uImage0          s0
//   float3    uSecondaryColor  c3  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uColor           c0  default (0, 0, 0, 0)
//   float3 uSecondaryColor  c1  default (0, 0, 0, 0)
//
//     neg r0.xyz, c1.xyz
//     add oc0.xyz, r0.xyz, c0.xyz
//     neg r0.xyz, c0.xyz
//     add oc1.xyz, (1), r0.xyz
//
    ps_2_0
    def c4, 0.33333334, -0.2, -0.4, 5
    def c5, 1, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    add r0.x, r0.y, r0.x
    add r0.x, r0.z, r0.x
    mad r0.y, r0.x, c4.x, c4.z
    mad r0.x, r0.x, c4.x, c4.y
    mul r0.z, r0.y, c4.w
    min r1.w, r0.z, c5.x
    mov r1.xyz, c1
    mad r1.xyz, r1.w, r1, c2
    mul r1.w, r0.x, c4.w
    mov r2.xyz, c3
    mad r2.xyz, r1.w, c0, r2
    cmp r1.xyz, r0.y, r1, r2
    cmp r1.xyz, r0.x, r1, c3
    mov r1.w, c5.x
    mul r0, r0.w, r1
    min r1, r0, c5.x
    max r0.xyz, v0, v0.w
    lrp r2.xyz, -c4.y, r0, v0
    mov r2.w, v0.w
    mul r0, r1, r2
    mov oC0, r0

// 22 instructions
