// PixelShader.Technique1[53] MagicMissile
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
//   sampler2D uImage0   s0
//   sampler2D uImage1   s1
//   sampler2D uImage2   s2
//   float     uOpacity  c0  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float uSaturation  c0  default (0, 0, 0, 0)
//   float uTime        c1  default (0, 0, 0, 0)
//
//     mul oc1.x, c0.x, c1.x
//
    ps_2_0
    def c2, 1.5, -0.5, 0, 0
    dcl v0
    dcl t0.xyz
    dcl_2d s0
    dcl_2d s1
    dcl_2d s2
    mad r0.w, t0.x, c2.x, c2.y
    rcp r0.x, t0.z
    mul r0.y, r0.x, t0.y
    add r0.x, t0.x, c1.x
    mov r0.z, t0.x
    mov r1.x, r0.z
    mov r1.y, r0.y
    texld r2, r0, s2
    texld r3, r0, s1
    texld r1, r1, s0
    add r0.x, -r0.w, r2.x
    add_sat r0.x, r0.x, r0.x
    mul r0.x, r3.x, r0.x
    mul r0, r1, r0.x
    mul r0, r0, v0
    mul r0, r0, c0.x
    mov oC0, r0

// 17 instructions
