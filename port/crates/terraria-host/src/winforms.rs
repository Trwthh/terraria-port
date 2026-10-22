//! `System.Windows.Forms` as the game calls it (`Main.SetDisplayMode`, `SetDisplayModeAsBorderless`,
//! `ApplyBorderlessResolution`, `WindowStateController`): `Control.FromHandle`, `Form` (location, client size, border
//! style, window state, minimum size, `BringToFront` / `SendToBack`), `Screen` (`AllScreens`, `FromPoint`, `Bounds`,
//! `WorkingArea`, `DeviceName`), `MessageBox.Show`, plus the display-mode list of `GraphicsAdapter`.
//!
//! There is one window. `Control.FromHandle(handle)` returns THE form for any handle (the game passes
//! `Window.Handle`, which is 0 when the app has no native handle to give). The form is a view of the host's window
//! state ([`crate::game::window_state`]: position, client size) plus the few properties WinForms keeps itself
//! (border style, window state, minimum size). Everything the game changes is queued as a [`WindowCommand`]; the app
//! takes them each frame ([`take_commands`]) and applies them to its window (Bevy `Window`: `decorations`, `position`,
//! `set_maximized`, `resize_constraints`, `resolution`). Until the app reports the new geometry, reads return the
//! value just set, so `SetDisplayMode`'s read-after-write logic sees its own writes.
//!
//! Monitors come from the app ([`set_monitors`]); without them one monitor of the reported display size at (0, 0)
//! is used. Display modes ([`set_display_modes`]) default to the standard resolutions that fit that monitor, plus
//! the monitor's own size.
//!
//! `MessageBox.Show` never opens a dialog: it is logged to stderr, recorded ([`message_boxes`]) and answered with the
//! default positive button (`OK`, `Yes`, `Retry` are 1, 6, 4) unless the app is interactive ([`set_interactive`]) and
//! has installed an answer through [`set_message_box_answer`], or the app shows dialogs itself
//! ([`set_message_box_presenter`]: a visible, unscripted run).

use std::cell::RefCell;

use terraria_gen_rt::rt::{self, Arr, Gc, List, Str};

use crate::gfx::states::{plain_trace, throw};
use crate::sys::drawing::{Point, Rectangle, Size};

/// `FormBorderStyle`
pub const BORDER_NONE: i32 = 0;
pub const BORDER_SIZABLE: i32 = 4;
/// `FormWindowState`
pub const WINDOW_NORMAL: i32 = 0;
pub const WINDOW_MINIMIZED: i32 = 1;
pub const WINDOW_MAXIMIZED: i32 = 2;

