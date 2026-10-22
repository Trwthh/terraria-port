//! `Console`, `Environment`, `Guid`, `System.Random`, `Process`, the registry, the debugger.

use std::cell::RefCell;
use std::io::Write;

use terraria_gen_rt::host;
use terraria_gen_rt::rt::{self, Arr, Gc, Obj, Str};

use crate::gfx::states::{plain_trace, throw};

// ------------------------------------------------------------------------------------------------ Console

thread_local! {
    static CONSOLE_TITLE: RefCell<String> = const { RefCell::new(String::new()) };
    /// When set, console output is appended here instead of stdout (tests, an in-game console).
    static CONSOLE_CAPTURE: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Capture console output in memory (`true`) or send it to stdout again (`false`).
pub fn console_capture(on: bool) {
    CONSOLE_CAPTURE.with(|c| *c.borrow_mut() = if on { Some(String::new()) } else { None });
}
/// What was captured since [`console_capture`]`(true)` or the last call.
pub fn console_take_captured() -> String {
    CONSOLE_CAPTURE.with(|c| c.borrow_mut().as_mut().map(std::mem::take).unwrap_or_default())
}

fn out(text: &str) {
    let captured = CONSOLE_CAPTURE.with(|c| match c.borrow_mut().as_mut() {
        Some(buf) => {
            buf.push_str(text);
            true
        }
        None => false,
    });
    if !captured {
        let mut o = std::io::stdout().lock();
        let _ = o.write_all(text.as_bytes());
        let _ = o.flush();
    }
}

/// `Console.Write(string)`: null writes nothing.
pub fn console_write(value: Str) {
    out(value.get().unwrap_or(""));
}
/// `Console.WriteLine(string)`
pub fn console_write_line(value: Str) {
    out(value.get().unwrap_or(""));
    out("\r\n");
}
/// `Console.WriteLine()`
pub fn console_write_line_empty() {
    out("\r\n");
}
/// `Console.Title.set`: kept (a GUI process has no console window to title).
pub fn console_set_title(value: Str) {
    CONSOLE_TITLE.with(|t| *t.borrow_mut() = value.get().unwrap_or("").to_owned());
}
pub fn console_title() -> Str {
    CONSOLE_TITLE.with(|t| Str::new(&t.borrow()))
}
/// `Console.ReadLine()`: a line from stdin without its terminator; null at end of input.
///
/// The browser build (WASI): stdin is the page's console (web/www/HOST_ABI.md "Server console": the server Worker's
/// fd 0 answers EAGAIN while no line was typed). A line is taken when it is complete ([`stdin_lines`]); until then a
/// work fiber gives the frame back (`threading::wait_a_frame`), anything else gets null as at the end of input.
pub fn console_read_line() -> Str {
    #[cfg(target_os = "wasi")]
    loop {
        match stdin_lines::try_line() {
            Some(Some(line)) => return Str::new(&line),
            Some(None) => return Str::NULL,
            None => {
                if !super::threading::wait_a_frame() {
                    return Str::NULL;
                }
            }
        }
    }
    #[cfg(not(target_os = "wasi"))]
    {
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(0) | Err(_) => Str::NULL,
            Ok(_) => Str::new(line.trim_end_matches(['\r', '\n'])),
        }
    }
}

/// `Console.ReadLine()` inside a resumable form (the translator's config "resumable" / "await": the browser build's
/// `Main.ReadLineInput` of the dedicated server's input thread `Main.startDedInputCallBack`): while no line has been
/// typed into the page's console, the thread's machine parks (`threading::park_async`, polled again at the next
/// pump) instead of keeping a suspended stack. At the end of input it parks once, then returns null (the game's
/// `ReadLineInput` loops while null: a closed stdin costs one poll per pump, not a spinning fiber). Natively the
/// resumable forms are never called; there it is [`console_read_line`].
pub async fn console_read_line_async() -> Str {
    #[cfg(target_os = "wasi")]
    loop {
        match stdin_lines::try_line() {
            Some(Some(line)) => return Str::new(&line),
            Some(None) => {
                super::threading::park_async().await;
                return Str::NULL;
            }
            None => super::threading::park_async().await,
        }
    }
    #[cfg(not(target_os = "wasi"))]
    console_read_line()
}

