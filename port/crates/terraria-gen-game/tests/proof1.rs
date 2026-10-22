//! Proof 1: the translated `Main.Initialize()` as ONE call in dedicated-server mode, in the sequence the reference
//! harness used (`tools/refdump/RefDump.cs`, `refdata/MANIFEST.json` "initSequence"), then EVERY dump of `refdata`
//! compared, in the order the harness wrote them (the order matters: `SetDefaults` draws from `Main.rand`).
//!
//! The comparison reads the translated objects the way the harness read the real ones: by reflection
//! (`tests/p1/dump.rs` is `RefDump.S` / `Members` over the generated reflection data).
//!
//! The test fails when a step throws or stops, when anything unimplemented is reached (a `void` library member
//! that is not declared inert in `tools/cs2rs/game.json`, a gap), when the reflection port could not read
//! something, or when a compared value differs. The table is printed either way and written to
//! `<target>/tmp/cs2rs-proof1.txt`.

#[path = "p1/dump.rs"]
mod dump;
#[path = "p1/json.rs"]
mod json;

use dump::{compare, ord, Dumper, Got, Stats};
use json::Json;
use std::fmt::Write as _;
use terraria_gen_game as game;
use terraria_gen_game::gen::{enums, meta, terraria};
use terraria_gen_game::rt::{self, Obj, Str, TypeMeta};

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_owned()
    } else {
        "<non-string panic payload>".to_owned()
    }
}

struct Report {
    text: String,
    failures: Vec<String>,
}
impl Report {
    fn line(&mut self, s: impl AsRef<str>) {
        eprintln!("{}", s.as_ref());
        self.text.push_str(s.as_ref());
        self.text.push('\n');
    }
    fn fail(&mut self, s: String) {
        self.line(format!("FAIL {s}"));
        self.failures.push(s);
    }
}

/// One step of the sequence: a C# exception or a missing piece ends the proof with the step named.
fn step(r: &mut Report, name: &str, f: impl FnOnce()) -> bool {
    let t0 = std::time::Instant::now();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt::run(f))) {
        Ok(Ok(())) => {
            r.line(format!("ok   {name}   ({:.2} s)", t0.elapsed().as_secs_f64()));
            true
        }
        Ok(Err(e)) => {
            r.fail(format!("step `{name}` threw a C# exception: {e}"));
            false
        }
        Err(p) => {
            r.fail(format!("step `{name}` stopped: {}", panic_text(&p)));
            false
        }
    }
}

fn refdata(name: &str) -> Json {
    let path = terraria_host::sys::io::project_dir().join("refdata").join(name);
    json::parse(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
}

fn meta_by_name(clr_name: &str) -> &'static TypeMeta {
    meta::ALL.iter().copied().find(|m| m.clr_name == clr_name).unwrap_or_else(|| panic!("no reflection data for {clr_name}"))
}

fn static_field(m: &'static TypeMeta, name: &str) -> Obj {
    let f = m.fields.iter().find(|f| f.is_static && f.name == name).unwrap_or_else(|| panic!("no static field {}.{name}", m.clr_name));
    unsafe { (f.get)(rt::Gc::NULL) }
}

/// One row of the result table.
struct Row {
    file: &'static str,
    st: Stats,
    notes: Vec<String>,
}

fn row(r: &mut Report, rows: &mut Vec<Row>, file: &'static str, st: Stats, notes: Vec<String>) {
    r.line(format!("     {file}: {} values compared, {} exact, {} not exact", st.compared, st.exact, st.not_exact()));
    for e in &st.examples {
        r.line(format!("         {e}"));
    }
    rows.push(Row { file, st, notes });
}

const SIMPLE_ELEMENTS: &[&str] = &["System.Boolean", "System.Byte", "System.SByte", "System.Int16", "System.UInt16", "System.Int32", "System.UInt32", "System.Int64", "System.UInt64", "System.Single", "System.Double", "System.Char", "System.String", "System.Decimal"];

