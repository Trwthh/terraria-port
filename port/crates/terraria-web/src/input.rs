//! The page's input records (web/www/HOST_ABI.md section 4, 32 bytes each) -> the host's [`InputSnapshot`], with the
//! translations the native shell gets from Windows: XNA `Keys` are Windows virtual-key codes, the text is every
//! WM_CHAR (control characters of Backspace / Enter / Tab / Escape and Ctrl+letter included), the wheel counts 120
//! per notch with the opposite sign of the DOM's, game pads are XInput's raw state (no dead zone: the host applies
//! XNA's).

use std::collections::BTreeSet;

use terraria_host::input::{GamePadInput, InputSnapshot};

/// Windows virtual-key code of each key code id (HOST_ABI.md "Key code ids"); 0: no XNA key.
const VK: [u8; 136] = [
    0, // Unidentified
    0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, // KeyA..KeyZ
    0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, // Digit0..9
    0x0D, 0x1B, 0x08, 0x09, 0x20, // Enter Escape Backspace Tab Space
    0xBD, 0xBB, 0xDB, 0xDD, 0xDC, 0xBA, 0xDE, 0xC0, 0xBC, 0xBE, 0xBF, // Minus Equal [ ] \ ; ' ` , . /
    0x14, // CapsLock
    0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7B, // F1..F12
    0x2C, 0x91, 0x13, 0x2D, 0x24, 0x21, 0x2E, 0x23, 0x22, // PrintScreen ScrollLock Pause Insert Home PageUp Delete End PageDown
    0x27, 0x25, 0x28, 0x26, // ArrowRight Left Down Up
    0x90, 0x6F, 0x6A, 0x6D, 0x6B, 0x0D, // NumLock NumpadDivide Multiply Subtract Add NumpadEnter (Keys.Enter, as Windows)
    0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x60, 0x6E, // Numpad1..9, Numpad0, NumpadDecimal
    0xE2, 0x5D, 0x00, // IntlBackslash (OEM_102) ContextMenu (Apps) NumpadEqual
    0x7C, 0x7D, 0x7E, 0x7F, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, // F13..F24
    0xA2, 0xA0, 0xA4, 0x5B, 0xA3, 0xA1, 0xA5, 0x5C, // ControlLeft ShiftLeft AltLeft MetaLeft ControlRight ShiftRight AltRight MetaRight
    0xC1, 0x00, 0x15, 0x15, 0x19, 0x1C, 0x1D, // IntlRo IntlYen KanaMode Lang1 Lang2 Convert NonConvert
    0xAD, 0xAE, 0xAF, 0xB0, 0xB1, 0xB2, 0xB3, 0xA6, 0xA7, // volume, media, browser back / forward
    0x6C, // NumpadComma (VK_SEPARATOR)
];

/// With NumLock off a numpad digit key is the navigation key printed on it (what Windows reports).
fn numpad_navigation(code: u32) -> Option<u8> {
    Some(match code {
        85 => 0x23, // 1 End
        86 => 0x28, // 2 Down
        87 => 0x22, // 3 PageDown
        88 => 0x25, // 4 Left
        89 => 0x0C, // 5 Clear
        90 => 0x27, // 6 Right
        91 => 0x24, // 7 Home
        92 => 0x26, // 8 Up
        93 => 0x21, // 9 PageUp
        94 => 0x2D, // 0 Insert
        95 => 0x2E, // . Delete
        _ => return None,
    })
}

/// XInput button bits of the standard Gamepad API mapping (buttons 0..16).
const PAD: [u32; 17] = [0x1000, 0x2000, 0x4000, 0x8000, 0x0100, 0x0200, 0, 0, 0x0020, 0x0010, 0x0040, 0x0080, 0x0001, 0x0002, 0x0004, 0x0008, 0x0800];

#[derive(Default, Clone, Copy)]
struct Pad {
    connected: bool,
    axes: [f32; 4],
    pressed: u32,
    triggers: (f32, f32),
    packet: u32,
}

