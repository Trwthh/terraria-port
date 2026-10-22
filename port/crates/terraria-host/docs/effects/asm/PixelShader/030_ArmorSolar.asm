// PixelShader.Technique1[30] ArmorSolar
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor   c4  default (0, 0, 0, 0)
//   sampler2D uImage0  s0
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uColor           c0  default (0, 0, 0, 0)
//   float2 uImageSize0      c3  default (0, 0, 0, 0)
//   float3 uSecondaryColor  c1  default (0, 0, 0, 0)
//   float  uTime            c2  default (0, 0, 0, 0)
//
//     mul r0.x, c2.x, (0.477464829275686)
//     add r1.x, r0.x, (0.5)
//     frc r0.x, r1.x
//     mul r1.x, r0.x, (6.283185307179586)
//     add r0.x, r1.x, (-3.141592653589793)
//     sin r1.y, r0.x
//     mul r0.x, r1.y, (0.2)
//     add oc2.x, r0.x, (1)
//     neg r0.xyz, c0.xyz
//     add oc3.xyz, r0.xyz, c1.xyz
//     rcp r0.x, c3.x
//     rcp r0.y, c3.y
//     add r1.xy, r0.xy, r0.xy
//     mov oc0.x, r1.y
//     mov oc1.x, r1.x
//
    ps_2_0
    def c5, -0.05, -0.56, 0.5, 1.5384616
    def c6, 0, 0.33333334, 1, -0.3
    def c7, -0.46153846, -2, 3, 0.7692308
    def c8, -1.3, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    mov r0.x, -c1.x
    mov r0.y, c6.x
    add r0.xy, r0, t0
    add r1.x, t0.x, c1.x
    mov r1.y, t0.y
    mov r2.x, t0.x
    add r2.y, t0.y, c0.x
    mov r3.x, t0.x
    add r3.y, t0.y, -c0.x
    texld r0, r0, s0
    texld r1, r1, s0
    texld r2, r2, s0
    texld r3, r3, s0
    texld r4, t0, s0
    add r0.xyz, r0, -r1
    add r2.w, r0.y, r0.x
    add r2.w, r0.z, r2.w
    mul r0.x, r2.w, c6.y
    add r1.xyz, -r2, r3
    add r0.w, r1.y, r1.x
    add r0.w, r1.z, r0.w
    mul r0.y, r0.w, c6.y
    mul r0.w, r0.y, r0.y
    mad r0.w, r0.x, r0.x, r0.w
    add r0.w, -r0.w, c6.z
    rsq r1.x, r0.w
    rcp r1.x, r1.x
    cmp r0.z, r0.w, r1.x, c6.x
    dp3 r0.x, r0, c5
    add r0.y, r0.x, c6.w
    mul r0.y, r0.y, c5.w
    cmp_sat r0.x, r0.x, r0.y, c7.x
    mad r0.y, r0.x, c7.y, c7.z
    mul r0.x, r0.x, r0.x
    mul r0.x, r0.y, r0.x
    add r0.x, r0.x, r0.x
    mul r0.y, r0.x, c2.x
    mov r1.x, c2.x
    mad r0.x, r0.x, r1.x, c8.x
    add_sat r0.x, r0.x, r0.x
    mul r0.z, r0.y, -c6.w
    max r1.xyz, v0, v0.w
    lrp r2.xyz, r0.z, r1, v0
    mad r0.z, r0.x, c7.y, c7.z
    mul r0.x, r0.x, r0.x
    mul r0.x, r0.z, r0.x
    mul_sat r0.z, r0.y, c7.w
    mad r0.w, r0.z, c7.y, c7.z
    mul r0.z, r0.z, r0.z
    mul r0.z, r0.w, r0.z
    mov r1.xyz, c4
    mad r1.xyz, r0.z, c3, r1
    lrp r3.xyz, r0.x, c6.z, r1
    mul r3.w, r4.w, r0.y
    mul r0.xyz, r3, r3.w
    mov r0.w, r4.w
    min r1, r0, c6.z
    mov r2.w, v0.w
    mul r0, r2, r1
    add r1.x, r4.y, r4.x
    add r1.x, r4.z, r1.x
    mul r1.x, r1.x, c6.y
    mul r1.xyz, r1.x, c4
    mul r1.xyz, r1, c5.z
    mad r4.xyz, r4, c5.z, r1
    mad r0, r4, v0, r0
    mov oC0, r0

// 67 instructions
