//! `Microsoft.Xna.Framework.Input`: `Keyboard`, `Mouse`, `GamePad`, and text input, all fed from one plain
//! [`InputSnapshot`] that the app fills once per frame (README.md, driver protocol). No OS input API is touched
//! here.
//!
//! `KeyboardState`: 256 key bits, as XNA (reference/xna/KeyboardState.cs); `GetPressedKeys` lists them in
//! ascending key code. `MouseState`: position in window client pixels, cumulative wheel value (120 per notch on
//! Windows), five buttons. Game pads report "not connected" unless the snapshot carries one.

use std::cell::RefCell;

use terraria_core::xna::Vector2;
use terraria_gen_rt::rt::Arr;

use crate::gfx::states::plain_trace;
pub use crate::input_keys::keys;

/// What the app knows about input at the start of a frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputSnapshot {
    /// `Keys` values (Windows virtual-key codes) that are down
    pub keys_down: Vec<i32>,
    /// cursor position in client pixels of the game window
    pub mouse_x: i32,
    pub mouse_y: i32,
    /// cumulative scroll wheel value (XNA: 120 per notch, never reset)
    pub scroll_wheel: i32,
    pub left: bool,
    pub middle: bool,
    pub right: bool,
    pub x_button1: bool,
    pub x_button2: bool,
    /// characters typed since the last frame (WM_CHAR order; control characters such as `\b` and `\r` included)
    pub text: Vec<char>,
    /// connected game pads by player index 0..3
    pub game_pads: [Option<GamePadInput>; 4],
}

/// One connected game pad as XInput reports it (`XINPUT_GAMEPAD`, reference/xna/XINPUT_GAMEPAD.cs): the raw values
/// XNA's `GamePad.GetState` starts from. Dead zones and scaling are applied by [`game_pad_get_state_with`], as
/// XNA applies them (reference/xna/GamePadDeadZoneUtils.cs).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GamePadInput {
    /// `wButtons` (reference/xna/ButtonValues.cs: Up 1, Down 2, Left 4, Right 8, Start 0x10, Back 0x20, LeftThumb
    /// 0x40, RightThumb 0x80, LeftShoulder 0x100, RightShoulder 0x200, BigButton 0x800, A 0x1000, B 0x2000,
    /// X 0x4000, Y 0x8000); the low 16 bits count
    pub buttons: u32,
    /// `bLeftTrigger` / `bRightTrigger`: 0..255
    pub left_trigger: u8,
    pub right_trigger: u8,
    /// `sThumbLX` .. `sThumbRY`: -32768..32767, Y up is positive
    pub thumb_lx: i16,
    pub thumb_ly: i16,
    pub thumb_rx: i16,
    pub thumb_ry: i16,
    /// `dwPacketNumber`: changes whenever the state changed
    pub packet: u32,
}

impl GamePadInput {
    /// From normalised values (sticks -1..1 with Y up, triggers 0..1), as an input library that does not hand
    /// out the raw XInput numbers reports them: `round(v * 32767)` and `round(v * 255)`, clamped. XInput's own
    /// -32768 is not reachable this way (it becomes -32767; XNA's dead-zone code clamps both to -1).
    pub fn from_normalized(buttons: u32, left_stick: (f32, f32), right_stick: (f32, f32), left_trigger: f32, right_trigger: f32) -> GamePadInput {
        let axis = |v: f32| (v.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        let trigger = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        GamePadInput { buttons, left_trigger: trigger(left_trigger), right_trigger: trigger(right_trigger), thumb_lx: axis(left_stick.0), thumb_ly: axis(left_stick.1), thumb_rx: axis(right_stick.0), thumb_ry: axis(right_stick.1), packet: 0 }
    }
}

thread_local! {
    static CURRENT: RefCell<InputSnapshot> = RefCell::new(InputSnapshot::default());
    /// `Mouse.SetPosition` requests since the last `take_mouse_position_request`.
    static SET_POSITION: RefCell<Option<(i32, i32)>> = const { RefCell::new(None) };
    /// `GamePad.SetVibration` requests since the last `take_vibration_requests`, per player index.
    static VIBRATION: RefCell<[Option<(f32, f32)>; 4]> = const { RefCell::new([None; 4]) };
}

/// Driver: the input for the frame that starts now.
pub fn set_snapshot(s: InputSnapshot) {
    CURRENT.with(|c| *c.borrow_mut() = s);
}
/// The snapshot of the current frame.
pub fn snapshot() -> InputSnapshot {
    CURRENT.with(|c| c.borrow().clone())
}

// ------------------------------------------------------------------------------------------------ keyboard

/// `KeyboardState` (a struct): one bit per key code 0..255.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyboardState {
    pub bits: [u32; 8],
}
plain_trace!(KeyboardState, MouseState, GamePadState, GamePadThumbSticks, GamePadTriggers);

