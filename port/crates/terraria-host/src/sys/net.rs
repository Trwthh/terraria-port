//! `System.Net` / `System.Net.Sockets`: what the game's networking calls (`Terraria.Net.Sockets.TcpSocket`,
//! `DebugNetworkStream`, `Netplay`, `TcpAddress`), on non-blocking OS sockets.
//!
//! # Model: one thread, polled sockets, completions as queued work
//! The runtime is single-threaded (`threading.rs`). The original's asynchronous socket calls complete on I/O threads
//! of the thread pool; here:
//! - `NetworkStream.BeginRead` / `BeginWrite` register an operation and return at once. [`pump`] reads / writes
//!   what the socket takes without blocking; a finished operation's `AsyncCallback` is handed out by [`deliver`]
//!   and never runs inside `BeginRead` / `BeginWrite` themselves. Two places deliver: `threading::run_pending_work`
//!   (once per `Update`, and at every main-thread sleep of a blocking main loop) queues the callbacks as work
//!   items; a thread of the game that sleeps (`Thread.Sleep` on a fiber) runs them there and then, as the I/O
//!   threads of the original do while that thread sleeps. Either way a callback runs as a work item of its own:
//!   its own thread-static state, able to wait for a monitor. `EndRead` / `EndWrite` return the result or throw
//!   the `IOException` the operation ended with.
//! - `TcpListener.AcceptTcpClient` (the game's "TCP Listen Thread" blocks in it) and `TcpClient.Connect` (the
//!   "TCP Client Thread") wait by giving the frame back (`threading::wait_a_frame`): they are only called from
//!   threads of the game, which are fibers here. `Connect` runs the OS connect on a helper OS thread (no game
//!   code on it) so an unreachable host does not freeze the frame.
//! - The game's endless loops (`Netplay.ServerLoop`, `InnerClientLoop`, `ListenLoop`, `BroadcastThread`) need
//!   nothing special: they sleep every iteration, and a fiber that sleeps is suspended until the next pump.
//!
//! # What listens where
//! `TcpListener(address, port).Start()` binds what the game asks for: `IPAddress.Any` unless the launch parameter
//! `-ip` names an address (TcpSocket.cs `StartListening`), exactly as the original. With
//! `TERRARIA_NET_LOOPBACK_ONLY=1` (every automated test sets it) a listener on an unspecified address is bound to
//! 127.0.0.1 instead and `UdpClient.Send` (the LAN broadcast of `Netplay.BroadcastThread`, one datagram per second
//! to 255.255.255.255:8888) sends nothing; both are logged once.
//!
//! Not here: IPv6 listeners beyond what `std::net` gives, `Socket` members the game does not call, timeouts
//! (`ReceiveTimeout` / `SendTimeout`: the game sets none), `HttpWebRequest` (SteelSeries GameSense: left out).
//!
//! # The browser build (`target_family = "wasm"`)
//! A page cannot open TCP sockets. A `TcpClient`'s connection is a WebSocket to a local bridge
//! (`tools/ws-bridge`), which opens the TCP connection to the address the game asked for and copies bytes both
//! ways ([`wasm_net`]; the driver, terraria-web, owns the `tw.net_*` imports and installs them as a
//! [`wasm_net::Backend`], as it does for the fibers). Everything above the byte stream is the code below,
//! unchanged: the polled reads / writes, the queued callbacks. `Connect` gives the frame back
//! (`threading::wait_a_frame`, a fiber suspend) until the bridge has the TCP connection or refused it.
//!
//! Listening (the browser build's dedicated server, run by "Host & Play" in a second Worker of the page:
//! terraria-web `tw_server_*`) is the page's too: `TcpListener.Start` registers the port with the page
//! (`net_listen`), and connections arrive there from the hosting client of the same page (a MessageChannel) or from
//! outside through the bridge in reverse mode (`ws_bridge.py --listen <tcp port> --to-page`: a WebSocket per TCP
//! connection, opened by the page). `AcceptTcpClient` takes one (`net_accept`); the accepted connection is a
//! [`wasm_net::WsStream`] like any other. In the resumable `TcpSocket.ListenLoop` (tools/cs2rs DESIGN.md 8f) the
//! wait is [`TcpListener::accept_tcp_client_async`]: the thread parks instead of keeping a stack. Nothing listens on
//! a network interface of the computer except the bridge (127.0.0.1). The LAN announcement sends nothing there.

use std::cell::{Cell, RefCell};
use std::io::{ErrorKind, Read, Write};
#[cfg(not(target_family = "wasm"))]
use std::net::TcpStream;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

/// The byte stream under a connected `Socket`: an OS socket natively, a WebSocket through the bridge in the browser.
#[cfg(not(target_family = "wasm"))]
type Stream = TcpStream;
#[cfg(target_family = "wasm")]
type Stream = wasm_net::WsStream;

/// The browser build's connections (module docs, "The browser build"): a WebSocket per `TcpClient`, through the
/// driver's `tw.net_*` imports (web/www/HOST_ABI.md, "Network").
#[cfg(target_family = "wasm")]
pub mod wasm_net {
    use std::cell::Cell;
    use std::io::{Error, ErrorKind};
    use std::net::SocketAddr;

    /// The driver's imports. Handles are >= 1; a negative result is `-errno` (WASI preview1 numbering).
    #[derive(Clone, Copy)]
    pub struct Backend {
        /// Starts a connection to `host` (UTF-8 address text) : `port` through the bridge; never blocks. A handle.
        pub connect: unsafe fn(host: *const u8, len: usize, port: i32) -> i32,
        /// 0 connecting, 1 connected, 2 the peer closed (queued bytes can still be read), `-errno` failed.
        pub status: unsafe fn(handle: i32) -> i32,
        /// Queues `len` bytes for sending: the count, or `-errno`.
        pub send: unsafe fn(handle: i32, ptr: *const u8, len: usize) -> i32,
        /// Copies up to `cap` received bytes (removes them unless `peek` is 1): the count; 0 at the end of the
        /// stream; `-EAGAIN` when nothing has arrived yet.
        pub recv: unsafe fn(handle: i32, ptr: *mut u8, cap: usize, peek: i32) -> i32,
        /// Closes the connection and forgets the handle.
        pub close: unsafe fn(handle: i32) -> i32,
        /// Registers a listener on `port` with the page: a listener handle, or `-errno` (`-EADDRINUSE`).
        pub listen: unsafe fn(port: i32) -> i32,
        /// Takes a connection that arrived for listener `handle`: a connection handle (connected), `-EAGAIN` when
        /// none is waiting, or `-errno`.
        pub accept: unsafe fn(listener: i32) -> i32,
        /// Stops listener `handle`.
        pub unlisten: unsafe fn(listener: i32) -> i32,
        /// The remote end of a connection as "address:port" text (UTF-8, up to `cap` bytes): its length, or `-errno`.
        pub peer: unsafe fn(handle: i32, out: *mut u8, cap: usize) -> i32,
        /// Starts the page's dedicated server (a second Worker running this module's `tw_server_*`) with the game's
        /// launch parameters (UTF-8 text): an id, or `-errno`.
        pub server_start: unsafe fn(args: *const u8, len: usize) -> i32,
        /// 0 while that server runs, 1 when it has ended, `-errno` for an unknown id.
        pub server_status: unsafe fn(id: i32) -> i32,
        /// Ends that server at once (`Process.Kill`).
        pub server_kill: unsafe fn(id: i32) -> i32,
    }

    thread_local! {
        static BACKEND: Cell<Option<Backend>> = const { Cell::new(None) };
    }

    /// Driver: the connection imports (`None`: connecting fails with "no network in this build").
    pub fn install(backend: Option<Backend>) {
        BACKEND.set(backend);
    }

    const EADDRINUSE: i32 = 3;
    const EAGAIN: i32 = 6;
    const ECONNREFUSED: i32 = 14;
    const ECONNRESET: i32 = 15;
    const ETIMEDOUT: i32 = 73;

