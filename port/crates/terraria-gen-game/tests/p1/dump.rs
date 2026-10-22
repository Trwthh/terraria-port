//! `tools/refdump/RefDump.cs` in Rust: the reflection serialiser `S(object, depth)` / `Members(...)` that wrote
//! `refdata/*.json`, applied to the TRANSLATED objects through the generated reflection data (`gen::meta`) and the
//! runtime's view of its containers (`rt::obj_reflect`). Same rules, same order of member reads (the order matters:
//! property getters run game code). The result is a tree that `compare` checks against the dump value by value.
//!
//! Line numbers in comments refer to RefDump.cs.

use super::json::Json;
use std::collections::{BTreeMap, HashMap};
use terraria_core::xna::{Color, Point, Point16, Rectangle, Vector2, Vector3, Vector4};
use terraria_gen_game::gen::{meta, types};
use terraria_gen_game::rt::{self, tid, FieldMeta, Obj, PropMeta, Reflect, TypeMeta};

/// A serialised value: what RefDump would have written as JSON text.
#[derive(Clone, Debug)]
pub enum Got {
    Null,
    Bool(bool),
    Int(i128),
    F32(f32),
    F64(f64),
    Str(String),
    Arr(Vec<Got>),
    Obj(Vec<(String, Got)>),
    /// A delegate with a lambda among its targets. RefDump printed the compiler-generated method name
    /// (`<>c.<.cctor>b__96_0`), which translated code does not have: comparable only as "a lambda".
    Lambda,
}

pub const MAX_DEPTH: u32 = 7;

pub struct Dumper {
    methods: HashMap<u64, &'static str>,
    stack: Vec<usize>,
    /// What RefDump calls `Skip`: members left out because their value is not serialisable.
    pub skipped: Vec<String>,
    /// Things this port could not do (a runtime type without reflection data ...): each fails the proof.
    pub problems: BTreeMap<String, u64>,
    pub ctx: String,
}

fn type_simple(t: Obj) -> bool {
    if t.is_null() {
        return false;
    }
    let (name, flags, _) = rt::type_info(t);
    let name = name.as_str();
    (flags & rt::type_flags::PRIMITIVE != 0 && name != "System.IntPtr" && name != "System.UIntPtr") || name == "System.String" || flags & rt::type_flags::ENUM != 0 || name == "System.Decimal"
}
fn type_simple_or_xna(t: Obj) -> bool {
    if t.is_null() {
        return false;
    }
    type_simple(t) || matches!(rt::type_info(t).0.as_str(), "Microsoft.Xna.Framework.Color" | "Microsoft.Xna.Framework.Vector2" | "Microsoft.Xna.Framework.Rectangle" | "Microsoft.Xna.Framework.Point")
}

