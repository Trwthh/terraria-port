//! The game's own P/Invokes (and the platform SDKs behind its optional integrations), answered the way the original
//! behaves when the thing is not there. Every function names the source line that handles the answer.
//!
//! **Platform services.** `SocialAPI.Initialize()` (Program.cs:224) is called without a mode, so a client starts in
//! `SocialMode.Steam`; the Steam modules then call Steamworks.NET. The game has a mode without any platform,
//! `SocialMode.None` (the default for a dedicated server without `-steam`, SocialAPI.cs:36-47): no module is
//! loaded, `SocialAPI.Cloud / Friends / Achievements / Overlay / Workshop / Network / Platform / DLCs` stay null and
//! every use of them is guarded (`AchievementManager.cs:43`, `Main.cs:4156` ...). **The port starts in that mode**:
//! its driver calls `SocialAPI.Initialize(SocialMode.None)` (REQUESTS.md) instead of `Program.Main`'s call. If
//! Steam mode is entered anyway, the answers below make the game do what it does without a running Steam client:
//! `SteamAPI.RestartAppIfNecessary` false and `SteamAPI.Init` false lead to `MessageBox.Show(Error.LaunchFromSteam)` and
//! `Environment.Exit(1)` (CoreSocialModule.cs:28-40): a clean refusal to start, not a crash. There is no Rail SDK
//! use outside the unused WeGame IPC code.
//!
//! **RGB peripherals** (`ReLogic.Peripherals.RGB.{Corsair, Logitech, Razer}.NativeMethods`): the vendor DLLs'
//! entry points throw `DllNotFoundException`, which every device group's `Initialize()` catches
//! (`catch (Exception)`: RazerDeviceGroup.cs:46, CorsairDeviceGroup.cs:69, LogitechDeviceGroup.cs:45) and turns into a
//! disabled group. SteelSeries talks HTTP to a local service through `GameSenseConnection`; with the host's network
//! stack not provided it fails inside that class's own error handling.
//!
//! **user32 / kernel32** (Main.cs): window-menu manipulation and console-window hiding are for the Windows
//! form / dedicated-server console and return the Win32 failure values for a missing window.

use std::cell::Cell;

use terraria_gen_rt::rt::{self, Str};

/// A vendor SDK that is not installed: what the CLR throws for an unloadable DLL in a `DllImport`.
pub fn dll_not_found(dll: &str) -> ! {
    rt::throw(rt::exc("System.DllNotFoundException", Str::new(&format!("Unable to load DLL '{dll}': The specified module could not be found. (Exception from HRESULT: 0x8007007E)"))))
}

// ------------------------------------------------------------------------------------------------ kernel32 / user32

thread_local! {
    static EXECUTION_STATE: Cell<u32> = const { Cell::new(ES_CONTINUOUS) };
}
pub const ES_CONTINUOUS: u32 = 0x8000_0000;
pub const ES_SYSTEM_REQUIRED: u32 = 1;
pub const ES_DISPLAY_REQUIRED: u32 = 2;

/// `SetThreadExecutionState(flags)` (Main.cs:5360, 5368): returns the previous state, remembers the new one and asks
/// the app to keep the system awake while `ES_SYSTEM_REQUIRED` / `ES_DISPLAY_REQUIRED` is set with `ES_CONTINUOUS`.
pub fn set_thread_execution_state(flags: u32) -> u32 {
    let previous = EXECUTION_STATE.with(|s| s.replace(flags));
    if flags & ES_CONTINUOUS != 0 {
        crate::winforms::push_keep_awake(flags & (ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED) != 0);
    }
    previous
}
/// `LoadLibrary(path)` (Main.cs:5353, the dedicated server's `-lib`): NULL, the failure value; the result is ignored.
pub fn load_library(_path: Str) -> isize {
    0
}
/// `FindWindow(null, title)` (Main.cs:5381): NULL, no such window.
pub fn find_window(_class: Str, _title: Str) -> isize {
    0
}
/// `ShowWindow(hwnd, cmd)` (Main.cs:5384): false (the previous visibility; nothing was changed).
pub fn show_window(_hwnd: isize, _cmd: i32) -> bool {
    false
}
/// `GetSystemMenu(hwnd, revert)` (Main.cs:6857): NULL.
pub fn get_system_menu(_hwnd: isize, _revert: bool) -> isize {
    0
}
/// `GetMenuItemCount(NULL)` (Main.cs:6858): -1, Win32's failure value for an invalid menu.
pub fn get_menu_item_count(_menu: isize) -> i32 {
    -1
}
/// `RemoveMenu(NULL, ...)` (Main.cs:6859): 0, failure.
pub fn remove_menu(_menu: isize, _position: i32, _flags: i32) -> i32 {
    0
}
/// `GetAsyncKeyState(vKey)` (Testing/QuickLoad.cs:349, shift held): the high bit when the key is down in the input
/// snapshot (`Keys` values are the Win32 virtual-key codes).
pub fn get_async_key_state(vkey: i32) -> i16 {
    if crate::input::keyboard_get_state().is_key_down(vkey) {
        i16::MIN
    } else {
        0
    }
}

// ------------------------------------------------------------------------------------------------ nfd.dll

/// `nfdresult_t`
pub const NFD_ERROR: i32 = 0;
pub const NFD_OKAY: i32 = 1;
pub const NFD_CANCEL: i32 = 2;

/// `NFD_OpenDialog` / `NFD_SaveDialog` / `NFD_PickFolder` (nativefiledialog.cs:73-124): the host opens no dialog;
/// the answer is the user pressing Cancel (`NFD_CANCEL`, no path), which every caller handles.
///
/// # Safety
/// `out_path` must be valid for one `isize` write (the address of the `out IntPtr` argument).
pub unsafe fn nfd_cancel(out_path: *mut isize) -> i32 {
    *out_path = 0;
    NFD_CANCEL
}
/// `NFD_OpenDialogMultiple`: the path set (three pointers) is zeroed; Cancel.
///
/// # Safety
/// `out` must be valid for 24 bytes (the `nfdpathset_t` of the argument).
pub unsafe fn nfd_cancel_multiple(out: *mut u8) -> i32 {
    core::ptr::write_bytes(out, 0, 3 * core::mem::size_of::<isize>());
    NFD_CANCEL
}
