// ScreenShader.Technique1[24] FilterTest2
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   sampler2D uImage0   s0
//   float     uOpacity  c0  default (0, 0, 0, 0)
//
    ps_2_0
    def c1, 0.55, -0.1, 0, -0.25
    def c2, 0.60784316, 0.7372549, 0.05882353, 0
    def c3, 0.05882353, 0.21960784, 0.05882353, 0
    def c4, 0.1882353, 0.38431373, 0.1882353, 0
    def c5, 0.54509807, 0.6745098, 0.05882353, 0
    def c6, -0.5, -0.75, 0, 0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    max r1.w, r0.y, r0.z
    max r2.w, r0.x, r1.w
    min r1.x, r0.z, r0.y
    min r2.x, r1.x, r0.x
    add r1.x, r2.w, r2.x
    mad r1.x, r1.x, c1.x, c1.y
    max r2.x, r1.x, c1.z
    add r1.x, r2.x, c6.y
    mov r3.xyz, c2
    cmp r1.xyz, r1.x, r3, c5
    add r1.w, r2.x, c6.x
    add r2.x, r2.x, c1.w
    cmp r1.xyz, r1.w, r1, c4
    cmp r1.xyz, r2.x, r1, c3
    lrp r2.xyz, c0.x, r1, r0
    mov r2.w, r0.w
    mov oC0, r2

// 18 instructions
