// PixelShader.Technique1[60] Aurora
//
// ---- vertex shader
// Constant table (vs_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float4x4 MatrixTransform  c16..c19  default (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)
//
    vs_2_0
    dcl_position v0
    dcl_color v1
    dcl_texcoord v2
    dp4 oPos.x, v0, c16
    dp4 oPos.y, v0, c17
    dp4 oPos.z, v0, c18
    dp4 oPos.w, v0, c19
    mov oD0, v1
    mov oT0.xyz, v2

// 6 instructions

// ---- pixel shader
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0              s0
//   sampler2D uImage1              s1
//   float4    uShaderSpecificData  c4  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float4 uShaderSpecificData  c1  default (0, 0, 0, 0)
//   float  uTime                c0  default (0, 0, 0, 0)
//
//     add r0.x, c0.x, c0.x
//     add r1.x, r0.x, c1.y
//     mul oc0.x, r1.x, (-0.01)
//     mul oc1.x, r1.x, (0.01)
//     mul oc2.x, r1.x, (-0.02)
//     mul oc3.x, r1.x, (0.02)
//
    ps_2_0
    def c5, 1.2, 3, 6, 1
    def c6, 0, 20, 0.7, 1
    def c7, 0.1, 0.7, 0.3, 0.9
    dcl v0.xyz
    dcl t0.xyz
    dcl_2d s0
    dcl_2d s1
    mov r0.y, c3.x
    add r0.z, t0.x, c4.y
    mov r0.w, c6.y
    mad r0.x, r0.z, r0.w, c2.x
    mov r0.w, c5.z
    mad r1.x, r0.z, r0.w, c0.x
    rcp r0.z, t0.z
    mul r2.y, r0.z, t0.y
    mov r1.y, c1.x
    mov r2.x, t0.x
    texld r3, r0, s0
    texld r1, r1, s0
    texld r4, r2, s1
    mul r0.x, r3.x, r3.x
    mul r0.x, r3.x, r0.x
    mad_sat r0.x, r0.x, -c6.z, c6.w
    mad r0.y, t0.y, r0.z, c7.x
    lrp r1.y, r0.y, c5.w, r0.x
    add r0.x, -r1.y, c5.w
    mad r0.x, c4.y, r0.x, r1.y
    mul r0.y, r2.y, c4.y
    lrp r1.y, r0.y, c5.w, r0.x
    mad r0.x, r1.y, c7.w, c7.x
    mul r0.z, r1.x, r1.x
    mad r0.z, r0.z, -r0.z, c5.w
    max r1.x, r0.z, c6.x
    add r0.z, -r1.x, c5.w
    mad r0.z, c4.y, r0.z, r1.x
    lrp r1.x, r0.y, c5.w, r0.z
    mad r0.y, r1.x, c7.y, c7.z
    mul r0.x, r0.x, r0.y
    mul_sat r0.y, r2.y, c5.y
    pow r0.z, r4.x, c5.x
    mul r0.y, r0.y, r0.z
    mul r0.x, r0.x, r0.y
    mul_sat r0.yzw, v0.wzyx, c4.x
    add r0.yzw, r0, v0.wzyx
    mul r0.xyz, r0.x, r0.wzyx
    mov r0.w, c6.x
    mov oC0, r0

// 40 instructions
