//! Proof 4: every NPC type and every projectile type, simulated by the translated game, against the real dedicated
//! server (`refdata/entities`, harness `tools/refdump_entities/Entities.cs`; format `refdata/entities/README.md`).
//!
//! The test performs the harness's sequence with translated code only: the ticks harness's load (same pins), then
//! batch by batch the same reset (changed tiles restored, `NPC.ClearAll`, `Projectile.ClearAll`, fresh world items and
//! slot players, `Main.rand` and `Main._rngs`), the same slot players (`Player.Spawn` at the dumped SpawnX / SpawnY),
//! the same spawn calls at the dumped positions, and per tick the entity part of `Main.DoUpdateInWorld` in its
//! `SwapRandom` scopes. After every tick it compares, bit for bit, every leaf of every active NPC, projectile, player
//! and world item (all primitive instance fields, flat structs, primitive arrays as hashes: 195 / 109 / 1180 / 32
//! leaves) and the state of `Main.rand` and of every `Main._rngs` stream. Before every batch it compares the tiles
//! the previous batch changed (count and hash).
//!
//! Report: per batch the first difference (tick, entity, leaf, both values) and the slot it belongs to (nearest slot
//! by the entity's position in the reference); per type `exact`, `root` (its slot holds its batch's first difference),
//! `after` (differs only after another slot's first difference in the same batch: shared RNG streams and the player
//! slots make a later difference possibly a consequence) or `spawn` (the spawn call gave another slot / exception).
//! A table goes to `target-gen/entities-<kind>.tsv`.
//!
//! Items (`ENTITIES_KIND=item`): every item type with a use style is in inventory slot 0 of its slot's player, whose
//! controls say "use item" (facing right) before every tick, as a client holding the button sends them.
//!
//! Client-side item use (`ENTITIES_KIND=citem`, tools/refdump_entities/ClientItems.cs): the same item types, one per
//! batch, used by the OWNING player of a single player game (Main.myPlayer = 0, netMode 0) for 60 ticks at a fixed
//! target: shooting, placing / digging tiles, consumables, mana and ammo. The test performs the harness's set-up
//! (screen, game view, achievements, lighting engine, stand-in fonts, Main.ignoreErrors = false), its reset (tiles,
//! chests / signs / tile entities created since the load, entities, the player with the item and the ammo list of the
//! dump), the held mouse button on the dumped target every tick, and compares after every tick also the tiles of the
//! columns around the player that differ from the loaded world (count and hash). A tick that throws on both sides at
//! the same tick (Main.ignoreErrors = false) is a match; the batch ends there.
//!
//! `ENTITIES_TRACE=<batch>:<class>:<index>` prints every differing leaf of one entity per tick
//! (`ENTITIES_TRACE_LEAVES=a,b`: those leaves also while equal; `ENTITIES_TRACE_STEPS=1`: that entity's leaves, or
//! the listed ones, after each part of every tick: players, NPCs, projectiles, items).
//! `ENTITIES_KIND=npc|proj|item|citem` (default npc); `ENTITIES_REF=<file>`: another dump of that kind; `ENTITIES_BATCHES=<n>` stops after n batches; `ENTITIES_ONLY=<b>[,<b>..]`
//! runs only those batches (each batch starts from the same reset, so any one of them can run alone);
//! `ENTITIES_REQUIRE=all` fails unless every type is exact; `ENTITIES_ALLOW=<file>`: fails when a type differs that
//! the file (lines `<kind> <type> <reason>`) does not list, or a listed one is exact.
//! Run: `CARGO_TARGET_DIR=target-gen/fast CARGO_PROFILE_DEV_OPT_LEVEL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo test -p
//! terraria-gen-game --test entities -- --nocapture`

use std::collections::{BTreeMap, HashMap};
use terraria_core::xna::{Color, Point, Point16, Rectangle, Vector2, Vector3, Vector4};
use terraria_gen_game::gen::{ifaces, meta, terraria};
use terraria_gen_game::rt::{self, tid, FieldMeta, Gc, Obj, PropMeta, Str, TypeMeta};

const MISSING: u64 = 0xDEADBEEFDEADBEEF;
const FNV_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;
const CLASSES: [&str; 4] = ["npc", "projectile", "player", "item"];
const COUNTS: [i32; 4] = [200, 1000, 255, 400];
const SLOT_COUNT: i32 = 31;

fn fnv(mut h: u64, b: &[u8]) -> u64 {
    for x in b {
        h = (h ^ *x as u64).wrapping_mul(FNV_PRIME);
    }
    h
}

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_owned()
    } else {
        "<non-string panic payload>".to_owned()
    }
}

/// Runs `f` as translated code: Err(text) for a C# exception or a stop (panic).
fn guarded(f: impl FnOnce()) -> Result<(), String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt::run(f))) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(format!("C# exception: {e}")),
        Err(p) => Err(format!("stopped: {}", panic_text(&p))),
    }
}

fn step(name: &str, f: impl FnOnce()) {
    let t0 = std::time::Instant::now();
    match guarded(f) {
        Ok(()) => eprintln!("ok   {name}   ({:.2} s)", t0.elapsed().as_secs_f64()),
        Err(e) => panic!("{name}: {e}"),
    }
}

// ------------------------------------------------------------------------------------------------ dump reader
struct Rd {
    b: Vec<u8>,
    p: usize,
}
impl Rd {
    fn take<const N: usize>(&mut self) -> [u8; N] {
        let a: [u8; N] = self.b[self.p..self.p + N].try_into().unwrap();
        self.p += N;
        a
    }
    fn u8(&mut self) -> u8 {
        self.take::<1>()[0]
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take())
    }
    fn i32(&mut self) -> i32 {
        i32::from_le_bytes(self.take())
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take())
    }
    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take())
    }
    fn f32(&mut self) -> f32 {
        f32::from_le_bytes(self.take())
    }
    fn str(&mut self) -> String {
        let n = self.u16() as usize;
        let s = String::from_utf8(self.b[self.p..self.p + n].to_vec()).unwrap();
        self.p += n;
        s
    }
}

