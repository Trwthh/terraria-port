//! Game pads: Bevy's raw gamepad events (gilrs) -> the XInput state `terraria_host::input::GamePadInput` that
//! XNA's `GamePad.GetState` starts from.
//!
//! Bevy reports buttons by name and sticks / triggers as numbers in -1..1 / 0..1. XNA reads `XINPUT_GAMEPAD`
//! (reference\xna\XINPUT_GAMEPAD.cs): a 16-bit button mask, two trigger bytes, four signed 16-bit stick values, and
//! applies its dead zones to those integers (the host does that, `input::dead_zone`). [`PadAdapter`] rebuilds the
//! integers: `round(v * 32767)`, `round(v * 255)`. The RAW events are used (`RawGamepadEvent`: what gilrs
//! delivered, before Bevy's own `GamepadSettings` dead zones and thresholds), so that the only dead zones are
//! XNA's.
//!
//! One adapter (`terraria_host::input::PadAdapter`), two feeders, the same [`PadEvent`]s: the shell turns
//! `RawGamepadEvent` messages into them ([`from_bevy`]), and an input script's `pad ...` lines are them
//! (`script.rs`). Nothing else writes a pad's state.
//!
//! Button mapping (XInput / XNA name <- Bevy / gilrs name): A South, B East, X West, Y North, LeftShoulder
//! LeftTrigger, RightShoulder RightTrigger, Back Select, Start Start, BigButton Mode, LeftStick LeftThumb,
//! RightStick RightThumb, DPad* DPad*. The analog triggers are the VALUES of the buttons LeftTrigger2 /
//! RightTrigger2. Stick Y is up-positive on both sides.
//!
//! Player index: the first free of the four slots when a pad connects (gilrs does not tell the XInput user index).
//!
//! `GamePad.SetVibration`: [`rumble_requests`] turns the host's recorded requests into Bevy
//! `GamepadRumbleRequest`s for the pad of that slot (left motor = low frequency = `strong_motor`, right motor =
//! high frequency = `weak_motor`); a scripted pad has no device behind it and the request is logged as inert.

use bevy::input::gamepad::{GamepadAxis, GamepadButton, GamepadConnection, GamepadRumbleIntensity, GamepadRumbleRequest, RawGamepadEvent};
use bevy::prelude::Entity;
pub use terraria_host::input::{xinput, PadAdapter, PadAxis, PadEvent};

/// The XInput button bit of a Bevy button (None: the triggers, which are analog, and buttons XInput does not have).
pub fn button_bit(b: GamepadButton) -> Option<u32> {
    Some(match b {
        GamepadButton::South => xinput::A,
        GamepadButton::East => xinput::B,
        GamepadButton::West => xinput::X,
        GamepadButton::North => xinput::Y,
        GamepadButton::LeftTrigger => xinput::LEFT_SHOULDER,
        GamepadButton::RightTrigger => xinput::RIGHT_SHOULDER,
        GamepadButton::Select => xinput::BACK,
        GamepadButton::Start => xinput::START,
        GamepadButton::Mode => xinput::BIG_BUTTON,
        GamepadButton::LeftThumb => xinput::LEFT_THUMB,
        GamepadButton::RightThumb => xinput::RIGHT_THUMB,
        GamepadButton::DPadUp => xinput::DPAD_UP,
        GamepadButton::DPadDown => xinput::DPAD_DOWN,
        GamepadButton::DPadLeft => xinput::DPAD_LEFT,
        GamepadButton::DPadRight => xinput::DPAD_RIGHT,
        _ => return None,
    })
}

/// A raw Bevy gamepad event as (pad id, change). A digital button counts as down from half way (gilrs reports 0
/// or 1 for them).
pub fn from_bevy(e: &RawGamepadEvent) -> Option<(u64, PadEvent)> {
    Some(match e {
        RawGamepadEvent::Connection(c) => (c.gamepad.to_bits(), if matches!(c.connection, GamepadConnection::Connected { .. }) { PadEvent::Connected } else { PadEvent::Disconnected }),
        RawGamepadEvent::Button(b) => (
            b.gamepad.to_bits(),
            match b.button {
                GamepadButton::LeftTrigger2 => PadEvent::Trigger(false, b.value),
                GamepadButton::RightTrigger2 => PadEvent::Trigger(true, b.value),
                other => PadEvent::Button(button_bit(other)?, b.value >= 0.5),
            },
        ),
        RawGamepadEvent::Axis(a) => (
            a.gamepad.to_bits(),
            PadEvent::Axis(
                match a.axis {
                    GamepadAxis::LeftStickX => PadAxis::LeftX,
                    GamepadAxis::LeftStickY => PadAxis::LeftY,
                    GamepadAxis::RightStickX => PadAxis::RightX,
                    GamepadAxis::RightStickY => PadAxis::RightY,
                    _ => return None,
                },
                a.value,
            ),
        ),
    })
}

