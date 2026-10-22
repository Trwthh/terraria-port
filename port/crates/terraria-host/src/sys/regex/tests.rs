use super::*;
use crate::json::tests::b64d;
use terraria_gen_rt::rt::{Gc, Str};

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\r', "\\r").replace('\n', "\\n").replace('\t', "\\t")
}
fn dotnet(file: &str) -> std::path::PathBuf {
    crate::manifest_dir().join("tests").join("dotnet").join(file)
}
fn s(x: &str) -> Str {
    Str::new(x)
}
fn val(g: Gc<Match>) -> (bool, i32, i32, String) {
    // SAFETY: live match objects
    let g = unsafe { &*g.ptr() };
    (g.success(), g.index(), g.length(), g.value().as_str().to_owned())
}

/// The game's patterns and 18 more, 87 inputs each (2610 cases): every match with all groups (index, length, value),
/// `IsMatch`, `Replace` with `<$0|$1|${0}>` and `Split`, against real .NET (tests/dotnet/regex_probe.ps1).
#[test]
fn matches_replace_split_equal_dotnet() {
    let pats = std::fs::read_to_string(dotnet("regex_patterns.expected")).expect("regex_patterns.expected");
    let mut regexes = std::collections::HashMap::new();
    for l in pats.lines() {
        let p: Vec<&str> = l.split('\t').collect();
        let (pattern, opts) = (b64d(p[1]), p[2].parse::<i32>().expect("options"));
        regexes.insert(p[0].to_owned(), (Regex::new_gc(s(&pattern), opts), pattern));
    }
    let cases = std::fs::read_to_string(dotnet("regex_cases.expected")).expect("regex_cases.expected");
    let (mut n, mut bad) = (0, Vec::new());
    for l in cases.lines() {
        let p: Vec<&str> = l.split('\t').collect();
        let name = p[0];
        let input = b64d(p[1]);
        let (exp_matches, exp_rep, exp_split) = (b64d(p[2]), b64d(p[3]), b64d(p[4]));
        let (re, pattern) = &regexes[name];
        // SAFETY: live regex object
        let re = unsafe { &*re.ptr() };
        let mut got = String::new();
        let ms = re.matches(s(&input));
        for m in ms.as_slice() {
            // SAFETY: live match
            let mm = unsafe { &*m.ptr() };
            let (_, idx, len, v) = val(*m);
            got.push_str(&format!("M {idx} {len} [{}]\n", esc(&v)));
            for i in 1..mm.group_count() {
                let (ok, gi, gl, gv) = val(mm.group_at(i));
                let nm = mm.names[i as usize].clone().unwrap_or_else(|| i.to_string());
                got.push_str(&format!("  G{i} {nm} {} {gi} {gl} [{}]\n", if ok { "True" } else { "False" }, esc(&gv)));
            }
        }
        let first = unsafe { &*re.match_(s(&input)).ptr() }.success();
        got.push_str(&format!("first {} isMatch {}\n", if first { "True" } else { "False" }, if re.is_match(s(&input)) { "True" } else { "False" }));
        let rep = esc(re.replace(s(&input), s("<$0|$1|${0}>")).as_str());
        let split = re.split(s(&input)).as_slice().iter().map(|x| esc(x.as_str())).collect::<Vec<_>>().join("\u{a6}");
        n += 1;
        if got != exp_matches || rep != exp_rep || split != exp_split {
            bad.push(format!("{name} /{pattern}/ on {input:?}\n got {got:?}\n exp {exp_matches:?}\n rep {rep:?} vs {exp_rep:?}\n split {split:?} vs {exp_split:?}"));
        }
    }
    println!("{n} cases, {} differ", bad.len());
    assert!(bad.is_empty(), "{}", bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn chat_tags_with_look_behind() {
    // ChatManager.Regexes.Format
    let re = Regex::new_gc(s(r"(?<!\\)\[(?<tag>[a-zA-Z]{1,10})(\/(?<options>[^:]+))?:(?<text>.+?)(?<!\\)\]"), COMPILED | SINGLELINE);
    let re = unsafe { &*re.ptr() };
    let ms = re.matches(s("a [c/FF0000:red] b \\[i:1\\] [i:2]"));
    let v: Vec<String> = ms.as_slice().iter().map(|m| val(*m).3).collect();
    assert_eq!(v, ["[c/FF0000:red]", "[i:2]"]);
    let m = unsafe { &*ms.as_slice()[0].ptr() };
    // named groups come after the unnamed one: 1 = (/options), 2 = tag, 3 = options, 4 = text
    assert_eq!(val(m.group_at(1)).3, "/FF0000");
    assert_eq!(val(m.group_named(s("tag"))).3, "c");
    assert_eq!(val(m.group_at(3)).3, "FF0000");
    assert_eq!(val(m.group_named(s("text"))).3, "red");
}

#[test]
fn unsupported_constructs_are_refused_not_misread() {
    for p in ["(?(1)a|b)", "a(?<x-y>b)", "[a-z-[aeiou]]", "\\G"] {
        let r = terraria_gen_rt::rt::run(|| Regex::new_gc(s(p), 0));
        assert!(r.is_err(), "{p} must be refused");
    }
    assert!(terraria_gen_rt::rt::run(|| Regex::new_gc(s("(a"), 0)).is_err());
    assert_eq!(regex_escape(s("a.b*c d")).as_str(), "a\\.b\\*c\\ d");
}