/// The input state the records have built so far.
#[derive(Default)]
pub struct Input {
    /// physical keys down (code id) and the virtual key each one reported when it went down
    down: std::collections::BTreeMap<u32, u8>,
    mouse: (i32, i32),
    buttons: u32,
    wheel: f64,
    text: Vec<char>,
    pub active: bool,
    /// canvas size in device pixels (RESIZE)
    pub size: Option<(i32, i32)>,
    pads: [Pad; 4],
    pub dropped: u64,
}

impl Input {
    pub fn new() -> Input {
        Input { active: true, ..Input::default() }
    }

    /// Applies one 32-byte record.
    pub fn apply(&mut self, r: &[u8]) {
        let u32_at = |o: usize| u32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let f32_at = |o: usize| f32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let (kind, state, mods, a) = (r[0], r[1], u16::from_le_bytes([r[2], r[3]]), u32_at(4));
        let (ctrl, alt) = (mods & 2 != 0, mods & 4 != 0);
        match kind {
            1 => {
                let numlock = mods & 32 != 0;
                let vk = if !numlock { numpad_navigation(a) } else { None }.or_else(|| VK.get(a as usize).copied()).unwrap_or(0);
                if state == 1 {
                    if vk != 0 {
                        self.down.insert(a, vk);
                    }
                    // the WM_CHAR a key press produces that is no TEXT record (HOST_ABI: Enter, Backspace, Tab ...)
                    let c = match a {
                        39 => Some('\u{8}'),
                        37 | 84 => Some('\r'),
                        40 if !alt => Some('\t'),
                        38 => Some('\u{1b}'),
                        1..=26 if ctrl && !alt => char::from_u32(a), // Ctrl+A .. Ctrl+Z: 0x01 .. 0x1A
                        _ => None,
                    };
                    if let Some(c) = c {
                        self.text.push(c);
                    }
                } else {
                    self.down.remove(&a);
                }
            }
            2 => {
                let n = (a as usize).min(16);
                if let Ok(s) = std::str::from_utf8(&r[16..16 + n]) {
                    // a character outside the BMP is two WM_CHARs (a surrogate pair) on Windows
                    for c in s.chars() {
                        let mut buf = [0u16; 2];
                        for unit in c.encode_utf16(&mut buf) {
                            self.text.push(char::from_u32(*unit as u32).unwrap_or('\u{fffd}'));
                        }
                    }
                }
            }
            3 => {
                self.mouse = (f32_at(16).floor() as i32, f32_at(20).floor() as i32);
                self.buttons = u32_at(24);
            }
            4 => {
                self.mouse = (f32_at(16).floor() as i32, f32_at(20).floor() as i32);
                self.buttons = u32_at(24);
            }
            5 => {
                // CSS pixels down -> XNA units up: Chrome on Windows scrolls 100 px per notch, XNA counts 120
                self.wheel -= f32_at(24) as f64 * 1.2;
            }
            6 => {
                self.active = state == 1;
                if !self.active {
                    // keys held while the page lost focus never send their key up
                    self.down.clear();
                    self.buttons = 0;
                }
            }
            7 => self.size = Some((a as i32, u32_at(28) as i32)),
            8 => {
                if let Some(p) = self.pads.get_mut(a as usize) {
                    *p = Pad { connected: state == 1, ..Pad::default() };
                }
            }
            9 => {
                if let Some(p) = self.pads.get_mut(a as usize) {
                    p.axes = [f32_at(16), f32_at(20), f32_at(24), f32_at(28)];
                    p.packet = p.packet.wrapping_add(1);
                }
            }
            10 => {
                if let Some(p) = self.pads.get_mut(a as usize) {
                    p.pressed = u32_at(16);
                    p.triggers = (f32_at(20), f32_at(24));
                    p.packet = p.packet.wrapping_add(1);
                }
            }
            15 => self.dropped += a as u64,
            _ => {}
        }
    }

