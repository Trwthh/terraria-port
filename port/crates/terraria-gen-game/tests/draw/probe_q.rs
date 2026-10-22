// The probe run, part Q: tools/refdump_draw/ProbeQ.cs, step for step. Included by tests/draw.rs.

/// ProbeQ.cs `ProbeQ`.
unsafe fn probe_q(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    let m = terraria::main::s();
    let pl = local_player();
    let p_ = pl.ptr();
    let me = (*m).my_player;
    if probe_group(r, "pylonmap") {
        use terraria::game_content::teleport_pylons_system as TPS;
        clear_entities();
        place_player(x0 + 2, gy - 1);
        r.frames(2);
        for p in [[x0 + 8, 0], [x0 + 20, 1]] {
            terraria::world_gen::place_object(p[0], gy - 1, 597, false, p[1], 0, -1, 1);
            let (tx, ty) = top_left_of(597, p[0] + 1, gy - 1);
            let id = terraria::game_content::tile_entities::te_teleportation_pylon::placement_preview_hook_after_placement(p[0], gy - 1, 597, p[1], 1, 0);
            r.probe(&format!("pylon_{}", p[1]), &[&tx, &ty, &id]);
        }
        let sys = (*m).pylon_system.ptr();
        TPS::request_immediate_update(sys);
        let pylons = (*sys)._pylons;
        r.probe("pylons", &[&(*pylons.ptr()).count()]);
        if (*pylons.ptr()).count() > 0 {
            (*m).map_pylon_tile = (*pylons.ptr()).get_item(0).position_in_tiles;
        }
        let inv = (*m).player_inventory;
        (*m).player_inventory = false;
        (*m).map_fullscreen = true;
        (*m).map_fullscreen_scale = 1.0;
        (*m).map_fullscreen_pos = v2((x0 + 14) as f32, (gy - 2) as f32);
        r.input.x = 640;
        r.input.y = 360;
        r.frames(3);
        r.record("pylonmap", 1);
        r.probe("pylonmap_open", &[&(*m).map_fullscreen]);
        (*m).map_fullscreen = false;
        (*m).player_inventory = inv;
        (*m).map_pylon_tile.x = -1;
        (*m).map_pylon_tile.y = -1;
        new_npc_at((x0 + 19) * 16, (gy - 2) * 16, 22);
        new_npc_at((x0 + 23) * 16, (gy - 2) * 16, 17);
        r.frames(1);
        if (*pylons.ptr()).count() > 1 {
            TPS::handle_teleport_request(sys, (*pylons.ptr()).get_item(1), me);
            r.probe("pylon_teleport", &[&(*p_).base.position]);
        }
        place_player(x0 + 2, gy - 1);
        r.frames(2);
        clear_area(x0, x0 + 30, gy);
        TPS::request_immediate_update(sys);
        r.frames(2);
        r.state.push_str(&state_line("pylonmap"));
    }
    if probe_group(r, "capturemode") {
        use terraria::graphics::capture::capture_interface as CI;
        use terraria::graphics::capture::capture_manager as CM;
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let cmgr = (*CM::s()).instance.ptr();
        let ci = (*cmgr)._interface.ptr();
        CM::set_active(cmgr, true);
        let st = CI::s();
        (*st).edge_a = terraria_core::xna::Point { x: sx - 10, y: sy - 8 };
        (*st).edge_b = terraria_core::xna::Point { x: sx + 12, y: sy + 6 };
        (*st).edge_a_pinned = true;
        (*st).edge_b_pinned = true;
        let modes = (*st).modes;
        for mode in 0..3 {
            (*ci).selected_mode = mode;
            r.input.x = 600;
            r.input.y = 300;
            r.frames(2);
            r.record(&format!("capture_mode{mode}"), 1);
        }
        (*ci).selected_mode = 1;
        CI::mode_drag_bounds::drag_bounds((*modes.ptr()).get_item(1).up::<CI::mode_drag_bounds::ModeDragBounds>().ptr(), v2(700.0, 340.0));
        r.probe("dragbounds", &[&(*st).edge_a, &(*st).edge_b]);
        let mut res = terraria_core::xna::Point { x: 0, y: 0 };
        let ok = CI::get_map_coords(sx, sy, 0, &mut res);
        r.probe("mapcoords", &[&ok, &res]);
        (*ci).selected_mode = 0;
        CM::set_active(cmgr, false);
        r.frames(2);
        r.input.x = 640;
        r.input.y = 360;
        let area = CI::full_screen_area();
        let settings = terraria::graphics::capture::capture_settings::new();
        let _rs = rt::Rooted::new(settings);
        (*settings.ptr()).area = area;
        (*settings.ptr()).biome = terraria::graphics::capture::capture_biome::get_capture_biome(-1);
        let _ = terraria::graphics::capture::capture_biome::get_capture_biome(1);
        (*settings.ptr()).capture_background = true;
        (*settings.ptr()).output_name = s!("probe");
        use terraria::graphics::capture::capture_camera as CC;
        let cam = (*cmgr)._camera.ptr();
        CC::capture(cam, settings);
        let prog = CC::get_progress(cam);
        let capturing = CM::is_capturing(cmgr);
        let queued = (*(*cam)._render_queue.ptr()).count();
        r.probe("capture", &[&area, &prog, &capturing, &queued]);
        let inst = (*m).instance.ptr();
        let chunk = rect(sx - 20, sy - 15, 40, 30);
        r.direct("capture_draw", |_| {
            terraria::game_content::drawing::tile_drawing::prepare_for_area_drawing((*inst).tiles_renderer.ptr(), chunk.x, chunk.x + chunk.width, chunk.y, chunk.y + chunk.height, false);
            terraria::game_content::tile_paint_system_v2::prepare_all_requests((*inst).tile_paint_system.ptr());
            (*(*cam)._graphics.ptr()).set_render_target((*cam)._frame_buffer);
            terraria::main::draw_capture(inst, chunk, settings, (*cmgr)._camera);
            (*(*cam)._graphics.ptr()).set_render_target(Gc::<terraria_host::gfx::Texture2D>::NULL);
        });
        (*(*cam)._render_queue.ptr()).clear();
        (*cam)._output_data = Arr::NULL;
        (*cam)._scaled_frame_data = Arr::NULL;
        if !(*cam)._scaled_frame_buffer.is_null() {
            (*(*cam)._scaled_frame_buffer.ptr()).dispose();
            (*cam)._scaled_frame_buffer = Gc::NULL;
        }
        (*cam)._active_settings = Gc::NULL;
        (*m).global_timer_paused = false;
        CI::end_camera();
        r.frames(2);
        r.record("capture_done", 1);
        r.state.push_str(&state_line("capturemode"));
    }
}
