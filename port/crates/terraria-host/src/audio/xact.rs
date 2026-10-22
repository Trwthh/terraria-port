//! XACT3 data files of the legacy music path: the global settings `TerrariaMusic.xgs` ("XGSF", tool version 42),
//! the sound bank `Sound Bank.xsb` ("SDBK", tool version 43) and the wave bank header `Wave Bank.xwb` (parsed in
//! `audio.rs`). No XACT runtime source is available here: the layout was read from the real files (hex dumps) and
//! from the FAudio parser's description (field order, `read_volbyte`), and is checked by the tests in `audio.rs`
//! against the player's install (104 cues, 104 simple sounds, 1 wave bank, 3 categories, 8 variables, 1 RPC).
//!
//! What is understood: simple and complex-free banks like Terraria's: every cue is a "simple cue" that points at one
//! sound; a sound has a category, a volume byte, a pitch, a priority, one wave (index into a named wave bank) and
//! RPC codes (offsets of RPC curves in the `.xgs`). Complex sounds (several tracks, events, variations,
//! transitions), DSP presets and interactive cues are NOT parsed: a cue that needs them fails with a message.
//!
//! Volume: a byte `b` means `3969 * log10(b / 28240) + 8715` hundredths of a decibel (the formula FAudio uses;
//! 180 is 0 dB, 90 is -11.94 dB, which is the constant `Terraria.Audio.ASoundEffectBasedAudioTrack` subtracts when
//! it emulates XACT for OGG music: `dB = 31 * Volume - 25 - 11.94`; the 31 * Volume - 25 is the `Volume` RPC
//! curve of the .xgs, a straight line from -25 dB at 0 to +6 dB at 1).

fn u16_at(b: &[u8], o: usize) -> Result<u16, String> {
    b.get(o..o + 2).map(|x| u16::from_le_bytes([x[0], x[1]])).ok_or_else(|| format!("truncated at {o}"))
}
fn u32_at(b: &[u8], o: usize) -> Result<u32, String> {
    b.get(o..o + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]])).ok_or_else(|| format!("truncated at {o}"))
}
fn f32_at(b: &[u8], o: usize) -> Result<f32, String> {
    Ok(f32::from_bits(u32_at(b, o)?))
}
fn cstr_at(b: &[u8], o: usize) -> Result<String, String> {
    let rest = b.get(o..).ok_or_else(|| format!("name offset {o} outside the file"))?;
    let end = rest.iter().position(|c| *c == 0).ok_or("unterminated name")?;
    Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
}