// ------------------------------------------------------------------------------------------------ leaves
/// Value bits of a boxed primitive / enum (the harness's `Bits`).
unsafe fn bits(v: Obj) -> Option<u64> {
    if v.is_null() {
        return None;
    }
    let id = rt::type_id_of(v.0 as *const u8);
    let id = rt::enum_info(id).map(|i| i.underlying).unwrap_or(id);
    Some(match id {
        tid::BOOL => *(v.0 as *const bool) as u64,
        tid::U8 => *(v.0 as *const u8) as u64,
        tid::I8 => *(v.0 as *const i8) as i64 as u64,
        tid::I16 => *(v.0 as *const i16) as i64 as u64,
        tid::U16 | tid::CHAR => *(v.0 as *const u16) as u64,
        tid::I32 => *(v.0 as *const i32) as i64 as u64,
        tid::U32 => *(v.0 as *const u32) as u64,
        tid::I64 => *(v.0 as *const i64) as u64,
        tid::U64 => *(v.0 as *const u64),
        tid::F32 => (*(v.0 as *const f32)).to_bits() as u64,
        tid::F64 => (*(v.0 as *const f64)).to_bits(),
        _ => return None,
    })
}
/// Raw little-endian bytes of a boxed array element (the harness's `RawBytes`).
unsafe fn raw(v: Obj, out: &mut Vec<u8>) -> bool {
    if v.is_null() {
        return false;
    }
    let id = rt::type_id_of(v.0 as *const u8);
    let id = rt::enum_info(id).map(|i| i.underlying).unwrap_or(id);
    match id {
        tid::BOOL | tid::U8 | tid::I8 => out.push(*(v.0 as *const u8)),
        tid::I16 | tid::U16 | tid::CHAR => out.extend_from_slice(&*(v.0 as *const [u8; 2])),
        tid::I32 | tid::U32 | tid::F32 => out.extend_from_slice(&*(v.0 as *const [u8; 4])),
        tid::I64 | tid::U64 | tid::F64 => out.extend_from_slice(&*(v.0 as *const [u8; 8])),
        tid::VECTOR2 => {
            let p = *(v.0 as *const Vector2);
            out.extend_from_slice(&p.x.to_le_bytes());
            out.extend_from_slice(&p.y.to_le_bytes());
        }
        _ => return false,
    }
    true
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
fn find_field(m: &'static TypeMeta, name: &str) -> Option<&'static FieldMeta> {
    levels(m).into_iter().find_map(|l| l.fields.iter().find(|f| !f.is_static && f.name == name))
}
fn find_prop(m: &'static TypeMeta, name: &str) -> Option<&'static PropMeta> {
    levels(m).into_iter().find_map(|l| l.props.iter().find(|p| !p.is_static && p.name == name))
}

enum Mode {
    Scalar,
    Struct(Vec<String>),
    /// element leaves after the hash leaf
    Array(usize),
    Prop(&'static PropMeta),
}
struct Group {
    field: Option<&'static FieldMeta>,
    mode: Mode,
    first: usize,
}
struct Schema {
    leaves: Vec<(String, u8)>,
    groups: Vec<Group>,
    /// leaves this port cannot read (no field of that name in its reflection data ...): not compared, reported
    unreadable: Vec<String>,
    pos_x: Option<usize>,
}

fn base_name(leaf: &str) -> &str {
    let e = leaf.find(|c| c == '.' || c == '[').unwrap_or(leaf.len());
    &leaf[..e]
}

fn make_schema(m: &'static TypeMeta, leaves: Vec<(String, u8)>) -> Schema {
    let mut groups = Vec::new();
    let mut unreadable = Vec::new();
    let mut i = 0;
    while i < leaves.len() {
        let name = leaves[i].0.clone();
        if let Some(p) = name.strip_prefix('=') {
            match find_prop(m, p).filter(|p| p.get.is_some()) {
                Some(pm) => groups.push(Group { field: None, mode: Mode::Prop(pm), first: i }),
                None => unreadable.push(name.clone()),
            }
            i += 1;
            continue;
        }
        let base = base_name(&name).to_owned();
        let mut j = i + 1;
        while j < leaves.len() && base_name(&leaves[j].0) == base && !leaves[j].0.starts_with('=') {
            j += 1;
        }
        let mode = if name.ends_with("[]") {
            Mode::Array(j - i - 1)
        } else if name.contains('.') {
            Mode::Struct(leaves[i..j].iter().map(|(n, _)| n[base.len() + 1..].to_owned()).collect())
        } else {
            Mode::Scalar
        };
        match find_field(m, &base) {
            Some(f) => groups.push(Group { field: Some(f), mode, first: i }),
            None => unreadable.extend(leaves[i..j].iter().map(|(n, _)| n.clone())),
        }
        i = j;
    }
    let pos_x = leaves.iter().position(|(n, _)| n == "position.X");
    Schema { leaves, groups, unreadable, pos_x }
}

impl Schema {
    /// The leaves of `o`; `None` where this port cannot produce the value.
    unsafe fn read(&self, o: Obj, out: &mut Vec<Option<u64>>) {
        out.clear();
        out.resize(self.leaves.len(), None);
        for g in &self.groups {
            match &g.mode {
                Mode::Prop(p) => out[g.first] = bits((p.get.unwrap())(o)),
                Mode::Scalar => out[g.first] = bits((g.field.unwrap().get)(o)),
                Mode::Struct(subs) => {
                    let v = (g.field.unwrap().get)(o);
                    for (k, s) in subs.iter().enumerate() {
                        out[g.first + k] = struct_part(v, s);
                    }
                }
                Mode::Array(n) => {
                    let f = g.field.unwrap();
                    let v = (f.get)(o);
                    if v.is_null() {
                        out[g.first] = Some(0);
                        for k in 0..*n {
                            out[g.first + 1 + k] = Some(MISSING);
                        }
                        continue;
                    }
                    let tname = rt::type_info((f.field_type)()).0;
                    let (hash, elems) = array_leaves(v, tname.as_str(), *n);
                    out[g.first] = hash;
                    for k in 0..*n {
                        out[g.first + 1 + k] = elems.get(k).copied().flatten().or(if k >= elems.len() { Some(MISSING) } else { None });
                    }
                }
            }
        }
    }
}

unsafe fn struct_part(v: Obj, s: &str) -> Option<u64> {
    if v.is_null() {
        return None;
    }
    let id = rt::type_id_of(v.0 as *const u8);
    let f32b = |x: f32| Some(x.to_bits() as u64);
    let i32b = |x: i32| Some(x as i64 as u64);
    match id {
        tid::VECTOR2 => {
            let p = *(v.0 as *const Vector2);
            return match s { "X" => f32b(p.x), "Y" => f32b(p.y), _ => None };
        }
        tid::VECTOR3 => {
            let p = *(v.0 as *const Vector3);
            return match s { "X" => f32b(p.x), "Y" => f32b(p.y), "Z" => f32b(p.z), _ => None };
        }
        tid::VECTOR4 => {
            let p = *(v.0 as *const Vector4);
            return match s { "X" => f32b(p.x), "Y" => f32b(p.y), "Z" => f32b(p.z), "W" => f32b(p.w), _ => None };
        }
        tid::RECTANGLE => {
            let p = *(v.0 as *const Rectangle);
            return match s { "X" => i32b(p.x), "Y" => i32b(p.y), "Width" => i32b(p.width), "Height" => i32b(p.height), _ => None };
        }
        tid::POINT => {
            let p = *(v.0 as *const Point);
            return match s { "X" => i32b(p.x), "Y" => i32b(p.y), _ => None };
        }
        tid::COLOR => {
            let c = *(v.0 as *const Color);
            return if s == "packedValue" { Some(c.packed_value() as u64) } else { None };
        }
        _ => {}
    }
    if rt::obj_is::<Point16>(v) {
        let p = *(v.0 as *const Point16);
        return match s { "X" => Some(p.x as i64 as u64), "Y" => Some(p.y as i64 as u64), _ => None };
    }
    let m = rt::meta_of_obj(v)?;
    let f = find_field(m, s)?;
    bits((f.get)(v))
}

/// (hash leaf, element leaves) of an array; fast paths for the primitive element types by the field's declared type.
unsafe fn array_leaves(v: Obj, tname: &str, n: usize) -> (Option<u64>, Vec<Option<u64>>) {
    macro_rules! prim {
        ($t:ty, $bytes:expr, $leaf:expr) => {{
            let a = rt::cast_rt::<rt::Arr<$t>>(v);
            let s = a.as_slice();
            let mut h = fnv(FNV_BASIS, &(s.len() as i32).to_le_bytes());
            for x in s {
                h = fnv(h, &$bytes(*x));
            }
            let e = s.iter().take(n).map(|x| Some($leaf(*x))).collect();
            (Some(h), e)
        }};
    }
    match tname {
        "System.Single[]" => prim!(f32, |x: f32| x.to_le_bytes(), |x: f32| x.to_bits() as u64),
        "System.Double[]" => prim!(f64, |x: f64| x.to_le_bytes(), |x: f64| x.to_bits()),
        "System.Int32[]" => prim!(i32, |x: i32| x.to_le_bytes(), |x: i32| x as i64 as u64),
        "System.UInt32[]" => prim!(u32, |x: u32| x.to_le_bytes(), |x: u32| x as u64),
        "System.Int16[]" => prim!(i16, |x: i16| x.to_le_bytes(), |x: i16| x as i64 as u64),
        "System.UInt16[]" => prim!(u16, |x: u16| x.to_le_bytes(), |x: u16| x as u64),
        "System.Byte[]" => prim!(u8, |x: u8| [x], |x: u8| x as u64),
        "System.SByte[]" => prim!(i8, |x: i8| [x as u8], |x: i8| x as i64 as u64),
        "System.Boolean[]" => prim!(bool, |x: bool| [x as u8], |x: bool| x as u64),
        "System.Int64[]" => prim!(i64, |x: i64| x.to_le_bytes(), |x: i64| x as u64),
        "System.UInt64[]" => prim!(u64, |x: u64| x.to_le_bytes(), |x: u64| x),
        "Microsoft.Xna.Framework.Vector2[]" => {
            let a = rt::cast_rt::<rt::Arr<Vector2>>(v);
            let s = a.as_slice();
            let mut h = fnv(FNV_BASIS, &(s.len() as i32).to_le_bytes());
            for p in s {
                h = fnv(h, &p.x.to_le_bytes());
                h = fnv(h, &p.y.to_le_bytes());
            }
            (Some(h), Vec::new())
        }
        _ => match rt::obj_reflect(v) {
            rt::Reflect::Seq { items, .. } => {
                let mut bytes = Vec::new();
                let mut ok = true;
                for e in &items {
                    ok &= raw(*e, &mut bytes);
                }
                let h = fnv(fnv(FNV_BASIS, &(items.len() as i32).to_le_bytes()), &bytes);
                let e = items.iter().take(n).map(|x| bits(*x)).collect();
                (if ok { Some(h) } else { None }, e)
            }
            _ => (None, Vec::new()),
        },
    }
}

// ------------------------------------------------------------------------------------------------ the scenario
unsafe fn swap(key: &str) -> Obj {
    terraria::main::swap_random(Str::new(key))
}
/// `using (SwapRandom(key)) f();` with the try / finally of the C# `using`.
unsafe fn scoped(key: &str, f: impl FnOnce()) {
    let u = swap(key);
    let r = rt::try_(|| -> rt::Flow<()> {
        f();
        rt::Flow::Next
    });
    if !u.is_null() {
        ifaces::system__i_disposable__dispose(u);
    }
    if let Err(e) = r {
        rt::rethrow(e);
    }
}
/// The entity part of Main.DoUpdateInWorld (Entities.cs EntityTick).
/// `step(name)` runs after each of the five parts (ENTITIES_TRACE_STEPS: the traced entity between them).
unsafe fn entity_tick(main: Gc<terraria::main::Main>, step: &mut dyn FnMut(&str)) {
    scoped("DoUpdateInWorld", || {
        let ms = &mut *terraria::main::s();
        ms._game_update_count = ms._game_update_count.wrapping_add(1);
        *ms.tile_solid.at(379) = false;
        scoped("UpdatePlayers", || terraria::main::update_world_players(main.ptr()));
        step("players");
        terraria::game_content::coin_loss_revenge_system::update((*terraria::npc::s()).revenge_manager.ptr());
        scoped("UpdateNPCs", || terraria::main::update_world_np_cs(main.ptr()));
        step("npcs");
        scoped("UpdateProjectiles", || terraria::main::update_world_projectiles(main.ptr()));
        step("projectiles");
        scoped("UpdateItems", || terraria::main::update_world_items(main.ptr()));
        step("items");
        *(*terraria::main::s()).tile_solid.at(379) = true;
    });
}

/// 14-byte tile records, column-major (Common.cs TakeSnapshot).
unsafe fn tile_records() -> Vec<u8> {
    let ms = &*terraria::main::s();
    let mut buf = vec![0u8; ms.max_tiles_x as usize * ms.max_tiles_y as usize * 14];
    let mut o = 0;
    for x in 0..ms.max_tiles_x {
        for y in 0..ms.max_tiles_y {
            let t = *ms.tile.at(x, y);
            if !t.is_null() {
                let t = &*t.ptr();
                buf[o..o + 2].copy_from_slice(&t.type_.to_le_bytes());
                buf[o + 2..o + 4].copy_from_slice(&t.wall.to_le_bytes());
                buf[o + 4] = t.liquid;
                buf[o + 5..o + 7].copy_from_slice(&t.s_tile_header.to_le_bytes());
                buf[o + 7] = t.b_tile_header;
                buf[o + 8] = t.b_tile_header2;
                buf[o + 9] = t.b_tile_header3;
                buf[o + 10..o + 12].copy_from_slice(&t.frame_x.to_le_bytes());
                buf[o + 12..o + 14].copy_from_slice(&t.frame_y.to_le_bytes());
            }
            o += 14;
        }
    }
    buf
}
/// Entities.cs TilesBack: count and hash the tiles that differ from the loaded world, then restore them.
unsafe fn tiles_back(loaded: &[u8]) -> (i32, u64) {
    let ms = &*terraria::main::s();
    let now = tile_records();
    let h = ms.max_tiles_y as usize;
    let (mut n, mut hash) = (0i32, FNV_BASIS);
    for i in 0..now.len() / 14 {
        let o = i * 14;
        if now[o..o + 14] == loaded[o..o + 14] {
            continue;
        }
        n += 1;
        let (x, y) = ((i / h) as i32, (i % h) as i32);
        hash = fnv(hash, &x.to_le_bytes());
        hash = fnv(hash, &y.to_le_bytes());
        hash = fnv(hash, &now[o..o + 14]);
        let t = &mut *(*ms.tile.at(x, y)).ptr();
        let r = &loaded[o..o + 14];
        t.type_ = u16::from_le_bytes([r[0], r[1]]);
        t.wall = u16::from_le_bytes([r[2], r[3]]);
        t.liquid = r[4];
        t.s_tile_header = u16::from_le_bytes([r[5], r[6]]);
        t.b_tile_header = r[7];
        t.b_tile_header2 = r[8];
        t.b_tile_header3 = r[9];
        t.frame_x = i16::from_le_bytes([r[10], r[11]]);
        t.frame_y = i16::from_le_bytes([r[12], r[13]]);
    }
    (n, hash)
}

/// ClientItems.cs RegionTiles: the tiles of columns [x0, x1], rows [y0, y1] that differ from the loaded world (count,
/// hash of (x, y, record), x outer); `restore`: set back to the loaded record.
unsafe fn region_tiles(loaded: &[u8], x0: i32, x1: i32, y0: i32, y1: i32, restore: bool) -> (i32, u64) {
    let ms = &*terraria::main::s();
    let h = ms.max_tiles_y;
    let (mut n, mut hash) = (0i32, FNV_BASIS);
    let mut rec = [0u8; 14];
    for x in x0.max(0)..=x1.min(ms.max_tiles_x - 1) {
        for y in y0.max(0)..=y1.min(h - 1) {
            rec = [0u8; 14];
            let t = *ms.tile.at(x, y);
            if !t.is_null() {
                let t = &*t.ptr();
                rec[0..2].copy_from_slice(&t.type_.to_le_bytes());
                rec[2..4].copy_from_slice(&t.wall.to_le_bytes());
                rec[4] = t.liquid;
                rec[5..7].copy_from_slice(&t.s_tile_header.to_le_bytes());
                rec[7] = t.b_tile_header;
                rec[8] = t.b_tile_header2;
                rec[9] = t.b_tile_header3;
                rec[10..12].copy_from_slice(&t.frame_x.to_le_bytes());
                rec[12..14].copy_from_slice(&t.frame_y.to_le_bytes());
            }
            let o = (x as usize * h as usize + y as usize) * 14;
            if rec[..] == loaded[o..o + 14] {
                continue;
            }
            n += 1;
            hash = fnv(hash, &x.to_le_bytes());
            hash = fnv(hash, &y.to_le_bytes());
            hash = fnv(hash, &rec);
            if restore {
                let t = &mut *(*ms.tile.at(x, y)).ptr();
                let r = &loaded[o..o + 14];
                t.type_ = u16::from_le_bytes([r[0], r[1]]);
                t.wall = u16::from_le_bytes([r[2], r[3]]);
                t.liquid = r[4];
                t.s_tile_header = u16::from_le_bytes([r[5], r[6]]);
                t.b_tile_header = r[7];
                t.b_tile_header2 = r[8];
                t.b_tile_header3 = r[9];
                t.frame_x = i16::from_le_bytes([r[10], r[11]]);
                t.frame_y = i16::from_le_bytes([r[12], r[13]]);
            }
        }
    }
    let _ = rec;
    (n, hash)
}

/// ClientItems.cs FakeFonts: the stand-in font of both sides (U+0020 .. U+007E, glyph 10 x 20, padding 0, kerning
/// (0, 10, 0), default '*', spacing 0, line spacing 20) for ItemStack, MouseText, DeathText and both CombatText fonts.
unsafe fn fake_fonts() {
    use terraria_gen_game::gen::re_logic;
    let font = re_logic::graphics::dynamic_sprite_font::new(0.0, 20, b'*' as u16);
    let glyphs = rt::List::<Rectangle>::new_gc();
    let pads = rt::List::<Rectangle>::new_gc();
    let chars = rt::List::<u16>::new_gc();
    let kern = rt::List::<Vector3>::new_gc();
    for c in 0x20u16..=0x7e {
        (*glyphs.ptr()).add(Rectangle { x: 0, y: 0, width: 10, height: 20 });
        (*pads.ptr()).add(Rectangle { x: 0, y: 0, width: 0, height: 0 });
        (*chars.ptr()).add(c);
        (*kern.ptr()).add(Vector3 { x: 0.0, y: 10.0, z: 0.0 });
    }
    let page = re_logic::graphics::font_page::new(Gc::NULL, glyphs, pads, chars, kern);
    let pages = rt::Arr::<Gc<re_logic::graphics::font_page::FontPage>>::new(1);
    *pages.at(0) = rt::wbv(page);
    re_logic::graphics::dynamic_sprite_font::set_pages(font.ptr(), pages);
    let asset = || {
        let a = re_logic::content::asset__dynamic_sprite_font::new(Str::lit("Fonts/StandIn"));
        (*a.ptr()).value__backing = rt::wbv(font);
        a
    };
    let fa = &mut *terraria::game_content::font_assets::s();
    fa.item_stack = rt::wbv(asset());
    fa.mouse_text = rt::wbv(asset());
    fa.death_text = rt::wbv(asset());
    for i in 0..fa.combat_text.len() {
        *fa.combat_text.at(i) = rt::wbv(asset());
    }
}

unsafe fn rand_state(g: Gc<terraria_core::UnifiedRandom>) -> (u32, u64) {
    let (inext, seed) = (*g.ptr()).state();
    let mut h = FNV_BASIS;
    for x in seed.iter() {
        h = fnv(h, &x.to_le_bytes());
    }
    (inext as u32, h)
}

unsafe fn entity_obj(c: usize, i: i32) -> Option<Obj> {
    let ms = &*terraria::main::s();
    let (o, active) = match c {
        0 => {
            let p = *ms.npc.at(i);
            (p.up::<rt::Object>(), !p.is_null() && (*p.ptr()).active)
        }
        1 => {
            let p = *ms.projectile.at(i);
            (p.up::<rt::Object>(), !p.is_null() && (*p.ptr()).active)
        }
        2 => {
            let p = *ms.player.at(i);
            (p.up::<rt::Object>(), !p.is_null() && (*p.ptr()).active)
        }
        _ => {
            let p = *ms.item.at(i);
            (p.up::<rt::Object>(), !p.is_null() && terraria::world_item::active(p.ptr()))
        }
    };
    active.then_some(o)
}

#[derive(Clone, Copy)]
struct Slot {
    type_: i32,
    ex: i32,
    ey: i32,
    px: i32,
    py: i32,
    pl: i32,
    owner: i32,
    vx: f32,
    vy: f32,
    index: i32,
}

struct Diff {
    tick: i32,
    what: String,
    slot: Option<usize>,
}

#[test]
fn entities_match_the_real_server() {
    let run = || unsafe {
        std::panic::set_hook(Box::new(|_| {}));
        let kind = std::env::var("ENTITIES_KIND").unwrap_or_else(|_| "npc".to_owned());
        assert!(kind == "npc" || kind == "proj" || kind == "item" || kind == "citem", "ENTITIES_KIND=npc|proj|item|citem");
        let citem = kind == "citem";
        let root = terraria_host::sys::io::project_dir();
        terraria_gen_game::host::launch::install_resources(&root);
        terraria_host::game::init(terraria_host::game::WindowState { width: 800, height: 600, x: 0, y: 0, active: true, handle: 0, display_width: 1920, display_height: 1080, scale: 1.0 });
        meta::register_all();
        meta::register_json();

        // ---- the dump
        let ref_path = std::env::var("ENTITIES_REF").map(std::path::PathBuf::from).unwrap_or_else(|_| root.join(format!("refdata/entities/{kind}.ents.zlib")));
        let z = std::fs::read(&ref_path).expect("refdata/entities/<kind>.ents.zlib");
        let data = miniz_oxide::inflate::decompress_to_vec_zlib(&z).expect("zlib");
        eprintln!("reference: {} bytes, {} uncompressed", z.len(), data.len());
        let mut rd = Rd { b: data, p: 0 };
        assert_eq!(&rd.take::<4>(), b"ENTS");
        assert_eq!(rd.u32(), 1);
        assert_eq!(rd.str(), kind);
        let ticks = rd.i32();
        let nbatches = rd.i32();
        let ncls = rd.i32() as usize;
        assert_eq!(ncls, 4);
        let metas: [&'static TypeMeta; 4] = [&meta::TERRARIA_NPC, &meta::TERRARIA_PROJECTILE, &meta::TERRARIA_PLAYER, &meta::TERRARIA_WORLD_ITEM];
        let mut schemas = Vec::new();
        for c in 0..ncls {
            assert_eq!(rd.str(), CLASSES[c]);
            let n = rd.i32() as usize;
            let leaves: Vec<(String, u8)> = (0..n).map(|_| (rd.str(), rd.u8())).collect();
            let s = make_schema(metas[c], leaves);
            if !s.unreadable.is_empty() {
                eprintln!("{}: {} of {} leaves not readable here: {}", CLASSES[c], s.unreadable.len(), s.leaves.len(), s.unreadable.join(", "));
            }
            schemas.push(s);
        }
        // citem: screen size, the per-tick tile box and the per-batch reset region (x0, x1, y0, y1 relative to the
        // player's spawn tile), the ammo list (ClientItems.cs)
        let (screen_w, screen_h) = if citem { (rd.i32(), rd.i32()) } else { (0, 0) };
        let tbox: Vec<i32> = if citem { (0..4).map(|_| rd.i32()).collect() } else { Vec::new() };
        let treset: Vec<i32> = if citem { (0..4).map(|_| rd.i32()).collect() } else { Vec::new() };
        let ammo: Vec<i32> = if citem { (0..rd.i32()).map(|_| rd.i32()).collect() } else { Vec::new() };

        // ---- load (Entities.cs EntRun = Ticks.cs Run)
        let save = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("entities-save-{kind}"));
        let _ = std::fs::remove_dir_all(&save);
        std::fs::create_dir_all(save.join("Worlds")).expect("save root");
        let world = save.join("Worlds").join("ref_small.wld");
        std::fs::copy(root.join("work/refsave/Worlds/ref_small.wld"), &world).expect("copy of the reference world");
        std::env::set_var("TERRARIA_SAVE_DIR", &save);
        let mut main = Gc::NULL;
        step("Program.SavePath = <scratch>", || (*terraria::program::s()).save_path = Str::new(&save.to_string_lossy()));
        step("Main.dedServ = true; Main.showSplash = false", || {
            (*terraria::main::s()).ded_serv = true;
            (*terraria::main::s()).show_splash = false;
        });
        step("LanguageManager.Instance.SetLanguage(GameCulture.DefaultCulture)", || {
            let lm = (*terraria::localization::language_manager::s()).instance;
            terraria::localization::language_manager::set_language__game_culture(lm.ptr(), terraria::localization::game_culture::default_culture());
        });
        step("main = new Main()", || main = terraria::main::new());
        step("Lang.InitializeLegacyLocalization()", || terraria::lang::initialize_legacy_localization());
        step("-world <existing>: WorldFile.GetAllMetadata(path, false).SetAsActive()", || {
            let wfd = terraria::io::world_file::get_all_metadata(Str::new(&world.to_string_lossy()), false);
            assert!(!wfd.is_null(), "GetAllMetadata returned null");
            terraria::io::world_file_data::set_as_active(wfd.ptr());
        });
        step("Main.rand = new UnifiedRandom(0)", || (*terraria::main::s()).rand = rt::wbv(rt::alloc_init(terraria_core::UnifiedRandom::new(0))));
        step("main.Initialize()", || terraria::main::initialize(main.ptr()));
        step("WorldGen.serverLoadWorldCallBack() with Main.rand = new UnifiedRandom(1357)", || {
            (*terraria::main::s()).rand = rt::wbv(rt::alloc_init(terraria_core::UnifiedRandom::new(1357)));
            terraria::world_gen::server_load_world_call_back();
            assert!(!(*terraria::world_gen::s()).load_failed, "WorldGen.loadFailed");
        });
        step("server state: myPlayer = 255; netMode = 2; Netplay.Disconnect = false; Clients[i] = new RemoteClient", || {
            (*terraria::main::s()).my_player = 255;
            (*terraria::main::s()).net_mode = 2;
            (*terraria::netplay::s()).disconnect = false;
            let clients = (*terraria::netplay::s()).clients;
            for i in 0..256 {
                let c = terraria::remote_client::new();
                terraria::remote_client::reset(c.ptr());
                (*c.ptr()).id = i;
                (*c.ptr()).read_buffer = rt::wbv(rt::Arr::<u8>::new(1024));
                *clients.at(i) = rt::wbv(c);
            }
        });
        (*terraria::main::s()).game_menu = false;
        let loaded = tile_records();
        // the harness's boss test before the batches: new NPC().SetDefaults(type) for every type in order
        // the harness's use-style test before the batches: new Item().SetDefaults(type, null) for every item type in order
        if kind == "item" || citem {
            step("new Item().SetDefaults(type, null) for every item type (the harness's use-style test)", || {
                for ty in 1..(*terraria::id::item_id::s()).count as i32 {
                    let it = terraria::item::new__void();
                    terraria::item::set_defaults(it.ptr(), ty, Gc::NULL);
                }
            });
        }
        if kind == "npc" {
            step("new NPC().SetDefaults(type) for every NPC type (the harness's boss test)", || {
                for ty in 1..(*terraria::id::npcid::s()).count as i32 {
                    let n = terraria::npc::new();
                    terraria::npc::set_defaults(n.ptr(), ty, rt::zero());
                }
            });
        }

        // ---- citem: the owning client's set-up (ClientItems.cs CItemRun, same order)
        let mut chest_null: Vec<bool> = Vec::new();
        let mut sign_null: Vec<bool> = Vec::new();
        let mut te_ids: Vec<i32> = Vec::new();
        let mut te_next = 0;
        if citem {
            step("client set-up: myPlayer 0, netMode 0, screen, game view, achievements, lighting, fonts, ignoreErrors = false", || {
                let ms = &mut *terraria::main::s();
                ms.my_player = 0;
                ms.net_mode = 0;
                ms.screen_width = screen_w;
                ms.screen_height = screen_h;
                let svm = terraria::graphics::sprite_view_matrix::new(Gc::NULL);
                terraria::graphics::sprite_view_matrix::set_viewport_override(svm.ptr(), terraria_host::gfx::Viewport::new(0, 0, screen_w, screen_h));
                ms.game_view_matrix = rt::wbv(svm);
                (*main.ptr())._achievements = rt::wbv(terraria::achievements::achievement_manager::new());
                terraria::initializers::achievement_initializer::load();
                terraria::lighting::initialize();
                fake_fonts();
                (*terraria::main::s()).ignore_errors = false;
            });
            let ms = &*terraria::main::s();
            chest_null = (0..ms.chest.len()).map(|i| (*ms.chest.at(i)).is_null()).collect();
            sign_null = (0..ms.sign.len()).map(|i| (*ms.sign.at(i)).is_null()).collect();
            let ts = &*terraria::data_structures::tile_entity::s();
            te_ids = rt::Dictionary::keys(ts.by_id).to_vec();
            te_next = ts.tile_entities_next_id;
        }

        // ---- batches
        let _main_root = rt::Rooted::new(main);
        let only: Option<Vec<i32>> = std::env::var("ENTITIES_ONLY").ok().map(|v| v.split(',').map(|x| x.trim().parse().expect("ENTITIES_ONLY")).collect());
        let limit: i32 = std::env::var("ENTITIES_BATCHES").ok().and_then(|v| v.parse().ok()).unwrap_or(nbatches);
        let mut all_types: Vec<(i32, i32)> = Vec::new(); // (type, batch)
        let mut status: BTreeMap<i32, String> = BTreeMap::new();
        let mut batch_lines = Vec::new();
        let mut batches_exact = 0;
        let mut batches_run = 0;
        let mut tile_problems = Vec::new();
        let mut both_threw: Vec<String> = Vec::new();
        let t_all = std::time::Instant::now();
        let mut cur = vec![Vec::<Option<u64>>::new(); 1];
        // ENTITIES_TRACE=<batch>:<class 0-3>:<index>: every differing leaf of that entity at every tick;
        // ENTITIES_TRACE_LEAVES=<leaf>,..: those leaves also while they are equal
        let trace: Option<(i32, usize, i32)> = std::env::var("ENTITIES_TRACE").ok().map(|v| {
            let p: Vec<i32> = v.split(':').map(|x| x.parse().expect("ENTITIES_TRACE=<batch>:<class>:<index>")).collect();
            (p[0], p[1] as usize, p[2])
        });
        // ENTITIES_PERTURB=1: a negative control (Main.rand seeded with seed + 1; items: the players face left): the
        // comparison must then report differences
        let perturb = std::env::var_os("ENTITIES_PERTURB").is_some();
        let trace_steps = std::env::var_os("ENTITIES_TRACE_STEPS").is_some();
        let trace_leaves: Vec<String> = std::env::var("ENTITIES_TRACE_LEAVES").map(|v| v.split(',').map(str::to_owned).collect()).unwrap_or_default();
        for b in 0..nbatches {
            let bi = rd.i32();
            assert_eq!(bi, b);
            let seed = rd.i32();
            let (want_tc, want_th) = (rd.i32(), rd.u64());
            let ns = rd.i32() as usize;
            let slots: Vec<Slot> = (0..ns)
                .map(|_| Slot { type_: rd.i32(), ex: rd.i32(), ey: rd.i32(), px: rd.i32(), py: rd.i32(), pl: rd.i32(), owner: rd.i32(), vx: rd.f32(), vy: rd.f32(), index: rd.i32() })
                .collect();
            for s in &slots {
                all_types.push((s.type_, b));
            }
            // citem: mouse position on the screen, chests / signs / tile entities the reset removed
            let (mouse_x, mouse_y, want_removed) = if citem { (rd.i32(), rd.i32(), rd.i32()) } else { (0, 0, 0) };
            // the tick records of this batch (read even when the batch is not run)
            let mut recs: Vec<(i32, bool, usize)> = Vec::new();
            for _ in 0..=ticks {
                let t = rd.i32();
                let err = rd.u8() != 0;
                let at = rd.p;
                recs.push((t, err, at));
                if err {
                    break;
                }
                // skip the record
                rd.u32();
                rd.u64();
                let n = rd.u16();
                for _ in 0..n {
                    rd.str();
                    rd.u32();
                    rd.u64();
                }
                for _ in 0..4 {
                    let n = rd.u16();
                    for _ in 0..n {
                        rd.u16();
                        let nch = rd.u16() as usize;
                        rd.p += nch * 10;
                    }
                }
                if citem {
                    rd.i32();
                    rd.u64();
                }
            }
            let wanted = b < limit && only.as_ref().map(|o| o.contains(&b)).unwrap_or(true);
            if !wanted {
                for s in &slots {
                    status.entry(s.type_).or_insert_with(|| "not run".to_owned());
                }
                continue;
            }
            batches_run += 1;
            let tb = std::time::Instant::now();
            // 1-4: reset (citem: the reset region around the player, every batch)
            let (tc, th) = if citem {
                let s0 = &slots[0];
                region_tiles(&loaded, s0.px + treset[0], s0.px + treset[1], s0.py + treset[2], s0.py + treset[3], true)
            } else {
                tiles_back(&loaded)
            };
            if (b > 0 || citem) && only.is_none() && (tc, th) != (want_tc, want_th) {
                tile_problems.push(format!("before batch {b}: {tc} tiles changed by the previous batch (hash {th:016x}), real {want_tc} ({want_th:016x})"));
            }
            let mut diffs: Vec<Diff> = Vec::new();
            let mut spawn_diffs: Vec<usize> = Vec::new();
            // the collector runs only when the host calls it (rt::collect, between batches, no generated code on the
            // stack; Main is rooted below): every 25 batches, for every kind. Without it the item run (150 batches)
            // kept every object of the run registered; its object list reached 4 GB at batch 127 and the next growth
            // (8 GB) failed (gates 2026-10-16)
            if b % 25 == 0 {
                rt::collect();
            }
            if citem {
                let mut removed = 0;
                let r = guarded(|| {
                    let ms = &mut *terraria::main::s();
                    for i in 0..ms.chest.len() {
                        if chest_null[i as usize] && !(*ms.chest.at(i)).is_null() {
                            *ms.chest.at(i) = Gc::NULL;
                            removed += 1;
                        }
                    }
                    for i in 0..ms.sign.len() {
                        if sign_null[i as usize] && !(*ms.sign.at(i)).is_null() {
                            *ms.sign.at(i) = Gc::NULL;
                            removed += 1;
                        }
                    }
                    let ts = &mut *terraria::data_structures::tile_entity::s();
                    let mut gone: Vec<i32> = rt::Dictionary::keys(ts.by_id).to_vec().into_iter().filter(|k| !te_ids.contains(k)).collect();
                    gone.sort();
                    for id in gone {
                        (*ts.by_id.ptr()).remove(id);
                        removed += 1;
                    }
                    for k in rt::Dictionary::keys(ts.by_position).to_vec() {
                        let te = (*ts.by_position.ptr()).get_item(k);
                        if !te_ids.contains(&(*te.ptr()).id) {
                            (*ts.by_position.ptr()).remove(k);
                        }
                    }
                    ts.tile_entities_next_id = te_next;
                });
                if let Err(e) = r {
                    tile_problems.push(format!("before batch {b}: removing chests / signs / tile entities: {e}"));
                }
                if removed != want_removed {
                    tile_problems.push(format!("before batch {b}: {removed} chests / signs / tile entities removed here, {want_removed} in the real run"));
                }
            }
            let r = guarded(|| {
                let ms = &mut *terraria::main::s();
                terraria::npc::clear_all();
                terraria::projectile::clear_all();
                for i in 0..400 {
                    let wi = terraria::world_item::new__void();
                    (*wi.ptr()).base.who_am_i = i;
                    *ms.item.at(i) = rt::wbv(wi);
                    *ms.time_item_slot_cannot_be_reused_for.at(i) = 0;
                }
                for i in 0..(if citem { 1 } else { SLOT_COUNT }) {
                    let p = terraria::player::new();
                    (*p.ptr()).base.who_am_i = i;
                    *ms.player.at(i) = rt::wbv(p);
                }
                ms.rand = rt::wbv(rt::alloc_init(terraria_core::UnifiedRandom::new(if perturb && (kind != "item") { seed + 1 } else { seed })));
                ms._rngs = rt::wbv(rt::Dictionary::<Str, Gc<terraria_core::UnifiedRandom>>::new_gc());
                // 5: players
                if citem {
                    let s = &slots[0];
                    let p = *ms.player.at(0);
                    let pl = &mut *p.ptr();
                    pl.active = true;
                    pl.name = Str::lit("Ref");
                    pl.base.who_am_i = 0;
                    pl.stat_life_max = 100;
                    pl.stat_life_max2 = 100;
                    pl.stat_life = 60;
                    pl.stat_mana_max = 200;
                    pl.stat_mana_max2 = 200;
                    pl.stat_mana = 100;
                    pl.spawn_x = s.px;
                    pl.spawn_y = s.py;
                    // spawned as a server spawns it (Main.myPlayer = 255 meanwhile), then the local player again
                    ms.my_player = 255;
                    terraria::player::spawn(p.ptr(), 1);
                    (*terraria::main::s()).my_player = 0;
                    let it = *pl.inventory.at(0);
                    terraria::item::set_defaults(it.ptr(), s.type_, Gc::NULL);
                    (*it.ptr()).stack = (*it.ptr()).max_stack;
                    for (k, a) in ammo.iter().enumerate() {
                        let am = *pl.inventory.at(10 + k as i32);
                        terraria::item::set_defaults(am.ptr(), *a, Gc::NULL);
                        (*am.ptr()).stack = (*am.ptr()).max_stack;
                    }
                    pl.release_use_item = true;
                    // the target: screen position from the dump (vx, vy of the slot record)
                    (*terraria::main::s()).screen_position = Vector2 { x: s.vx, y: s.vy };
                    return;
                }
                for s in &slots {
                    let p = *ms.player.at(s.pl);
                    let pl = &mut *p.ptr();
                    pl.active = true;
                    pl.name = Str::lit("Ref");
                    pl.base.who_am_i = s.pl;
                    pl.stat_life_max = 100;
                    pl.stat_life_max2 = 100;
                    pl.stat_life = 100;
                    pl.stat_mana_max = 20;
                    pl.stat_mana_max2 = 20;
                    pl.stat_mana = 20;
                    pl.spawn_x = s.px;
                    pl.spawn_y = s.py;
                    // PlayerSpawnContext.SpawningIntoWorld = 1
                    terraria::player::spawn(p.ptr(), 1);
                }
                // 6: entities
                for (k, s) in slots.iter().enumerate() {
                    let got = if kind == "item" {
                        // inventory slot 0 of the slot's player (the selected slot of a new Player)
                        let p = *ms.player.at(s.pl);
                        rt::run(|| {
                            terraria::item::set_defaults(((*(*p.ptr()).inventory.at(0)).ptr()), s.type_, Gc::NULL);
                            0
                        })
                    } else if kind == "npc" {
                        rt::run(|| terraria::npc::new_npc(terraria::data_structures::entity_source_spawn_npc::new().up::<rt::Object>(), s.ex * 16 + 8, s.ey * 16, s.type_, 0, 0.0, 0.0, 0.0, 0.0, 255))
                    } else {
                        rt::run(|| {
                            terraria::projectile::new_projectile__i_entity_source_vector2_vector2_int_int_float_int_float_float_float_new_projectile_modifier(
                                terraria::data_structures::entity_source_debug_command::new().up::<rt::Object>(),
                                Vector2 { x: (s.ex * 16 + 8) as f32, y: ((s.ey - 3) * 16) as f32 },
                                Vector2 { x: s.vx, y: s.vy },
                                s.type_,
                                20,
                                2.0,
                                s.owner,
                                0.0,
                                0.0,
                                0.0,
                                rt::zero::<rt::Delegate<dyn Fn(Gc<terraria::projectile::Projectile>)>>(),
                            )
                        })
                    };
                    let got = got.unwrap_or(-2);
                    if got != s.index {
                        spawn_diffs.push(k);
                        diffs.push(Diff { tick: 0, what: format!("spawn of type {}: slot {got} here, {} in the real server", s.type_, s.index), slot: Some(k) });
                    }
                }
            });
            if let Err(e) = r {
                diffs.push(Diff { tick: 0, what: format!("reset / spawn: {e}"), slot: None });
            }
            // nearest slot of an x position (pixels)
            let slot_of = |x: f64| -> Option<usize> {
                if slots.len() == 1 {
                    return Some(0);
                }
                let tx = x / 16.0;
                slots.iter().enumerate().min_by(|a, b| {
                    let da = (tx - (a.1.ex + a.1.px) as f64 / 2.0).abs();
                    let db = (tx - (b.1.ex + b.1.px) as f64 / 2.0).abs();
                    da.partial_cmp(&db).unwrap()
                }).map(|(k, _)| k)
            };
            // the ticks
            let mut state: Vec<HashMap<u16, Vec<u64>>> = vec![HashMap::new(); 4];
            let mut port_err: Option<String> = None;
            let save_p = rd.p;
            for &(t, err, at) in &recs {
                if t > 0 && port_err.is_none() {
                    if kind == "item" {
                        for s in &slots {
                            let p = &mut *(*(*terraria::main::s()).player.at(s.pl)).ptr();
                            p.control_use_item = true;
                            p.base.direction = if perturb { -1 } else { 1 };
                        }
                    }
                    if citem {
                        // the owning client holds the left button on the target (ClientItems.cs, per tick)
                        let ms = &mut *terraria::main::s();
                        let cur = (*(*terraria::game_input::player_input::s()).triggers.ptr()).current;
                        terraria::game_input::triggers_set::set_mouse_left(cur.ptr(), true);
                        ms.mouse_left = true;
                        ms.mouse_left_release = t == 1;
                        ms.mouse_x = mouse_x;
                        ms.mouse_y = mouse_y;
                    }
                    // ENTITIES_TRACE_STEPS=1: the traced entity's leaves (ENTITIES_TRACE_LEAVES, or all) after each part
                    let mut step_fn = |part: &str| {
                        if let (Some((tb, tc, ti)), true) = (trace, trace_steps) {
                            if tb == b {
                                if let Some(o) = entity_obj(tc, ti) {
                                    let mut v = Vec::new();
                                    schemas[tc].read(o, &mut v);
                                    for (k, (name, kind)) in schemas[tc].leaves.iter().enumerate() {
                                        if !trace_leaves.is_empty() && !trace_leaves.iter().any(|x| x == name) {
                                            continue;
                                        }
                                        if let Some(g) = v[k] {
                                            eprintln!("STEP t{t} after {part}: {}[{ti}] {name} = {g:016x} ({:?})", CLASSES[tc], if *kind == 1 { f32::from_bits(g as u32) as f64 } else if *kind == 2 { f64::from_bits(g) } else { g as i64 as f64 });
                                        }
                                    }
                                }
                            }
                        }
                    };
                    if let Err(e) = guarded(|| entity_tick(main, &mut step_fn)) {
                        port_err = Some(format!("tick {t}: {e}"));
                        diffs.push(Diff { tick: t, what: format!("the tick here: {e}"), slot: None });
                    }
                }
                if err {
                    if port_err.is_none() {
                        diffs.push(Diff { tick: t, what: "the real tick threw (summary.json), not here".to_owned(), slot: None });
                    } else if citem {
                        // both threw at this tick (Main.ignoreErrors = false): a match; the batch ends here as there
                        diffs.retain(|d| !(d.tick == t && d.what.starts_with("the tick here:")));
                        both_threw.push(format!("batch {b} type {}: tick {t}: {}", slots[0].type_, port_err.as_deref().unwrap_or("")));
                    }
                    break;
                }
                if port_err.is_some() {
                    break;
                }
                rd.p = at;
                // RNG streams
                let ms = &*terraria::main::s();
                let (wi, wh) = (rd.u32(), rd.u64());
                let (gi, gh) = rand_state(ms.rand);
                if (wi, wh) != (gi, gh) {
                    diffs.push(Diff { tick: t, what: format!("Main.rand: real inext {wi} seed {wh:016x}, here {gi} {gh:016x}"), slot: None });
                }
                let n = rd.u16();
                let mut real_keys = Vec::new();
                for _ in 0..n {
                    let k = rd.str();
                    let (wi, wh) = (rd.u32(), rd.u64());
                    let key = Str::new(&k);
                    if ms._rngs.is_null() || !(*ms._rngs.ptr()).contains_key(key) {
                        diffs.push(Diff { tick: t, what: format!("rand stream {k}: not here"), slot: None });
                    } else {
                        let (gi, gh) = rand_state((*ms._rngs.ptr()).get_item(key));
                        if (wi, wh) != (gi, gh) {
                            diffs.push(Diff { tick: t, what: format!("rand stream {k}: real inext {wi}, here {gi}"), slot: None });
                        }
                    }
                    real_keys.push(k);
                }
                if !ms._rngs.is_null() {
                    for k in rt::Dictionary::keys(ms._rngs).to_vec() {
                        if !real_keys.iter().any(|x| x == k.as_str()) {
                            diffs.push(Diff { tick: t, what: format!("rand stream {}: here, not in the real server", k.as_str()), slot: None });
                        }
                    }
                }
                // entities
                for c in 0..4 {
                    let n = rd.u16();
                    let mut real_active = Vec::with_capacity(n as usize);
                    for _ in 0..n {
                        let idx = rd.u16();
                        let nch = rd.u16();
                        let st = state[c].entry(idx).or_insert_with(|| vec![0u64; schemas[c].leaves.len()]);
                        for _ in 0..nch {
                            let leaf = rd.u16() as usize;
                            st[leaf] = rd.u64();
                        }
                        real_active.push(idx);
                    }
                    for i in 0..COUNTS[c] {
                        let here = entity_obj(c, i);
                        let real = real_active.contains(&(i as u16));
                        match (here, real) {
                            (None, false) => {}
                            (Some(_), false) => diffs.push(Diff { tick: t, what: format!("{}[{i}]: active here, not in the real server", CLASSES[c]), slot: None }),
                            (None, true) => {
                                let st = &state[c][&(i as u16)];
                                let slot = schemas[c].pos_x.and_then(|p| slot_of(f32::from_bits(st[p] as u32) as f64));
                                diffs.push(Diff { tick: t, what: format!("{}[{i}]: active in the real server, not here", CLASSES[c]), slot });
                            }
                            (Some(o), true) => {
                                let st = &state[c][&(i as u16)];
                                schemas[c].read(o, &mut cur[0]);
                                let mut first: Option<String> = None;
                                let mut nd = 0;
                                let traced = trace.as_ref().map(|(tb, tc, ti)| *tb == b && *tc == c && *ti == i).unwrap_or(false);
                                let fmt = |name: &str, kind: u8, w: u64, g: u64| match kind {
                                    1 => format!("{name}: real {:?} ({:08x}), here {:?} ({:08x})", f32::from_bits(w as u32), w, f32::from_bits(g as u32), g),
                                    2 => format!("{name}: real {:?} ({w:016x}), here {:?} ({g:016x})", f64::from_bits(w), f64::from_bits(g)),
                                    3 => format!("{name}: hash real {w:016x}, here {g:016x}"),
                                    _ => format!("{name}: real {}, here {}", w as i64, g as i64),
                                };
                                for (k, (name, kind)) in schemas[c].leaves.iter().enumerate() {
                                    let Some(g) = cur[0][k] else { continue };
                                    if g != st[k] {
                                        nd += 1;
                                        if traced {
                                            eprintln!("TRACE t{t} {}[{i}] DIFF {}", CLASSES[c], fmt(name, *kind, st[k], g));
                                        }
                                        if first.is_none() {
                                            first = Some(fmt(name, *kind, st[k], g));
                                        }
                                    } else if traced && trace_leaves.iter().any(|x| x == name) {
                                        eprintln!("TRACE t{t} {}[{i}] same {name} = {:016x} ({:?})", CLASSES[c], g, if *kind == 1 { f32::from_bits(g as u32) as f64 } else if *kind == 2 { f64::from_bits(g) } else { g as i64 as f64 });
                                    }
                                }
                                if let Some(f) = first {
                                    let slot = schemas[c].pos_x.and_then(|p| slot_of(f32::from_bits(st[p] as u32) as f64));
                                    let ty = if c < 2 { schemas[c].leaves.iter().position(|(n, _)| n == "type").map(|p| format!(" type {}", st[p] as i64)).unwrap_or_default() } else { String::new() };
                                    diffs.push(Diff { tick: t, what: format!("{}[{i}]{ty} ({nd} leaves) {f}", CLASSES[c]), slot });
                                }
                            }
                        }
                    }
                }
                if citem {
                    let (wc, wh) = (rd.i32(), rd.u64());
                    let s0 = &slots[0];
                    let (gc, gh) = region_tiles(&loaded, s0.px + tbox[0], s0.px + tbox[1], s0.py + tbox[2], s0.py + tbox[3], false);
                    if (wc, wh) != (gc, gh) {
                        diffs.push(Diff { tick: t, what: format!("tiles around the player: {gc} differ from the loaded world here (hash {gh:016x}), real {wc} ({wh:016x})"), slot: Some(0) });
                    }
                }
            }
            rd.p = save_p;
            // ---- the batch's verdict
            let types: Vec<String> = slots.iter().map(|s| s.type_.to_string()).collect();
            let first_tick = diffs.iter().map(|d| d.tick).min();
            match first_tick {
                None => {
                    batches_exact += 1;
                    for s in &slots {
                        status.insert(s.type_, "exact".to_owned());
                    }
                    batch_lines.push(format!("batch {b:3} [{}]: exact ({:.1} s)", types.join(","), tb.elapsed().as_secs_f64()));
                }
                Some(ft) => {
                    let firsts: Vec<&Diff> = diffs.iter().filter(|d| d.tick == ft).collect();
                    let roots: Vec<usize> = {
                        let mut v: Vec<usize> = firsts.iter().filter_map(|d| d.slot).collect();
                        v.sort();
                        v.dedup();
                        v
                    };
                    let mut per_slot_first: HashMap<usize, &Diff> = HashMap::new();
                    for d in &diffs {
                        if let Some(s) = d.slot {
                            per_slot_first.entry(s).or_insert(d);
                        }
                    }
                    for (k, s) in slots.iter().enumerate() {
                        let st = if spawn_diffs.contains(&k) {
                            "spawn".to_owned()
                        } else if roots.contains(&k) {
                            format!("root t{ft}: {}", per_slot_first[&k].what)
                        } else if let Some(d) = per_slot_first.get(&k) {
                            format!("after t{}: {}", d.tick, d.what)
                        } else if roots.is_empty() {
                            format!("after t{ft} (batch-wide: {})", firsts[0].what)
                        } else {
                            "exact".to_owned()
                        };
                        status.insert(s.type_, st);
                    }
                    batch_lines.push(format!(
                        "batch {b:3} [{}]: first difference at tick {ft} ({} at that tick, {} in all), slots {:?} = types {:?}; {}",
                        types.join(","),
                        firsts.len(),
                        diffs.len(),
                        roots,
                        roots.iter().map(|k| slots[*k].type_).collect::<Vec<_>>(),
                        firsts.iter().take(3).map(|d| d.what.clone()).collect::<Vec<_>>().join(" | ")
                    ));
                }
            }
            eprintln!("{}", batch_lines.last().unwrap());
        }
        // the end marker: what the last batch left in the tiles
        let end = rd.i32();
        assert_eq!(end, -1);
        let (want_tc, want_th) = (rd.i32(), rd.u64());
        if only.is_none() && limit >= nbatches {
            let (tc, th) = tiles_back(&loaded);
            if (tc, th) != (want_tc, want_th) {
                tile_problems.push(format!("after the last batch: {tc} tiles changed (hash {th:016x}), real {want_tc} ({want_th:016x})"));
            }
        }

        // ---- report
        let n_exact = status.values().filter(|s| *s == "exact").count();
        let n_root = status.values().filter(|s| s.starts_with("root")).count();
        let n_after = status.values().filter(|s| s.starts_with("after")).count();
        let n_spawn = status.values().filter(|s| *s == "spawn").count();
        let n_run = status.values().filter(|s| *s != "not run").count();
        let out = root.join("port").join("target-gen").join(format!("entities-{kind}.tsv"));
        let mut tsv = String::from("type\tbatch\tstatus\n");
        for (ty, b) in &all_types {
            tsv.push_str(&format!("{ty}\t{b}\t{}\n", status.get(ty).map(|s| s.as_str()).unwrap_or("?")));
        }
        let _ = std::fs::write(&out, tsv);
        eprintln!("==== {kind}: {batches_run} of {nbatches} batches run in {:.1} s, {batches_exact} exact", t_all.elapsed().as_secs_f64());
        eprintln!("==== {kind}: types run {n_run}: exact {n_exact}, root {n_root}, after {n_after}, spawn {n_spawn}; table {}", out.display());
        for p in &tile_problems {
            eprintln!("==== tiles: {p}");
        }
        if !both_threw.is_empty() {
            eprintln!("==== {} batches end where the real run's tick threw too (the same tick):", both_threw.len());
            for x in &both_threw {
                eprintln!("====   {x}");
            }
        }
        for s in &schemas {
            if !s.unreadable.is_empty() {
                eprintln!("==== leaves not compared ({}): {}", s.unreadable.len(), s.unreadable.join(", "));
            }
        }
        let mut fail = Vec::new();
        if std::env::var("ENTITIES_REQUIRE").ok().as_deref() == Some("all") && (n_exact != n_run || !tile_problems.is_empty()) {
            fail.push(format!("{} of {n_run} types are not exact", n_run - n_exact));
        }
        if let Ok(f) = std::env::var("ENTITIES_ALLOW") {
            let text = std::fs::read_to_string(&f).unwrap_or_else(|_| panic!("ENTITIES_ALLOW file {f}"));
            let allowed: Vec<i32> = text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).filter_map(|l| {
                let mut it = l.split_whitespace();
                (it.next() == Some(kind.as_str())).then(|| it.next().unwrap().parse().unwrap())
            }).collect();
            for (ty, st) in &status {
                if st == "not run" {
                    continue;
                }
                if st != "exact" && !allowed.contains(ty) {
                    fail.push(format!("type {ty} differs and is not in {f}: {st}"));
                }
                if st == "exact" && allowed.contains(ty) {
                    fail.push(format!("type {ty} is exact but listed in {f}"));
                }
            }
            if !tile_problems.is_empty() {
                fail.push("tile changes differ".to_owned());
            }
        }
        for f in &fail {
            eprintln!("FAIL {f}");
        }
        assert!(fail.is_empty(), "{} failure(s)", fail.len());
    };
    terraria_host::run_with_stack(512 << 20, "the entities test", run);
}
