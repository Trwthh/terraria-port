//! `Microsoft.Xna.Framework.Game` and what hangs off it (`GameWindow`, `GraphicsDeviceManager`, `GameTime`,
//! `GameServiceContainer`), driven from outside: the host never calls translated code for `Update` / `Draw`; it
//! tells the driver when to (README.md, "Driver protocol").
//!
//! There is one `Game` per process, so its state is this thread's [`HostState`] and the `Game.*` members are free
//! functions (the receiver, the translated `Main` object, is not needed).
//!
//! Timing is `Game.Tick` (reference/xna/Game.cs:312), line for line: elapsed time is clamped to 0..500 ms; with
//! `IsFixedTimeStep` an elapsed time within 1/64 of the target counts as exactly the target, the accumulator
//! decides how many `Update`s run (possibly none: then nothing is drawn either), `IsRunningSlowly` follows the
//! two counters; without it there is one `Update` with the real elapsed time. `Draw` is skipped when every
//! `Update` of the tick called `SuppressDraw`.

use std::cell::RefCell;

use terraria_core::xna::Rectangle;
use terraria_gen_rt::rt::{self, time::TimeSpan, Delegate, Gc, Marker, Obj, Rooted, Str, Trace};

use crate::audio::AudioCommand;
use crate::content::ContentManager;
use crate::gfx::device::EventHandler;
use crate::gfx::states::{plain_trace, throw};
use crate::gfx::{Frame, GraphicsDevice, PresentationParameters};
use crate::input::InputSnapshot;

/// `GameTime` (a class).
pub struct GameTime {
    pub total_game_time: TimeSpan,
    pub elapsed_game_time: TimeSpan,
    pub is_running_slowly: bool,
}
plain_trace!(GameTime);
impl GameTime {
    /// `new GameTime()` / `new GameTime(totalGameTime, elapsedGameTime)`
    pub fn new_gc(total: TimeSpan, elapsed: TimeSpan) -> Gc<GameTime> {
        rt::alloc_init(GameTime { total_game_time: total, elapsed_game_time: elapsed, is_running_slowly: false })
    }
    pub fn total_game_time(&self) -> TimeSpan {
        self.total_game_time
    }
    pub fn elapsed_game_time(&self) -> TimeSpan {
        self.elapsed_game_time
    }
    pub fn is_running_slowly(&self) -> bool {
        self.is_running_slowly
    }
}

/// `GameWindow`
pub struct GameWindow {
    pub title: Str,
    pub client_bounds: Rectangle,
    pub allow_user_resizing: bool,
    /// `Handle` (an `IntPtr` value): whatever the app reports; 0 when it has no native handle to give
    pub handle: isize,
    pub client_size_changed: EventHandler,
}
// SAFETY: every handle is reported.
unsafe impl Trace for GameWindow {
    unsafe fn trace(&self, m: &mut Marker) {
        self.title.trace(m);
        self.client_size_changed.trace(m);
    }
}
impl GameWindow {
    pub fn title(&self) -> Str {
        self.title
    }
    /// `ScreenDeviceName`: the device name of the monitor the window is on (the primary one until the app says more).
    pub fn screen_device_name(&self) -> Str {
        Str::new(&crate::winforms::primary_device_name())
    }
    /// `Title.set` (also what `IWindowService.SetUnicodeTitle` does): the app reads it from [`FrameOutput`].
    pub fn set_title(&mut self, v: Str) {
        if v.is_null() {
            throw("System.ArgumentNullException", "value");
        }
        self.title = v;
    }
    pub fn client_bounds(&self) -> Rectangle {
        self.client_bounds
    }
    pub fn allow_user_resizing(&self) -> bool {
        self.allow_user_resizing
    }
    pub fn set_allow_user_resizing(&mut self, v: bool) {
        self.allow_user_resizing = v;
    }
    pub fn handle(&self) -> isize {
        self.handle
    }
    pub fn add_client_size_changed(&mut self, h: EventHandler) {
        self.client_size_changed = Delegate::combine(self.client_size_changed, h);
    }
    pub fn remove_client_size_changed(&mut self, h: EventHandler) {
        self.client_size_changed = Delegate::remove(self.client_size_changed, h);
    }
}

/// `PreparingDeviceSettingsEventArgs`, its `GraphicsDeviceInformation`, in one object: the handler reaches
/// `e.GraphicsDeviceInformation.PresentationParameters`, which is the device's own parameter object.
pub struct PreparingDeviceSettingsEventArgs {
    pub presentation_parameters: Gc<PresentationParameters>,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for PreparingDeviceSettingsEventArgs {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.presentation_parameters)
    }
}
impl PreparingDeviceSettingsEventArgs {
    /// `GraphicsDeviceInformation.Adapter = adapter`: this host has one adapter (the device the app renders with),
    /// so the only value the game can assign is the one already in use (`Main.SetMonitorOnce` picks from
    /// `GraphicsAdapter.Adapters`). Anything else is refused loudly.
    pub fn set_adapter(&mut self, adapter: Gc<GraphicsAdapter>) {
        if adapter != GraphicsAdapter::default_adapter() {
            throw("System.ArgumentException", "GraphicsDeviceInformation.Adapter: not an adapter of this host");
        }
    }
    /// `GraphicsDeviceInformation` (this object) `.PresentationParameters`
    pub fn presentation_parameters(&self) -> Gc<PresentationParameters> {
        self.presentation_parameters
    }
}
pub type PreparingHandler = Delegate<dyn Fn(Obj, Gc<PreparingDeviceSettingsEventArgs>)>;