/// Lines of WASI fd 0 read without blocking (the browser build: the server Worker's stdin, which answers EAGAIN
/// while nothing was typed). Not `std::io::stdin()`: its buffered `read_line` cannot tell "nothing yet" from a
/// partial line.
#[cfg(target_os = "wasi")]
mod stdin_lines {
    use std::cell::{Cell, RefCell};
    use std::io::Read;

    thread_local! {
        static BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
        static EOF: Cell<bool> = const { Cell::new(false) };
    }

    /// `Some(Some(line))` a whole line (without `\n` / `\r\n`); `Some(None)` the end of input; `None` no whole line
    /// yet (the Worker answered EAGAIN).
    pub fn try_line() -> Option<Option<String>> {
        BUF.with(|b| {
            let mut buf = b.borrow_mut();
            loop {
                if let Some(i) = buf.iter().position(|c| *c == b'\n') {
                    let mut line: Vec<u8> = buf.drain(..=i).collect();
                    line.pop();
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                    return Some(Some(String::from_utf8_lossy(&line).into_owned()));
                }
                if EOF.get() {
                    if buf.is_empty() {
                        return Some(None);
                    }
                    let rest: Vec<u8> = buf.drain(..).collect();
                    return Some(Some(String::from_utf8_lossy(&rest).into_owned()));
                }
                let mut chunk = [0u8; 1024];
                // SAFETY: fd 0 is the process's stdin for its whole life; ManuallyDrop never closes it.
                let mut stdin = std::mem::ManuallyDrop::new(unsafe { <std::fs::File as std::os::fd::FromRawFd>::from_raw_fd(0) });
                match stdin.read(&mut chunk) {
                    Ok(0) => EOF.set(true),
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::Interrupted => return None,
                    Err(_) => EOF.set(true),
                }
            }
        })
    }
}
/// `Console.Clear()`: on a terminal the screen is cleared (ANSI: erase display, cursor home). When the output is
/// not a console (redirected to a file or a pipe, or the process has none) .NET throws `IOException` ("The handle
/// is invalid."), and so does this: every call of the game is inside `try { Console.Clear(); } catch { }`
/// (Main.DedServ).
pub fn console_clear() {
    use std::io::IsTerminal;
    let out = std::io::stdout();
    if out.is_terminal() {
        let mut o = out.lock();
        let _ = o.write_all(b"[2J[H");
        let _ = o.flush();
    } else {
        rt::throw(rt::exc("System.IO.IOException", Str::lit("The handle is invalid.")))
    }
}

// ------------------------------------------------------------------------------------------------ Environment

/// `Environment.NewLine`
pub const NEW_LINE: &str = "\r\n";

/// `Environment.ProcessorCount`: the machine's. (The translated game runs its "threads" on one; the game uses the
/// number to size worker counts, which the threading model ignores.)
/// `TERRARIA_PROCESSOR_COUNT` overrides it (comparison runs: the browser build has no thread count to read).
pub fn environment_processor_count() -> i32 {
    if let Some(n) = std::env::var("TERRARIA_PROCESSOR_COUNT").ok().and_then(|v| v.trim().parse::<i32>().ok()).filter(|n| *n > 0) {
        return n;
    }
    std::thread::available_parallelism().map_or(1, |n| n.get() as i32)
}

/// `Environment.SpecialFolder` values the game passes.
pub const FOLDER_PERSONAL: i32 = 5;
pub const FOLDER_APPLICATION_DATA: i32 = 26;
pub const FOLDER_LOCAL_APPLICATION_DATA: i32 = 28;

