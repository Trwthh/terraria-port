// PixelShader.Technique1[57] RainbowTownSlime
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0  s0
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize0  c2  default (0, 0, 0, 0)
//   float4 uSourceRect  c1  default (0, 0, 0, 0)
//   float  uTime        c0  default (0, 0, 0, 0)
//
//     rcp r0.x, c2.x
//     rcp r0.y, c2.y
//     mul oc0.xy, r0.xy, c1.xy
//     mul r1.xy, r0.xy, c1.zw
//     rcp oc1.x, r1.x
//     rcp oc1.y, r1.y
//     mul oc2.x, c0.x, (0.16)
//
    ps_2_0
    def c3, -0.5, 0.33, 1, -1
    def c4, 3.3333333, 6, -3, -2
    def c5, 6, -4, 0.65, 0.25
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    add r1.xy, t0, -c0
    mov r2.w, c3.x
    mad r1.xy, r1, c1, r2.w
    mul r1.y, r1.y, c3.y
    mad r1.x, r1.x, c3.y, r1.y
    add r1.x, r1.x, c2.x
    cmp r1.y, r1.x, c3.z, c3.w
    mul r1.x, r1.x, r1.y
    frc r1.x, r1.x
    mad r1.x, r1.y, -r1.x, c3.z
    mad r1.y, r1.x, c4.y, c4.z
    abs r1.y, r1.y
    add_sat r2.x, r1.y, c3.w
    mad r1.y, r1.x, c4.y, c4.w
    mad r1.x, r1.x, c5.x, c5.y
    abs r1.x, r1.x
    add_sat r2.z, -r1.x, -c4.w
    abs r1.x, r1.y
    add_sat r2.y, -r1.x, -c4.w
    mov r2.w, c3.z
    lrp r1, c5.z, r0, r2
    mad r1, r0, c5.w, r1
    mul_sat r2.x, r0.x, c4.x
    lrp r3, r2.x, r1, r0
    mul r1, r3, v0
    mul r0, r0.w, r1
    mov oC0, r0

// 28 instructions
