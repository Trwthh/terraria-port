#!/usr/bin/env python3
"""Writes native_xna.json: the interop table for the XNA value types that terraria-core ports
(port/crates/terraria-core/src/xna). Deterministic; run it again when terraria-core gains a member.

    python gen_native_xna.py

Keys are Roslyn documentation ids. Every Rust target named here exists in terraria-core (a wrong name is a rustc
error in the generated crate, never wrong behaviour). The behaviour of each target is tested in terraria-core
against reference/xna/*.cs.
"""
import json, os, re

HERE = os.path.dirname(os.path.abspath(__file__))
XNA = os.path.join(HERE, "..", "..", "port", "crates", "terraria-core", "src", "xna")
NS = "Microsoft.Xna.Framework."
R = "terraria_core::xna::"
F = "System.Single"
m = {}


def pascal(screaming):
    return "".join(p.capitalize() for p in screaming.split("_"))


def vec(name, fields):
    T = NS + name
    RT = R + name
    n = len(fields)
    m[f"M:{T}.#ctor({','.join([F] * n)})"] = f"{RT}::new({', '.join('{%d}' % i for i in range(n))})"
    m[f"M:{T}.#ctor({F})"] = f"{RT}::splat({{0}})"
    for f in fields:
        m[f"F:{T}.{f.upper()}"] = "{this}." + f
    for prop in ["Zero", "One"] + ["Unit" + f.upper() for f in fields]:
        m[f"P:{T}.{prop}"] = {"get": f"{RT}::{re.sub(r'(?<=[a-z])(?=[A-Z])', '_', prop).upper()}"}
    for op, sym in [("Addition", "+"), ("Subtraction", "-"), ("Multiply", "*"), ("Division", "/")]:
        m[f"M:{T}.op_{op}({T},{T})"] = f"({{0}} {sym} {{1}})"
    m[f"M:{T}.op_Multiply({T},{F})"] = "({0} * {1})"
    m[f"M:{T}.op_Multiply({F},{T})"] = "({0} * {1})"
    m[f"M:{T}.op_Division({T},{F})"] = "({0} / {1})"
    m[f"M:{T}.op_UnaryNegation({T})"] = "(-{0})"
    m[f"M:{T}.op_Equality({T},{T})"] = "({0} == {1})"
    m[f"M:{T}.op_Inequality({T},{T})"] = "({0} != {1})"
    m[f"M:{T}.Equals({T})"] = "({this} == {0})"
    m[f"M:{T}.Length"] = "{this}.length()"
    m[f"M:{T}.LengthSquared"] = "{this}.length_squared()"
    m[f"M:{T}.Normalize"] = "{this}.normalize()"
    m[f"M:{T}.Normalize({T})"] = f"{RT}::normalized({{0}})"
    for meth, rust, k in [("Distance", "distance", 2), ("DistanceSquared", "distance_squared", 2), ("Dot", "dot", 2), ("Reflect", "reflect", 2),
                          ("Min", "min", 2), ("Max", "max", 2), ("Clamp", "clamp", 3), ("Negate", "negate", 1), ("Add", "add", 2),
                          ("Subtract", "subtract", 2), ("Multiply", "multiply", 2), ("Divide", "divide", 2)]:
        m[f"M:{T}.{meth}({','.join([T] * k)})"] = f"{RT}::{rust}({', '.join('{%d}' % i for i in range(k))})"
    m[f"M:{T}.Multiply({T},{F})"] = f"{RT}::multiply_scalar({{0}}, {{1}})"
    m[f"M:{T}.Divide({T},{F})"] = f"{RT}::divide_scalar({{0}}, {{1}})"
    m[f"M:{T}.Lerp({T},{T},{F})"] = f"{RT}::lerp({{0}}, {{1}}, {{2}})"
    m[f"M:{T}.SmoothStep({T},{T},{F})"] = f"{RT}::smooth_step({{0}}, {{1}}, {{2}})"
    m[f"M:{T}.Barycentric({T},{T},{T},{F},{F})"] = f"{RT}::barycentric({{0}}, {{1}}, {{2}}, {{3}}, {{4}})"
    m[f"M:{T}.CatmullRom({T},{T},{T},{T},{F})"] = f"{RT}::catmull_rom({{0}}, {{1}}, {{2}}, {{3}}, {{4}})"
    m[f"M:{T}.Hermite({T},{T},{T},{T},{F})"] = f"{RT}::hermite({{0}}, {{1}}, {{2}}, {{3}}, {{4}})"