/// `Environment.GetFolderPath(folder)`: folders inside the save root's sandbox (`<save root>\_folders\...`), never
/// the user's real profile folders. `Personal` / `MyDocuments` (5) -> `...\Documents`.
pub fn environment_get_folder_path(folder: i32) -> Str {
    let name = match folder {
        FOLDER_PERSONAL => "Documents",
        FOLDER_APPLICATION_DATA => "AppData\\Roaming",
        FOLDER_LOCAL_APPLICATION_DATA => "AppData\\Local",
        _ => return host::unimplemented_value("System.Environment.GetFolderPath(SpecialFolder) for a folder other than Personal / ApplicationData / LocalApplicationData"),
    };
    Str::new(&format!("{}\\{name}", super::io::save_root().join("_folders").display()))
}

/// `Environment.ExpandEnvironmentVariables(name)`: `%NAME%` is replaced for the variables that name a folder,
/// each inside `<save root>\_folders` like [`environment_get_folder_path`] (the port never reads the user's real
/// profile or machine-wide folders: SteelSeries Engine's `%PROGRAMDATA%` properties file, `%APPDATA%`); any other
/// `%NAME%` stays as written, which is what .NET does for a variable that is not set.
pub fn environment_expand_environment_variables(name: Str) -> Str {
    if name.is_null() {
        throw("System.ArgumentNullException", "name");
    }
    let base = super::io::save_root().join("_folders");
    let folder = |sub: &str| format!("{}\\{sub}", base.display());
    let mut out = String::new();
    let mut rest = name.as_str();
    while let Some(a) = rest.find('%') {
        out.push_str(&rest[..a]);
        let after = &rest[a + 1..];
        match after.find('%') {
            Some(b) => {
                let var = &after[..b];
                let value = match var.to_ascii_uppercase().as_str() {
                    "PROGRAMDATA" | "ALLUSERSPROFILE" => Some(folder("ProgramData")),
                    "APPDATA" => Some(folder("AppData\\Roaming")),
                    "LOCALAPPDATA" => Some(folder("AppData\\Local")),
                    "USERPROFILE" => Some(folder("UserProfile")),
                    "TEMP" | "TMP" => Some(folder("Temp")),
                    _ => None,
                };
                match value {
                    Some(v) => out.push_str(&v),
                    None => {
                        out.push('%');
                        out.push_str(var);
                        out.push('%');
                    }
                }
                rest = &after[b + 1..];
            }
            None => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    Str::new(&out)
}

/// `Environment.CurrentDirectory.get`
pub fn environment_current_directory() -> Str {
    Str::new(&std::env::current_dir().unwrap_or_default().to_string_lossy())
}

/// `Environment.GetCommandLineArgs()`: the process's, including the program name.
pub fn environment_get_command_line_args() -> Arr<Str> {
    let v: Vec<Str> = std::env::args().map(|a| Str::new(&a)).collect();
    Arr::from_slice(&v)
}

/// `Environment.GetEnvironmentVariable(name)`: null when unset.
pub fn environment_get_environment_variable(name: Str) -> Str {
    match std::env::var(name.as_str()) {
        Ok(v) => Str::new(&v),
        Err(_) => Str::NULL,
    }
}

// ------------------------------------------------------------------------------------------------ Guid

/// `System.Guid` with .NET's field layout (int, short, short, 8 bytes).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Guid {
    pub a: u32,
    pub b: u16,
    pub c: u16,
    pub d: [u8; 8],
}
plain_trace!(Guid);

thread_local! {
    static GUID_EMPTY: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}
/// Tests: `Guid.NewGuid()` returns `Guid.Empty` from now on (the reference harnesses pin the game thread's GUIDs so:
/// tools/refdump_draw RefDumpDraw.cs `FakeNewGuid`, tools/refdump_net NetServer.cs `NetFakeNewGuid`).
pub fn set_new_guid_empty(on: bool) {
    GUID_EMPTY.set(on);
}

impl Guid {
    /// `Guid.NewGuid()`: random (version 4, variant 1), like `CoCreateGuid` ([`set_new_guid_empty`]: `Guid.Empty`).
    pub fn new_guid() -> Guid {
        if GUID_EMPTY.get() {
            return Guid { a: 0, b: 0, c: 0, d: [0; 8] };
        }
        let mut r = [0u8; 16];
        super::crypto::os_random(&mut r);
        r[7] = (r[7] & 0x0F) | 0x40;
        r[8] = (r[8] & 0x3F) | 0x80;
        Guid { a: u32::from_le_bytes([r[0], r[1], r[2], r[3]]), b: u16::from_le_bytes([r[4], r[5]]), c: u16::from_le_bytes([r[6], r[7]]), d: [r[8], r[9], r[10], r[11], r[12], r[13], r[14], r[15]] }
    }
    /// `ToString()`: format "D", lower case, `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`.
    pub fn to_str(&self) -> Str {
        let d = self.d;
        Str::new(&format!("{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}", self.a, self.b, self.c, d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]))
    }
    /// `new Guid(byte[16])`: the inverse of [`Guid::to_byte_array`] (`ArgumentException` for another length).
    pub fn from_bytes(b: Arr<u8>) -> Guid {
        if b.is_null() {
            throw("System.ArgumentNullException", "b");
        }
        let r = b.as_slice();
        if r.len() != 16 {
            throw("System.ArgumentException", "Byte array for GUID must be exactly 16 bytes long.");
        }
        Guid { a: u32::from_le_bytes([r[0], r[1], r[2], r[3]]), b: u16::from_le_bytes([r[4], r[5]]), c: u16::from_le_bytes([r[6], r[7]]), d: [r[8], r[9], r[10], r[11], r[12], r[13], r[14], r[15]] }
    }
    /// `ToByteArray()`
    pub fn to_byte_array(&self) -> Arr<u8> {
        let mut v = Vec::with_capacity(16);
        v.extend_from_slice(&self.a.to_le_bytes());
        v.extend_from_slice(&self.b.to_le_bytes());
        v.extend_from_slice(&self.c.to_le_bytes());
        v.extend_from_slice(&self.d);
        Arr::from_slice(&v)
    }
}

