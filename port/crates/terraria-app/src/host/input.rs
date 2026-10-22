//! Bevy input -> `terraria_host::input::InputSnapshot` (the only place where Bevy's input types meet XNA's).
//!
//! Two sources, one per run, never mixed (so no character and no key arrives twice):
//!
//! 1. **The window's own messages** (`shell/native.rs`, a hook on the shell's window; Windows, every run with a
//!    real window that is not scripted). Text is every `WM_CHAR` the window receives, keys are the virtual keys of
//!    the key messages: what the original reads (`WindowsIme`'s message filter, `GetKeyboardState`). The keyboard
//!    layout, Shift, AltGr, Caps Lock, dead keys, NumLock and input methods are the system's work.
//! 2. **Bevy's events** ([`collect`]; no hook: other systems, tests). Text is `KeyboardInput::text` of every
//!    press and repeat (winit: the characters the key produced with the layout and every modifier applied; a
//!    dead key has none and the key after it carries the combined character) plus `Ime::Commit`. Keys are
//!    derived per key press ([`virtual_key`]): numpad keys by what they produce (NumLock), a key that types a
//!    Latin letter is that letter's key (QWERTZ, AZERTY), `,` `.` `-` `+` are the keys Windows names after them,
//!    everything else is the key at that position of a US keyboard. The last rule is an approximation for the
//!    punctuation keys of other layouts (the hook has the real virtual key).
//!
//! Mouse: buttons and wheel from the window's messages (Bevy), the wheel accumulates 120 per line like Windows.
//! Position: the last `CursorMoved` of the window; it stays there when the cursor leaves the client area
//! (`CursorLeft` empties Bevy's `Window::cursor_position`; XNA's `Mouse.GetState` never jumps to 0,0). The shell
//! replaces it with the real pointer position, also outside the client area, when `native::mouse_source` says
//! so. Mouse messages posted to a window the real pointer is not over stay usable this way: Windows answers
//! winit's `TrackMouseEvent` with an immediate `WM_MOUSELEAVE` there.
//! Focus: `HostInputState::focused`, kept by `track_focus` from the window's `WindowFocused` messages (the shell
//! overrides it with XNA's `WM_ACTIVATEAPP` rule where the hook is installed).

use bevy::input::keyboard::{Key, KeyboardFocusLost, KeyboardInput};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::{CursorMoved, Ime, PrimaryWindow, WindowFocused};
use terraria_host::game::WindowState;
use terraria_host::input::{keys, InputSnapshot};