vec("Vector2", "xy")
vec("Vector3", "xyz")
vec("Vector4", "xyzw")
V2, V3, V4, MX, C, RC, PT, MH = (NS + x for x in ["Vector2", "Vector3", "Vector4", "Matrix", "Color", "Rectangle", "Point", "MathHelper"])

m[f"M:{V3}.#ctor({V2},{F})"] = R + "Vector3::from_vector2({0}, {1})"
m[f"M:{V4}.#ctor({V2},{F},{F})"] = R + "Vector4::from_vector2({0}, {1}, {2})"
m[f"M:{V4}.#ctor({V3},{F})"] = R + "Vector4::from_vector3({0}, {1})"
for prop in ["Up", "Down", "Right", "Left", "Forward", "Backward"]:
    m[f"P:{V3}.{prop}"] = {"get": f"{R}Vector3::{prop.upper()}"}
m[f"M:{V3}.Cross({V3},{V3})"] = R + "Vector3::cross({0}, {1})"
m[f"M:{V2}.Transform({V2},{MX})"] = R + "Vector2::transform({0}, &{1})"
m[f"M:{V2}.TransformNormal({V2},{MX})"] = R + "Vector2::transform_normal({0}, &{1})"
m[f"M:{V3}.Transform({V3},{MX})"] = R + "Vector3::transform({0}, &{1})"
m[f"M:{V3}.TransformNormal({V3},{MX})"] = R + "Vector3::transform_normal({0}, &{1})"
m[f"M:{V4}.Transform({V2},{MX})"] = R + "Vector4::transform_vector2({0}, &{1})"
m[f"M:{V4}.Transform({V3},{MX})"] = R + "Vector4::transform_vector3({0}, &{1})"
m[f"M:{V4}.Transform({V4},{MX})"] = R + "Vector4::transform({0}, &{1})"
# Vector2.Hermite(ref, ref, ref, ref, float, out) (ItemTransferParticle.Draw): XNA's machine code in the gallery run
# (work/jit/client/gallery, jit_disasm.py) keeps amount and the six weights on the x87 stack (53-bit precision) and
# rounds only the two result components (fstp dword): this model, in the same operation order
m[f"M:{V2}.Hermite({V2}@,{V2}@,{V2}@,{V2}@,{F},{V2}@)"] = (
    "{ let __v1 = *{0}; let __t1 = *{1}; let __v2 = *{2}; let __t2 = *{3}; let __a: f64 = (({4}) as f32) as f64; "
    "let __n: f64 = __a * __a; let __n2: f64 = __a * __n; let __n3: f64 = (__n2 * 2.0f64 - __n * 3.0f64) + 1.0f64; "
    "let __n4: f64 = __n2 * -2.0f64 + __n * 3.0f64; let __n5: f64 = (__n2 - __n * 2.0f64) + __a; let __n6: f64 = __n2 - __n; "
    "*{5} = " + R + "Vector2::new(((((__v1.x as f64) * __n3 + (__v2.x as f64) * __n4) + (__t1.x as f64) * __n5) + (__t2.x as f64) * __n6) as f32, "
    "((((__v1.y as f64) * __n3 + (__v2.y as f64) * __n4) + (__t1.y as f64) * __n5) + (__t2.y as f64) * __n6) as f32); }")
# Vector2.Hermite(Vector2, Vector2, Vector2, Vector2, float) (Projectile.DoPalAppearEffect, a pet's appear effect):
# XNA's machine code in the gallery run (work/jit/client/gallery, jit_disasm.py) keeps amount and the weights on the
# x87 stack too, except the first weight (2a^3 - 3a^2 + 1: `fstp dword [ebp-4]`, a frame local), and rounds the two
# result components; the f32 arithmetic of terraria_core's hermite was 1 ulp off in a dust's position and its velocity
# (gallery scene proj_35, frame 2561)
m[f"M:{V2}.Hermite({V2},{V2},{V2},{V2},{F})"] = (
    "{ let __v1 = {0}; let __t1 = {1}; let __v2 = {2}; let __t2 = {3}; let __a: f64 = (({4}) as f32) as f64; "
    "let __n: f64 = __a * __a; let __n2: f64 = __a * __n; let __n3: f64 = (((__n2 * 2.0f64 - __n * 3.0f64) + 1.0f64) as f32) as f64; "
    "let __n4: f64 = __n2 * -2.0f64 + __n * 3.0f64; let __n5: f64 = (__n2 - __n * 2.0f64) + __a; let __n6: f64 = __n2 - __n; "
    + R + "Vector2::new(((((__v1.x as f64) * __n3 + (__v2.x as f64) * __n4) + (__t1.x as f64) * __n5) + (__t2.x as f64) * __n6) as f32, "
    "((((__v1.y as f64) * __n3 + (__v2.y as f64) * __n4) + (__t1.y as f64) * __n5) + (__t2.y as f64) * __n6) as f32) }")
