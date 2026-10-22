// ScreenShader.Technique1[21] FilterCrystalDestructionVortex
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0            s0
//   sampler2D uImage1            s1
//   float     uIntensity         c4  default (0, 0, 0, 0)
//   float     uProgress          c5  default (0, 0, 0, 0)
//   float2    uScreenResolution  c3  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize1      c3  default (0, 0, 0, 0)
//   float2 uScreenPosition  c0  default (0, 0, 0, 0)
//   float2 uTargetPosition  c1  default (0, 0, 0, 0)
//   float  uTime            c2  default (0, 0, 0, 0)
//
//     neg r0.xy, c0.xy
//     add oc0.xy, r0.xy, c1.xy
//     rcp oc1.x, c3.y
//     mul oc2.x, c2.x, (-2.5)
//
    ps_2_0
    def c6, 0, 0.0208351, -0.085133, 0.180141
    def c7, -0.3302995, 0.999866, 0, 1
    def c8, -2, 1.5707964, -0, -3.1415927
    def c9, 0.31831926, 0.02, 3.141592, 0.1
    def c10, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c11, -0.020833334, -0.125, 1, 0.5
    dcl t0.xy
    dcl_2d s0
    dcl_2d s1
    mov r0.xy, c3
    mad r0.xy, t0, r0, -c0
    abs r0.z, r0.y
    abs r0.w, r0.x
    max r1.w, r0.z, r0.w
    rcp r1.x, r1.w
    min r1.y, r0.w, r0.z
    add r0.z, -r0.z, r0.w
    cmp r0.z, r0.z, c7.z, c7.w
    mul r0.w, r1.x, r1.y
    mul r1.x, r0.w, r0.w
    mad r1.y, r1.x, c6.y, c6.z
    mad r1.y, r1.x, r1.y, c6.w
    mad r1.y, r1.x, r1.y, c7.x
    mad r1.x, r1.x, r1.y, c7.y
    mul r0.w, r0.w, r1.x
    mad r1.x, r0.w, c8.x, c8.y
    mad r0.z, r1.x, r0.z, r0.w
    cmp r0.w, r0.x, c8.z, c8.w
    add r0.z, r0.z, r0.w
    add r0.w, r0.z, r0.z
    max r1.x, r0.y, r0.x
    cmp r1.x, r1.x, c7.w, c7.z
    min r1.y, r0.x, r0.y
    dp2add r0.x, r0, r0, c6.x
    rsq r0.x, r0.x
    rcp r0.x, r0.x
    cmp r0.y, r1.y, c6.x, r1.x
    mad r0.y, r0.y, -r0.w, r0.z
    mad r1.x, r0.x, c1.x, r0.y
    mul r0.y, r0.x, c1.x
    mov r2.w, c1.x
    mad r0.x, r0.x, r2.w, -c5.x
    max r1.z, r0.x, c6.x
    min r0.x, r1.z, c9.z
    sincos r2.y, r0.x, c10, c11
    mul r0.x, r2.y, c9.w
    add r1.y, r0.y, c2.x
    mul r1.xy, r1, c9
    texld r1, r1, s1
    texld r2, t0, s0
    mul r0.yz, r1.zxyw, r1.zxyw
    mul r0.yz, r1.zxyw, r0
    mul r0.xy, r0.x, r0.yzxw
    mad r1.xyz, r0.x, -c4.x, r2
    mad r2.xyz, r0.y, c4.x, r1
    mov oC0, r2

// 47 instructions
