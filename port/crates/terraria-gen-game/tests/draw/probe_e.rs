// The probe run, part E: tools/refdump_draw/ProbeE.cs, step for step. Included by tests/draw.rs.

unsafe fn active_npcs() -> i32 {
    (*terraria::main::s()).npc.as_slice().iter().filter(|n| (*n.ptr()).active).count() as i32
}
unsafe fn active_items() -> i32 {
    (*terraria::main::s()).item.as_slice().iter().filter(|wi| terraria::world_item::active(wi.ptr())).count() as i32
}

/// ProbeE.cs `ProbeE`.
unsafe fn probe_e(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    let m = terraria::main::s();
    let pl = local_player();
    // ---- the lunatic cultists' ritual
    if probe_group(r, "cultist") {
        use terraria::game_content::events::cultist_ritual as cr;
        let (dx, dy) = ((*m).dungeon_x, (*m).dungeon_y);
        let a = cr::try_spawning(dx, dy, false);
        r.probe("cultist_try", &[&a]);
        let b = cr::try_spawning(dx, dy, true);
        r.probe("cultist_force", &[&b]);
        r.probe("cultist_npcs", &[&active_npcs()]);
        clear_entities();
        r.frames(2);
    }
    // ---- the Old One's Army
    if probe_group(r, "dd2") {
        use terraria::game_content::events::dd2_event as dd2;
        let stx = x0 + 30;
        let ok = terraria::world_gen::place_object(stx, gy - 1, 466, false, 0, 0, -1, 1);
        r.note(format!("dd2 stand: {} {}", cs_bool(ok), cover_diag(stx - 2, gy - 4, 5, 4)));
        place_player(stx - 6, gy - 1);
        let f = dd2::would_fail_spawning_here(stx, gy - 1);
        r.probe("dd2_fail_here", &[&f]);
        dd2::summon_crystal_direct(stx, gy - 1, (*m).my_player);
        r.frames(5);
        let (a, b) = (dd2::is_stand_active(stx, gy - 1), dd2::is_stand_active(stx + 20, gy - 1));
        r.probe("dd2_stand", &[&a, &b]);
        dd2::request_to_skip_wait_time(stx, gy - 1);
        let c = dd2::should_drop_crystals();
        r.probe("dd2_crystals", &[&c]);
        dd2::drop_medals(3);
        let ns = terraria::npc::s();
        for wave in [1, 5, 7] {
            (*ns).wave_number = wave;
            (*ns).wave_kills = 300.0;
            for id in [552, 555, 561, 564, 565, 576, 577] {
                let (p1, p2) = (dd2::difficulty_1_get_monster_points_worth(id), dd2::difficulty_2_get_monster_points_worth(id));
                r.probe(&format!("dd2_points_{wave}_{id}"), &[&p1, &p2]);
            }
        }
        (*ns).wave_number = 0;
        (*ns).wave_kills = 0.0;
        let pp = (*pl.ptr()).base.position;
        let spots = (*dd2::s())._dead_goblin_spots;
        for k in 0..4 {
            (*spots.ptr()).add(v2(pp.x + 60.0 * k as f32 - 90.0, pp.y));
        }
        let mage = new_npc_at(pp.x as i32 + 100, pp.y as i32, 564);
        dd2::raise_goblins(npc_at(mage), v2(pp.x, pp.y));
        r.frames(10);
        r.record("dd2_raise", 1);
        dd2::start_victory_scene();
        r.frames(30);
        r.record("dd2_victory", 1);
        dd2::stop_invasion(false);
        dd2::wipe_entities();
        (*spots.ptr()).clear();
        clear_entities();
        clear_items();
        kill_tile(stx, gy - 1);
        r.frames(30);
    }
    // ---- Moon Lord's death drama
    if probe_group(r, "mldrama") {
        let pp = (*pl.ptr()).base.position;
        terraria::game_content::events::moonlord_death_drama::throw_pieces(v2(pp.x, pp.y - 200.0), 37);
        r.frames(20);
        r.record("mldrama_a", 1);
        r.frames(120);
        r.record("mldrama_b", 1);
        r.frames(60);
    }
    // ---- the fairies of a mystic log
    if probe_group(r, "mysticlog") {
        use terraria::game_content::events::mystic_log_fairies_event as ml;
        set_time(false, 3600.0);
        let lx = x0 + 10;
        let ok = terraria::world_gen::place_object(lx, gy - 1, 488, false, 0, 0, -1, 1);
        r.note(format!("fallen log: {} {}", cs_bool(ok), cover_diag(lx - 2, gy - 3, 6, 3)));
        let ev = (*terraria::world_gen::s()).mystic_logs_event;
        ml::start_night(ev.ptr());
        for _ in 0..40 {
            ml::try_spawning_fairies(ev.ptr());
        }
        r.probe("mysticlog_npcs", &[&active_npcs()]);
        r.frames(10);
        r.record("mysticlog", 1);
        clear_entities();
        clear_area(x0, x0 + 20, gy);
        set_time(true, 27000.0);
        r.frames(5);
    }
    // ---- golf
    if probe_group(r, "golf") {
        use terraria::game_content::golf::golf_helper as gh;
        place_player(x0 + 10, gy - 1);
        r.frames(2);
        let pp = (*pl.ptr()).base.position;
        for club in [4039, 4092, 4093, 4094, 4587, 4588, 4589, 4590, 4591, 4592, 4593, 4594, 4595, 4596, 4597, 4598, 4599, 4600] {
            let cp = gh::get_club_properties(club as i16);
            r.probe(&format!("golf_club_{club}"), &[&cp.minimum_strength, &cp.maximum_strength, &cp.rough_land_resistance]);
            for v in [v2(3.0, -4.0), v2(0.1, -0.2), v2(-20.0, -9.0)] {
                let ss = gh::calculate_shot_strength__vector2_club_properties(v, cp);
                r.probe(&format!("golf_shot_{club}_{}", v.pv()), &[&ss.absolute_strength, &ss.relative_strength, &ss.rough_land_resistance]);
            }
        }
        let bi = gallery_new_proj(pp.x + 30.0, pp.y + 20.0, 0.0, 0.0, 721);
        let ball = proj_at(bi);
        r.frames(20);
        give_item(0, 4039);
        select_item(0);
        let ok = gh::is_golf_shot_valid(ball.up(), pl);
        r.probe("golf_valid", &[&ok]);
        let gs = (*m).local_golf_state;
        terraria::game_content::golf::golf_state::record_swing(gs.ptr(), ball);
        use terraria::game_content::golf::fancy_golf_prediction_line as fl;
        let line = fl::new(20);
        let _root = rt::Rooted::new(line);
        for charge in [0.2f32, 0.7] {
            fl::update(line.ptr(), ball.up(), v2(4.0, -6.0), 1.0);
            r.direct(&format!("golf_line_{}", charge.pv()), |_| in_batch(|| fl::draw(line.ptr(), (*terraria::main::s()).camera, (*terraria::main::s()).sprite_batch, charge)));
        }
        gh::hit_golf_ball(ball.up(), v2(5.0, -7.0), 1.0);
        r.frames(40);
        terraria::game_content::golf::golf_state::record_swing(gs.ptr(), ball);
        let (score, adj) = (terraria::game_content::golf::golf_state::get_golf_ball_score(gs.ptr(), ball), terraria::game_content::golf::golf_state::score_adjustment(gs.ptr()));
        r.probe("golf_score", &[&score, &adj]);
        clear_fx();
        gh::contact_listener::emit_golfball_explosion(v2(pp.x, pp.y - 40.0));
        probe_fx(r, "golf_explosion");
        r.frames(2);
        r.record("golf", 1);
        give_item(0, 0);
        clear_entities();
        r.frames(3);
    }
    // ---- drop rules
    if probe_group(r, "droprules") {
        use terraria::game_content::item_drop_rules as idr;
        let frost = idr::conditions::frost_moon_drop_gating_chance::new();
        let pumpkin = idr::conditions::pumpkin_moon_drop_gating_chance::new();
        let npc = new_npc_at(sx * 16, sy * 16 - 100, 3);
        let mut info: idr::drop_attempt_info::DropAttemptInfo = rt::zero();
        info.npc = npc_at(npc);
        info.player = pl;
        info.rng = (*m).rand;
        let ns = terraria::npc::s();
        for moon in ["snowMoon", "pumpkinMoon"] {
            if moon == "snowMoon" {
                (*m).snow_moon = true;
            } else {
                (*m).pumpkin_moon = true;
            }
            for wave in [1, 6, 12, 15, 20] {
                (*ns).wave_number = wave;
                let mut a = Vec::new();
                for _ in 0..8 {
                    a.push(if moon == "snowMoon" { idr::conditions::frost_moon_drop_gating_chance::can_drop(frost.ptr(), info) } else { idr::conditions::pumpkin_moon_drop_gating_chance::can_drop(pumpkin.ptr(), info) });
                }
                let refs: Vec<&dyn Pv> = a.iter().map(|v| v as &dyn Pv).collect();
                r.probe(&format!("droprule_{moon}_{wave}"), &refs);
            }
            (*m).snow_moon = false;
            (*m).pumpkin_moon = false;
        }
        (*ns).wave_number = 0;
        let rule = idr::mech_boss_spawners_drop_rule::new();
        let drops = List::<idr::drop_rate_info::DropRateInfo>::new_gc();
        let feed = idr::drop_rate_info_chain_feed::new(0.5);
        idr::mech_boss_spawners_drop_rule::report_droprates(rule.ptr(), drops, feed);
        for k in 0..(*drops.ptr()).count() {
            let d = (*drops.ptr()).get_item(k);
            r.probe("mechdrop", &[&d.item_id, &d.stack_min, &d.stack_max, &d.drop_rate]);
        }
        idr::common_code::drop_item_for_each_interacting_player_on_the_player(npc_at(npc), 29, (*m).rand, 1, 1, 2, false);
        r.probe("droprule_items", &[&active_items()]);
        clear_items();
        clear_entities();
        r.frames(2);
    }
    // ---- every whip on a target dummy with a finch minion
    if probe_group(r, "whips") {
        clear_entities();
        let wx = x0 + 20;
        place_player(wx - 5, gy - 1);
        r.frames(2);
        let staff = cover_new_item(4281);
        let (mshoot, mbuff) = ((*staff.ptr()).shoot, (*staff.ptr()).buff_type);
        r.note(format!("minion {mshoot} buff {mbuff}"));
        terraria::world_gen::place_object(wx, gy - 1, 378, false, 0, 0, -1, 1);
        let (dlx, dly) = top_left_of(378, wx + 1, gy - 1);
        let tid = terraria::game_content::tile_entities::te_training_dummy::hook_after_placement(dlx, dly, 378, 0, 1, 0);
        r.frames(30);
        let dummy = (*cover_entity(tid).up::<terraria::game_content::tile_entities::te_training_dummy::TETrainingDummy>().ptr()).npc;
        let dn = npc_at(dummy);
        // the slot gets a new NPC object when the dummy respawns (NPC.NewNPCInstanceInSlot); the reference keeps reading
        // the old one, so this one is rooted (else its memory could be reused by a later object)
        let _root_dn = rt::Rooted::new(dn);
        let ppos = (*pl.ptr()).base.position;
        r.note(format!(
            "whip dummy at {dlx},{dly} npc {dummy} type {} active {} life {} pos {} player {}",
            (*dn.ptr()).type_,
            cs_bool((*dn.ptr()).active),
            (*dn.ptr()).life,
            (*dn.ptr()).base.position.pv(),
            ppos.pv()
        ));
        let whip = (*terraria::id::item_id::sets::s()).summoner_weapon_that_scales_with_attack_speed;
        let count = (*terraria::id::item_id::s()).count as i32;
        let whips: Vec<i32> = (1..count).filter(|&t| *whip.at(t)).collect();
        r.note(format!("whips: {}", whips.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(",")));
        let dpos = (*dn.ptr()).base.position;
        for w in whips {
            terraria::player::add_buff(pl.ptr(), mbuff, 3600, false);
            let pp = (*pl.ptr()).base.position;
            gallery_new_proj(pp.x, pp.y - 30.0, 0.0, 0.0, mshoot);
            give_item(0, w);
            select_item(0);
            r.aim(dpos.x + 10.0, dpos.y + 20.0);
            r.input.left = true;
            r.frames(40);
            r.input.left = false;
            r.frames(60);
            r.probe(&format!("whip_{w}"), &[&(*dn.ptr()).life, &(*pl.ptr()).stat_life]);
            r.record(&format!("whip_{w}"), 1);
            for p in (*m).projectile.as_slice() {
                (*p.ptr()).active = false;
            }
            r.frames(2);
        }
        give_item(0, 0);
        r.input.x = 640;
        r.input.y = 360;
        kill_tile(dlx, dly);
        clear_entities();
        r.frames(5);
    }
}
