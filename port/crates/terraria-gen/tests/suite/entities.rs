//! `SetDefaults` of every item, NPC and projectile against `refdata/{items,npcs,projectiles}.json`.
//!
//! `tools/refdump` did, for every id: `new T(); t.SetDefaults(id)` (optional parameters at their defaults) and dumped
//! every public instance field and every simple public property by reflection. This does the same with the
//! translated classes (`reflect_fields` / `reflect_prop` are generated from the same member lists) and compares
//! every dumped value. Nothing is skipped silently: a member the translation cannot produce is a mismatch, a
//! member whose type has no reflection in the generator is listed as "not compared" with its type.

use crate::common::Json;
use crate::game::{refdata, run, Report};
use std::collections::BTreeMap;
use terraria_gen::gen::terraria;
use terraria_gen::rt::{self, Gc, Value};

fn num_eq_f32(text: &str, v: f32) -> bool {
    matches!(text.parse::<f32>(), Ok(e) if e == v)
}

/// `None` when equal, otherwise a description of the difference. RefDump prints floats with "R" formatting, so a
/// float is equal when the dumped text parses to the same `f32` / `f64`.
fn diff(expected: &Json, got: &Value) -> Option<String> {
    let bad = || Some(format!("expected {} got {:?}", cut(&expected.text()), got));
    let num = |k: &str| expected.get(k);
    let f = |k: &str, v: f32| matches!(num(k), Some(Json::Num(t)) if num_eq_f32(t, v));
    let i = |k: &str, v: i64| matches!(num(k), Some(j @ Json::Num(_)) if j.int() == v);
    match (expected, got) {
        (Json::Null, Value::Null) => None,
        (Json::Bool(a), Value::Bool(b)) if a == b => None,
        (Json::Num(a), Value::Int(b)) if a.parse::<i128>().ok() == Some(*b) => None,
        (Json::Num(a), Value::F32(b)) if num_eq_f32(a, *b) => None,
        (Json::Num(a), Value::F64(b)) if a.parse::<f64>().ok() == Some(*b) => None,
        // RefDump writes NaN / Infinity as strings
        (Json::Str(a), Value::F32(b)) if (a == "NaN" && b.is_nan()) || a.parse::<f32>().ok() == Some(*b) => None,
        (Json::Str(a), Value::Str(b)) if a == b => None,
        (Json::Str(a), Value::Char(c)) if a.encode_utf16().collect::<Vec<_>>() == [*c] => None,
        (Json::Obj(_), Value::Color { r, g, b, a }) if i("r", *r as i64) && i("g", *g as i64) && i("b", *b as i64) && i("a", *a as i64) => None,
        (Json::Obj(_), Value::Vector2 { x, y }) if f("X", *x) && f("Y", *y) => None,
        (Json::Obj(_), Value::Point { x, y }) if i("X", *x as i64) && i("Y", *y as i64) => None,
        (Json::Obj(_), Value::Rectangle { x, y, width, height }) if i("X", *x as i64) && i("Y", *y as i64) && i("Width", *width as i64) && i("Height", *height as i64) => None,
        (Json::Arr(a), Value::Arr(b)) => {
            if a.len() != b.len() {
                return Some(format!("array length {} expected {}", b.len(), a.len()));
            }
            for (k, (x, y)) in a.iter().zip(b).enumerate() {
                if let Some(d) = diff(x, y) {
                    return Some(format!("[{k}]: {d}"));
                }
            }
            None
        }
        (Json::Obj(m), Value::Obj(fields)) => {
            // nested object: RefDump writes its public fields
            if m.len() != fields.len() {
                return bad();
            }
            for (k, v) in m {
                match fields.iter().find(|(n, _)| n == k) {
                    Some((_, f)) => {
                        if let Some(d) = diff(v, f) {
                            return Some(format!("{k}: {d}"));
                        }
                    }
                    None => return Some(format!("{k}: missing")),
                }
            }
            None
        }
        _ => bad(),
    }
}

fn cut(s: &str) -> String {
    if s.len() > 120 {
        format!("{}...", &s[..s.char_indices().take_while(|(i, _)| *i < 120).last().map(|(i, c)| i + c.len_utf8()).unwrap_or(0)])
    } else {
        s.to_owned()
    }
}

