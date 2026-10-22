// PixelShader.Technique1[58] HorizonClouds
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
    mov oT0.xy, v2
    mov oT1.xy, v0

// 7 instructions

// ---- pixel shader
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor               c0  default (0, 0, 0, 0)
//   float2    uDrawPosition        c1  default (0, 0, 0, 0)
//   sampler2D uImage0              s0
//   float4    uShaderSpecificData  c2  default (0, 0, 0, 0)
//
    ps_2_0
    def c3, 0, 0.02, 1, 0.3
    def c4, 0.00055555557, 1.3, 2, -1
    def c5, 0.016666668, 0, -0.035, -0.62
    def c6, 0, 0.14, -0.035, 4
    def c7, 0.035, 0, 0.2, 1
    def c8, 0.6, -0.05, 0.5, 0
    dcl v0
    dcl t0.xy
    dcl t1.xy
    dcl_2d s0
    add r0.xy, t0, c5.yzxw
    add r1.xy, t0, c6
    add r2.xy, t0, c6.zxyw
    add r3.xy, t0, c7
    texld r0, r0, s0
    texld r1, r1, s0
    texld r2, r2, s0
    texld r3, r3, s0
    texld r4, t0, s0
    add r0.x, r0.w, r1.w
    add r0.x, r2.w, r0.x
    add r0.x, r3.w, r0.x
    add r0.x, r4.w, r0.x
    mad r0.x, r0.x, -c7.z, c7.w
    add r0.y, r4.x, c5.w
    mul_sat r0.y, r0.y, c6.w
    add r0.y, -r0.y, c3.z
    max r1.x, r0.y, r0.x
    add r0.xy, t1, -c1
    dp2add r0.z, r0, r0, c3.x
    rsq r0.z, r0.z
    mul r0.xy, r0, r0.z
    add r2.xy, -t1, c2
    dp2add r0.z, r2, r2, c3.x
    rsq r0.z, r0.z
    mul r2.xy, r2, r0.z
    rcp r0.z, r0.z
    dp2add r0.x, r0, r2, c3.x
    mul r0.y, r0.z, c5.x
    mad r0.y, r0.y, -r0.y, c3.z
    max r1.y, r0.x, r0.y
    mad r0.xy, t0, c4.z, c4.w
    dp2add r0.x, r0, r0, c3.x
    rsq r0.x, r0.x
    rcp r0.x, r0.x
    mad r0.y, r1.y, r0.x, -c3.w
    mad_sat r0.x, r1.y, r0.x, c8.y
    mad r0.y, r1.x, c8.x, r0.y
    mad r0.w, r0.z, -c3.y, c3.z
    mad r0.z, r0.z, -c4.x, c4.y
    max r1.y, c3.w, r0.z
    cmp r0.z, r0.w, -r0.w, -c3.x
    add r0.y, r0.y, r0.z
    mad r0.x, r0.x, r1.x, r0.y
    mul r0.x, r0.x, c2.z
    mul r0.y, r1.y, r1.y
    mul r0.y, r1.y, r0.y
    mul_sat r0.x, r0.x, r0.y
    add r1.xyz, -v0, c0
    add r1.w, -v0.w, c3.z
    mad r1, r1, c8.z, v0
    mul r1, r4.w, r1
    mul r2, r4, v0
    mad r1, r1, v0.w, -r2
    mad r0, r0.x, r1, r2
    mov oC0, r0

// 56 instructions
