//! The multiplayer reference run, SERVER side: the translated dedicated server, fed the bytes the ORIGINAL client
//! sent to the original server tick by tick, against what the original server sent back (refdata/net,
//! tools/refdump_net/NetServer.cs: RefDumpNet.exe server in lock step with RefDumpDraw.exe net, the real
//! Terraria.exe client).
//!
//! The test performs NetServer.cs's sequence with translated code: the load and the server state of tests/ticks.rs
//! (the same pins: Main.rand 424242, the load thread's 1357, UnpausedUpdateSeed, TileFrameSeed, the wind counters,
//! `Environment.TickCount` = 1000000, `DateTime.Now` = 2026-06-15 12:00), virtual connections
//! (terraria-host `sys::net::set_virtual`: NetVirt.cs's behaviour under the game's own TcpSocket), the client's
//! connection handed to `Netplay.OnConnectionAccepted` at tick 1, and per tick:
//!   1. the client's bytes of that tick (`server_in.stream`) queued, as many as fit delivered through
//!      `NetMessage.ReceiveBytes(bytes, n, 0)`
//!   2. `Netplay.UpdateConnectedClients()`
//!   3. `HasFullyConnectedClients ? (DetailedFPS.StartNextFrame(); Main.Update(new GameTime())) : Netplay.UpdateInMainThread()`
//!   4. the server script (NetServer.cs `ServerScript`, by ticks since the client is in the world)
//!   5. what the server sent this tick, against `server_out.stream`, byte for byte; the state line against
//!      `server.state` (NetServer.cs `NetStateLine`, the same bytes)
//!
//! Output: one line per difference kind, `==== net server: <n> of <m> ticks exact`, the first differing tick with
//! the message (type, offset) where the bytes part, and message counts by type of both directions.
//! `NET_REF=<dir>`: another reference directory (default refdata/net).

#![allow(unused_mut, unused_variables, unused_assignments, unused_unsafe, unused_parens, unused_braces, non_snake_case, unused_imports)]

use terraria_gen_game::gen::{meta, terraria};
use terraria_gen_game::rt::{self, Arr, Gc, Str};

#[path = "net/common.rs"]
mod netc;
use netc::*;

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_owned()
    } else {
        "<non-string panic payload>".to_owned()
    }
}

fn step(name: &str, f: impl FnOnce()) {
    let t0 = std::time::Instant::now();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt::run(f))) {
        Ok(Ok(())) => eprintln!("ok   {name}   ({:.2} s)", t0.elapsed().as_secs_f64()),
        Ok(Err(e)) => panic!("{name}: C# exception: {e}"),
        Err(p) => panic!("{name}: stopped: {}", panic_text(&p)),
    }
}