/// `GraphicsDeviceManager`
pub struct GraphicsDeviceManager {
    pub device: Gc<GraphicsDevice>,
    pub preferred_back_buffer_width: i32,
    pub preferred_back_buffer_height: i32,
    pub is_full_screen: bool,
    pub synchronize_with_vertical_retrace: bool,
    pub graphics_profile: i32,
    pub prefer_multi_sampling: bool,
    pub preparing_device_settings: PreparingHandler,
}
// SAFETY: every handle is reported.
unsafe impl Trace for GraphicsDeviceManager {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.device);
        self.preparing_device_settings.trace(m);
    }
}
impl GraphicsDeviceManager {
    pub fn graphics_device(&self) -> Gc<GraphicsDevice> {
        self.device
    }
    pub fn preferred_back_buffer_width(&self) -> i32 {
        self.preferred_back_buffer_width
    }
    pub fn set_preferred_back_buffer_width(&mut self, v: i32) {
        if v <= 0 {
            throw("System.ArgumentOutOfRangeException", "value: the back buffer size must be greater than zero.");
        }
        self.preferred_back_buffer_width = v;
    }
    pub fn preferred_back_buffer_height(&self) -> i32 {
        self.preferred_back_buffer_height
    }
    pub fn set_preferred_back_buffer_height(&mut self, v: i32) {
        if v <= 0 {
            throw("System.ArgumentOutOfRangeException", "value: the back buffer size must be greater than zero.");
        }
        self.preferred_back_buffer_height = v;
    }
    pub fn is_full_screen(&self) -> bool {
        self.is_full_screen
    }
    pub fn set_is_full_screen(&mut self, v: bool) {
        self.is_full_screen = v;
    }
    pub fn synchronize_with_vertical_retrace(&self) -> bool {
        self.synchronize_with_vertical_retrace
    }
    pub fn set_synchronize_with_vertical_retrace(&mut self, v: bool) {
        self.synchronize_with_vertical_retrace = v;
    }
    pub fn set_graphics_profile(&mut self, v: i32) {
        self.graphics_profile = v;
    }
    pub fn add_preparing_device_settings(&mut self, h: PreparingHandler) {
        self.preparing_device_settings = Delegate::combine(self.preparing_device_settings, h);
    }
    pub fn remove_preparing_device_settings(&mut self, h: PreparingHandler) {
        self.preparing_device_settings = Delegate::remove(self.preparing_device_settings, h);
    }
    /// `ToggleFullScreen()`
    pub fn toggle_full_screen(&mut self) {
        self.is_full_screen = !self.is_full_screen;
        self.apply_changes();
    }
    /// `ApplyChanges()`: raises `PreparingDeviceSettings`, then the device takes the preferred back buffer size
    /// (viewport, scissor rectangle and `Window.ClientBounds` follow) and the app is asked to make the window match
    /// ([`FrameOutput::window_request`]).
    pub fn apply_changes(&mut self) {
        let (device, handler) = (self.device, self.preparing_device_settings);
        // SAFETY: the manager keeps its device alive.
        let pp = unsafe { (*device.ptr()).presentation_parameters };
        if !handler.is_null() {
            let args = rt::alloc_init(PreparingDeviceSettingsEventArgs { presentation_parameters: pp });
            // translated code runs here; no Rust reference into a host object is alive across the call
            handler.invoke(|f| f(Gc::NULL, args));
        }
        // re-read: the handler may have changed the manager
        // SAFETY: `self` is a live object; the handler cannot free it (no collection runs under generated code).
        let (w, h, full) = (self.preferred_back_buffer_width, self.preferred_back_buffer_height, self.is_full_screen);
        // SAFETY: live objects of this host.
        unsafe {
            (*pp.ptr()).is_full_screen = full;
            (*pp.ptr()).presentation_interval = if self.synchronize_with_vertical_retrace { 1 } else { 3 };
            (*device.ptr()).graphics_profile = self.graphics_profile;
            let d = &mut *device.ptr();
            let old = ((*pp.ptr()).back_buffer_width, (*pp.ptr()).back_buffer_height);
            if old != (w, h) {
                d.reset_back_buffer(w, h);
            }
        }
        with(|s| {
            // SAFETY: the window object is rooted by the host state.
            unsafe { (*s.roots.get().window.ptr()).client_bounds.width = w };
            unsafe { (*s.roots.get().window.ptr()).client_bounds.height = h };
            if WINDOW_FOLLOWS_APPLY_CHANGES.get() {
                // XNA resizes the form inside ApplyChanges: `Form.ClientSize` is the new size from here on
                s.window.width = w;
                s.window.height = h;
            }
            s.window_request = Some(WindowRequest { width: w, height: h, full_screen: full, vsync: self.synchronize_with_vertical_retrace });
        });
    }
}