    fn error_of(code: i32) -> Error {
        match -code {
            EAGAIN => Error::from(ErrorKind::WouldBlock),
            EADDRINUSE => Error::new(ErrorKind::AddrInUse, "Only one usage of each socket address (protocol/network address/port) is normally permitted (a server of this page already listens on this port)."),
            ECONNREFUSED => Error::new(ErrorKind::ConnectionRefused, "No connection could be made because the target machine actively refused it (through the WebSocket bridge)."),
            ECONNRESET => Error::new(ErrorKind::ConnectionReset, "An existing connection was forcibly closed by the remote host."),
            ETIMEDOUT => Error::new(ErrorKind::TimedOut, "A connection attempt failed because the connected party did not properly respond after a period of time."),
            e => Error::other(format!("network error {e} (WASI errno)")),
        }
    }

    /// A connection through the bridge.
    pub struct WsStream {
        handle: i32,
        backend: Backend,
    }

    /// Opens a connection and waits for it, giving the frame back meanwhile (only threads of the game connect, and
    /// they are fibers). Windows' own bound: about 21 s.
    pub fn connect(addr: SocketAddr) -> std::io::Result<WsStream> {
        let Some(b) = BACKEND.get() else {
            return Err(Error::other("this build has no network backend (the page installs one)"));
        };
        let host = addr.ip().to_string();
        // SAFETY: a valid byte range for the duration of the call.
        let h = unsafe { (b.connect)(host.as_ptr(), host.len(), addr.port() as i32) };
        if h < 0 {
            return Err(error_of(h));
        }
        let s = WsStream { handle: h, backend: b };
        let t0 = std::time::Instant::now();
        loop {
            // SAFETY: plain import, numbers only.
            match unsafe { (b.status)(h) } {
                0 => {
                    if t0.elapsed() > std::time::Duration::from_secs(21) {
                        return Err(error_of(-ETIMEDOUT));
                    }
                    // the WebSocket's events arrive only between frames: a caller that cannot give the frame back
                    // would wait for ever
                    if !super::super::threading::wait_a_frame() {
                        return Err(Error::other("TcpClient.Connect in the browser build needs a thread of the game (a work fiber)"));
                    }
                }
                1 | 2 => return Ok(s),
                e => return Err(error_of(e)),
            }
        }
    }

    fn backend() -> std::io::Result<Backend> {
        BACKEND.get().ok_or_else(|| Error::other("this build has no network backend (the page installs one)"))
    }

    /// `TcpListener.Start` of the browser build: a listener of the page on `port` (module docs).
    pub fn listen(port: u16) -> std::io::Result<i32> {
        let b = backend()?;
        // SAFETY: plain import, numbers only.
        let h = unsafe { (b.listen)(port as i32) };
        if h < 0 {
            Err(error_of(h))
        } else {
            Ok(h)
        }
    }
    /// `TcpListener.Stop`.
    pub fn unlisten(listener: i32) {
        if let Ok(b) = backend() {
            // SAFETY: plain import.
            unsafe { (b.unlisten)(listener) };
        }
    }
    /// A connection waiting for `listener`, with its remote address; `WouldBlock` when none is.
    pub fn accept(listener: i32) -> std::io::Result<(WsStream, SocketAddr)> {
        let b = backend()?;
        // SAFETY: plain import, numbers only.
        let h = unsafe { (b.accept)(listener) };
        if h < 0 {
            return Err(error_of(h));
        }
        let mut buf = [0u8; 96];
        // SAFETY: the buffer is ours for the duration of the call.
        let n = unsafe { (b.peer)(h, buf.as_mut_ptr(), buf.len()) };
        let peer = if n > 0 { std::str::from_utf8(&buf[..n as usize]).ok().and_then(|t| t.parse::<SocketAddr>().ok()) } else { None };
        // a connection from the hosting client of the same page has no address of its own: loopback, a port per handle
        let peer = peer.unwrap_or_else(|| SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 50000u16.wrapping_add(h as u16)));
        Ok((WsStream { handle: h, backend: b }, peer))
    }

    /// `Process.Start()` of `TerrariaServer.exe` (Host & Play) in the browser build: the page starts its dedicated
    /// server Worker with these launch parameters. An id for [`server_exited`] / [`server_kill`].
    pub fn server_start(args: &str) -> std::io::Result<i32> {
        let b = backend()?;
        // SAFETY: a valid byte range for the duration of the call.
        let id = unsafe { (b.server_start)(args.as_ptr(), args.len()) };
        if id < 0 {
            Err(error_of(id))
        } else {
            Ok(id)
        }
    }
    /// `Process.HasExited` of a server [`server_start`] started.
    pub fn server_exited(id: i32) -> bool {
        // SAFETY: plain import.
        backend().map(|b| unsafe { (b.server_status)(id) } != 0).unwrap_or(true)
    }
    /// `Process.Kill()` of a server [`server_start`] started.
    pub fn server_kill(id: i32) {
        if let Ok(b) = backend() {
            // SAFETY: plain import.
            unsafe { (b.server_kill)(id) };
        }
    }

    impl WsStream {
        fn recv(&self, buf: &mut [u8], peek: bool) -> std::io::Result<usize> {
            // SAFETY: the buffer is ours for the duration of the call.
            let n = unsafe { (self.backend.recv)(self.handle, buf.as_mut_ptr(), buf.len(), peek as i32) };
            if n >= 0 {
                Ok(n as usize)
            } else {
                Err(error_of(n))
            }
        }
        /// As `TcpStream::peek`: the bytes waiting, `Ok(0)` at the end of the stream, `WouldBlock` when none yet.
        pub fn peek(&self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.recv(buf, true)
        }
        pub fn shutdown(&self, _how: std::net::Shutdown) -> std::io::Result<()> {
            // SAFETY: plain import.
            unsafe { (self.backend.close)(self.handle) };
            Ok(())
        }
        /// The bridge sets TCP_NODELAY on its side; nothing to do here.
        pub fn set_nodelay(&self, _v: bool) -> std::io::Result<()> {
            Ok(())
        }
        /// Always non-blocking.
        pub fn set_nonblocking(&self, _v: bool) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl std::io::Read for WsStream {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.recv(buf, false)
        }
    }
    impl std::io::Write for WsStream {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if buf.is_empty() {
                return Ok(0);
            }
            // SAFETY: as `recv`.
            let n = unsafe { (self.backend.send)(self.handle, buf.as_ptr(), buf.len()) };
            if n >= 0 {
                Ok(n as usize)
            } else {
                Err(error_of(n))
            }
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl Drop for WsStream {
        fn drop(&mut self) {
            // SAFETY: plain import; closing a closed handle is harmless (-EBADF).
            unsafe { (self.backend.close)(self.handle) };
        }
    }
}

use terraria_gen_rt::rt::{self, Arr, Delegate, Gc, Marker, Obj, Rooted, Str, Trace};

/// `AsyncCallback`
pub type AsyncCallback = Delegate<dyn Fn(Obj)>;

fn socket_error(message: &str) -> ! {
    rt::throw(rt::exc("System.Net.Sockets.SocketException", Str::new(message)))
}
fn io_error(message: &str) -> ! {
    rt::throw(rt::exc("System.IO.IOException", Str::new(message)))
}
fn disposed(what: &'static str) -> ! {
    rt::throw(rt::exc("System.ObjectDisposedException", Str::lit(what)))
}

/// Tests: listeners on 127.0.0.1 only, no LAN broadcast (module docs).
pub fn loopback_only() -> bool {
    std::env::var_os("TERRARIA_NET_LOOPBACK_ONLY").is_some_and(|v| !v.is_empty() && v != "0")
}

fn log_once(flag: &'static std::thread::LocalKey<Cell<bool>>, text: &str) {
    if !flag.replace(true) {
        eprintln!("[host] net: {text}");
    }
}

