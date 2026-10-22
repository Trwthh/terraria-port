use super::*;
use crate::json::tests::b64d;

fn set_culture(name: &str) {
    use terraria_gen_rt::rt::sys::CultureInfo;
    CultureInfo::set_current(CultureInfo::new_gc(Str::new(name)));
}

/// Every (culture, date, format) of tests/dotnet/culture_probe.ps1 against real .NET Framework: 12 game cultures +
/// invariant, 5 dates, 17 formats and ToShortDateString / ToLongDateString.
#[test]
fn datetime_formats_match_dotnet_in_every_game_culture() {
    let t = std::fs::read_to_string(crate::manifest_dir().join("tests").join("dotnet").join("datetime.expected")).expect("datetime.expected");
    let (mut n, mut bad) = (0, Vec::new());
    for line in t.lines() {
        let p: Vec<&str> = line.split('\t').collect();
        let (culture, ticks, format, expected) = (p[0], p[1].parse::<i64>().unwrap(), p[2], b64d(p[3]));
        // the invariant culture is used through ToString(format, InvariantCulture); the probe formats it as the thread culture
        let d = DateTime::from_ticks(ticks);
        let got = if culture.is_empty() {
            match format {
                "ToShortDateString" => Str::new(&d.format_custom(CULTURES[0].short_date, &CULTURES[0])),
                "ToLongDateString" => Str::new(&d.format_custom(CULTURES[0].long_date, &CULTURES[0])),
                f => d.to_string_invariant(Str::new(f)),
            }
        } else {
            set_culture(culture);
            match format {
                "ToShortDateString" => d.to_short_date_string(),
                "ToLongDateString" => d.to_long_date_string(),
                f => d.to_string_format(Str::new(f)),
            }
        };
        n += 1;
        if got.as_str() != expected {
            bad.push(format!("{culture} {ticks} {format}: got {:?}, expected {expected:?}", got.as_str()));
        }
    }
    set_culture("en-US");
    println!("{n} formatted values, {} differ", bad.len());
    assert!(bad.is_empty(), "{} differences, first: {}", bad.len(), bad.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
}
