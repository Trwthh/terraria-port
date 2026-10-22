// ScreenShader.Technique1[7] FilterNoir
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0       s0
//   sampler2D uImage1       s1
//   float2    uImageOffset  c9  default (0, 0, 0, 0)
//   float2    uImageSize0   c8  default (0, 0, 0, 0)
//   float     uOpacity      c7  default (0, 0, 0, 0)
//   float2    uSceneOffset  c10  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize1  c2  default (0, 0, 0, 0)
//   float  uIntensity   c1  default (0, 0, 0, 0)
//   float2 uSceneSize   c3  default (0, 0, 0, 0)
//   float  uTime        c0  default (0, 0, 0, 0)
//
//     neg r0.x, c1.x
//     add r1.x, r0.x, (0.5)
//     rcp oc1.x, r1.x
//     neg r0.x, r1.x
//     mov oc0.x, r1.x
//     add r1.x, r0.x, (1)
//     rcp oc2.x, r1.x
//     rcp oc3.x, c2.x
//     rcp oc3.y, c2.y
//     mul r0.x, c0.x, (5)
//     frc r1.x, r0.x
//     neg r0.y, r1.x
//     add r1.x, r0.x, r0.y
//     mul r0.x, r1.x, (0.0031)
//     frc oc5.x, r0.x
//     rcp oc6.x, c3.x
//     rcp oc6.y, c3.y
//
    ps_2_0
    def c11, 0.33333334, 0.16666667, 1, 0.5
    def c12, 2, -1, 0.2, -0.6
    def c13, 1.4, 0, 1, 19.831825
    def c14, 1024, -0.9999, 32, 1
    def c15, -2, 3, 0, 0
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    add r0.w, t0.x, c5.x
    mul r0.x, r0.w, c13.w
    exp r0.x, r0.x
    cmp r0.y, -r0.x, c13.z, c13.y
    add r0.y, r0.x, r0.y
    log r0.y, r0.y
    frc r0.z, -r0.y
    add r0.y, r0.y, r0.z
    exp r0.y, r0.y
    rcp r0.y, r0.y
    mul r0.x, r0.x, r0.y
    mul r0.x, r0.x, c14.x
    frc r0.x, r0.x
    add r0.x, r0.x, c14.y
    mov r1.xy, c8
    mad r0.yz, t0.zxyw, r1.zxyw, c10.zxyw
    mov r1.xy, c3
    mad r1.xy, r0.yzxw, r1, c9
    texld r1, r1, s1
    texld r2, t0, s0
    cmp r0.x, r0.x, r1.x, c13.y
    add r0.w, r1.x, -c11.w
    add r1.x, r2.y, r2.x
    add r1.x, r2.z, r1.x
    mad r1.y, r1.x, -c11.x, c11.z
    mul r1.y, r1.y, c11.w
    mov r3.xz, c11
    mad r1.y, r1.y, -c2.x, r3.z
    mul r1.z, r1.x, c11.y
    mad r1.x, r1.x, r3.x, -c0.x
    mul r1.z, r1.z, c1.x
    cmp r1.x, r1.x, r1.y, r1.z
    mad r1.x, r1.x, c12.x, c12.y
    abs r1.y, r1.x
    mul r1.z, r1.y, r1.y
    mul r1.w, r1.y, r1.z
    mul r1.z, r1.z, c12.z
    mad r1.z, r1.w, c12.w, r1.z
    mad r1.y, r1.y, c13.x, r1.z
    cmp r1.z, -r1.x, c13.y, c13.z
    cmp r1.x, r1.x, -c13.y, -c13.z
    add r1.x, r1.z, r1.x
    mad r1.x, r1.y, r1.x, c11.z
    mad r1.y, r1.x, -c11.w, c11.z
    mul r1.x, r1.x, c11.w
    mul r0.w, r0.w, r1.x
    mul r0.w, r1.y, r0.w
    mad r0.w, r0.w, c12.z, r1.x
    mul r0.x, r0.x, r0.w
    add_sat r0.x, r0.x, r0.x
    add r0.x, r0.w, r0.x
    mul r1.xy, r0.yzxw, c6
    mad r0.yz, r0, -c6.zxyw, r3.z
    mul r0.yz, r1.zxyw, r0
    mul r0.y, r0.z, r0.y
    mad r0.y, r0.y, c14.z, c14.w
    mul_sat r0.y, r0.y, c11.w
    mad r0.z, r0.y, c15.x, c15.y
    mul r0.y, r0.y, r0.y
    mul r0.y, r0.z, r0.y
    mad r0.xyz, r0.x, r0.y, -r2
    mad r2.xyz, c7.x, r0, r2
    mov oC0, r2

// 63 instructions
