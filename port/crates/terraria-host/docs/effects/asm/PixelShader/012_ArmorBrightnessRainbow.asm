// PixelShader.Technique1[12] ArmorBrightnessRainbow
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
    def c3, 0.5, -2, 3, -1
    def c4, -0.4, 0.266, -0.067, 1.8
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mov r1.w, c1.x
    mad r1.x, t0.x, r1.w, -c0.x
    mul_sat r1.x, r1.x, c2.x
    mad r1.y, r1.x, c3.y, c3.z
    mul r1.x, r1.x, r1.x
    mul r1.x, r1.y, r1.x
    mad r1.xyz, r1.x, c4.w, c4
    abs r2.xyz, r1
    frc r2.xyz, r2
    cmp r1.xyz, r1, r2, -r2
    mad r1.xyz, r1, c3.z, c3.w
    abs r1.xyz, r1
    add r1.xyz, -r1, -c3.w
    add r1.w, r0.y, r0.x
    add r1.w, r0.z, r1.w
    mul r1.w, r1.w, c3.x
    mul r1.xyz, r1, r1.w
    mul r0.xyz, r0.w, r1
    mul r0, r0, v0
    mov oC0, r0

// 21 instructions