/// `Keys` value of a physical key, if XNA has one for it.
pub fn xna_key(code: KeyCode) -> Option<i32> {
    use KeyCode as K;
    Some(match code {
        K::KeyA => keys::A,
        K::KeyB => keys::B,
        K::KeyC => keys::C,
        K::KeyD => keys::D,
        K::KeyE => keys::E,
        K::KeyF => keys::F,
        K::KeyG => keys::G,
        K::KeyH => keys::H,
        K::KeyI => keys::I,
        K::KeyJ => keys::J,
        K::KeyK => keys::K,
        K::KeyL => keys::L,
        K::KeyM => keys::M,
        K::KeyN => keys::N,
        K::KeyO => keys::O,
        K::KeyP => keys::P,
        K::KeyQ => keys::Q,
        K::KeyR => keys::R,
        K::KeyS => keys::S,
        K::KeyT => keys::T,
        K::KeyU => keys::U,
        K::KeyV => keys::V,
        K::KeyW => keys::W,
        K::KeyX => keys::X,
        K::KeyY => keys::Y,
        K::KeyZ => keys::Z,
        K::Digit0 => keys::D0,
        K::Digit1 => keys::D1,
        K::Digit2 => keys::D2,
        K::Digit3 => keys::D3,
        K::Digit4 => keys::D4,
        K::Digit5 => keys::D5,
        K::Digit6 => keys::D6,
        K::Digit7 => keys::D7,
        K::Digit8 => keys::D8,
        K::Digit9 => keys::D9,
        K::F1 => keys::F1,
        K::F2 => keys::F2,
        K::F3 => keys::F3,
        K::F4 => keys::F4,
        K::F5 => keys::F5,
        K::F6 => keys::F6,
        K::F7 => keys::F7,
        K::F8 => keys::F8,
        K::F9 => keys::F9,
        K::F10 => keys::F10,
        K::F11 => keys::F11,
        K::F12 => keys::F12,
        K::Numpad0 => keys::NumPad0,
        K::Numpad1 => keys::NumPad1,
        K::Numpad2 => keys::NumPad2,
        K::Numpad3 => keys::NumPad3,
        K::Numpad4 => keys::NumPad4,
        K::Numpad5 => keys::NumPad5,
        K::Numpad6 => keys::NumPad6,
        K::Numpad7 => keys::NumPad7,
        K::Numpad8 => keys::NumPad8,
        K::Numpad9 => keys::NumPad9,
        K::NumpadAdd => keys::Add,
        K::NumpadSubtract => keys::Subtract,
        K::NumpadMultiply => keys::Multiply,
        K::NumpadDivide => keys::Divide,
        K::NumpadDecimal => keys::Decimal,
        K::NumpadEnter => keys::Enter,
        K::NumLock => keys::NumLock,
        K::Escape => keys::Escape,
        K::Enter => keys::Enter,
        K::Space => keys::Space,
        K::Tab => keys::Tab,
        K::Backspace => keys::Back,
        K::Delete => keys::Delete,
        K::Insert => keys::Insert,
        K::Home => keys::Home,
        K::End => keys::End,
        K::PageUp => keys::PageUp,
        K::PageDown => keys::PageDown,
        K::ArrowLeft => keys::Left,
        K::ArrowRight => keys::Right,
        K::ArrowUp => keys::Up,
        K::ArrowDown => keys::Down,
        K::ShiftLeft => keys::LeftShift,
        K::ShiftRight => keys::RightShift,
        K::ControlLeft => keys::LeftControl,
        K::ControlRight => keys::RightControl,
        K::AltLeft => keys::LeftAlt,
        K::AltRight => keys::RightAlt,
        K::SuperLeft => keys::LeftWindows,
        K::SuperRight => keys::RightWindows,
        K::ContextMenu => keys::Apps,
        K::CapsLock => keys::CapsLock,
        K::ScrollLock => keys::Scroll,
        K::Pause => keys::Pause,
        K::PrintScreen => keys::PrintScreen,
        K::Backquote => keys::OemTilde,
        K::Minus => keys::OemMinus,
        K::Equal => keys::OemPlus,
        K::BracketLeft => keys::OemOpenBrackets,
        K::BracketRight => keys::OemCloseBrackets,
        K::Backslash => keys::OemPipe,
        K::Semicolon => keys::OemSemicolon,
        K::Quote => keys::OemQuotes,
        K::Comma => keys::OemComma,
        K::Period => keys::OemPeriod,
        K::Slash => keys::OemQuestion,
        K::IntlBackslash => keys::OemBackslash,
        _ => return None,
    })
}

/// The `Keys` value of one key press as Bevy reports it (source 2 of the module docs).
pub fn virtual_key(code: KeyCode, logical: &Key) -> Option<i32> {
    use KeyCode as K;
    // the numpad: what the key does depends on NumLock, and Windows names the virtual key after what it does
    let numpad = matches!(code, K::Numpad0 | K::Numpad1 | K::Numpad2 | K::Numpad3 | K::Numpad4 | K::Numpad5 | K::Numpad6 | K::Numpad7 | K::Numpad8 | K::Numpad9 | K::NumpadDecimal);
    if numpad {
        let nav = match logical {
            Key::Insert => Some(keys::Insert),
            Key::End => Some(keys::End),
            Key::ArrowDown => Some(keys::Down),
            Key::PageDown => Some(keys::PageDown),
            Key::ArrowLeft => Some(keys::Left),
            // VK_CLEAR (12) is not a member of Keys: the 5 without NumLock is no key for XNA
            Key::Clear => return None,
            Key::ArrowRight => Some(keys::Right),
            Key::Home => Some(keys::Home),
            Key::ArrowUp => Some(keys::Up),
            Key::PageUp => Some(keys::PageUp),
            Key::Delete => Some(keys::Delete),
            _ => None,
        };
        return nav.or_else(|| xna_key(code));
    }
    if let Key::Character(c) = logical {
        let mut it = c.chars();
        if let (Some(ch), None) = (it.next(), it.next()) {
            // the letter keys: VK_A .. VK_Z are the letters the layout puts on the keys
            if ch.is_ascii_alphabetic() && !matches!(code, K::NumpadAdd | K::NumpadSubtract | K::NumpadMultiply | K::NumpadDivide) {
                return Some(keys::A + (ch.to_ascii_uppercase() as u8 - b'A') as i32);
            }
            // VK_OEM_COMMA, _PERIOD, _MINUS, _PLUS: named after the character on every layout
            let main_block = !matches!(code, K::NumpadAdd | K::NumpadSubtract | K::NumpadMultiply | K::NumpadDivide | K::NumpadDecimal | K::NumpadComma);
            if main_block {
                match ch {
                    ',' => return Some(keys::OemComma),
                    '.' => return Some(keys::OemPeriod),
                    '-' => return Some(keys::OemMinus),
                    '+' => return Some(keys::OemPlus),
                    _ => {}
                }
            }
        }
    }
    xna_key(code)
}

