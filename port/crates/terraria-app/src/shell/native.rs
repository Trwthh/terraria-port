//! What the shell takes from the window system directly, because the original takes it there and winit does not
//! hand it on: the characters of `WM_CHAR`, the virtual keys of the key messages, `WM_ACTIVATEAPP`, the pointer
//! position outside the client area, `Mouse.SetPosition`, the timer resolution.
//!
//! # The message hook
//! [`install`] subclasses the shell's OWN window (`SetWindowLongPtrW(GWLP_WNDPROC)`, on the thread that owns it;
//! every message goes on to winit's procedure unchanged). The original does the same on its window:
//! `ReLogic.OS.Windows.WindowsMessageHook` puts a `WndProc` in front of the form's and `WindowsIme` reads `WM_CHAR`
//! there (decompile\ReLogic\ReLogic\Localization\IME\WindowsIme.cs:120-122). What the hook sees goes through
//! [`MessageState::on_message`], which is plain data in, plain data out (the tests feed it message sequences):
//!
//! - **Text**: every `WM_CHAR` is one UTF-16 unit of typed text, in order, exactly once. This is the only text
//!   source of a window with the hook: characters from key presses (`TranslateMessage`, so the keyboard layout,
//!   Shift, AltGr, Caps Lock and dead keys are the system's work), characters posted by other programs, and the
//!   result string of an input method (`WM_IME_COMPOSITION` with `GCS_RESULTSTR`, read with
//!   `ImmGetCompositionStringW`, because winit answers that message itself and no `WM_CHAR` follows; in the
//!   original `DefWindowProc` turns it into `WM_IME_CHAR` and then `WM_CHAR`). `WM_SYSCHAR` (Alt + key) and
//!   `WM_UNICHAR` are not text: the original's filter reads message 258 only, and `DefWindowProcW` of a Unicode
//!   window does not turn `WM_UNICHAR` into `WM_CHAR` (posted to the port's window: nothing arrives, measured).
//!   The text of Bevy's `KeyboardInput` / `Ime::Commit` events is NOT used in such a window, so nothing arrives
//!   twice (measured: characters typed by key messages after posted ones arrive once each).
//! - **Keys**: `WM_KEYDOWN` / `WM_SYSKEYDOWN` set, `WM_KEYUP` / `WM_SYSKEYUP` clear the virtual key of `wParam`:
//!   the keys `GetKeyboardState` reports to XNA's `Keyboard.GetState` (reference\xna\Keyboard.cs), which are the
//!   layout's virtual keys (the key labelled Z on a German keyboard is `Keys.Z`). Shift, Control and Alt are
//!   stored by side (`VK_LSHIFT` .. `VK_RMENU`: scan code 0x36 is the right Shift, the extended bit is the right
//!   Control / Alt), because XNA's `Keys` has only those. `WM_KILLFOCUS` releases everything.
//! - **Focus**: `WM_ACTIVATEAPP` is the one message that changes XNA's `Game.IsActive`
//!   (reference\xna\WindowsGameForm.cs:263-270 -> `Game.HostActivated` / `HostDeactivated`); `WM_SIZE` says whether
//!   the window is minimised (`Form.WindowState`, which the game's `FocusHelper.UpdateFocus` reads itself).
//!   [`FocusMachine`] is that rule.
//!
//! # The pointer
//! XNA's `Mouse.GetState` is `GetCursorPos` + `ScreenToClient` at every call, with or without the focus
//! (reference\xna\Mouse.cs:26-49), so the position goes on outside the client area (negative, or beyond the size).
//! [`mouse_source`] decides where the shell takes the position from and [`set_position_action`] what
//! `Mouse.SetPosition` does; both are pure functions over [`WindowFacts`] so that the rule is tested without a
//! window (and without ever moving a pointer).

use std::cell::RefCell;

pub const WM_SIZE: u32 = 0x0005;
pub const WM_KILLFOCUS: u32 = 0x0008;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_ACTIVATEAPP: u32 = 0x001C;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_KEYUP: u32 = 0x0101;
pub const WM_CHAR: u32 = 0x0102;
pub const WM_DEADCHAR: u32 = 0x0103;
pub const WM_SYSKEYDOWN: u32 = 0x0104;
pub const WM_SYSKEYUP: u32 = 0x0105;
pub const WM_SYSCHAR: u32 = 0x0106;
pub const WM_UNICHAR: u32 = 0x0109;
pub const WM_IME_COMPOSITION: u32 = 0x010F;
pub const GCS_RESULTSTR: isize = 0x0800;
/// other focus messages, named for the tests' sequences (the machine ignores them, as XNA does)
pub const WM_ACTIVATE: u32 = 0x0006;
pub const WM_SETFOCUS: u32 = 0x0007;
pub const WM_NCACTIVATE: u32 = 0x0086;
pub const WM_SYSCOMMAND: u32 = 0x0112;