// ------------------------------------------------------------------------------------------------ Process / Registry / Debugger

/// `Debugger.IsAttached`: no managed debugger exists for the translated game.
pub fn debugger_is_attached() -> bool {
    false
}

/// `GC.MaxGeneration`: the runtime's collector has one generation; .NET's desktop CLR answers 2. The game only
/// uses it to index `GC.CollectionCount` for a debug overlay, which is not provided.
pub fn gc_max_generation() -> i32 {
    host::unimplemented_value("System.GC.MaxGeneration.get")
}

/// `System.Diagnostics.Process` / `ProcessStartInfo`: settings are stored. ONE thing is ever launched: the dedicated
/// server of "Host & Play" (`Main.HostAndPlay` starts `TerrariaServer.exe`), which here is this executable with
/// `-server` in front of the game's arguments ([`Process::start`]).
pub struct Process {
    pub file_name: Str,
    pub arguments: Str,
    pub use_shell_execute: bool,
    pub create_no_window: bool,
    pub is_current: bool,
    /// the server process this object started
    child: Option<std::process::Child>,
    /// the browser build: the page's dedicated server Worker this object started (its id; 0: none)
    web_server: i32,
}
// SAFETY: both strings are reported.
unsafe impl rt::Trace for Process {
    unsafe fn trace(&self, m: &mut rt::Marker) {
        self.file_name.trace(m);
        self.arguments.trace(m);
    }
}