/// A reflected object: its public fields, the names of its simple properties and a getter for property `i`.
struct Reflected {
    fields: Vec<(&'static str, Value)>,
    props: &'static [&'static str],
    prop: Box<dyn Fn(usize) -> Value>,
}

struct Outcome {
    /// (member, records): values that `SetDefaults` draws from `Main.rand` (found by running it under two seeds).
    rng_dependent: BTreeMap<String, u64>,
    records: usize,
    values: u64,
    exact: u64,
    mismatches: BTreeMap<String, Vec<(i64, String)>>,
    not_compared: BTreeMap<String, u64>,
    members: BTreeMap<String, u64>,
    control: usize,
}

fn skip_reason(v: &Value) -> Option<String> {
    match v {
        Value::Skip(t) => Some((*t).to_owned()),
        Value::Arr(a) => a.iter().find_map(skip_reason),
        Value::Obj(f) => f.iter().find_map(|(_, v)| skip_reason(v)),
        _ => None,
    }
}

fn compare(records: &[Json], excluded: &[(&str, &str)], make: &dyn Fn(i32) -> Reflected, trivially_different: &[&str]) -> Outcome {
    let mut o = Outcome { rng_dependent: BTreeMap::new(), records: records.len(), values: 0, exact: 0, mismatches: BTreeMap::new(), not_compared: BTreeMap::new(), members: BTreeMap::new(), control: 0 };
    for rec in records {
        let id = rec.get("id").expect("id").int();
        // The dump was taken with whatever state `Main.rand` had after the harness' initialisation and the earlier
        // dumps; that state cannot be reproduced without every draw of the untranslated initialisers. A member
        // whose value changes with the seed is therefore reported and not compared: found by running under two seeds.
        let all_members = |seed: i32| -> Result<Vec<(String, Value)>, String> {
            run(|| {
                unsafe { (*terraria::main::s()).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(seed)) };
                let obj = make(id as i32);
                let mut v: Vec<(String, Value)> = obj.fields.iter().map(|(n, x)| (n.to_string(), x.clone())).collect();
                for (i, n) in obj.props.iter().enumerate() {
                    if let Ok(x) = run(|| (obj.prop)(i)) {
                        v.push((n.to_string(), x));
                    }
                }
                v
            })
        };
        let rng: Vec<String> = match (all_members(1), all_members(2)) {
            (Ok(a), Ok(b)) => a.iter().zip(&b).filter(|(x, y)| x.1 != y.1).map(|(x, _)| x.0.clone()).collect(),
            _ => Vec::new(),
        };
        match run(|| make(id as i32)) {
            Err(e) => o.mismatches.entry("<SetDefaults failed>".to_owned()).or_default().push((id, e)),
            Ok(obj) => {
                for (key, expected) in rec.obj() {
                    if key == "id" || excluded.iter().any(|(n, _)| n == key) {
                        continue;
                    }
                    if rng.contains(key) {
                        *o.rng_dependent.entry(key.clone()).or_insert(0) += 1;
                        continue;
                    }
                    let got: Result<Value, String> = if let Some((_, v)) = obj.fields.iter().find(|(n, _)| n == key) {
                        Ok(v.clone())
                    } else if let Some(i) = obj.props.iter().position(|n| n == key) {
                        run(|| (obj.prop)(i)).map_err(|e| format!("getter failed: {e}"))
                    } else {
                        Err("member missing from the translated class".to_owned())
                    };
                    if let Ok(v) = &got {
                        if let Some(t) = skip_reason(v) {
                            *o.not_compared.entry(format!("{key}: {t}")).or_insert(0) += 1;
                            continue;
                        }
                    }
                    *o.members.entry(key.clone()).or_insert(0) += 1;
                    o.values += 1;
                    let d = match &got {
                        Ok(v) => diff(expected, v),
                        Err(e) => Some(e.clone()),
                    };
                    match d {
                        None => o.exact += 1,
                        Some(d) => o.mismatches.entry(key.clone()).or_default().push((id, d)),
                    }
                }
                // members the translation has that the dump does not: also a mismatch
                for n in obj.fields.iter().map(|(n, _)| *n).chain(obj.props.iter().copied()) {
                    if rec.get(n).is_none() {
                        o.mismatches.entry(format!("{n} (not in the dump)")).or_default().push((id, "extra member".to_owned()));
                    }
                }
            }
        }
    }
    // Negative control: each object against the NEXT id's record, ignoring the members that trivially differ.
    // A comparison that could not see differences would report 0 here.
    for pair in records.windows(2) {
        let id = pair[0].get("id").expect("id").int();
        let differs = run(|| {
            let obj = make(id as i32);
            pair[1].obj().iter().any(|(k, v)| k != "id" && !trivially_different.contains(&k.as_str()) && obj.fields.iter().find(|(n, _)| n == k).map(|(_, f)| diff(v, f).is_some()).unwrap_or(false))
        });
        if matches!(differs, Ok(true)) {
            o.control += 1;
        }
    }
    o
}