/// A change the game asked of its window.
#[derive(Clone, Debug, PartialEq)]
pub enum WindowCommand {
    /// `FormBorderStyle` set: decorations on / off
    Borderless(bool),
    Maximized(bool),
    Minimized(bool),
    /// `Form.Location = ` (desktop position of the window)
    Location(i32, i32),
    /// `Control.Width = ` / `Height = `: the new outer size (equal to the client size without a border)
    Size(i32, i32),
    MinimumSize(i32, i32),
    BringToFront,
    SendToBack,
    /// `IWindowService.StartFlashingIcon`
    FlashIcon,
    /// `SetThreadExecutionState`: keep the system awake (true) or allow sleep
    KeepAwake(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub device_name: String,
    pub bounds: Rectangle,
    pub working_area: Rectangle,
    pub primary: bool,
}

#[derive(Default)]
struct State {
    monitors: Vec<Monitor>,
    display_modes: Option<Vec<(i32, i32)>>,
    border_style: Option<i32>,
    window_state: i32,
    minimum_size: Size,
    location: Option<Point>,
    size: Option<Size>,
    commands: Vec<WindowCommand>,
    interactive: bool,
    answer: Option<i32>,
    boxes: Vec<MessageBoxRecord>,
    /// the window is minimised (`report_minimized`): `Form.WindowState` is `Minimized` whatever else was reported
    minimized: bool,
    /// the app's answer to "is the game's window the foreground window" (`set_foreground`); None: not known,
    /// `Game.IsActive` stands in
    foreground: Option<bool>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}
fn push(c: WindowCommand) {
    with(|s| s.commands.push(c));
}

// ------------------------------------------------------------------------------------------------ app side

/// App: the monitors, primary first or flagged.
pub fn set_monitors(m: Vec<Monitor>) {
    with(|s| s.monitors = m);
}
/// App: the resolutions the primary adapter supports (`GraphicsAdapter.SupportedDisplayModes`).
pub fn set_display_modes(m: Vec<(i32, i32)>) {
    with(|s| s.display_modes = Some(m));
}
/// App: the window's real state after it applied commands (clears the "just set" values).
pub fn report_window(maximized: bool, borderless: bool) {
    with(|s| {
        s.location = None;
        s.size = None;
        s.window_state = if maximized { WINDOW_MAXIMIZED } else { WINDOW_NORMAL };
        s.border_style = Some(if borderless { BORDER_NONE } else { BORDER_SIZABLE });
    });
}
/// App: the window is / is not minimised (`WM_SIZE` with `SIZE_MINIMIZED`, or the scripted `minimise`).
/// `Form.WindowState` then reads `Minimized`, which is what `FocusHelper.UpdateFocus` (FocusHelper.cs:144-150)
/// looks at: a minimised window is never the selected application, whatever `Game.IsActive` says.
pub fn report_minimized(minimized: bool) {
    with(|s| s.minimized = minimized);
}
/// App: whether the game's window is the system's foreground window (`GetForegroundWindow() == handle`), which is
/// what WinForms' `Form.ActiveForm` answers. `None` (the default, and what a run without a real window uses):
/// `Game.IsActive` stands in.
pub fn set_foreground(foreground: Option<bool>) {
    with(|s| s.foreground = foreground);
}
/// `IWindowService.StartFlashingIcon(window)`: the app flashes the task-bar button.
pub fn flash_icon() {
    push(WindowCommand::FlashIcon);
}
/// `SetThreadExecutionState` (pinvoke.rs): the app is asked to keep the system awake or not.
pub fn push_keep_awake(awake: bool) {
    push(WindowCommand::KeepAwake(awake));
}
/// App: the commands queued since the last call, oldest first.
pub fn take_commands() -> Vec<WindowCommand> {
    with(|s| std::mem::take(&mut s.commands))
}
/// App: it can show dialogs; `MessageBox.Show` then waits for [`set_message_box_answer`] instead of answering itself.
pub fn set_interactive(v: bool) {
    with(|s| s.interactive = v);
}
thread_local! {
    static PRESENTER: std::cell::Cell<Option<fn(&MessageBoxRecord) -> Option<i32>>> = const { std::cell::Cell::new(None) };
}
/// App: shows a message box to the user and returns the button (`DialogResult`), or `None` to let the host answer.
/// `MessageBox.Show` blocks in it, as the original blocks in the modal dialog.
pub fn set_message_box_presenter(f: Option<fn(&MessageBoxRecord) -> Option<i32>>) {
    PRESENTER.set(f);
}
/// App: the button the user pressed for the next / pending message box (`DialogResult`).
pub fn set_message_box_answer(result: i32) {
    with(|s| s.answer = Some(result));
}

#[derive(Clone, Debug, PartialEq)]
pub struct MessageBoxRecord {
    pub text: String,
    pub caption: String,
    pub buttons: i32,
    pub icon: i32,
}
/// Every `MessageBox.Show` so far (tests, diagnostics).
pub fn message_boxes() -> Vec<MessageBoxRecord> {
    with(|s| s.boxes.clone())
}

fn monitors() -> Vec<Monitor> {
    let m = with(|s| s.monitors.clone());
    if !m.is_empty() {
        return m;
    }
    let (w, h) = crate::game::display_mode();
    let r = Rectangle::new(0, 0, w.max(1), h.max(1));
    vec![Monitor { device_name: "\\\\.\\DISPLAY1".into(), bounds: r, working_area: r, primary: true }]
}

/// `GameWindow.ScreenDeviceName`: the primary monitor's device name.
pub fn primary_device_name() -> String {
    let m = monitors();
    m.iter().find(|m| m.primary).unwrap_or(&m[0]).device_name.clone()
}

/// The modes `GraphicsAdapter.SupportedDisplayModes` lists.
pub fn display_modes() -> Vec<(i32, i32)> {
    if let Some(m) = with(|s| s.display_modes.clone()) {
        return m;
    }
    let (w, h) = crate::game::display_mode();
    const STANDARD: [(i32, i32); 14] = [(640, 480), (800, 600), (1024, 768), (1152, 864), (1280, 720), (1280, 800), (1280, 1024), (1360, 768), (1366, 768), (1600, 900), (1680, 1050), (1920, 1080), (2560, 1440), (3840, 2160)];
    let mut v: Vec<(i32, i32)> = STANDARD.iter().copied().filter(|(a, b)| *a <= w && *b <= h).collect();
    if !v.contains(&(w, h)) {
        v.push((w, h));
    }
    v
}

// ------------------------------------------------------------------------------------------------ Form / Control

/// `System.Windows.Forms.Form` (also `Control`: the game only reaches controls through `FromHandle`).
pub struct Form;
plain_trace!(Form);

impl Form {
    /// `Control.FromHandle(handle)`: the one window.
    pub fn from_handle(_handle: isize) -> Gc<Form> {
        thread_local! { static FORM: RefCell<Option<rt::Rooted<Gc<Form>>>> = const { RefCell::new(None) }; }
        FORM.with(|f| {
            let mut f = f.borrow_mut();
            if f.is_none() {
                *f = Some(rt::Rooted::new(rt::alloc_init(Form)));
            }
            f.as_ref().expect("created above").get()
        })
    }
    /// `Form.ActiveForm`: the game's window while it has focus, else null (`FocusHelper` compares it with the
    /// game's form to decide whether input belongs to the game).
    pub fn active_form() -> Gc<Form> {
        if with(|s| s.foreground).unwrap_or_else(|| crate::game::window_state().active) {
            Self::from_handle(0)
        } else {
            Gc::NULL
        }
    }
    fn client() -> (Point, Size) {
        let w = crate::game::window_state();
        let (p, s) = with(|st| (st.location, st.size));
        (p.unwrap_or(Point::new(w.x, w.y)), s.unwrap_or(Size::new(w.width, w.height)))
    }
    /// `Location.get`
    pub fn location(&self) -> Point {
        Self::client().0
    }
    /// `Location.set`
    pub fn set_location(&mut self, p: Point) {
        with(|s| s.location = Some(p));
        push(WindowCommand::Location(p.x, p.y));
    }
    /// `ClientSize.get`
    pub fn client_size(&self) -> Size {
        Self::client().1
    }
    /// `Size.get` (outer size; without a border it is the client size)
    pub fn size(&self) -> Size {
        Self::client().1
    }
    pub fn width(&self) -> i32 {
        Self::client().1.width
    }
    pub fn height(&self) -> i32 {
        Self::client().1.height
    }
    /// `Width.set`
    pub fn set_width(&mut self, w: i32) {
        let s = Self::client().1;
        self.set_size(Size::new(w, s.height));
    }
    /// `Height.set`
    pub fn set_height(&mut self, h: i32) {
        let s = Self::client().1;
        self.set_size(Size::new(s.width, h));
    }
    pub fn set_size(&mut self, s: Size) {
        with(|st| {
            st.size = Some(s);
            // `Width = w; Height = h;` is one resize: a size command right behind another replaces it
            match st.commands.last_mut() {
                Some(WindowCommand::Size(w, h)) => (*w, *h) = (s.width, s.height),
                _ => st.commands.push(WindowCommand::Size(s.width, s.height)),
            }
        });
    }
    /// `Bounds.get`
    pub fn bounds(&self) -> Rectangle {
        let (p, s) = Self::client();
        Rectangle::new(p.x, p.y, s.width, s.height)
    }
    /// `FormBorderStyle.get`: sizable until set (the window the app opens has a frame).
    pub fn form_border_style(&self) -> i32 {
        with(|s| s.border_style.unwrap_or(BORDER_SIZABLE))
    }
    /// `FormBorderStyle.set`: only None (0) and Sizable (4) are meaningful to the app; other styles count as framed.
    pub fn set_form_border_style(&mut self, v: i32) {
        with(|s| s.border_style = Some(v));
        push(WindowCommand::Borderless(v == BORDER_NONE));
    }
    pub fn window_state(&self) -> i32 {
        with(|s| if s.minimized { WINDOW_MINIMIZED } else { s.window_state })
    }
    pub fn set_window_state(&mut self, v: i32) {
        with(|s| s.window_state = v);
        push(match v {
            WINDOW_MAXIMIZED => WindowCommand::Maximized(true),
            WINDOW_MINIMIZED => WindowCommand::Minimized(true),
            _ => WindowCommand::Maximized(false),
        });
    }
    pub fn minimum_size(&self) -> Size {
        with(|s| s.minimum_size)
    }
    pub fn set_minimum_size(&mut self, v: Size) {
        with(|s| s.minimum_size = v);
        push(WindowCommand::MinimumSize(v.width, v.height));
    }
    pub fn bring_to_front(&mut self) {
        push(WindowCommand::BringToFront);
    }
    pub fn send_to_back(&mut self) {
        push(WindowCommand::SendToBack);
    }
}

// ------------------------------------------------------------------------------------------------ Screen

/// `System.Windows.Forms.Screen`
pub struct Screen {
    pub bounds: Rectangle,
    pub working_area: Rectangle,
    pub device_name: Str,
    pub primary: bool,
}
// SAFETY: the name is reported.
unsafe impl rt::Trace for Screen {
    unsafe fn trace(&self, m: &mut rt::Marker) {
        self.device_name.trace(m)
    }
}
impl Screen {
    fn make(m: &Monitor) -> Gc<Screen> {
        rt::alloc_init(Screen { bounds: m.bounds, working_area: m.working_area, device_name: Str::new(&m.device_name), primary: m.primary })
    }
    /// `Screen.AllScreens`
    pub fn all_screens() -> Arr<Gc<Screen>> {
        let v: Vec<Gc<Screen>> = monitors().iter().map(Self::make).collect();
        Arr::from_slice(&v)
    }
    /// `Screen.PrimaryScreen`
    pub fn primary_screen() -> Gc<Screen> {
        let m = monitors();
        Self::make(m.iter().find(|m| m.primary).unwrap_or(&m[0]))
    }
    /// `Screen.FromPoint(point)`: the monitor containing the point, else the nearest one (WinForms'
    /// `MONITOR_DEFAULTTONEAREST`).
    pub fn from_point(p: Point) -> Gc<Screen> {
        let m = monitors();
        if let Some(hit) = m.iter().find(|m| m.bounds.contains(p)) {
            return Self::make(hit);
        }
        let dist = |m: &Monitor| {
            let dx = (m.bounds.x - p.x).max(0).max(p.x - m.bounds.right() + 1) as i64;
            let dy = (m.bounds.y - p.y).max(0).max(p.y - m.bounds.bottom() + 1) as i64;
            dx * dx + dy * dy
        };
        Self::make(m.iter().min_by_key(|m| dist(m)).expect("at least one monitor"))
    }
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }
    pub fn working_area(&self) -> Rectangle {
        self.working_area
    }
    pub fn device_name(&self) -> Str {
        self.device_name
    }
    pub fn primary(&self) -> bool {
        self.primary
    }
}

