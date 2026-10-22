// ScreenShader.Technique1[17] FilterHeatDistortion
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0            s0
//   sampler2D uImage1            s1
//   float2    uScreenPosition    c5  default (0, 0, 0, 0)
//   float2    uScreenResolution  c4  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize1        c4  default (0, 0, 0, 0)
//   float  uIntensity         c3  default (0, 0, 0, 0)
//   float  uOpacity           c1  default (0, 0, 0, 0)
//   float2 uScreenResolution  c0  default (0, 0, 0, 0)
//   float  uTime              c2  default (0, 0, 0, 0)
//
//     rcp oc0.x, c4.x
//     rcp oc0.y, c4.y
//     mul oc1.x, c2.x, (0.04)
//     mul oc2.x, c2.x, (-0.04)
//     mul r0.x, c1.x, c3.x
//     rcp r1.x, c0.x
//     rcp r1.y, c0.y
//     mul oc3.xy, r0.x, r1.xy
//
    ps_2_0
    def c6, 0.5, -0.5, 0, 0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    mov r0.xy, c4
    mad r0.xy, t0, r0, c5
    mul r0.xy, r0, c0
    mov r1.w, c6.x
    mad r1.y, r0.y, r1.w, c1.x
    mul r1.x, r0.x, c6.x
    mad r0.y, r0.y, r1.w, c2.x
    mov r0.x, r1.x
    texld r0, r0, s1
    texld r1, r1, s1
    add r0.xy, -r0, r1
    add r0.xy, r0, c6.y
    mad r0.xy, r0, c3, t0
    texld r0, r0, s0
    mov oC0, r0

// 15 instructions