# ref / out forms the game uses: arguments arrive as pointers
m[f"M:{V3}.Max({V3}@,{V3}@,{V3}@)"] = "{ *{2} = " + R + "Vector3::max(*{0}, *{1}); }"
m[f"M:{RC}.Union({RC}@,{RC}@,{RC}@)"] = "{ *{2} = " + R + "Rectangle::union(*{0}, *{1}); }"

for i in range(1, 5):
    for j in range(1, 5):
        m[f"F:{MX}.M{i}{j}"] = "{this}.m%d%d" % (i, j)
m[f"P:{MX}.Identity"] = {"get": R + "Matrix::IDENTITY"}
m[f"P:{MX}.Translation"] = {"get": "{this}.translation()", "set": "{this}.set_translation({value})"}
m[f"M:{MX}.op_Multiply({MX},{MX})"] = "({0} * {1})"
m[f"M:{MX}.Multiply({MX},{MX})"] = R + "Matrix::multiply({0}, {1})"
m[f"M:{MX}.CreateTranslation({V3})"] = R + "Matrix::create_translation_vector3({0})"
m[f"M:{MX}.CreateTranslation({F},{F},{F})"] = R + "Matrix::create_translation({0}, {1}, {2})"
m[f"M:{MX}.CreateScale({F})"] = R + "Matrix::create_scale_uniform({0})"
m[f"M:{MX}.CreateScale({F},{F},{F})"] = R + "Matrix::create_scale({0}, {1}, {2})"
m[f"M:{MX}.CreateScale({V3})"] = R + "Matrix::create_scale_vector3({0})"
for ax in "XYZ":
    m[f"M:{MX}.CreateRotation{ax}({F})"] = R + f"Matrix::create_rotation_{ax.lower()}({{0}})"
m[f"M:{MX}.CreateOrthographic({F},{F},{F},{F})"] = R + "Matrix::create_orthographic({0}, {1}, {2}, {3})"
m[f"M:{MX}.CreateOrthographicOffCenter({F},{F},{F},{F},{F},{F})"] = R + "Matrix::create_orthographic_off_center({0}, {1}, {2}, {3}, {4}, {5})"
m[f"M:{MX}.Transpose({MX})"] = R + "Matrix::transpose({0})"
m[f"M:{MX}.Invert({MX})"] = R + "Matrix::invert({0})"
m[f"M:{MX}.Determinant"] = "{this}.determinant()"

I = "System.Int32"
m[f"M:{C}.#ctor({I},{I},{I})"] = R + "Color::from_ints_rgb({0}, {1}, {2})"
m[f"M:{C}.#ctor({I},{I},{I},{I})"] = R + "Color::from_ints({0}, {1}, {2}, {3})"
m[f"M:{C}.#ctor({F},{F},{F})"] = R + "Color::from_floats_rgb({0}, {1}, {2})"
m[f"M:{C}.#ctor({F},{F},{F},{F})"] = R + "Color::from_floats({0}, {1}, {2}, {3})"
m[f"M:{C}.#ctor({V3})"] = R + "Color::from_vector3({0})"
m[f"M:{C}.#ctor({V4})"] = R + "Color::from_vector4({0})"
m[f"M:{C}.FromNonPremultiplied({V4})"] = R + "Color::from_non_premultiplied_vector4({0})"
m[f"M:{C}.FromNonPremultiplied({I},{I},{I},{I})"] = R + "Color::from_non_premultiplied({0}, {1}, {2}, {3})"
for p, r in [("R", "r"), ("G", "g"), ("B", "b"), ("A", "a")]:
    m[f"P:{C}.{p}"] = {"get": "{this}.%s()" % r, "set": "{this}.set_%s({value})" % r}
