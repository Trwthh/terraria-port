// PixelShader.Technique1[11] ArmorColoredRainbow
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0      s0
//   float2    uImageSize0  c5  default (0, 0, 0, 0)
//   float     uSaturation  c3  default (0, 0, 0, 0)
//   float4    uSourceRect  c4  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float  uSaturation  c0  default (0, 0, 0, 0)
//   float4 uSourceRect  c1  default (0, 0, 0, 0)
//
//     rcp oc0.x, c1.z
//     neg r0.x, c0.x
//     add r1.x, r0.x, (1)
//     rcp r0.x, c0.x
//     cmp r2.x, r1.x, (1), r0.x
//     neg r0.x, r2.x
//     mov oc1.x, r2.x
//     add oc2.x, r0.x, (1)
//
    ps_2_0
    def c6, -2, 3, -1, 1
    def c7, -0.4, 0.266, -0.067, 1.8
    def c8, 0.5, 1.5, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mov r1.w, c5.x
    mad r1.x, t0.x, r1.w, -c4.x
    mul_sat r1.x, r1.x, c0.x
    mad r1.y, r1.x, c6.x, c6.y
    mul r1.x, r1.x, r1.x
    mul r1.x, r1.y, r1.x
    mad r1.xyz, r1.x, c7.w, c7
    abs r2.xyz, r1
    frc r2.xyz, r2
    cmp r1.xyz, r1, r2, -r2
    mad r1.xyz, r1, c6.y, c6.z
    abs r1.xyz, r1
    add r1.xyz, -r1, c6.w
    add r2.xyz, -r1, c6.w
    max r1.w, r0.y, r0.z
    max r2.w, r0.x, r1.w
    min r1.w, r0.z, r0.y
    min r3.w, r1.w, r0.x
    add r1.w, r2.w, r3.w
    add r2.w, r2.w, -r3.w
    mul r2.w, r2.w, c3.x
    mov r0.x, c1.x
    mad r2.w, r2.w, r0.x, c2.x
    mad r0.x, r1.w, -c8.x, c8.y
    mad r0.xyz, r0.x, -r2, c6.w
    mul r1.xyz, r1, r1.w
    mad r2.x, r1.w, -c8.x, c8.x
    cmp r0.xyz, r2.x, r1, r0
    mad r0.xyz, r1.w, -c8.x, r0
    mul r1.x, r1.w, c8.x
    mad r1.xyz, r2.w, r0, r1.x
    mov r1.w, c6.w
    mul r0, r0.w, r1
    mul r0, r0, v0
    mov oC0, r0

// 36 instructions
