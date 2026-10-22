// The probe run, part I: tools/refdump_draw/ProbeI.cs, step for step. Included by tests/draw.rs.

/// ProbeI.cs `NpcNear`.
unsafe fn npc_near(ty: i32, dx: f32, dy: f32) -> Gc<terraria::npc::NPC> {
    let pp = (*local_player().ptr()).base.position;
    let n = npc_at(new_npc_at((pp.x + dx) as i32, (pp.y + dy) as i32, ty));
    (*n.ptr()).target = (*terraria::main::s()).my_player;
    n
}
unsafe fn npc_center(n: Gc<terraria::npc::NPC>) -> V2 {
    terraria::entity::center(n.ptr() as *mut terraria::entity::Entity)
}

/// ProbeI.cs `ProbeI`.
unsafe fn probe_i(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    use terraria::npc as N;
    let m = terraria::main::s();
    let pl = local_player();
    let me = (*m).my_player;
    // ---- boss and enemy attacks
    if probe_group(r, "npcattacks") {
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let ks = npc_near(50, 200.0, -100.0);
        let (mut tx, mut ty) = (0.0f32, 0.0f32);
        let ok = N::ai_015_king_slime_find_teleport_spot(ks.ptr(), false, &mut tx, &mut ty);
        r.probe("ks_teleport", &[&ok, &tx, &ty]);
        let (mut tx, mut ty) = (0.0f32, 0.0f32);
        let ok = N::ai_015_king_slime_find_teleport_spot(ks.ptr(), true, &mut tx, &mut ty);
        r.probe("ks_teleport_anticheese", &[&ok, &tx, &ty]);
        N::add_king_slime_teleport_cache_tiles(ks.ptr(), sx - 10, sx + 10, sy - 7, sy + 3);
        r.probe("ks_cache", &[&(*N::s()).king_slime_point_cache_size]);
        let eol = npc_near(636, -200.0, -200.0);
        clear_fx();
        for e in 0..4 {
            N::ai_120_hallow_boss_do_magic_effect(eol.ptr(), npc_center(eol), e, 0.5);
        }
        let (a, b) = (N::boss_can_shoot_extra_at(eol.ptr(), me, 0, 3, 2000.0, false), N::boss_can_shoot_extra_at(eol.ptr(), me, 1, 3, 2000.0, true));
        r.probe("eol_canshoot", &[&a, &b]);
        probe_fx(r, "eol_magic");
        let qs = npc_near(657, 300.0, -300.0);
        for _ in 0..5 {
            N::ai_121_queen_slime_fly_movement(qs.ptr());
        }
        r.probe("qs_fly", &[&(*qs.ptr()).base.velocity, &(*qs.ptr()).no_gravity]);
        let dc = npc_near(668, 150.0, -50.0);
        let mut td = N::get_target_data(dc.ptr(), true);
        *(*dc.ptr()).ai.at(0) = 56.0;
        N::ai_123_deerclops_make_spikes_both_sides(dc.ptr(), 0, td);
        let mut src_pt = terraria_core::xna::Point { x: sx, y: sy };
        N::ai_123_deerclops_shoot_rubble_up(dc.ptr(), &mut td, &mut src_pt, 3, 2, 10.0, 1);
        N::spawn_passive_shadow_hands(dc.ptr(), 0.4, 20);
        let mimic = npc_near(473, -150.0, -30.0);
        N::ai_87_big_mimic_shoot_item(mimic.ptr(), 3005);
        N::big_mimic_spawn_smoke(mimic.ptr());
        let elder = npc_near(1, 60.0, -30.0);
        N::ai_124_elder_slime_chest(elder.ptr());
        let naut = npc_near(618, 250.0, 0.0);
        N::blood_nautilus_call_for_help(naut.ptr());
        let worm = npc_near(402, -300.0, -100.0);
        N::spawn_stardust_mark_stardust_worm(worm.ptr());
        let hornet = npc_near(42, 90.0, -60.0);
        N::stinger_explosion(hornet.ptr());
        N::violently_spawn_nerdy_slime(hornet.ptr());
        r.state.push_str(&state_line("npcattacks"));
        probe_fx(r, "npcattacks");
        r.frames(3);
        r.record("npcattacks", 1);
        clear_entities();
        clear_fx();
        r.frames(2);
    }
    // ---- town NPC helpers
    if probe_group(r, "npctown") {
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let guide = npc_near(22, 40.0, 0.0);
        let spot = N::ai_007_town_entities_shimmer_scan_for_best_spot_to_land_on(guide.ptr());
        r.probe("shimmer_spot", &[&spot]);
        let mut dest = v2(0.0, 0.0);
        let ok = N::ai_attempt_to_find_teleport_spot_near_books(guide.ptr(), terraria_core::xna::Point { x: sx, y: sy }, 30, &mut dest);
        r.probe("books_teleport", &[&ok, &dest]);
        let (mut dest, mut oob) = (v2(0.0, 0.0), false);
        let ok = N::ai_attempt_to_find_teleport_spot_near_books_search_wall(guide.ptr(), sx, sy - 5, true, &mut dest, &mut oob);
        r.probe("books_wall", &[&ok, &dest, &oob]);
        let mut bp = terraria_core::xna::Point { x: 0, y: 0 };
        let ok = N::ai_find_nearby_book(terraria_core::xna::Point { x: sx - 20, y: sy - 15 }, 40, 30, &mut bp, true, false);
        r.probe("books_near", &[&ok, &bp]);
        for s in [[20, 5, 1, 30, 30], [20, 15, -1, 40, 20]] {
            let tup = N::get_swing_stats(guide.ptr(), s[0], s[1], s[2], s[3], s[4]);
            let mut rc = rect(10, 20, 30, 40);
            N::tweak_swing_stats(guide.ptr(), s[0], s[1], s[2], &mut rc);
            r.probe(&format!("swing_{}", s[1]), &[&(*tup.ptr()).item1, &(*tup.ptr()).item2, &rc]);
        }
        let cat = npc_near(637, -40.0, 0.0);
        let pi = N::get_petting_info(cat.ptr(), pl);
        r.probe("petting", &[&pi.is_petting, &pi.npc, &pi.type_, &pi.offset_from_pet, &pi.is_pet_small]);
        terraria::player::pet_animal(pl.ptr(), pi);
        let rr = N::reroll_variation(cat.ptr());
        r.probe("reroll", &[&rr, &(*cat.ptr()).town_npc_variation_index]);
        let nt = terraria::localization::network_text::from_literal(s!("Probe was slain"));
        N::drop_tombstone_town_npc(guide.ptr(), nt);
        npc_near(20, 80.0, 0.0);
        N::have_dryad_do_stardew_animation();
        r.probe("merchant_allowed", &[&N::spawn_allowed_merchant()]);
        r.probe("dryadsbane", &[&N::get_dryads_bane_damage_per_second(guide.ptr())]);
        let (f1, f2) = (N::get_fairy_circle_offset(guide.ptr(), 1.5, 0.3, 20.0), N::get_fairy_circle_offset(guide.ptr(), 7.25, 2.0, 5.0));
        r.probe("fairycircle", &[&f1, &f2]);
        let frog = npc_near(677, 30.0, 0.0);
        r.probe("mysticfrog", &[&N::try_teleporting_caught_mystic_frog(frog.ptr())]);
        r.state.push_str(&state_line("npctown"));
        r.frames(3);
        r.record("npctown", 1);
        clear_entities();
        r.frames(2);
    }
    // ---- spawning and death helpers
    if probe_group(r, "npcspawns") {
        clear_entities();
        place_player(480, 1009);
        r.frames(2);
        let wof = npc_near(113, 0.0, -40.0);
        let wp = (*wof.ptr()).base.position;
        r.probe_tiles("wofbox_before", (wp.x / 16.0) as i32 - 30, (wp.y / 16.0) as i32 - 30, 60, 60);
        N::create_brick_box_for_wall_of_flesh(wof.ptr());
        r.probe_tiles("wofbox_after", (wp.x / 16.0) as i32 - 30, (wp.y / 16.0) as i32 - 30, 60, 60);
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let pp = (*pl.ptr()).base.position;
        let core = npc_near(398, 0.0, -300.0);
        N::moon_lord_spawn_true_eye_of_cthulhu(core.ptr());
        let (a, b) = (N::mech_spawn(pp.x, pp.y, 3), N::mech_spawn(pp.x + 500.0, pp.y, 1));
        r.probe("mechspawn", &[&a, &b]);
        N::spawn_skeletron(me, false);
        N::red_hat_skeletron(me);
        clear_fx();
        N::lady_bug_killed(v2(pp.x + 30.0, pp.y), false);
        N::lady_bug_killed(v2(pp.x - 30.0, pp.y), true);
        N::fairy_effects(v2(pp.x, pp.y - 40.0), 583);
        probe_fx(r, "npcspawns_fx");
        let (f, l, c) = (N::any_helpful_fairies(), N::any_life_crystal_slimes(), N::anyone_near_cultists());
        r.probe("anyfairies", &[&f, &l, &c]);
        let head = new_npc_at(pp.x as i32 + 200, pp.y as i32 - 200, 13);
        let (mut idx, mut pos) = (0i32, v2(0.0, 0.0));
        let ok = N::get_npc_location(head, true, true, &mut idx, &mut pos);
        r.probe("npclocation", &[&ok, &idx, &pos]);
        r.probe("devourer", &[&N::spawner::can_spawn_devourer(sx)]);
        let sp = N::spawner::new();
        N::spawner::spawn_frog(sp.ptr(), sx + 10, sy);
        let hn = N::spawner::spawn_hornet(sp.ptr(), sx - 10, sy);
        let lc = N::spawner::spawn_lava_bait_critters(sp.ptr(), sx + 20, sy);
        let (ht, lt) = (if hn.is_null() { -1 } else { (*hn.ptr()).type_ }, if lc.is_null() { -1 } else { (*lc.ptr()).type_ });
        r.probe("spawner", &[&ht, &lt]);
        N::spawner::spawn_faelings(pl);
        r.state.push_str(&state_line("npcspawns"));
        r.frames(3);
        r.record("npcspawns", 1);
        clear_entities();
        clear_fx();
        r.frames(2);
    }
    // ---- small NPC helpers
    if probe_group(r, "npcmisc") {
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let z = npc_near(3, 120.0, 0.0);
        N::add_buff(z.ptr(), 20, 300, false);
        N::add_buff(z.ptr(), 24, 300, false);
        N::del_buff(z.ptr(), 0, true);
        r.probe("delbuff", &[&*(*z.ptr()).buff_type.at(0), &*(*z.ptr()).buff_time.at(0)]);
        let hm = (*m).hard_mode;
        (*m).hard_mode = true;
        N::scale_stats_for_expert_hardmode(z.ptr());
        r.probe("expert_hm", &[&(*z.ptr()).life_max, &(*z.ptr()).damage, &(*z.ptr()).defense]);
        (*m).hard_mode = hm;
        r.probe("keybrand", &[&N::key_brand_strike(z.ptr(), me, 100, npc_center(z))]);
        let pp = (*pl.ptr()).base.position;
        let pr = gallery_new_proj(pp.x, pp.y, 6.0, 0.0, 1);
        N::reflect_projectile(z.ptr(), proj_at(pr));
        let p = &*proj_at(pr).ptr();
        r.probe("reflect", &[&p.base.velocity, &p.hostile, &p.friendly]);
        for c in [[1.0f32, 0.5, 0.2, 1.0], [0.1, 0.9, 0.3, 0.5]] {
            let col = N::buff_color(terraria_core::xna::Color::new(200, 150, 100, 255), c[0], c[1], c[2], c[3]);
            r.probe(&format!("buffcolor_{}", c[0].pv()), &[&col]);
        }
        let mut tally: N::dot_tally::DOTTally = std::mem::zeroed();
        tally.npc = z;
        let zp = (*z.ptr()).base.position;
        for _ in 0..3 {
            gallery_new_proj(zp.x, zp.y, 0.0, 0.0, 598);
        }
        for p in (*m).projectile.as_slice() {
            if (*p.ptr()).active && (*p.ptr()).type_ == 598 {
                *(*p.ptr()).ai.at(0) = 1.0;
                *(*p.ptr()).ai.at(1) = (*z.ptr()).base.who_am_i as f32;
            }
        }
        N::dot_tally::add_stacked_projecitle(&mut tally, 598, 3, 1);
        r.probe("dottally", &[&tally.life_regen_loss, &tally.life_regen_loss_that_can_be_boosted, &tally.expected_damage_per_second]);
        r.state.push_str(&state_line("npcmisc"));
        clear_entities();
        r.frames(2);
    }
    // ---- the dual dungeons seed's dungeon zones
    if probe_group(r, "dualdungeons") {
        (*m).dual_dungeons_seed = true;
        place_player(3352, 295);
        r.frames(30);
        r.record("dualdungeons", 1);
        (*m).dual_dungeons_seed = false;
        place_player(sx, sy);
        r.frames(10);
    }
}
