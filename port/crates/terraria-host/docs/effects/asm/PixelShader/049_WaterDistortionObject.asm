// PixelShader.Technique1[49] WaterDistortionObject
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//
    ps_2_0
    def c0, -0.5, 0, -0.2, 25.12
    def c1, 6.28, -12.56, 0.15915494, 0.5
    def c2, 6.2831855, -3.1415927, -0.0063711135, 0
    def c3, -0.0000015500992, -0.00002170139, 0.0026041667, 0.00026041668
    def c4, -0.020833334, -0.125, 1, 0.5
    dcl v0.xyz
    dcl t0.xy
    add r0.xy, v0, c0.x
    dp2add r0.z, r0, r0, c0.y
    rsq r0.z, r0.z
    mul r0.xy, r0, r0.z
    add r1.xy, t0, c0.x
    dp2add r0.z, r1, r1, c0.y
    rsq r0.z, r0.z
    mul r1.xy, r1, r0.z
    rcp r0.z, r0.z
    mul r0.z, r0.z, c0.w
    max r1.z, r0.z, c1.x
    dp2add r0.x, -r0, r1, c0.z
    max r1.x, r0.x, c0.y
    mad r0.x, r1.z, c1.z, c1.w
    add r0.y, r1.z, c1.y
    frc r0.x, r0.x
    mad r0.x, r0.x, c2.x, c2.y
    cmp r0.x, r0.y, c2.z, r0.x
    sincos r2.x, r0.x, c3, c4
    cmp r0.x, r2.x, c0.y, -r2.x
    mul r0.x, r1.x, r0.x
    mul r0.xw, r0.x, v0.z
    mov r0.yz, c0.y
    mov oC0, r0

// 24 instructions