/// `GameServiceContainer` (`Game.Services`): services by type name.
pub struct GameServiceContainer {
    services: Vec<(String, Obj)>,
}
// SAFETY: every handle is reported.
unsafe impl Trace for GameServiceContainer {
    unsafe fn trace(&self, m: &mut Marker) {
        for (_, o) in &self.services {
            m.mark(*o);
        }
    }
}
impl GameServiceContainer {
    /// `AddService(Type type, object provider)`
    pub fn add_service(&mut self, service_type: Obj, provider: Obj) {
        if provider.is_null() {
            throw("System.ArgumentNullException", "provider");
        }
        let name = rt::type_full_name(service_type).as_str().to_owned();
        if self.services.iter().any(|(n, _)| *n == name) {
            throw("System.ArgumentException", "The service is already present in the container.");
        }
        self.services.push((name, provider));
    }
    /// `GetService(Type type)`: null when there is none.
    pub fn get_service(&self, service_type: Obj) -> Obj {
        self.get_service_named(rt::type_full_name(service_type).as_str())
    }
    pub fn get_service_named(&self, type_name: &str) -> Obj {
        self.services.iter().find(|(n, _)| n == type_name).map_or(Gc::NULL, |(_, o)| *o)
    }
    pub fn remove_service(&mut self, service_type: Obj) {
        let name = rt::type_full_name(service_type).as_str().to_owned();
        self.services.retain(|(n, _)| *n != name);
    }
}

/// `GraphicsAdapter` (`GraphicsAdapter.DefaultAdapter`, `GraphicsDevice.Adapter`): the one display the window is on.
pub struct GraphicsAdapter;
/// `DisplayMode`
pub struct DisplayMode {
    pub width: i32,
    pub height: i32,
}
plain_trace!(GraphicsAdapter, DisplayMode);

impl GraphicsAdapter {
    /// `GraphicsAdapter.DefaultAdapter`: ONE object per thread (`Main.CacheSupportedDisplaySizes` compares the
    /// adapters of `GraphicsAdapter.Adapters` with it by reference).
    pub fn default_adapter() -> Gc<GraphicsAdapter> {
        thread_local! {
            static DEFAULT: Rooted<Gc<GraphicsAdapter>> = Rooted::new(rt::alloc_init(GraphicsAdapter));
        }
        DEFAULT.with(|d| d.get())
    }
    /// `GraphicsAdapter.Adapters`: the one adapter this host models (the device the app renders with). A fresh
    /// read-only list per call, as XNA hands out its collection.
    pub fn adapters() -> Gc<rt::List<Gc<GraphicsAdapter>>> {
        let l = rt::List::<Gc<GraphicsAdapter>>::new_gc();
        // SAFETY: fresh list.
        unsafe { (*l.ptr()).add(Self::default_adapter()) };
        l
    }
    /// `DeviceName`: the display the adapter drives (`\\.\DISPLAY1` for the primary monitor), which is what
    /// `Main.SetMonitorOnce` compares with the "Display" setting.
    pub fn device_name(&self) -> Str {
        Str::new(&crate::winforms::primary_device_name())
    }
    /// `CurrentDisplayMode`: the size the app reported for the window's monitor (`WindowState::display_*`).
    pub fn current_display_mode(&self) -> Gc<DisplayMode> {
        let (width, height) = display_mode();
        rt::alloc_init(DisplayMode { width, height })
    }
    /// `IsProfileSupported(profile)`: Reach and HiDef both; the backend has no capability the game's HiDef use lacks.
    pub fn is_profile_supported(&self, profile: i32) -> bool {
        (0..=1).contains(&profile)
    }
}
impl DisplayMode {
    pub fn width(&self) -> i32 {
        self.width
    }
    pub fn height(&self) -> i32 {
        self.height
    }
    /// `Format`: `SurfaceFormat.Color`
    pub fn format(&self) -> i32 {
        0
    }
}

// ------------------------------------------------------------------------------------------------ host state

thread_local! {
    static WINDOW_FOLLOWS_APPLY_CHANGES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// App: does the window take the size of `GraphicsDeviceManager.ApplyChanges` at once, as XNA's form does?
///
/// In XNA `ApplyChanges` resizes the window before it returns, so `Form.ClientSize` is the new back buffer size
/// in the very next statement. `Main.SetDisplayMode` relies on it: every `Update` it compares the form's client
/// size with the preferred back buffer and takes the smaller one for a user's resize (Main.cs:67405-67417). With
/// `false` (the default: what the scripted, hidden runs were recorded with) the window state keeps the size the
/// app reported at the start of the frame, so the game sees its own request as "the window is smaller" and
/// shrinks back: a window that starts at 1280 x 720 stays there, and the game's default start (a borderless
/// window with the monitor's size) never happens. With `true` the game gets what it asked for, and the app is
/// expected to resize its window to match (`FrameOutput::window_request`) before the next frame's report.
pub fn set_window_follows_apply_changes(v: bool) {
    WINDOW_FOLLOWS_APPLY_CHANGES.set(v);
}

/// What the app knows about its window at the start of a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowState {
    /// client area in physical pixels
    pub width: i32,
    pub height: i32,
    /// position of the client area on the desktop
    pub x: i32,
    pub y: i32,
    /// the window has focus and is not minimised (`Game.IsActive`)
    pub active: bool,
    /// native window handle if the app has one to give (`GameWindow.Handle`)
    pub handle: isize,
    /// size of the monitor the window is on (`GraphicsAdapter.CurrentDisplayMode`)
    pub display_width: i32,
    pub display_height: i32,
    /// DPI scale of the window (`IWindowService.GetScaling`): 1.0 = 96 dpi
    pub scale: f32,
}

