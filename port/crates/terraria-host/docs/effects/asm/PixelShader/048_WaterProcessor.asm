// PixelShader.Technique1[48] WaterProcessor
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0  s0
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float2 uImageSize0  c0  default (0, 0, 0, 0)
//
//     rcp oc1.x, c0.x
//     rcp oc0.x, c0.y
//
    ps_2_0
    def c2, -0.055141553, 0.25, 0.05, 1
    def c3, 2, -1, 3.3333333, -1.6666666
    def c4, 0.3, 0.5, 512, 0.001953125
    def c5, 0, 1, -0, -1
    dcl t0.xy
    dcl_2d s0
    mov r0.x, -c1.x
    mov r0.y, c5.z
    add r0.xy, r0, t0
    add r1.x, t0.x, c1.x
    mov r1.y, t0.y
    mov r2.x, c5.z
    mov r2.y, -c0.x
    add r2.xy, r2, t0
    mov r3.x, t0.x
    add r3.y, t0.y, c0.x
    texld r0, r0, s0
    texld r1, r1, s0
    texld r2, r2, s0
    texld r3, r3, s0
    texld r4, t0, s0
    mad r4.w, r0.x, c3.x, c3.y
    mad r4.w, r1.x, c3.x, r4.w
    mad r0.x, r2.x, c3.x, c3.y
    add r4.w, r4.w, r0.x
    mad r0.x, r3.x, c3.x, c3.y
    add r4.w, r4.w, r0.x
    add r4.w, r4.w, c3.y
    mad r0.x, r4.x, c3.x, c3.y
    mad r0.y, r4.w, c2.y, -r0.x
    add r0.z, -r4.z, -c3.y
    mad r0.w, r4.y, c3.z, c3.w
    mul r0.y, r0.y, r0.z
    add r0.y, r0.y, r0.y
    cmp r1.x, -r0.w, c5.x, c5.y
    cmp r1.y, r0.w, c5.z, c5.w
    mul r0.w, r0.w, r0.w
    add r1.x, r1.x, r1.y
    mad r0.y, r0.w, r1.x, r0.y
    add r0.y, r0.x, r0.y
    mul r0.w, r0.z, c2.x
    add r0.z, -r0.z, -c3.y
    mad r0.z, r0.z, -c2.z, c2.w
    exp r0.w, r0.w
    mad r0.x, r0.y, r0.w, -r0.x
    mul r0.y, r0.y, r0.w
    mad r0.y, r0.y, c4.y, c4.y
    mad r0.y, r0.y, c4.z, c4.y
    mul r0.x, r0.z, r0.x
    cmp r0.z, -r0.x, c5.x, c5.y
    cmp r0.w, r0.x, c5.z, c5.w
    abs r0.x, r0.x
    rsq r0.x, r0.x
    rcp r0.x, r0.x
    add r0.z, r0.z, r0.w
    mul r0.x, r0.x, r0.z
    mad r1.y, r0.x, c4.x, c4.y
    frc r0.x, r0.y
    add r0.x, r0.y, -r0.x
    mul r1.x, r0.x, c4.w
    mov r1.zw, -c5
    mov oC0, r1

// 56 instructions
