//! `TERRARIA_INPUT_SCRIPT=<file>`: scripted input for hidden runs. The script feeds the same
//! `InputSnapshot` the real keyboard / mouse feed (nothing is injected into the OS), so input-driven behaviour of a
//! game can be tested without touching the real devices.
//!
//! One command per line, `#` starts a comment. `N` is the shell tick (1 = the first `begin_frame`; a scripted run
//! uses a fixed time step, so one tick is exactly one `Update`):
//!
//! ```text
//! frame N key W down          key W is held from tick N on          (names: A..Z, D0..D9, F1..F12, Escape, Space,
//! frame N key W up                                                    Enter, Tab, Back, Delete, Left, Right, Up, Down,
//! frame N key W press         down at tick N, up at tick N + 1        LeftShift, LeftControl, LeftAlt, ... or a number)
//! frame N mouse 400 300       the cursor moves to client pixel (400, 300)
//! frame N click left          button down at tick N, up at N + 1      (left, right, middle, x1, x2)
//! frame N button left down    button held / released                  (also: up)
//! frame N wheel 120           the cumulative wheel value changes by 120
//! frame N text hello          the characters arrive in tick N's snapshot ('\n' is Enter, '\b' is Back)
//! frame N resize 1024 600     the window is resized to 1024 x 600 client pixels
//! frame N shot [file.png]     a screenshot of the window is taken (to the file, or to TERRARIA_SCREENSHOT)
//! frame N wait S COND...     the script's clock stops at tick N until the game adapter says COND holds (a
//!                             condition of `debug_state::condition`: `ingame`, `players 2`, `chat hello` ...), for
//!                             at most S seconds of real time; a time-out is a failed run. For what another
//!                             process decides: a server that is still loading, the other player's actions
//! frame N loadshots F E C     armed from tick N on: while queued work is suspended (a world load, save or
//!                             generation: the script's clock stands still then) a screenshot is taken every E
//!                             such ticks, at most C of them, to F with `{}` replaced by 00, 01, ...; each one
//!                             also runs `dump ui` (the progress text goes to the state log)
//! frame N quit                the window is closed (Exiting is raised, then the app exits)
//! frame N pad connect [K]     a game pad appears (K = 1..4 names the pad in the script, default 1; it takes the
//! frame N pad disconnect [K]  first free XInput slot) / goes away
//! frame N pad button A down   a pad button is held / released / pressed for one tick. Names: A B X Y Back Start
//! frame N pad button A up     BigButton LeftShoulder RightShoulder LeftStick RightStick DPadUp DPadDown DPadLeft
//! frame N pad button A press  DPadRight
//! frame N pad stick left X Y  a stick (left / right) is at X, Y: each -1..1, Y up is positive; it stays there
//! frame N pad trigger right V a trigger (left / right) is at V: 0..1
//!                             The pad lines are the SAME `PadEvent`s the shell makes from Bevy's gamepad events
//!                             (`pad.rs`), applied to the same `terraria_host::input::PadAdapter`; append ` K` to
//!                             address another pad than 1
//! frame N dump|aim|game|set .. a command for the game adapter (`HostedGame::command`), run before tick N's Update;
//!                             the translated client's are in `terraria-gen-game/src/host/debug_state.rs` (included by the binaries)
//!                             (`dump player`, `dump tile X Y`, `aim tile X Y`, ...). `aim` moves the scripted cursor
//! ```

use terraria_host::input::{keys, xinput, InputSnapshot, PadAdapter, PadAxis, PadEvent};

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Key(i32, bool),
    KeyPress(i32),
    Mouse(i32, i32),
    Button(usize, bool),
    ButtonPress(usize),
    Wheel(i32),
    Text(String),
    Resize(u32, u32),
    /// `focus lost | gained`: the window stops / starts being the active one (`Game.IsActive`, `Deactivated`)
    Focus(bool),
    /// `minimise` / `restore`: a minimised window is inactive and draws nothing
    Minimise(bool),
    Shot(Option<String>),
    /// `loadshots <file with {}> <every> <count>`: screenshots while queued work is suspended
    LoadShots(String, u32, u32),
    /// `wait <seconds> <condition>`: the clock stops here until the condition holds
    Wait(u32, String),
    Quit,
    /// `dump ...`, `aim ...`, `game ...`, `set ...`: the whole command text, for the game adapter
    Command(String),
    /// `pad ...`: one change of scripted pad K (the id is 1000 + K)
    Pad(u64, PadEvent),
    /// `pad button NAME press`: down now, up at the next tick
    PadPress(u64, u32),
}

