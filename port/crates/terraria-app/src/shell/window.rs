//! The window side of the shell: monitors and display modes for the host, and the `WindowCommand`s / `WindowRequest`
//! the game produced, applied to the Bevy window.
//!
//! How much is applied depends on the run ([`Apply`]):
//!
//! - [`Apply::Real`], a normal visible window: everything.
//! - [`Apply::Hidden`] (`TERRARIA_HIDDEN=1`): a hidden run must never show or move a real window over the user's
//!   desktop, so decorations, maximise / minimise / fullscreen and focus requests are only *recorded* in
//!   [`WindowShadow`] and reported back to the game (`winforms::report_window`) as if the window had obeyed. Size and
//!   position changes are applied (a hidden window can be resized, which is what exercises the resize path).
//! - [`Apply::Passive`] (`TERRARIA_NOACTIVATE=1` alone: a visible window that must never come to the front or cover a
//!   monitor): borderless, maximise, minimise, fullscreen, bring-to-front and moves are recorded, reported back and
//!   logged (`[shell] window: ... (not applied)`), not applied.
//! - [`Apply::Fake`] (`TERRARIA_NOACTIVATE=1` with `TERRARIA_FAKE_MONITOR=x,y,w,h`): the game is told that it has
//!   ONE monitor of that size at (0, 0), as a primary monitor is (the game's own code moves a borderless window
//!   to the literal point (0, 0): `Main.SetBorderlessFormStyle`, Main.cs:53217-53221), and its window requests are
//!   APPLIED for real inside the rectangle: borderless (the frame really goes), moves, sizes, maximise and
//!   fullscreen. The game's desktop coordinates are shifted by the rectangle's origin on the way out and back.
//!   The rectangle lies outside the real desktop (the shell refuses to start otherwise) and every placement is
//!   checked against the real desktop before it is made, so the window stays where nobody sees it; it is never
//!   activated and never minimised by the shell.
//!
//! # Borderless
//! `FormBorderStyle.None` is `Window::decorations = false`. winit keeps the window's style bits (`WS_CAPTION`,
//! `WS_THICKFRAME`: it wants the system's snap and resize behaviour) and takes the frame away by answering
//! `WM_NCCALCSIZE` with the whole window as client area. So a borderless window of the port is recognised by
//! `GetClientRect` == `GetWindowRect`, not by its style bits; the original's borderless form has neither bit.
//! WinForms keeps the CLIENT size when the border style changes; winit keeps the OUTER size, so the shell asks for
//! the old client size again after the change ([`WindowShadow::keep_client`], `enforce_client_size`).
//!
//! # Fullscreen
//! XNA's `GraphicsDeviceManager.IsFullScreen` is Direct3D 9 exclusive fullscreen: the display mode is switched to
//! the back buffer's resolution and the device owns the monitor. wgpu has no exclusive mode (and a mode switch
//! would rearrange the user's desktop), so the port maps it to a borderless window that covers the monitor the
//! window is on: `WindowMode::BorderlessFullscreen(Current)` in a normal run. The back buffer then has the
//! monitor's resolution, not the one the game asked for (the game is told through the client size and follows:
//! `Main.SetDisplayMode` adopts the viewport's size); a lower fullscreen resolution is therefore not scaled up as
//! exclusive fullscreen does. In [`Apply::Fake`] the same thing is done by hand (no frame, the monitor's origin,
//! the monitor's size), because winit's fullscreen activates the window.
//!
//! Leaving fullscreen: with exclusive fullscreen the back buffer keeps the resolution the game chose, so the
//! original comes back to a window of that size. Here the game has meanwhile adopted the monitor's size (it was
//! told the client area is that big) and asks for a WINDOW of the monitor's size, which with a frame is larger
//! than the monitor. The shell therefore remembers the client size the window had before fullscreen
//! ([`WindowShadow::windowed_size`]) and goes back to it when the game leaves fullscreen asking for the monitor's
//! size; the game then adopts that size as it adopts a user's resize. A size the game asks for that is NOT the
//! monitor's (it was chosen in the menu meanwhile) is taken as asked.
//!
//! # Maximise in `Apply::Fake`
//! `ShowWindow(SW_MAXIMIZE)` maximises onto the monitor Windows thinks the window is nearest to: the real one.
//! The shell places the window itself instead: at the fake monitor's origin, with an outer size equal to the
//! monitor (the frame stays, as on a maximised window). The window does not carry `WS_MAXIMIZE`; the game is told
//! `FormWindowState.Maximized`, which is all it reads.
//!
//! The present mode (vsync) is applied in every mode except hidden.