// ------------------------------------------------------------------------------------------------ IPAddress

/// `System.Net.IPAddress`
pub struct IPAddress {
    pub addr: IpAddr,
}
// SAFETY: no handles.
unsafe impl Trace for IPAddress {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut Marker) {}
    unsafe fn obj_to_string(this: *const Self) -> Str {
        Str::new(&(*this).addr.to_string())
    }
    unsafe fn obj_equals(this: *const Self, other: Obj) -> bool {
        rt::obj_is::<IPAddress>(other) && (*(other.0 as *const IPAddress)).addr == (*this).addr
    }
}

thread_local! {
    static WELL_KNOWN: RefCell<Option<[Rooted<Gc<IPAddress>>; 3]>> = const { RefCell::new(None) };
    static OPS: RefCell<Vec<Rooted<Gc<Op>>>> = const { RefCell::new(Vec::new()) };
    /// finished operations whose callback has not been handed out yet ([`deliver`])
    static DONE: RefCell<Vec<Rooted<Gc<Op>>>> = const { RefCell::new(Vec::new()) };
    static LOGGED_BIND: Cell<bool> = const { Cell::new(false) };
    static LOGGED_BROADCAST: Cell<bool> = const { Cell::new(false) };
    /// [`set_virtual`]
    static VIRTUAL: Cell<bool> = const { Cell::new(false) };
    /// the virtual connections, in the order they were connected: what each sent, closed or not
    static VCONNS: RefCell<Vec<(Vec<u8>, bool)>> = const { RefCell::new(Vec::new()) };
}

/// Tests (the net run, tools/refdump_net/NetVirt.cs, the same behaviour): every `TcpClient.Connect` from now on
/// gives a VIRTUAL connection at once, to the address asked for, without an OS socket. It is connected until closed;
/// what is written to it is kept ([`virtual_take`]); nothing ever arrives on it (no data available, reads never
/// complete): the test delivers the other side's bytes through the game's `NetMessage.ReceiveBytes`.
pub fn set_virtual(on: bool) {
    VIRTUAL.set(on);
}
/// What virtual connection `k` (in connection order) was sent since the last call.
pub fn virtual_take(k: usize) -> Vec<u8> {
    VCONNS.with(|v| v.borrow_mut().get_mut(k).map(|c| core::mem::take(&mut c.0)).unwrap_or_default())
}
/// Number of virtual connections made so far.
pub fn virtual_count() -> usize {
    VCONNS.with(|v| v.borrow().len())
}

fn well_known() -> (Gc<IPAddress>, Gc<IPAddress>, Gc<IPAddress>) {
    WELL_KNOWN.with(|w| {
        let mut w = w.borrow_mut();
        if w.is_none() {
            *w = Some([Ipv4Addr::UNSPECIFIED, Ipv4Addr::LOCALHOST, Ipv4Addr::BROADCAST].map(|a| Rooted::new(IPAddress::new_gc(IpAddr::V4(a)))));
        }
        let w = w.as_ref().expect("set above");
        (w[0].get(), w[1].get(), w[2].get())
    })
}

impl IPAddress {
    pub fn new_gc(addr: IpAddr) -> Gc<IPAddress> {
        rt::alloc_init(IPAddress { addr })
    }
    /// `IPAddress.Any` (0.0.0.0)
    pub fn any() -> Gc<IPAddress> {
        well_known().0
    }
    /// `IPAddress.Loopback` (127.0.0.1)
    pub fn loopback() -> Gc<IPAddress> {
        well_known().1
    }
    /// `IPAddress.Broadcast` (255.255.255.255)
    pub fn broadcast() -> Gc<IPAddress> {
        well_known().2
    }
    /// `IPAddress.TryParse(text, out address)`: dotted IPv4 and IPv6 text. Deviation: .NET also accepts the
    /// shortened and numeric IPv4 forms of `inet_aton` ("127.1", "2130706433"); those answer false here.
    ///
    /// # Safety
    /// `out` must be writable.
    pub unsafe fn try_parse(text: Str, out: *mut Gc<IPAddress>) -> bool {
        let parsed = text.get().and_then(|t| {
            let t = t.trim();
            t.parse::<Ipv4Addr>().map(IpAddr::V4).ok().or_else(|| t.trim_start_matches('[').trim_end_matches(']').parse::<Ipv6Addr>().map(IpAddr::V6).ok())
        });
        match parsed {
            Some(a) => {
                *out = IPAddress::new_gc(a);
                true
            }
            None => {
                *out = Gc::NULL;
                false
            }
        }
    }
    /// `AddressFamily`: `InterNetwork` (2) or `InterNetworkV6` (23).
    pub fn address_family(&self) -> i32 {
        match self.addr {
            IpAddr::V4(_) => 2,
            IpAddr::V6(_) => 23,
        }
    }
    pub fn to_str(&self) -> Str {
        Str::new(&self.addr.to_string())
    }
    /// `Equals(object)`: the same address.
    pub fn equals(&self, other: Obj) -> bool {
        // SAFETY: the type was checked.
        rt::obj_is::<IPAddress>(other) && unsafe { (*(other.0 as *const IPAddress)).addr } == self.addr
    }
}

/// `System.Net.IPEndPoint`
pub struct IPEndPoint {
    pub address: Gc<IPAddress>,
    pub port: i32,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for IPEndPoint {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.address)
    }
    unsafe fn obj_to_string(this: *const Self) -> Str {
        (*this).to_str()
    }
}
impl IPEndPoint {
    pub fn new_gc(address: Gc<IPAddress>, port: i32) -> Gc<IPEndPoint> {
        if address.is_null() {
            rt::throw(rt::exc("System.ArgumentNullException", Str::lit("address")));
        }
        if !(0..=65535).contains(&port) {
            rt::throw(rt::exc("System.ArgumentOutOfRangeException", Str::lit("port")));
        }
        rt::alloc_init(IPEndPoint { address, port })
    }
    pub fn address(&self) -> Gc<IPAddress> {
        self.address
    }
    pub fn port(&self) -> i32 {
        self.port
    }
    fn socket_addr(&self) -> SocketAddr {
        // SAFETY: the constructor refuses null.
        SocketAddr::new(unsafe { (*self.address.ptr()).addr }, self.port as u16)
    }
    /// `ToString()`: `address:port` (`[address]:port` for IPv6), as .NET.
    pub fn to_str(&self) -> Str {
        Str::new(&self.socket_addr().to_string())
    }
}

/// `System.Net.IPHostEntry`
pub struct IPHostEntry {
    pub address_list: Arr<Gc<IPAddress>>,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for IPHostEntry {
    unsafe fn trace(&self, m: &mut Marker) {
        self.address_list.trace(m)
    }
}
impl IPHostEntry {
    pub fn address_list(&self) -> Arr<Gc<IPAddress>> {
        self.address_list
    }
}

#[cfg_attr(target_family = "wasm", allow(unreachable_code))]
fn resolve(host: Str) -> Result<Vec<IpAddr>, String> {
    let name = host.get().unwrap_or("");
    if name.is_empty() {
        // .NET resolves "" to the local host's addresses
        return Ok(vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]);
    }
    // the browser build has no name service (WASI): addresses and "localhost" only
    #[cfg(target_family = "wasm")]
    {
        if name.eq_ignore_ascii_case("localhost") {
            return Ok(vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]);
        }
        if let Ok(a) = name.trim_start_matches('[').trim_end_matches(']').parse::<IpAddr>() {
            return Ok(vec![a]);
        }
        return Err(format!("No such host is known: {name} (the browser build resolves no names; type the address)"));
    }
    match (name, 0u16).to_socket_addrs() {
        Ok(it) => {
            let mut out: Vec<IpAddr> = Vec::new();
            for a in it {
                if !out.contains(&a.ip()) {
                    out.push(a.ip());
                }
            }
            if out.is_empty() {
                Err(format!("No such host is known: {name}"))
            } else {
                Ok(out)
            }
        }
        Err(e) => Err(format!("No such host is known: {name} ({e})")),
    }
}
fn address_array(list: &[IpAddr]) -> Arr<Gc<IPAddress>> {
    let v: Vec<Gc<IPAddress>> = list.iter().map(|a| IPAddress::new_gc(*a)).collect();
    Arr::from_slice(&v)
}

