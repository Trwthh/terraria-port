// PixelShader.Technique1[15] ArmorLivingOcean
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0      s0
//   float2    uImageSize0  c3  default (0, 0, 0, 0)
//   float4    uSourceRect  c2  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float4 uSourceRect  c1  default (0, 0, 0, 0)
//   float  uTime        c0  default (0, 0, 0, 0)
//
//     rcp oc0.x, c1.z
//     mul oc1.x, c0.x, (0.1)
//
    ps_2_0
    def c4, 0.33333334, -2, 3, -0.5
    def c5, 5, 0.99949306, 0.5, 0
    def c6, 0, 1, 1, 0
    def c7, 6.2831855, -3.1415927, 0.2, 0.6
    def c8, 0, -0.9, 0, 0
    def c9, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c10, -0.020833334, -0.125, 1, 0.5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mov r1.w, c3.x
    mad r1.x, t0.x, r1.w, -c2.x
    mul_sat r1.x, r1.x, c0.x
    mad r1.y, r1.x, c4.y, c4.z
    mul r1.x, r1.x, r1.x
    mad r1.x, r1.y, r1.x, c4.w
    add r1.y, r0.y, r0.x
    add r1.y, r0.z, r1.y
    mul r1.y, r1.y, c4.x
    mul r1.x, r1.x, r1.y
    mov r1.w, c4.w
    mad r1.x, r1.x, -r1.w, c1.x
    mul r1.z, r1.x, c5.x
    abs r1.z, r1.z
    frc r1.z, r1.z
    cmp r1.x, r1.x, r1.z, -r1.z
    mad r1.x, r1.x, c5.y, c5.z
    frc r1.x, r1.x
    mad r1.x, r1.x, c7.x, c7.y
    sincos r2.x, r1.x, c9, c10
    mad r1.x, r2.x, -c7.z, c7.w
    mov r2.xyz, c8
    mad r2.xyz, r1.x, r2, c6
    mul r1.xyz, r1.y, r2
    mul r0.xyz, r0.w, r1
    mul r0, r0, v0.w
    mov oC0, r0

// 28 instructions