use bevy::prelude::*;
use bevy::window::{CursorOptions, Monitor, MonitorSelection, PresentMode, PrimaryMonitor, WindowMode, WindowPosition};
use terraria_host::game::WindowRequest;
use terraria_host::sys::drawing::Rectangle;
use terraria_host::winforms::{self, WindowCommand};

/// A rectangle of the virtual screen: x, y, width, height (physical pixels).
pub type Rect4 = (i32, i32, i32, i32);

/// How the game's window requests are carried out (module docs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Apply {
    Real,
    Hidden,
    Passive,
    Fake {
        /// the rectangle the game is told is its only monitor
        monitor: Rect4,
        /// the real virtual screen: no placement may touch it
        desktop: Rect4,
    },
}

impl Apply {
    /// nothing that shows, raises or enlarges the window is applied
    fn recorded_only(self) -> bool {
        matches!(self, Apply::Hidden | Apply::Passive)
    }
}

/// What the window is, as far as the game has been told.
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct WindowShadow {
    pub maximized: bool,
    pub minimized: bool,
    pub borderless: bool,
    pub full_screen: bool,
    /// `Apply::Fake`: position and client size before the window was maximised / made fullscreen
    pub restore: Option<(IVec2, (u32, u32))>,
    /// the client size the window had when the game went fullscreen (module docs, "Leaving fullscreen")
    pub windowed_size: Option<(u32, u32)>,
    /// the frame was switched on or off: the client size the window must have again, and for how many ticks
    /// more the shell insists on it (module docs, "Borderless")
    pub keep_client: Option<((u32, u32), u32)>,
}

/// After a change of the frame: asks for the kept client size until the window has it (or the ticks run out).
/// Returns the size to report to the game meanwhile.
pub fn enforce_client_size(window: &mut Mut<Window>, shadow: &mut WindowShadow) -> Option<(u32, u32)> {
    let (size, left) = shadow.keep_client?;
    let now = (window.physical_width(), window.physical_height());
    if now.0 == 0 || now.1 == 0 {
        // minimised: nothing to insist on
        return None;
    }
    if left == 0 {
        shadow.keep_client = None;
        return None;
    }
    shadow.keep_client = Some((size, left - 1));
    if now != size {
        window.resolution.set_physical_resolution(size.0, size.1);
    }
    Some(size)
}

fn intersects(a: Rect4, b: Rect4) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// `TERRARIA_FAKE_MONITOR`: the rectangle must not touch the real desktop. `Err` says why not.
pub fn check_fake_monitor(monitor: Rect4, desktop: Rect4) -> Result<(), String> {
    if monitor.2 < 800 || monitor.3 < 600 {
        return Err(format!("{}x{} is smaller than the game's minimum window (800x600)", monitor.2, monitor.3));
    }
    if desktop.2 > 0 && desktop.3 > 0 && intersects(monitor, desktop) {
        return Err(format!("the rectangle {monitor:?} touches the real desktop {desktop:?}: it must lie outside every monitor"));
    }
    if monitor.0 < -32000 || monitor.1 < -32000 || monitor.0 + monitor.2 > 32000 || monitor.1 + monitor.3 > 32000 {
        return Err(format!("the rectangle {monitor:?} leaves the coordinate range of the window system"));
    }
    Ok(())
}

/// The largest fake monitor of the wanted size that fits below and right of the real desktop (the place the test
/// driver uses): the wanted size, cut down to what the window system's coordinate range leaves.
pub fn fake_monitor_for(desktop: Rect4, wanted: (i32, i32)) -> Rect4 {
    let (x, y) = (desktop.0 + desktop.2 + 240, desktop.1 + desktop.3 + 160);
    (x, y, wanted.0.min(32000 - x), wanted.1.min(32000 - y))
}