/// `ArraysOfLength(main, len)` (RefDump.cs:513): public static one-dimensional arrays of `Terraria.Main` with
/// simple elements and that length, by name.
unsafe fn arrays_of_length(d: &mut Dumper, len: usize) -> Got {
    let main = meta_by_name("Terraria.Main");
    let mut parts = Vec::new();
    for f in Dumper::static_public_fields(main) {
        let tn = rt::type_info((f.field_type)()).0.as_str().to_owned();
        let Some(elem) = tn.strip_suffix("[]") else { continue };
        if elem.ends_with(']') {
            continue;
        }
        let a = (f.get)(rt::Gc::NULL);
        if a.is_null() {
            continue;
        }
        let rt::Reflect::Seq { items, .. } = rt::obj_reflect(a) else { continue };
        if items.len() != len {
            continue;
        }
        let simple = SIMPLE_ELEMENTS.contains(&elem) || items.first().map(|e| !e.is_null() && rt::enum_info(rt::type_id_of(e.0 as *const u8)).is_some()).unwrap_or(false);
        if !simple {
            continue;
        }
        match d.s(a, 0) {
            Some(g) => parts.push((f.name.to_owned(), g)),
            None => d.problem(format!("Main.{} not serialisable", f.name)),
        }
    }
    Got::Obj(parts)
}

unsafe fn dump_tile_arrays(d: &mut Dumper) -> Stats {
    let j = refdata("tile_arrays.json");
    let mut st = Stats::default();
    let tc = (*terraria::id::tile_id::s()).count as usize;
    let wc = (*terraria::id::wall_id::s()).count as usize;
    compare(j.get("tileCount").unwrap(), Some(&Got::Int(tc as i128)), "tileCount", "tileCount", &mut st);
    compare(j.get("wallCount").unwrap(), Some(&Got::Int(wc as i128)), "wallCount", "wallCount", &mut st);
    let tiles = arrays_of_length(d, tc);
    let walls = arrays_of_length(d, wc);
    compare(j.get("tile").unwrap(), Some(&tiles), "tile", "", &mut st);
    compare(j.get("wall").unwrap(), Some(&walls), "wall", "", &mut st);
    let tm = (*terraria::main::s()).tile_merge;
    let mut pairs = Vec::new();
    for a in 0..tm.len() {
        let rowa = *tm.at(a);
        if !rowa.is_null() {
            for b in 0..rowa.len() {
                if *rowa.at(b) {
                    pairs.push(Got::Arr(vec![Got::Int(a as i128), Got::Int(b as i128)]));
                }
            }
        }
    }
    compare(j.get("tileMerge").unwrap(), Some(&Got::Arr(pairs)), "tileMerge", "tileMerge", &mut st);
    st
}

unsafe fn dump_id_sets(d: &mut Dumper, notes: &mut Vec<String>) -> Stats {
    let j = refdata("id_sets.json");
    let mut st = Stats::default();
    let mut types: Vec<&'static TypeMeta> = meta::ALL.iter().copied().filter(|m| m.clr_name.starts_with("Terraria.ID.") && !m.clr_name.contains('<') && m.clr_name.split('+').skip(1).any(|x| x == "Sets")).collect();
    types.sort_by(|a, b| ord(a.clr_name, b.clr_name));
    let mut seen = std::collections::BTreeSet::new();
    let mut factories = 0;
    for t in &types {
        let cls = t.clr_name["Terraria.ID.".len()..].replace('+', ".");
        for f in Dumper::static_public_fields(t) {
            let key = format!("{cls}.{}", f.name);
            let ft = rt::type_info((f.field_type)()).0.as_str().to_owned();
            if ft.rsplit('.').next() == Some("SetFactory") {
                factories += 1;
                continue;
            }
            d.ctx = format!("{key}: ");
            let val = (f.get)(rt::Gc::NULL);
            let got = d.s(val, 0);
            d.ctx.clear();
            match (j.get(&key), got) {
                (Some(e), g) => {
                    seen.insert(key.clone());
                    compare(e, g.as_ref(), &key, &key, &mut st)
                }
                (None, Some(_)) => {
                    st.compared += 1;
                    st.by_member.insert(key.clone(), 1);
                    st.examples.push(format!("{key}: not in the dump"));
                }
                (None, None) => {}
            }
        }
    }
    for (k, v) in j.obj() {
        if !seen.contains(k) {
            compare(v, None, k, k, &mut st);
        }
    }
    notes.push(format!("{} `Sets` types, {} entries of the dump, {factories} `SetFactory` fields skipped as in the dump (31 there)", types.len(), j.obj().len()));
    st
}