/// Accumulated across frames: the wheel value never resets in XNA.
#[derive(Resource, Default)]
pub struct HostInputState {
    pub scroll_wheel: i32,
    /// the last cursor position inside the client area, physical pixels (None: the cursor was never there)
    pub mouse: Option<(i32, i32)>,
    /// what the window system said last about the focus (`track_focus`); None: nothing yet, `Window::focused` counts
    pub focused: Option<bool>,
    /// the newest `CursorMoved` position (logical pixels) that `gather` has not taken yet
    pub mouse_event: Option<Vec2>,
    /// keys that are down, by physical key, with the `Keys` value each was pressed as ([`collect`])
    pub keys: Vec<(KeyCode, i32)>,
    /// characters typed since the last `take_text` ([`collect`])
    pub text: Vec<char>,
    /// what an input method is composing (`Ime::Preedit`); empty: nothing
    pub composition: String,
}

impl HostInputState {
    /// The `Keys` values that are down (source 2).
    pub fn keys_down(&self) -> Vec<i32> {
        let mut v: Vec<i32> = self.keys.iter().map(|(_, k)| *k).collect();
        v.sort_unstable();
        v.dedup();
        v
    }
    pub fn take_text(&mut self) -> Vec<char> {
        std::mem::take(&mut self.text)
    }
}

/// Keeps `HostInputState::focused` from the `WindowFocused` messages. With `no_activate` (`TERRARIA_NOACTIVATE`)
/// it also puts `Window::focused` back to false: bevy_winit answers a `Window::focused` that turned true with
/// `focus_window()` (winit: a synthetic Alt key through `SendInput` and `SetForegroundWindow`) whenever the window
/// is not the foreground window, which is exactly the case of a window that was told by messages that it has the
/// focus. Runs in `First`; bevy_winit compares in `Last`.
pub fn track_focus(state: &mut HostInputState, focus: &mut MessageReader<WindowFocused>, window: &mut Mut<Window>, no_activate: bool) {
    for e in focus.read() {
        if no_activate && state.focused != Some(e.focused) {
            eprintln!("[shell] window focus {} (a message of the window system)", if e.focused { "gained" } else { "lost" });
        }
        state.focused = Some(e.focused);
    }
    if no_activate {
        if state.focused.is_none() {
            state.focused = Some(false);
        }
        if window.focused {
            window.focused = false;
        }
    }
}

/// Source 2 of the module docs: Bevy's keyboard, text, wheel and cursor events into [`HostInputState`]. A system
/// of its own (it needs no window), run before the shell's tick; the shell uses its keys and text only when the
/// window has no message hook.
pub fn collect(
    mut state: ResMut<HostInputState>,
    mut wheel: MessageReader<MouseWheel>,
    mut typed: MessageReader<KeyboardInput>,
    mut ime: MessageReader<Ime>,
    mut lost: MessageReader<KeyboardFocusLost>,
    mut moved: MessageReader<CursorMoved>,
) {
    let state = &mut *state;
    for e in wheel.read() {
        // Windows: WHEEL_DELTA = 120 per line; pixel deltas (touch pads) are scaled as 1 line = 100 px... winit
        // reports lines for wheels, so only that case is exact
        state.scroll_wheel += match e.unit {
            MouseScrollUnit::Line => (e.y * 120.0).round() as i32,
            MouseScrollUnit::Pixel => (e.y * 1.2).round() as i32,
        };
    }
    for e in typed.read() {
        if e.state != ButtonState::Pressed {
            state.keys.retain(|(c, _)| *c != e.key_code);
            continue;
        }
        // the key: once per press (a repeat is the same key, still down)
        if !state.keys.iter().any(|(c, _)| *c == e.key_code) {
            if let Some(k) = virtual_key(e.key_code, &e.logical_key) {
                state.keys.push((e.key_code, k));
            }
        }
        // the text: every press and every repeat, as WM_CHAR. Where the window system gives no text for the
        // control characters a text field needs, they are made from the key
        match &e.text {
            Some(t) => state.text.extend(t.chars()),
            None => match &e.logical_key {
                Key::Enter => state.text.push('\r'),
                Key::Backspace => state.text.push('\u{8}'),
                Key::Tab => state.text.push('\t'),
                Key::Escape => state.text.push('\u{1b}'),
                _ => {}
            },
        }
    }
    for e in ime.read() {
        match e {
            // an input method's result: characters that no key press carries
            Ime::Commit { value, .. } => {
                state.text.extend(value.chars());
                state.composition.clear();
            }
            Ime::Preedit { value, .. } => value.clone_into(&mut state.composition),
            Ime::Disabled { .. } => state.composition.clear(),
            Ime::Enabled { .. } => {}
        }
    }
    if lost.read().next().is_some() {
        // the releases went to another window
        state.keys.clear();
    }
    for e in moved.read() {
        // logical, from the top left: `gather` scales it to physical pixels
        state.mouse_event = Some(e.position);
    }
}

