// ScreenShader.Technique1[10] FilterBloodMoon
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   float3    uColor        c2  default (0, 0, 0, 0)
//   sampler2D uImage0       s0
//   float2    uImageSize0   c5  default (0, 0, 0, 0)
//   float     uIntensity    c4  default (0, 0, 0, 0)
//   float     uOpacity      c3  default (0, 0, 0, 0)
//   float2    uSceneOffset  c6  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uColor      c0  default (0, 0, 0, 0)
//   float2 uSceneSize  c1  default (0, 0, 0, 0)
//
//     rcp oc0.x, c1.x
//     rcp oc0.y, c1.y
//     neg r0.xyz, c0.xyz
//     add oc1.xyz, (1), r0.xyz
//
    ps_2_0
    def c7, -0.5, -0.3, 2, 1
    def c8, 0.5, 1.5, 0.16666667, 1
    def c9, 0.7, 0, 0, 0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mov r1.xy, c5
    mad r1.xy, t0, r1, c6
    mov r2.xw, c7
    mad r1.xy, r1, c0, r2.x
    mul r1.y, r1.y, r1.y
    mad r1.x, r1.x, r1.x, r1.y
    rsq r1.x, r1.x
    rcp r1.x, r1.x
    add r1.x, r1.x, c7.y
    mad r1.y, r1.x, -c7.z, c7.w
    cmp r1.x, r1.x, r1.y, c7.w
    max r1.y, r0.y, r0.z
    max r2.x, r0.x, r1.y
    min r1.y, r0.z, r0.y
    min r2.y, r1.y, r0.x
    add r1.y, r2.x, r2.y
    add r1.z, r2.x, -r2.y
    add r1.z, r1.z, c8.z
    mad r1.w, r1.y, -c8.x, c8.y
    mad r2.xyz, r1.w, -c1, r2.w
    mad r1.w, r1.y, c7.x, -c7.x
    mul r3.xyz, r1.y, c2
    cmp r2.xyz, r1.w, r3, r2
    mad r2.xyz, r1.y, c7.x, r2
    mul r1.w, r1.y, -c7.x
    mad r2.w, r1.y, -c8.x, c8.w
    mul r2.w, r2.w, c4.x
    mul r2.w, r2.w, c9.x
    mad r3.xyz, r1.z, r2, r1.w
    mov r3.w, c7.w
    mul r4, r0.w, r3
    mul r4, r4, r4
    mad r3, r3, r0.w, -r4
    mad r3, r3, -c7.y, r4
    lrp r4, r2.w, r3, r0
    mad r1, r1.x, r4, -r0
    mad r0, c3.x, r1, r0
    mov oC0, r0

// 39 instructions