impl KeyboardState {
    /// `new KeyboardState(params Keys[] keys)`, and what `Keyboard.GetState` builds from the 256 key states: a
    /// value that is not a member of the `Keys` enum is dropped (`AddPressedKey` masks with `stateMask0..7`, the
    /// set of all enum members: reference/xna/KeyboardState.cs:82-149). That is why the mouse buttons (1, 2, 4)
    /// and the side-less Shift / Control / Alt (16, 17, 18) of `GetKeyboardState` never show up as pressed keys.
    pub fn from_keys(keys: &[i32]) -> KeyboardState {
        fn mask() -> &'static [u32; 8] {
            static MASK: std::sync::OnceLock<[u32; 8]> = std::sync::OnceLock::new();
            MASK.get_or_init(|| {
                let mut m = [0u32; 8];
                for (_, v) in keys::ALL {
                    if (0..256).contains(v) {
                        m[(*v >> 5) as usize] |= 1 << (*v & 31);
                    }
                }
                m
            })
        }
        let mut s = KeyboardState::default();
        for k in keys {
            if (0..256).contains(k) {
                s.bits[(*k >> 5) as usize] |= (1 << (*k & 31)) & mask()[(*k >> 5) as usize];
            }
        }
        s
    }
    /// `IsKeyDown(key)`: false for a value outside 0..255.
    pub fn is_key_down(&self, key: i32) -> bool {
        (0..256).contains(&key) && self.bits[(key >> 5) as usize] & (1 << (key & 31)) != 0
    }
    pub fn is_key_up(&self, key: i32) -> bool {
        !self.is_key_down(key)
    }
    /// `this[key]`: `KeyState.Down` 1 / `KeyState.Up` 0.
    pub fn key_state(&self, key: i32) -> i32 {
        self.is_key_down(key) as i32
    }
    /// `GetPressedKeys()`: ascending key codes.
    pub fn get_pressed_keys(&self) -> Arr<i32> {
        let v: Vec<i32> = (0..256).filter(|k| self.is_key_down(*k)).collect();
        Arr::from_slice(&v)
    }
}

/// `Keyboard.GetState()`
pub fn keyboard_get_state() -> KeyboardState {
    CURRENT.with(|c| KeyboardState::from_keys(&c.borrow().keys_down))
}

// ------------------------------------------------------------------------------------------------ mouse

/// `MouseState` (a struct). Buttons are `ButtonState`: Released 0, Pressed 1.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseState {
    pub x: i32,
    pub y: i32,
    pub scroll_wheel_value: i32,
    pub left_button: i32,
    pub middle_button: i32,
    pub right_button: i32,
    pub x_button1: i32,
    pub x_button2: i32,
}

impl MouseState {
    /// `new MouseState(x, y, scrollWheel, leftButton, middleButton, rightButton, xButton1, xButton2)`
    #[allow(clippy::too_many_arguments)]
    pub fn new(x: i32, y: i32, scroll_wheel: i32, left: i32, middle: i32, right: i32, x_button1: i32, x_button2: i32) -> MouseState {
        MouseState { x, y, scroll_wheel_value: scroll_wheel, left_button: left, middle_button: middle, right_button: right, x_button1, x_button2 }
    }
}

/// `Mouse.GetState()`
pub fn mouse_get_state() -> MouseState {
    CURRENT.with(|c| {
        let s = c.borrow();
        MouseState { x: s.mouse_x, y: s.mouse_y, scroll_wheel_value: s.scroll_wheel, left_button: s.left as i32, middle_button: s.middle as i32, right_button: s.right as i32, x_button1: s.x_button1 as i32, x_button2: s.x_button2 as i32 }
    })
}

/// `Mouse.SetPosition(x, y)`: recorded for the app ([`take_mouse_position_request`]) and applied to the current
/// snapshot so a `GetState` in the same frame sees it, as on Windows. Whether the real pointer is moved is the
/// app's decision (the development app never moves it).
pub fn mouse_set_position(x: i32, y: i32) {
    SET_POSITION.with(|p| *p.borrow_mut() = Some((x, y)));
    CURRENT.with(|c| {
        let mut s = c.borrow_mut();
        s.mouse_x = x;
        s.mouse_y = y;
    });
}
pub fn take_mouse_position_request() -> Option<(i32, i32)> {
    SET_POSITION.with(|p| p.borrow_mut().take())
}

