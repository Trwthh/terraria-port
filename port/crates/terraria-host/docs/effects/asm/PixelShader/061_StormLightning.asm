// PixelShader.Technique1[61] StormLightning
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
//   sampler2D uImage0      s0
//   float     uSaturation  c0  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float uOpacity  c0  default (0, 0, 0, 0)
//
//     mul r0.x, c0.x, (0.7)
//     add oc1.x, r0.x, (0.3)
//
    ps_2_0
    def c2, 8, 0, 0, 0
    dcl v0
    dcl t0.xyz
    dcl_2d s0
    rcp r0.w, t0.z
    mul r0.y, r0.w, t0.y
    mov r0.x, t0.x
    texld r0, r0, s0
    mul r0.x, r0.x, c2.x
    mul r0.x, r0.x, c1.x
    mul r0.x, r0.x, c0.x
    mul r0, r0.x, v0
    mov oC0, r0

// 9 instructions