/// FAudio's `read_volbyte`: hundredths of a decibel from the volume byte.
pub fn volume_byte_to_centi_db(b: u8) -> f32 {
    if b == 0 {
        return -9600.0;
    }
    (3969.0 * (b as f64 / 28240.0).log10() + 8715.0) as f32
}
pub fn db_to_amplitude(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[derive(Debug, Clone)]
pub struct CategoryDef {
    pub name: String,
    pub instance_limit: u8,
    pub parent: i16,
    /// volume of the category in decibels
    pub volume_db: f32,
}
#[derive(Debug, Clone)]
pub struct VariableDef {
    pub name: String,
    /// bit 0: public, 1: read-only, 2: cue instance (per cue), 3: reserved
    pub flags: u8,
    pub initial: f32,
    pub min: f32,
    pub max: f32,
}
#[derive(Debug, Clone)]
pub struct RpcDef {
    /// file offset of the record: the "RPC code" sounds refer to
    pub code: u32,
    pub variable: u16,
    /// what the curve controls: 0 volume (hundredths of dB), 1 pitch, 2 reverb send, 3.. filter / DSP
    pub parameter: u16,
    /// (x, y, curve type)
    pub points: Vec<(f32, f32, u8)>,
}
impl RpcDef {
    /// The curve at `x`: linear between points, flat outside (other curve types are treated as linear).
    pub fn eval(&self, x: f32) -> f32 {
        let p = &self.points;
        if p.is_empty() {
            return 0.0;
        }
        if x <= p[0].0 {
            return p[0].1;
        }
        for w in p.windows(2) {
            if x <= w[1].0 {
                let span = w[1].0 - w[0].0;
                if span <= 0.0 {
                    return w[1].1;
                }
                return w[0].1 + (w[1].1 - w[0].1) * (x - w[0].0) / span;
            }
        }
        p[p.len() - 1].1
    }
}

/// `TerrariaMusic.xgs`
#[derive(Debug, Clone, Default)]
pub struct Xgs {
    pub categories: Vec<CategoryDef>,
    pub variables: Vec<VariableDef>,
    /// every RPC curve found by following the sound bank's codes is parsed on demand ([`Xgs::rpc`])
    pub bytes: Vec<u8>,
}

impl Xgs {
    pub fn parse(b: &[u8]) -> Result<Xgs, String> {
        if b.get(0..4) != Some(b"XGSF") {
            return Err("not an XACT global settings file (no XGSF signature)".into());
        }
        let tool = u16_at(b, 6)?;
        if tool != 42 {
            return Err(format!("unsupported XGS tool version {tool}"));
        }
        let cat_count = u16_at(b, 0x13)? as usize;
        let var_count = u16_at(b, 0x15)? as usize;
        let cat_off = u32_at(b, 0x21)? as usize;
        let var_off = u32_at(b, 0x25)? as usize;
        // the header holds 11 offsets from 0x21: [0] categories, [1] variables, [6] category names, [7] variable
        // names, [8] RPC curves (the other entries are index / hash tables and DSP data this reader does not use)
        let offsets: Vec<usize> = (0..11).map(|i| u32_at(b, 0x21 + 4 * i).map(|v| v as usize)).collect::<Result<_, _>>()?;
        let names_at = |start: usize, count: usize| -> Result<Vec<String>, String> {
            let mut o = start;
            let mut v = Vec::new();
            for _ in 0..count {
                let n = cstr_at(b, o)?;
                o += n.len() + 1;
                v.push(n);
            }
            Ok(v)
        };
        let cat_names = names_at(offsets[6], cat_count)?;
        let var_names = names_at(offsets[7], var_count)?;
        let mut categories = Vec::new();
        for i in 0..cat_count {
            let r = cat_off + 10 * i;
            categories.push(CategoryDef {
                name: cat_names[i].clone(),
                instance_limit: *b.get(r).ok_or("truncated category")?,
                parent: u16_at(b, r + 5)? as i16,
                volume_db: volume_byte_to_centi_db(*b.get(r + 8).ok_or("truncated category")?) / 100.0,
            });
        }
        let mut variables = Vec::new();
        for i in 0..var_count {
            let r = var_off + 13 * i;
            variables.push(VariableDef { name: var_names[i].clone(), flags: *b.get(r).ok_or("truncated variable")?, initial: f32_at(b, r + 1)?, min: f32_at(b, r + 5)?, max: f32_at(b, r + 9)? });
        }
        Ok(Xgs { categories, variables, bytes: b.to_vec() })
    }

    pub fn category_index(&self, name: &str) -> Option<usize> {
        self.categories.iter().position(|c| c.name == name)
    }
    pub fn variable_index(&self, name: &str) -> Option<usize> {
        self.variables.iter().position(|v| v.name == name)
    }

    /// The RPC curve record at file offset `code`.
    pub fn rpc(&self, code: u32) -> Result<RpcDef, String> {
        let b = &self.bytes;
        let o = code as usize;
        let variable = u16_at(b, o)?;
        let count = *b.get(o + 2).ok_or("truncated RPC")? as usize;
        let parameter = u16_at(b, o + 3)?;
        let mut points = Vec::new();
        for i in 0..count {
            let r = o + 5 + 9 * i;
            points.push((f32_at(b, r)?, f32_at(b, r + 4)?, *b.get(r + 8).ok_or("truncated RPC")?));
        }
        Ok(RpcDef { code, variable, parameter, points })
    }
}

#[derive(Debug, Clone)]
pub struct SoundDef {
    pub flags: u8,
    pub category: u16,
    pub volume_db: f32,
    /// XACT pitch in hundredths of a semitone... stored as written (Terraria uses 0)
    pub pitch: i16,
    pub priority: u8,
    /// (wave index, wave bank index) of the single track of a simple sound
    pub wave: Option<(u16, u8)>,
    pub rpc_codes: Vec<u32>,
    /// complex sounds are not decoded
    pub complex: bool,
}

#[derive(Debug, Clone)]
pub struct CueDef {
    pub name: String,
    pub flags: u8,
    /// index into `sounds` (simple cues)
    pub sound: Option<usize>,
}

/// `Sound Bank.xsb`
#[derive(Debug, Clone, Default)]
pub struct Xsb {
    pub name: String,
    pub wavebanks: Vec<String>,
    pub cues: Vec<CueDef>,
    pub sounds: Vec<SoundDef>,
}

impl Xsb {
    pub fn parse(b: &[u8]) -> Result<Xsb, String> {
        if b.get(0..4) != Some(b"SDBK") {
            return Err("not an XACT sound bank (no SDBK signature)".into());
        }
        let tool = u16_at(b, 6)?;
        if tool != 43 {
            return Err(format!("unsupported XSB tool version {tool}"));
        }
        let simple_cues = u16_at(b, 19)? as usize;
        let complex_cues = u16_at(b, 21)? as usize;
        let total = u16_at(b, 25)? as usize;
        let wavebank_count = *b.get(27).ok_or("truncated")? as usize;
        let sound_count = u16_at(b, 28)? as usize;
        let simple_off = u32_at(b, 34)?;
        let complex_off = u32_at(b, 38)?;
        let wavebank_names = u32_at(b, 58)? as usize;
        let name_vals = u32_at(b, 66)? as usize;
        let sounds_off = u32_at(b, 70)? as usize;
        let name = cstr_at(b, 74)?;
        if complex_cues != 0 || complex_off != u32::MAX {
            return Err(format!("{complex_cues} complex cues: not parsed (this reader handles simple cues only)"));
        }
        let mut wavebanks = Vec::new();
        for i in 0..wavebank_count {
            wavebanks.push(cstr_at(b, wavebank_names + 64 * i)?);
        }
        // sounds: consecutive records, each starts with its own length at +7
        let mut sounds = Vec::new();
        let mut offsets = Vec::new();
        let mut o = sounds_off;
        for _ in 0..sound_count {
            offsets.push(o);
            let flags = *b.get(o).ok_or("truncated sound")?;
            let len = u16_at(b, o + 7)? as usize;
            let complex = flags & 1 != 0;
            let mut rpc_codes = Vec::new();
            let mut wave = None;
            if !complex {
                wave = Some((u16_at(b, o + 9)?, *b.get(o + 11).ok_or("truncated sound")?));
                if flags & 0x0E != 0 {
                    let mut r = o + 12 + 2;
                    if flags & 0x02 != 0 {
                        let n = *b.get(r).ok_or("truncated sound")? as usize;
                        r += 1;
                        for i in 0..n {
                            rpc_codes.push(u32_at(b, r + 4 * i)?);
                        }
                    }
                }
            }
            sounds.push(SoundDef { flags, category: u16_at(b, o + 1)?, volume_db: volume_byte_to_centi_db(*b.get(o + 3).ok_or("truncated sound")?) / 100.0, pitch: u16_at(b, o + 4)? as i16, priority: *b.get(o + 6).ok_or("truncated sound")?, wave, rpc_codes, complex });
            if len == 0 {
                return Err("sound record of length 0".into());
            }
            o += len;
        }
        // cues: simple cue table is `flags u8, sound offset u32`; names come with their cue index
        let mut cues: Vec<CueDef> = Vec::new();
        for i in 0..simple_cues {
            let r = simple_off as usize + 5 * i;
            let flags = *b.get(r).ok_or("truncated cue")?;
            let so = u32_at(b, r + 1)? as usize;
            let sound = offsets.iter().position(|x| *x == so);
            cues.push(CueDef { name: String::new(), flags, sound });
        }
        // the name table lists the cues in table order; the u16 after each name offset is a hash-chain link
        // (0xFFFF for most), not a cue index (checked: cue k -> sound -> wave k for all 104 names)
        for i in 0..total.min(cues.len()) {
            let name_off = u32_at(b, name_vals + 6 * i)? as usize;
            cues[i].name = cstr_at(b, name_off)?;
        }
        Ok(Xsb { name, wavebanks, cues, sounds })
    }

    pub fn cue_index(&self, name: &str) -> Option<usize> {
        self.cues.iter().position(|c| c.name == name)
    }
}
