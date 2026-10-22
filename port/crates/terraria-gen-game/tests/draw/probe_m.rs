// The probe run, part M: tools/refdump_draw/ProbeM.cs, step for step. Included by tests/draw.rs.

/// ProbeM.cs `NewWorldItem`.
unsafe fn new_world_item(x: i32, y: i32, ty: i32, stack: i32) -> i32 {
    terraria::item::new_item__i_entity_source_int_int_int_int_int_int_bool_int_new_item_ownership_nullable_vector2_new_item_modifier(
        src(),
        x,
        y,
        16,
        16,
        ty,
        stack,
        false,
        0,
        0,
        rt::zero::<Nullable<terraria_core::xna::Vector2>>(),
        rt::zero::<rt::Delegate<dyn Fn(Gc<terraria::world_item::WorldItem>)>>(),
    )
}

/// ProbeM.cs `ProbeM`.
unsafe fn probe_m(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    use terraria::world_item as WI;
    use terraria_core::xna::{Color, Point};
    let m = terraria::main::s();
    let pl = local_player();
    let p_ = pl.ptr();
    let me = (*m).my_player;
    let wi_at = |i: i32| *(*m).item.at(i);
    if probe_group(r, "miscdirect") {
        clear_entities();
        clear_fx();
        place_player(x0 + 2, gy - 1);
        r.frames(2);
        let pp = (*p_).base.position;
        WI::spawn_shimmered_item__i_entity_source_vector2_int_int_int(src(), v2(pp.x + 40.0, pp.y), 400, 9, 3);
        let z = npc_near(3, 30.0, 0.0);
        let coin = wi_at(new_world_item(pp.x as i32 + 30, pp.y as i32, 72, 5));
        WI::get_picked_up_by_monsters_money(coin.ptr());
        r.probe("monstermoney", &[&(*z.ptr()).extra_value, &WI::active(coin.ptr())]);
        let bucket = wi_at(new_world_item((x0 + 20) * 16, (gy - 2) * 16, 205, 1));
        let wt = gallery_tile(x0 + 20, gy - 1);
        (*wt.ptr()).liquid = 255;
        terraria::tile::liquid_type__int(wt.ptr(), 0);
        WI::try_fill_bucket(bucket.ptr());
        r.probe("bucket", &[&WI::type_(bucket.ptr()), &(*wt.ptr()).liquid]);
        (*wt.ptr()).liquid = 0;
        let doll = wi_at(new_world_item((x0 + 22) * 16, (gy - 2) * 16, 267, 1));
        npc_near(22, 200.0, 0.0);
        WI::voodoo_doll_lava_death(doll.ptr());
        r.probe("voodoo", &[&(find_npc(22) >= 0), &WI::active(doll.ptr())]);
        clear_items();
        place_tile(x0 + 25, gy - 1, 1, 0);
        terraria::wiring::de_active(x0 + 25, gy - 1);
        r.probe_tiles("deactive", x0 + 25, gy - 1, 1, 1);
        place_tile(x0 + 28, gy - 1, 21, 0);
        terraria::wiring::hopper(x0 + 28, gy - 2);
        terraria::map::world_map::unlock_map_tile_pretty((*m).map.ptr(), sx + 3, sy - 5);
        let pt = terraria::popup_text::new_text__popup_text_context_int_vector2_bool(2, 1, v2(pp.x, pp.y - 40.0), false);
        r.probe("popup", &[&pt]);
        if pt >= 0 {
            terraria::popup_text::emit_fancy_flash_dust(*(*terraria::popup_text::s()).popup_text.at(pt));
        }
        r.probe("actcolor", &[&terraria::tile::act_color__color(gallery_tile(x0 + 25, gy - 1).ptr(), Color::new(200, 150, 100, 255))]);
        terraria::player::pet_mount(p_, rt::zero());
        let mut tpos = v2(0.0, 0.0);
        let tok = terraria::game_content::teleport_helpers::find_closest_teleport_spot_no_space(pl, &mut tpos);
        r.probe("teleportspot", &[&tok, &tpos]);
        let (mut bx, mut by) = (0i32, 0i32);
        let po = terraria::game_content::portal_helper::get_portal_outing_point(v2(20.0, 42.0), v2(pp.x + 100.0, pp.y), 1.0, &mut bx, &mut by);
        r.probe("portalout", &[&po, &bx, &by]);
        r.probe("smartdir", &[&terraria::game_content::smart_cursor_helper::get_desired_direction_from(v2(30.0, -50.0))]);
        let g = terraria::gore::new_gore(v2(pp.x, pp.y - 30.0), v2(0.0, 0.0), 1, 1.0);
        (*wt.ptr()).liquid = 200;
        r.probe("chumwater", &[&terraria::gore::chum_floating_chunk_get_water_line((*(*m).gore.at(g)).ptr(), x0 + 20, gy - 1)]);
        (*wt.ptr()).liquid = 0;
        r.probe("largerange", &[&(*(*m).rand.ptr()).next_range(i32::MIN + 5, i32::MAX - 5)]);
        let font = re_logic::content::asset__dynamic_sprite_font::value((*terraria::game_content::font_assets::s()).mouse_text.ptr());
        r.probe("chattext", &[&terraria::ui::chat::chat_manager::add_chat_text(font, s!("[i:9]"), v2(1.0, 1.0))]);
        let club = proj_at(gallery_new_proj(pp.x, pp.y, 0.0, 0.0, 721));
        let ball = proj_at(gallery_new_proj(pp.x + 20.0, pp.y, 0.0, 0.0, 721));
        let strength = terraria::game_content::golf::golf_helper::calculate_shot_strength__projectile_entity(club, ball.up::<terraria::entity::Entity>());
        r.probe("golfstrength", &[&strength.absolute_strength, &strength.relative_strength]);
        let _ = terraria::game_content::void_lens_helper::new__vector2_float(v2(pp.x, pp.y - 50.0), 0.8);
        let tags = (*terraria::id::item_id::sets::s()).unique_tag_effects;
        let proj = proj_at(gallery_new_proj(pp.x, pp.y, 0.0, 0.0, 1));
        for wi in [4680, 5476, 4678, 5477, 5478, 4912] {
            use terraria::game_content::items::unique_tag_effect as UT;
            let tag = (*tags.at(wi)).ptr();
            let mut ch: terraria::game_content::items::tag_damage_changes::TagDamageChanges = rt::zero();
            UT::modify_tagged_hit__v(tag, pl, proj, z, &mut ch);
            UT::modify_proc_hit__v(tag, pl, proj, z, &mut ch);
            UT::on_tagged_hit__v(tag, pl, proj, z, 40);
            let ok = UT::on_proc_hit__v(tag, pl, proj, z, 40);
            r.probe(&format!("whiptag_{wi}"), &[&ok]);
        }
        let mut flags: terraria::npc_spawning_flags_for_dual_dungeons::NPCSpawningFlagsForDualDungeons = rt::zero();
        let (a, b) = (
            terraria::npc_spawning_flags_for_dual_dungeons::scan_zones_for(&mut flags, x0 + 3, gy),
            terraria::npc_spawning_flags_for_dual_dungeons::scan_zones_for(&mut flags, sx, sy + 1),
        );
        r.probe("dualscan", &[&a, &b]);
        {
            use terraria::utilities::weighted_random__color as WR;
            let wr = WR::new__void();
            WR::add(wr.ptr(), Color::new(200, 160, 20, 180), 3.0);
            WR::add(wr.ptr(), Color::new(103, 98, 122, 180), 5.0);
            let (a, b) = (WR::get(wr.ptr()), WR::get(wr.ptr()));
            r.probe("weighted", &[&a, &b]);
        }
        {
            use terraria::game_content::drawing::particle_orchestrator as PO;
            let mut pos: terraria::game_content::drawing::particle_orchestra_settings::ParticleOrchestraSettings = rt::zero();
            pos.position_in_world = v2(pp.x + 60.0, pp.y + 8.0);
            pos.movement_vector = v2(1.0, 0.0);
            pos.unique_info_piece = 2290;
            PO::spawn_fake_fish(pos, false);
            let bob = proj_at(gallery_new_proj(pp.x + 70.0, pp.y, 0.0, 0.0, 360));
            PO::ping_fake_fish(bob, 2290);
            PO::push_away_fake_fish(bob, 2290);
            use terraria::graphics::renderers::fake_fish_particle as FF;
            let parts = (*(*m).particle_system_world_behind_players.ptr()).particles;
            let mut fish: Option<Gc<FF::FakeFishParticle>> = None;
            for k in 0..(*parts.ptr()).count() {
                let pa = (*parts.ptr()).get_item(k);
                if !pa.is_null() && rt::obj_is::<FF::FakeFishParticle>(pa) {
                    fish = Some(pa.up());
                }
            }
            if let Some(fish) = fish {
                let mut rd: terraria::graphics::renderers::particle_repel_details::ParticleRepelDetails = rt::zero();
                rd.source_position = v2(pp.x + 50.0, pp.y);
                rd.radius = 30.0;
                rd.is_in_water = true;
                FF::be_repelled(fish.ptr(), &mut rd);
                r.probe("fakefish", &[&(*fish.ptr()).position, &(*fish.ptr()).velocity]);
            }
        }
        terraria::game_content::teleport_pylons_system::spawn_in_world_dust(1, rect(pp.x as i32, pp.y as i32, 48, 64));
        place_tile(x0 + 32, gy - 1, 597, 0);
        let mut pinfo: terraria::game_content::teleport_pylon_info::TeleportPylonInfo = rt::zero();
        pinfo.position_in_tiles.x = (x0 + 31) as i16;
        pinfo.position_in_tiles.y = (gy - 4) as i16;
        pinfo.type_of_pylon = 0;
        terraria::game_content::teleport_pylons_system::handle_teleport_request((*m).pylon_system.ptr(), pinfo, me);
        r.probe("pylon", &[&(*p_).base.position]);
        place_player(x0 + 2, gy - 1);
        probe_fx(r, "miscdirect");
        gallery_immune();
        (*p_).stat_life = 100;
        r.state.push_str(&state_line("miscdirect"));
        r.frames(3);
        r.record("miscdirect", 1);
        clear_entities();
        clear_fx();
        clear_items();
        clear_area(x0, x0 + 40, gy);
        clear_liquid(x0, gy - 12, 41, 12);
        r.frames(2);
    }
    if probe_group(r, "drawmisc") {
        clear_entities();
        clear_fx();
        place_player(sx, sy);
        r.frames(2);
        let pp = (*p_).base.position;
        use terraria::physics::collision_visualizer as CV;
        let doll = terraria::game_content::tile_entities::te_display_doll::new();
        let rack = terraria::game_content::tile_entities::te_hat_rack::new();
        let sb = (*m).sprite_batch;
        r.direct("drawmisc", |_| {
            in_batch(|| {
                (*CV::s()).enabled = true;
                (*CV::s()).color_self = Color::new(50, 205, 50, 255);
                CV::visualize_cone_fast(pp, rect(pp.x as i32 + 40, pp.y as i32, 20, 20), 0.3, 0.6, 90.0, true);
                CV::visualize_cone_slow(pp, rect(pp.x as i32 + 40, pp.y as i32, 20, 20), -0.3, 0.5, 80.0, true);
                let hit = terraria::data_structures::multi_point_hitbox::new(Point { x: 4, y: 4 }, Arr::<terraria_core::xna::Vector2>::from_slice(&[pp, pp + v2(30.0, -20.0), pp + v2(60.0, 10.0)]));
                CV::visualize_aab_bv_multi_point(rect(pp.x as i32, pp.y as i32, 20, 20), hit);
                (*CV::s()).enabled = false;
                terraria::game_content::tile_entities::te_display_doll::draw_ui(doll.ptr(), pl, sb);
                terraria::game_content::tile_entities::te_hat_rack::draw_inner(rack.ptr(), pl, sb);
                let renderer: Gc<terraria::graphics::renderers::legacy_player_renderer::LegacyPlayerRenderer> = (*m).player_renderer.up();
                terraria::graphics::renderers::legacy_player_renderer::draw_player_head(renderer.ptr(), (*m).camera, pl, v2(400.0, 300.0), 1.0, 1.2, Color::WHITE);
            })
        });
        terraria::player::add_buff(p_, 156, 120, false);
        r.frames(2);
        r.record("stoned", 1);
        clear_player_buffs();
        r.state.push_str(&state_line("drawmisc"));
        r.frames(2);
    }
}