/// Everything the host needs to know about input and the window for the frame that starts now, from
/// [`HostInputState`] (source 2: the shell replaces keys and text when it has the window's messages) and the window.
pub fn gather(state: &mut HostInputState, buttons: &ButtonInput<MouseButton>, window: &Window) -> (InputSnapshot, WindowState) {
    let scale = window.scale_factor();
    if let Some(p) = state.mouse_event.take() {
        state.mouse = Some(((p.x * scale).round() as i32, (p.y * scale).round() as i32));
    }
    if let Some(p) = window.physical_cursor_position() {
        state.mouse = Some((p.x.round() as i32, p.y.round() as i32));
    }
    let (mx, my) = state.mouse.unwrap_or((0, 0));
    let snapshot = InputSnapshot {
        keys_down: state.keys_down(),
        mouse_x: mx,
        mouse_y: my,
        scroll_wheel: state.scroll_wheel,
        left: buttons.pressed(MouseButton::Left),
        middle: buttons.pressed(MouseButton::Middle),
        right: buttons.pressed(MouseButton::Right),
        x_button1: buttons.pressed(MouseButton::Back),
        x_button2: buttons.pressed(MouseButton::Forward),
        text: state.take_text(),
        game_pads: [None; 4],
    };
    let (x, y) = match window.position {
        WindowPosition::At(p) => (p.x, p.y),
        _ => (0, 0),
    };
    let ws = WindowState {
        width: window.physical_width().max(1) as i32,
        height: window.physical_height().max(1) as i32,
        x,
        y,
        active: state.focused.unwrap_or(window.focused),
        handle: 0,
        // the monitor's size is not known here without a monitor query; the window's own size is reported until
        // the app passes the real display mode (host_demo does not need it)
        display_width: window.physical_width().max(1) as i32,
        display_height: window.physical_height().max(1) as i32,
        scale,
    };
    (snapshot, ws)
}