// ------------------------------------------------------------------------------------------------ game pad

/// `GamePadDeadZone`
pub const DEAD_ZONE_NONE: i32 = 0;
pub const DEAD_ZONE_INDEPENDENT_AXES: i32 = 1;
pub const DEAD_ZONE_CIRCULAR: i32 = 2;

/// `GamePadDeadZoneUtils` (reference/xna/GamePadDeadZoneUtils.cs), line for line.
pub mod dead_zone {
    use super::*;

    pub const LEFT_STICK: f32 = 7849.0;
    pub const RIGHT_STICK: f32 = 8689.0;
    pub const TRIGGER: f32 = 30.0;

    /// `ApplyLinearDeadZone(value, maxValue, deadZoneSize)`
    pub fn linear(mut value: f32, max_value: f32, dead_zone_size: f32) -> f32 {
        if value < 0.0 - dead_zone_size {
            value += dead_zone_size;
        } else {
            if !(value > dead_zone_size) {
                return 0.0;
            }
            value -= dead_zone_size;
        }
        (value / (max_value - dead_zone_size)).clamp(-1.0, 1.0)
    }

    /// `ApplyStickDeadZone(x, y, deadZoneMode, deadZoneSize)`
    pub fn stick(x: i32, y: i32, mode: i32, dead_zone_size: f32) -> Vector2 {
        match mode {
            DEAD_ZONE_INDEPENDENT_AXES => Vector2 { x: linear(x as f32, 32767.0, dead_zone_size), y: linear(y as f32, 32767.0, dead_zone_size) },
            DEAD_ZONE_CIRCULAR => {
                // `(float)Math.Sqrt(x * x + y * y)` with int products, as in C# (wrapping only for (-32768, -32768))
                let sq = x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y));
                let num = (sq as f64).sqrt() as f32;
                let num2 = linear(num, 32767.0, dead_zone_size);
                let num3 = if num2 > 0.0 { num2 / num } else { 0.0 };
                Vector2 { x: (x as f32 * num3).clamp(-1.0, 1.0), y: (y as f32 * num3).clamp(-1.0, 1.0) }
            }
            _ => Vector2 { x: linear(x as f32, 32767.0, 0.0), y: linear(y as f32, 32767.0, 0.0) },
        }
    }

    /// `ApplyTriggerDeadZone(value, deadZoneMode)`
    pub fn trigger(value: i32, mode: i32) -> f32 {
        if mode == DEAD_ZONE_NONE {
            linear(value as f32, 255.0, 0.0)
        } else {
            linear(value as f32, 255.0, TRIGGER)
        }
    }
}

/// `Buttons` values that are not XInput button bits (reference/xna/Buttons.cs): `IsButtonDown` derives them from
/// the sticks and triggers.
pub mod buttons {
    pub const LEFT_THUMBSTICK_LEFT: u32 = 0x20_0000;
    pub const RIGHT_TRIGGER: u32 = 0x40_0000;
    pub const LEFT_TRIGGER: u32 = 0x80_0000;
    pub const RIGHT_THUMBSTICK_UP: u32 = 0x100_0000;
    pub const RIGHT_THUMBSTICK_DOWN: u32 = 0x200_0000;
    pub const RIGHT_THUMBSTICK_RIGHT: u32 = 0x400_0000;
    pub const RIGHT_THUMBSTICK_LEFT: u32 = 0x800_0000;
    pub const LEFT_THUMBSTICK_UP: u32 = 0x1000_0000;
    pub const LEFT_THUMBSTICK_DOWN: u32 = 0x2000_0000;
    pub const LEFT_THUMBSTICK_RIGHT: u32 = 0x4000_0000;
    /// `_normalButtonMask` of `GamePadState.IsButtonDown`: every `ButtonValues` bit (64511 = 0xFBFF)
    pub const NORMAL_MASK: u32 = 0xFBFF;
}