/// Feeds `winforms` with the machine's monitors and the primary monitor's resolutions, when they changed.
/// Returns the size of the monitor the window is on (the primary one if the window position is unknown).
pub fn sync_monitors<'a>(monitors: impl Iterator<Item = (&'a Monitor, bool)>, window_pos: Option<IVec2>, last: &mut Vec<winforms::Monitor>, last_modes: &mut Vec<(i32, i32)>) -> Option<(i32, i32)> {
    let mut list = Vec::new();
    let mut modes: Vec<(i32, i32)> = Vec::new();
    let mut on_window: Option<(i32, i32)> = None;
    let mut primary_size: Option<(i32, i32)> = None;
    for (m, primary) in monitors {
        let r = Rectangle::new(m.physical_position.x, m.physical_position.y, m.physical_width as i32, m.physical_height as i32);
        list.push(winforms::Monitor { device_name: m.name.clone().unwrap_or_else(|| format!("\\\\.\\DISPLAY{}", list.len() + 1)), bounds: r, working_area: r, primary });
        if primary {
            primary_size = Some((r.width, r.height));
            for v in &m.video_modes {
                let mode = (v.physical_size.x as i32, v.physical_size.y as i32);
                if !modes.contains(&mode) {
                    modes.push(mode);
                }
            }
        }
        if let Some(p) = window_pos {
            if p.x >= r.x && p.x < r.x + r.width && p.y >= r.y && p.y < r.y + r.height {
                on_window = Some((r.width, r.height));
            }
        }
    }
    modes.sort();
    if list != *last {
        winforms::set_monitors(list.clone());
        *last = list;
    }
    // GraphicsAdapter.SupportedDisplayModes: the primary monitor's modes (without them the host derives a list)
    if !modes.is_empty() && modes != *last_modes {
        winforms::set_display_modes(modes.clone());
        *last_modes = modes;
    }
    on_window.or(primary_size)
}

/// One monitor, given: the game is told about this rectangle and nothing else (`TERRARIA_DISPLAY`,
/// `TERRARIA_FAKE_MONITOR`). The display modes are the standard sizes that fit it (the host derives them).
pub fn single_monitor(r: Rect4, last: &mut Vec<winforms::Monitor>) -> (i32, i32) {
    let b = Rectangle::new(r.0, r.1, r.2, r.3);
    let list = vec![winforms::Monitor { device_name: "\\\\.\\DISPLAY1".to_owned(), bounds: b, working_area: b, primary: true }];
    if list != *last {
        winforms::set_monitors(list.clone());
        *last = list;
    }
    (r.2, r.3)
}

fn position_of(window: &Window) -> Option<IVec2> {
    match window.position {
        WindowPosition::At(p) => Some(p),
        _ => None,
    }
}

/// `Apply::Fake`: puts the window at `pos` with the client size `size`, unless its outer rectangle would touch
/// the real desktop. `frame` is what the window's frame adds to the client size (0, 0 without decorations).
fn place(window: &mut Mut<Window>, pos: IVec2, size: (u32, u32), frame: (i32, i32), desktop: Rect4, what: &str) -> bool {
    let frame = if window.decorations { frame } else { (0, 0) };
    let outer = (pos.x, pos.y, size.0 as i32 + frame.0, size.1 as i32 + frame.1);
    if desktop.2 > 0 && desktop.3 > 0 && intersects(outer, desktop) {
        eprintln!("[shell] window: {what} would put the window at {outer:?}, on the real desktop {desktop:?} (not applied)");
        return false;
    }
    if position_of(window) != Some(pos) {
        window.position = WindowPosition::At(pos);
    }
    if (window.physical_width(), window.physical_height()) != size {
        window.resolution.set_physical_resolution(size.0.max(1), size.1.max(1));
    }
    true
}