/// `Rectangle.IntersectsWith(Rectangle)`
pub fn rectangle_intersects_with(a: Rectangle, b: Rectangle) -> bool {
    b.x < a.right() && a.x < b.right() && b.y < a.bottom() && a.y < b.bottom()
}

// ------------------------------------------------------------------------------------------------ MessageBox

/// `MessageBox.Show(text, caption, buttons, icon)` -> `DialogResult`. See the module docs. `buttons`:
/// OK 0, OKCancel 1, AbortRetryIgnore 2, YesNoCancel 3, YesNo 4, RetryCancel 5.
pub fn message_box_show(text: Str, caption: Str, buttons: i32, icon: i32) -> i32 {
    let rec = MessageBoxRecord { text: text.get().unwrap_or("").to_owned(), caption: caption.get().unwrap_or("").to_owned(), buttons, icon };
    terraria_gen_rt::host::log(&format!("[MessageBox] {}: {}", rec.caption, rec.text));
    let shown = PRESENTER.get().and_then(|f| f(&rec));
    let answer = with(|s| {
        s.boxes.push(rec);
        if shown.is_some() {
            shown
        } else if s.interactive {
            s.answer.take()
        } else {
            None
        }
    });
    answer.unwrap_or(match buttons {
        3 | 4 => 6,       // Yes
        2 => 3,           // Abort... the first button of AbortRetryIgnore is Abort (3)
        5 => 4,           // Retry
        _ => 1,           // OK
    })
}
pub fn message_box_show_text(text: Str) -> i32 {
    message_box_show(text, Str::lit(""), 0, 0)
}
pub fn message_box_show_caption(text: Str, caption: Str) -> i32 {
    message_box_show(text, caption, 0, 0)
}