unsafe fn dump_id_constants(d: &mut Dumper, notes: &mut Vec<String>) -> Stats {
    let j = refdata("id_constants.json");
    let mut st = Stats::default();
    let mut out: Vec<(String, Got)> = Vec::new();
    for m in meta::ALL.iter().copied() {
        let Some(name) = m.clr_name.strip_prefix("Terraria.ID.") else { continue };
        if name.contains('.') || name.contains('+') || name.contains('<') {
            continue;
        }
        let mut parts: Vec<(String, Got)> = Vec::new();
        for f in m.fields.iter().filter(|f| f.is_static && f.public) {
            // RefDump.cs:600 true consts, plus static readonly simple values that are not strings
            let ft = rt::type_info((f.field_type)());
            let simple = (ft.1 & rt::type_flags::PRIMITIVE != 0 && ft.0.as_str() != "System.IntPtr") || ft.1 & rt::type_flags::ENUM != 0 || ft.0.as_str() == "System.String";
            if !f.literal && !(f.init_only && simple && ft.0.as_str() != "System.String") {
                continue;
            }
            let v = (f.get)(rt::Gc::NULL);
            if v.is_null() {
                continue;
            }
            if let Some(g) = d.s(v, 0) {
                parts.push((f.name.to_owned(), g));
            }
        }
        if !parts.is_empty() {
            parts.sort_by(|a, b| ord(&a.0, &b.0));
            out.push((name.to_owned(), Got::Obj(parts)));
        }
    }
    for e in enums::ALL.iter().copied() {
        let Some(name) = e.name.strip_prefix("Terraria.ID.") else { continue };
        if name.contains('.') || name.contains('+') {
            continue;
        }
        let bits = match e.underlying {
            rt::tid::U8 | rt::tid::I8 => 8,
            rt::tid::I16 | rt::tid::U16 => 16,
            rt::tid::I64 | rt::tid::U64 => 64,
            _ => 32,
        };
        let mut parts: Vec<(String, Got)> = e
            .names
            .iter()
            .map(|(v, n)| {
                let v = if e.signed { ((*v as i64) << (64 - bits) >> (64 - bits)) as i128 } else { *v as i128 };
                ((*n).to_owned(), Got::Int(v))
            })
            .collect();
        parts.sort_by(|a, b| ord(&a.0, &b.0));
        out.push((name.to_owned(), Got::Obj(parts)));
    }
    let total: usize = out.iter().map(|(_, g)| if let Got::Obj(p) = g { p.len() } else { 0 }).sum();
    notes.push(format!("{} ID classes / enums with constants here, {} in the dump; {total} constants here (12465 in the dump)", out.len(), j.obj().len()));
    compare(&j, Some(&Got::Obj(out)), "", "", &mut st);
    st
}

/// `RecordList` + `WithId(id, Members(o, 0, 1))` (RefDump.cs:615-650).
unsafe fn dump_entities(d: &mut Dumper, r: &mut Report, file: &str, key: &str, make: &dyn Fn(i32) -> Obj) -> Stats {
    let j = refdata(file);
    let mut st = Stats::default();
    for rec in j.get(key).unwrap().arr() {
        let id = rec.get("id").unwrap().int() as i32;
        let got = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt::run(|| make(id)))) {
            Ok(Ok(o)) => {
                d.ctx.clear();
                match d.members(o, 0, 1) {
                    Some(Got::Obj(mut p)) => {
                        p.insert(0, ("id".to_owned(), Got::Int(id as i128)));
                        Some(Got::Obj(p))
                    }
                    _ => None,
                }
            }
            Ok(Err(e)) => {
                r.fail(format!("{file}: SetDefaults({id}) threw {e}"));
                None
            }
            Err(p) => {
                r.fail(format!("{file}: SetDefaults({id}) stopped: {}", panic_text(&p)));
                None
            }
        };
        compare(rec, got.as_ref(), &format!("{key}[id {id}]"), "", &mut st);
    }
    st
}

unsafe fn dump_buffs(d: &mut Dumper) -> Stats {
    let j = refdata("buffs.json");
    let mut st = Stats::default();
    let bc = (*terraria::id::buff_id::s()).count as usize;
    compare(j.get("buffCount").unwrap(), Some(&Got::Int(bc as i128)), "buffCount", "buffCount", &mut st);
    let arrays = arrays_of_length(d, bc);
    compare(j.get("arrays").unwrap(), Some(&arrays), "arrays", "", &mut st);
    st
}

