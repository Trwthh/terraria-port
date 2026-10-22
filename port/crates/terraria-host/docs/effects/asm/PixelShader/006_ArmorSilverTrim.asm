// PixelShader.Technique1[6] ArmorSilverTrim
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0      s0
//   float     uSaturation  c2  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float uSaturation  c0  default (0, 0, 0, 0)
//
//     neg r0.x, c0.x
//     add r1.x, r0.x, (1)
//     rcp r0.x, c0.x
//     cmp r2.x, r1.x, (1), r0.x
//     neg r0.x, r2.x
//     mov oc0.x, r2.x
//     add oc1.x, r0.x, (1)
//
    ps_2_0
    def c3, 0.5, 1.5, 1, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    max r1.w, r0.y, r0.z
    max r2.w, r0.x, r1.w
    min r1.x, r0.z, r0.y
    min r2.x, r1.x, r0.x
    add r1.x, r2.w, r2.x
    add r1.y, r2.w, -r2.x
    mul r1.y, r1.y, c2.x
    mov r2.x, c0.x
    mad r1.y, r1.y, r2.x, c1.x
    mul r1.x, r1.x, c3.x
    mul r1.x, r1.y, r1.x
    mul r1.x, r0.w, r1.x
    mul r2.xyz, r0, r0
    mad r3.xyz, r1.x, c3.y, -r2
    mad r0.xyz, r1.y, r3, r2
    min r1, r0, c3.z
    mul r0, r1, v0
    mov oC0, r0

// 19 instructions