impl Process {
    /// `new Process()` / `new ProcessStartInfo()` / `new ProcessStartInfo(fileName)`. `Process.StartInfo` is the
    /// process object itself.
    pub fn new_gc(file_name: Str) -> Gc<Process> {
        rt::alloc_init(Process { file_name, arguments: Str::lit(""), use_shell_execute: true, create_no_window: false, is_current: false, child: None, web_server: 0 })
    }
    /// `Process.GetCurrentProcess()`
    pub fn current() -> Gc<Process> {
        rt::alloc_init(Process { file_name: Str::lit(""), arguments: Str::lit(""), use_shell_execute: false, create_no_window: false, is_current: true, child: None, web_server: 0 })
    }
    pub fn arguments(&self) -> Str {
        self.arguments
    }
    pub fn set_arguments(&mut self, v: Str) {
        self.arguments = v;
    }
    pub fn file_name(&self) -> Str {
        self.file_name
    }
    pub fn set_file_name(&mut self, v: Str) {
        self.file_name = v;
    }
    pub fn set_use_shell_execute(&mut self, v: bool) {
        self.use_shell_execute = v;
    }
    pub fn set_create_no_window(&mut self, v: bool) {
        self.create_no_window = v;
    }
    /// `PriorityClass.set`: deliberately nothing. The dedicated server asks for `High` priority at start-up
    /// (LaunchInitializer.cs:58-98; `-forcepriority n` picks another class). The host leaves the scheduling
    /// priority of its process alone: a server started by "Host & Play" or by a test must not outrank what the
    /// user is doing on the machine. Counted as an inert member (`host::inert_reached`).
    pub fn set_priority_class(&mut self, _v: i32) {
        host::inert("System.Diagnostics.Process.PriorityClass.set (the host keeps its process priority: sys/misc.rs)");
    }
    /// `Start()`. `TerrariaServer.exe` (`Main.HostAndPlay`): THIS executable is started with `-server` followed by
    /// the game's argument string as it stands (`-autoshutdown -password ".." -lang n -world ".." ...), without a
    /// console window, its output in `server-last-run.log` of the save root. Tests add to the arguments:
    /// `TERRARIA_HOSTPLAY_PORT=<n>` gives `-port n` (the game has no port choice for Host & Play: 7777;
    /// `sys::net` sends the client's connection to that port too), `TERRARIA_NET_LOOPBACK_ONLY=1` gives
    /// `-ip 127.0.0.1` (the game's own launch parameter for the listener's address). Anything else is not
    /// launched (opening the save folder, URLs): a recorded unimplemented member.
    #[cfg_attr(target_family = "wasm", allow(unreachable_code))]
    pub fn start(&mut self) -> bool {
        let name = self.file_name.get().unwrap_or("").replace('\\', "/");
        let leaf = name.rsplit('/').next().unwrap_or("").to_ascii_lowercase();
        if leaf != "terrariaserver.exe" && leaf != "terrariaserver" {
            return host::unimplemented_value("System.Diagnostics.Process.Start() (the host launches nothing but its own dedicated server)");
        }
        // the browser build: the page runs the server in a Worker of its own (terraria-web `tw_server_*`), with the
        // game's argument string unchanged; it listens in the page, where this client's connection to
        // 127.0.0.1:7777 goes (sys/net.rs, "The browser build")
        #[cfg(target_family = "wasm")]
        {
            let args = self.arguments.get().unwrap_or("").to_owned();
            return match crate::sys::net::wasm_net::server_start(&args) {
                Ok(id) => {
                    host::log(&format!("[host] started the dedicated server in the page: server {id} (-server {args})"));
                    self.web_server = id;
                    true
                }
                Err(e) => rt::throw(rt::exc("System.ComponentModel.Win32Exception", Str::new(&format!("the dedicated server could not be started: {e}")))),
            };
        }
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => rt::throw(rt::exc("System.ComponentModel.Win32Exception", Str::new(&format!("the running executable cannot be found: {e}")))),
        };
        let mut cmd = std::process::Command::new(&exe);
        cmd.arg("-server");
        let args = self.arguments.get().unwrap_or("").to_owned();
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // the game's own quoting, unchanged
            cmd.raw_arg(&args);
            // CREATE_NO_WINDOW (ProcessStartInfo.CreateNoWindow = true)
            cmd.creation_flags(0x0800_0000);
        }
        #[cfg(not(windows))]
        {
            cmd.args(args.split_whitespace());
        }
        if let Some(p) = std::env::var("TERRARIA_HOSTPLAY_PORT").ok().and_then(|v| v.parse::<u16>().ok()) {
            cmd.args(["-port", &p.to_string()]);
        }
        if crate::sys::net::loopback_only() {
            cmd.args(["-ip", "127.0.0.1"]);
        }
        let log = crate::sys::io::save_root().join("server-last-run.log");
        match std::fs::File::create(&log).and_then(|f| Ok((f.try_clone()?, f))) {
            Ok((out, err)) => {
                cmd.stdout(out).stderr(err);
            }
            Err(_) => {
                cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
            }
        }
        cmd.stdin(std::process::Stdio::null());
        match cmd.spawn() {
            Ok(child) => {
                host::log(&format!("[host] started the dedicated server: pid {} ({} -server {args}); its output: {}", child.id(), exe.display(), log.display()));
                self.child = Some(child);
                true
            }
            Err(e) => rt::throw(rt::exc("System.ComponentModel.Win32Exception", Str::new(&format!("the dedicated server could not be started: {e}")))),
        }
    }
    /// `Id`: of a started server, its process id.
    pub fn id(&self) -> i32 {
        if self.is_current {
            return std::process::id() as i32;
        }
        if self.web_server != 0 {
            return self.web_server;
        }
        match &self.child {
            Some(c) => c.id() as i32,
            None => throw("System.InvalidOperationException", "No process is associated with this object."),
        }
    }
    /// `HasExited`: of the current process, false; of a process that was never started, .NET throws.
    pub fn has_exited(&mut self) -> bool {
        if self.is_current {
            return false;
        }
        #[cfg(target_family = "wasm")]
        if self.web_server != 0 {
            return crate::sys::net::wasm_net::server_exited(self.web_server);
        }
        match self.child.as_mut() {
            Some(c) => !matches!(c.try_wait(), Ok(None)),
            None => throw("System.InvalidOperationException", "No process is associated with this object."),
        }
    }
    /// `Kill()`
    pub fn kill(&mut self) {
        if self.is_current {
            host::unimplemented("System.Diagnostics.Process.Kill() on the current process");
            return;
        }
        #[cfg(target_family = "wasm")]
        if self.web_server != 0 {
            crate::sys::net::wasm_net::server_kill(self.web_server);
            return;
        }
        match self.child.as_mut() {
            Some(c) => {
                let _ = c.kill();
                let _ = c.wait();
            }
            None => throw("System.InvalidOperationException", "No process is associated with this object."),
        }
    }
}