const VK_SHIFT: usize = 0x10;
const VK_CONTROL: usize = 0x11;
const VK_MENU: usize = 0x12;

/// XNA's `Game.IsActive` and what the game's own focus code reads next to it.
///
/// `Game.isActive` starts false and follows `WM_ACTIVATEAPP` alone: `wParam != 0` -> `HostActivated`, 0 ->
/// `HostDeactivated`; closing the form deactivates (reference\xna\WindowsGameWindow.cs:234-237). Neither
/// `WM_ACTIVATE`, `WM_NCACTIVATE`, `WM_SETFOCUS`, `WM_KILLFOCUS` nor minimising changes it. `Game.Tick` sleeps
/// `InactiveSleepTime` while it is false (reference\xna\Game.cs:318-321) and `DrawFrame` draws nothing while the
/// window is minimised (`Window.IsMinimized`: a client area of 0 x 0).
///
/// The game adds its own rule (decompile\Terraria\Terraria\FocusHelper.cs:137-171, [`Self::selected_application`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FocusMachine {
    /// `Game.IsActive`
    pub active: bool,
    /// `WM_SIZE` said `SIZE_MINIMIZED` last
    pub minimized: bool,
    /// `WM_SIZE` said `SIZE_MAXIMIZED` last
    pub maximized: bool,
}

impl FocusMachine {
    pub fn on_message(&mut self, msg: u32, wparam: usize) {
        match msg {
            WM_ACTIVATEAPP => self.active = wparam != 0,
            WM_CLOSE => self.active = false,
            // SIZE_RESTORED 0, SIZE_MINIMIZED 1, SIZE_MAXIMIZED 2 (3 and 4 are about other windows)
            WM_SIZE if wparam <= 2 => {
                self.minimized = wparam == 1;
                self.maximized = wparam == 2;
            }
            _ => {}
        }
    }

    /// `FocusHelper.IsSelectedApplication` after `UpdateFocus` on Windows: `Main.instance.IsActive`, or the
    /// game's form is `Form.ActiveForm` (the system's foreground window), and never while the form is minimised.
    pub fn selected_application(&self, form_is_foreground: bool) -> bool {
        (self.active || form_is_foreground) && !self.minimized
    }

    /// `wantsToPause` of `FocusHelper.UpdateFocus`: taken from `IsActive` BEFORE the form is looked at
    /// (FocusHelper.cs:143-145), for a single-player game that does not play unfocused.
    pub fn wants_to_pause(&self) -> bool {
        !self.active
    }
}

/// Everything the hook collects between two shell ticks.
pub struct MessageState {
    /// UTF-16 units of `WM_CHAR`, oldest first
    units: Vec<u16>,
    /// virtual keys that are down (by side for Shift / Control / Alt)
    keys: [bool; 256],
    pub focus: FocusMachine,
    /// a `WM_ACTIVATEAPP` was seen (before the first one the app seeds `focus.active` itself)
    pub saw_activate_app: bool,
    /// characters taken so far (diagnostics)
    pub chars_total: u64,
}

impl Default for MessageState {
    fn default() -> Self {
        MessageState { units: Vec::new(), keys: [false; 256], focus: FocusMachine::default(), saw_activate_app: false, chars_total: 0 }
    }
}

impl MessageState {
    /// One message of the window, before the window's own procedure sees it. `ime_result` is asked for the
    /// result string when the message is a `WM_IME_COMPOSITION` that carries one.
    pub fn on_message(&mut self, msg: u32, wparam: usize, lparam: isize, ime_result: &mut dyn FnMut() -> Option<Vec<u16>>) {
        match msg {
            WM_CHAR => self.units.push(wparam as u16),
            WM_IME_COMPOSITION if lparam & GCS_RESULTSTR != 0 => {
                if let Some(text) = ime_result() {
                    self.units.extend(text);
                }
            }
            WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP => {
                let down = matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN);
                let scan = (lparam >> 16) & 0xFF;
                let extended = (lparam >> 24) & 1 != 0;
                let vk = match wparam & 0xFF {
                    VK_SHIFT => {
                        if scan == 0x36 {
                            0xA1
                        } else {
                            0xA0
                        }
                    }
                    VK_CONTROL => 0xA2 + extended as usize,
                    VK_MENU => 0xA4 + extended as usize,
                    v => v,
                };
                self.keys[vk] = down;
            }
            WM_KILLFOCUS => self.keys = [false; 256],
            WM_ACTIVATEAPP => {
                self.saw_activate_app = true;
                self.focus.on_message(msg, wparam);
            }
            _ => self.focus.on_message(msg, wparam),
        }
    }

    /// The typed characters since the last call. A high surrogate whose low half has not arrived yet stays for
    /// the next call; a lone surrogate becomes U+FFFD.
    pub fn take_text(&mut self) -> Vec<char> {
        let mut keep = Vec::new();
        if let Some(last) = self.units.last() {
            if (0xD800..0xDC00).contains(last) {
                keep.push(self.units.pop().expect("checked"));
            }
        }
        let text: Vec<char> = char::decode_utf16(self.units.drain(..)).map(|r| r.unwrap_or('\u{FFFD}')).collect();
        self.units = keep;
        self.chars_total += text.len() as u64;
        text
    }

    /// The `Keys` values (virtual keys) that are down, ascending.
    pub fn keys_down(&self) -> Vec<i32> {
        (0..256).filter(|k| self.keys[*k as usize]).collect()
    }

    pub fn release_all_keys(&mut self) {
        self.keys = [false; 256];
    }
}