/// `Dns.GetHostName()`: the machine's name.
pub fn dns_get_host_name() -> Str {
    Str::new(&std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "localhost".to_owned()))
}
/// `Dns.GetHostEntry(name)`: blocking, as in .NET. `SocketException` for an unknown host.
pub fn dns_get_host_entry(host: Str) -> Gc<IPHostEntry> {
    match resolve(host) {
        Ok(list) => rt::alloc_init(IPHostEntry { address_list: address_array(&list) }),
        Err(e) => socket_error(&e),
    }
}
/// `Dns.BeginGetHostAddresses(name, callback, state)`: resolved now (the OS call blocks; `Netplay.SetRemoteIPAsync`
/// calls it from the menu for a typed server name), the callback is queued.
pub fn dns_begin_get_host_addresses(host: Str, callback: AsyncCallback, state: Obj) -> Obj {
    let r = match resolve(host) {
        Ok(list) => rt::alloc_init(AsyncResult { state, bytes: 0, error: None, addresses: address_array(&list), completed: true }),
        Err(e) => rt::alloc_init(AsyncResult { state, bytes: 0, error: Some(e), addresses: Arr::from_slice(&[]), completed: true }),
    };
    if !callback.is_null() {
        super::threading::thread_pool_queue_user_work_item(callback, r.up());
    }
    r.up()
}
/// `Dns.EndGetHostAddresses(result)`
pub fn dns_end_get_host_addresses(result: Obj) -> Arr<Gc<IPAddress>> {
    let r = async_result(result);
    // SAFETY: `async_result` checked the type.
    unsafe {
        if let Some(e) = &(*r.ptr()).error {
            socket_error(e);
        }
        (*r.ptr()).addresses
    }
}

// ------------------------------------------------------------------------------------------------ IAsyncResult

/// The `IAsyncResult` of a socket operation or a name lookup.
pub struct AsyncResult {
    pub state: Obj,
    bytes: i32,
    error: Option<String>,
    addresses: Arr<Gc<IPAddress>>,
    completed: bool,
}
// SAFETY: both handles are reported.
unsafe impl Trace for AsyncResult {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.state);
        self.addresses.trace(m);
    }
}
fn async_result(o: Obj) -> Gc<AsyncResult> {
    if o.is_null() {
        rt::throw(rt::exc("System.ArgumentNullException", Str::lit("asyncResult")));
    }
    if !rt::obj_is::<AsyncResult>(o) {
        rt::throw(rt::exc("System.ArgumentException", Str::lit("The IAsyncResult object was not returned from the corresponding asynchronous method on this class.")));
    }
    Gc(o.0 as *mut AsyncResult)
}
/// `IAsyncResult.AsyncState` of an object of this module (the host body of the interface member: translated
/// implementers are dispatched by the generated code before it gets here).
pub fn async_state(o: Obj) -> Obj {
    // SAFETY: `async_result` checked the type.
    unsafe { (*async_result(o).ptr()).state }
}
/// `IAsyncResult.IsCompleted`
pub fn async_is_completed(o: Obj) -> bool {
    // SAFETY: as above.
    unsafe { (*async_result(o).ptr()).completed }
}

// ------------------------------------------------------------------------------------------------ Socket

/// `System.Net.Sockets.Socket` of a `TcpClient`.
pub struct Socket {
    stream: Option<Stream>,
    connected: bool,
    remote: Option<SocketAddr>,
    closed: bool,
    /// a virtual connection ([`set_virtual`]): its index
    virt: Option<usize>,
}
// SAFETY: no handles.
unsafe impl Trace for Socket {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut Marker) {}
}
impl Socket {
    fn new_gc() -> Gc<Socket> {
        rt::alloc_init(Socket { stream: None, connected: false, remote: None, closed: false, virt: None })
    }
    /// Bytes that can be read without blocking, up to 64 KiB (the game compares with zero).
    fn peek(&self) -> std::io::Result<usize> {
        if self.virt.is_some() {
            // a virtual connection: nothing ever arrives
            return if self.connected { Err(std::io::Error::from(ErrorKind::WouldBlock)) } else { Ok(0) };
        }
        match &self.stream {
            Some(s) => {
                let mut buf = [0u8; 1];
                match s.peek(&mut buf) {
                    Ok(0) => Ok(0),
                    Ok(_) => {
                        let mut big = vec![0u8; 65536];
                        s.peek(&mut big)
                    }
                    Err(e) => Err(e),
                }
            }
            None => Err(std::io::Error::new(ErrorKind::NotConnected, "the socket is not connected")),
        }
    }
    /// `Available`: 0 when nothing is waiting, also at end of stream and on a connection the peer reset (.NET asks
    /// the OS for the byte count, `FIONREAD`, which answers 0 there; the failure shows at the next read. The
    /// game's `TcpSocket.IsConnected` relies on it: `!Poll(0, SelectRead) || Available != 0`).
    pub fn available(&self) -> i32 {
        if self.closed {
            disposed("System.Net.Sockets.Socket");
        }
        match self.peek() {
            Ok(n) => n as i32,
            Err(_) => 0,
        }
    }
    /// `Poll(microSeconds, mode)` with a zero wait (the game's only use: `Poll(0, SelectMode.SelectRead)`).
    /// `SelectRead` (0): data is waiting, or the peer closed, or the connection failed. `SelectWrite` (1): connected.
    /// `SelectError` (2): false.
    pub fn poll(&self, _micro_seconds: i32, mode: i32) -> bool {
        if self.closed {
            disposed("System.Net.Sockets.Socket");
        }
        match mode {
            0 => match self.peek() {
                Ok(_) => true,
                Err(e) => e.kind() != ErrorKind::WouldBlock,
            },
            1 => self.connected,
            _ => false,
        }
    }
    /// `RemoteEndPoint`
    pub fn remote_end_point(&self) -> Gc<IPEndPoint> {
        match self.remote {
            Some(a) => IPEndPoint::new_gc(IPAddress::new_gc(a.ip()), a.port() as i32),
            None => socket_error("The socket is not connected."),
        }
    }
}

// ------------------------------------------------------------------------------------------------ TcpClient

/// `System.Net.Sockets.TcpClient`
pub struct TcpClient {
    client: Gc<Socket>,
    stream: Gc<NetworkStream>,
    no_delay: bool,
}
// SAFETY: both handles are reported.
unsafe impl Trace for TcpClient {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.client);
        m.mark(self.stream);
    }
}

/// The address a connection really goes to. `TERRARIA_HOSTPLAY_PORT=<n>` (tests): "Host & Play" always uses port
/// 7777 (`Main.HostAndPlay`: `Netplay.ListenPort = 7777`, no `-port` for the server it starts); with the variable
/// the server gets `-port n` (`misc::Process::start`) and the client's connection to 127.0.0.1:7777 goes to n.
fn redirect(addr: SocketAddr) -> SocketAddr {
    if addr.ip().is_loopback() && addr.port() == 7777 {
        if let Some(p) = std::env::var("TERRARIA_HOSTPLAY_PORT").ok().and_then(|v| v.parse::<u16>().ok()) {
            return SocketAddr::new(addr.ip(), p);
        }
    }
    addr
}

