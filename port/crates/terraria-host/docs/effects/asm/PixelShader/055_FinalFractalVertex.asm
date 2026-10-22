// PixelShader.Technique1[55] FinalFractalVertex
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
//   float     uOpacity             c0  default (0, 0, 0, 0)
//   float4    uShaderSpecificData  c1  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float4 uShaderSpecificData  c0  default (0, 0, 0, 0)
//
//     rcp oc2.x, c0.w
//
    ps_2_0
    def c3, 1.5, -0.5, 1, 0
    dcl v0
    dcl t0.xyz
    dcl_2d s0
    dcl_2d s1
    mad r0.w, t0.x, c3.x, c3.y
    mad r0.x, t0.x, c1.x, c1.y
    abs r0.y, r0.x
    frc r0.y, r0.y
    cmp r1.x, r0.x, r0.y, -r0.y
    mov r0.z, c3.z
    mad r0.x, r0.x, -c2.x, r0.z
    rcp r0.y, t0.z
    mul r1.y, r0.y, t0.y
    texld r2, r1, s1
    texld r1, r1, s0
    mul r1, r1.x, v0
    mul r1, r1, c0.x
    add r0.y, -r0.w, r2.x
    add_sat r0.y, r0.y, r0.y
    mul r1, r1, r0.y
    mul r0, r0.x, r1
    mov oC0, r0

// 18 instructions