m[f"P:{C}.PackedValue"] = {"get": "{this}.packed_value()", "set": "{this}.set_packed_value({value})"}
m[f"M:{C}.ToVector3"] = "{this}.to_vector3()"
m[f"M:{C}.ToVector4"] = "{this}.to_vector4()"
m[f"M:{C}.Multiply({C},{F})"] = R + "Color::multiply({0}, {1})"
m[f"M:{C}.op_Multiply({C},{F})"] = "({0} * {1})"
m[f"M:{C}.op_Equality({C},{C})"] = "({0} == {1})"
m[f"M:{C}.op_Inequality({C},{C})"] = "({0} != {1})"
m[f"M:{C}.Equals({C})"] = "({this} == {0})"
m[f"M:{C}.Lerp({C},{C},{F})"] = R + "Color::lerp({0}, {1}, {2})"
with open(os.path.join(XNA, "color.rs"), encoding="utf-8") as fh:
    for name in re.findall(r"pub const ([A-Z][A-Z_]*): Color = Color::from_packed_value", fh.read()):
        m[f"P:{C}.{pascal(name)}"] = {"get": f"{R}Color::{name}"}

m[f"M:{RC}.#ctor({I},{I},{I},{I})"] = R + "Rectangle::new({0}, {1}, {2}, {3})"
for f in ["X", "Y", "Width", "Height"]:
    m[f"F:{RC}.{f}"] = "{this}." + f.lower()
for p in ["Left", "Right", "Top", "Bottom", "Center", "IsEmpty"]:
    m[f"P:{RC}.{p}"] = {"get": "{this}.%s()" % re.sub(r"(?<=[a-z])(?=[A-Z])", "_", p).lower()}
m[f"P:{RC}.Empty"] = {"get": R + "Rectangle::EMPTY"}
m[f"P:{RC}.Location"] = {"get": "{this}.location()", "set": "{this}.set_location({value})"}
m[f"M:{RC}.Intersects({RC})"] = "{this}.intersects({0})"
m[f"M:{RC}.Contains({I},{I})"] = "{this}.contains_xy({0}, {1})"
m[f"M:{RC}.Contains({PT})"] = "{this}.contains_point({0})"
m[f"M:{RC}.Contains({RC})"] = "{this}.contains({0})"
m[f"M:{RC}.Inflate({I},{I})"] = "{this}.inflate({0}, {1})"
m[f"M:{RC}.Offset({I},{I})"] = "{this}.offset_xy({0}, {1})"
m[f"M:{RC}.Offset({PT})"] = "{this}.offset({0})"
m[f"M:{RC}.Intersect({RC},{RC})"] = R + "Rectangle::intersect({0}, {1})"
m[f"M:{RC}.Union({RC},{RC})"] = R + "Rectangle::union({0}, {1})"
m[f"M:{RC}.op_Equality({RC},{RC})"] = "({0} == {1})"
m[f"M:{RC}.op_Inequality({RC},{RC})"] = "({0} != {1})"
m[f"M:{RC}.Equals({RC})"] = "({this} == {0})"

m[f"M:{PT}.#ctor({I},{I})"] = R + "Point::new({0}, {1})"
m[f"F:{PT}.X"] = "{this}.x"
m[f"F:{PT}.Y"] = "{this}.y"
m[f"P:{PT}.Zero"] = {"get": R + "Point::ZERO"}
m[f"M:{PT}.op_Equality({PT},{PT})"] = "({0} == {1})"
m[f"M:{PT}.op_Inequality({PT},{PT})"] = "({0} != {1})"
m[f"M:{PT}.Equals({PT})"] = "({this} == {0})"

for meth, rust, k in [("Clamp", "clamp", 3), ("Lerp", "lerp", 3), ("WrapAngle", "wrap_angle", 1), ("Max", "max", 2), ("Min", "min", 2),
                      ("ToRadians", "to_radians", 1), ("ToDegrees", "to_degrees", 1), ("Distance", "distance", 2), ("SmoothStep", "smooth_step", 3),
                      ("Barycentric", "barycentric", 5), ("CatmullRom", "catmull_rom", 5), ("Hermite", "hermite", 5)]:
    m[f"M:{MH}.{meth}({','.join([F] * k)})"] = f"{R}math_helper::{rust}({', '.join('{%d}' % i for i in range(k))})"

out = {
    "nativeTypes": {
        "T:" + NS + n: {"rust": R + n, "kind": "struct"} for n in ["Color", "Vector2", "Vector3", "Vector4", "Matrix", "Rectangle", "Point"]
    },
    "nativeMembers": dict(sorted(m.items())),
}
path = os.path.join(HERE, "native_xna.json")
text = "// @generated by gen_native_xna.py. Do not edit.\n" + json.dumps(out, indent=2) + "\n"
with open(path, "w", encoding="utf-8", newline="\n") as fh:
    fh.write(text)
print(f"{len(m)} members -> {path}")