impl TcpClient {
    /// `new TcpClient()`
    pub fn new_gc() -> Gc<TcpClient> {
        let client = Socket::new_gc();
        rt::alloc_init(TcpClient { client, stream: Gc::NULL, no_delay: false })
    }
    /// The connection `AcceptTcpClient` returns.
    fn of_stream(s: Stream, remote: SocketAddr) -> Gc<TcpClient> {
        let _ = s.set_nonblocking(true);
        let client = rt::alloc_init(Socket { stream: Some(s), connected: true, remote: Some(remote), closed: false, virt: None });
        rt::alloc_init(TcpClient { client, stream: Gc::NULL, no_delay: false })
    }
    /// `NoDelay` (set)
    pub fn set_no_delay(&mut self, v: bool) {
        self.no_delay = v;
        // SAFETY: the socket object lives as long as the client.
        if let Some(s) = unsafe { &(*self.client.ptr()).stream } {
            let _ = s.set_nodelay(v);
        }
    }
    /// `Client`: null after `Close()`, as .NET.
    pub fn client(&self) -> Gc<Socket> {
        // SAFETY: as above.
        if unsafe { (*self.client.ptr()).closed } {
            Gc::NULL
        } else {
            self.client
        }
    }
    /// `Connected`
    pub fn connected(&self) -> bool {
        // SAFETY: as above.
        unsafe { (*self.client.ptr()).connected && !(*self.client.ptr()).closed }
    }
    /// `Connect(address, port)`: blocks the calling THREAD OF THE GAME until the connection stands or fails
    /// (`SocketException`). The OS connect runs on a helper thread; the caller gives the frame back meanwhile.
    pub fn connect(&mut self, address: Gc<IPAddress>, port: i32) {
        if address.is_null() {
            rt::throw(rt::exc("System.ArgumentNullException", Str::lit("address")));
        }
        // SAFETY: checked above; the socket object lives as long as the client.
        let (addr, sock) = unsafe { (redirect(SocketAddr::new((*address.ptr()).addr, port as u16)), &mut *self.client.ptr()) };
        if sock.closed {
            disposed("System.Net.Sockets.TcpClient");
        }
        if sock.connected {
            socket_error("A connect request was made on an already connected socket.");
        }
        if VIRTUAL.get() {
            sock.virt = Some(VCONNS.with(|v| {
                let mut v = v.borrow_mut();
                v.push((Vec::new(), false));
                v.len() - 1
            }));
            sock.remote = Some(addr);
            sock.connected = true;
            return;
        }
        #[cfg(target_family = "wasm")]
        let result = wasm_net::connect(addr);
        #[cfg(not(target_family = "wasm"))]
        let result = {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::Builder::new()
                .name("host connect".to_owned())
                .spawn(move || {
                    // Windows gives up after about 21 s by itself; the same bound here
                    let _ = tx.send(TcpStream::connect_timeout(&addr, Duration::from_secs(21)));
                })
                .expect("host net: the connect helper thread");
            loop {
                match rx.try_recv() {
                    Ok(r) => break r,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        if !super::threading::wait_a_frame() {
                            std::thread::sleep(Duration::from_millis(1));
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => break Err(std::io::Error::other("the connect helper thread ended")),
                }
            }
        };
        match result {
            Ok(s) => {
                let _ = s.set_nonblocking(true);
                let _ = s.set_nodelay(self.no_delay);
                sock.remote = Some(addr);
                sock.stream = Some(s);
                sock.connected = true;
            }
            Err(e) => socket_error(&format!("{e} {addr}")),
        }
    }
    /// `Close()`: the socket is closed; operations still pending on it end with an error at the next [`pump`].
    pub fn close(&mut self) {
        // SAFETY: the socket object lives as long as the client.
        let sock = unsafe { &mut *self.client.ptr() };
        if let Some(s) = sock.stream.take() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
        if let Some(k) = sock.virt {
            VCONNS.with(|v| v.borrow_mut()[k].1 = true);
        }
        sock.connected = false;
        sock.closed = true;
    }
    /// `GetStream()`: `InvalidOperationException` when not connected, as .NET.
    pub fn get_stream(this: Gc<TcpClient>) -> Gc<NetworkStream> {
        // SAFETY: `this` is a live client (null: NullReferenceException in `ptr`).
        unsafe {
            let c = &mut *this.ptr();
            if (*c.client.ptr()).closed {
                disposed("System.Net.Sockets.TcpClient");
            }
            if !(*c.client.ptr()).connected {
                rt::throw(rt::exc("System.InvalidOperationException", Str::lit("The operation is not allowed on non-connected sockets.")));
            }
            if c.stream.is_null() {
                c.stream = rt::wbv(rt::alloc_init(NetworkStream { socket: c.client }));
            }
            c.stream
        }
    }
}

// ------------------------------------------------------------------------------------------------ NetworkStream

/// `System.Net.Sockets.NetworkStream`
pub struct NetworkStream {
    socket: Gc<Socket>,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for NetworkStream {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.socket)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum OpKind {
    Read,
    Write,
}

/// A read or write that has not finished.
struct Op {
    kind: OpKind,
    socket: Gc<Socket>,
    /// read: where the bytes go
    buffer: Arr<u8>,
    offset: i32,
    count: i32,
    /// write: the bytes (copied when the operation began: the kernel does the same)
    data: Vec<u8>,
    written: usize,
    callback: AsyncCallback,
    result: Gc<AsyncResult>,
}
// SAFETY: every handle is reported.
unsafe impl Trace for Op {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.socket);
        self.buffer.trace(m);
        self.callback.trace(m);
        m.mark(self.result);
    }
}

fn check_range(buffer: Arr<u8>, offset: i32, count: i32) {
    if buffer.is_null() {
        rt::throw(rt::exc("System.ArgumentNullException", Str::lit("buffer")));
    }
    if offset < 0 || offset > buffer.len() {
        rt::throw(rt::exc("System.ArgumentOutOfRangeException", Str::lit("offset")));
    }
    if count < 0 || count > buffer.len() - offset {
        rt::throw(rt::exc("System.ArgumentOutOfRangeException", Str::lit("size")));
    }
}

impl NetworkStream {
    /// `DataAvailable`
    pub fn data_available(&self) -> bool {
        // SAFETY: the socket object lives as long as the stream.
        let s = unsafe { &*self.socket.ptr() };
        if s.closed {
            disposed("System.Net.Sockets.NetworkStream");
        }
        matches!(s.peek(), Ok(n) if n > 0)
    }
    fn begin(&self, kind: OpKind, buffer: Arr<u8>, offset: i32, count: i32, callback: AsyncCallback, state: Obj) -> Obj {
        check_range(buffer, offset, count);
        // SAFETY: the socket object lives as long as the stream.
        if unsafe { (*self.socket.ptr()).closed } {
            disposed("System.Net.Sockets.NetworkStream");
        }
        let result = rt::alloc_init(AsyncResult { state, bytes: 0, error: None, addresses: Arr::from_slice(&[]), completed: false });
        let data = if kind == OpKind::Write { buffer.as_slice()[offset as usize..(offset + count) as usize].to_vec() } else { Vec::new() };
        let op = rt::alloc_init(Op { kind, socket: self.socket, buffer, offset, count, data, written: 0, callback, result });
        OPS.with(|o| o.borrow_mut().push(Rooted::new(op)));
        result.up()
    }
    /// `BeginRead(buffer, offset, size, callback, state)`: completes when at least one byte arrived (or the peer
    /// closed: 0 bytes), at a [`pump`].
    pub fn begin_read(&self, buffer: Arr<u8>, offset: i32, count: i32, callback: AsyncCallback, state: Obj) -> Obj {
        self.begin(OpKind::Read, buffer, offset, count, callback, state)
    }
    /// `BeginWrite(buffer, offset, size, callback, state)`: completes when every byte was handed to the OS. Writes
    /// on one socket complete in the order they began.
    pub fn begin_write(&self, buffer: Arr<u8>, offset: i32, count: i32, callback: AsyncCallback, state: Obj) -> Obj {
        let r = self.begin(OpKind::Write, buffer, offset, count, callback, state);
        // most writes fit the socket buffer: hand them over now, so the bytes leave before the next frame
        pump();
        r
    }
    /// `EndRead(result)`: the number of bytes, or the `IOException` the read ended with.
    pub fn end_read(&self, result: Obj) -> i32 {
        let r = async_result(result);
        // SAFETY: `async_result` checked the type.
        unsafe {
            if let Some(e) = &(*r.ptr()).error {
                io_error(e);
            }
            (*r.ptr()).bytes
        }
    }
    /// `EndWrite(result)`
    pub fn end_write(&self, result: Obj) {
        let r = async_result(result);
        // SAFETY: as above.
        unsafe {
            if let Some(e) = &(*r.ptr()).error {
                io_error(e);
            }
        }
    }
}

