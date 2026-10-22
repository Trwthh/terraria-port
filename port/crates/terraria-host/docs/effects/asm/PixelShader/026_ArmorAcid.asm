// PixelShader.Technique1[26] ArmorAcid
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor       c2  default (0, 0, 0, 0)
//   sampler2D uImage0      s0
//   float2    uImageSize0  c4  default (0, 0, 0, 0)
//   float4    uSourceRect  c3  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float4 uSourceRect  c1  default (0, 0, 0, 0)
//   float  uTime        c0  default (0, 0, 0, 0)
//
//     rcp oc0.x, c1.z
//     rcp oc0.y, c1.w
//     mul oc1.x, c0.x, (4)
//
    ps_2_0
    def c5, 0.33333334, 0, -0.5, 0.1
    def c6, 5, 20, 0.31830996, 3.141592
    def c7, 1.5, 1, 2, 0.1
    def c8, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c9, -0.020833334, -0.125, 1, 0.5
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mov r1.xy, c4
    mad r1.xy, t0, r1, -c3
    mov r2.yz, c5
    mad r1.xy, r1, c0, r2.yzxw
    add r2.x, r1.x, c5.z
    add r0.x, r0.y, r0.x
    add r0.x, r0.z, r0.x
    mul r0.x, r0.x, c5.x
    rsq r0.x, r0.x
    rcp r0.x, r0.x
    mad r2.y, r0.x, c5.w, r1.y
    dp2add r0.y, r2, r2, c5.y
    rsq r0.y, r0.y
    rcp r0.y, r0.y
    mul r0.z, r0.x, c6.x
    mad r0.y, r0.y, c6.y, -r0.z
    add r0.y, r0.y, -c1.x
    mul r0.z, r0.y, c6.z
    abs r0.z, r0.z
    frc r0.z, r0.z
    cmp r0.y, r0.y, r0.z, -r0.z
    mul r0.y, r0.y, c6.w
    sincos r1.y, r0.y, c8, c9
    add r0.y, r1.y, c7.x
    mul r0.z, r0.x, r0.x
    mad r0.z, r0.z, -r0.x, c7.y
    mul r1.xyz, r0.x, c2
    mad r1.w, r0.y, -r0.z, c7.y
    mul r1.xyz, r1, r1.w
    mov r1.w, c7.y
    mul r0, r0.w, r1
    mad r1.x, v0.w, c7.z, c7.w
    min r2, r1.x, r0
    min r0, r2, c7.y
    max r1.xyz, v0, v0.w
    lrp r2.xyz, -c5.z, r1, v0
    mov r2.w, v0.w
    mul r0, r0, r2
    mov oC0, r0

// 40 instructions
