// PixelShader.Technique1[54] FinalFractal
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0          s0
//   float3    uSecondaryColor  c0  default (0, 0, 0, 0)
//
// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,
// r# = temporaries, (..) = literals):
//   float3 uSecondaryColor  c0  default (0, 0, 0, 0)
//
//     neg r0.x, c0.y
//     add r1.x, r0.x, (0.5)
//     rcp oc1.x, r1.x
//
    ps_2_0
    def c2, -0.5, 0, 0.0208351, -0.085133
    def c3, 0.180141, -0.3302995, 0.999866, 1
    def c4, 0, 1, -2, 1.5707964
    def c5, -0, -3.1415927, 0.31830996, 1
    dcl t0.xy
    dcl_2d s0
    add r0.xy, t0, c2.x
    abs r0.z, r0.y
    abs r0.w, r0.x
    max r1.w, r0.z, r0.w
    rcp r1.x, r1.w
    min r1.y, r0.w, r0.z
    add r0.z, -r0.z, r0.w
    cmp r0.z, r0.z, c4.x, c4.y
    mul r0.w, r1.x, r1.y
    mul r1.x, r0.w, r0.w
    mad r1.y, r1.x, c2.z, c2.w
    mad r1.y, r1.x, r1.y, c3.x
    mad r1.y, r1.x, r1.y, c3.y
    mad r1.x, r1.x, r1.y, c3.z
    mul r0.w, r0.w, r1.x
    mad r1.x, r0.w, c4.z, c4.w
    mad r0.z, r1.x, r0.z, r0.w
    cmp r0.w, r0.x, c5.x, c5.y
    add r0.z, r0.z, r0.w
    add r0.w, r0.z, r0.z
    max r1.x, r0.y, r0.x
    cmp r1.x, r1.x, c4.y, c4.x
    min r1.y, r0.x, r0.y
    dp2add r0.x, r0, r0, c2.y
    rsq r0.x, r0.x
    rcp r0.x, r0.x
    add r0.x, r0.x, -c0.y
    mov r1.w, c3.w
    mad r0.x, r0.x, -c1.x, r1.w
    cmp r0.y, r1.y, c2.y, r1.x
    mad r0.y, r0.y, -r0.w, r0.z
    mad r0.y, r0.y, c5.z, c5.w
    mul r0.y, r0.y, c0.x
    mov r1.x, c2.x
    mad r0.y, r0.y, -r1.x, c0.z
    abs r0.z, r0.y
    frc r0.z, r0.z
    cmp r1.x, r0.y, r0.z, -r0.z
    mov r1.y, r0.x
    texld r2, r1, s0
    add r0.x, -r0.x, c3.w
    cmp r0.y, r1.y, c4.x, c4.y
    cmp r0.x, r0.x, c4.x, c4.y
    add r0.x, r0.y, r0.x
    cmp r0, -r0.x, r2, c2.y
    mov oC0, r0

// 46 instructions
