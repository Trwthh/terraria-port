// PixelShader.Technique1[3] ArmorColored
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor       c3  default (0, 0, 0, 0)
//   sampler2D uImage0      s0
//   float     uSaturation  c4  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uColor       c0  default (0, 0, 0, 0)
//   float  uSaturation  c1  default (0, 0, 0, 0)
//
//     neg r0.xyz, c0.xyz
//     add oc0.xyz, (1), r0.xyz
//     neg r0.x, c1.x
//     add r1.x, r0.x, (1)
//     rcp r0.x, c1.x
//     cmp r2.x, r1.x, (1), r0.x
//     neg r0.x, r2.x
//     mov oc1.x, r2.x
//     add oc2.x, r0.x, (1)
//
    ps_2_0
    def c5, 0.5, 1.5, 1, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    max r1.w, r0.y, r0.z
    max r2.w, r0.x, r1.w
    min r1.x, r0.z, r0.y
    min r2.x, r1.x, r0.x
    add r0.x, r2.w, r2.x
    add r0.y, r2.w, -r2.x
    mul r0.y, r0.y, c4.x
    mov r1.x, c1.x
    mad r0.y, r0.y, r1.x, c2.x
    mad r0.z, r0.x, -c5.x, c5.y
    mov r1.z, c5.z
    mad r1.xyz, r0.z, -c0, r1.z
    mad r1.w, r0.x, -c5.x, c5.x
    mul r2.xyz, r0.x, c3
    cmp r1.xyz, r1.w, r2, r1
    mad r1.xyz, r0.x, -c5.x, r1
    mul r1.w, r0.x, c5.x
    mad r1.xyz, r0.y, r1, r1.w
    mov r1.w, c5.z
    mul r0, r0.w, r1
    mul r0, r0, v0
    mov oC0, r0

// 23 instructions