pub fn ord(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

impl Got {
    fn num(&self) -> Option<f64> {
        match self {
            Got::Int(i) => Some(*i as f64),
            Got::Bool(b) => Some(*b as u8 as f64),
            Got::F32(f) => Some(*f as f64),
            Got::F64(f) => Some(*f),
            _ => None,
        }
    }
}

/// The type id in the object's header and, for an enum, the underlying integer id.
unsafe fn header(v: Obj) -> (u32, Option<&'static rt::EnumInfo>) {
    let id = rt::type_id_of(v.0 as *const u8);
    (id, rt::enum_info(id))
}

unsafe fn read_int(v: Obj, id: u32) -> Option<i128> {
    Some(match id {
        tid::U8 => *(v.0 as *const u8) as i128,
        tid::I8 => *(v.0 as *const i8) as i128,
        tid::I16 => *(v.0 as *const i16) as i128,
        tid::U16 => *(v.0 as *const u16) as i128,
        tid::I32 => *(v.0 as *const i32) as i128,
        tid::U32 => *(v.0 as *const u32) as i128,
        tid::I64 => *(v.0 as *const i64) as i128,
        tid::U64 => *(v.0 as *const u64) as i128,
        _ => return None,
    })
}

fn float_got32(f: f32) -> Got {
    // :249 NaN / Infinity are written as strings
    if f.is_nan() {
        Got::Str("NaN".into())
    } else if f.is_infinite() {
        Got::Str(if f > 0.0 { "Infinity" } else { "-Infinity" }.into())
    } else {
        Got::F32(f)
    }
}
fn float_got64(f: f64) -> Got {
    if f.is_nan() {
        Got::Str("NaN".into())
    } else if f.is_infinite() {
        Got::Str(if f > 0.0 { "Infinity" } else { "-Infinity" }.into())
    } else {
        Got::F64(f)
    }
}

impl Dumper {
    pub fn new() -> Dumper {
        meta::register_all();
        Dumper { methods: meta::METHOD_NAMES.iter().copied().collect(), stack: Vec::new(), skipped: Vec::new(), problems: BTreeMap::new(), ctx: String::new() }
    }

    fn skip(&mut self, what: String) {
        let w = format!("{}{}", self.ctx, what);
        if !self.skipped.contains(&w) {
            self.skipped.push(w);
        }
    }
    pub fn problem(&mut self, what: String) {
        *self.problems.entry(what).or_insert(0) += 1;
    }

    /// `IsSimple(v.GetType())` values (:244 `Prim`).
    unsafe fn prim(&mut self, v: Obj) -> Option<Got> {
        let (id, en) = header(v);
        if let Some(info) = en {
            return read_int(v, info.underlying).map(Got::Int);
        }
        Some(match id {
            tid::BOOL => Got::Bool(*(v.0 as *const bool)),
            tid::F32 => float_got32(*(v.0 as *const f32)),
            tid::F64 => float_got64(*(v.0 as *const f64)),
            tid::CHAR => Got::Str(String::from_utf16_lossy(&[*(v.0 as *const u16)])),
            _ => Got::Int(read_int(v, id)?),
        })
    }

    /// `S(v, depth)` (:259): `None` when the value cannot be represented (the caller records a skip).
    pub unsafe fn s(&mut self, v: Obj, depth: u32) -> Option<Got> {
        if v.is_null() {
            return Some(Got::Null);
        }
        let (id, _) = header(v);
        if id == tid::STRING {
            return Some(Got::Str(rt::obj_str(v).as_str().to_owned()));
        }
        if let Some(p) = self.prim(v) {
            return Some(p);
        }
        if id == tid::INT_PTR {
            return None;
        }
        let container = rt::obj_reflect(v);
        if let Reflect::Delegate { targets } = &container {
            // :266 "<delegate:Declaring.Type.Method;...>"
            let mut names = Vec::new();
            for (m, _) in targets {
                match self.methods.get(&(*m as u64)) {
                    Some(n) => names.push(*n),
                    None => return Some(Got::Lambda),
                }
            }
            return Some(Got::Str(format!("<delegate:{}>", names.join(";"))));
        }
        // :271 Type / MemberInfo / Assembly / Stream / Module
        if id == tid::TYPE || rt::obj_is::<rt::FieldInfo>(v) || rt::obj_is::<rt::io::Stream>(v) {
            return None;
        }
        let m = rt::meta_of_obj(v);
        let fname = m.map(|m| m.clr_name).unwrap_or("");
        if fname.starts_with("Microsoft.Xna.Framework.Graphics.") || fname.starts_with("ReLogic.Content.") || fname.starts_with("System.Threading") || fname.starts_with("System.Reflection") {
            return None;
        }
        if id == tid::COLOR {
            let c = *(v.0 as *const Color);
            return Some(Got::Obj(vec![("a".into(), Got::Int(c.a() as i128)), ("b".into(), Got::Int(c.b() as i128)), ("g".into(), Got::Int(c.g() as i128)), ("r".into(), Got::Int(c.r() as i128))]));
        }
        if fname == "Terraria.Localization.LocalizedText" {
            let m = m.unwrap();
            let key = self.member(v, m, "Key")?;
            let value = self.member(v, m, "Value")?;
            return Some(Got::Obj(vec![("key".into(), self.s(key, depth + 1)?), ("value".into(), self.s(value, depth + 1)?)]));
        }
        if fname == "Terraria.Item" && depth > 0 {
            let m = m.unwrap();
            let (st, ty) = (self.member(v, m, "stack")?, self.member(v, m, "type")?);
            return Some(Got::Obj(vec![("stack".into(), self.prim(st)?), ("type".into(), self.prim(ty)?)]));
        }
        if fname == "Terraria.RecipeGroup" {
            let m = m.unwrap();
            let get_text = self.member(v, m, "GetText")?;
            let name = match rt::run(|| rt::as_rt::<rt::Delegate<dyn Fn() -> rt::Str>>(get_text).invoke(|f| f())) {
                Ok(s) if !s.is_null() => Got::Str(s.as_str().to_owned()),
                _ => Got::Null,
            };
            let mut parts = Vec::new();
            for (key, member) in [("decraftItemId", "DecraftItemId"), ("items", "Items")] {
                let x = self.member(v, m, member)?;
                parts.push((key.to_owned(), self.s(x, depth + 1).unwrap_or(Got::Null)));
            }
            parts.push(("name".into(), name));
            for (key, member) in [("registeredId", "RegisteredId"), ("validItems", "ValidItems")] {
                let x = self.member(v, m, member)?;
                parts.push((key.to_owned(), self.s(x, depth + 1).unwrap_or(Got::Null)));
            }
            return Some(Got::Obj(parts));
        }
        if depth > MAX_DEPTH {
            return None;
        }
        match container {
            Reflect::Seq { kind, items } => {
                // :298 arrays, :341 IEnumerable
                let mut list = Vec::with_capacity(items.len());
                let mut bad = false;
                let mut all_simple = !items.is_empty();
                for e in &items {
                    if e.is_null() || header(*e).0 == tid::STRING || self.prim(*e).is_none() {
                        all_simple = false;
                    }
                }
                for e in items {
                    match self.s(e, depth + 1) {
                        Some(g) => list.push(g),
                        None => {
                            bad = true;
                            list.push(Got::Null)
                        }
                    }
                }
                // :346 a HashSet of simple values is written sorted
                if kind == "HashSet" && all_simple {
                    list.sort_by(|a, b| a.num().partial_cmp(&b.num()).unwrap_or(std::cmp::Ordering::Equal));
                }
                if bad {
                    self.skip(format!("element(s) of {kind} not serialisable"));
                }
                Some(Got::Arr(list))
            }
            Reflect::Multi { dims, items } => {
                // :376 SMulti: nested arrays, row-major
                fn nest(d: &mut Dumper, dims: &[usize], items: &[Obj], depth: u32) -> Got {
                    if dims.len() == 1 {
                        return Got::Arr(items.iter().map(|e| unsafe { d.s(*e, depth + 1) }.unwrap_or(Got::Null)).collect());
                    }
                    let step = items.len() / dims[0].max(1);
                    Got::Arr((0..dims[0]).map(|i| nest(d, &dims[1..], &items[i * step..(i + 1) * step], depth)).collect())
                }
                Some(nest(self, &dims, &items, depth))
            }
            Reflect::Dict { items } => {
                // :319 IDictionary: keys as text, numeric keys sorted by value, others ordinal
                let mut rows: Vec<(String, Option<f64>, Obj)> = Vec::new();
                let mut all_num = true;
                for (k, val) in items {
                    let kid = header(k).0;
                    let numeric = rt::enum_info(kid).is_some() || matches!(kid, tid::U8 | tid::I8 | tid::I16 | tid::U16 | tid::I32 | tid::U32 | tid::I64 | tid::U64);
                    all_num &= numeric;
                    let (text, n) = if kid == tid::STRING {
                        (rt::obj_str(k).as_str().to_owned(), None)
                    } else {
                        match self.prim(k) {
                            Some(Got::Int(i)) => (i.to_string(), Some(i as f64)),
                            Some(Got::Bool(b)) => (b.to_string(), None),
                            Some(Got::Str(s)) => (s, None),
                            Some(Got::F32(f)) => (format!("{f}"), None),
                            Some(Got::F64(f)) => (format!("{f}"), None),
                            _ => (rt::obj_to_string(k).as_str().to_owned(), None),
                        }
                    };
                    rows.push((text, n, val));
                }
                if all_num {
                    rows.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                } else {
                    rows.sort_by(|a, b| ord(&a.0, &b.0));
                }
                let mut parts = Vec::new();
                let mut bad = false;
                for (k, _, val) in rows {
                    let g = match self.s(val, depth + 1) {
                        Some(g) => g,
                        None => {
                            bad = true;
                            Got::Null
                        }
                    };
                    parts.push((k, g));
                }
                if bad {
                    self.skip("value(s) of Dictionary`2 not serialisable".into());
                }
                Some(Got::Obj(parts))
            }
            Reflect::Delegate { .. } => unreachable!(),
            Reflect::Other => {
                // :362 plain struct / class
                let is_ref = m.map(|m| m.tid == 0).unwrap_or(true);
                if is_ref {
                    if self.stack.contains(&v.addr()) {
                        return None;
                    }
                    self.stack.push(v.addr());
                }
                let r = self.members(v, depth, 0);
                if is_ref {
                    self.stack.pop();
                }
                r
            }
        }
    }

    /// Native value types (no generated reflection data): their public fields, as RefDump read them.
    unsafe fn native_fields(&mut self, v: Obj) -> Option<Vec<(String, Got)>> {
        let id = header(v).0;
        let f = float_got32;
        let i = |x: i32| Got::Int(x as i128);
        Some(match id {
            tid::VECTOR2 => {
                let p = *(v.0 as *const Vector2);
                vec![("X".into(), f(p.x)), ("Y".into(), f(p.y))]
            }
            tid::VECTOR3 => {
                let p = *(v.0 as *const Vector3);
                vec![("X".into(), f(p.x)), ("Y".into(), f(p.y)), ("Z".into(), f(p.z))]
            }
            tid::VECTOR4 => {
                let p = *(v.0 as *const Vector4);
                vec![("W".into(), f(p.w)), ("X".into(), f(p.x)), ("Y".into(), f(p.y)), ("Z".into(), f(p.z))]
            }
            tid::POINT => {
                let p = *(v.0 as *const Point);
                vec![("X".into(), i(p.x)), ("Y".into(), i(p.y))]
            }
            tid::RECTANGLE => {
                let p = *(v.0 as *const Rectangle);
                vec![("Height".into(), i(p.height)), ("Width".into(), i(p.width)), ("X".into(), i(p.x)), ("Y".into(), i(p.y))]
            }
            _ if rt::obj_is::<Point16>(v) => {
                let p = *(v.0 as *const Point16);
                vec![("X".into(), i(p.x as i32)), ("Y".into(), i(p.y as i32))]
            }
            _ => return None,
        })
    }

    fn levels(m: &'static TypeMeta) -> Vec<&'static TypeMeta> {
        let mut v = vec![m];
        let mut c = m;
        while let Some(b) = c.base {
            v.push(b);
            c = b;
        }
        v
    }

    /// `GetMember(o, name)` (:391): a field or property of any visibility, most derived first.
    pub unsafe fn member(&mut self, v: Obj, m: &'static TypeMeta, name: &str) -> Option<Obj> {
        for l in Self::levels(m) {
            if let Some(f) = l.fields.iter().find(|f| !f.is_static && f.name == name) {
                return Some((f.get)(v));
            }
            if let Some(p) = l.props.iter().find(|p| !p.is_static && p.name == name) {
                if let Some(g) = p.get {
                    return Some(g(v));
                }
            }
        }
        self.problem(format!("member {name} not found on {}", m.clr_name));
        None
    }

    /// `FieldsOf(t)` (:406): public instance fields including inherited ones, by ordinal name.
    fn fields_of(&mut self, m: &'static TypeMeta) -> Vec<&'static FieldMeta> {
        let mut out: Vec<&'static FieldMeta> = Vec::new();
        for l in Self::levels(m) {
            if !l.members && l.instance_fields {
                self.problem(format!("instance members of {} are not generated (config \"reflect\")", l.clr_name));
            }
            out.extend(l.fields.iter().filter(|f| !f.is_static && f.public));
        }
        out.sort_by(|a, b| ord(a.name, b.name));
        out
    }
    /// `PropsOf(t)` (:416): readable public instance properties, the most derived declaration of each name.
    fn props_of(m: &'static TypeMeta) -> Vec<&'static PropMeta> {
        let mut out: Vec<&'static PropMeta> = Vec::new();
        for l in Self::levels(m) {
            for p in l.props.iter().filter(|p| !p.is_static && p.public) {
                if !out.iter().any(|x| x.name == p.name) {
                    out.push(p);
                }
            }
        }
        out.sort_by(|a, b| ord(a.name, b.name));
        out
    }

    /// `Members(v, depth, propsMode)` (:427). 0: public fields (simple properties when there are none);
    /// 1: fields + simple public properties; 2: fields + every readable public property.
    pub unsafe fn members(&mut self, v: Obj, depth: u32, props_mode: u32) -> Option<Got> {
        let Some(m) = rt::meta_of_obj(v) else {
            if let Some(parts) = self.native_fields(v) {
                return Some(Got::Obj(parts));
            }
            let id = header(v).0;
            let name = if (id as usize) < types::NAMES.len() && id >= tid::FIRST_GENERATED { types::NAMES[id as usize].to_owned() } else { format!("type id {id}: {}", rt::obj_to_string(v).as_str()) };
            self.problem(format!("no reflection data for the runtime type {name} (config \"reflect\")"));
            return None;
        };
        let tname = m.clr_name.rsplit(|c| c == '.' || c == '+').next().unwrap_or(m.clr_name);
        let mut parts: Vec<(String, Got)> = Vec::new();
        let fs = self.fields_of(m);
        for f in &fs {
            let fv = (f.get)(v);
            match self.s(fv, depth + 1) {
                Some(g) => parts.push((f.name.to_owned(), g)),
                None => self.skip(format!("{tname}.{}", f.name)),
            }
        }
        if props_mode > 0 || fs.is_empty() {
            let mode = if props_mode == 0 { 1 } else { props_mode };
            for p in Self::props_of(m) {
                if mode == 1 && !type_simple_or_xna((p.prop_type)()) {
                    continue;
                }
                if parts.iter().any(|x| x.0 == p.name) {
                    continue;
                }
                let Some(get) = p.get else {
                    self.problem(format!("property {}.{} has no generated getter", m.clr_name, p.name));
                    continue;
                };
                // :448 a getter that throws is a skip
                let pv = match rt::run(|| get(v)) {
                    Ok(x) => x,
                    Err(e) => {
                        self.skip(format!("{tname}.{}: getter threw {}", p.name, e.split(':').next().unwrap_or("")));
                        continue;
                    }
                };
                match self.s(pv, depth + 1) {
                    Some(g) => parts.push((p.name.to_owned(), g)),
                    None => self.skip(format!("{tname}.{}", p.name)),
                }
            }
        }
        if parts.is_empty() {
            // :455 no public data at all: the private instance fields
            let mut private: Vec<&'static FieldMeta> = Vec::new();
            for (k, l) in Self::levels(m).into_iter().enumerate() {
                // (inherited private fields are not visible to GetFields(NonPublic); protected / internal ones are,
                // which the metadata does not tell apart: only the type's own level is read)
                if k == 0 {
                    private.extend(l.fields.iter().filter(|f| !f.is_static && !f.public && !f.name.contains('<')));
                }
            }
            private.sort_by(|a, b| ord(a.name, b.name));
            for f in private {
                let fv = (f.get)(v);
                match self.s(fv, depth + 1) {
                    Some(g) => parts.push((f.name.to_owned(), g)),
                    None => self.skip(format!("{tname}.{}", f.name)),
                }
            }
        }
        if parts.is_empty() {
            return None;
        }
        parts.sort_by(|a, b| ord(&a.0, &b.0));
        Some(Got::Obj(parts))
    }

    /// `StaticPublicFields(t)` (:508): public static non-literal fields by ordinal name.
    pub fn static_public_fields(m: &'static TypeMeta) -> Vec<&'static FieldMeta> {
        let mut v: Vec<&'static FieldMeta> = m.fields.iter().filter(|f| f.is_static && f.public && !f.literal).collect();
        v.sort_by(|a, b| ord(a.name, b.name));
        v
    }
}

// ---------------------------------------------------------------------------------------------- comparison

#[derive(Default)]
pub struct Stats {
    pub compared: u64,
    pub exact: u64,
    /// Lambda delegates: compared as "is a lambda" only (see `Got::Lambda`).
    pub lambdas: u64,
    /// First mismatches (path, description), and the count per member name.
    pub examples: Vec<String>,
    pub by_member: BTreeMap<String, u64>,
}

impl Stats {
    pub fn not_exact(&self) -> u64 {
        self.compared - self.exact
    }
    fn miss(&mut self, path: &str, member: &str, what: String) {
        *self.by_member.entry(member.to_owned()).or_insert(0) += 1;
        if self.examples.len() < 12 {
            self.examples.push(format!("{path}: {what}"));
        }
    }
}

fn leaves(j: &Json) -> u64 {
    match j {
        Json::Arr(a) => a.iter().map(leaves).sum(),
        Json::Obj(m) => m.iter().map(|(_, v)| leaves(v)).sum(),
        _ => 1,
    }
}

fn cut(s: String) -> String {
    if s.chars().count() > 100 {
        s.chars().take(100).collect::<String>() + "..."
    } else {
        s
    }
}

/// Compares one dumped value with the translated one. Every scalar of the dump counts as one value; a
/// structural difference (missing member, other length, other kind) makes all scalars below it not exact.
pub fn compare(expected: &Json, got: Option<&Got>, path: &str, member: &str, st: &mut Stats) {
    let n = leaves(expected);
    let Some(got) = got else {
        st.compared += n;
        st.miss(path, member, format!("expected {} but the translated side has no value", cut(expected.text())));
        return;
    };
    match (expected, got) {
        (Json::Arr(a), Got::Arr(b)) => {
            if a.len() != b.len() {
                st.compared += n.max(1);
                st.miss(path, member, format!("array length {} expected {}", b.len(), a.len()));
                return;
            }
            for (k, (x, y)) in a.iter().zip(b).enumerate() {
                compare(x, Some(y), &format!("{path}[{k}]"), member, st);
            }
        }
        (Json::Obj(m), Got::Obj(f)) => {
            for (k, v) in m {
                let g = f.iter().find(|(n, _)| n == k).map(|(_, g)| g);
                compare(v, g, &format!("{path}.{k}"), if member.is_empty() { k } else { member }, st);
            }
            for (k, _) in f {
                if !m.iter().any(|(n, _)| n == k) {
                    st.compared += 1;
                    st.miss(path, if member.is_empty() { k } else { member }, format!("member {k} is not in the dump"));
                }
            }
        }
        _ => {
            st.compared += n.max(1);
            let ok = match (expected, got) {
                (Json::Null, Got::Null) => true,
                (Json::Bool(a), Got::Bool(b)) => a == b,
                (Json::Num(a), Got::Int(b)) => a.parse::<i128>().ok() == Some(*b),
                // "R" formatting round-trips: equal when the text parses to the same bits
                (Json::Num(a), Got::F32(b)) => matches!(a.parse::<f32>(), Ok(e) if e.to_bits() == b.to_bits() || (e == 0.0 && *b == 0.0 && a.starts_with('-') == b.is_sign_negative())),
                (Json::Num(a), Got::F64(b)) => matches!(a.parse::<f64>(), Ok(e) if e.to_bits() == b.to_bits() || (e == 0.0 && *b == 0.0 && a.starts_with('-') == b.is_sign_negative())),
                (Json::Str(a), Got::Str(b)) => a == b,
                (Json::Str(a), Got::Lambda) => {
                    let lambda = a.starts_with("<delegate:") && a.contains("<>c");
                    if lambda {
                        st.lambdas += 1;
                    }
                    lambda
                }
                _ => false,
            };
            if ok && n == 1 {
                st.exact += 1;
            } else {
                st.miss(path, member, format!("expected {} got {}", cut(expected.text()), cut(format!("{got:?}"))));
            }
        }
    }
}