/// Applies the commands the game queued through `System.Windows.Forms` and updates the shadow state. `frame`: what
/// the window's frame adds to its client size (outer size minus client size), for `Apply::Fake`.
pub fn apply_commands(commands: Vec<WindowCommand>, window: &mut Mut<Window>, shadow: &mut WindowShadow, mode: Apply, frame: (i32, i32)) {
    let recorded = mode.recorded_only();
    let decorations_before = window.decorations;
    let size_before = (window.physical_width(), window.physical_height());
    let mut sized = false;
    for c in commands {
        if mode == Apply::Passive && matches!(c, WindowCommand::Borderless(_) | WindowCommand::Maximized(_) | WindowCommand::Minimized(_) | WindowCommand::BringToFront) {
            eprintln!("[shell] window: the game asked for {c:?} (not applied: TERRARIA_NOACTIVATE)");
        }
        if matches!(mode, Apply::Fake { .. }) {
            eprintln!("[shell] window: the game asked for {c:?}");
        }
        match c {
            WindowCommand::Borderless(b) => {
                shadow.borderless = b;
                if !recorded && window.decorations == b && !shadow.full_screen {
                    window.decorations = !b;
                }
            }
            WindowCommand::Maximized(m) => {
                let was = shadow.maximized;
                shadow.maximized = m;
                if m {
                    shadow.minimized = false;
                }
                match mode {
                    Apply::Real => window.set_maximized(m),
                    Apply::Fake { monitor, desktop } if m != was && !shadow.full_screen => {
                        if m {
                            if let Some(p) = position_of(window) {
                                shadow.restore.get_or_insert((p, (window.physical_width(), window.physical_height())));
                            }
                            let f = if window.decorations { frame } else { (0, 0) };
                            place(window, IVec2::new(monitor.0, monitor.1), ((monitor.2 - f.0).max(1) as u32, (monitor.3 - f.1).max(1) as u32), frame, desktop, "Maximized");
                        } else if let Some((p, s)) = shadow.restore.take() {
                            place(window, p, s, frame, desktop, "the restore after Maximized");
                        }
                    }
                    _ => {}
                }
            }
            WindowCommand::Minimized(m) => {
                shadow.minimized = m;
                match mode {
                    Apply::Real => window.set_minimized(m),
                    // minimising a window hands the activation to another one: not from a test window
                    Apply::Fake { .. } => eprintln!("[shell] window: Minimized({m}) is recorded, not applied (TERRARIA_NOACTIVATE: the driver minimises with SW_SHOWMINNOACTIVE)"),
                    _ => {}
                }
            }
            // a TERRARIA_NOACTIVATE window stays where it was put (the game centres its window / moves a
            // borderless one to the monitor's corner: onto the desktop), unless it has a monitor of its own
            WindowCommand::Location(x, y) => match mode {
                Apply::Passive => eprintln!("[shell] window: the game asked for Location({x}, {y}) (not applied: TERRARIA_NOACTIVATE)"),
                Apply::Fake { monitor, desktop } => {
                    // the game's (0, 0) is the fake monitor's corner
                    let size = (window.physical_width(), window.physical_height());
                    place(window, IVec2::new(x + monitor.0, y + monitor.1), size, frame, desktop, "Location");
                }
                _ => window.position = WindowPosition::At(IVec2::new(x, y)),
            },
            WindowCommand::Size(w, h) => match mode {
                Apply::Fake { desktop, .. } => {
                    sized = true;
                    // Form.Width / Height are the outer size: the client size is what is left inside the frame
                    let f = if window.decorations { frame } else { (0, 0) };
                    let pos = position_of(window).unwrap_or(IVec2::new(desktop.0 + desktop.2 + 240, desktop.1 + desktop.3 + 160));
                    place(window, pos, ((w - f.0).max(1) as u32, (h - f.1).max(1) as u32), frame, desktop, "Size");
                }
                _ => {
                    sized = true;
                    window.resolution.set_physical_resolution(w.max(1) as u32, h.max(1) as u32)
                }
            },
            WindowCommand::MinimumSize(w, h) => {
                let scale = window.scale_factor().max(0.01);
                window.resize_constraints.min_width = (w.max(1) as f32 / scale).max(1.0);
                window.resize_constraints.min_height = (h.max(1) as f32 / scale).max(1.0);
            }
            // winit has no portable "put behind"; focus is only taken in a normal run
            WindowCommand::BringToFront => {
                if mode == Apply::Real {
                    window.focused = true;
                }
            }
            WindowCommand::SendToBack => {}
            // no Bevy API for the task bar flash or SetThreadExecutionState: recorded by the host, nothing to do
            WindowCommand::FlashIcon | WindowCommand::KeepAwake(_) => {}
        }
    }
    if window.decorations != decorations_before && !shadow.full_screen {
        // the frame changed: the client area keeps its size (WinForms), or takes the one set in the same batch
        let size = if sized { (window.physical_width(), window.physical_height()) } else { size_before };
        shadow.keep_client = Some((size, 30));
    } else if sized && shadow.keep_client.is_some() {
        shadow.keep_client = Some(((window.physical_width(), window.physical_height()), 30));
    }
    winforms::report_window(shadow.maximized, shadow.borderless);
}