/// What a tick needs besides the input snapshot.
#[derive(Default, Debug, PartialEq)]
pub struct Effects {
    pub resize: Option<(u32, u32)>,
    pub shot: bool,
    /// `shot <file>`: where this screenshot goes (default: `TERRARIA_SCREENSHOT`)
    pub shot_path: Option<std::path::PathBuf>,
    pub quit: bool,
    /// `loadshots`: (file pattern, every that many suspended ticks, at most that many)
    pub load_shots: Option<(String, u32, u32)>,
    /// `wait`: (seconds at most, condition); the events after it are not consumed before the driver clears it
    pub wait: Option<(u32, String)>,
    /// game-adapter commands of this tick, in script order
    pub commands: Vec<String>,
}

pub struct Script {
    events: Vec<(u64, Action)>,
    next: usize,
    keys: Vec<i32>,
    mouse: (i32, i32),
    buttons: [bool; 5],
    focused: bool,
    minimised: bool,
    wheel: i32,
    /// released at the start of the next tick
    release_keys: Vec<i32>,
    release_buttons: Vec<usize>,
    pads: PadAdapter,
    release_pad_buttons: Vec<(u64, u32)>,
    pub last_tick: u64,
}

const PAD_BUTTONS: &[(&str, u32)] = &[
    ("a", xinput::A),
    ("b", xinput::B),
    ("x", xinput::X),
    ("y", xinput::Y),
    ("back", xinput::BACK),
    ("start", xinput::START),
    ("bigbutton", xinput::BIG_BUTTON),
    ("leftshoulder", xinput::LEFT_SHOULDER),
    ("rightshoulder", xinput::RIGHT_SHOULDER),
    ("leftstick", xinput::LEFT_THUMB),
    ("rightstick", xinput::RIGHT_THUMB),
    ("dpadup", xinput::DPAD_UP),
    ("dpaddown", xinput::DPAD_DOWN),
    ("dpadleft", xinput::DPAD_LEFT),
    ("dpadright", xinput::DPAD_RIGHT),
];

/// A `pad ...` line (the words after `pad`).
fn parse_pad(words: &[&str]) -> Result<Vec<Action>, String> {
    let float = |s: &str| s.parse::<f32>().map_err(|_| format!("not a number: {s:?}"));
    let id = |w: Option<&&str>| -> Result<u64, String> {
        match w {
            None => Ok(1001),
            Some(k) => k.parse::<u64>().ok().filter(|k| (1..=4).contains(k)).map(|k| 1000 + k).ok_or_else(|| format!("the pad number is 1..4, not {k:?}")),
        }
    };
    let side = |s: &str| match s {
        "left" => Ok(false),
        "right" => Ok(true),
        _ => Err(format!("`left` or `right` expected, not {s:?}")),
    };
    Ok(match words {
        ["connect", rest @ ..] if rest.len() <= 1 => vec![Action::Pad(id(rest.first())?, PadEvent::Connected)],
        ["disconnect", rest @ ..] if rest.len() <= 1 => vec![Action::Pad(id(rest.first())?, PadEvent::Disconnected)],
        ["button", name, state, rest @ ..] if rest.len() <= 1 => {
            let lower = name.to_ascii_lowercase();
            let bit = PAD_BUTTONS.iter().find(|(n, _)| *n == lower).map(|(_, b)| *b).ok_or_else(|| format!("unknown pad button {name:?}"))?;
            let id = id(rest.first())?;
            match *state {
                "down" => vec![Action::Pad(id, PadEvent::Button(bit, true))],
                "up" => vec![Action::Pad(id, PadEvent::Button(bit, false))],
                "press" => vec![Action::PadPress(id, bit)],
                _ => return Err("`down`, `up` or `press` expected".to_owned()),
            }
        }
        ["stick", which, x, y, rest @ ..] if rest.len() <= 1 => {
            let id = id(rest.first())?;
            let (ax, ay) = if side(which)? { (PadAxis::RightX, PadAxis::RightY) } else { (PadAxis::LeftX, PadAxis::LeftY) };
            vec![Action::Pad(id, PadEvent::Axis(ax, float(x)?)), Action::Pad(id, PadEvent::Axis(ay, float(y)?))]
        }
        ["trigger", which, v, rest @ ..] if rest.len() <= 1 => vec![Action::Pad(id(rest.first())?, PadEvent::Trigger(side(which)?, float(v)?))],
        _ => return Err("`pad connect|disconnect [K]`, `pad button NAME down|up|press [K]`, `pad stick left|right X Y [K]`, `pad trigger left|right V [K]`".to_owned()),
    })
}

