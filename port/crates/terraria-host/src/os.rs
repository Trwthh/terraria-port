//! ReLogic's platform edge: `ReLogic.OS.Platform` and its services (`IPathService`, `IClipboard`,
//! `IWindowService`), `ReLogic.Localization.IME.IImeService`.
//!
//! In the original these are `ReLogic.OS.Windows.*` classes full of P/Invoke (user32, shell32, imm32, a WndProc
//! hook). They are replaced as a whole: `Platform.Get<T>()` hands out the native service objects below.
//!
//! - Paths: `GetStoragePath("Terraria")` is the save root (`sys::io::save_root`: `TERRARIA_SAVE_DIR`, default
//!   `<project>\work\port-save`), so `Main.SavePath` never points at the real `Documents\My Games\Terraria`.
//!   `OpenURL` and `MoveToRecycleBin` do not reach the shell: the first is `unimplemented`, the second deletes the
//!   file INSIDE the save root only by moving it to `<save root>\_recycled` (nothing is ever removed for good).
//! - Clipboard: a process-local string by default; `set_os_clipboard(true)` (the interactive app) bridges it to the
//!   Windows clipboard, text only (`CF_UNICODETEXT`).
//! - Window: title and scaling go through the `Game` window; flashing the task-bar icon is `unimplemented`.
//! - IME: characters typed (from `InputSnapshot::text`) are delivered to the key listeners by the driver
//!   (`game::dispatch_events`); there is no composition window, so the candidate list is always empty.

use std::cell::RefCell;

use terraria_gen_rt::host;
use terraria_gen_rt::rt::{self, Delegate, Gc, Marker, Rooted, Str, Trace};

use crate::game::GameWindow;
use crate::gfx::states::{plain_trace, throw};

/// `Platform.IsWindows` / `IsLinux` / `IsOSX`: the translated game is the Windows build.
pub const IS_WINDOWS: bool = true;
pub const IS_LINUX: bool = false;
pub const IS_OSX: bool = false;

/// `PlatformType` of `Platform.Current.Type`: Windows = 0.
pub const PLATFORM_TYPE_WINDOWS: i32 = 0;

// ------------------------------------------------------------------------------------------------ paths

/// `IPathService`
pub struct PathService;
plain_trace!(PathService, Clipboard, WindowService);

/// `IPathService.GetStoragePath(subfolder)`: the save root for "Terraria" (the game's only argument); another
/// subfolder is a sibling folder inside the save root's parent sandbox.
pub fn storage_path(subfolder: Str) -> Str {
    let root = crate::sys::io::save_root();
    match subfolder.get() {
        None | Some("Terraria") => Str::new(&root.to_string_lossy()),
        Some(other) => Str::new(&root.join("_storage").join(other).to_string_lossy()),
    }
}