// ------------------------------------------------------------------------------------------------ DisplayModeCollection

/// `GraphicsAdapter.SupportedDisplayModes`: the modes as an array of `DisplayMode`.
pub fn supported_display_modes() -> Gc<List<Gc<crate::game::DisplayMode>>> {
    let l = List::<Gc<crate::game::DisplayMode>>::new_gc();
    for (w, h) in display_modes() {
        // SAFETY: fresh list
        unsafe { (*l.ptr()).add(rt::alloc_init(crate::game::DisplayMode { width: w, height: h })) };
    }
    l
}

#[allow(dead_code)]
fn _unused() {
    let _ = throw;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Main.SetDisplayMode` for a borderless window, as the game writes it (Main.cs:53192 - 53221, 67310 - 67453).
    fn init_window() {
        crate::game::init(crate::game::WindowState { width: 1280, height: 720, x: 100, y: 80, active: true, handle: 0, display_width: 1920, display_height: 1080, scale: 1.0 });
    }

    #[test]
    fn borderless_switch_through_the_form() {
        init_window();
        take_commands();
        set_monitors(vec![
            Monitor { device_name: "\\\\.\\DISPLAY1".into(), bounds: Rectangle::new(0, 0, 1920, 1080), working_area: Rectangle::new(0, 0, 1920, 1040), primary: true },
            Monitor { device_name: "\\\\.\\DISPLAY2".into(), bounds: Rectangle::new(1920, 0, 2560, 1440), working_area: Rectangle::new(1920, 0, 2560, 1400), primary: false },
        ]);
        unsafe {
            let f = &mut *Form::from_handle(0).ptr();
            assert_eq!(Form::from_handle(12345), Form::from_handle(0), "one window");
            assert_eq!(f.form_border_style(), BORDER_SIZABLE);
            assert_eq!(f.window_state(), WINDOW_NORMAL);
            // SetDisplayModeAsBorderless: Screen.FromPoint(form.Location).Bounds
            f.set_location(Point::new(2000, 100));
            let b = (*Screen::from_point(f.location()).ptr()).bounds();
            assert_eq!((b.x, b.y, b.width, b.height), (1920, 0, 2560, 1440), "the window is on the second monitor");
            // ApplyBorderlessResolution
            f.set_location(Point::new(b.x, b.y));
            f.set_form_border_style(BORDER_NONE);
            f.set_width(b.width);
            f.set_height(b.height);
            assert_eq!((f.width(), f.height(), f.location()), (2560, 1440, Point::new(1920, 0)), "reads see the writes");
            f.send_to_back();
            f.bring_to_front();
            f.set_minimum_size(Size::new(800, 600));
            f.set_window_state(WINDOW_MAXIMIZED);
        }
        let cmds = take_commands();
        assert_eq!(
            cmds,
            vec![
                WindowCommand::Location(2000, 100),
                WindowCommand::Location(1920, 0),
                WindowCommand::Borderless(true),
                WindowCommand::Size(2560, 1440),
                WindowCommand::SendToBack,
                WindowCommand::BringToFront,
                WindowCommand::MinimumSize(800, 600),
                WindowCommand::Maximized(true),
            ]
        );
        // a point off every monitor: the nearest one
        let near = unsafe { (*Screen::from_point(Point::new(-50, 500)).ptr()).device_name() };
        assert_eq!(near.as_str(), "\\\\.\\DISPLAY1");
        assert_eq!(unsafe { (*Screen::all_screens().as_slice()[1].ptr()).primary() }, false);
        assert!(unsafe { (*Screen::primary_screen().ptr()).primary() });
        // the app reports the real state: the "just set" values are dropped
        report_window(true, true);
        unsafe {
            assert_eq!((*Form::from_handle(0).ptr()).window_state(), WINDOW_MAXIMIZED);
            assert_eq!((*Form::from_handle(0).ptr()).form_border_style(), BORDER_NONE);
            // minimised: reported on its own, and it wins; restored: what was there before
            report_minimized(true);
            assert_eq!((*Form::from_handle(0).ptr()).window_state(), WINDOW_MINIMIZED);
            report_minimized(false);
            assert_eq!((*Form::from_handle(0).ptr()).window_state(), WINDOW_MAXIMIZED);
            // Form.ActiveForm: Game.IsActive until the app knows the foreground window
            assert_eq!(Form::active_form(), Form::from_handle(0), "the test window is active");
            set_foreground(Some(false));
            assert!(Form::active_form().is_null());
            set_foreground(Some(true));
            assert_eq!(Form::active_form(), Form::from_handle(0));
            set_foreground(None);
        }
        set_monitors(Vec::new());
    }

    #[test]
    fn message_box_is_logged_and_answered_without_a_dialog() {
        let before = message_boxes().len();
        assert_eq!(message_box_show(Str::lit("Run it from Steam"), Str::lit("Error"), 0, 16), 1);
        assert_eq!(message_box_show(Str::lit("Delete?"), Str::lit("Q"), 4, 32), 6);
        let boxes = message_boxes();
        assert_eq!(boxes.len(), before + 2);
        assert_eq!(boxes[before].caption, "Error");
        // an interactive app supplies the answer
        set_interactive(true);
        set_message_box_answer(7);
        assert_eq!(message_box_show(Str::lit("Delete?"), Str::lit("Q"), 4, 32), 7);
        set_interactive(false);
    }

    #[test]
    fn display_modes_fit_the_monitor() {
        init_window();
        let m = display_modes();
        assert!(!m.is_empty() && m.iter().all(|(w, h)| *w >= 640 && *h >= 480));
        set_display_modes(vec![(800, 600), (1920, 1080)]);
        let l = supported_display_modes();
        assert_eq!(unsafe { (*l.ptr()).count() }, 2);
        with(|s| s.display_modes = None);
    }

    #[test]
    fn rectangles() {
        let a = Rectangle::new(0, 0, 100, 100);
        assert!(rectangle_intersects_with(a, Rectangle::new(50, 50, 100, 100)));
        assert!(!rectangle_intersects_with(a, Rectangle::new(100, 0, 10, 10)));
    }
}