/// The window the game asked for (`GraphicsDeviceManager.ApplyChanges`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowRequest {
    pub width: i32,
    pub height: i32,
    pub full_screen: bool,
    pub vsync: bool,
}

/// What `end_draw` hands to the app.
#[derive(Debug)]
pub struct FrameOutput {
    pub frame: Frame,
    pub audio: Vec<AudioCommand>,
    pub title: String,
    pub is_mouse_visible: bool,
    pub window_request: Option<WindowRequest>,
    /// `Mouse.SetPosition` asked for this client position
    pub mouse_position_request: Option<(i32, i32)>,
    /// `Game.Exit()` was called
    pub exit: bool,
}

#[derive(Clone, Copy)]
pub struct Roots {
    pub device: Gc<GraphicsDevice>,
    pub manager: Gc<GraphicsDeviceManager>,
    pub window: Gc<GameWindow>,
    pub content: Gc<ContentManager>,
    pub services: Gc<GameServiceContainer>,
    pub game_time: Gc<GameTime>,
    pub exiting: EventHandler,
    pub activated: EventHandler,
    pub deactivated: EventHandler,
}
// SAFETY: every handle is reported.
unsafe impl Trace for Roots {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.device);
        m.mark(self.manager);
        m.mark(self.window);
        m.mark(self.content);
        m.mark(self.services);
        m.mark(self.game_time);
        self.exiting.trace(m);
        self.activated.trace(m);
        self.deactivated.trace(m);
    }
}

pub struct HostState {
    pub roots: Rooted<Roots>,
    pub is_active: bool,
    pub is_fixed_time_step: bool,
    pub is_mouse_visible: bool,
    pub target_elapsed_time: TimeSpan,
    pub inactive_sleep_time: TimeSpan,
    pub window: WindowState,
    should_exit: bool,
    suppress_draw: bool,
    accumulated: i64,
    total: i64,
    last_frame_elapsed: i64,
    updates_since_slow1: i32,
    updates_since_slow2: i32,
    draw_running_slowly: bool,
    /// (updates still to run this tick, elapsed time of each)
    pending_updates: (u32, i64),
    all_suppressed: bool,
    ran_update: bool,
    window_request: Option<WindowRequest>,
    events: Vec<EventHandler>,
}

thread_local! {
    static HOST: RefCell<Option<HostState>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut HostState) -> R) -> R {
    HOST.with(|h| match h.borrow_mut().as_mut() {
        Some(s) => f(s),
        None => panic!("terraria-host: game::init has not been called on this thread"),
    })
}

/// `TimeSpan.FromTicks(166667)`: XNA's default `TargetElapsedTime` (1/60 s).
pub const DEFAULT_TARGET_ELAPSED_TICKS: i64 = 166_667;
const MAX_ELAPSED_TICKS: i64 = 5_000_000;

/// Creates the game's host objects for a window (what `new Game()` + `new GraphicsDeviceManager(game)` + the
/// first `ApplyChanges` leave behind). XNA defaults: fixed time step at 60 Hz, `IsMouseVisible` false, vsync on,
/// `Content.RootDirectory` empty.
pub fn init(window: WindowState) {
    // the install folder is read-only for every file stream of the runtime too (`FileStream`, `File.Open*`); this
    // crate's own `File.*` / `Directory.*` refuse such writes in `sys::io`
    rt::io::add_read_only_root(&crate::content::install_dir());
    let device = GraphicsDevice::new_gc(window.width, window.height);
    let roots = Roots {
        device,
        manager: rt::alloc_init(GraphicsDeviceManager {
            device,
            preferred_back_buffer_width: window.width,
            preferred_back_buffer_height: window.height,
            is_full_screen: false,
            synchronize_with_vertical_retrace: true,
            graphics_profile: 1,
            prefer_multi_sampling: false,
            preparing_device_settings: Delegate::NULL,
        }),
        window: rt::alloc_init(GameWindow { title: Str::lit(""), client_bounds: Rectangle::new(window.x, window.y, window.width, window.height), allow_user_resizing: false, handle: window.handle, client_size_changed: Delegate::NULL }),
        content: ContentManager::new_gc(device, Str::lit("")),
        services: rt::alloc_init(GameServiceContainer { services: Vec::new() }),
        game_time: GameTime::new_gc(TimeSpan::ZERO, TimeSpan::ZERO),
        exiting: Delegate::NULL,
        activated: Delegate::NULL,
        deactivated: Delegate::NULL,
    };
    // SAFETY: fresh objects.
    unsafe {
        (*device.ptr()).presentation_parameters().ptr().as_mut().expect("pp").device_window_handle = window.handle;
        (*roots.services.ptr()).services.push(("Microsoft.Xna.Framework.Graphics.IGraphicsDeviceService".to_owned(), roots.manager.up()));
        (*roots.services.ptr()).services.push(("Microsoft.Xna.Framework.IGraphicsDeviceManager".to_owned(), roots.manager.up()));
    }
    let state = HostState {
        roots: Rooted::new(roots),
        is_active: window.active,
        is_fixed_time_step: true,
        is_mouse_visible: false,
        target_elapsed_time: TimeSpan::new(DEFAULT_TARGET_ELAPSED_TICKS),
        inactive_sleep_time: TimeSpan::new(200_000),
        window,
        should_exit: false,
        suppress_draw: false,
        accumulated: 0,
        total: 0,
        last_frame_elapsed: 0,
        updates_since_slow1: i32::MAX,
        updates_since_slow2: i32::MAX,
        draw_running_slowly: false,
        pending_updates: (0, 0),
        all_suppressed: true,
        ran_update: false,
        window_request: None,
        events: Vec::new(),
    };
    HOST.with(|h| *h.borrow_mut() = Some(state));
}