unsafe fn int_member(d: &mut Dumper, o: Obj, name: &str) -> i128 {
    let m = rt::meta_of_obj(o).expect("reflection data");
    match d.member(o, m, name).and_then(|v| d.s(v, 99)) {
        Some(Got::Int(i)) => i,
        _ => 0,
    }
}

unsafe fn dump_recipes(d: &mut Dumper) -> Stats {
    let j = refdata("recipes.json");
    let mut st = Stats::default();
    let num = (*terraria::recipe::s()).num_recipes;
    compare(j.get("numRecipes").unwrap(), Some(&Got::Int(num as i128)), "numRecipes", "numRecipes", &mut st);
    let recipe_t = meta_by_name("Terraria.Recipe");
    let mut fields: Vec<&'static rt::FieldMeta> = recipe_t.fields.iter().filter(|f| !f.is_static && f.public).collect();
    fields.sort_by(|a, b| ord(a.name, b.name));
    let rec = (*terraria::main::s()).recipe;
    let mut list = Vec::new();
    for i in 0..num {
        let ro: Obj = (*rec.at(i)).up();
        let mut parts = vec![("index".to_owned(), Got::Int(i as i128))];
        for f in &fields {
            let v = (f.get)(ro);
            // RefDump.cs:680 trailing empty slots of the two fixed 15-slot arrays are trimmed
            let trimmed = |d: &mut Dumper, a: &str, b: &str| -> Option<Got> {
                let rt::Reflect::Seq { items, .. } = rt::obj_reflect(v) else { return None };
                let mut last: i64 = -1;
                for (k, it) in items.iter().enumerate() {
                    if !it.is_null() && (int_member(d, *it, a) != 0 || int_member(d, *it, b) != 0) {
                        last = k as i64;
                    }
                }
                Some(Got::Arr((0..=last).map(|k| d.s(items[k as usize], 1).unwrap_or(Got::Null)).collect()))
            };
            let s = match f.name {
                "requiredItem" => trimmed(d, "type", "stack"),
                "requiredItemQuickLookup" => trimmed(d, "itemIdOrRecipeGroup", "stack"),
                _ => d.s(v, 1),
            };
            if let Some(s) = s {
                parts.push((f.name.to_owned(), s));
            }
        }
        list.push(Got::Obj(parts));
    }
    compare(j.get("recipes").unwrap(), Some(&Got::Arr(list)), "recipes", "", &mut st);
    let groups = d.s(static_field(meta_by_name("Terraria.RecipeGroup"), "recipeGroups"), 0);
    compare(j.get("recipeGroups").unwrap(), groups.as_ref(), "recipeGroups", "recipeGroups", &mut st);
    let used = d.s(static_field(recipe_t, "TileUsedInRecipes"), 0);
    compare(j.get("tileUsedInRecipes").unwrap(), used.as_ref(), "tileUsedInRecipes", "tileUsedInRecipes", &mut st);
    let counts = d.s(static_field(recipe_t, "TileCountsAs"), 0);
    compare(j.get("tileCountsAs").unwrap(), counts.as_ref(), "tileCountsAs", "tileCountsAs", &mut st);
    st
}