fn i4(mut h: u64, v: i32) -> u64 {
    for k in 0..4 {
        h ^= ((v >> (8 * k)) & 0xff) as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

/// NetServer.cs `NetStateLine`, the same bytes.
unsafe fn state_line(tick: i32, inb: &[u8], outb: &[u8]) -> String {
    let m = terraria::main::s();
    let fb = |f: f32| f.to_bits() as i32;
    let (inext, sa) = (*(*m).rand.ptr()).state();
    let mut rh = FNV_BASIS;
    for v in sa {
        rh = i4(rh, v);
    }
    let (mut ph, mut nh, mut prh, mut ih) = (FNV_BASIS, FNV_BASIS, FNV_BASIS, FNV_BASIS);
    let (mut np, mut nn, mut npr, mut ni) = (0, 0, 0, 0);
    for i in 0..255 {
        let p = (*(*m).player.at(i)).ptr();
        if !(*p).active {
            continue;
        }
        np += 1;
        let (pos, vel) = ((*p).base.position, (*p).base.velocity);
        ph = i4(i4(i4(i4(i4(i4(i4(i4(ph, i), fb(pos.x)), fb(pos.y)), fb(vel.x)), fb(vel.y)), (*p).stat_life), (*p).stat_mana), terraria::player::selected_item(p));
        ph = i4(ph, ((*p).control_left as i32) | (((*p).control_right as i32) << 1) | (((*p).control_jump as i32) << 2) | (((*p).control_use_item as i32) << 3));
    }
    for (i, n) in (*m).npc.as_slice().iter().enumerate() {
        let n = n.ptr();
        if !(*n).active {
            continue;
        }
        nn += 1;
        let (pos, vel) = ((*n).base.position, (*n).base.velocity);
        nh = i4(i4(i4(i4(i4(i4(i4(nh, i as i32), (*n).type_), fb(pos.x)), fb(pos.y)), fb(vel.x)), fb(vel.y)), (*n).life);
        for a in (*n).ai.as_slice() {
            nh = i4(nh, fb(*a));
        }
    }
    for (i, n) in (*m).projectile.as_slice().iter().enumerate() {
        let n = n.ptr();
        if !(*n).active {
            continue;
        }
        npr += 1;
        let (pos, vel) = ((*n).base.position, (*n).base.velocity);
        prh = i4(i4(i4(i4(i4(i4(i4(prh, i as i32), (*n).type_), (*n).owner), fb(pos.x)), fb(pos.y)), fb(vel.x)), fb(vel.y));
        for a in (*n).ai.as_slice() {
            prh = i4(prh, fb(*a));
        }
        prh = i4(prh, (*n).time_left);
    }
    for (i, n) in (*m).item.as_slice().iter().enumerate() {
        let n = n.ptr();
        if !terraria::world_item::active(n) {
            continue;
        }
        ni += 1;
        let (pos, vel) = ((*n).base.position, (*n).base.velocity);
        ih = i4(i4(i4(i4(i4(i4(i4(ih, i as i32), terraria::world_item::type_(n)), terraria::world_item::stack(n)), fb(pos.x)), fb(pos.y)), fb(vel.x)), fb(vel.y));
    }
    format!(
        "tick {tick} in {}:{:016x} out {}:{:016x} rand {inext} {rh:016x} players {np}:{ph:016x} npc {nn}:{nh:016x} proj {npr}:{prh:016x} item {ni}:{ih:016x} time {:016x} day {}\n",
        inb.len(),
        fnv(inb),
        outb.len(),
        fnv(outb),
        (*m).time.to_bits(),
        (*m).day_time as i32
    )
}

/// NetServer.cs `ServerScript`.
unsafe fn server_script(t: i32) {
    let m = terraria::main::s();
    let p0 = *(*m).player.at(0);
    let pos = (*p0.ptr()).base.position;
    let send = |ty: i32, n2: f32, n3: f32| terraria::net_message::send_data(ty, -1, -1, Gc::NULL, 0, n2, n3, 0.0, 0, 0, 0);
    let src = || terraria::data_structures::entity_source_debug_command::new().up::<rt::Object>();
    match t {
        600 => {
            let _ = terraria::npc::new_npc(src(), pos.x as i32 + 200, pos.y as i32, 1, 0, 0.0, 0.0, 0.0, 0.0, 255);
        }
        650 => {
            terraria::main::start_rain(false, rt::zero::<rt::Nullable<f32>>(), false);
            send(7, 0.0, 0.0);
        }
        700 => {
            let _ = terraria::item::new_item__i_entity_source_int_int_int_int_int_int_bool_int_new_item_ownership_nullable_vector2_new_item_modifier(
                src(),
                pos.x as i32 - 64,
                pos.y as i32,
                16,
                16,
                73,
                3,
                false,
                0,
                0,
                rt::zero::<rt::Nullable<terraria_core::xna::Vector2>>(),
                rt::zero::<rt::Delegate<dyn Fn(Gc<terraria::world_item::WorldItem>)>>(),
            );
        }
        750 => {
            let text = terraria::localization::network_text::from_literal(Str::lit("hello from the server"));
            terraria::chat::chat_helper::broadcast_chat_message(text, terraria_core::xna::Color::new(255, 240, 20, 255), -1);
        }
        800 => {
            let (tx, ty) = (((pos.x + 8.0) / 16.0) as i32 - 3, ((pos.y + 42.0) / 16.0) as i32 + 1);
            terraria::world_gen::kill_tile(tx, ty, false, false, false);
            send(17, tx as f32, ty as f32);
        }
        850 => {
            (*m).day_time = false;
            (*m).time = 0.0;
            send(18, 0.0, 0.0);
        }
        _ => {}
    }
}

#[test]
fn server_answers_the_original_client_as_the_original_server() {
    let run = || unsafe {
        std::panic::set_hook(Box::new(|_| {}));
        let root = terraria_host::sys::io::project_dir();
        let ref_dir = std::env::var_os("NET_REF").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("refdata/net"));
        terraria_gen_game::host::launch::install_resources(&root);
        terraria_host::game::init(terraria_host::game::WindowState { width: 800, height: 600, x: 0, y: 0, active: true, handle: 0, display_width: 1920, display_height: 1080, scale: 1.0 });
        meta::register_all();
        meta::register_json();
        // NetServer.cs pins: the tick counter and the clock
        terraria_host::sys::time::set_fixed_tick_count(Some(1_000_000));
        terraria_host::sys::time::set_fixed_now(Some(terraria_host::sys::time::DateTime::from_ymd(2026, 6, 15).ticks() + 12 * 36_000_000_000));
        terraria_host::sys::net::set_virtual(true);
        // NetServer.cs `NetFakeNewGuid`: the main thread's GUIDs are Guid.Empty
        terraria_host::sys::misc::set_new_guid_empty(true);

        let save = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("net-server-save");
        let _ = std::fs::remove_dir_all(&save);
        std::fs::create_dir_all(save.join("Worlds")).expect("save root");
        let world = save.join("Worlds").join("ref_small.wld");
        std::fs::copy(root.join("work/refsave/Worlds/ref_small.wld"), &world).expect("copy of the reference world");
        std::env::set_var("TERRARIA_SAVE_DIR", &save);

        let mut main = Gc::NULL;
        step("Program.SavePath = <scratch>", || (*terraria::program::s()).save_path = Str::new(&save.to_string_lossy()));
        step("Main.dedServ = true; Main.showSplash = false", || {
            (*terraria::main::s()).ded_serv = true;
            (*terraria::main::s()).show_splash = false;
        });
        step("LanguageManager.Instance.SetLanguage(GameCulture.DefaultCulture)", || {
            let lm = (*terraria::localization::language_manager::s()).instance;
            terraria::localization::language_manager::set_language__game_culture(lm.ptr(), terraria::localization::game_culture::default_culture());
        });
        step("main = new Main()", || main = terraria::main::new());
        step("Lang.InitializeLegacyLocalization()", || terraria::lang::initialize_legacy_localization());
        step("-world <existing>: WorldFile.GetAllMetadata(path, false).SetAsActive()", || {
            let wfd = terraria::io::world_file::get_all_metadata(Str::new(&world.to_string_lossy()), false);
            assert!(!wfd.is_null(), "GetAllMetadata returned null");
            terraria::io::world_file_data::set_as_active(wfd.ptr());
        });
        step("Main.rand = new UnifiedRandom(0)", || (*terraria::main::s()).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(0)));
        step("main.Initialize()", || terraria::main::initialize(main.ptr()));
        step("WorldGen.serverLoadWorldCallBack() with Main.rand = new UnifiedRandom(1357)", || {
            (*terraria::main::s()).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(1357));
            terraria::world_gen::server_load_world_call_back();
            assert!(!(*terraria::world_gen::s()).load_failed, "WorldGen.loadFailed");
        });
        step("server state: myPlayer = 255; netMode = 2; Netplay.Disconnect = false; Clients[i] = new RemoteClient", || {
            (*terraria::main::s()).my_player = 255;
            (*terraria::main::s()).net_mode = 2;
            (*terraria::netplay::s()).disconnect = false;
            let clients = (*terraria::netplay::s()).clients;
            for i in 0..256 {
                let c = terraria::remote_client::new();
                terraria::remote_client::reset(c.ptr());
                (*c.ptr()).id = i;
                (*c.ptr()).read_buffer = rt::Arr::<u8>::new(1024);
                *clients.at(i) = c;
            }
        });
        (*terraria::main::s()).game_menu = false;
        (*terraria::main::s()).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(424242));
        (*terraria::main::s()).unpaused_update_seed__backing = 123456789;
        (*terraria::main::s()).tile_frame_seed = 987654321;
        (*terraria::main::s()).wind_counter = 1500;
        (*terraria::main::s()).extreme_wind_counter = 25;

        let inputs = read_stream(&ref_dir.join("server_in.stream"));
        let outputs = read_stream(&ref_dir.join("server_out.stream"));
        let states: Vec<String> = std::fs::read_to_string(ref_dir.join("server.state")).expect("server.state").lines().map(str::to_owned).collect();
        assert_eq!(inputs.len(), outputs.len(), "server_in / server_out tick counts");
        let mut pending: Vec<u8> = Vec::new();
        let mut connected_at = -1;
        let mut wind_resets = 0;
        let (mut exact_bytes, mut exact_state) = (0usize, 0usize);
        let mut first: Option<String> = None;
        let mut first_state: Option<String> = None;
        let mut sent_types = std::collections::BTreeMap::<u8, usize>::new();
        let t0 = std::time::Instant::now();
        for (k, (tick, from_client)) in inputs.iter().enumerate() {
            let tick = *tick;
            assert_eq!(tick as usize, k + 1);
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                rt::run(|| {
                    if tick == 1 {
                        // TcpSocket.ListenLoop's work for a new client: a connected TcpSocket to Netplay.OnConnectionAccepted
                        let sock = terraria::net::sockets::tcp_socket::new__void();
                        let addr = terraria::net::tcp_address::new(terraria_host::sys::net::IPAddress::loopback(), 7777);
                        terraria::net::sockets::tcp_socket::connect(sock.ptr(), addr.up());
                        terraria::netplay::on_connection_accepted(sock.up());
                    }
                    pending.extend_from_slice(from_client);
                    deliver(&mut pending, 0);
                    terraria::netplay::update_connected_clients();
                    if (*terraria::netplay::s()).has_fully_connected_clients {
                        terraria::testing::detailed_fps::start_next_frame();
                        let wind_before = (*terraria::main::s()).wind_counter;
                        let gt = terraria_host::game::GameTime::new_gc(rt::zero(), rt::zero());
                        terraria::main::update(main.ptr(), gt);
                        if (*terraria::main::s()).wind_counter > wind_before {
                            (*terraria::main::s()).wind_counter = 900 + (wind_resets * 773) % 1801;
                            wind_resets += 1;
                        }
                    } else {
                        terraria::netplay::update_in_main_thread();
                    }
                    let c0 = *(*terraria::netplay::s()).clients.at(0);
                    if connected_at < 0 && (*c0.ptr()).state == 10 {
                        connected_at = tick;
                        eprintln!("client in the world at tick {tick}");
                    }
                    if connected_at >= 0 {
                        server_script(tick - connected_at);
                    }
                })
            }));
            match r {
                Ok(Ok(())) => {}
                Ok(Err(e)) => panic!("tick {tick}: C# exception: {e}"),
                Err(p) => panic!("tick {tick}: stopped: {}", panic_text(&p)),
            }
            (*main.ptr())._is_drawing_or_updating = false;
            terraria_host::sys::threading::run_pending_work();
            let sent = terraria_host::sys::net::virtual_take(0);
            for (_, _, t) in messages(&sent) {
                *sent_types.entry(t).or_default() += 1;
            }
            let want = &outputs[k].1;
            if &sent == want {
                exact_bytes += 1;
            } else if first.is_none() {
                first = Some(format!("tick {tick}: {}", first_difference(want, &sent)));
            }
            let line = state_line(tick, from_client, &sent);
            if line.trim_end() == states[k] {
                exact_state += 1;
            } else if first_state.is_none() {
                first_state = Some(format!("tick {tick}:\n  original: {}\n  port:     {}", states[k], line.trim_end()));
            }
            if first.is_some() && std::env::var_os("NET_KEEP_GOING").is_none() && k > 0 && first_state.is_some() {
                break;
            }
        }
        let n = inputs.len();
        eprintln!("==== net server: {exact_bytes} of {n} ticks exact (bytes sent), {exact_state} of {n} state lines equal; client in the world at tick {connected_at}; {:.1} s", t0.elapsed().as_secs_f64());
        eprintln!("==== net server: message types sent: {}", sent_types.iter().map(|(t, c)| format!("{t}:{c}")).collect::<Vec<_>>().join(" "));
        if let Some(f) = &first {
            eprintln!("==== net server: first difference (bytes): {f}");
        }
        if let Some(f) = &first_state {
            eprintln!("==== net server: first difference (state): {f}");
        }
        assert!(first.is_none() && first_state.is_none(), "the server's answers differ from the original's");
    };
    terraria_host::run_with_stack(512 << 20, "the net server test", run);
}