/// `GamePadState` flattened (also answers `GamePadCapabilities.IsConnected`): the processed values XNA exposes
/// (`ThumbSticks`, `Triggers`, the button bits) and the raw XInput state it keeps for `IsButtonDown`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GamePadState {
    pub is_connected: bool,
    pub packet_number: i32,
    /// the `ButtonValues` bits (buttons and D-pad)
    pub buttons: u32,
    pub left_stick_x: f32,
    pub left_stick_y: f32,
    pub right_stick_x: f32,
    pub right_stick_y: f32,
    pub left_trigger: f32,
    pub right_trigger: f32,
    /// `_state.GamePad`: the raw values
    pub raw_left_trigger: u8,
    pub raw_right_trigger: u8,
    pub raw_thumb_lx: i16,
    pub raw_thumb_ly: i16,
    pub raw_thumb_rx: i16,
    pub raw_thumb_ry: i16,
}
impl GamePadState {
    pub fn is_connected(&self) -> bool {
        self.is_connected
    }
    /// `PacketNumber`
    pub fn packet_number(&self) -> i32 {
        self.packet_number
    }
    /// `IsButtonDown(Buttons)` (reference/xna/GamePadState.cs): every flag of the argument is down. The thumbstick
    /// direction and trigger "buttons" are derived from the raw values with the `IndependentAxes` dead zones,
    /// whatever mode the state was read with.
    pub fn is_button_down(&self, button: i32) -> bool {
        let button = button as u32;
        let mut down = self.buttons & buttons::NORMAL_MASK;
        let left = dead_zone::stick(self.raw_thumb_lx as i32, self.raw_thumb_ly as i32, DEAD_ZONE_INDEPENDENT_AXES, dead_zone::LEFT_STICK);
        let right = dead_zone::stick(self.raw_thumb_rx as i32, self.raw_thumb_ry as i32, DEAD_ZONE_INDEPENDENT_AXES, dead_zone::RIGHT_STICK);
        for (flag, on) in [
            (buttons::LEFT_THUMBSTICK_LEFT, left.x < 0.0),
            (buttons::LEFT_THUMBSTICK_RIGHT, left.x > 0.0),
            (buttons::LEFT_THUMBSTICK_DOWN, left.y < 0.0),
            (buttons::LEFT_THUMBSTICK_UP, left.y > 0.0),
            (buttons::RIGHT_THUMBSTICK_LEFT, right.x < 0.0),
            (buttons::RIGHT_THUMBSTICK_RIGHT, right.x > 0.0),
            (buttons::RIGHT_THUMBSTICK_DOWN, right.y < 0.0),
            (buttons::RIGHT_THUMBSTICK_UP, right.y > 0.0),
            (buttons::LEFT_TRIGGER, dead_zone::trigger(self.raw_left_trigger as i32, DEAD_ZONE_INDEPENDENT_AXES) > 0.0),
            (buttons::RIGHT_TRIGGER, dead_zone::trigger(self.raw_right_trigger as i32, DEAD_ZONE_INDEPENDENT_AXES) > 0.0),
        ] {
            if button & flag == flag && on {
                down |= flag;
            }
        }
        button & down == button
    }
    /// `IsButtonUp(Buttons)`: `!IsButtonDown(button)`.
    pub fn is_button_up(&self, button: i32) -> bool {
        !self.is_button_down(button)
    }
    /// `ThumbSticks`: both sticks, each axis -1..1, Y up.
    pub fn thumb_sticks(&self) -> GamePadThumbSticks {
        GamePadThumbSticks { left: Vector2 { x: self.left_stick_x, y: self.left_stick_y }, right: Vector2 { x: self.right_stick_x, y: self.right_stick_y } }
    }
    /// `Triggers`: 0..1 each.
    pub fn triggers(&self) -> GamePadTriggers {
        GamePadTriggers { left: self.left_trigger, right: self.right_trigger }
    }
}

/// `GamePadThumbSticks` (a struct): `Left` / `Right`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GamePadThumbSticks {
    pub left: Vector2,
    pub right: Vector2,
}

/// `GamePadTriggers` (a struct): `Left` / `Right`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GamePadTriggers {
    pub left: f32,
    pub right: f32,
}

fn pad_input(player_index: i32) -> Option<GamePadInput> {
    CURRENT.with(|c| usize::try_from(player_index).ok().and_then(|i| c.borrow().game_pads.get(i).copied().flatten()))
}

/// `GamePad.GetState(playerIndex)`: `GamePadDeadZone.IndependentAxes` (reference/xna/GamePad.cs).
pub fn game_pad_get_state(player_index: i32) -> GamePadState {
    game_pad_get_state_with(player_index, DEAD_ZONE_INDEPENDENT_AXES)
}