/// `Process.Start(string)` / `Process.Start(ProcessStartInfo)`: never launches anything.
pub fn process_start_static() -> Gc<Process> {
    host::unimplemented_value("System.Diagnostics.Process.Start(...) (the host does not launch processes)")
}

/// `Registry.CurrentUser.CreateSubKey(name)` / `OpenSubKey(name)`: there is no registry; `CreateSubKey` returning
/// null is what .NET does when the operation fails, and `OpenSubKey` when the key does not exist.
/// Call sites (`Main.cs`, bunny / Steam-install lookups) test the key for null or catch the failure.
pub fn registry_sub_key(_name: Str) -> Obj {
    Gc::NULL
}

/// `Microsoft.Win32.RegistryKey`: a key of a registry that holds nothing.
pub struct RegistryKey;
plain_trace!(RegistryKey);
impl RegistryKey {
    /// `Registry.CurrentUser` / `LocalMachine`
    pub fn root() -> Gc<RegistryKey> {
        rt::alloc_init(RegistryKey)
    }
    /// `CreateSubKey(name)` / `OpenSubKey(name)`: null. `OpenSubKey` returns null for a missing key; `CreateSubKey`
    /// returns null when the operation fails. `Main.cs:4414` (the only caller) tests the result for null inside
    /// a `try`.
    pub fn sub_key(&self, _name: Str) -> Gc<RegistryKey> {
        Gc::NULL
    }
    /// `GetValue(name)`: null (no such value).
    pub fn get_value(&self, _name: Str) -> Obj {
        Gc::NULL
    }
}

