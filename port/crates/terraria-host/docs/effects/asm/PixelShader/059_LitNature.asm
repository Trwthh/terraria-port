// PixelShader.Technique1[59] LitNature
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
//   float3    uColor               c8  default (0, 0, 0, 0)
//   float2    uDrawPosition        c10  default (0, 0, 0, 0)
//   sampler2D uImage0              s0
//   sampler2D uImage1              s1
//   float2    uImageSize0          c11  default (0, 0, 0, 0)
//   float4    uShaderSpecificData  c12  default (0, 0, 0, 0)
//   float4    uSourceRect          c9  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize0          c1  default (0, 0, 0, 0)
//   float4 uShaderSpecificData  c2  default (0, 0, 0, 0)
//   float4 uSourceRect          c0  default (0, 0, 0, 0)
//
//     add oc0.xy, c0.zw, c0.xy
//     add r0.x, c2.w, (-8)
//     cmp r1.x, r0.x, r0.x, c2.w
//     mov oc1.x, r0.x
//     add r0.x, r1.x, (-4)
//     cmp r2.x, r0.x, r0.x, r1.x
//     mov oc2.x, r0.x
//     add r0.x, r2.x, (-2)
//     cmp r1.x, r0.x, r0.x, r2.x
//     mov oc3.x, r0.x
//     add oc4.x, r1.x, (-1)
//     mov oc5.x, r1.x
//     rcp r0.x, c1.x
//     rcp r0.y, c1.y
//     add r1.xy, r0.xy, r0.xy
//     mov oc6.x, r1.y
//     mov oc7.x, r1.x
//
    ps_2_0
    def c13, 0.5, 100, 0.25, 0
    def c14, 0.00041666668, 1, 0.05, -0.3
    def c15, 3, 2, 0, 0
    dcl v0
    dcl t0.xy
    dcl t1.xy
    dcl_2d s0
    dcl_2d s1
    mov r0.x, t0.x
    add r0.y, t0.y, -c6.x
    mov r1.x, t0.x
    add r1.y, t0.y, c6.x
    mov r2.x, -c7.x
    mov r2.y, c13.w
    add r2.xy, r2, t0
    add r3.x, t0.x, c7.x
    mov r3.y, t0.y
    mov r4.xy, c13.x
    texld r0, r0, s0
    texld r1, r1, s0
    texld r2, r2, s0
    texld r3, r3, s0
    texld r4, r4, s1
    texld r5, t0, s0
    min r2.x, r1.w, r0.w
    min r0.x, r2.w, r2.x
    min r1.x, r3.w, r0.x
    add r0.x, -r1.x, c14.y
    add r1.xy, t1, -c10
    dp2add r0.y, r1, r1, c13.w
    rsq r0.y, r0.y
    mul r1.xy, r1, r0.y
    add r2.xy, -t1, c12
    dp2add r0.y, r2, r2, c13.w
    rsq r0.y, r0.y
    mul r2.xy, r2, r0.y
    rcp r0.y, r0.y
    dp2add r0.z, r1, r2, c14.w
    mul_sat r0.z, r0.z, c15.x
    mad r0.x, r0.x, c15.y, r0.z
    mul r0.z, r4.w, c12.z
    mul r0.x, r0.x, r0.z
    mul_sat r0.z, r0.y, c14.z
    mad r0.y, r0.y, -c14.x, c14.y
    mul r0.z, r0.z, r0.y
    cmp r0.y, r0.y, r0.z, c13.w
    mul r0.y, r0.y, r0.y
    mul r0.x, r0.x, r0.y
    mul r0.yz, t0.zxyw, c11.zxyw
    frc r1.xy, r0.yzxw
    add r0.yz, r0, -r1.zxyw
    add r0.yz, r0, c13.x
    add r1.xy, r0.yzxw, -c9
    add r0.yz, -r0, c0.zxyw
    min r1.z, r0.z, c13.y
    mov r2.yw, c13
    cmp r0.z, c1.x, r1.z, r2.y
    min r2.x, r1.y, r0.z
    cmp r0.z, c2.x, r2.x, r0.z
    min r1.y, r0.y, r0.z
    cmp r0.y, c3.x, r1.y, r0.z
    min r2.x, r1.x, r0.y
    cmp r0.y, c4.x, r2.x, r0.y
    frc r0.z, r0.y
    add r0.y, r0.y, -r0.z
    mul r0.y, r0.y, c13.z
    cmp r0.y, c5.x, r0.y, r2.w
    mul r0.x, r0.x, r0.y
    mul r1.xyz, r5.w, c8
    mov r1.w, r5.w
    mad r0, r1, r0.x, r5
    mul r0, r0, v0
    mov oC0, r0

// 65 instructions