#[allow(dead_code)]
pub type PrimaryWindowQuery<'w, 's> = Single<'w, 's, &'static Window, With<PrimaryWindow>>;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::keyboard::NativeKey;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<KeyboardInput>()
            .add_message::<MouseWheel>()
            .add_message::<Ime>()
            .add_message::<KeyboardFocusLost>()
            .add_message::<CursorMoved>()
            .init_resource::<HostInputState>()
            .add_systems(Update, collect);
        app
    }

    fn key(app: &mut App, code: KeyCode, logical: Key, text: Option<&str>, state: ButtonState, repeat: bool) {
        let window = Entity::PLACEHOLDER;
        app.world_mut().write_message(KeyboardInput { key_code: code, logical_key: logical, state, text: text.map(Into::into), repeat, window });
    }
    fn press(app: &mut App, code: KeyCode, logical: Key, text: Option<&str>) {
        key(app, code, logical, text, ButtonState::Pressed, false);
    }
    fn release(app: &mut App, code: KeyCode, logical: Key) {
        key(app, code, logical, None, ButtonState::Released, false);
    }
    fn ch(s: &str) -> Key {
        Key::Character(s.into())
    }
    /// One frame, then what the host is given: (text, keys) as `gather` puts them into the `InputSnapshot`, read
    /// back through the host's `Keyboard.GetState`.
    fn frame(app: &mut App) -> (String, Vec<i32>) {
        app.update();
        let mut st = app.world_mut().resource_mut::<HostInputState>();
        let snapshot = InputSnapshot { keys_down: st.keys_down(), text: st.take_text(), ..Default::default() };
        let text: String = snapshot.text.iter().collect();
        terraria_host::input::set_snapshot(snapshot);
        (text, terraria_host::input::keyboard_get_state().get_pressed_keys().as_slice().to_vec())
    }

    /// Key events as winit writes them on Windows for each case (physical key, logical key, text), through the
    /// adapter in a headless `App`, read back from the host's `InputSnapshot` / `Keyboard.GetState`.
    #[test]
    fn keyboard_events_become_host_text_and_keys() {
        let mut app = app();
        // Shift + a: 'A'; keys LeftShift, A
        press(&mut app, KeyCode::ShiftLeft, Key::Shift, None);
        press(&mut app, KeyCode::KeyA, ch("A"), Some("A"));
        assert_eq!(frame(&mut app), ("A".to_owned(), vec![keys::A, keys::LeftShift]));
        // held: the character repeats, the key is down once
        key(&mut app, KeyCode::KeyA, ch("A"), Some("A"), ButtonState::Pressed, true);
        key(&mut app, KeyCode::KeyA, ch("A"), Some("A"), ButtonState::Pressed, true);
        assert_eq!(frame(&mut app), ("AA".to_owned(), vec![keys::A, keys::LeftShift]));
        release(&mut app, KeyCode::KeyA, ch("A"));
        // Shift + 1: '!'; the key is D1
        press(&mut app, KeyCode::Digit1, ch("!"), Some("!"));
        assert_eq!(frame(&mut app), ("!".to_owned(), vec![keys::D1, keys::LeftShift]));
        release(&mut app, KeyCode::Digit1, ch("!"));
        release(&mut app, KeyCode::ShiftLeft, Key::Shift);
        assert_eq!(frame(&mut app), (String::new(), vec![]));
        // AltGr + q on a German layout: '@'. Windows presses a left Control with the right Alt; the key is Q
        press(&mut app, KeyCode::ControlLeft, Key::Control, None);
        press(&mut app, KeyCode::AltRight, Key::AltGraph, None);
        press(&mut app, KeyCode::KeyQ, ch("@"), Some("@"));
        assert_eq!(frame(&mut app), ("@".to_owned(), vec![keys::Q, keys::LeftControl, keys::RightAlt]));
        release(&mut app, KeyCode::KeyQ, ch("@"));
        release(&mut app, KeyCode::AltRight, Key::AltGraph);
        release(&mut app, KeyCode::ControlLeft, Key::Control);
        // a dead key (acute; the key right of the German sharp s) and then e: nothing, then one composed character
        press(&mut app, KeyCode::Equal, Key::Dead(Some('\u{b4}')), None);
        assert_eq!(frame(&mut app).0, "");
        release(&mut app, KeyCode::Equal, Key::Dead(Some('\u{b4}')));
        press(&mut app, KeyCode::KeyE, ch("\u{e9}"), Some("\u{e9}"));
        assert_eq!(frame(&mut app), ("\u{e9}".to_owned(), vec![keys::E]));
        release(&mut app, KeyCode::KeyE, ch("\u{e9}"));
        // a dead key and a letter it does not combine with: both characters, in one press
        press(&mut app, KeyCode::KeyX, ch("x"), Some("\u{b4}x"));
        assert_eq!(frame(&mut app), ("\u{b4}x".to_owned(), vec![keys::X]));
        release(&mut app, KeyCode::KeyX, ch("x"));
        // a Cyrillic layout: the character is Cyrillic, the key is the one at that position (VK_Z)
        press(&mut app, KeyCode::KeyZ, ch("\u{44f}"), Some("\u{44f}"));
        assert_eq!(frame(&mut app), ("\u{44f}".to_owned(), vec![keys::Z]));
        release(&mut app, KeyCode::KeyZ, ch("\u{44f}"));
        // German layout: the key at the US Y position types z and IS Keys.Z; AZERTY: the key at the US Q position is A
        press(&mut app, KeyCode::KeyY, ch("z"), Some("z"));
        press(&mut app, KeyCode::KeyQ, ch("a"), Some("a"));
        assert_eq!(frame(&mut app), ("za".to_owned(), vec![keys::A, keys::Z]));
        release(&mut app, KeyCode::KeyY, ch("z"));
        release(&mut app, KeyCode::KeyQ, ch("a"));
        // the sharp s (German, at the US minus position): the character arrives; the key is the approximation
        // (US position: OemMinus; Windows says OemOpenBrackets there, which only the message hook knows)
        press(&mut app, KeyCode::Minus, ch("\u{df}"), Some("\u{df}"));
        assert_eq!(frame(&mut app), ("\u{df}".to_owned(), vec![keys::OemMinus]));
        release(&mut app, KeyCode::Minus, ch("\u{df}"));
        // the numpad with NumLock on: digits, NumPad keys
        press(&mut app, KeyCode::Numpad1, ch("1"), Some("1"));
        press(&mut app, KeyCode::Numpad5, ch("5"), Some("5"));
        press(&mut app, KeyCode::NumpadDecimal, ch("."), Some("."));
        assert_eq!(frame(&mut app), ("15.".to_owned(), vec![keys::NumPad1, keys::NumPad5, keys::Decimal]));
        release(&mut app, KeyCode::Numpad1, ch("1"));
        release(&mut app, KeyCode::Numpad5, ch("5"));
        release(&mut app, KeyCode::NumpadDecimal, ch("."));
        // ... and off: navigation keys, no text; the 5 is VK_CLEAR, which is no member of Keys
        press(&mut app, KeyCode::Numpad1, Key::End, None);
        press(&mut app, KeyCode::Numpad8, Key::ArrowUp, None);
        press(&mut app, KeyCode::Numpad5, Key::Clear, None);
        press(&mut app, KeyCode::NumpadDecimal, Key::Delete, None);
        assert_eq!(frame(&mut app), (String::new(), vec![keys::End, keys::Up, keys::Delete]));
        release(&mut app, KeyCode::Numpad1, Key::End);
        release(&mut app, KeyCode::Numpad8, Key::ArrowUp);
        release(&mut app, KeyCode::Numpad5, Key::Clear);
        release(&mut app, KeyCode::NumpadDecimal, Key::Delete);
        // both Enter keys: Keys.Enter and a carriage return each
        press(&mut app, KeyCode::Enter, Key::Enter, Some("\r"));
        assert_eq!(frame(&mut app), ("\r".to_owned(), vec![keys::Enter]));
        release(&mut app, KeyCode::Enter, Key::Enter);
        press(&mut app, KeyCode::NumpadEnter, Key::Enter, Some("\r"));
        assert_eq!(frame(&mut app), ("\r".to_owned(), vec![keys::Enter]));
        release(&mut app, KeyCode::NumpadEnter, Key::Enter);
        // Backspace / Tab / Escape with the text winit gives, and where a window system gives none
        press(&mut app, KeyCode::Backspace, Key::Backspace, Some("\u{8}"));
        press(&mut app, KeyCode::Tab, Key::Tab, None);
        press(&mut app, KeyCode::Escape, Key::Escape, None);
        assert_eq!(frame(&mut app), ("\u{8}\t\u{1b}".to_owned(), vec![keys::Back, keys::Tab, keys::Escape]));
        // an input method commits text no key carries; the composition string follows Preedit
        let window = Entity::PLACEHOLDER;
        app.world_mut().write_message(Ime::Preedit { window, value: "kan".into(), cursor: None });
        app.update();
        assert_eq!(app.world().resource::<HostInputState>().composition, "kan");
        app.world_mut().write_message(Ime::Commit { window, value: "\u{6f22}\u{5b57}".into() });
        let (text, _) = frame(&mut app);
        assert_eq!(text, "\u{6f22}\u{5b57}");
        assert!(app.world().resource::<HostInputState>().composition.is_empty());
        // the focus goes: every key is released (their key-up messages go elsewhere)
        app.world_mut().write_message(KeyboardFocusLost);
        assert_eq!(frame(&mut app), (String::new(), vec![]));
        // a key the shell has no Keys value for is no key
        press(&mut app, KeyCode::F24, Key::Unidentified(NativeKey::Unidentified), None);
        assert!(frame(&mut app).1.is_empty());
    }

    #[test]
    fn wheel_and_cursor_events() {
        let mut app = app();
        let window = Entity::PLACEHOLDER;
        app.world_mut().write_message(MouseWheel { unit: MouseScrollUnit::Line, x: 0.0, y: -2.0, window, phase: bevy::input::touch::TouchPhase::Moved });
        app.world_mut().write_message(CursorMoved { window, position: Vec2::new(400.4, 299.6), delta: None });
        app.update();
        let st = app.world().resource::<HostInputState>();
        assert_eq!((st.scroll_wheel, st.mouse_event), (-240, Some(Vec2::new(400.4, 299.6))));
    }
}
