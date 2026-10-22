// TileShader.Technique1[40] Pass41
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D s0  s0
//
    ps_2_0
    def c0, 0, 0.7, 0.49, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    mov r1.xw, v0
    mov r1.yz, c0
    max r2.w, r0.x, r0.y
    max r3.x, r2.w, r0.z
    mov r3.w, r0.w
    mul r3.y, r3.x, v0.y
    mul r3.z, r3.x, v0.z
    mul r0, r1, r3
    mov oC0, r0

// 10 instructions