/// `GraphicsDeviceManager.ApplyChanges`: size, full screen, vsync. `display`: the size of the monitor the window
/// is on, as the game was told.
pub fn apply_request(r: WindowRequest, window: &mut Mut<Window>, shadow: &mut WindowShadow, mode: Apply, frame: (i32, i32), display: (i32, i32)) {
    let (mut w, mut h) = (r.width.max(1) as u32, r.height.max(1) as u32);
    if r.full_screen != shadow.full_screen {
        shadow.full_screen = r.full_screen;
        if matches!(mode, Apply::Real | Apply::Fake { .. }) {
            if r.full_screen {
                if window.physical_width() > 0 && window.physical_height() > 0 {
                    shadow.windowed_size = Some((window.physical_width(), window.physical_height()));
                }
            } else if let Some(size) = shadow.windowed_size.take() {
                if (r.width, r.height) == display {
                    eprintln!("[shell] window: leaving full screen: back to the {}x{} window it was (the game asks for the monitor's {}x{}, which it adopted while full screen)", size.0, size.1, r.width, r.height);
                    (w, h) = size;
                }
            }
        }
        match mode {
            Apply::Passive => eprintln!("[shell] window: the game asked for full screen = {} at {}x{} (not applied: TERRARIA_NOACTIVATE; a normal run sets WindowMode::{})", r.full_screen, r.width, r.height, if r.full_screen { "BorderlessFullscreen(Current)" } else { "Windowed" }),
            Apply::Real => {
                window.mode = if r.full_screen { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed };
                if !r.full_screen {
                    window.resolution.set_physical_resolution(w, h);
                }
            }
            Apply::Fake { monitor, desktop } => {
                eprintln!("[shell] window: the game asked for full screen = {} at {}x{}: a borderless window over the monitor {monitor:?}", r.full_screen, r.width, r.height);
                if r.full_screen {
                    if let Some(p) = position_of(window) {
                        shadow.restore.get_or_insert((p, (window.physical_width(), window.physical_height())));
                    }
                    if window.decorations {
                        window.decorations = false;
                    }
                    place(window, IVec2::new(monitor.0, monitor.1), (monitor.2 as u32, monitor.3 as u32), (0, 0), desktop, "full screen");
                    shadow.keep_client = Some(((monitor.2 as u32, monitor.3 as u32), 30));
                } else {
                    if window.decorations == shadow.borderless {
                        window.decorations = !shadow.borderless;
                    }
                    let (pos, _) = shadow.restore.take().unwrap_or((IVec2::new(monitor.0, monitor.1), (w, h)));
                    place(window, pos, (w, h), frame, desktop, "the window after full screen");
                    shadow.keep_client = Some(((w, h), 30));
                }
            }
            Apply::Hidden => {}
        }
    }
    // a fullscreen window takes the monitor's size; asking winit for another one is ignored
    let sized_by_monitor = shadow.full_screen && matches!(mode, Apply::Real | Apply::Fake { .. });
    if !sized_by_monitor && (window.physical_width(), window.physical_height()) != (w, h) {
        match mode {
            Apply::Fake { desktop, .. } => {
                if let Some(pos) = position_of(window) {
                    place(window, pos, (w, h), frame, desktop, "the back buffer size");
                }
            }
            _ => window.resolution.set_physical_resolution(w, h),
        }
        if shadow.keep_client.is_some() {
            shadow.keep_client = Some(((window.physical_width(), window.physical_height()), 30));
        }
    }
    let present = if r.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };
    if window.present_mode != present && mode != Apply::Hidden {
        window.present_mode = present;
    }
}

pub fn apply_cursor(visible: bool, cursor: &mut Mut<CursorOptions>) {
    if cursor.visible != visible {
        cursor.visible = visible;
    }
}