unsafe fn dump_tile_object_data(d: &mut Dumper) -> Stats {
    let j = refdata("tile_object_data.json");
    let mut st = Stats::default();
    let tod = meta_by_name("Terraria.ObjectData.TileObjectData");
    compare(j.get("tileCount").unwrap(), Some(&Got::Int((*terraria::id::tile_id::s()).count as i128)), "tileCount", "tileCount", &mut st);
    let rt::Reflect::Seq { items: data, .. } = rt::obj_reflect(static_field(tod, "_data")) else { panic!("TileObjectData._data is not a list") };
    let list_of = |d: &mut Dumper, o: Obj, prop: &str| -> Option<Vec<Obj>> {
        let l = d.member(o, tod, prop)?;
        if l.is_null() {
            return None;
        }
        match rt::obj_reflect(l) {
            rt::Reflect::Seq { items, .. } => Some(items),
            _ => None,
        }
    };
    let props = |d: &mut Dumper, o: Obj| -> Got { d.members(o, 0, 2).unwrap_or(Got::Null) };
    let alts = |d: &mut Dumper, o: Obj| -> Got {
        let Some(al) = list_of(d, o, "Alternates") else { return Got::Arr(Vec::new()) };
        Got::Arr(al.iter().enumerate().map(|(a, x)| if x.is_null() { Got::Null } else { Got::Obj(vec![("alternate".to_owned(), Got::Int(a as i128 + 1)), ("data".to_owned(), props(d, *x))]) }).collect())
    };
    let mut out = Vec::new();
    for (ty, dd) in data.iter().enumerate() {
        if dd.is_null() {
            continue;
        }
        let mut styles = Vec::new();
        if let Some(subs) = list_of(d, *dd, "SubTiles") {
            for (s, sd) in subs.iter().enumerate() {
                let own = !sd.is_null();
                let eff = if own { *sd } else { *dd };
                styles.push(Got::Obj(vec![("style".to_owned(), Got::Int(s as i128)), ("explicit".to_owned(), Got::Bool(own)), ("data".to_owned(), props(d, eff)), ("alternates".to_owned(), alts(d, eff))]));
            }
        }
        out.push(Got::Obj(vec![("tile".to_owned(), Got::Int(ty as i128)), ("base".to_owned(), props(d, *dd)), ("alternates".to_owned(), alts(d, *dd)), ("styles".to_owned(), Got::Arr(styles))]));
    }
    compare(j.get("tileObjectData").unwrap(), Some(&Got::Arr(out)), "tileObjectData", "", &mut st);
    st
}

unsafe fn dump_framing(d: &mut Dumper) -> Stats {
    let j = refdata("framing_tables.json");
    let mut st = Stats::default();
    let fr = meta_by_name("Terraria.Framing");
    let mut fields: Vec<&'static rt::FieldMeta> = fr.fields.iter().filter(|f| f.is_static && !f.literal && !f.name.contains('<')).collect();
    fields.sort_by(|a, b| ord(a.name, b.name));
    let mut parts = Vec::new();
    for f in fields {
        if let Some(g) = d.s((f.get)(rt::Gc::NULL), 0) {
            parts.push((f.name.to_owned(), g));
        }
    }
    compare(&j, Some(&Got::Obj(parts)), "", "", &mut st);
    st
}

unsafe fn dump_lang(r: &mut Report) -> Stats {
    let j = refdata("lang_en.json");
    let mut st = Stats::default();
    let lm = (*terraria::localization::language_manager::s()).instance;
    let culture = terraria::localization::language_manager::active_culture(lm.ptr());
    let cname = terraria::localization::game_culture::name(culture.ptr());
    compare(j.get("culture").unwrap(), Some(&Got::Str(cname.as_str().to_owned())), "culture", "culture", &mut st);
    let ic = (*terraria::id::item_id::s()).count as i32;
    let nc = (*terraria::id::npcid::s()).count as i32;
    let nneg = terraria::id::npcid::NEGATIVE_ID_COUNT as i32;
    let pc = (*terraria::id::projectile_id::s()).count as i32;
    let bc = (*terraria::id::buff_id::s()).count;
    let xc = (*terraria::id::prefix_id::s()).count;
    let mut table = |r: &mut Report, name: &str, from: i32, to: i32, get: &dyn Fn(i32) -> Str| {
        let mut parts = Vec::new();
        for id in from..=to {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt::run(|| get(id)))) {
                Ok(Ok(s)) if !s.is_null() => parts.push((id.to_string(), Got::Str(s.as_str().to_owned()))),
                Ok(Ok(_)) => {}
                Ok(Err(_)) => {}
                Err(p) => r.fail(format!("lang_en.json {name}[{id}] stopped: {}", panic_text(&p))),
            }
        }
        compare(j.get(name).unwrap(), Some(&Got::Obj(parts)), name, name, &mut st);
    };
    let value = |t: rt::Gc<terraria::localization::localized_text::LocalizedText>| -> Str {
        if t.is_null() {
            Str::NULL
        } else {
            terraria::localization::localized_text::value(t.ptr())
        }
    };
    table(r, "item", 1, ic - 1, &|id| terraria::lang::get_item_name_value(id));
    table(r, "npc", nneg + 1, nc - 1, &|id| terraria::lang::get_npc_name_value(id));
    table(r, "projectile", 1, pc - 1, &|id| value(terraria::lang::get_projectile_name(id)));
    table(r, "buffName", 1, bc - 1, &|id| terraria::lang::get_buff_name(id));
    table(r, "buffDescription", 1, bc - 1, &|id| terraria::lang::get_buff_description(id));
    table(r, "prefix", 1, xc - 1, &|id| value(*(*terraria::lang::s()).prefix.at(id)));
    st
}