/// `GamePad.GetState(playerIndex, deadZoneMode)`: the constructor `GamePadState(ref XINPUT_STATE, ErrorCodes,
/// GamePadDeadZone)`. An index outside 0..3 or an empty slot is a disconnected pad with everything released.
pub fn game_pad_get_state_with(player_index: i32, dead_zone_mode: i32) -> GamePadState {
    let Some(p) = pad_input(player_index) else { return GamePadState::default() };
    let left = dead_zone::stick(p.thumb_lx as i32, p.thumb_ly as i32, dead_zone_mode, dead_zone::LEFT_STICK);
    let right = dead_zone::stick(p.thumb_rx as i32, p.thumb_ry as i32, dead_zone_mode, dead_zone::RIGHT_STICK);
    GamePadState {
        is_connected: true,
        packet_number: p.packet as i32,
        buttons: p.buttons & buttons::NORMAL_MASK,
        left_stick_x: left.x,
        left_stick_y: left.y,
        right_stick_x: right.x,
        right_stick_y: right.y,
        left_trigger: dead_zone::trigger(p.left_trigger as i32, dead_zone_mode),
        right_trigger: dead_zone::trigger(p.right_trigger as i32, dead_zone_mode),
        raw_left_trigger: p.left_trigger,
        raw_right_trigger: p.right_trigger,
        raw_thumb_lx: p.thumb_lx,
        raw_thumb_ly: p.thumb_ly,
        raw_thumb_rx: p.thumb_rx,
        raw_thumb_ry: p.thumb_ry,
    }
}

/// `GamePadCapabilities` (reference/xna/GamePadCapabilities.cs). A connected pad is reported as a standard
/// XInput game pad (`XINPUT_DEVSUBTYPE_GAMEPAD`): every button, both sticks, both triggers, both motors, no voice.
/// What a particular device lacks is not known to this layer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GamePadCapabilities {
    pub is_connected: bool,
}
plain_trace!(GamePadCapabilities);
impl GamePadCapabilities {
    pub fn is_connected(&self) -> bool {
        self.is_connected
    }
    /// `GamePadType`: `GamePad` (1) for a connected pad, `Unknown` (0) otherwise.
    pub fn game_pad_type(&self) -> i32 {
        self.is_connected as i32
    }
    /// Every `Has...Button`, `Has...ThumbStick`, `Has...Trigger`, `Has...VibrationMotor` property.
    pub fn has_control(&self) -> bool {
        self.is_connected
    }
    pub fn has_voice_support(&self) -> bool {
        false
    }
}

/// `GamePad.GetCapabilities(playerIndex)`
pub fn game_pad_get_capabilities(player_index: i32) -> GamePadCapabilities {
    GamePadCapabilities { is_connected: pad_input(player_index).is_some() }
}

/// `GamePad.SetVibration(playerIndex, leftMotor, rightMotor)`: true when the pad is connected (XNA: `XInputSetState`
/// succeeded), false otherwise. The request is recorded for the app ([`take_vibration_requests`]), which drives
/// the motors if its input library can; XNA truncates the speeds to 16 bits (`(short)(motor * 65535f)`), the
/// values are handed on as given, clamped to 0..1.
pub fn game_pad_set_vibration(player_index: i32, left: f32, right: f32) -> bool {
    if pad_input(player_index).is_none() {
        return false;
    }
    VIBRATION.with(|v| v.borrow_mut()[player_index as usize] = Some((left.clamp(0.0, 1.0), right.clamp(0.0, 1.0))));
    true
}
/// App: the last `SetVibration` of each player index since the previous call (left = low-frequency motor, right =
/// high-frequency motor, 0..1).
pub fn take_vibration_requests() -> [Option<(f32, f32)>; 4] {
    VIBRATION.with(|v| std::mem::take(&mut *v.borrow_mut()))
}

// ------------------------------------------------------------------------------------------------ pad events