// ------------------------------------------------------------------------------------------------ AppDomain

/// `UnhandledExceptionEventArgs`
pub struct UnhandledExceptionEventArgs {
    pub exception_object: Obj,
    pub is_terminating: bool,
}
// SAFETY: the one handle is reported.
unsafe impl rt::Trace for UnhandledExceptionEventArgs {
    unsafe fn trace(&self, m: &mut rt::Marker) {
        m.mark(self.exception_object)
    }
}
impl UnhandledExceptionEventArgs {
    pub fn exception_object(&self) -> Obj {
        self.exception_object
    }
    pub fn is_terminating(&self) -> bool {
        self.is_terminating
    }
}
pub type UnhandledExceptionHandler = rt::Delegate<dyn Fn(Obj, Gc<UnhandledExceptionEventArgs>)>;

thread_local! {
    static UNHANDLED: RefCell<Option<rt::Rooted<UnhandledExceptionHandler>>> = const { RefCell::new(None) };
    static DOMAIN: rt::Rooted<Obj> = rt::Rooted::new(rt::new_object());
}

/// `AppDomain.CurrentDomain`: the one domain, an object without members of its own (the events below are static
/// functions of this module).
pub fn app_domain() -> Obj {
    DOMAIN.with(|d| d.get())
}
/// `AppDomain.CurrentDomain.UnhandledException += handler` (`CrashWatcher`: the crash log).
pub fn app_domain_add_unhandled_exception(h: UnhandledExceptionHandler) {
    UNHANDLED.with(|u| {
        let mut u = u.borrow_mut();
        let cur = u.as_ref().map_or(rt::Delegate::NULL, |r| r.get());
        *u = Some(rt::Rooted::new(rt::Delegate::combine(cur, h)));
    });
}
/// Driver: a C# exception left `Initialize` / `Update` / `Draw` uncaught. Runs the game's `UnhandledException`
/// handlers (the CLR does this before it terminates the process); the driver ends the run afterwards.
pub fn raise_unhandled_exception(e: Gc<rt::Exception>) {
    let h = UNHANDLED.with(|u| u.borrow().as_ref().map_or(rt::Delegate::NULL, |r| r.get()));
    if h.is_null() {
        return;
    }
    let args = rt::alloc_init(UnhandledExceptionEventArgs { exception_object: e.up(), is_terminating: true });
    h.invoke(|f| f(app_domain(), args));
}

/// `Trace.WriteLine(string)` / `Debug.WriteLine(string)`: stderr (there are no trace listeners).
pub fn trace_write_line(message: Str) {
    eprintln!("{}", message.get().unwrap_or(""));
}

/// `Trace.Assert(condition, message)` / `Debug.Assert`: a failed assertion is reported on stderr; .NET shows a
/// dialog with Abort / Retry / Ignore, where "Ignore" continues.
pub fn trace_assert(condition: bool, message: Str) {
    if !condition {
        eprintln!("[assert failed] {}", message.get().unwrap_or(""));
    }
}

#[cfg(test)]
mod guid_bytes_tests {
    use super::*;

