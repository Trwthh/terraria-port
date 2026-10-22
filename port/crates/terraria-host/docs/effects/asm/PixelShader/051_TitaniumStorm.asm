// PixelShader.Technique1[51] TitaniumStorm
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor    c2  default (0, 0, 0, 0)
//   sampler2D uImage0   s0
//   sampler2D uImage1   s1
//   float     uOpacity  c3  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uColor  c0  default (0, 0, 0, 0)
//   float  uTime   c1  default (0, 0, 0, 0)
//
//     mul oc0.x, c0.x, c1.x
//     neg r0.x, c0.z
//     add oc1.x, r0.x, (1)
//
    ps_2_0
    def c4, 0.5, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    texld r0, t0, s0
    mad r1.w, r0.x, c2.y, t0.x
    add r1.x, r1.w, c0.x
    add r1.x, r1.x, c3.x
    abs r1.y, r1.x
    frc r1.y, r1.y
    cmp r1.x, r1.x, r1.y, -r1.y
    mov r1.y, c4.x
    texld r1, r1, s1
    mul r2, r0, c1.x
    mad r1, r1, c2.z, r2
    mul r1, r1, v0
    mul r0, r0.w, r1
    mov oC0, r0

// 14 instructions