/// The XInput state of up to four pads, built from single changes ([`PadEvent`]): what an app gets from an input
/// library that reports buttons by name and sticks / triggers as numbers, and what an input script writes. The
/// integers XNA applies its dead zones to are rebuilt here: `round(v * 32767)` for a stick axis, `round(v * 255)`
/// for a trigger.
/// `ButtonValues` (reference\xna\ButtonValues.cs)
pub mod xinput {
    pub const DPAD_UP: u32 = 0x0001;
    pub const DPAD_DOWN: u32 = 0x0002;
    pub const DPAD_LEFT: u32 = 0x0004;
    pub const DPAD_RIGHT: u32 = 0x0008;
    pub const START: u32 = 0x0010;
    pub const BACK: u32 = 0x0020;
    pub const LEFT_THUMB: u32 = 0x0040;
    pub const RIGHT_THUMB: u32 = 0x0080;
    pub const LEFT_SHOULDER: u32 = 0x0100;
    pub const RIGHT_SHOULDER: u32 = 0x0200;
    pub const BIG_BUTTON: u32 = 0x0800;
    pub const A: u32 = 0x1000;
    pub const B: u32 = 0x2000;
    pub const X: u32 = 0x4000;
    pub const Y: u32 = 0x8000;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PadAxis {
    LeftX,
    LeftY,
    RightX,
    RightY,
}

/// One change of one pad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PadEvent {
    Connected,
    Disconnected,
    /// an XInput button bit ([`xinput`]) went down / up
    Button(u32, bool),
    /// a stick axis, -1..1 (Y up)
    Axis(PadAxis, f32),
    /// a trigger, 0..1: left (false) / right (true)
    Trigger(bool, f32),
}

/// The four XInput slots.
#[derive(Default, Debug)]
pub struct PadAdapter {
    slots: [Option<(u64, GamePadInput)>; 4],
}

impl PadAdapter {
    fn slot_of(&self, id: u64) -> Option<usize> {
        self.slots.iter().position(|s| s.is_some_and(|(i, _)| i == id))
    }

