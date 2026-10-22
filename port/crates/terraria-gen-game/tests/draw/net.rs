// The net run's CLIENT side (`DRAW_MODE=net`, refdata/net): tools/refdump_draw/NetClient.cs, step for step, with the
// server replaced by what the ORIGINAL server sent in the reference run (client_in.stream: per frame, delivered before
// the next frame) and what the client sends compared, frame by frame, with what the original client sent
// (client_out.stream). The client's network thread is the port's own (Netplay.StartTcpClient: a fiber resumed at the
// end of every Update, twice: terraria-host game.rs end_update; NetClient.cs gates the original's thread to that
// schedule). Included by tests/draw.rs.

/// The net run's state of the port's test (`Run::net`).
struct NetRun {
    /// client_in.stream: what the original server sent after each frame (the frames after the join started)
    server: Vec<(i32, Vec<u8>)>,
    /// client_out.stream: what the original client sent in each frame
    client: Vec<(i32, Vec<u8>)>,
    active: bool,
    frame: usize,
    pending: Vec<u8>,
    exact: usize,
    first: Option<String>,
    sent_types: std::collections::BTreeMap<u8, usize>,
}

impl Run {
    /// NetClient.cs `NetPre`: the server's pending bytes into buffer 256.
    unsafe fn net_pre(&mut self) {
        if let Some(n) = &mut self.net {
            if n.active {
                netc::deliver(&mut n.pending, 256);
            }
        }
    }
    /// NetClient.cs `NetPost`: what the client sent this frame against the original's; the server's reply queued.
    unsafe fn net_post(&mut self) {
        let frame_no = self.frame;
        let Some(n) = &mut self.net else { return };
        if !n.active {
            return;
        }
        let sent = terraria_host::sys::net::virtual_take(0);
        for (_, _, t) in netc::messages(&sent) {
            *n.sent_types.entry(t).or_default() += 1;
        }
        let k = n.frame;
        n.frame += 1;
        match n.client.get(k) {
            Some((_, want)) if *want == sent => n.exact += 1,
            Some((_, want)) => {
                if n.first.is_none() {
                    n.first = Some(format!("linked frame {} (frame {frame_no}): {}", k + 1, netc::first_difference(want, &sent)));
                }
            }
            None => {
                if n.first.is_none() {
                    n.first = Some(format!("linked frame {} (frame {frame_no}): the original client's run had ended", k + 1));
                }
            }
        }
        if let Some((_, s)) = n.server.get(k) {
            n.pending.extend_from_slice(s);
        }
    }
}

unsafe fn net_connection() -> Gc<terraria::remote_server::RemoteServer> {
    (*terraria::netplay::s()).connection
}

/// NetClient.cs `NetScenario`.
unsafe fn net_scenario(r: &mut Run) {
    let m = terraria::main::s();
    let player = (*m).pending_player;
    (*player.ptr()).name = s!("Ref");
    let created = terraria::io::player_file_data::create_and_save(player);
    let pfd = terraria::player::get_file_data(terraria::io::file_data::path(&mut (*created.ptr()).base), false);
    (*m).menu_multiplayer = true;
    terraria::main::select_player(pfd);
    let s = format!("selected: menuMode {}", (*m).menu_mode);
    r.note(s);
    (*m).get_ip = s!("127.0.0.1");
    (*terraria::netplay::s()).listen_port = 7777;
    (*m).auto_pass = false;
    let _ = terraria::netplay::set_remote_ip(s!("127.0.0.1"));
    r.net.as_mut().expect("net run").active = true;
    terraria::main::start_client_gameplay();
    let mut joined = -1i64;
    for _ in 0..6000 {
        r.frames(1);
        if (*m).net_mode == 1 && !(*m).game_menu && (*net_connection().ptr()).state == 10 {
            joined = r.frame as i64;
            break;
        }
    }
    let s = format!(
        "joined at frame {joined} (state {}, netMode {}, gameMenu {})",
        (*net_connection().ptr()).state,
        (*m).net_mode,
        if (*m).game_menu { "True" } else { "False" }
    );
    r.note(s);
    assert!(joined >= 0, "the client did not get into the world");
    r.frames(60);
    r.record("net_joined", 2);
    net_client_script(r);
    r.record("net_end", 2);
    r.net.as_mut().expect("net run").active = false;
}