const KEY_NAMES: &[(&str, i32)] = &[
    ("escape", keys::Escape),
    ("esc", keys::Escape),
    ("space", keys::Space),
    ("enter", keys::Enter),
    ("tab", keys::Tab),
    ("back", keys::Back),
    ("backspace", keys::Back),
    ("delete", keys::Delete),
    ("left", keys::Left),
    ("right", keys::Right),
    ("up", keys::Up),
    ("down", keys::Down),
    ("leftshift", keys::LeftShift),
    ("rightshift", keys::RightShift),
    ("leftcontrol", keys::LeftControl),
    ("rightcontrol", keys::RightControl),
    ("leftalt", keys::LeftAlt),
    ("rightalt", keys::RightAlt),
    ("home", keys::Home),
    ("end", keys::End),
    ("pageup", keys::PageUp),
    ("pagedown", keys::PageDown),
    ("insert", keys::Insert),
];

pub fn key_by_name(name: &str) -> Option<i32> {
    let lower = name.to_ascii_lowercase();
    if let Some(&(_, v)) = KEY_NAMES.iter().find(|(n, _)| *n == lower) {
        return Some(v);
    }
    let b = lower.as_bytes();
    if b.len() == 1 && b[0].is_ascii_lowercase() {
        return Some(keys::A + (b[0] - b'a') as i32);
    }
    if b.len() == 2 && b[0] == b'd' && b[1].is_ascii_digit() {
        return Some(keys::D0 + (b[1] - b'0') as i32);
    }
    if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse::<i32>().ok()) {
        if (1..=24).contains(&n) {
            return Some(keys::F1 + n - 1);
        }
    }
    lower.parse::<i32>().ok().filter(|v| (0..256).contains(v))
}

fn button_by_name(name: &str) -> Option<usize> {
    Some(match name.to_ascii_lowercase().as_str() {
        "left" => 0,
        "middle" => 1,
        "right" => 2,
        "x1" => 3,
        "x2" => 4,
        _ => return None,
    })
}