/// Operations registered and not finished.
pub fn pending_operations() -> usize {
    OPS.with(|o| o.borrow().len())
}

/// Hands out the callbacks of the operations that finished since the last call, oldest first, each with its
/// `IAsyncResult`. The caller decides how they run: `threading::run_pending_work` queues them as work items,
/// a sleeping thread of the game runs them inline (threading.rs, "Threads that never end").
pub fn deliver(f: &mut dyn FnMut(AsyncCallback, Obj)) {
    let done: Vec<Rooted<Gc<Op>>> = DONE.with(|d| core::mem::take(&mut *d.borrow_mut()));
    for r in done {
        // SAFETY: rooted, so alive.
        let (cb, res) = unsafe { ((*r.get().ptr()).callback, (*r.get().ptr()).result.up::<rt::Object>()) };
        // the callback may run a collection point: keep the pair alive through the call
        let _keep = r;
        f(cb, res);
    }
}

/// Moves every pending read / write as far as its socket allows without blocking. The callbacks of the finished
/// ones wait for [`deliver`]. Returns how many finished. Never runs game code.
pub fn pump() -> usize {
    let ops: Vec<Rooted<Gc<Op>>> = OPS.with(|o| core::mem::take(&mut *o.borrow_mut()));
    if ops.is_empty() {
        return 0;
    }
    let mut kept: Vec<Rooted<Gc<Op>>> = Vec::with_capacity(ops.len());
    // a socket with an unfinished write: later writes on it wait their turn
    let mut blocked: Vec<*const Socket> = Vec::new();
    let mut finished = 0;
    for r in ops {
        let g = r.get();
        // SAFETY: rooted, so alive; nothing else holds a reference into it during this loop.
        let op = unsafe { &mut *g.ptr() };
        // SAFETY: the socket handle is traced by the operation.
        let sock = unsafe { &mut *op.socket.ptr() };
        let sp = op.socket.0 as *const Socket;
        let mut outcome: Option<Result<i32, String>> = None;
        if let Some(k) = sock.virt {
            // a virtual connection: a write is kept whole at once, a read never completes; after Close both fail
            match op.kind {
                _ if sock.closed => outcome = Some(Err("The socket was closed.".to_owned())),
                OpKind::Write => {
                    VCONNS.with(|v| v.borrow_mut()[k].0.extend_from_slice(&op.data));
                    op.written = op.data.len();
                    outcome = Some(Ok(op.data.len() as i32));
                }
                OpKind::Read => {}
            }
        } else {
        match (&mut sock.stream, op.kind) {
            (None, _) => outcome = Some(Err("The socket was closed.".to_owned())),
            (Some(s), OpKind::Read) => {
                let mut tmp = vec![0u8; op.count as usize];
                match s.read(&mut tmp) {
                    Ok(n) => {
                        // SAFETY: `check_range` validated offset + count against the array when the read began.
                        unsafe { core::ptr::copy_nonoverlapping(tmp.as_ptr(), op.buffer.at(0).add(op.offset as usize), n) };
                        if n == 0 && op.count > 0 {
                            sock.connected = false;
                        }
                        outcome = Some(Ok(n as i32));
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::Interrupted => {}
                    Err(e) => {
                        sock.connected = false;
                        outcome = Some(Err(format!("Unable to read data from the transport connection: {e}")));
                    }
                }
            }
            (Some(s), OpKind::Write) => {
                if blocked.contains(&sp) {
                    kept.push(r);
                    continue;
                }
                loop {
                    if op.written == op.data.len() {
                        outcome = Some(Ok(op.data.len() as i32));
                        break;
                    }
                    match s.write(&op.data[op.written..]) {
                        Ok(0) => {
                            sock.connected = false;
                            outcome = Some(Err("Unable to write data to the transport connection: the connection was closed.".to_owned()));
                            break;
                        }
                        Ok(n) => op.written += n,
                        Err(e) if e.kind() == ErrorKind::WouldBlock => {
                            blocked.push(sp);
                            break;
                        }
                        Err(e) if e.kind() == ErrorKind::Interrupted => {}
                        Err(e) => {
                            sock.connected = false;
                            outcome = Some(Err(format!("Unable to write data to the transport connection: {e}")));
                            break;
                        }
                    }
                }
            }
        }
        }
        match outcome {
            None => kept.push(r),
            Some(res) => {
                // SAFETY: the result object is traced by the operation.
                unsafe {
                    let ar = &mut *op.result.ptr();
                    match res {
                        Ok(n) => ar.bytes = n,
                        Err(e) => ar.error = Some(e),
                    }
                    ar.completed = true;
                }
                if !op.callback.is_null() {
                    DONE.with(|d| d.borrow_mut().push(r));
                }
                finished += 1;
            }
        }
    }
    // operations begun meanwhile (none: nothing above runs game code) stay behind the kept ones
    OPS.with(|o| {
        let mut o = o.borrow_mut();
        kept.append(&mut o);
        *o = kept;
    });
    finished
}

// ------------------------------------------------------------------------------------------------ TcpListener

/// `System.Net.Sockets.TcpListener`
pub struct TcpListener {
    addr: IpAddr,
    port: i32,
    #[cfg(not(target_family = "wasm"))]
    listener: Option<std::net::TcpListener>,
    /// the page's listener (module docs, "The browser build")
    #[cfg(target_family = "wasm")]
    listener: Option<i32>,
}
// SAFETY: no handles.
unsafe impl Trace for TcpListener {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut Marker) {}
}
impl TcpListener {
    /// `new TcpListener(localaddr, port)`
    pub fn new_gc(address: Gc<IPAddress>, port: i32) -> Gc<TcpListener> {
        if address.is_null() {
            rt::throw(rt::exc("System.ArgumentNullException", Str::lit("localaddr")));
        }
        if !(0..=65535).contains(&port) {
            rt::throw(rt::exc("System.ArgumentOutOfRangeException", Str::lit("port")));
        }
        // SAFETY: checked above.
        rt::alloc_init(TcpListener { addr: unsafe { (*address.ptr()).addr }, port, listener: None })
    }
    /// `Start()` of the browser build: the page's listener on the port (module docs, "The browser build").
    #[cfg(target_family = "wasm")]
    pub fn start(&mut self) {
        if self.listener.is_some() {
            return;
        }
        match wasm_net::listen(self.port as u16) {
            Ok(h) => {
                eprintln!("[host] net: listening on port {} of the page (the hosting client of the page and the bridge's reverse mode connect to it)", self.port);
                self.listener = Some(h);
            }
            Err(e) => socket_error(&format!("{e} {}:{}", self.addr, self.port)),
        }
    }
    /// `Stop()`
    #[cfg(target_family = "wasm")]
    pub fn stop(&mut self) {
        if let Some(h) = self.listener.take() {
            wasm_net::unlisten(h);
        }
    }
    /// A waiting connection, if there is one (`None`: none yet). `SocketException` when the listener was stopped.
    fn try_accept(this: Gc<TcpListener>) -> Option<Gc<TcpClient>> {
        // SAFETY: `this` is a live listener (null: NullReferenceException in `ptr`); the reference ends here.
        #[cfg(target_family = "wasm")]
        let r = match unsafe { (*this.ptr()).listener } {
            Some(h) => wasm_net::accept(h),
            None => socket_error("A blocking operation was interrupted by a call to WSACancelBlockingCall."),
        };
        // SAFETY: as above.
        #[cfg(not(target_family = "wasm"))]
        let r = match unsafe { &(*this.ptr()).listener } {
            Some(l) => l.accept(),
            None => socket_error("A blocking operation was interrupted by a call to WSACancelBlockingCall."),
        };
        match r {
            Ok((s, remote)) => Some(TcpClient::of_stream(s, remote)),
            Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::Interrupted => None,
            Err(e) => socket_error(&e.to_string()),
        }
    }
    /// `AcceptTcpClient()` inside a resumable form (the translator's config "resumable" / "await": the
    /// browser build's `TcpSocket.ListenLoop`): while no connection is waiting, the thread's machine parks
    /// (`threading::park_async`) instead of keeping a suspended stack. Elsewhere it waits as [`Self::accept_tcp_client`].
    pub fn accept_tcp_client_async(this: Gc<TcpListener>) -> impl std::future::Future<Output = Gc<TcpClient>> {
        // SAFETY: as `accept_tcp_client` (the check runs at the first poll).
        async move {
            if unsafe { (*this.ptr()).listener.is_none() } {
                rt::throw(rt::exc("System.InvalidOperationException", Str::lit("Not listening. You must call the Start() method before calling this method.")));
            }
            loop {
                if let Some(c) = Self::try_accept(this) {
                    return c;
                }
                super::threading::park_async().await;
            }
        }
    }
    /// `Start()`: binds and listens; `SocketException` when the address is taken. See the module docs for
    /// `TERRARIA_NET_LOOPBACK_ONLY`.
    #[cfg(not(target_family = "wasm"))]
    pub fn start(&mut self) {
        if self.listener.is_some() {
            return;
        }
        let mut addr = self.addr;
        if addr.is_unspecified() && loopback_only() {
            log_once(&LOGGED_BIND, "TERRARIA_NET_LOOPBACK_ONLY: a listener on every interface was asked for; listening on 127.0.0.1 only");
            addr = IpAddr::V4(Ipv4Addr::LOCALHOST);
        }
        match std::net::TcpListener::bind(SocketAddr::new(addr, self.port as u16)) {
            Ok(l) => {
                if let Err(e) = l.set_nonblocking(true) {
                    socket_error(&e.to_string());
                }
                eprintln!("[host] net: listening on {}", l.local_addr().map(|a| a.to_string()).unwrap_or_default());
                self.listener = Some(l);
            }
            Err(e) => socket_error(&format!("{e} {addr}:{}", self.port)),
        }
    }
    /// `Stop()`
    #[cfg(not(target_family = "wasm"))]
    pub fn stop(&mut self) {
        self.listener = None;
    }
    /// `AcceptTcpClient()`: waits for a connection, giving the frame back while there is none. `SocketException`
    /// when the listener is stopped meanwhile; `InvalidOperationException` when it was never started.
    pub fn accept_tcp_client(this: Gc<TcpListener>) -> Gc<TcpClient> {
        // SAFETY: `this` is a live listener (null: NullReferenceException in `ptr`); the reference is re-taken
        // after every wait, during which other code of the game runs.
        if unsafe { (*this.ptr()).listener.is_none() } {
            rt::throw(rt::exc("System.InvalidOperationException", Str::lit("Not listening. You must call the Start() method before calling this method.")));
        }
        loop {
            if let Some(c) = Self::try_accept(this) {
                return c;
            }
            if !super::threading::wait_a_frame() {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------ UdpClient

/// `System.Net.Sockets.UdpClient` (the LAN announcement of `Netplay.BroadcastThread`)
pub struct UdpClient {
    socket: Option<UdpSocket>,
    broadcast: bool,
    closed: bool,
}
// SAFETY: no handles.
unsafe impl Trace for UdpClient {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut Marker) {}
}
impl UdpClient {
    /// `new UdpClient()`: no socket yet (.NET creates one without binding; it is bound by the first `Send`).
    pub fn new_gc() -> Gc<UdpClient> {
        rt::alloc_init(UdpClient { socket: None, broadcast: false, closed: false })
    }
    /// `EnableBroadcast` (set)
    pub fn set_enable_broadcast(&mut self, v: bool) {
        self.broadcast = v;
        if let Some(s) = &self.socket {
            let _ = s.set_broadcast(v);
        }
    }
    /// `Send(dgram, bytes, endPoint)`: the number of bytes sent. Nothing is sent with `TERRARIA_NET_LOOPBACK_ONLY`
    /// unless the destination is a loopback address.
    pub fn send(&mut self, dgram: Arr<u8>, bytes: i32, end_point: Gc<IPEndPoint>) -> i32 {
        if self.closed {
            disposed("System.Net.Sockets.UdpClient");
        }
        check_range(dgram, 0, bytes);
        if end_point.is_null() {
            rt::throw(rt::exc("System.ArgumentNullException", Str::lit("endPoint")));
        }
        // SAFETY: checked above.
        let to = unsafe { (*end_point.ptr()).socket_addr() };
        // a page has no UDP: the LAN announcement of the browser build's server goes nowhere
        if cfg!(target_family = "wasm") {
            log_once(&LOGGED_BROADCAST, &format!("the browser build sends no datagrams: datagrams to {to} (the LAN announcement of a server) are not sent"));
            return bytes;
        }
        if loopback_only() && !to.ip().is_loopback() {
            log_once(&LOGGED_BROADCAST, &format!("TERRARIA_NET_LOOPBACK_ONLY: datagrams to {to} (the LAN announcement of a server) are not sent"));
            return bytes;
        }
        if self.socket.is_none() {
            match UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)) {
                Ok(s) => {
                    let _ = s.set_broadcast(self.broadcast);
                    let _ = s.set_nonblocking(true);
                    self.socket = Some(s);
                }
                Err(e) => socket_error(&e.to_string()),
            }
        }
        match self.socket.as_ref().expect("bound above").send_to(&dgram.as_slice()[..bytes as usize], to) {
            Ok(n) => n as i32,
            Err(e) => socket_error(&e.to_string()),
        }
    }
    /// `Close()`
    pub fn close(&mut self) {
        self.socket = None;
        self.closed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::threading;

    fn bytes(a: Arr<u8>) -> Vec<u8> {
        a.as_slice().to_vec()
    }

    /// A listener on 127.0.0.1, a client, an accepted connection; data both ways through BeginWrite / BeginRead with
    /// callbacks queued as work; a large write that does not fit the socket buffer at once; end of stream; an error
    /// after Close. All sockets are loopback sockets on an OS-chosen port.
    #[test]
    #[cfg_attr(target_family = "wasm", ignore = "sockets: the browser build gets a WebSocket backend (web/WEB_PORT.md stage 4)")]
    fn loopback_round_trip_with_queued_callbacks() {

        // an OS-chosen free port
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port() as i32;
        let listener = TcpListener::new_gc(IPAddress::loopback(), port);
        // SAFETY (whole test): handles from `new_gc` on this thread; no collection runs.
        unsafe {
            (*listener.ptr()).start();
            let client = TcpClient::new_gc();
            (*client.ptr()).set_no_delay(true);
            assert!(!(*client.ptr()).connected());
            (*client.ptr()).connect(IPAddress::loopback(), port);
            assert!((*client.ptr()).connected());
            let served = TcpListener::accept_tcp_client(listener);
            let remote = (*(*served.ptr()).client().ptr()).remote_end_point();
            assert_eq!((*(*remote.ptr()).address().ptr()).to_str().as_str(), "127.0.0.1");
            let cs = TcpClient::get_stream(client);
            let ss = TcpClient::get_stream(served);
            assert!(!(*ss.ptr()).data_available());
            assert!(!(*(*served.ptr()).client().ptr()).poll(0, 0), "nothing to read yet");

            thread_local! { static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) }; }
            let note = |tag: &'static str| -> AsyncCallback { Delegate::new(Box::new(move |ar: Obj| LOG.with(|l| l.borrow_mut().push(format!("{tag} {}", rt::obj_to_string_or_empty(async_state(ar)).as_str()))))) };
            // client -> server
            let out = Arr::from_slice(b"..hello server..");
            let w = (*cs.ptr()).begin_write(out, 2, 12, note("wrote"), rt::str_obj(Str::lit("w1")));
            let inbuf = Arr::<u8>::new(64);
            let r = (*ss.ptr()).begin_read(inbuf, 4, 60, note("read"), rt::str_obj(Str::lit("r1")));
            // callbacks are queued, not run inline
            LOG.with(|l| assert!(l.borrow().is_empty()));
            let mut spins = 0;
            while !async_is_completed(r) || !async_is_completed(w) {
                threading::run_pending_work();
                std::thread::sleep(Duration::from_millis(1));
                spins += 1;
                assert!(spins < 5000, "the loopback bytes never arrived");
            }
            threading::run_pending_work();
            (*cs.ptr()).end_write(w);
            assert_eq!((*ss.ptr()).end_read(r), 12);
            assert_eq!(&bytes(inbuf)[4..16], b"hello server");
            LOG.with(|l| {
                let l = l.borrow();
                assert!(l.contains(&"wrote w1".to_owned()) && l.contains(&"read r1".to_owned()), "{l:?}");
            });

            // server -> client, 4 MB: more than the socket buffers take at once; two writes keep their order
            let big: Vec<u8> = (0..4_000_000u32).map(|i| (i * 7 + i / 251) as u8).collect();
            let big_arr = Arr::from_slice(&big);
            let w1 = (*ss.ptr()).begin_write(big_arr, 0, big.len() as i32, Delegate::NULL, Gc::NULL);
            let w2 = (*ss.ptr()).begin_write(Arr::from_slice(b"TAIL"), 0, 4, Delegate::NULL, Gc::NULL);
            let mut got: Vec<u8> = Vec::new();
            let chunk = Arr::<u8>::new(65536);
            let mut spins = 0;
            while got.len() < big.len() + 4 {
                let r = (*cs.ptr()).begin_read(chunk, 0, 65536, Delegate::NULL, Gc::NULL);
                while !async_is_completed(r) {
                    pump();
                    spins += 1;
                    if spins % 64 == 0 {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    assert!(spins < 2_000_000, "the large write stalled at {} bytes", got.len());
                }
                let n = (*cs.ptr()).end_read(r) as usize;
                assert!(n > 0);
                got.extend_from_slice(&bytes(chunk)[..n]);
            }
            assert!(async_is_completed(w1) && async_is_completed(w2));
            assert!(got[..big.len()] == big[..] && &got[big.len()..] == b"TAIL");
            assert_eq!(pending_operations(), 0);

            // the server closes: the client reads 0 bytes, then the game's "is it still connected" test says no
            (*served.ptr()).close();
            assert!((*served.ptr()).client().is_null());
            let r = (*cs.ptr()).begin_read(chunk, 0, 16, Delegate::NULL, Gc::NULL);
            let mut spins = 0;
            while !async_is_completed(r) {
                pump();
                std::thread::sleep(Duration::from_millis(1));
                spins += 1;
                assert!(spins < 5000);
            }
            assert_eq!((*cs.ptr()).end_read(r), 0);
            let sock = (*client.ptr()).client();
            assert!((*sock.ptr()).poll(0, 0) && (*sock.ptr()).available() == 0, "TcpSocket.IsConnected: Poll(SelectRead) && Available == 0 means closed");
            // a read on a closed stream ends with an IOException at EndRead
            (*client.ptr()).close();
            let e = rt::try_(|| (*cs.ptr()).begin_read(chunk, 0, 16, Delegate::NULL, Gc::NULL)).err().expect("ObjectDisposedException");
            assert!(rt::exc_is(e, "System.ObjectDisposedException"));
            (*listener.ptr()).stop();
            // the port is free again
            std::net::TcpListener::bind(("127.0.0.1", port as u16)).expect("the listener released its port");
        }
    }

    #[test]
    #[cfg_attr(target_family = "wasm", ignore = "sockets: the browser build gets a WebSocket backend (web/WEB_PORT.md stage 4)")]
    fn addresses_and_names() {

        // SAFETY: handles from this module on this thread.
        unsafe {
            let mut a = Gc::NULL;
            assert!(IPAddress::try_parse(Str::lit("192.168.1.20"), &mut a) && (*a.ptr()).address_family() == 2);
            assert_eq!((*a.ptr()).to_str().as_str(), "192.168.1.20");
            let mut b = Gc::NULL;
            assert!(IPAddress::try_parse(Str::lit("::1"), &mut b) && (*b.ptr()).address_family() == 23);
            assert!(!IPAddress::try_parse(Str::lit("not an address"), &mut b) && b.is_null());
            assert!(!IPAddress::try_parse(Str::NULL, &mut b));
            assert!((*IPAddress::loopback().ptr()).equals(IPAddress::new_gc("127.0.0.1".parse().unwrap()).up()));
            assert!(!(*IPAddress::loopback().ptr()).equals(IPAddress::any().up()));
            let ep = IPEndPoint::new_gc(a, 7777);
            assert_eq!(((*ep.ptr()).to_str().as_str().to_owned(), (*ep.ptr()).port()), ("192.168.1.20:7777".to_owned(), 7777));
            assert_eq!((*IPAddress::broadcast().ptr()).to_str().as_str(), "255.255.255.255");
            let entry = dns_get_host_entry(Str::lit("localhost"));
            assert!((*entry.ptr()).address_list().len() >= 1);
            assert!(!dns_get_host_name().as_str().is_empty());
            // a refused connection is a SocketException (nothing listens on the port just released)
            let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port() as i32;
            let c = TcpClient::new_gc();
            let e = rt::try_(|| (*c.ptr()).connect(IPAddress::loopback(), port)).err().expect("SocketException");
            assert!(rt::exc_is(e, "System.Net.Sockets.SocketException"));
            assert!(!(*c.ptr()).connected());
        }
    }

    /// With TERRARIA_NET_LOOPBACK_ONLY a listener on IPAddress.Any is reachable on 127.0.0.1 and bound to nothing
    /// else, and the LAN broadcast sends nothing.
    #[test]
    #[cfg_attr(target_family = "wasm", ignore = "sockets: the browser build gets a WebSocket backend (web/WEB_PORT.md stage 4)")]
    fn loopback_only_mode() {

        std::env::set_var("TERRARIA_NET_LOOPBACK_ONLY", "1");
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port() as i32;
        let l = TcpListener::new_gc(IPAddress::any(), port);
        // SAFETY: handles from this module on this thread.
        unsafe {
            (*l.ptr()).start();
            let bound = (*l.ptr()).listener.as_ref().unwrap().local_addr().unwrap();
            assert!(bound.ip().is_loopback(), "bound to {bound}");
            (*l.ptr()).stop();
            let u = UdpClient::new_gc();
            (*u.ptr()).set_enable_broadcast(true);
            let n = (*u.ptr()).send(Arr::from_slice(&[1u8, 2, 3]), 3, IPEndPoint::new_gc(IPAddress::broadcast(), 8888));
            assert_eq!(n, 3);
            assert!((*u.ptr()).socket.is_none(), "no socket was opened for the suppressed broadcast");
            (*u.ptr()).close();
        }
        std::env::remove_var("TERRARIA_NET_LOOPBACK_ONLY");
    }
}