/// NetClient.cs `NetClientScript`.
unsafe fn net_client_script(r: &mut Run) {
    r.input.keys = vec![KEY_D];
    r.frames(60);
    r.input.keys = vec![KEY_A];
    r.frames(30);
    r.input.keys = vec![KEY_SPACE];
    r.frames(20);
    r.input.keys.clear();
    r.frames(40);
    r.record("net_walked", 2);
    let pl = local_player();
    terraria::chat::chat_helper::send_chat_message_from_client(terraria::chat::chat_message::new__string(s!("hello from the client")));
    r.frames(30);
    r.record("net_chat", 1);
    r.press(KEY_D2);
    let (fx, fy) = feet_tile();
    let (gx, gy) = (fx + 1, fy);
    r.aim((gx * 16 + 8) as f32, (gy * 16 + 8) as f32);
    r.input.left = true;
    r.frames(90);
    r.input.left = false;
    r.frames(20);
    r.record("net_mined", 1);
    {
        let it = *(*pl.ptr()).inventory.at(9);
        terraria::item::set_defaults(it.ptr(), 2, Gc::NULL);
        (*it.ptr()).stack = 50;
        r.press(KEY_D0);
        let (fx, fy) = feet_tile();
        r.aim(((fx + 2) * 16 + 8) as f32, ((fy - 2) * 16 + 8) as f32);
        r.input.left = true;
        r.frames(15);
        r.input.left = false;
        r.frames(20);
        r.record("net_placed", 1);
        terraria::player::drop_selected_item__void(pl.ptr());
        r.frames(30);
        r.record("net_dropped", 1);
    }
    let me = (*terraria::main::s()).my_player;
    let send = |ty: i32, n: i32| terraria::net_message::send_data(ty, -1, -1, Gc::NULL, n, 0.0, 0.0, 0.0, 0, 0, 0);
    (*pl.ptr()).hostile = true;
    send(30, me);
    (*pl.ptr()).team = 1;
    send(45, me);
    r.frames(20);
    terraria::game_content::ui::emote_bubble::make_local_player_emote(0);
    r.frames(30);
    r.record("net_social", 1);
    terraria::player::kill_me(pl.ptr(), terraria::data_structures::player_death_reason::by_custom_reason(s!("the net run")), 1000.0, 1, false);
    r.frames(30);
    r.record("net_dead", 1);
    let mut i = 0;
    while i < 1200 && (*pl.ptr()).dead {
        r.frames(1);
        i += 1;
    }
    r.frames(30);
    r.record("net_respawned", 1);
    r.frames(400);
    r.record("net_server_events", 2);
    terraria::game_content::quick_stacking::quick_stack_to_nearby_chests__player_bool(pl, false);
    r.frames(30);
    r.record("net_quickstack", 1);
    let m = terraria::main::s();
    let plate = (*m).team_name_plate_distance;
    (*m).team_name_plate_distance = 400;
    let pp = (*pl.ptr()).base.position;
    let offs = [terraria_core::xna::Vector2 { x: 120.0, y: -20.0 }, terraria_core::xna::Vector2 { x: 1400.0, y: -60.0 }];
    let mut saved = Vec::new();
    for (k, off) in offs.iter().enumerate() {
        let slot = 200 + k as i32;
        saved.push(*(*m).player.at(slot));
        let o = terraria::player::new();
        (*o.ptr()).name = Str::new(&format!("Mate{k}"));
        (*o.ptr()).base.who_am_i = slot;
        (*o.ptr()).team = 1;
        (*o.ptr()).active = true;
        (*o.ptr()).base.position = pp + *off;
        *(*m).player.at(slot) = o;
    }
    r.frames(3);
    r.record("net_teammates", 1);
    for (k, p) in saved.iter().enumerate() {
        *(*m).player.at(200 + k as i32) = *p;
    }
    (*m).team_name_plate_distance = plate;
    r.frames(3);
}
const KEY_D0: i32 = 48;
const KEY_A: i32 = 65;

/// The net run's result lines and failures (after the scenario).
fn net_report(r: &mut Run) {
    let Some(n) = &r.net else { return };
    let total = n.client.len();
    let s = format!("==== net client: {} of {total} linked frames exact (bytes sent); frames run {}", n.exact, n.frame);
    eprintln!("{s}");
    eprintln!("==== net client: message types sent: {}", n.sent_types.iter().map(|(t, c)| format!("{t}:{c}")).collect::<Vec<_>>().join(" "));
    let first = n.first.clone();
    let frames = n.frame;
    if let Some(f) = first {
        let s = format!("==== net client: first difference: {f}");
        eprintln!("{s}");
        r.failures.push(s);
    } else if frames != total {
        let s = format!("==== net client: {frames} frames linked here, {total} in the original run");
        eprintln!("{s}");
        r.failures.push(s);
    }
}