    /// Applies one change. A change of a pad that never connected connects it first (an event can arrive before
    /// the connection message of the same frame was read); a fifth pad is ignored.
    pub fn apply(&mut self, id: u64, event: PadEvent) {
        if event == PadEvent::Disconnected {
            if let Some(i) = self.slot_of(id) {
                self.slots[i] = None;
            }
            return;
        }
        let i = match self.slot_of(id) {
            Some(i) => i,
            None => {
                let Some(free) = self.slots.iter().position(Option::is_none) else { return };
                self.slots[free] = Some((id, GamePadInput::default()));
                free
            }
        };
        let (_, pad) = self.slots[i].as_mut().expect("just looked up");
        let before = *pad;
        let axis = |v: f32| (v.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        let trigger = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        match event {
            PadEvent::Connected | PadEvent::Disconnected => {}
            PadEvent::Button(bit, true) => pad.buttons |= bit,
            PadEvent::Button(bit, false) => pad.buttons &= !bit,
            PadEvent::Axis(PadAxis::LeftX, v) => pad.thumb_lx = axis(v),
            PadEvent::Axis(PadAxis::LeftY, v) => pad.thumb_ly = axis(v),
            PadEvent::Axis(PadAxis::RightX, v) => pad.thumb_rx = axis(v),
            PadEvent::Axis(PadAxis::RightY, v) => pad.thumb_ry = axis(v),
            PadEvent::Trigger(false, v) => pad.left_trigger = trigger(v),
            PadEvent::Trigger(true, v) => pad.right_trigger = trigger(v),
        }
        if *pad != before {
            // XInput's dwPacketNumber: changes with the state
            pad.packet = before.packet.wrapping_add(1);
        }
    }

    /// `InputSnapshot::game_pads`
    pub fn snapshot(&self) -> [Option<GamePadInput>; 4] {
        self.slots.map(|s| s.map(|(_, p)| p))
    }

    /// The pad id in a slot.
    pub fn id_of(&self, slot: usize) -> Option<u64> {
        self.slots.get(slot).copied().flatten().map(|(id, _)| id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_have_xna_values() {
        assert_eq!((keys::A, keys::Z, keys::D0, keys::Space, keys::Enter, keys::Escape, keys::Left, keys::Up, keys::Right, keys::Down), (65, 90, 48, 32, 13, 27, 37, 38, 39, 40));
        assert_eq!((keys::LeftShift, keys::RightShift, keys::LeftControl, keys::LeftAlt, keys::F1, keys::F12, keys::OemTilde, keys::Back, keys::Tab), (160, 161, 162, 164, 112, 123, 192, 8, 9));
        assert_eq!(keys::ALL.len(), 160);
        assert!(keys::ALL.iter().all(|(_, v)| (0..256).contains(v)));
    }

    #[test]
    fn states_come_from_the_snapshot() {
        set_snapshot(InputSnapshot { keys_down: vec![keys::W, keys::LeftShift, keys::A, 999], mouse_x: 10, mouse_y: 20, scroll_wheel: -240, left: true, x_button2: true, ..Default::default() });
        let k = keyboard_get_state();
        assert!(k.is_key_down(keys::W) && k.is_key_down(keys::A) && k.is_key_up(keys::D) && !k.is_key_down(999) && !k.is_key_down(-1));
        assert_eq!(k.get_pressed_keys().as_slice(), [keys::A, keys::W, keys::LeftShift]);
        assert_eq!(k.key_state(keys::W), 1);
        // not members of Keys: the left mouse button, VK_SHIFT, VK_CONTROL, VK_MENU of GetKeyboardState
        let odd = KeyboardState::from_keys(&[1, 2, 4, 16, 17, 18, keys::LeftControl]);
        assert_eq!(odd.get_pressed_keys().as_slice(), [keys::LeftControl]);
        let m = mouse_get_state();
        assert_eq!((m.x, m.y, m.scroll_wheel_value, m.left_button, m.right_button, m.middle_button, m.x_button1, m.x_button2), (10, 20, -240, 1, 0, 0, 0, 1));
        mouse_set_position(5, 6);
        assert_eq!((mouse_get_state().x, mouse_get_state().y), (5, 6));
        assert_eq!(take_mouse_position_request(), Some((5, 6)));
        assert_eq!(take_mouse_position_request(), None);
        assert!(!game_pad_get_state(0).is_connected() && !game_pad_get_state(7).is_connected());
        assert!(!game_pad_set_vibration(0, 1.0, 1.0), "not connected: false, as XNA");
        assert!(!game_pad_get_capabilities(0).is_connected());
        let mut s = snapshot();
        s.game_pads[1] = Some(GamePadInput::from_normalized(4096 | 16, (0.5, -1.0), (0.0, 0.0), 0.0, 0.75));
        set_snapshot(s);
        let g = game_pad_get_state(1);
        assert!(g.is_connected() && g.is_button_down(4096) && g.is_button_down(4096 | 16) && !g.is_button_down(4096 | 8192) && g.is_button_up(8192));
        // 0.5 -> 16384 raw; IndependentAxes: (16384 - 7849) / (32767 - 7849)
        assert_eq!((g.left_stick_x, g.left_stick_y), ((16384.0f32 - 7849.0) / (32767.0 - 7849.0), -1.0));
        assert_eq!((g.thumb_sticks().left.x, g.thumb_sticks().left.y), (g.left_stick_x, -1.0));
        // 0.75 -> 191 raw; (191 - 30) / (255 - 30)
        assert_eq!((g.triggers().left, g.triggers().right), (0.0, (191.0f32 - 30.0) / 225.0));
        assert!(game_pad_get_capabilities(1).is_connected() && game_pad_get_capabilities(1).has_control());
        assert!(game_pad_set_vibration(1, 0.25, 2.0));
        assert_eq!(take_vibration_requests(), [None, Some((0.25, 1.0)), None, None]);
        assert_eq!(take_vibration_requests(), [None; 4]);
        assert_eq!(MouseState::new(1, 2, 3, 1, 0, 1, 0, 0), MouseState { x: 1, y: 2, scroll_wheel_value: 3, left_button: 1, middle_button: 0, right_button: 1, x_button1: 0, x_button2: 0 });
    }

    /// Hand-computed from reference/xna/GamePadDeadZoneUtils.cs and GamePadState.cs.
    #[test]
    fn dead_zones_and_derived_buttons_follow_xna() {
        use dead_zone::*;
        // linear: inside the zone 0, at the edge 0, just outside, full scale, beyond full scale (-32768)
        assert_eq!(linear(7849.0, 32767.0, LEFT_STICK), 0.0);
        assert_eq!(linear(-7849.0, 32767.0, LEFT_STICK), 0.0);
        assert_eq!(linear(7850.0, 32767.0, LEFT_STICK), 1.0 / 24918.0);
        assert_eq!(linear(32767.0, 32767.0, LEFT_STICK), 1.0);
        assert_eq!(linear(-32768.0, 32767.0, LEFT_STICK), -1.0);
        assert_eq!(linear(-20000.0, 32767.0, RIGHT_STICK), (-20000.0f32 + 8689.0) / (32767.0 - 8689.0));
        // triggers: 30 of 255 dead unless the mode is None
        assert_eq!((trigger(30, DEAD_ZONE_INDEPENDENT_AXES), trigger(31, DEAD_ZONE_INDEPENDENT_AXES), trigger(255, DEAD_ZONE_CIRCULAR)), (0.0, 1.0 / 225.0, 1.0));
        assert_eq!((trigger(30, DEAD_ZONE_NONE), trigger(255, DEAD_ZONE_NONE)), (30.0 / 255.0, 1.0));
        // sticks, three modes
        let none = stick(5000, -5000, DEAD_ZONE_NONE, LEFT_STICK);
        assert_eq!((none.x, none.y), (5000.0 / 32767.0, -5000.0 / 32767.0));
        let ind = stick(5000, -20000, DEAD_ZONE_INDEPENDENT_AXES, LEFT_STICK);
        assert_eq!((ind.x, ind.y), (0.0, (-20000.0f32 + 7849.0) / 24918.0));
        // circular: (6000, 6000) has length 8485.28 > 7849: alive although each axis alone is dead
        let c = stick(6000, 6000, DEAD_ZONE_CIRCULAR, LEFT_STICK);
        let len = ((6000i32 * 6000 + 6000 * 6000) as f64).sqrt() as f32;
        let scaled = (len - 7849.0) / 24918.0;
        assert_eq!((c.x, c.y), (6000.0 * (scaled / len), 6000.0 * (scaled / len)));
        assert!(c.x > 0.0 && stick(6000, 6000, DEAD_ZONE_INDEPENDENT_AXES, LEFT_STICK).x == 0.0);
        let dead = stick(3000, 4000, DEAD_ZONE_CIRCULAR, LEFT_STICK);
        assert_eq!((dead.x, dead.y), (0.0, 0.0));
        // the state: the right stick has its own, larger zone; the derived "buttons" use IndependentAxes on the raw values
        let mut s = InputSnapshot::default();
        s.game_pads[0] = Some(GamePadInput { buttons: 0x1000 | 1 | 0x0400, left_trigger: 31, right_trigger: 30, thumb_lx: -8000, thumb_ly: 8000, thumb_rx: 8000, thumb_ry: -9000, packet: 7 });
        set_snapshot(s);
        let g = game_pad_get_state(0);
        assert_eq!(g.packet_number(), 7);
        assert_eq!(g.buttons, 0x1001, "bit 0x400 is not a ButtonValues bit");
        assert_eq!((g.left_stick_x, g.left_stick_y), ((-8000.0f32 + 7849.0) / 24918.0, (8000.0f32 - 7849.0) / 24918.0));
        assert_eq!((g.right_stick_x, g.right_stick_y), (0.0, (-9000.0f32 + 8689.0) / 24078.0));
        assert!(g.is_button_down(buttons::LEFT_THUMBSTICK_LEFT as i32) && g.is_button_down(buttons::LEFT_THUMBSTICK_UP as i32));
        assert!(!g.is_button_down(buttons::LEFT_THUMBSTICK_RIGHT as i32) && !g.is_button_down(buttons::RIGHT_THUMBSTICK_RIGHT as i32));
        assert!(g.is_button_down(buttons::RIGHT_THUMBSTICK_DOWN as i32) && !g.is_button_down(buttons::RIGHT_THUMBSTICK_UP as i32));
        assert!(g.is_button_down(buttons::LEFT_TRIGGER as i32) && !g.is_button_down(buttons::RIGHT_TRIGGER as i32));
        assert!(g.is_button_down((0x1000 | buttons::LEFT_TRIGGER) as i32) && !g.is_button_down((0x2000 | buttons::LEFT_TRIGGER) as i32));
        // IsButtonUp is !IsButtonDown: a combination with one flag up counts as "up"
        assert!(g.is_button_up(0x2000) && g.is_button_up(0x1000 | 0x2000) && !g.is_button_up(0x1000 | 1));
        // GetState with a mode
        let n = game_pad_get_state_with(0, DEAD_ZONE_NONE);
        assert_eq!((n.right_stick_x, n.right_trigger), (8000.0 / 32767.0, 30.0 / 255.0));
        // from_normalized: rounding and clamping
        let p = GamePadInput::from_normalized(0, (1.5, -1.0), (0.0, 0.24), 1.0, -3.0);
        assert_eq!((p.thumb_lx, p.thumb_ly, p.thumb_rx, p.thumb_ry, p.left_trigger, p.right_trigger), (32767, -32767, 0, 7864, 255, 0));
    }
}