fn print(report: &mut Report, what: &str, o: &Outcome, excluded: &[(&str, &str)], min_control: usize) {
    report.line(format!(
        "{what}: {} records, {} members, {} values compared: {} exact, {} mismatches",
        o.records,
        o.members.len(),
        o.values,
        o.exact,
        o.values - o.exact
    ));
    for (n, why) in excluded {
        report.line(format!("  excluded: {n}: {why}"));
    }
    if !o.rng_dependent.is_empty() {
        let total: u64 = o.rng_dependent.values().sum();
        report.line(format!(
            "  not compared ({total} values): members SetDefaults draws from Main.rand (value differs between two seeds; the dump's generator state is not reproducible): {}",
            o.rng_dependent.iter().map(|(k, n)| format!("{k} x{n}")).collect::<Vec<_>>().join(", ")
        ));
    }
    for (k, n) in &o.not_compared {
        report.line(format!("  not compared ({n} values): {k} has no reflection in the generator"));
    }
    report.line(format!("  negative control: {} of {} objects differ from the next id's record", o.control, o.records.saturating_sub(1)));
    if o.control < min_control {
        report.fail(format!("{what}: negative control found only {} differing neighbours: the comparison does not discriminate", o.control));
    }
    if !o.mismatches.is_empty() {
        let total: usize = o.mismatches.values().map(|v| v.len()).sum();
        let mut by_size: Vec<_> = o.mismatches.iter().collect();
        by_size.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
        for (field, list) in by_size.iter().take(60) {
            report.line(format!("  MISMATCH {field}: {} records", list.len()));
            for (id, d) in list.iter().take(3) {
                report.line(format!("      id {id}: {}", cut(d)));
            }
        }
        report.fail(format!("{what}: {total} mismatching values in {} members", o.mismatches.len()));
    }
}

const ITEM_EXCLUDED: &[(&str, &str)] = &[(
    "ToolTip",
    "ItemTooltip is built from Lang.GetTooltip (Lang is outside the translated set; the config stubs it with null)",
)];

fn make_item(id: i32) -> Reflected {
    unsafe {
        // exactly what RefDump does: new Item(); item.SetDefaults(id) with the optional variant = null
        let item = terraria::item::new__void();
        terraria::item::set_defaults(item.ptr(), id, Gc::NULL);
        let p = item.ptr();
        Reflected { fields: terraria::item::reflect_fields(p), props: terraria::item::REFLECT_PROPS, prop: Box::new(move |i| terraria::item::reflect_prop(p, i)) }
    }
}

pub fn items(report: &mut Report) {
    let dump = refdata("items.json");
    let items = dump.get("items").expect("items").arr();
    if items.len() != 6195 {
        report.fail(format!("refdata/items.json holds {} items, expected 6195", items.len()));
    }
    let o = compare(items, ITEM_EXCLUDED, &make_item, &["type", "Name", "HoverName", "ToolTip", "netID"]);
    print(report, "Item.SetDefaults", &o, ITEM_EXCLUDED, 3000);
}

const NPC_EXCLUDED: &[(&str, &str)] = &[];

