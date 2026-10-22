// The probe run, part P: tools/refdump_draw/ProbeP.cs, step for step. Included by tests/draw.rs.

/// ProbeP.cs `ProbeP`.
unsafe fn probe_p(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    let m = terraria::main::s();
    let pl = local_player();
    let p_ = pl.ptr();
    if probe_group(r, "doordirect") {
        use terraria::game_content::door_opening_helper as DH;
        clear_entities();
        place_player(x0 + 8, gy - 1);
        r.frames(2);
        let helper = (*p_).door_helper;
        let handlers = (*helper.ptr())._handler_by_tile_type;
        for x in x0..=x0 + 30 {
            place_tile(x, gy - 6, 30, 0);
        }
        place_tile(x0 + 10, gy - 4, 30, 0);
        place_tile(x0 + 10, gy - 5, 30, 0);
        let (a, b) = (place_tile(x0 + 10, gy - 1, 10, 0), place_tile(x0 + 16, gy - 1, 388, 0));
        r.probe("door", &[&a, &b]);
        let mut n = 0;
        for d in [[10, x0 + 10], [388, x0 + 16]] {
            let h = (*handlers.ptr()).get_item(d[0]);
            place_player(d[1] - 2, gy - 1);
            (*p_).control_right = true;
            let pt = terraria_core::xna::Point { x: d[1], y: gy - 2 };
            let open = DH::get_player_info_for_opening_door(helper.ptr(), pl);
            let (opened, closed);
            if d[0] == 10 {
                use DH::common_door_opening_info_provider as C;
                let hp = h.up::<C::CommonDoorOpeningInfoProvider>().ptr();
                let info = C::provide_info(hp, pt);
                opened = C::try_open_door(hp, info, open);
                r.probe_tiles(&format!("door_open_{n}"), d[1] - 2, gy - 6, 5, 6);
                place_player(d[1] - 6, gy - 1);
                (*p_).control_right = false;
                let close = DH::get_player_info_for_closing_door(helper.ptr(), pl);
                closed = C::try_close_door(hp, info, close);
            } else {
                use DH::tall_gate_opening_info_provider as G;
                let hp = h.up::<G::TallGateOpeningInfoProvider>().ptr();
                let info = G::provide_info(hp, pt);
                opened = G::try_open_door(hp, info, open);
                r.probe_tiles(&format!("door_open_{n}"), d[1] - 2, gy - 6, 5, 6);
                place_player(d[1] - 6, gy - 1);
                (*p_).control_right = false;
                let close = DH::get_player_info_for_closing_door(helper.ptr(), pl);
                closed = G::try_close_door(hp, info, close);
            }
            r.probe(&format!("door_{n}"), &[&opened, &closed]);
            r.probe_tiles(&format!("door_closed_{n}"), d[1] - 2, gy - 6, 5, 6);
            n += 1;
        }
        r.state.push_str(&state_line("doordirect"));
        clear_area(x0, x0 + 30, gy);
        r.frames(2);
    }
    if probe_group(r, "stardew") {
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let pp = (*p_).base.position;
        let p = proj_at(gallery_new_proj(pp.x + 40.0, pp.y - 40.0, 0.0, 0.0, 995));
        let (mut t, mut k) = (0, 0);
        while t < 600 {
            if !(*p.ptr()).active {
                break;
            }
            *(*p.ptr()).ai.at(0) = t as f32;
            (*p.ptr()).time_left = 100;
            r.frames(1);
            if k % 8 == 0 {
                r.record(&format!("stardew_{t}"), 1);
            }
            t += 15;
            k += 1;
        }
        r.state.push_str(&state_line("stardew"));
        clear_entities();
        r.frames(2);
    }
    if probe_group(r, "leashednet") {
        use terraria::game_content::leashed_entities as LE;
        use terraria::game_content::leashed_entity as L;
        clear_entities();
        place_player(x0 + 20, gy - 1);
        let (day, time) = ((*m).day_time, (*m).time);
        set_time(false, 3600.0);
        let mut ax = x0;
        let kites = (*terraria::id::item_id::sets::s()).is_a_kite;
        let kite = (0..kites.len()).find(|&k| *kites.at(k)).unwrap_or(-1);
        for e in [[1992, 724], [2740, 724], [2019, 724], [kite, 723]] {
            give_item(0, e[0]);
            select_item(0);
            terraria::world_gen::place_object(ax, gy - 1, e[1], false, 0, 0, -1, 1);
            let (tx, ty) = top_left_of(e[1], ax, gy - 1);
            if e[1] == 724 {
                terraria::game_content::tile_entities::te_critter_anchor::hook_after_placement(tx, ty, e[1], 0, 1, 0);
            } else {
                terraria::game_content::tile_entities::te_kite_anchor::hook_after_placement(tx, ty, e[1], 0, 1, 0);
            }
            ax += 3;
        }
        give_item(0, 0);
        r.frames(120);
        r.record("leashednet", 1);
        let all = (*L::s()).by_who_am_i;
        let list: Vec<Gc<L::LeashedEntity>> = (0..(*all.ptr()).count()).map(|i| (*all.ptr()).get_item(i)).collect();
        let mut k2 = 0;
        for e in list {
            if e.is_null() || !(*e.ptr()).active {
                continue;
            }
            let ms = rt::io::Stream::new_memory();
            let w = rt::io::BinaryWriter::new_gc(ms);
            L::net_send__v(e.ptr(), w, true);
            (*w.ptr()).flush();
            let bytes = (*ms.ptr()).to_array();
            L::net_receive__v(e.ptr(), rt::io::BinaryReader::new_gc(rt::io::Stream::from_bytes(bytes)), true);
            let o: Obj = e.up();
            if rt::obj_is::<LE::firefly_leashed_critter::FireflyLeashedCritter>(o) {
                LE::firefly_leashed_critter::add_light(e.up::<LE::firefly_leashed_critter::FireflyLeashedCritter>().ptr());
            }
            if rt::obj_is::<LE::jumper_leashed_critter::JumperLeashedCritter>(o) {
                let j = LE::jumper_leashed_critter::try_start_jump(e.up::<LE::jumper_leashed_critter::JumperLeashedCritter>().ptr());
                r.probe("jumper_jump", &[&j]);
            }
            if rt::is_instance(o, LE::leashed_critter::TYPE_ID, &types::PARENT) {
                LE::leashed_critter::recall_create_dust_at_location(e.up::<LE::leashed_critter::LeashedCritter>().ptr());
            }
            let mut h: u32 = 2166136261;
            for i in 0..bytes.len() {
                h = (h ^ *bytes.at(i) as u32).wrapping_mul(16777619);
            }
            r.probe(&format!("leashed_{k2}"), &[&L::type_(e.ptr()), &bytes.len(), &h, &(*e.ptr()).position]);
            k2 += 1;
        }
        clear_area(x0 - 1, ax + 1, gy);
        set_time(day, time);
        r.frames(30);
        r.state.push_str(&state_line("leashednet"));
    }
    if probe_group(r, "effects2") {
        clear_entities();
        clear_fx();
        place_player(sx, sy);
        r.frames(2);
        let pp = (*p_).base.position;
        let sky = (*terraria::graphics::effects::sky_manager::s()).instance;
        terraria::graphics::effects::effect_manager__custom_sky::activate(&mut (*sky.ptr()).base, s!("Sandstorm"), pp, Arr::<Obj>::new(0));
        let ov = (*terraria::graphics::effects::overlays::s()).scene;
        terraria::graphics::effects::effect_manager__overlay::activate(&mut (*ov.ptr()).base, s!("Sandstorm"), pp, Arr::<Obj>::new(0));
        let mods = (*(*m).instance.ptr()).camera_modifiers;
        let cmod = terraria::graphics::camera_modifiers::fixed_move_per_frame_camera_modifier::new(pp, v2(40.0, -30.0), 2.0, 1.5, s!("probe"));
        terraria::graphics::camera_modifiers::camera_modifier_stack::add(mods.ptr(), cmod.up());
        terraria::game_content::events::moonlord_death_drama::add_explosion(v2(pp.x + 80.0, pp.y - 60.0));
        terraria::game_content::events::moonlord_death_drama::add_explosion(v2(pp.x - 80.0, pp.y - 20.0));
        set_armor(1, 3777);
        let dyes = (*p_).dye;
        terraria::item::set_defaults((*dyes.at(1)).ptr(), 3026, Gc::NULL);
        r.frames(20);
        r.probe("dye_body", &[&(*p_).c_body, &(*(*dyes.at(1)).ptr()).dye]);
        r.record("effects2", 1);
        terraria::graphics::effects::effect_manager__custom_sky::deactivate(&mut (*sky.ptr()).base, s!("Sandstorm"), Arr::<Obj>::new(0));
        terraria::graphics::effects::effect_manager__overlay::deactivate(&mut (*ov.ptr()).base, s!("Sandstorm"), Arr::<Obj>::new(0));
        terraria::item::turn_to_air((*dyes.at(1)).ptr());
        set_armor(1, 0);
        let count = (*terraria::id::torch_id::s()).count as i32;
        let mut cs = Vec::new();
        for t in 0..count {
            let (mut a, mut b, mut c) = (0.0f32, 0.0f32, 0.0f32);
            terraria::id::torch_id::torch_color(t, &mut a, &mut b, &mut c);
            cs.push(format!("{},{},{}", a.pv(), b.pv(), c.pv()));
        }
        r.probe("torchcolors", &[&cs.join(";")]);
        {
            use terraria::id::torch_id as TI;
            let (mut a, mut b, mut c) = (0.0f32, 0.0f32, 0.0f32);
            let mut d: TI::demon_torch_light::DemonTorchLight = rt::zero();
            TI::demon_torch_light::get_rgb(&mut d, &mut a, &mut b, &mut c);
            r.probe("torch_DemonTorchLight", &[&a, &b, &c]);
            let mut d: TI::disco_torch_light::DiscoTorchLight = rt::zero();
            TI::disco_torch_light::get_rgb(&mut d, &mut a, &mut b, &mut c);
            r.probe("torch_DiscoTorchLight", &[&a, &b, &c]);
            let mut d: TI::shimmer_torch_light::ShimmerTorchLight = rt::zero();
            TI::shimmer_torch_light::get_rgb(&mut d, &mut a, &mut b, &mut c);
            r.probe("torch_ShimmerTorchLight", &[&a, &b, &c]);
        }
        let bob = proj_at(gallery_new_proj(pp.x + 70.0, pp.y, 0.0, 0.0, 360));
        let mut pos: terraria::game_content::drawing::particle_orchestra_settings::ParticleOrchestraSettings = rt::zero();
        pos.position_in_world = v2(pp.x + 60.0, pp.y + 8.0);
        pos.movement_vector = v2(1.0, 0.0);
        pos.unique_info_piece = 2290;
        terraria::game_content::drawing::particle_orchestrator::spawn_fake_fish(pos, false);
        terraria::game_content::drawing::particle_orchestrator::magnet_fake_fish(bob, 2290);
        r.frames(30);
        probe_fx(r, "effects2");
        r.state.push_str(&state_line("effects2"));
        clear_entities();
        clear_fx();
        r.frames(10);
    }
}