/// True once [`init`] has run on this thread.
pub fn is_initialized() -> bool {
    HOST.with(|h| h.borrow().is_some())
}

pub fn roots() -> Roots {
    with(|s| s.roots.get())
}

// ---- Game members

/// `Game.GraphicsDevice`
pub fn graphics_device() -> Gc<GraphicsDevice> {
    roots().device
}
/// The `GraphicsDeviceManager` (`new GraphicsDeviceManager(game)` returns the one the host made).
pub fn graphics_device_manager() -> Gc<GraphicsDeviceManager> {
    roots().manager
}
/// `Game.Window`
pub fn window() -> Gc<GameWindow> {
    roots().window
}
/// `Game.Content`
pub fn content() -> Gc<ContentManager> {
    roots().content
}
/// `IGraphicsDeviceService.GraphicsDevice` on a service held through the interface: the one implementer is the
/// `GraphicsDeviceManager` that `init` registered in `Game.Services`.
pub fn graphics_device_service_device(service: Obj) -> Gc<GraphicsDevice> {
    if rt::obj_is::<GraphicsDeviceManager>(service) {
        // SAFETY: type checked; the reference ends with this statement.
        return unsafe { (*(service.0 as *const GraphicsDeviceManager)).device };
    }
    let _ = service.ptr();
    terraria_gen_rt::host::unimplemented_value("IGraphicsDeviceService.GraphicsDevice on an object that is not the GraphicsDeviceManager")
}

/// `IServiceProvider.GetService(Type)` on an object held through the interface: the game's only provider is
/// `Game.Services` (`XnaExtensions.Get<T>(this IServiceProvider)`).
pub fn service_provider_get_service(provider: Obj, service_type: Obj) -> Obj {
    if rt::obj_is::<GameServiceContainer>(provider) {
        // SAFETY: type checked; the reference ends with this statement.
        return unsafe { (*(provider.0 as *const GameServiceContainer)).get_service(service_type) };
    }
    // null: NullReferenceException
    let _ = provider.ptr();
    terraria_gen_rt::host::unimplemented_value("System.IServiceProvider.GetService(Type) on an object that is not Game.Services")
}

/// `Game.Services`
pub fn services() -> Gc<GameServiceContainer> {
    roots().services
}
/// `Game.IsActive`
pub fn is_active() -> bool {
    with(|s| s.is_active)
}
pub fn is_fixed_time_step() -> bool {
    with(|s| s.is_fixed_time_step)
}
pub fn set_is_fixed_time_step(v: bool) {
    with(|s| s.is_fixed_time_step = v);
}
pub fn target_elapsed_time() -> TimeSpan {
    with(|s| s.target_elapsed_time)
}
/// `TargetElapsedTime.set`: must be positive.
pub fn set_target_elapsed_time(v: TimeSpan) {
    if v.ticks <= 0 {
        throw("System.ArgumentOutOfRangeException", "value: TargetElapsedTime must be greater than zero.");
    }
    with(|s| s.target_elapsed_time = v);
}
pub fn inactive_sleep_time() -> TimeSpan {
    with(|s| s.inactive_sleep_time)
}
/// `InactiveSleepTime.set`: stored; the driver decides how an inactive window is paced.
pub fn set_inactive_sleep_time(v: TimeSpan) {
    if v.ticks < 0 {
        throw("System.ArgumentOutOfRangeException", "value");
    }
    with(|s| s.inactive_sleep_time = v);
}
pub fn is_mouse_visible() -> bool {
    with(|s| s.is_mouse_visible)
}
pub fn set_is_mouse_visible(v: bool) {
    with(|s| s.is_mouse_visible = v);
}
/// `Game.Exit()`
pub fn exit() {
    with(|s| s.should_exit = true);
}
/// `Game.SuppressDraw()`
pub fn suppress_draw() {
    with(|s| s.suppress_draw = true);
}
/// `Game.Exiting += handler`
pub fn add_exiting(h: EventHandler) {
    with(|s| {
        let mut r = s.roots.get();
        r.exiting = Delegate::combine(r.exiting, h);
        s.roots.set(r);
    });
}
/// `Game.Activated += handler` / `Deactivated`
pub fn add_activated(h: EventHandler) {
    with(|s| {
        let mut r = s.roots.get();
        r.activated = Delegate::combine(r.activated, h);
        s.roots.set(r);
    });
}
pub fn add_deactivated(h: EventHandler) {
    with(|s| {
        let mut r = s.roots.get();
        r.deactivated = Delegate::combine(r.deactivated, h);
        s.roots.set(r);
    });
}
/// `base.Initialize()`, `base.Update(gameTime)`, `base.Draw(gameTime)`: XNA's own bodies only walk
/// `Game.Components`, which the game leaves empty.
pub fn base_noop() {}