impl Script {
    pub fn parse(text: &str) -> Result<Script, String> {
        let mut events = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let err = |m: &str| format!("input script line {}: {m}: {raw:?}", i + 1);
            let (head, rest) = line.split_once(char::is_whitespace).ok_or_else(|| err("expected `frame N <command>`"))?;
            if head != "frame" {
                return Err(err("a line starts with `frame`"));
            }
            let (n, cmd) = rest.trim().split_once(char::is_whitespace).unwrap_or((rest.trim(), ""));
            let n: u64 = n.parse().map_err(|_| err("the frame number is not a number"))?;
            let cmd = cmd.trim();
            let (verb, args) = cmd.split_once(char::is_whitespace).unwrap_or((cmd, ""));
            let args = args.trim();
            let words: Vec<&str> = args.split_whitespace().collect();
            let num = |s: &str| s.parse::<i32>().map_err(|_| err("not a number"));
            let action = match verb {
                "key" => {
                    let [name, state] = words[..] else { return Err(err("`key <name> down|up|press`")) };
                    let k = key_by_name(name).ok_or_else(|| err("unknown key"))?;
                    match state {
                        "down" => Action::Key(k, true),
                        "up" => Action::Key(k, false),
                        "press" => Action::KeyPress(k),
                        _ => return Err(err("`down`, `up` or `press` expected")),
                    }
                }
                "mouse" => {
                    let [x, y] = words[..] else { return Err(err("`mouse X Y`")) };
                    Action::Mouse(num(x)?, num(y)?)
                }
                "click" => {
                    let [b] = words[..] else { return Err(err("`click left|right|middle|x1|x2`")) };
                    Action::ButtonPress(button_by_name(b).ok_or_else(|| err("unknown button"))?)
                }
                "button" => {
                    let [b, state] = words[..] else { return Err(err("`button <name> down|up`")) };
                    Action::Button(button_by_name(b).ok_or_else(|| err("unknown button"))?, state == "down")
                }
                "wheel" => {
                    let [d] = words[..] else { return Err(err("`wheel DELTA`")) };
                    Action::Wheel(num(d)?)
                }
                // the text is the rest of the line, spaces included
                "text" => Action::Text(args.replace("\\n", "\n").replace("\\b", "\u{8}")),
                "resize" => {
                    let [w, h] = words[..] else { return Err(err("`resize W H`")) };
                    Action::Resize(num(w)?.max(1) as u32, num(h)?.max(1) as u32)
                }
                "focus" => match words[..] {
                    ["lost"] => Action::Focus(false),
                    ["gained"] => Action::Focus(true),
                    _ => return Err(err("`focus lost|gained`")),
                },
                "minimise" | "minimize" => Action::Minimise(true),
                "restore" => Action::Minimise(false),
                "shot" => Action::Shot(if args.is_empty() { None } else { Some(args.to_owned()) }),
                "loadshots" => {
                    let [file, every, count] = words[..] else { return Err(err("`loadshots <file with {}> EVERY COUNT`")) };
                    if !file.contains("{}") {
                        return Err(err("the file name needs `{}` for the number"));
                    }
                    Action::LoadShots(file.to_owned(), num(every)?.max(1) as u32, num(count)?.max(0) as u32)
                }
                "wait" => {
                    let (secs, cond) = args.split_once(char::is_whitespace).ok_or_else(|| err("`wait SECONDS CONDITION`"))?;
                    Action::Wait(num(secs)?.max(1) as u32, cond.trim().to_owned())
                }
                "quit" => Action::Quit,
                "pad" => {
                    for a in parse_pad(&words).map_err(|m| err(&m))? {
                        events.push((n, a));
                    }
                    continue;
                }
                "dump" | "aim" | "game" | "set" => Action::Command(cmd.to_owned()),
                _ => return Err(err("unknown command")),
            };
            events.push((n, action));
        }
        // stable: commands of one tick keep the order they were written in
        events.sort_by_key(|(n, _)| *n);
        let last_tick = events.last().map_or(0, |(n, _)| *n);
        Ok(Script { events, next: 0, keys: Vec::new(), mouse: (0, 0), buttons: [false; 5], wheel: 0, release_keys: Vec::new(), release_buttons: Vec::new(), pads: PadAdapter::default(), release_pad_buttons: Vec::new(), last_tick, focused: true, minimised: false })
    }

    /// The scripted window: (has the focus, is minimised).
    pub fn window_state(&self) -> (bool, bool) {
        (self.focused, self.minimised)
    }

    /// `Mouse.SetPosition` from the game moves the scripted cursor.
    pub fn set_mouse(&mut self, x: i32, y: i32) {
        self.mouse = (x, y);
    }

    /// The scripted cursor (client pixels).
    pub fn mouse(&self) -> (i32, i32) {
        self.mouse
    }

    /// The input of tick `tick`.
    pub fn step(&mut self, tick: u64) -> (InputSnapshot, Effects) {
        for k in std::mem::take(&mut self.release_keys) {
            self.keys.retain(|x| *x != k);
        }
        for b in std::mem::take(&mut self.release_buttons) {
            self.buttons[b] = false;
        }
        for (id, bit) in std::mem::take(&mut self.release_pad_buttons) {
            self.pads.apply(id, PadEvent::Button(bit, false));
        }
        let mut text = Vec::new();
        let mut fx = Effects::default();
        while self.next < self.events.len() && self.events[self.next].0 <= tick {
            match self.events[self.next].1.clone() {
                Action::Key(k, true) => {
                    if !self.keys.contains(&k) {
                        self.keys.push(k);
                    }
                }
                Action::Key(k, false) => self.keys.retain(|x| *x != k),
                Action::KeyPress(k) => {
                    if !self.keys.contains(&k) {
                        self.keys.push(k);
                    }
                    self.release_keys.push(k);
                }
                Action::Mouse(x, y) => self.mouse = (x, y),
                Action::Button(b, down) => self.buttons[b] = down,
                Action::ButtonPress(b) => {
                    self.buttons[b] = true;
                    self.release_buttons.push(b);
                }
                Action::Wheel(d) => self.wheel += d,
                Action::Text(t) => text.extend(t.chars().map(|c| if c == '\n' { '\r' } else { c })),
                Action::Resize(w, h) => fx.resize = Some((w, h)),
                Action::Focus(f) => self.focused = f,
                Action::Minimise(m) => self.minimised = m,
                Action::Shot(p) => {
                    fx.shot = true;
                    fx.shot_path = p.as_ref().map(std::path::PathBuf::from);
                }
                Action::LoadShots(f, e, c) => fx.load_shots = Some((f, e, c)),
                Action::Wait(secs, cond) => {
                    // nothing after the wait happens in this tick: the driver holds the clock here
                    fx.wait = Some((secs, cond));
                    self.next += 1;
                    break;
                }
                Action::Quit => fx.quit = true,
                Action::Command(c) => fx.commands.push(c),
                Action::Pad(id, e) => self.pads.apply(id, e),
                Action::PadPress(id, bit) => {
                    self.pads.apply(id, PadEvent::Button(bit, true));
                    self.release_pad_buttons.push((id, bit));
                }
            }
            self.next += 1;
        }
        let snapshot = InputSnapshot {
            keys_down: self.keys.clone(),
            mouse_x: self.mouse.0,
            mouse_y: self.mouse.1,
            scroll_wheel: self.wheel,
            left: self.buttons[0],
            middle: self.buttons[1],
            right: self.buttons[2],
            x_button1: self.buttons[3],
            x_button2: self.buttons[4],
            text,
            game_pads: self.pads.snapshot(),
        };
        (snapshot, fx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_steps() {
        let mut s = Script::parse("# c\nframe 2 key W down\nframe 3 mouse 400 300\nframe 3 click left\nframe 4 text hello world\nframe 5 resize 1024 600\nframe 6 key F12 press\nframe 7 quit\nframe 5 key W up\n").unwrap();
        let (a, _) = s.step(1);
        assert!(a.keys_down.is_empty());
        let (a, _) = s.step(2);
        assert_eq!(a.keys_down, [keys::W]);
        let (a, _) = s.step(3);
        assert_eq!((a.mouse_x, a.mouse_y, a.left), (400, 300, true));
        let (a, _) = s.step(4);
        assert!(!a.left);
        assert_eq!(a.text.iter().collect::<String>(), "hello world");
        let (a, fx) = s.step(5);
        assert_eq!(fx.resize, Some((1024, 600)));
        assert!(a.keys_down.is_empty());
        let (a, _) = s.step(6);
        assert_eq!(a.keys_down, [keys::F12]);
        let (a, fx) = s.step(7);
        assert!(a.keys_down.is_empty() && fx.quit);
        let mut s = Script::parse("frame 1 dump tile 10 20\nframe 1 aim rel 1 0\nframe 2 set time 0 night\n").unwrap();
        let (_, fx) = s.step(1);
        assert_eq!(fx.commands, ["dump tile 10 20", "aim rel 1 0"]);
        assert_eq!(s.step(2).1.commands, ["set time 0 night"]);
        s.set_mouse(7, 9);
        assert_eq!(s.mouse(), (7, 9));
        let mut s = Script::parse("frame 3 loadshots a_{}.png 50 4
").unwrap();
        assert_eq!(s.step(3).1.load_shots, Some(("a_{}.png".to_owned(), 50, 4)));
        assert!(Script::parse("frame 3 loadshots a.png 50 4").is_err());
        let mut s = Script::parse("frame 2 wait 30 players 2\nframe 2 key W down\nframe 3 quit\n").unwrap();
        let (a, fx) = s.step(2);
        assert_eq!(fx.wait, Some((30, "players 2".to_owned())));
        assert!(a.keys_down.is_empty(), "what follows the wait in the same tick comes when the clock moves again");
        assert_eq!(s.step(2).0.keys_down, [keys::W]);
        assert!(Script::parse("frame 2 wait soon ingame").is_err());
        assert!(Script::parse("frame x key W down").is_err());
        // game pads: the lines are pad events for the same adapter the shell feeds from Bevy
        let mut s = Script::parse("frame 1 pad connect\nframe 2 pad button A press\nframe 2 pad stick left 0.5 -1\nframe 3 pad trigger right 1\nframe 4 pad button DPadUp down\nframe 4 pad connect 2\nframe 4 pad button B down 2\nframe 5 pad disconnect\n").unwrap();
        assert!(s.step(0).0.game_pads.iter().all(Option::is_none));
        let p = s.step(1).0.game_pads[0].expect("connected");
        assert_eq!((p.buttons, p.thumb_lx, p.left_trigger), (0, 0, 0));
        let p = s.step(2).0.game_pads[0].unwrap();
        assert_eq!((p.buttons, p.thumb_lx, p.thumb_ly), (0x1000, 16384, -32767));
        let p = s.step(3).0.game_pads[0].unwrap();
        assert_eq!((p.buttons, p.right_trigger, p.thumb_lx), (0, 255, 16384), "the press ended, the stick stays");
        let pads = s.step(4).0.game_pads;
        assert_eq!((pads[0].unwrap().buttons, pads[1].unwrap().buttons), (1, 0x2000));
        let pads = s.step(5).0.game_pads;
        assert!(pads[0].is_none() && pads[1].is_some());
        assert!(Script::parse("frame 1 pad button Nope down").is_err());
        assert!(Script::parse("frame 1 pad stick up 0 0").is_err());
        assert!(Script::parse("frame 1 pad connect 9").is_err());
        assert!(Script::parse("frame 1 key Nope down").is_err());
    }
}
