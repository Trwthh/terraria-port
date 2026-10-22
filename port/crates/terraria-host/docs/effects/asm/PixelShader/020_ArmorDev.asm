// PixelShader.Technique1[20] ArmorDev
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float     uDirection   c4  default (0, 0, 0, 0)
//   sampler2D uImage0      s0
//   float2    uImageSize0  c5  default (0, 0, 0, 0)
//   float4    uSourceRect  c3  default (0, 0, 0, 0)
//   float     uTime        c2  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize0  c1  default (0, 0, 0, 0)
//   float4 uSourceRect  c0  default (0, 0, 0, 0)
//
//     rcp r0.x, c1.x
//     add oc0.x, r0.x, r0.x
//     rcp oc1.x, c0.z
//
    ps_2_0
    def c6, 0, 6, 1, 0
    def c7, 0.15915494, 0.5, 6.2831855, -3.1415927
    def c8, 43758.547, -2, 3, 4
    def c9, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c10, -0.020833334, -0.125, 1, 0.5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    add r0.w, t0.y, c2.x
    mad r0.x, r0.w, c7.x, c7.y
    frc r0.x, r0.x
    mad r0.x, r0.x, c7.z, c7.w
    sincos r1.y, r0.x, c9, c10
    mul r0.x, r1.y, c8.x
    frc r0.x, r0.x
    mul r0.x, r0.x, c0.x
    mul r0.x, r0.x, c4.x
    add r0.y, r0.x, r0.x
    mov r1.x, c5.x
    mad r0.z, t0.x, r1.x, -c3.x
    mul_sat r0.z, r0.z, c1.x
    mad r0.w, r0.z, c8.y, c8.z
    mul r0.z, r0.z, r0.z
    mad r0.y, r0.w, r0.z, r0.y
    add r1.x, r0.y, -c0.x
    mov r1.y, t0.y
    mov r0.w, c8.w
    mad r2.x, r0.x, r0.w, -c0.x
    mov r0.y, c6.y
    mad r0.x, r0.x, r0.y, -c0.x
    mov r2.y, c6.x
    add r2.xy, r2, t0
    mov r0.y, c6.x
    add r0.xy, r0, t0
    texld r1, r1, s0
    texld r2, r2, s0
    texld r0, r0, s0
    mov r0.x, r1.x
    mov r0.y, r2.y
    mov r0.w, c6.z
    mul r0, r1.w, r0
    mul r0, r0, v0
    mov oC0, r0

// 35 instructions