pub type MonitorQuery<'w, 's> = Query<'w, 's, (&'static Monitor, Has<PrimaryMonitor>)>;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::window::WindowResolution;

    const DESKTOP: Rect4 = (0, 0, 2560, 1440);
    const FAKE: Rect4 = (2800, 1600, 1920, 1080);
    const FRAME: (i32, i32) = (16, 39);

    fn world_with_window(decorations: bool) -> (World, Entity) {
        let mut world = World::new();
        let e = world.spawn(Window { resolution: WindowResolution::new(1280, 720).with_scale_factor_override(1.0), position: WindowPosition::At(IVec2::new(2800, 1600)), decorations, focused: false, ..default() }).id();
        (world, e)
    }

    fn geometry(w: &Window) -> (Option<IVec2>, (u32, u32), bool) {
        (position_of(w), (w.physical_width(), w.physical_height()), w.decorations)
    }

    #[test]
    fn fake_monitor_must_be_off_the_desktop() {
        assert!(check_fake_monitor(FAKE, DESKTOP).is_ok());
        assert!(check_fake_monitor((0, 0, 1920, 1080), DESKTOP).is_err());
        assert!(check_fake_monitor((2000, 1600, 1920, 1080), DESKTOP).is_ok(), "below the desktop");
        assert!(check_fake_monitor((2000, 1000, 1920, 1080), DESKTOP).is_err(), "overlaps the lower right corner");
        assert!(check_fake_monitor((2800, 1600, 640, 480), DESKTOP).is_err());
        assert!(check_fake_monitor((31000, 1600, 1920, 1080), DESKTOP).is_err());
        assert_eq!(fake_monitor_for(DESKTOP, (1920, 1080)), FAKE);
        assert_eq!(fake_monitor_for((0, 0, 30000, 1440), (1920, 1080)), (30240, 1600, 1760, 1080));
    }

    /// The game's default start (borderless over the monitor), then the frame back, maximise, restore, fullscreen
    /// and back: `Main.SetDisplayMode`'s commands (Main.cs:53204-53221, 67307-67453) against the fake monitor.
    #[test]
    fn fake_monitor_requests_are_applied_inside_it() {
        let (mut world, e) = world_with_window(true);
        let mode = Apply::Fake { monitor: FAKE, desktop: DESKTOP };
        let mut shadow = WindowShadow::default();
        let mut w = world.get_mut::<Window>(e).unwrap();
        // ApplyBorderlessResolution: Location = bounds origin, FormBorderStyle.None, Width / Height = bounds
        apply_commands(vec![WindowCommand::Location(0, 0), WindowCommand::Borderless(true), WindowCommand::Size(1920, 1080), WindowCommand::SendToBack, WindowCommand::BringToFront], &mut w, &mut shadow, mode, FRAME);
        assert_eq!(geometry(&w), (Some(IVec2::new(2800, 1600)), (1920, 1080), false), "borderless: the client area is the monitor; the game's (0, 0) is its corner");
        assert_eq!(shadow.keep_client, Some(((1920, 1080), 30)), "the size set with the border change is the one to keep");
        assert!(shadow.borderless && !w.focused, "never brought to the front");
        // windowed with a frame, 1280x720 outer
        apply_commands(vec![WindowCommand::Borderless(false), WindowCommand::Size(1280, 720)], &mut w, &mut shadow, mode, FRAME);
        assert_eq!(geometry(&w), (Some(IVec2::new(2800, 1600)), (1264, 681), true));
        // the frame alone: the client size is kept (WinForms); winit would keep the outer size, so the shell
        // insists until the window has it again
        apply_commands(vec![WindowCommand::Borderless(true)], &mut w, &mut shadow, mode, FRAME);
        assert_eq!((geometry(&w).2, shadow.keep_client), (false, Some(((1264, 681), 30))));
        w.resolution.set_physical_resolution(1280, 720); // what winit reports after taking the frame away
        assert_eq!(enforce_client_size(&mut w, &mut shadow), Some((1264, 681)));
        assert_eq!(geometry(&w).1, (1264, 681));
        apply_commands(vec![WindowCommand::Borderless(false)], &mut w, &mut shadow, mode, FRAME);
        shadow.keep_client = None;
        // a move inside the monitor is applied, one onto the desktop is refused
        apply_commands(vec![WindowCommand::Location(200, 100)], &mut w, &mut shadow, mode, FRAME);
        assert_eq!(position_of(&w), Some(IVec2::new(3000, 1700)));
        apply_commands(vec![WindowCommand::Location(-2800, -1600)], &mut w, &mut shadow, mode, FRAME);
        apply_commands(vec![WindowCommand::Location(-800, -600)], &mut w, &mut shadow, mode, FRAME);
        assert_eq!(position_of(&w), Some(IVec2::new(3000, 1700)), "the desktop is never touched");
        // maximise: the outer rectangle is the monitor; restore: back where it was
        apply_commands(vec![WindowCommand::Maximized(true)], &mut w, &mut shadow, mode, FRAME);
        assert_eq!(geometry(&w), (Some(IVec2::new(2800, 1600)), (1904, 1041), true));
        assert!(shadow.maximized);
        apply_commands(vec![WindowCommand::Maximized(false)], &mut w, &mut shadow, mode, FRAME);
        assert_eq!(geometry(&w), (Some(IVec2::new(3000, 1700)), (1264, 681), true));
        // minimise is recorded only
        apply_commands(vec![WindowCommand::Minimized(true)], &mut w, &mut shadow, mode, FRAME);
        assert!(shadow.minimized && geometry(&w).1 == (1264, 681));
        // fullscreen at 1024x768: a borderless window over the whole monitor, whatever resolution was asked for
        apply_request(WindowRequest { width: 1024, height: 768, full_screen: true, vsync: true }, &mut w, &mut shadow, mode, FRAME, (1920, 1080));
        assert_eq!(geometry(&w), (Some(IVec2::new(2800, 1600)), (1920, 1080), false));
        assert_eq!(w.mode, WindowMode::Windowed, "winit's fullscreen (which activates) is not used");
        // and back to a 1280x720 window with its frame, where it was (the game chose that size meanwhile)
        apply_request(WindowRequest { width: 1280, height: 720, full_screen: false, vsync: true }, &mut w, &mut shadow, mode, FRAME, (1920, 1080));
        assert_eq!(geometry(&w), (Some(IVec2::new(3000, 1700)), (1280, 720), true));
        // the usual round trip: the game adopted the monitor's size while full screen and asks for a window of
        // that size when it leaves; the window goes back to the size it had
        apply_request(WindowRequest { width: 1280, height: 720, full_screen: true, vsync: true }, &mut w, &mut shadow, mode, FRAME, (1920, 1080));
        assert_eq!(geometry(&w), (Some(IVec2::new(2800, 1600)), (1920, 1080), false));
        apply_request(WindowRequest { width: 1920, height: 1080, full_screen: false, vsync: true }, &mut w, &mut shadow, mode, FRAME, (1920, 1080));
        assert_eq!(geometry(&w), (Some(IVec2::new(3000, 1700)), (1280, 720), true));
        // a size that would reach the desktop from where the window is cannot happen inside the monitor; a
        // request far beyond it is still placed (the monitor is the game's limit, the desktop is the shell's)
        apply_request(WindowRequest { width: 1600, height: 900, full_screen: false, vsync: false }, &mut w, &mut shadow, mode, FRAME, (1920, 1080));
        assert_eq!((geometry(&w).1, w.present_mode), ((1600, 900), PresentMode::AutoNoVsync));
    }

    #[test]
    fn passive_and_hidden_runs_record_only() {
        for mode in [Apply::Passive, Apply::Hidden] {
            let (mut world, e) = world_with_window(true);
            let mut shadow = WindowShadow::default();
            let mut w = world.get_mut::<Window>(e).unwrap();
            apply_commands(vec![WindowCommand::Borderless(true), WindowCommand::Maximized(true), WindowCommand::BringToFront], &mut w, &mut shadow, mode, FRAME);
            apply_request(WindowRequest { width: 1280, height: 720, full_screen: true, vsync: true }, &mut w, &mut shadow, mode, FRAME, (1920, 1080));
            assert!(shadow.borderless && shadow.maximized && shadow.full_screen, "{mode:?}: the game is told it happened");
            assert_eq!(geometry(&w), (Some(IVec2::new(2800, 1600)), (1280, 720), true), "{mode:?}: nothing was applied");
            assert!(w.mode == WindowMode::Windowed && !w.focused);
        }
        // a normal run applies them through winit
        let (mut world, e) = world_with_window(true);
        let mut shadow = WindowShadow::default();
        let mut w = world.get_mut::<Window>(e).unwrap();
        apply_commands(vec![WindowCommand::Borderless(true), WindowCommand::Location(0, 0)], &mut w, &mut shadow, Apply::Real, FRAME);
        apply_request(WindowRequest { width: 1920, height: 1080, full_screen: true, vsync: true }, &mut w, &mut shadow, Apply::Real, FRAME, (1920, 1080));
        assert_eq!((w.decorations, position_of(&w), w.mode), (false, Some(IVec2::ZERO), WindowMode::BorderlessFullscreen(MonitorSelection::Current)));
        apply_request(WindowRequest { width: 1920, height: 1080, full_screen: false, vsync: true }, &mut w, &mut shadow, Apply::Real, FRAME, (1920, 1080));
        assert_eq!((w.mode, geometry(&w).1), (WindowMode::Windowed, (1280, 720)), "back to the window it was");
    }
}