impl PathService {
    pub fn get_storage_path(&self, subfolder: Str) -> Str {
        storage_path(subfolder)
    }
    /// `GetStoragePath()` (no subfolder): the folder that holds the "Terraria" save folder... the save root's
    /// parent is not ours to hand out, so the root itself; the game only calls the overload with a subfolder.
    pub fn get_storage_root(&self) -> Str {
        Str::new(&crate::sys::io::save_root().to_string_lossy())
    }
    /// `ExpandPathVariables(path)`: Windows `%NAME%` expansion from this process's environment; an unknown
    /// variable stays as written (what `Environment.ExpandEnvironmentVariables` does).
    pub fn expand_path_variables(&self, path: Str) -> Str {
        let Some(p) = path.get() else { throw("System.ArgumentNullException", "path") };
        let mut out = String::new();
        let mut rest = p;
        while let Some(start) = rest.find('%') {
            out.push_str(&rest[..start]);
            let after = &rest[start + 1..];
            match after.find('%') {
                Some(end) => match std::env::var(&after[..end]) {
                    Ok(v) if end > 0 => {
                        out.push_str(&v);
                        rest = &after[end + 1..];
                    }
                    _ => {
                        out.push('%');
                        out.push_str(&after[..end]);
                        rest = &after[end..];
                    }
                },
                None => {
                    out.push('%');
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        Str::new(&out)
    }
    /// `MoveToRecycleBin(path)`: see the module docs. A path outside the save root is refused.
    pub fn move_to_recycle_bin(&self, path: Str) -> bool {
        let Some(p) = path.get() else { throw("System.ArgumentNullException", "path") };
        let root = crate::sys::io::save_root();
        let src = std::path::PathBuf::from(p);
        let norm = |x: &std::path::Path| x.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
        if !norm(&src).starts_with(&(norm(&root) + "\\")) {
            host::unimplemented("ReLogic.OS.IPathService.MoveToRecycleBin(string) for a path outside the save root (not touched)");
            return false;
        }
        if !src.exists() {
            return false;
        }
        let bin = root.join("_recycled");
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
        let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        std::fs::create_dir_all(&bin).is_ok() && std::fs::rename(&src, bin.join(format!("{stamp}_{name}"))).is_ok()
    }
    /// `OpenURL(url)`: the host does not launch a browser.
    pub fn open_url(&self, _url: Str) {
        host::unimplemented("ReLogic.OS.IPathService.OpenURL(string) (the host does not launch processes)");
    }
}

// ------------------------------------------------------------------------------------------------ clipboard

thread_local! {
    static CLIPBOARD: RefCell<String> = const { RefCell::new(String::new()) };
    static SERVICES: RefCell<Option<Rooted<ServiceRoots>>> = const { RefCell::new(None) };
    static IME_LISTENERS: RefCell<Vec<Rooted<Delegate<dyn Fn(u16)>>>> = const { RefCell::new(Vec::new()) };
    /// the text an input method is composing (`set_ime_composition`)
    static IME_COMPOSITION: RefCell<String> = const { RefCell::new(String::new()) };
}

thread_local! {
    static OS_CLIPBOARD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// App: bridge `IClipboard` to the Windows clipboard (text only, `CF_UNICODETEXT`). Off by default so that tests and
/// headless runs never touch the user's clipboard; the interactive app turns it on.
pub fn set_os_clipboard(enabled: bool) {
    OS_CLIPBOARD.with(|c| c.set(enabled));
}

#[cfg(windows)]
mod win_clipboard {
    #[link(name = "user32")]
    extern "system" {
        fn OpenClipboard(hwnd: isize) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn GetClipboardData(format: u32) -> isize;
        fn SetClipboardData(format: u32, mem: isize) -> isize;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalAlloc(flags: u32, bytes: usize) -> isize;
        fn GlobalLock(mem: isize) -> *mut u8;
        fn GlobalUnlock(mem: isize) -> i32;
        fn GlobalFree(mem: isize) -> isize;
    }
    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 2;

    fn open() -> bool {
        // another program may hold the clipboard for a moment
        for _ in 0..10 {
            // SAFETY: plain Win32 call
            if unsafe { OpenClipboard(0) } != 0 {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        false
    }

    pub fn get() -> Option<String> {
        if !open() {
            return None;
        }
        // SAFETY: the clipboard is open; the handle's memory is a NUL-terminated UTF-16 string owned by the system
        let text = unsafe {
            let h = GetClipboardData(CF_UNICODETEXT);
            if h == 0 {
                None
            } else {
                let p = GlobalLock(h) as *const u16;
                if p.is_null() {
                    None
                } else {
                    let mut n = 0;
                    while *p.add(n) != 0 {
                        n += 1;
                    }
                    let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
                    GlobalUnlock(h);
                    Some(s)
                }
            }
        };
        // SAFETY: opened above
        unsafe { CloseClipboard() };
        Some(text.unwrap_or_default())
    }

    pub fn set(text: &str) -> bool {
        let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        if !open() {
            return false;
        }
        // SAFETY: the clipboard is open; the allocation is moved into the system on success, freed on failure
        let ok = unsafe {
            EmptyClipboard();
            let mem = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2);
            if mem == 0 {
                false
            } else {
                let p = GlobalLock(mem) as *mut u16;
                if p.is_null() {
                    GlobalFree(mem);
                    false
                } else {
                    core::ptr::copy_nonoverlapping(units.as_ptr(), p, units.len());
                    GlobalUnlock(mem);
                    if SetClipboardData(CF_UNICODETEXT, mem) == 0 {
                        GlobalFree(mem);
                        false
                    } else {
                        true
                    }
                }
            }
        };
        // SAFETY: opened above
        unsafe { CloseClipboard() };
        ok
    }
}

/// App side: replace / read the clipboard text: the Windows clipboard when [`set_os_clipboard`] is on and it
/// answers, the process-local string otherwise.
pub fn clipboard_set(text: &str) {
    #[cfg(windows)]
    if OS_CLIPBOARD.with(|c| c.get()) && win_clipboard::set(text) {
        return;
    }
    CLIPBOARD.with(|c| *c.borrow_mut() = text.to_owned());
}
pub fn clipboard_get() -> String {
    #[cfg(windows)]
    if OS_CLIPBOARD.with(|c| c.get()) {
        if let Some(t) = win_clipboard::get() {
            return t;
        }
    }
    CLIPBOARD.with(|c| c.borrow().clone())
}

/// `IClipboard`
pub struct Clipboard;
impl Clipboard {
    /// `Value.get`: one line; ReLogic's base class keeps only characters >= 32 and not 127.
    pub fn value(&self) -> Str {
        Str::new(&clipboard_get().chars().filter(|c| *c >= ' ' && *c != '\u{7f}').collect::<String>())
    }
    /// `Value.set`
    pub fn set_value(&mut self, v: Str) {
        clipboard_set(v.get().unwrap_or(""));
    }
    /// `MultiLineValue.get`: as `Value` but line feeds stay.
    pub fn multi_line_value(&self) -> Str {
        Str::new(&clipboard_get().chars().filter(|c| (*c >= ' ' && *c != '\u{7f}') || *c == '\n').collect::<String>())
    }
}

// ------------------------------------------------------------------------------------------------ window

/// `IWindowService`
pub struct WindowService;
impl WindowService {
    /// `GetScaling()`: the window's DPI scale (1.0 at 96 dpi).
    pub fn get_scaling(&self) -> f32 {
        crate::game::window_scale()
    }
    /// `SetUnicodeTitle(window, title)`
    pub fn set_unicode_title(&self, window: Gc<GameWindow>, title: Str) {
        // SAFETY: `ptr()` checks null.
        unsafe { (*window.ptr()).set_title(title) };
    }
    pub fn start_flashing_icon(&self, _window: Gc<GameWindow>) {
        host::unimplemented("ReLogic.OS.IWindowService.StartFlashingIcon(GameWindow)");
    }
    pub fn stop_flashing_icon(&self, _window: Gc<GameWindow>) {
        host::unimplemented("ReLogic.OS.IWindowService.StopFlashingIcon(GameWindow)");
    }
}

// ------------------------------------------------------------------------------------------------ IME

/// `IImeService`
pub struct ImeService {
    pub enabled: bool,
}
plain_trace!(ImeService);
impl ImeService {
    /// `AddKeyListener(Action<char>)`
    pub fn add_key_listener(&mut self, listener: Delegate<dyn Fn(u16)>) {
        if listener.is_null() {
            throw("System.ArgumentNullException", "listener");
        }
        IME_LISTENERS.with(|l| l.borrow_mut().push(Rooted::new(listener)));
    }
    pub fn remove_key_listener(&mut self, listener: Delegate<dyn Fn(u16)>) {
        IME_LISTENERS.with(|l| l.borrow_mut().retain(|r| r.get() != listener));
    }
    pub fn enable(&mut self) {
        self.enabled = true;
    }
    pub fn disable(&mut self) {
        self.enabled = false;
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    /// `CompositionString`: what the input method is composing, as the app reported it ([`set_ime_composition`]);
    /// empty when nothing is being composed. There is no candidate list (the app's window system does not
    /// hand one out).
    pub fn composition_string(&self) -> Str {
        IME_COMPOSITION.with(|c| Str::new(&c.borrow()))
    }
    pub fn is_candidate_list_visible(&self) -> bool {
        false
    }
    pub fn candidate_count(&self) -> u32 {
        0
    }
    pub fn selected_candidate(&self) -> u32 {
        0
    }
    /// `GetCandidate(index)`: empty (the list is empty; the original returns "" outside the list too).
    pub fn get_candidate(&self, _index: u32) -> Str {
        Str::lit("")
    }
}

/// App: has the game enabled the input method (`IImeService.Enable`: a text field is being edited)? The original
/// switches the IME on for its window then (`WindowsIme.OnEnable`); the app does the same with its window.
pub fn ime_enabled() -> bool {
    // SAFETY: the service object is rooted for the life of the thread.
    SERVICES.with(|s| s.borrow().as_ref().is_some_and(|r| unsafe { (*r.get().ime.ptr()).enabled }))
}
/// App: the text an input method is composing right now (empty: none), for `IImeService.CompositionString`.
pub fn set_ime_composition(text: &str) {
    IME_COMPOSITION.with(|c| {
        let mut c = c.borrow_mut();
        if *c != text {
            text.clone_into(&mut c);
        }
    });
}

/// Driver: delivers this frame's typed characters (UTF-16 units, in order) to every key listener, once.
/// Calls translated code.
pub fn ime_dispatch_text() {
    let mut snapshot = crate::input::snapshot();
    if snapshot.text.is_empty() {
        return;
    }
    let text = std::mem::take(&mut snapshot.text);
    crate::input::set_snapshot(snapshot);
    let listeners: Vec<Delegate<dyn Fn(u16)>> = IME_LISTENERS.with(|l| l.borrow().iter().map(|r| r.get()).collect());
    for c in text {
        let mut units = [0u16; 2];
        for u in c.encode_utf16(&mut units) {
            for l in &listeners {
                l.invoke(|f| f(*u));
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------ Platform

#[derive(Clone, Copy)]
struct ServiceRoots {
    path: Gc<PathService>,
    clipboard: Gc<Clipboard>,
    window: Gc<WindowService>,
    ime: Gc<ImeService>,
}
// SAFETY: every handle is reported.
unsafe impl Trace for ServiceRoots {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.path);
        m.mark(self.clipboard);
        m.mark(self.window);
        m.mark(self.ime);
    }
}

fn service_roots() -> ServiceRoots {
    SERVICES.with(|s| {
        if let Some(r) = s.borrow().as_ref() {
            return r.get();
        }
        let v = ServiceRoots { path: rt::alloc_init(PathService), clipboard: rt::alloc_init(Clipboard), window: rt::alloc_init(WindowService), ime: rt::alloc_init(ImeService { enabled: false }) };
        *s.borrow_mut() = Some(Rooted::new(v));
        v
    })
}

/// A service handle `Platform.Get<T>()` can hand out: `T` arrives as the Rust type of the C# type argument, which
/// for a mapped class is its handle (`Gc<PathService>`). One object per service and thread.
pub trait PlatformService: Copy + 'static {
    fn get() -> Self;
}
impl PlatformService for Gc<PathService> {
    fn get() -> Self {
        service_roots().path
    }
}
impl PlatformService for Gc<Clipboard> {
    fn get() -> Self {
        service_roots().clipboard
    }
}
impl PlatformService for Gc<WindowService> {
    fn get() -> Self {
        service_roots().window
    }
}
impl PlatformService for Gc<ImeService> {
    fn get() -> Self {
        service_roots().ime
    }
}

/// A service this host does not provide (`T` is then `object`: the interface is not a mapped type). `Platform.Get<T>()`
/// answers null for a service nobody registered, and the game guards the one such use
/// (`Platform.Get<IMouseNotifier>()?.AddMouseHandler(..)` in Program.cs:209, `?.ForceCursorHidden()` in FocusHelper.cs:169:
/// mouse hot-plug notifications of the WndProc hook, which this host has no source for).
impl PlatformService for rt::Obj {
    fn get() -> Self {
        Gc::NULL
    }
}

/// `Platform.Get<T>()`
pub fn platform_get<H: PlatformService>() -> H {
    H::get()
}

/// `Platform.InitializeClientServices(windowHandle)`: the original installs the WndProc hook and the IME here;
/// the services above exist from the start.
pub fn platform_initialize_client_services(_window_handle: isize) {
    service_roots();
}

#[cfg(test)]
mod clipboard_tests {
    use super::*;

    /// Process-local mode never touches the desktop clipboard.
    #[test]
    fn local_clipboard_by_default() {
        clipboard_set("hello\nworld");
        assert_eq!(clipboard_get(), "hello\nworld");
    }

    /// Opt-in (`TERRARIA_TEST_OS_CLIPBOARD=1`): writes through the Windows clipboard and puts the previous text back.
    #[test]
    fn os_clipboard_round_trip() {
        if std::env::var_os("TERRARIA_TEST_OS_CLIPBOARD").is_none() {
            eprintln!("skipped: set TERRARIA_TEST_OS_CLIPBOARD=1 (it replaces the clipboard text for a moment)");
            return;
        }
        set_os_clipboard(true);
        let old = clipboard_get();
        let probe = "terraria-host clipboard probe \u{e9}\u{65e5}\u{1f600}";
        clipboard_set(probe);
        assert_eq!(clipboard_get(), probe);
        clipboard_set(&old);
        set_os_clipboard(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::InputSnapshot;
    use std::rc::Rc;

    #[test]
    fn services_are_singletons_and_text_reaches_the_listeners() {
        assert_eq!(platform_get::<Gc<ImeService>>(), platform_get::<Gc<ImeService>>());
        let got = Rc::new(RefCell::new(Vec::new()));
        let g2 = got.clone();
        unsafe {
            let ime = &mut *platform_get::<Gc<ImeService>>().ptr();
            ime.add_key_listener(Delegate::new(Box::new(move |c: u16| g2.borrow_mut().push(c))));
            assert!(!ime.is_candidate_list_visible() && ime.candidate_count() == 0 && ime.composition_string().as_str().is_empty());
            assert!(!ime_enabled());
            ime.enable();
            assert!(ime_enabled());
            ime.disable();
            set_ime_composition("\u{304b}\u{3093}");
            assert_eq!(ime.composition_string().as_str(), "\u{304b}\u{3093}");
            set_ime_composition("");
            assert!(ime.composition_string().as_str().is_empty());
        }
        crate::input::set_snapshot(InputSnapshot { text: vec!['a', '\u{e9}', '\u{1F600}', '\r'], ..Default::default() });
        ime_dispatch_text();
        ime_dispatch_text();
        assert_eq!(*got.borrow(), [0x61, 0xE9, 0xD83D, 0xDE00, 13], "UTF-16 units, delivered once");
    }

    #[test]
    fn clipboard_and_paths() {
        unsafe {
            let c = &mut *platform_get::<Gc<Clipboard>>().ptr();
            c.set_value(Str::lit("line1\r\nline2\ttab"));
            assert_eq!(c.value().as_str(), "line1line2tab");
            assert_eq!(c.multi_line_value().as_str(), "line1\nline2tab");
            let p = &*platform_get::<Gc<PathService>>().ptr();
            let save = p.get_storage_path(Str::lit("Terraria"));
            assert_eq!(save.as_str(), crate::sys::io::save_root().to_string_lossy());
            std::env::set_var("TERRARIA_HOST_TEST_VAR", "X Y");
            assert_eq!(p.expand_path_variables(Str::lit("a\\%TERRARIA_HOST_TEST_VAR%\\%NOPE_NOT_SET%\\50%")).as_str(), "a\\X Y\\%NOPE_NOT_SET%\\50%");
            assert!(!p.move_to_recycle_bin(Str::lit("C:\\Windows\\notepad.exe")), "outside the save root: refused");
        }
    }
}
