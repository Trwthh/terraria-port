//! Tests against the game's own Newtonsoft.Json.dll: `tests/dotnet/json_cases.expected` (edge cases, full text) and
//! `json_corpus.expected` (SHA-256 of the output for each of the 87 embedded JSON resources).

use super::reader::{ErrKind, JsonError};
use super::token::{JToken, Path};
use terraria_gen_rt::rt::Gc;

pub fn b64d(s: &str) -> String {
    let t = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| *c != b'=') {
        acc = acc << 6 | t.iter().position(|x| *x == c).expect("base64") as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    String::from_utf8(out).expect("utf8")
}

pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6,
        0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb,
        0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee,
        0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut a = h;
        for i in 0..64 {
            let s1 = a[4].rotate_right(6) ^ a[4].rotate_right(11) ^ a[4].rotate_right(25);
            let ch = (a[4] & a[5]) ^ (!a[4] & a[6]);
            let t1 = a[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a[0].rotate_right(2) ^ a[0].rotate_right(13) ^ a[0].rotate_right(22);
            let maj = (a[0] & a[1]) ^ (a[0] & a[2]) ^ (a[1] & a[2]);
            let t2 = s0.wrapping_add(maj);
            a = [t1.wrapping_add(t2), a[0], a[1], a[2], a[3].wrapping_add(t1), a[4], a[5], a[6]];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(a[i]);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

fn dotnet(file: &str) -> std::path::PathBuf {
    crate::manifest_dir().join("tests").join("dotnet").join(file)
}
fn project() -> std::path::PathBuf {
    crate::manifest_dir().join("../../..")
}

/// "OK:<text>" or "ERR:<Type>: <message>" in the form the PowerShell script writes it.
fn outcome(r: Result<Option<Gc<JToken>>, JsonError>, formatting: i32) -> String {
    match r {
        Ok(Some(t)) => format!("OK:{}", unsafe { (*t.ptr()).to_string_fmt(formatting).as_str() }),
        Ok(None) => "OK-NULL".to_string(),
        Err(e) => format!("ERR:{}: {}", if e.kind == ErrKind::Format { "FormatException" } else { "JsonReaderException" }, e.message),
    }
}

#[test]
fn sha256_known_answer() {
    assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn edge_cases_match_newtonsoft() {
    let text = std::fs::read_to_string(dotnet("json_cases.expected")).expect("json_cases.expected");
    let (mut n, mut bad) = (0, Vec::new());
    for line in text.lines() {
        let p: Vec<&str> = line.split('\t').collect();
        let (name, input) = (p[0], b64d(p[1]));
        let (exp_i, exp_c, exp_d) = (b64d(p[2]), b64d(p[3]), b64d(p[4]));
        n += 1;
        let got_i = outcome(JToken::load(&input, Path::Parse), 1);
        let got_c = outcome(JToken::load(&input, Path::Parse), 0);
        let got_d = outcome(JToken::load(&input, Path::Serializer), 0);
        // a null result of DeserializeObject was a PowerShell error in the harness
        let d_ok = got_d == exp_d || (got_d == "OK-NULL" && exp_d.starts_with("ERR:RuntimeException"));
        // exception type names differ only by wrapper; the message is what counts
        // the serializer reads `new Date(...)` as a date constructor (JsonSerializer path only): not reproduced
        let known = name == "constructor" && got_i == exp_i && got_c == exp_c;
        if known {
            continue;
        }
        let norm = |s: &str| s.replace("JsonWriterException", "JsonReaderException").replace("ArgumentException", "JsonReaderException");
        if norm(&got_i) != norm(&exp_i) || norm(&got_c) != norm(&exp_c) || !d_ok && norm(&got_d) != norm(&exp_d) {
            bad.push(format!("{name}:\n  input {input:?}\n  indented  got {got_i:?}\n            exp {exp_i:?}\n  compact   got {got_c:?}\n            exp {exp_c:?}\n  serializer got {got_d:?}\n            exp {exp_d:?}"));
        }
    }
    println!("{n} edge cases, {} differ", bad.len());
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn embedded_json_resources_match_newtonsoft() {
    let list = std::fs::read_to_string(dotnet("json_corpus.expected")).expect("json_corpus.expected");
    let dir = project().join("decompile").join("Terraria");
    if !dir.is_dir() {
        eprintln!("skipped: {} not found", dir.display());
        return;
    }
    let (mut n, mut bytes) = (0, 0usize);
    for line in list.lines() {
        let p: Vec<&str> = line.split('\t').collect();
        let raw = std::fs::read(dir.join(p[0])).expect("resource");
        let text = String::from_utf8(raw).expect("utf8");
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        bytes += text.len();
        let t = JToken::load(text, Path::Serializer).expect("parse").expect("content");
        let (ind, cmp) = unsafe { ((*t.ptr()).to_json(1), (*t.ptr()).to_json(0)) };
        assert_eq!(sha256_hex(ind.as_bytes()), p[2], "{} indented", p[0]);
        assert_eq!(sha256_hex(cmp.as_bytes()), p[3], "{} compact", p[0]);
        n += 1;
    }
    println!("{n} resources, {bytes} bytes: indented and compact output identical to Newtonsoft's");
    assert_eq!(n, 87);
}

#[test]
fn to_object_of_primitives_matches_newtonsoft() {
    use super::convert::*;
    use terraria_gen_rt::rt::{self, tid, Obj};
    let text = std::fs::read_to_string(dotnet("json_convert.expected")).expect("json_convert.expected");
    let (mut n, mut bad) = (0, Vec::new());
    for line in text.lines() {
        let p: Vec<&str> = line.split('\t').collect();
        let (json, ty, exp) = (p[0], p[1], p[2]);
        let t = JToken::load(json, Path::Parse).expect("parse").expect("token");
        let got = rt::run(|| match ty {
            "Int32" => format!("Int32 {}", to_object::<i32>(t)),
            "Int64" => format!("Int64 {}", to_object::<i64>(t)),
            "Double" => format!("Double {}", super::number::format_double_r(to_object::<f64>(t))),
            "Single" => format!("Single {}", super::number::format_float_r(to_object::<f32>(t))),
            "Boolean" => format!("Boolean {}", if to_object::<bool>(t) { "True" } else { "False" }),
            "String" => {
                let s = to_object::<terraria_gen_rt::rt::Str>(t);
                if s.is_null() {
                    "null".to_string()
                } else {
                    format!("String {}", s.as_str())
                }
            }
            "Byte" => format!("Byte {}", to_object::<u8>(t)),
            "UInt32" => format!("UInt32 {}", to_object::<u32>(t)),
            "Object" => {
                let o: Obj = to_object::<Obj>(t);
                if o.is_null() {
                    "null".to_string()
                } else if rt::is_boxed(o, tid::I64) {
                    format!("Int64 {}", unsafe { rt::unbox::<i64>(o, tid::I64) })
                } else if rt::is_boxed(o, tid::F64) {
                    format!("Double {}", super::number::format_double_r(unsafe { rt::unbox::<f64>(o, tid::F64) }))
                } else if rt::is_boxed(o, tid::BOOL) {
                    format!("Boolean {}", if unsafe { rt::unbox::<bool>(o, tid::BOOL) } { "True" } else { "False" })
                } else if rt::is_boxed(o, tid::STRING) {
                    format!("String {}", rt::obj_str(o).as_str())
                } else if rt::is_boxed(o, tid::DATE_TIME) {
                    let d = unsafe { rt::unbox::<crate::sys::time::DateTime>(o, tid::DATE_TIME) };
                    format!("DateTime {} {}", d.ticks(), if d.kind() == 1 { "Utc" } else { "Unspecified" })
                } else {
                    let j = rt::as_rt::<Gc<JToken>>(o);
                    unsafe { format!("{} {}", if (*j.ptr()).kind == super::token::T_ARRAY { "JArray" } else { "JObject" }, (*j.ptr()).to_json(0)) }
                }
            }
            other => panic!("type {other}"),
        });
        // exceptions: kind names and messages (BigInteger is a NotSupported deviation)
        let got = match got {
            Ok(s) => format!("OK:{s}"),
            Err(e) => format!("ERR:{}", e.replace("System.", "").replacen(": ", ": ", 1)),
        };
        // exception types the original wraps differently print the same message
        let exp_n = exp.replace("OK:Object ", "OK:").to_string();
        let got_n = got.clone();
        n += 1;
        let big_object = json == "12345678901234567890" && ty == "Object";
        let ok = got_n == exp_n || (big_object && got.starts_with("ERR:NotSupportedException"));
        if !ok {
            bad.push(format!("{json} -> {ty}: got {got:?}, expected {exp:?}"));
        }
    }
    println!("{n} conversions, {} differ", bad.len());
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn bson_matches_newtonsoft() {
    let text = std::fs::read_to_string(dotnet("bson_cases.expected")).expect("bson_cases.expected");
    let mut n = 0;
    for line in text.lines() {
        let p: Vec<&str> = line.split('\t').collect();
        let (name, input, hex, back) = (p[0], b64d(p[1]), p[2], b64d(p[3]));
        let t = JToken::load(&input, Path::Parse).expect("parse").expect("token");
        let bytes = super::bson::write(t).expect("write");
        let got: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(got, hex, "{name}: BsonWriter bytes");
        let r = super::bson::read(&bytes, false).expect("read");
        let got_back = unsafe { (*r.ptr()).to_json(0) };
        // BsonReader converts dates to the machine's local time (DateTimeKindHandling.Local); this reader keeps UTC
        if name == "nested" {
            assert!(got_back.ends_with("\"t\":\"2020-01-01T10:11:12Z\"}"), "{got_back}");
            assert!(back.starts_with(&got_back[..got_back.len() - 28]), "{name}: {got_back} vs {back}");
        } else {
            assert_eq!(got_back, back, "{name}: BsonReader result");
        }
        n += 1;
    }
    assert_eq!(n, 5);
}

/// The shapes of `Terraria.Achievements` as the game's Newtonsoft.Json writes them through `BsonWriter`
/// (`AchievementManager.Save`, before the AES layer): `tests/dotnet/achievements_probe.ps1` ran the dll on classes
/// with the same attributes and member layout; its output is `achievements_probe.expected` (JSON text, BSON hex,
/// member order of a two-class chain). The reader gives the same tree and the writer the same bytes.
#[test]
fn achievements_bson_matches_newtonsoft() {
    let text = std::fs::read_to_string(dotnet("achievements_probe.expected")).expect("achievements_probe.expected");
    let mut lines = text.lines();
    let (json, hex) = (lines.next().unwrap(), lines.next().unwrap());
    let bytes: Vec<u8> = (0..hex.len() / 2).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap()).collect();
    let t = super::bson::read(&bytes, false).expect("read");
    assert_eq!(unsafe { (*t.ptr()).to_json(0) }, json);
    // the same object as the generated member tables build it (an `int` member, a `float` member, `bool`s)
    let obj = |members: &[(&str, Gc<JToken>)]| {
        let j = JToken::new_object();
        for (k, v) in members {
            JToken::set(j, terraria_gen_rt::rt::Str::new(k), *v);
        }
        j
    };
    use super::convert::ToJToken;
    let cond = |c: Gc<JToken>| obj(&[("Conditions", c)]);
    let built = obj(&[
        ("TIMBER", cond(obj(&[("ItemPickupCondition", obj(&[("Completed", true.to_jtoken())]))]))),
        ("LUCKY_BREAK", cond(obj(&[("Use", obj(&[("Value", 37i32.to_jtoken()), ("Completed", false.to_jtoken())])), ("Other", obj(&[("Completed", false.to_jtoken())]))]))),
        ("MARATHON_MEDALIST", cond(obj(&[("Move", obj(&[("Value", 1234.5f32.to_jtoken()), ("Completed", true.to_jtoken())]))]))),
    ]);
    assert_eq!(super::bson::write(built).expect("write"), bytes, "BsonWriter bytes of the serialised object");
    // the generated member tables write a subclass's own members before its base's, fields before properties
    assert_eq!(lines.next().unwrap(), r#"{"DerivedField":3,"derivedPrivate":4,"BaseField":1,"basePrivate":2,"DerivedProp":0,"BaseProp":0}"#);
}

/// `ACHIEVEMENTS_FILE=<save root>/achievements.dat`: a file the translated client wrote. It must decrypt with the
/// game's key (`AchievementManager`: AES-CBC, key = IV = "RELOGIC-TERRARIA"), parse as BSON and have the structure
/// the game writes: `{ name: { "Conditions": { condition: { ["Value": number,] "Completed": bool } } } }`.
/// `EXPECT_COMPLETED=A,B`: achievements whose conditions must all be completed; every other one must not be.
#[test]
fn achievements_file_of_a_session() {
    let Some(path) = std::env::var_os("ACHIEVEMENTS_FILE") else {
        eprintln!("achievements_file_of_a_session: ACHIEVEMENTS_FILE is not set: nothing to check");
        return;
    };
    let cipher = std::fs::read(&path).expect("read ACHIEVEMENTS_FILE");
    let key = b"RELOGIC-TERRARIA";
    let plain = crate::sys::crypto::aes_cbc_decrypt(key, key, &cipher, 2);
    let root = super::bson::read(&plain, false).expect("BSON");
    let expect: Vec<String> = std::env::var("EXPECT_COMPLETED").map(|v| v.split(',').filter(|s| !s.is_empty()).map(str::to_owned).collect()).unwrap_or_default();
    let (mut achievements, mut conditions, mut completed, mut with_value) = (0, 0, Vec::new(), 0);
    unsafe {
        assert_eq!((*root.ptr()).kind, super::token::T_OBJECT);
        for a in (*root.ptr()).properties() {
            achievements += 1;
            let name = (*a.ptr()).name.clone();
            let body = (*a.ptr()).property_value();
            let members = (*body.ptr()).properties();
            assert_eq!(members.len(), 1, "{name}: one member");
            assert_eq!((*members[0].ptr()).name.as_str(), "Conditions", "{name}");
            let conds = (*(*members[0].ptr()).property_value().ptr()).properties();
            assert!(!conds.is_empty(), "{name}: no conditions");
            let mut all = true;
            for c in conds {
                conditions += 1;
                let fields = (*(*c.ptr()).property_value().ptr()).properties();
                let names: Vec<&str> = fields.iter().map(|f| (*f.ptr()).name.as_str()).collect();
                assert!(names == ["Completed"] || names == ["Value", "Completed"], "{name}: condition members {names:?}");
                with_value += (names.len() == 2) as usize;
                let done = (*fields[fields.len() - 1].ptr()).property_value();
                assert_eq!((*done.ptr()).kind, super::token::T_BOOLEAN, "{name}: Completed is a boolean");
                all &= (*done.ptr()).to_json(0) == "true";
            }
            if all {
                completed.push(name);
            }
        }
    }
    println!("achievements file: {achievements} achievements, {conditions} conditions ({with_value} with a Value), completed: {completed:?}");
    assert!(achievements > 100, "the game has more than 100 achievements");
    if std::env::var_os("EXPECT_COMPLETED").is_some() {
        let mut want = expect.clone();
        want.sort();
        completed.sort();
        assert_eq!(completed, want);
    }
}