/// `GamePad.SetVibration` requests of the host (per slot: left, right in 0..1) as Bevy rumble requests for the
/// pads `adapter` knows, plus one log line per request that has no device to go to. XNA's vibration stays until
/// it is set again; Bevy's has a duration, so a running one is stopped and a long one started.
pub fn rumble_requests(requests: [Option<(f32, f32)>; 4], adapter: &PadAdapter, is_device: impl Fn(u64) -> bool) -> (Vec<GamepadRumbleRequest>, Vec<String>) {
    let (mut out, mut log) = (Vec::new(), Vec::new());
    for (slot, r) in requests.into_iter().enumerate() {
        let Some((left, right)) = r else { continue };
        match adapter.id_of(slot).filter(|id| is_device(*id)) {
            Some(id) => {
                let gamepad = Entity::from_bits(id);
                out.push(GamepadRumbleRequest::Stop { gamepad });
                if left > 0.0 || right > 0.0 {
                    out.push(GamepadRumbleRequest::Add { duration: std::time::Duration::from_secs(3600), intensity: GamepadRumbleIntensity { strong_motor: left, weak_motor: right }, gamepad });
                }
            }
            None => log.push(format!("GamePad.SetVibration({slot}, {left}, {right}): inert, the pad in this slot is not a device (a scripted pad has no motors)")),
        }
    }
    (out, log)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::gamepad::{GamepadConnectionEvent, RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent};
    use bevy::prelude::*;
    use terraria_host::input::{self, buttons, InputSnapshot};

    #[derive(Resource, Default)]
    struct Adapter(PadAdapter);

    /// The shell's reading of the raw events, as a system of its own.
    fn feed(mut adapter: ResMut<Adapter>, mut events: MessageReader<RawGamepadEvent>) {
        for e in events.read() {
            if let Some((id, ev)) = from_bevy(e) {
                adapter.0.apply(id, ev);
            }
        }
    }

    fn state(app: &App, slot: i32) -> input::GamePadState {
        input::set_snapshot(InputSnapshot { game_pads: app.world().resource::<Adapter>().0.snapshot(), ..Default::default() });
        input::game_pad_get_state(slot)
    }

    /// Synthetic connection / button / axis events through a headless Bevy `App` into the host's `GamePad.GetState`.
    /// No device and no gilrs: the events are written as bevy_gilrs writes them.
    #[test]
    fn bevy_gamepad_events_reach_the_host_state() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_message::<RawGamepadEvent>().init_resource::<Adapter>().add_systems(Update, feed);
        let pad = app.world_mut().spawn_empty().id();
        let second = app.world_mut().spawn_empty().id();
        let mut send = |app: &mut App, e: RawGamepadEvent| {
            app.world_mut().write_message(e);
            app.update();
        };
        app.update();
        assert!(!state(&app, 0).is_connected());
        send(&mut app, GamepadConnectionEvent { gamepad: pad, connection: GamepadConnection::Connected { name: "test pad".into(), vendor_id: None, product_id: None } }.into());
        let s = state(&app, 0);
        assert!(s.is_connected() && s.buttons == 0 && !state(&app, 1).is_connected());
        // every button
        for (b, bit) in [
            (GamepadButton::South, 0x1000),
            (GamepadButton::East, 0x2000),
            (GamepadButton::West, 0x4000),
            (GamepadButton::North, 0x8000),
            (GamepadButton::LeftTrigger, 0x100),
            (GamepadButton::RightTrigger, 0x200),
            (GamepadButton::Select, 0x20),
            (GamepadButton::Start, 0x10),
            (GamepadButton::Mode, 0x800),
            (GamepadButton::LeftThumb, 0x40),
            (GamepadButton::RightThumb, 0x80),
            (GamepadButton::DPadUp, 1),
            (GamepadButton::DPadDown, 2),
            (GamepadButton::DPadLeft, 4),
            (GamepadButton::DPadRight, 8),
        ] {
            send(&mut app, RawGamepadButtonChangedEvent::new(pad, b, 1.0).into());
            let s = state(&app, 0);
            assert!(s.is_button_down(bit) && s.buttons == bit as u32, "{b:?}");
            send(&mut app, RawGamepadButtonChangedEvent::new(pad, b, 0.0).into());
            assert!(state(&app, 0).is_button_up(bit), "{b:?}");
        }
        // A held, the left stick half right and fully up, the right stick inside its dead zone, the right trigger
        // at 0.5, the left trigger below the 30 / 255 zone
        send(&mut app, RawGamepadButtonChangedEvent::new(pad, GamepadButton::South, 1.0).into());
        send(&mut app, RawGamepadAxisChangedEvent::new(pad, GamepadAxis::LeftStickX, 0.5).into());
        send(&mut app, RawGamepadAxisChangedEvent::new(pad, GamepadAxis::LeftStickY, 1.0).into());
        send(&mut app, RawGamepadAxisChangedEvent::new(pad, GamepadAxis::RightStickX, -0.26).into());
        send(&mut app, RawGamepadAxisChangedEvent::new(pad, GamepadAxis::RightStickY, 0.2).into());
        send(&mut app, RawGamepadButtonChangedEvent::new(pad, GamepadButton::RightTrigger2, 0.5).into());
        send(&mut app, RawGamepadButtonChangedEvent::new(pad, GamepadButton::LeftTrigger2, 0.1).into());
        let s = state(&app, 0);
        // 0.5 -> 16384; (16384 - 7849) / 24918. -0.26 -> -8519 (inside 8689). 0.5 -> 128; (128 - 30) / 225. 0.1 -> 26 (inside 30)
        assert_eq!((s.left_stick_x, s.left_stick_y), ((16384.0f32 - 7849.0) / 24918.0, 1.0));
        assert_eq!((s.right_stick_x, s.right_stick_y), (0.0, 0.0));
        assert_eq!((s.left_trigger, s.right_trigger), (0.0, (128.0f32 - 30.0) / 225.0));
        assert_eq!((s.raw_thumb_lx, s.raw_thumb_ly, s.raw_thumb_rx, s.raw_thumb_ry, s.raw_left_trigger, s.raw_right_trigger), (16384, 32767, -8519, 6553, 26, 128));
        assert!(s.is_button_down(0x1000) && s.is_button_down(buttons::LEFT_THUMBSTICK_RIGHT as i32) && s.is_button_down(buttons::LEFT_THUMBSTICK_UP as i32));
        assert!(s.is_button_down(buttons::RIGHT_TRIGGER as i32) && !s.is_button_down(buttons::LEFT_TRIGGER as i32) && !s.is_button_down(buttons::RIGHT_THUMBSTICK_LEFT as i32));
        let packet = s.packet_number;
        // the same value again is no new packet; a new value is
        send(&mut app, RawGamepadAxisChangedEvent::new(pad, GamepadAxis::LeftStickX, 0.5).into());
        assert_eq!(state(&app, 0).packet_number, packet);
        send(&mut app, RawGamepadAxisChangedEvent::new(pad, GamepadAxis::LeftStickX, -1.0).into());
        assert_eq!((state(&app, 0).packet_number, state(&app, 0).left_stick_x), (packet + 1, -1.0));
        // a second pad takes the next slot; the first one going away frees slot 0 and leaves slot 1 alone
        send(&mut app, GamepadConnectionEvent { gamepad: second, connection: GamepadConnection::Connected { name: "second".into(), vendor_id: None, product_id: None } }.into());
        send(&mut app, RawGamepadButtonChangedEvent::new(second, GamepadButton::East, 1.0).into());
        assert!(state(&app, 1).is_button_down(0x2000) && !state(&app, 0).is_button_down(0x2000));
        send(&mut app, GamepadConnectionEvent { gamepad: pad, connection: GamepadConnection::Disconnected }.into());
        assert!(!state(&app, 0).is_connected() && state(&app, 1).is_connected());
        // vibration: to the device of the slot; nothing for an empty slot, a log line for a pad without a device
        let adapter = &app.world().resource::<Adapter>().0;
        let (req, log) = rumble_requests([Some((1.0, 0.5)), Some((0.25, 0.0)), None, None], adapter, |_| true);
        assert_eq!((req.len(), log.len()), (2, 1), "slot 0 is empty: inert; slot 1: stop + add");
        assert!(matches!(&req[1], GamepadRumbleRequest::Add { intensity, gamepad, .. } if *gamepad == second && intensity.strong_motor == 0.25 && intensity.weak_motor == 0.0));
        let (req, _) = rumble_requests([None, Some((0.0, 0.0)), None, None], adapter, |_| true);
        assert!(matches!(req[..], [GamepadRumbleRequest::Stop { .. }]));
        let (req, log) = rumble_requests([None, Some((1.0, 1.0)), None, None], adapter, |_| false);
        assert!(req.is_empty() && log.len() == 1 && log[0].contains("inert"));
    }
}
