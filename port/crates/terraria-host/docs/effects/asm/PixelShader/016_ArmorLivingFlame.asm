// PixelShader.Technique1[16] ArmorLivingFlame
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor           c0  default (0, 0, 0, 0)
//   sampler2D uImage0          s0
//   float2    uImageSize0      c4  default (0, 0, 0, 0)
//   float3    uSecondaryColor  c1  default (0, 0, 0, 0)
//   float4    uSourceRect      c3  default (0, 0, 0, 0)
//   float     uTime            c2  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float4 uSourceRect  c0  default (0, 0, 0, 0)
//
//     rcp oc5.x, c0.z
//
    ps_2_0
    def c6, 0.33333334, -2, 3, 0.6666667
    def c7, 0.7957747, 0.5, 6.2831855, -3.1415927
    def c8, 0.4, 0.6, 0, 0
    def c9, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c10, -0.020833334, -0.125, 1, 0.5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mov r1.w, c4.x
    mad r1.x, t0.x, r1.w, -c3.x
    mul_sat r1.x, r1.x, c5.x
    mad r1.y, r1.x, c6.y, c6.z
    mul r1.x, r1.x, r1.x
    mul r1.x, r1.y, r1.x
    add r1.y, r0.y, r0.x
    add r1.y, r0.z, r1.y
    mad r1.x, r1.y, c6.w, r1.x
    mul r1.y, r1.y, c6.x
    add r1.x, r1.x, c2.x
    abs r1.z, r1.x
    frc r1.z, r1.z
    cmp r1.x, r1.x, r1.z, -r1.z
    mad r1.x, r1.x, c7.x, c7.y
    frc r1.x, r1.x
    mad r1.x, r1.x, c7.z, c7.w
    sincos r2.x, r1.x, c9, c10
    mad r1.x, r2.x, -c7.y, c7.y
    mad r2.xyz, v0, c8.x, c8.y
    mov r3.xyz, c0
    mad r2.xyz, c1, r2, -r3
    mad r2.xyz, r1.x, r2, c0
    mul r0.xyz, r1.y, r2
    mul r0, r0, v0.w
    mov oC0, r0

// 27 instructions