    /// The snapshot for this frame; the typed text is handed over once.
    pub fn snapshot(&mut self) -> InputSnapshot {
        let keys: BTreeSet<i32> = self.down.values().map(|v| *v as i32).collect();
        let mut pads = [None; 4];
        for (i, p) in self.pads.iter().enumerate() {
            if p.connected {
                let mut buttons = 0;
                for (bit, mask) in PAD.iter().enumerate() {
                    if p.pressed >> bit & 1 == 1 {
                        buttons |= mask;
                    }
                }
                // DOM axes: y down positive; XInput: up positive
                let mut g = GamePadInput::from_normalized(buttons, (p.axes[0], -p.axes[1]), (p.axes[2], -p.axes[3]), p.triggers.0, p.triggers.1);
                g.packet = p.packet;
                pads[i] = Some(g);
            }
        }
        InputSnapshot {
            keys_down: keys.into_iter().collect(),
            mouse_x: self.mouse.0,
            mouse_y: self.mouse.1,
            scroll_wheel: self.wheel.round() as i32,
            left: self.buttons & 1 != 0,
            right: self.buttons & 2 != 0,
            middle: self.buttons & 4 != 0,
            x_button1: self.buttons & 8 != 0,
            x_button2: self.buttons & 16 != 0,
            text: std::mem::take(&mut self.text),
            game_pads: pads,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(kind: u8, state: u8, mods: u16, a: u32, s: [u32; 4]) -> [u8; 32] {
        let mut r = [0u8; 32];
        r[0] = kind;
        r[1] = state;
        r[2..4].copy_from_slice(&mods.to_le_bytes());
        r[4..8].copy_from_slice(&a.to_le_bytes());
        for (i, v) in s.iter().enumerate() {
            r[16 + i * 4..20 + i * 4].copy_from_slice(&v.to_le_bytes());
        }
        r
    }

    #[test]
    fn keys_text_wheel_and_pads() {
        assert_eq!(VK.len(), 136);
        let mut i = Input::new();
        i.apply(&rec(1, 1, 0, 1, [0; 4])); // KeyA down
        i.apply(&rec(1, 1, 32, 85, [0; 4])); // Numpad1 with NumLock on
        i.apply(&rec(1, 1, 0, 39, [0; 4])); // Backspace
        let mut t = rec(2, 0, 0, 3, [0; 4]);
        t[16..19].copy_from_slice("hé".as_bytes());
        i.apply(&t);
        i.apply(&rec(5, 0, 0, 0, [0, 0, 100f32.to_bits(), 0])); // one notch down
        i.apply(&rec(4, 1, 0, 0, [10.5f32.to_bits(), 20f32.to_bits(), 1, 0]));
        i.apply(&rec(8, 1, 0, 0, [1, 4, 17, 0]));
        i.apply(&rec(9, 0, 0, 0, [0, (-1f32).to_bits(), 0, 0]));
        i.apply(&rec(10, 0, 0, 0, [1 | (1 << 12), 0.5f32.to_bits(), 0, 0]));
        let s = i.snapshot();
        assert_eq!(s.keys_down, vec![0x08, 0x41, 0x61]); // Backspace is still held
        assert_eq!(s.text, vec!['\u{8}', 'h', 'é']);
        assert_eq!(s.scroll_wheel, -120);
        assert_eq!((s.mouse_x, s.mouse_y, s.left), (10, 20, true));
        let p = s.game_pads[0].unwrap();
        assert_eq!((p.buttons, p.thumb_ly, p.left_trigger), (0x1000 | 0x0001, 32767, 128));
        assert!(i.snapshot().text.is_empty());
        // NumLock off: the numpad 1 key is End; focus loss releases everything
        i.apply(&rec(1, 1, 0, 85, [0; 4]));
        assert!(i.snapshot().keys_down.contains(&0x23));
        i.apply(&rec(6, 0, 0, 0, [0; 4]));
        assert!(i.snapshot().keys_down.is_empty());
    }
}
