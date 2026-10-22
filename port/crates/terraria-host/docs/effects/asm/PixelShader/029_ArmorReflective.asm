// PixelShader.Technique1[29] ArmorReflective
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0       s0
//   float3    uLightSource  c2  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize0  c0  default (0, 0, 0, 0)
//
//     rcp r0.x, c0.x
//     rcp r0.y, c0.y
//     add r1.xy, r0.xy, r0.xy
//     mov oc0.x, r1.y
//     mov oc1.x, r1.x
//
    ps_2_0
    def c3, 0, 0.33333334, 1, -0.3
    def c4, 1.5384616, -0.46153846, -2, 3
    def c5, 0.5, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    mov r0.x, -c1.x
    mov r0.y, c3.x
    add r0.xy, r0, t0
    add r1.x, t0.x, c1.x
    mov r1.y, t0.y
    mov r2.x, t0.x
    add r2.y, t0.y, c0.x
    mov r3.x, t0.x
    add r3.y, t0.y, -c0.x
    texld r0, r0, s0
    texld r1, r1, s0
    texld r2, r2, s0
    texld r3, r3, s0
    texld r4, t0, s0
    add r0.xyz, r0, -r1
    add r2.w, r0.y, r0.x
    add r2.w, r0.z, r2.w
    mul r0.x, r2.w, c3.y
    add r1.xyz, -r2, r3
    add r0.w, r1.y, r1.x
    add r0.w, r1.z, r0.w
    mul r0.y, r0.w, c3.y
    mul r0.w, r0.y, r0.y
    mad r0.w, r0.x, r0.x, r0.w
    add r0.w, -r0.w, c3.z
    rsq r1.x, r0.w
    rcp r1.x, r1.x
    cmp r0.z, r0.w, r1.x, c3.x
    dp3 r0.x, r0, c2
    add r0.y, r0.x, c3.w
    mul r0.y, r0.y, c4.x
    cmp_sat r0.x, r0.x, r0.y, c4.y
    mad r0.y, r0.x, c4.z, c4.w
    mul r0.x, r0.x, r0.x
    mul r0.x, r0.y, r0.x
    add r0.x, r0.x, r0.x
    mul r0.x, r0.x, r0.x
    mul r0.x, r4.w, r0.x
    mul r0.yzw, r4.wzyx, c5.x
    mad r4.xyz, r0.x, r4, r0.wzyx
    mul r0, r4, v0
    mov oC0, r0

// 42 instructions