#[test]
fn main_initialize_as_one_call() {
    let run = || unsafe {
        std::panic::set_hook(Box::new(|_| {}));
        let mut r = Report { text: String::new(), failures: Vec::new() };
        // the game's embedded resources: files of the decompile, names in manifest order
        let root = terraria_host::sys::io::project_dir();
        terraria_gen_game::host::launch::install_resources(&root);
        // the host crate's Game needs its window description even though a dedicated server never opens one
        terraria_host::game::init(terraria_host::game::WindowState { width: 800, height: 600, x: 0, y: 0, active: true, handle: 0, display_width: 1920, display_height: 1080, scale: 1.0 });
        let mut ok = true;
        ok &= step(&mut r, "Program.SavePath = <scratch>", || {
            (*terraria::program::s()).save_path = Str::new("work/proof1-save");
        });
        ok &= step(&mut r, "Main.dedServ = true; Main.showSplash = false", || {
            (*terraria::main::s()).ded_serv = true;
            (*terraria::main::s()).show_splash = false;
        });
        ok &= step(&mut r, "LanguageManager.Instance.SetLanguage(GameCulture.DefaultCulture)", || {
            let lm = (*terraria::localization::language_manager::s()).instance;
            terraria::localization::language_manager::set_language__game_culture(lm.ptr(), terraria::localization::game_culture::default_culture());
        });
        let mut main = rt::Gc::NULL;
        ok &= step(&mut r, "main = new Main()", || main = terraria::main::new());
        ok &= step(&mut r, "Lang.InitializeLegacyLocalization()", || terraria::lang::initialize_legacy_localization());
        ok &= step(&mut r, "Main.rand = new UnifiedRandom(0)", || {
            (*terraria::main::s()).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(0));
        });
        ok &= ok && step(&mut r, "main.Initialize()", || terraria::main::initialize(main.ptr()));

        // ---- the dumps, in the harness' order (RefDump.cs:819)
        let mut rows: Vec<Row> = Vec::new();
        let mut d = Dumper::new();
        if ok {
            let mut dump = |r: &mut Report, rows: &mut Vec<Row>, d: &mut Dumper, file: &'static str, f: &mut dyn FnMut(&mut Report, &mut Dumper, &mut Vec<String>) -> Stats| {
                let mut notes = Vec::new();
                let t0 = std::time::Instant::now();
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(r, d, &mut notes))) {
                    Ok(st) => {
                        notes.push(format!("{:.1} s", t0.elapsed().as_secs_f64()));
                        row(r, rows, file, st, notes)
                    }
                    Err(p) => r.fail(format!("{file}: the comparison stopped: {}", panic_text(&p))),
                }
            };
            dump(&mut r, &mut rows, &mut d, "tile_arrays.json", &mut |_, d, _| dump_tile_arrays(d));
            dump(&mut r, &mut rows, &mut d, "id_sets.json", &mut |_, d, n| dump_id_sets(d, n));
            dump(&mut r, &mut rows, &mut d, "id_constants.json", &mut |_, d, n| dump_id_constants(d, n));
            dump(&mut r, &mut rows, &mut d, "items.json", &mut |r, d, _| {
                dump_entities(d, r, "items.json", "items", &|id| {
                    let o = terraria::item::new__void();
                    terraria::item::set_defaults(o.ptr(), id, rt::Gc::NULL);
                    o.up()
                })
            });
            dump(&mut r, &mut rows, &mut d, "npcs.json", &mut |r, d, _| {
                dump_entities(d, r, "npcs.json", "npcs", &|id| {
                    let o = terraria::npc::new();
                    terraria::npc::set_defaults(o.ptr(), id, rt::zero());
                    o.up()
                })
            });
            dump(&mut r, &mut rows, &mut d, "projectiles.json", &mut |r, d, _| {
                dump_entities(d, r, "projectiles.json", "projectiles", &|id| {
                    let o = terraria::projectile::new();
                    terraria::projectile::set_defaults(o.ptr(), id);
                    o.up()
                })
            });
            dump(&mut r, &mut rows, &mut d, "buffs.json", &mut |_, d, _| dump_buffs(d));
            dump(&mut r, &mut rows, &mut d, "recipes.json", &mut |_, d, _| dump_recipes(d));
            dump(&mut r, &mut rows, &mut d, "tile_object_data.json", &mut |_, d, _| dump_tile_object_data(d));
            dump(&mut r, &mut rows, &mut d, "framing_tables.json", &mut |_, d, _| dump_framing(d));
            dump(&mut r, &mut rows, &mut d, "lang_en.json", &mut |r, _, _| dump_lang(r));
        }

        // ---- the table
        r.line("");
        r.line("| refdata file | values compared | exact | not exact | lambda delegates (compared as \"a lambda\") | notes |");
        r.line("|---|---:|---:|---:|---:|---|");
        let (mut tc, mut te) = (0u64, 0u64);
        for row in &rows {
            tc += row.st.compared;
            te += row.st.exact;
            let mut worst: Vec<(&String, &u64)> = row.st.by_member.iter().collect();
            worst.sort_by(|a, b| b.1.cmp(a.1));
            let mut notes = row.notes.clone();
            if !worst.is_empty() {
                notes.push(format!("not exact by member: {}", worst.iter().take(8).map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ")));
            }
            let mut s = String::new();
            let _ = write!(s, "| `{}` | {} | {} | {} | {} | {} |", row.file, row.st.compared, row.st.exact, row.st.not_exact(), row.st.lambdas, notes.join("; "));
            r.line(s);
        }
        r.line(format!("| **total** | {tc} | {te} | {} | | |", tc - te));
        r.line("");
        r.line(format!("members the reflection port left out as RefDump did (not serialisable): {}", d.skipped.len()));
        for s in d.skipped.iter().take(40) {
            r.line(format!("    {s}"));
        }
        for row in &rows {
            if row.st.not_exact() > 0 {
                r.failures.push(format!("{}: {} of {} values not exact", row.file, row.st.not_exact(), row.st.compared));
            }
        }
        if ok && rows.len() != 11 {
            r.failures.push(format!("{} of 11 dumps compared", rows.len()));
        }
        for (p, n) in &d.problems {
            r.fail(format!("reflection port: {p} (x{n})"));
        }

        // ---- what the run reached that is not translated game code
        r.line(format!("gaps reached (each fails the proof): {}", rt::gaps().len()));
        for (g, n) in rt::gaps() {
            r.fail(format!("gap reached: {g} (x{n})"));
        }
        let void: Vec<_> = game::rt::host::unimplemented_reached();
        r.line(format!("unimplemented void library members and opaque library objects reached: {}", void.len()));
        for (g, n) in &void {
            if g.contains("..ctor") {
                r.line(format!("    opaque object (never looked into; any use would have stopped the run): {g} (x{n})"));
            } else {
                r.fail(format!("unimplemented void library member reached: {g} (x{n})"));
            }
        }
        r.line(format!("inert library members reached (game.json \"inert\", with reasons): {}", game::rt::host::inert_reached().len()));
        for (g, n) in game::rt::host::inert_reached() {
            r.line(format!("    {g}: {n}"));
        }
        r.line(format!("configured values used (game.json \"stubs\", with reasons): {}", rt::stubs_hit().len()));
        for (g, n) in rt::stubs_hit() {
            r.line(format!("    {g}: {n}"));
        }
        let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("cs2rs-proof1.txt");
        let _ = std::fs::write(&out, &r.text);
        eprintln!("report: {}", out.display());
        if !r.failures.is_empty() {
            eprintln!("proof 1 FAILED: {} failure(s)", r.failures.len());
            for f in r.failures.iter().take(60) {
                eprintln!("    {f}");
            }
            panic!("proof 1 failed");
        }
        eprintln!("proof 1 passed");
    };
    // the game recurses deeply in places; the main thread of the real process has 1 MB, the JIT's frames are smaller
    terraria_host::run_with_stack(256 << 20, "proof 1", run);
}
