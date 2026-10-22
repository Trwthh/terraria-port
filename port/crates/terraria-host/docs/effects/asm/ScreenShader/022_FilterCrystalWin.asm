// ScreenShader.Technique1[22] FilterCrystalWin
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0            s0
//   float     uProgress          c3  default (0, 0, 0, 0)
//   float2    uScreenResolution  c2  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float  uOpacity         c2  default (0, 0, 0, 0)
//   float  uProgress        c3  default (0, 0, 0, 0)
//   float2 uScreenPosition  c0  default (0, 0, 0, 0)
//   float2 uTargetPosition  c1  default (0, 0, 0, 0)
//
//     neg r0.xy, c0.xy
//     add oc0.xy, r0.xy, c1.xy
//     mul oc1.x, c2.x, c3.x
//
    ps_2_0
    def c4, 1, 0, 0, 0
    def c5, 0, 0.0078125, 0.5, 2
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mov r1.xy, c2
    mad r1.xy, t0, r1, -c0
    dp2add r1.x, r1, r1, c5.x
    rsq r1.x, r1.x
    rcp r1.x, r1.x
    mov r1.y, c5.y
    mad r1.x, r1.x, r1.y, c3.x
    add r1.y, -r1.x, c5.z
    rcp r1.x, r1.x
    cmp r1.x, r1.y, c5.w, r1.x
    mul r1.x, r1.x, c3.x
    mul r1.y, r1.x, r1.x
    mad r1.x, r1.x, r1.x, c4.x
    mad r1, r0, r1.x, r1.y
    lrp r2, c1.x, r1, r0
    mov oC0, r2

// 17 instructions