    /// `new Guid(g.ToByteArray()) == g`, and the byte order is .NET's (first three fields little endian).
    #[test]
    fn guid_round_trips_through_its_bytes() {
        let g = Guid { a: 0x0123_4567, b: 0x89AB, c: 0xCDEF, d: [1, 2, 3, 4, 5, 6, 7, 8] };
        let b = g.to_byte_array();
        assert_eq!(b.as_slice(), &[0x67, 0x45, 0x23, 0x01, 0xAB, 0x89, 0xEF, 0xCD, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(Guid::from_bytes(b), g);
        assert_eq!(g.to_str().as_str(), "01234567-89ab-cdef-0102-030405060708");
    }
}

#[cfg(test)]
mod expand_tests {
    use super::*;

    #[test]
    fn folder_variables_expand_inside_the_save_root_and_others_stay() {
        let e = |s: &str| environment_expand_environment_variables(Str::new(s)).as_str().to_owned();
        let base = format!("{}", super::super::io::save_root().join("_folders").display());
        assert_eq!(e("%PROGRAMDATA%/SteelSeries/x.json"), format!("{base}\\ProgramData/SteelSeries/x.json"));
        assert_eq!(e("a%NOT_A_FOLDER_VARIABLE%b"), "a%NOT_A_FOLDER_VARIABLE%b");
        assert_eq!(e("100%"), "100%");
        assert_eq!(e("plain"), "plain");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraria_core::UnifiedRandom;

    /// `System.Random` is mapped to `terraria_core::UnifiedRandom` (the same subtractive generator): checked
    /// against real .NET (tests/dotnet/system_probe.out, "Random").
    #[test]
    fn system_random_is_unified_random() {
        let mut r = UnifiedRandom::new(12345);
        assert_eq!([r.next_raw(), r.next_raw(), r.next_raw(), r.next_raw(), r.next_raw()], [143337951, 150666398, 1663795458, 1097663221, 1712597933]);
        assert!((r.next_double() - 0.827308291023275).abs() < 1e-15);
        assert_eq!(r.next(10), 1);
        assert_eq!(r.next_range(5, 50), 38);
        let mut r0 = UnifiedRandom::new(0);
        assert_eq!([r0.next_double(), r0.next_double(), r0.next_double()], [0.72624326996795985, 0.81732535959096875, 0.76802268939466345]);
        let mut rn = UnifiedRandom::new(-7);
        assert_eq!([rn.next_raw(), rn.next_raw(), rn.next_raw()], [822959691, 1871007331, 1419354884]);
    }

    #[test]
    fn console_guid_environment() {
        console_capture(true);
        console_write(Str::lit("a"));
        console_write(Str::NULL);
        console_write_line(Str::lit("b"));
        console_write_line_empty();
        assert_eq!(console_take_captured(), "ab\r\n\r\n");
        console_capture(false);
        console_set_title(Str::lit("Terraria Server"));
        assert_eq!(console_title().as_str(), "Terraria Server");

        let g = Guid::new_guid();
        let s = g.to_str().as_str();
        assert_eq!((s.len(), &s[14..15], s.as_bytes()[8], s.as_bytes()[23]), (36, "4", b'-', b'-'));
        assert!(matches!(&s[19..20], "8" | "9" | "a" | "b"));
        assert_ne!(Guid::new_guid(), g);
        assert_eq!(g.to_byte_array().len(), 16);
        let known = Guid { a: 0x01020304, b: 0x0506, c: 0x0708, d: [9, 10, 11, 12, 13, 14, 15, 16] };
        assert_eq!(known.to_str().as_str(), "01020304-0506-0708-090a-0b0c0d0e0f10");

        assert!(environment_processor_count() >= 1);
        let docs = environment_get_folder_path(FOLDER_PERSONAL).as_str();
        assert!(docs.ends_with("_folders\\Documents") && docs.contains("port-save") || std::env::var_os("TERRARIA_SAVE_DIR").is_some());
        assert!(registry_sub_key(Str::lit("Software\\Terraria")).is_null());
        assert!(!debugger_is_attached());
    }
}
