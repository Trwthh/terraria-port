// ScreenShader.Technique1[26] FilterCRT
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0            s0
//   bool      uMultiChunkScene   c5  default (0, 0, 0, 0)
//   float     uOpacity           c3  default (0, 0, 0, 0)
//   float2    uScreenResolution  c2  default (0, 0, 0, 0)
//   float2    uZoom              c4  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize0  c0  default (0, 0, 0, 0)
//   float2 uSceneSize   c1  default (0, 0, 0, 0)
//
//     rcp r0.x, c1.x
//     rcp r0.y, c1.y
//     mul r1.xy, r0.xy, c0.xy
//     rcp oc1.x, r1.x
//     rcp oc1.y, r1.y
//     mov oc0.xy, r1.xy
//
    ps_2_0
    def c6, 2, -1, 0.14285715, 0.5
    def c7, 0.16, 1, 0.13, 0
    def c8, 0.22265618, 0.5, 6.2831855, -3.1415927
    def c9, 1, 0, 0.5, -0.5
    def c10, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c11, -0.020833334, -0.125, 1, 0.5
    dcl t0.xy
    dcl_2d s0
    mul r0.xy, t0, c0
    mad r0.xy, r0, c6.x, c6.y
    mul r1.x, r0.y, c3.x
    mul r1.y, r0.x, c3.x
    mul r0.zw, r1.wzyx, c6.z
    mul r0.zw, r0, r0
    mad r0.xy, r0.wzyx, r0, r0
    mad r0.z, r0.x, c9.z, c9.w
    cmp r0.z, r0.z, c9.x, c9.y
    mad r1.xy, r0, c6.w, c6.w
    mad r0.x, r0.y, c9.z, c9.w
    cmp r0.x, r0.x, c9.x, c9.y
    cmp r0.y, -r1.x, c9.x, c9.y
    add r0.y, r0.z, r0.y
    cmp r0.y, -r0.y, c9.y, c9.x
    cmp r0.z, -r1.y, c9.x, c9.y
    mul r1.xy, r1, c1
    texld r1, r1, s0
    texld r2, t0, s0
    add r0.y, r0.y, r0.z
    cmp r0.y, -r0.y, c9.y, c9.x
    add r0.x, r0.x, r0.y
    cmp r0, -r0.x, r1, c9.y
    cmp r0, -c5.x, r0, r2
    mul r1.x, t0.y, c2.y
    mul r1.x, r1.x, c4.x
    mad r1.x, r1.x, c8.x, c8.y
    frc r1.x, r1.x
    mad r1.x, r1.x, c8.z, c8.w
    sincos r2.xy, r1.x, c10, c11
    add r1.x, r2.y, -c6.y
    add r1.y, r2.x, -c6.y
    mad r1.y, r1.y, c7.z, c7.y
    mul_sat r2.xz, r0, r1.y
    mad r1.x, r1.x, c7.x, c7.y
    mul_sat r2.y, r0.y, r1.x
    mov_sat r2.w, r0.w
    lrp r1, c3.x, r2, r0
    mov oC0, r1

// 39 instructions
