// PixelShader.Technique1[21] ArmorPlaid
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0      s0
//   float2    uImageSize0  c1  default (0, 0, 0, 0)
//   float4    uSourceRect  c0  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float4 uSourceRect  c0  default (0, 0, 0, 0)
//
//     rcp oc2.x, c0.z
//
    ps_2_0
    def c3, -2, 3, 0.5, 0.1
    def c4, 10, 0.12732396, 0.5, -0.2
    def c5, 6.2831855, -3.1415927, 0.33333334, 1
    def c6, 0, 0, 0, 0
    def c7, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c8, -0.020833334, -0.125, 1, 0.5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mov r1.xy, c1
    mad r1.yz, t0.zxyw, r1.zxyw, -c0.zxyw
    mad r1.z, r1.z, c4.y, c4.z
    mad r1.x, r1.y, r1.x, -c0.x
    mul_sat r1.x, r1.x, c2.x
    frc r1.y, r1.z
    mad r1.y, r1.y, c5.x, c5.y
    sincos r2.x, r1.y, c7, c8
    mad r1.y, r2.x, c4.z, c4.w
    mul_sat r1.y, r1.y, c4.x
    mad r1.z, r1.y, c3.x, c3.y
    mul r1.y, r1.y, r1.y
    mul r1.y, r1.z, r1.y
    mul r2.y, r1.y, c3.z
    mad r1.y, r1.x, c3.x, c3.y
    mul r1.x, r1.x, r1.x
    mul r1.x, r1.y, r1.x
    sincos r3.y, r1.x, c7, c8
    mad r1.x, r3.y, c3.z, c3.w
    mul_sat r1.x, r1.x, c4.x
    mad r1.y, r1.x, c3.x, c3.y
    mul r1.x, r1.x, r1.x
    mad r2.x, r1.y, r1.x, -r2.y
    rcp r1.x, r0.w
    mul r0.xy, r0, r1.x
    add r0.x, r0.y, r0.x
    mad r0.x, r0.z, r1.x, r0.x
    mul r0.xy, r0.x, c5.z
    mul r1.xy, r2, r0
    mov r1.w, c5.w
    mov r0.xy, v0
    mov r0.z, v0.w
    mul r1.xy, r1, r0
    mul r1.z, r1.w, r0.z
    mul r2.xy, r0.w, r1
    mul r2.w, r0.w, r1.z
    mov r2.z, c6.x
    mov oC0, r2

// 39 instructions
