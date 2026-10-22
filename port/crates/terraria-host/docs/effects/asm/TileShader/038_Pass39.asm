// TileShader.Technique1[38] Pass39
//
// No vertex shader in the pass (SpriteBatch's vertex stage).
//
// Constant table (ps_2_0; Microsoft (R) HLSL Shader Compiler 9.26.952.2844):
//   bool      invertSpecialGroupResult  c5  default (0, 0, 0, 0)
//   float     leafHueTestOffset         c4  default (0, 0, 0, 0)
//   float     leafMaxHue                c1  default (0, 0, 0, 0)
//   float     leafMaxSat                c3  default (0, 0, 0, 0)
//   float     leafMinHue                c0  default (0, 0, 0, 0)
//   float     leafMinSat                c2  default (0, 0, 0, 0)
//   sampler2D s0                        s0
//
    ps_2_0
    def c6, 6, 2, 4, 0.16666667
    def c7, 1, 0, 0, 0
    dcl v0
    dcl t0.xy
    dcl_2d s0
    texld r0, t0, s0
    min r1.w, r0.z, r0.y
    min r2.w, r1.w, r0.x
    max r1.x, r0.y, r0.z
    max r2.x, r0.x, r1.x
    add r1.x, -r2.w, r2.x
    rcp r1.y, r1.x
    add r3.xyz, -r0.zxyw, r2.x
    mul r2.yzw, r1.y, r3.wzyx
    mad r3.x, r3.x, r1.y, -r2.y
    mad r3.y, r3.y, r1.y, -r2.w
    mad r3.z, r3.z, r1.y, -r2.z
    add r1.yzw, r3.wzyx, c6.wzyx
    add r2.yzw, r0.wzyx, -r2.x
    rcp r2.x, r2.x
    mul r3.y, r1.x, r2.x
    mul r1.x, r1.x, r1.x
    cmp r2.xyz, r2.wzyx, c7.x, c7.y
    mul r1.yzw, r1, r2.wzyx
    max r2.x, r1.y, r1.w
    max r3.z, r1.z, r2.x
    mul r1.y, r3.z, c6.w
    frc r3.x, r1.y
    cmp r1.xy, -r1.x, c7.y, r3
    add r1.z, -r1.y, c3.x
    cmp r1.z, r1.z, c7.x, c7.y
    add r1.y, r1.y, -c2.x
    add r1.x, r1.x, c4.x
    cmp r1.y, r1.y, r1.z, c7.y
    abs r1.z, r1.x
    frc r1.z, r1.z
    cmp r1.x, r1.x, r1.z, -r1.z
    add r1.z, -r1.x, c1.x
    add r1.x, r1.x, -c0.x
    cmp r1.y, r1.z, r1.y, c7.y
    cmp r1.x, r1.x, r1.y, c7.y
    cmp r1.y, -r1.x, c7.x, c7.y
    cmp r1.x, -c5.x, r1.x, r1.y
    max r2.x, r0.x, r0.y
    min r2.y, r0.y, r0.x
    add r2.w, -r0.z, r2.x
    mov r2.z, r0.z
    cmp r3.x, r2.w, r2.x, r2.z
    cmp r3.y, r2.w, r2.z, r2.x
    min r4.y, r3.y, r2.y
    mov r4.xz, r3.x
    cmp r0.xyz, -r1.x, r0, r4
    mul r0, r0, v0
    mov oC0, r0

// 49 instructions