thread_local! {
    static STATE: RefCell<MessageState> = RefCell::new(MessageState::default());
    static HOOKED: std::cell::Cell<isize> = const { std::cell::Cell::new(0) };
}

/// The hook's state (the thread that owns the window).
pub fn with_state<R>(f: impl FnOnce(&mut MessageState) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// The window the hook is on (0: none).
pub fn hooked_window() -> isize {
    HOOKED.with(|h| h.get())
}

// ------------------------------------------------------------------------------------------------ the pointer

/// What kind of run this is, as far as the real pointer and the real focus are concerned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    /// a visible window a person plays in (`run-terraria.cmd`)
    Normal,
    /// `TERRARIA_NOACTIVATE`, `TERRARIA_HIDDEN` or a script: driven by a program; the real pointer and the real
    /// focus are somebody else's
    Test,
}

/// What the window system says right now (read-only queries).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowFacts {
    /// the window is the system's foreground window (`GetForegroundWindow() == hwnd`)
    pub foreground: bool,
    /// the window has the mouse capture (`GetCapture() == hwnd`: a button went down in it and is still held)
    pub captured: bool,
    /// the pointer in client pixels (`GetCursorPos` + `ScreenToClient`), if the system answered
    pub pointer_client: Option<(i32, i32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseSource {
    /// `GetCursorPos` + `ScreenToClient`, as XNA: follows the pointer outside the client area
    Pointer(i32, i32),
    /// the last mouse message of the window (stays where it was when the pointer leaves)
    Messages,
}

/// Where `Mouse.GetState` takes the position from.
///
/// XNA asks the system at every call and does not care about the focus. The shell does that while its window is
/// the foreground window or has the mouse capture; otherwise it keeps the position of the window's last mouse
/// message. The difference to XNA is not visible to the game: without the focus it keeps the old position itself
/// (`PlayerInput.MouseInput`, PlayerInput.cs:1400-1405: `new MouseState(MouseInfo.X, MouseInfo.Y, ...)`). A test
/// run never reads the pointer: its window is driven by posted mouse messages while the real pointer is the
/// user's, somewhere else.
pub fn mouse_source(kind: RunKind, facts: &WindowFacts) -> MouseSource {
    match (kind, facts.pointer_client) {
        (RunKind::Normal, Some((x, y))) if facts.foreground || facts.captured => MouseSource::Pointer(x, y),
        _ => MouseSource::Messages,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetPositionAction {
    /// move the real pointer to this client position (`ClientToScreen` + `SetCursorPos`, as XNA)
    MovePointer,
    /// the position is only the game's (and a script's cursor follows): nothing real moves
    Virtual(&'static str),
}

/// What `Mouse.SetPosition(x, y)` does. XNA moves the real pointer whenever it is called. The shell does so only
/// in a normal run while its window is the foreground window (the pointer is then the player's, in the game);
/// never in a test run and never from the background.
pub fn set_position_action(kind: RunKind, facts: &WindowFacts) -> SetPositionAction {
    match kind {
        RunKind::Test => SetPositionAction::Virtual("a test run never moves the real pointer"),
        RunKind::Normal if !facts.foreground => SetPositionAction::Virtual("the window is not the foreground window"),
        RunKind::Normal => SetPositionAction::MovePointer,
    }
}

// ------------------------------------------------------------------------------------------------ Win32

#[cfg(windows)]
mod win {
    use super::*;

    type Hwnd = isize;
    type WndProc = unsafe extern "system" fn(Hwnd, u32, usize, isize) -> isize;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct Rect {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
        fn CallWindowProcW(prev: isize, hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn GetForegroundWindow() -> Hwnd;
        fn GetCapture() -> Hwnd;
        fn GetCursorPos(p: *mut Point) -> i32;
        fn ScreenToClient(hwnd: Hwnd, p: *mut Point) -> i32;
        fn ClientToScreen(hwnd: Hwnd, p: *mut Point) -> i32;
        fn SetCursorPos(x: i32, y: i32) -> i32;
        fn GetSystemMetrics(index: i32) -> i32;
        fn GetWindowRect(hwnd: Hwnd, r: *mut Rect) -> i32;
        fn GetClientRect(hwnd: Hwnd, r: *mut Rect) -> i32;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn IsIconic(hwnd: Hwnd) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcessId() -> u32;
    }
    // no import library for imm32 in the toolchain's own set: the compiler makes one
    #[link(name = "imm32", kind = "raw-dylib")]
    extern "system" {
        fn ImmGetContext(hwnd: Hwnd) -> isize;
        fn ImmReleaseContext(hwnd: Hwnd, himc: isize) -> i32;
        fn ImmGetCompositionStringW(himc: isize, index: u32, buf: *mut core::ffi::c_void, len: u32) -> i32;
    }
    #[link(name = "winmm")]
    extern "system" {
        fn timeBeginPeriod(ms: u32) -> u32;
    }

    thread_local! {
        static PREVIOUS: std::cell::Cell<isize> = const { std::cell::Cell::new(0) };
    }
    const GWLP_WNDPROC: i32 = -4;

    fn ime_result_string(hwnd: Hwnd) -> Option<Vec<u16>> {
        // SAFETY: the window's own input context, released again; the buffer is as long as the system said.
        unsafe {
            let himc = ImmGetContext(hwnd);
            if himc == 0 {
                return None;
            }
            let bytes = ImmGetCompositionStringW(himc, GCS_RESULTSTR as u32, core::ptr::null_mut(), 0);
            let mut out = None;
            if bytes > 0 {
                let mut buf = vec![0u16; bytes as usize / 2];
                let got = ImmGetCompositionStringW(himc, GCS_RESULTSTR as u32, buf.as_mut_ptr().cast(), bytes as u32);
                if got > 0 {
                    buf.truncate(got as usize / 2);
                    out = Some(buf);
                }
            }
            ImmReleaseContext(hwnd, himc);
            out
        }
    }

    unsafe extern "system" fn hook_proc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
        // the state is not borrowed across the call into the window's own procedure (which can send messages
        // to the window, so this function is re-entered)
        let _ = STATE.try_with(|s| {
            if let Ok(mut s) = s.try_borrow_mut() {
                s.on_message(msg, wparam, lparam, &mut || ime_result_string(hwnd));
            }
        });
        let prev = PREVIOUS.with(|p| p.get());
        // SAFETY: the procedure that was the window's before `install`, with the message it was sent.
        unsafe { CallWindowProcW(prev, hwnd, msg, wparam, lparam) }
    }

    /// Puts the hook on the shell's own window. Call it on the thread that owns the window, once.
    pub fn install(hwnd: isize) -> bool {
        if hwnd == 0 || HOOKED.with(|h| h.get()) != 0 {
            return false;
        }
        let mut pid = 0u32;
        // SAFETY: plain queries; the window procedure is only replaced on a window of this process, on its thread.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid != GetCurrentProcessId() {
                return false;
            }
            let prev = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, hook_proc as WndProc as usize as isize);
            if prev == 0 {
                return false;
            }
            PREVIOUS.with(|p| p.set(prev));
        }
        HOOKED.with(|h| h.set(hwnd));
        true
    }

    pub fn facts(hwnd: isize) -> WindowFacts {
        if hwnd == 0 {
            return WindowFacts::default();
        }
        let mut p = Point::default();
        // SAFETY: read-only queries with valid out pointers.
        unsafe {
            let pointer = (GetCursorPos(&mut p) != 0 && ScreenToClient(hwnd, &mut p) != 0).then_some((p.x, p.y));
            WindowFacts { foreground: GetForegroundWindow() == hwnd, captured: GetCapture() == hwnd, pointer_client: pointer }
        }
    }

    /// `Mouse.SetPosition` on the real pointer. Refuses unless the window is the foreground window at this very
    /// moment (the caller has decided with `set_position_action`; this is the second lock).
    pub fn move_pointer(hwnd: isize, x: i32, y: i32) -> bool {
        let mut p = Point { x, y };
        // SAFETY: plain calls; the pointer is only moved while this window is the foreground window.
        unsafe { hwnd != 0 && GetForegroundWindow() == hwnd && ClientToScreen(hwnd, &mut p) != 0 && SetCursorPos(p.x, p.y) != 0 }
    }

    /// The virtual screen (every monitor): x, y, width, height.
    pub fn virtual_screen() -> (i32, i32, i32, i32) {
        // SAFETY: plain queries (SM_XVIRTUALSCREEN .. SM_CYVIRTUALSCREEN).
        unsafe { (GetSystemMetrics(76), GetSystemMetrics(77), GetSystemMetrics(78), GetSystemMetrics(79)) }
    }

    /// Outer rectangle and client size of the window: (x, y, width, height, client width, client height).
    pub fn window_rect(hwnd: isize) -> Option<(i32, i32, i32, i32, i32, i32)> {
        let (mut w, mut c) = (Rect::default(), Rect::default());
        // SAFETY: read-only queries with valid out pointers.
        unsafe { (hwnd != 0 && GetWindowRect(hwnd, &mut w) != 0 && GetClientRect(hwnd, &mut c) != 0).then_some((w.left, w.top, w.right - w.left, w.bottom - w.top, c.right, c.bottom)) }
    }

    pub fn is_iconic(hwnd: isize) -> bool {
        // SAFETY: read-only query.
        unsafe { hwnd != 0 && IsIconic(hwnd) != 0 }
    }

    /// The process (not only this window) owns the foreground window: what `WM_ACTIVATEAPP` last said, asked
    /// directly (for the moment the hook is installed, after the window was already activated).
    pub fn app_is_foreground() -> bool {
        let mut pid = 0u32;
        // SAFETY: read-only queries.
        unsafe {
            let h = GetForegroundWindow();
            h != 0 && GetWindowThreadProcessId(h, &mut pid) != 0 && pid == GetCurrentProcessId()
        }
    }

    /// `timeBeginPeriod(1)`: sleeps and waits of this process end on a 1 ms grid instead of the default 15.6 ms
    /// (the mixer thread's 2 ms sleeps, `InactiveSleepTime`, a script's waits). XNA's game loop does not sleep at
    /// all while active; the port has helper threads that do.
    pub fn fine_timer() -> bool {
        // SAFETY: plain call; the period ends with the process.
        unsafe { timeBeginPeriod(1) == 0 }
    }
}

#[cfg(windows)]
pub use win::{app_is_foreground, facts, fine_timer, install, is_iconic, move_pointer, virtual_screen, window_rect};

#[cfg(not(windows))]
mod other {
    use super::*;
    pub fn install(_hwnd: isize) -> bool {
        false
    }
    pub fn facts(_hwnd: isize) -> WindowFacts {
        WindowFacts::default()
    }
    pub fn move_pointer(_hwnd: isize, _x: i32, _y: i32) -> bool {
        false
    }
    pub fn virtual_screen() -> (i32, i32, i32, i32) {
        (0, 0, 0, 0)
    }
    pub fn window_rect(_hwnd: isize) -> Option<(i32, i32, i32, i32, i32, i32)> {
        None
    }
    pub fn is_iconic(_hwnd: isize) -> bool {
        false
    }
    pub fn app_is_foreground() -> bool {
        false
    }
    pub fn fine_timer() -> bool {
        false
    }
}
#[cfg(not(windows))]
pub use other::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// `lParam` of a key message: repeat count 1, the scan code, the extended bit, bit 31 for a release.
    fn key_lp(scan: isize, extended: bool, up: bool) -> isize {
        1 | (scan << 16) | ((extended as isize) << 24) | if up { 0xC000_0000 } else { 0 }
    }

    fn feed(s: &mut MessageState, msgs: &[(u32, usize, isize)]) {
        for (m, w, l) in msgs {
            s.on_message(*m, *w, *l, &mut || None);
        }
    }

    /// The messages Windows puts in a queue for these key strokes (US layout unless said otherwise), as
    /// `TranslateMessage` completes them. Text comes from WM_CHAR alone; keys are the virtual keys.
    #[test]
    fn text_is_every_wm_char_once_and_keys_are_virtual_keys() {
        let mut s = MessageState::default();
        // Shift + a -> 'A'
        feed(&mut s, &[(WM_KEYDOWN, 0x10, key_lp(0x2A, false, false)), (WM_KEYDOWN, 0x41, key_lp(0x1E, false, false)), (WM_CHAR, 'A' as usize, key_lp(0x1E, false, false))]);
        assert_eq!(s.keys_down(), [0x41, 0xA0], "A and LeftShift (VK_SHIFT itself is not a member of Keys)");
        assert_eq!(s.take_text(), ['A']);
        feed(&mut s, &[(WM_KEYUP, 0x41, key_lp(0x1E, false, true)), (WM_KEYUP, 0x10, key_lp(0x2A, false, true))]);
        assert!(s.keys_down().is_empty());
        // right Shift + 1 -> '!'
        feed(&mut s, &[(WM_KEYDOWN, 0x10, key_lp(0x36, false, false)), (WM_KEYDOWN, 0x31, key_lp(0x02, false, false)), (WM_CHAR, '!' as usize, 0)]);
        assert_eq!((s.keys_down(), s.take_text()), (vec![0x31, 0xA1], vec!['!']));
        s.release_all_keys();
        // AltGr + q on a German layout -> '@': Windows sends a left Control, then the right Alt (extended), and
        // the character arrives as WM_CHAR (not WM_SYSCHAR)
        feed(&mut s, &[(WM_KEYDOWN, 0x11, key_lp(0x1D, false, false)), (WM_KEYDOWN, 0x12, key_lp(0x38, true, false)), (WM_KEYDOWN, 0x51, key_lp(0x10, false, false)), (WM_CHAR, '@' as usize, 0)]);
        assert_eq!((s.keys_down(), s.take_text()), (vec![0x51, 0xA2, 0xA5], vec!['@']), "Q, LeftControl, RightAlt");
        s.release_all_keys();
        // dead key (acute, VK_OEM_6 on a German layout) then e -> one 'é'; the WM_DEADCHAR is not text
        feed(&mut s, &[(WM_KEYDOWN, 0xDD, key_lp(0x0D, false, false)), (WM_DEADCHAR, 0xB4, 0), (WM_KEYUP, 0xDD, key_lp(0x0D, false, true))]);
        assert!(s.take_text().is_empty());
        feed(&mut s, &[(WM_KEYDOWN, 0x45, key_lp(0x12, false, false)), (WM_CHAR, 0xE9, 0), (WM_KEYUP, 0x45, key_lp(0x12, false, true))]);
        assert_eq!(s.take_text(), ['\u{e9}']);
        // dead key then a letter it does not combine with: Windows sends both characters, both arrive
        feed(&mut s, &[(WM_KEYDOWN, 0xDD, 0), (WM_DEADCHAR, 0xB4, 0), (WM_KEYUP, 0xDD, 0), (WM_KEYDOWN, 0x58, 0), (WM_CHAR, 0xB4, 0), (WM_CHAR, 'x' as usize, 0), (WM_KEYUP, 0x58, 0)]);
        assert_eq!(s.take_text(), ['\u{b4}', 'x']);
        // a Cyrillic layout: the key is VK_Z, the character is 'я'
        feed(&mut s, &[(WM_KEYDOWN, 0x5A, key_lp(0x2C, false, false)), (WM_CHAR, 0x44F, 0)]);
        assert_eq!((s.keys_down(), s.take_text()), (vec![0x5A], vec!['\u{44f}']));
        s.release_all_keys();
        // the key labelled Z on a German keyboard (scan code of the US Y): VK_Z, 'z'
        feed(&mut s, &[(WM_KEYDOWN, 0x5A, key_lp(0x15, false, false)), (WM_CHAR, 'z' as usize, 0)]);
        assert_eq!((s.keys_down(), s.take_text()), (vec![0x5A], vec!['z']));
        s.release_all_keys();
        // numpad 1 with NumLock on: VK_NUMPAD1 and '1'; off: VK_END (not extended), no character
        feed(&mut s, &[(WM_KEYDOWN, 0x61, key_lp(0x4F, false, false)), (WM_CHAR, '1' as usize, 0)]);
        assert_eq!((s.keys_down(), s.take_text()), (vec![0x61], vec!['1']));
        s.release_all_keys();
        feed(&mut s, &[(WM_KEYDOWN, 0x23, key_lp(0x4F, false, false))]);
        assert_eq!((s.keys_down(), s.take_text()), (vec![0x23], vec![]));
        s.release_all_keys();
        // both Enter keys: VK_RETURN (the numpad one is extended), '\r' each; a held key repeats its character
        feed(&mut s, &[(WM_KEYDOWN, 0x0D, key_lp(0x1C, false, false)), (WM_CHAR, 13, 0), (WM_KEYUP, 0x0D, key_lp(0x1C, false, true)), (WM_KEYDOWN, 0x0D, key_lp(0x1C, true, false)), (WM_CHAR, 13, 0), (WM_KEYDOWN, 0x0D, key_lp(0x1C, true, false) | (1 << 30)), (WM_CHAR, 13, 0)]);
        assert_eq!((s.keys_down(), s.take_text()), (vec![0x0D], vec!['\r', '\r', '\r']));
        // Alt + key: WM_SYSKEYDOWN sets the key, WM_SYSCHAR is not text (the original reads WM_CHAR only)
        s.release_all_keys();
        feed(&mut s, &[(WM_SYSKEYDOWN, 0x12, key_lp(0x38, false, false)), (WM_SYSKEYDOWN, 0x0D, key_lp(0x1C, false, false) | (1 << 29)), (WM_SYSCHAR, 13, 0)]);
        assert_eq!((s.keys_down(), s.take_text()), (vec![0x0D, 0xA4], vec![]), "Alt+Enter: Enter and LeftAlt, no text");
        // losing the focus releases every key
        feed(&mut s, &[(WM_KILLFOCUS, 0, 0)]);
        assert!(s.keys_down().is_empty());
    }

    /// Characters that come without a key: posted by a program, made from an input method's result, or a pair
    /// of surrogates split over two ticks.
    #[test]
    fn characters_without_a_key_press() {
        let mut s = MessageState::default();
        feed(&mut s, &[(WM_CHAR, 'A' as usize, 1), (WM_CHAR, '!' as usize, 1), (WM_CHAR, 0xDF, 1), (WM_CHAR, 0x44F, 1), (WM_CHAR, 0x6F22, 1)]);
        assert_eq!(s.take_text(), ['A', '!', '\u{df}', '\u{44f}', '\u{6f22}']);
        assert!(s.keys_down().is_empty() && s.take_text().is_empty(), "taken once");
        // an input method commits two characters: WM_IME_COMPOSITION with GCS_RESULTSTR; winit answers the
        // message itself, so no WM_CHAR follows and the hook reads the string
        let mut asked = 0;
        s.on_message(WM_IME_COMPOSITION, 0, GCS_RESULTSTR | 0x0008, &mut || {
            asked += 1;
            Some("\u{6f22}\u{5b57}".encode_utf16().collect())
        });
        // a composition update without a result is not text
        s.on_message(WM_IME_COMPOSITION, 0, 0x0008, &mut || panic!("no result string in this message"));
        assert_eq!((asked, s.take_text()), (1, vec!['\u{6f22}', '\u{5b57}']));
        // U+1F600 as two WM_CHAR, the tick boundary between them
        feed(&mut s, &[(WM_CHAR, 'a' as usize, 0), (WM_CHAR, 0xD83D, 0)]);
        assert_eq!(s.take_text(), ['a']);
        feed(&mut s, &[(WM_CHAR, 0xDE00, 0)]);
        assert_eq!(s.take_text(), ['\u{1F600}']);
        // a lone low surrogate
        feed(&mut s, &[(WM_CHAR, 0xDE00, 0)]);
        assert_eq!(s.take_text(), ['\u{FFFD}']);
        assert_eq!(s.chars_total, 10);
    }

    const WA_INACTIVE: usize = 0;
    const WA_ACTIVE: usize = 1;
    const WA_CLICKACTIVE: usize = 2;
    const SC_MINIMIZE: usize = 0xF020;

    /// (IsActive, minimised, IsSelectedApplication with the given foreground answer)
    fn run(m: &mut FocusMachine, msgs: &[(u32, usize)], foreground: bool) -> (bool, bool, bool) {
        let mut s = MessageState { focus: *m, ..Default::default() };
        for (msg, w) in msgs {
            s.on_message(*msg, *w, 0, &mut || None);
        }
        *m = s.focus;
        (m.active, m.minimized, m.selected_application(foreground))
    }

    /// The sequences are the ones Windows documents for each action (the order of WM_NCACTIVATE, WM_ACTIVATE,
    /// WM_ACTIVATEAPP, WM_KILLFOCUS / WM_SETFOCUS, WM_SIZE); they were written down from the documentation, not
    /// recorded on this machine (recording them means taking the focus). The rule under test is XNA's: only
    /// WM_ACTIVATEAPP counts.
    #[test]
    fn game_is_active_follows_wm_activateapp_only() {
        let mut m = FocusMachine::default();
        assert!(!m.active, "Game.isActive starts false");
        // the window is shown and activated at start-up
        assert_eq!(run(&mut m, &[(WM_ACTIVATEAPP, 1), (WM_NCACTIVATE, 1), (WM_ACTIVATE, WA_ACTIVE), (WM_SETFOCUS, 0), (WM_SIZE, 0)], true), (true, false, true));
        // Alt+Tab away, and back
        assert_eq!(run(&mut m, &[(WM_NCACTIVATE, 0), (WM_ACTIVATE, WA_INACTIVE), (WM_ACTIVATEAPP, 0), (WM_KILLFOCUS, 0)], false), (false, false, false));
        assert!(m.wants_to_pause());
        assert_eq!(run(&mut m, &[(WM_ACTIVATEAPP, 1), (WM_NCACTIVATE, 1), (WM_ACTIVATE, WA_ACTIVE), (WM_SETFOCUS, 0)], true), (true, false, true));
        // a click on another program's window, then a click back into the game
        assert_eq!(run(&mut m, &[(WM_NCACTIVATE, 0), (WM_ACTIVATE, WA_INACTIVE), (WM_ACTIVATEAPP, 0), (WM_KILLFOCUS, 0)], false), (false, false, false));
        assert_eq!(run(&mut m, &[(WM_ACTIVATEAPP, 1), (WM_NCACTIVATE, 1), (WM_ACTIVATE, WA_CLICKACTIVE), (WM_SETFOCUS, 0)], true), (true, false, true));
        // a window of the SAME program takes the activation (a message box of the game): no WM_ACTIVATEAPP, the
        // game stays active; the form is not the foreground window, which does not matter while IsActive is true
        assert_eq!(run(&mut m, &[(WM_NCACTIVATE, 0), (WM_ACTIVATE, WA_INACTIVE), (WM_KILLFOCUS, 0)], false), (true, false, true));
        assert_eq!(run(&mut m, &[(WM_NCACTIVATE, 1), (WM_ACTIVATE, WA_ACTIVE), (WM_SETFOCUS, 0)], true), (true, false, true));
        // minimise from the task bar: the window is minimised first, then another window is activated
        assert_eq!(run(&mut m, &[(WM_SYSCOMMAND, SC_MINIMIZE), (WM_SIZE, 1)], true), (true, true, false), "minimised but not yet deactivated: not selected (the game's rule), still IsActive (XNA's)");
        assert_eq!(run(&mut m, &[(WM_NCACTIVATE, 0), (WM_ACTIVATE, WA_INACTIVE | (1 << 16)), (WM_ACTIVATEAPP, 0), (WM_KILLFOCUS, 0)], false), (false, true, false));
        // restore from the task bar
        assert_eq!(run(&mut m, &[(WM_ACTIVATEAPP, 1), (WM_NCACTIVATE, 1), (WM_ACTIVATE, WA_ACTIVE | (1 << 16)), (WM_SIZE, 0), (WM_SETFOCUS, 0)], true), (true, false, true));
        // Win+D: every window is minimised and the desktop takes the activation; again to bring them back
        assert_eq!(run(&mut m, &[(WM_SIZE, 1), (WM_NCACTIVATE, 0), (WM_ACTIVATE, WA_INACTIVE | (1 << 16)), (WM_ACTIVATEAPP, 0), (WM_KILLFOCUS, 0)], false), (false, true, false));
        assert_eq!(run(&mut m, &[(WM_SIZE, 0)], false), (false, false, false), "shown again without the activation: not active");
        assert_eq!(run(&mut m, &[(WM_ACTIVATEAPP, 1), (WM_NCACTIVATE, 1), (WM_ACTIVATE, WA_ACTIVE), (WM_SETFOCUS, 0)], true), (true, false, true));
        // a UAC prompt (the secure desktop takes over): the deactivation arrives, the WM_KILLFOCUS may come before
        // it, after it or not at all; the result is the same
        for order in [&[(WM_KILLFOCUS, 0usize), (WM_NCACTIVATE, 0), (WM_ACTIVATE, WA_INACTIVE), (WM_ACTIVATEAPP, 0)][..], &[(WM_NCACTIVATE, 0), (WM_ACTIVATE, WA_INACTIVE), (WM_ACTIVATEAPP, 0)][..]] {
            let mut u = m;
            assert_eq!(run(&mut u, order, false), (false, false, false));
            assert_eq!(run(&mut u, &[(WM_ACTIVATEAPP, 1), (WM_NCACTIVATE, 1), (WM_ACTIVATE, WA_ACTIVE), (WM_SETFOCUS, 0)], true), (true, false, true));
        }
        // the lock screen (Win+L): the input desktop changes and the window gets no activation message at all.
        // XNA goes on as the active game (it is not told); the foreground window is not the game's then
        assert_eq!(run(&mut m, &[], false), (true, false, true));
        // focus messages alone (what a test driver posted before this stage) change nothing
        let mut n = FocusMachine::default();
        assert_eq!(run(&mut n, &[(WM_NCACTIVATE, 1), (WM_SETFOCUS, 0), (WM_ACTIVATE, WA_ACTIVE)], false), (false, false, false));
        // ... but the game's own rule takes the foreground window as well (Form.ActiveForm)
        assert!(n.selected_application(true) && n.wants_to_pause());
        // closing the window deactivates
        assert_eq!(run(&mut m, &[(WM_CLOSE, 0)], true).0, false);
    }

    /// The pointer rules, over made-up window facts: no window and no pointer is involved.
    #[test]
    fn pointer_rules() {
        let inside = WindowFacts { foreground: true, captured: false, pointer_client: Some((100, 200)) };
        let outside = WindowFacts { pointer_client: Some((-40, 900)), ..inside };
        let background = WindowFacts { foreground: false, captured: false, pointer_client: Some((300, 300)) };
        let dragging_out = WindowFacts { foreground: false, captured: true, pointer_client: Some((1400, -5)) };
        let no_answer = WindowFacts { foreground: true, captured: false, pointer_client: None };
        // a normal run follows the pointer while the window is in front or holds the capture, also outside
        assert_eq!(mouse_source(RunKind::Normal, &inside), MouseSource::Pointer(100, 200));
        assert_eq!(mouse_source(RunKind::Normal, &outside), MouseSource::Pointer(-40, 900));
        assert_eq!(mouse_source(RunKind::Normal, &dragging_out), MouseSource::Pointer(1400, -5));
        assert_eq!(mouse_source(RunKind::Normal, &background), MouseSource::Messages);
        assert_eq!(mouse_source(RunKind::Normal, &no_answer), MouseSource::Messages);
        // a test run never reads it, whatever the facts say
        for f in [inside, outside, background, dragging_out, no_answer] {
            assert_eq!(mouse_source(RunKind::Test, &f), MouseSource::Messages);
            assert!(matches!(set_position_action(RunKind::Test, &f), SetPositionAction::Virtual(_)), "a test run never moves the pointer");
        }
        // Mouse.SetPosition moves the real pointer only for the foreground window of a normal run
        assert_eq!(set_position_action(RunKind::Normal, &inside), SetPositionAction::MovePointer);
        assert_eq!(set_position_action(RunKind::Normal, &outside), SetPositionAction::MovePointer);
        assert!(matches!(set_position_action(RunKind::Normal, &background), SetPositionAction::Virtual(_)));
        assert!(matches!(set_position_action(RunKind::Normal, &dragging_out), SetPositionAction::Virtual(_)));
    }
}