thread_local! {
    /// The game's `LoadContent` override (see [`set_load_content`]).
    static LOAD_CONTENT: RefCell<Option<std::rc::Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Driver: registers the game's override of `Game.LoadContent()`. `Main : Game` does not derive from a native class,
/// so the one virtual call XNA makes from inside a base method (`Game.Initialize` -> `LoadContent`,
/// reference/xna/Game.cs:503-514) needs the override handed over. Whatever the closure captures must be rooted by
/// the driver.
pub fn set_load_content(f: impl Fn() + 'static) {
    LOAD_CONTENT.with(|c| *c.borrow_mut() = Some(std::rc::Rc::new(f)));
}

/// `base.Initialize()`: XNA's body initialises `Game.Components` (empty in this game), then, with a graphics device
/// present, calls the virtual `LoadContent()`. Without a registered override this stops: continuing would skip
/// content loading silently.
pub fn base_initialize() {
    let f = LOAD_CONTENT.with(|c| c.borrow().clone());
    match f {
        Some(f) => f(),
        None => panic!("host: Game.Initialize() reached without a LoadContent override (game::set_load_content)"),
    }
}

/// The current display mode (`GraphicsAdapter.DefaultAdapter.CurrentDisplayMode`): (width, height).
pub fn display_mode() -> (i32, i32) {
    with(|s| (s.window.display_width, s.window.display_height))
}
/// The window as the app reported it at the start of the frame (position and size of the client area, monitor).
pub fn window_state() -> WindowState {
    with(|s| s.window)
}
/// `IWindowService.GetScaling()`
pub fn window_scale() -> f32 {
    with(|s| s.window.scale)
}

// ---- driver

/// Driver, step 1: input, window and elapsed real time for the tick. Returns the number of `Update` calls the
/// driver has to make (0..n). Window changes are applied to the device here; the events they raise are queued
/// for [`dispatch_events`].
pub fn begin_frame(input: InputSnapshot, window: WindowState, elapsed_seconds: f64) -> u32 {
    crate::input::set_snapshot(input);
    crate::audio::advance(elapsed_seconds);
    with(|s| {
        let r = s.roots.get();
        // SAFETY: rooted objects of this host.
        unsafe {
            let w = &mut *r.window.ptr();
            w.handle = window.handle;
            let pp = &*(*r.device.ptr()).presentation_parameters.ptr();
            if (window.width, window.height) != (pp.back_buffer_width, pp.back_buffer_height) && window.width > 0 && window.height > 0 {
                // XNA's `GraphicsDeviceManager.GameWindowClientSizeChanged`: the device is reset to the client size
                // (`resizedBackBufferWidth` / `Height`); `PreferredBackBufferWidth` / `Height` keep what the game
                // set. The game relies on that difference to notice a resize (`Main.SetDisplayMode` compares the
                // preferred size with the viewport, adopts the viewport's and applies it).
                (*r.device.ptr()).reset_back_buffer(window.width, window.height);
                if !w.client_size_changed.is_null() {
                    s.events.push(w.client_size_changed);
                }
            }
            w.client_bounds = Rectangle::new(window.x, window.y, window.width, window.height);
        }
        if window.active != s.is_active {
            let h = if window.active { r.activated } else { r.deactivated };
            if !h.is_null() {
                s.events.push(h);
            }
        }
        s.is_active = window.active;
        s.window = window;

        // Game.Tick
        s.pending_updates = (0, 0);
        s.all_suppressed = true;
        s.ran_update = false;
        if s.should_exit {
            return 0;
        }
        let mut elapsed = ((elapsed_seconds * 1e7).round() as i64).clamp(0, MAX_ELAPSED_TICKS);
        let target = s.target_elapsed_time.ticks;
        if s.is_fixed_time_step {
            if (elapsed - target).abs() < target >> 6 {
                elapsed = target;
            }
            s.accumulated += elapsed;
            let n = s.accumulated / target;
            s.accumulated %= target;
            s.last_frame_elapsed = 0;
            if n == 0 {
                return 0;
            }
            if n > 1 {
                s.updates_since_slow2 = s.updates_since_slow1;
                s.updates_since_slow1 = 0;
            } else {
                s.updates_since_slow1 = s.updates_since_slow1.saturating_add(1);
                s.updates_since_slow2 = s.updates_since_slow2.saturating_add(1);
            }
            s.draw_running_slowly = s.updates_since_slow2 < 20;
            s.pending_updates = (n as u32, target);
        } else {
            s.draw_running_slowly = false;
            s.updates_since_slow1 = i32::MAX;
            s.updates_since_slow2 = i32::MAX;
            s.last_frame_elapsed = 0;
            s.pending_updates = (1, elapsed);
        }
        s.pending_updates.0
    })
}

/// Driver, step 2: runs the event handlers queued by [`begin_frame`] (`Window.ClientSizeChanged`, `Activated`,
/// `Deactivated`) and delivers typed characters to the IME service's key listeners. Calls translated code.
pub fn dispatch_events() {
    let events = with(|s| std::mem::take(&mut s.events));
    for h in events {
        h.invoke(|f| f(Gc::NULL, Gc::NULL));
    }
    crate::os::ime_dispatch_text();
}

/// Driver, step 3 (once per `Update`): the `GameTime` to pass to `Main.Update`, or `None` when no update is
/// left this tick or the game has exited.
pub fn begin_update() -> Option<Gc<GameTime>> {
    crate::sys::threading::next_phase();
    with(|s| {
        if s.pending_updates.0 == 0 || s.should_exit {
            return None;
        }
        let gt = s.roots.get().game_time;
        // SAFETY: rooted object.
        unsafe {
            let g = &mut *gt.ptr();
            g.elapsed_game_time = TimeSpan::new(s.pending_updates.1);
            g.total_game_time = TimeSpan::new(s.total);
            g.is_running_slowly = s.draw_running_slowly;
        }
        Some(gt)
    })
}

/// Driver, step 4 (after each `Main.Update`): advances game time and runs queued "thread" work
/// (`sys::threading::run_pending_work`, which calls translated code).
pub fn end_update() {
    with(|s| {
        s.all_suppressed &= s.suppress_draw;
        s.suppress_draw = false;
        s.ran_update = true;
        s.last_frame_elapsed += s.pending_updates.1;
        s.total += s.pending_updates.1;
        s.pending_updates.0 = s.pending_updates.0.saturating_sub(1);
    });
    crate::sys::threading::next_phase();
    crate::sys::threading::run_pending_work_and_dispatchers();
}

/// Driver, step 5: the `GameTime` to pass to `Main.Draw`, or `None` when this tick draws nothing (no update ran,
/// every update suppressed the draw, or the game has exited). `Game.BeginDraw` happens here.
pub fn begin_draw() -> Option<Gc<GameTime>> {
    crate::sys::threading::next_phase();
    with(|s| {
        if !s.ran_update || s.all_suppressed || s.should_exit {
            return None;
        }
        let r = s.roots.get();
        // SAFETY: rooted objects.
        unsafe {
            (*r.device.ptr()).begin_frame();
            let g = &mut *r.game_time.ptr();
            g.elapsed_game_time = TimeSpan::new(s.last_frame_elapsed);
            g.total_game_time = TimeSpan::new(s.total);
            g.is_running_slowly = s.draw_running_slowly;
        }
        Some(r.game_time)
    })
}

/// Driver, step 6: `Game.EndDraw` (`GraphicsDevice.Present`) and everything the app needs for this tick.
/// Call it every tick, also when [`begin_draw`] returned `None` (then the frame holds only resource commands).
pub fn end_draw(drew: bool) -> FrameOutput {
    let (device, window, mouse_visible, request, exit) = with(|s| (s.roots.get().device, s.roots.get().window, s.is_mouse_visible, s.window_request.take(), s.should_exit));
    // SAFETY: rooted objects; `present` may throw a C# exception (render target still bound), no borrow is held.
    let frame = unsafe { if drew { (*device.ptr()).present() } else { (*device.ptr()).list.take_frame() } };
    // SAFETY: rooted object.
    let title = unsafe { (*window.ptr()).title.get().unwrap_or("").to_owned() };
    FrameOutput { frame, audio: crate::audio::take_commands(), title, is_mouse_visible: mouse_visible, window_request: request, mouse_position_request: crate::input::take_mouse_position_request(), exit }
}

/// Driver, at shutdown: the `Exiting` handlers (`Game.Run` raises it after the loop ends). Calls translated code.
pub fn raise_exiting() {
    let h = with(|s| s.roots.get().exiting);
    if !h.is_null() {
        h.invoke(|f| f(Gc::NULL, Gc::NULL));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn window(w: i32, h: i32) -> WindowState {
        WindowState { width: w, height: h, x: 100, y: 50, active: true, handle: 0, display_width: 1920, display_height: 1080, scale: 1.0 }
    }

    /// One tick: returns (updates run, total game time after, drew).
    fn tick(dt: f64, suppress: bool) -> (u32, i64, bool) {
        let n = begin_frame(InputSnapshot::default(), window(800, 600), dt);
        let mut ran = 0;
        while let Some(gt) = begin_update() {
            let _ = gt;
            if suppress {
                suppress_draw();
            }
            end_update();
            ran += 1;
        }
        assert_eq!(ran, n);
        let drew = begin_draw().is_some();
        let _ = end_draw(drew);
        (n, with(|s| s.total), drew)
    }

    /// Hand-derived from `Game.Tick` (reference/xna/Game.cs:312) with TargetElapsedTime = 166667 ticks.
    #[test]
    fn fixed_time_step_follows_game_tick() {
        init(window(800, 600));
        // 16.9 ms is within 1/64 of the target (2604 ticks): exactly one update, no remainder
        assert_eq!(tick(0.0169, false), (1, 166_667, true));
        assert_eq!(with(|s| s.accumulated), 0);
        // 10 ms: not enough for an update, nothing is drawn
        assert_eq!(tick(0.010, false), (0, 166_667, false));
        // + 10 ms = 200000 ticks accumulated: one update, 33333 left
        assert_eq!(tick(0.010, false), (1, 333_334, true));
        assert_eq!(with(|s| s.accumulated), 33_333);
        // 50 ms + 33333 = 533333: three updates, 33332 left
        assert_eq!(tick(0.050, false), (3, 833_335, true));
        // the first slow tick only arms the counters: drawRunningSlowly needs two slow ticks within 20 updates
        assert_eq!(with(|s| (s.accumulated, s.draw_running_slowly)), (33_332, false));
        // a 3 s stall is clamped to 500 ms: (5000000 + 33332) / 166667 = 30 updates; the second slow tick in a row
        assert_eq!(tick(3.0, false).0, 30);
        assert!(with(|s| s.draw_running_slowly));
        // every update suppressed the draw: nothing is drawn
        assert_eq!(tick(0.0167, true).2, false);
        unsafe {
            let gt = &*roots().game_time.ptr();
            // GameTime.TotalGameTime is the total BEFORE the update it was passed to
            assert_eq!(gt.total_game_time().ticks, with(|s| s.total) - 166_667);
        }
    }

    #[test]
    fn variable_time_step_exit_and_window_changes() {
        init(window(800, 600));
        set_is_fixed_time_step(false);
        let n = begin_frame(InputSnapshot::default(), window(800, 600), 0.0123);
        assert_eq!(n, 1);
        let gt = begin_update().unwrap();
        unsafe { assert_eq!(((*gt.ptr()).elapsed_game_time().ticks, (*gt.ptr()).is_running_slowly()), (123_000, false)) };
        end_update();
        assert!(begin_update().is_none());
        let draw = begin_draw().unwrap();
        unsafe { assert_eq!((*draw.ptr()).elapsed_game_time().ticks, 123_000) };
        let out = end_draw(true);
        assert!(!out.exit && out.frame.back_buffer == (800, 600));

        // the user resizes the window: device and ClientBounds follow, ClientSizeChanged is raised; the manager's
        // preferred size stays what the game set (XNA keeps the resized size apart; Terraria's
        // `Main.SetDisplayMode` notices a resize by `PreferredBackBufferWidth != Viewport.Width`)
        use std::rc::Rc;
        let fired = Rc::new(std::cell::Cell::new(0));
        let f2 = fired.clone();
        unsafe { (*window_obj().ptr()).add_client_size_changed(Delegate::new(Box::new(move |_: Obj, _: Obj| f2.set(f2.get() + 1)))) };
        begin_frame(InputSnapshot::default(), window(1024, 768), 0.016);
        assert_eq!(fired.get(), 0, "events run in dispatch_events, not inside begin_frame");
        dispatch_events();
        assert_eq!(fired.get(), 1);
        unsafe {
            let r = roots();
            assert_eq!((*r.device.ptr()).viewport().bounds(), Rectangle::new(0, 0, 1024, 768));
            assert_eq!(((*r.manager.ptr()).preferred_back_buffer_width(), (*r.manager.ptr()).preferred_back_buffer_height()), (800, 600));
            assert_eq!((*r.window.ptr()).client_bounds(), Rectangle::new(100, 50, 1024, 768));
        }
        while begin_update().is_some() {
            end_update();
        }
        let _ = end_draw(begin_draw().is_some());

        // the game asks for a size: the device has it at once and the app gets the request
        unsafe {
            let m = &mut *graphics_device_manager().ptr();
            let seen = Rc::new(std::cell::Cell::new(-1));
            let s2 = seen.clone();
            m.add_preparing_device_settings(Delegate::new(Box::new(move |_: Obj, e: Gc<PreparingDeviceSettingsEventArgs>| {
                // what Main's handler does: RenderTargetUsage.PreserveContents
                (*(*e.ptr()).presentation_parameters().ptr()).render_target_usage = 1;
                s2.set(1);
            })));
            m.set_preferred_back_buffer_width(640);
            m.set_preferred_back_buffer_height(480);
            m.apply_changes();
            assert_eq!(seen.get(), 1);
            let d = &*graphics_device().ptr();
            assert_eq!((*d.presentation_parameters().ptr()).render_target_usage, 1);
            assert_eq!(((*d.presentation_parameters().ptr()).back_buffer_width(), (*d.presentation_parameters().ptr()).back_buffer_height()), (640, 480));
        }
        begin_frame(InputSnapshot::default(), window(640, 480), 0.016);
        while begin_update().is_some() {
            end_update();
        }
        let out = end_draw(begin_draw().is_some());
        assert_eq!(out.window_request, Some(WindowRequest { width: 640, height: 480, full_screen: false, vsync: true }));

        exit();
        assert_eq!(begin_frame(InputSnapshot::default(), window(640, 480), 0.016), 0);
        assert!(begin_update().is_none() && begin_draw().is_none());
        assert!(end_draw(false).exit);
    }

    fn window_obj() -> Gc<GameWindow> {
        super::window()
    }

    #[test]
    fn service_container() {
        init(window(320, 240));
        unsafe {
            let s = &mut *services().ptr();
            assert_eq!(s.get_service_named("Microsoft.Xna.Framework.Graphics.IGraphicsDeviceService"), graphics_device_manager().up());
            assert!(s.get_service_named("Some.Other.IService").is_null());
        }
    }
}