fn make_npc(id: i32) -> Reflected {
    unsafe {
        // RefDump: new NPC(); npc.SetDefaults(id, default(NPCSpawnParams))
        let npc = terraria::npc::new();
        terraria::npc::set_defaults(npc.ptr(), id, rt::zero());
        let p = npc.ptr();
        Reflected { fields: terraria::npc::reflect_fields(p), props: terraria::npc::REFLECT_PROPS, prop: Box::new(move |i| terraria::npc::reflect_prop(p, i)) }
    }
}

pub fn npcs(report: &mut Report) {
    let dump = refdata("npcs.json");
    let list = dump.get("npcs").expect("npcs").arr();
    let first = list.first().map(|r| r.get("id").unwrap().int());
    let last = list.last().map(|r| r.get("id").unwrap().int());
    if (first, last, list.len()) != (Some(-65), Some(696), 762) {
        report.fail(format!("refdata/npcs.json: {} records {first:?}..{last:?}, expected 762 records -65..696", list.len()));
    }
    let o = compare(list, NPC_EXCLUDED, &make_npc, &["type", "netID", "TypeName", "FullName", "GivenOrTypeName"]);
    print(report, "NPC.SetDefaults", &o, NPC_EXCLUDED, 400);
}

const PROJECTILE_EXCLUDED: &[(&str, &str)] = &[(
    "Name",
    "Projectile.Name is Lang.GetProjectileName(type).Value (Lang is outside the translated set)",
)];

fn make_projectile(id: i32) -> Reflected {
    unsafe {
        // RefDump: new Projectile(); projectile.SetDefaults(id)
        let proj = terraria::projectile::new();
        terraria::projectile::set_defaults(proj.ptr(), id);
        let p = proj.ptr();
        Reflected {
            fields: terraria::projectile::reflect_fields(p),
            props: terraria::projectile::REFLECT_PROPS,
            prop: Box::new(move |i| terraria::projectile::reflect_prop(p, i)),
        }
    }
}

pub fn projectiles(report: &mut Report) {
    let dump = refdata("projectiles.json");
    let list = dump.get("projectiles").expect("projectiles").arr();
    if list.len() != 1135 {
        report.fail(format!("refdata/projectiles.json holds {} records, expected 1135", list.len()));
    }
    let o = compare(list, PROJECTILE_EXCLUDED, &make_projectile, &["type", "Name"]);
    print(report, "Projectile.SetDefaults", &o, PROJECTILE_EXCLUDED, 600);
}

/// The collector on a real heap: everything the sections above allocated and dropped is garbage, everything the
/// game's statics hold must survive. Poison mode is on, so a wrongly freed object would be read as `0xDD..`
/// (and, with the `gc-check` feature, panic on first use): the item comparison is run again afterwards.
pub fn collect_and_recheck(report: &mut Report) {
    rt::set_poison(true);
    let before = rt::heap_size();
    // SAFETY: no translated frame is on the stack here, and the test holds no handle outside the game's statics
    // (the host tables are rooted or static).
    let stats = unsafe { rt::collect() };
    report.line(format!(
        "collect: {} objects / {} MB before; freed {} objects / {} MB; live {} objects / {} MB",
        before.0,
        before.1 >> 20,
        stats.freed_objects,
        stats.freed_bytes >> 20,
        stats.live_objects,
        stats.live_bytes >> 20
    ));
    if stats.freed_objects == 0 {
        report.fail("collect freed nothing: the earlier sections must have left garbage".to_owned());
    }
    let again = unsafe { rt::collect() };
    report.line(format!("second collect: freed {} objects (expected 0)", again.freed_objects));
    if again.freed_objects != 0 {
        report.fail("a second collection right after the first freed objects".to_owned());
    }
    let dump = refdata("items.json");
    let items = dump.get("items").expect("items").arr();
    let o = compare(items, ITEM_EXCLUDED, &make_item, &["type", "Name", "HoverName", "ToolTip", "netID"]);
    print(report, "Item.SetDefaults after the collection", &o, ITEM_EXCLUDED, 3000);
    let third = unsafe { rt::collect() };
    report.line(format!("third collect (garbage of the re-check): freed {} objects, live {}", third.freed_objects, third.live_objects));
    if third.live_objects != again.live_objects {
        report.line(format!("  note: live objects changed by {} across the re-check (lazily created statics)", third.live_objects as i64 - again.live_objects as i64));
    }
}
