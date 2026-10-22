// The probe run, part R: tools/refdump_draw/ProbeR.cs, step for step. Included by tests/draw.rs.

/// ProbeR.cs `ProbeR`.
unsafe fn probe_r(r: &mut Run, sx: i32, sy: i32, _x0: i32, _gy: i32) {
    let m = terraria::main::s();
    if probe_group(r, "uilinks") {
        use terraria::ui::gamepad::ui_link_point_navigator as NAV;
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let pages = (*NAV::s()).pages;
        for id in 0..=100 {
            if !(*pages.ptr()).contains_key(id) {
                continue;
            }
            let page = (*pages.ptr()).get_item(id);
            let ev = (*page.ptr()).reach_end_event__event;
            if ev.is_null() {
                continue;
            }
            for next in [-1, -2, -3, -4] {
                ev.invoke(|f| f(0, next));
            }
        }
        let sc = NAV::shortcuts::s();
        let back = (*sc).back_button_command;
        let menu = (*m).menu_mode;
        (*sc).back_button_command = 3;
        terraria::initializers::ui_links_initializer::fancy_exit();
        (*sc).back_button_command = back;
        (*m).menu_mode = menu;
        r.frames(2);
        r.state.push_str(&state_line("uilinks"));
    }
    if probe_group(r, "menuclicks") {
        use terraria::game_content::ui::elements as E;
        let menu = (*m).menu_mode;
        let pli = E::ui_character_list_item::new((*m).active_player_file_data, 0);
        let plie: Gc<terraria::ui::ui_element::UIElement> = pli.up();
        E::ui_character_list_item::delete_button_click(pli.ptr(), mouse_evt(plie), plie);
        r.probe("menu_deleteplayer", &[&(*m).menu_mode]);
        (*m).menu_mode = menu;
        let wli = E::ui_world_list_item::new((*m).active_world_file_data, 0, true, true, false);
        let wlie: Gc<terraria::ui::ui_element::UIElement> = wli.up();
        E::ui_world_list_item::delete_button_click(wli.ptr(), mouse_evt(wlie), wlie);
        r.probe("menu_deleteworld", &[&(*m).menu_mode]);
        (*m).menu_mode = menu;
        let ws = terraria::game_content::ui::states::ui_world_select::new();
        let wse: Gc<terraria::ui::ui_element::UIElement> = ws.up();
        let nwn = (*m).new_world_name;
        terraria::game_content::ui::states::ui_world_select::new_world_click(ws.ptr(), mouse_evt(wse), wse);
        r.probe("menu_newworld", &[&(*m).menu_mode, &(*m).new_world_name]);
        terraria::ui::user_interface::set_state((*m).menu_ui.ptr(), Gc::NULL);
        (*m).menu_mode = menu;
        (*m).new_world_name = nwn;
        r.frames(2);
        r.state.push_str(&state_line("menuclicks"));
    }
    if probe_group(r, "timelogfmt") {
        let fmt = (*terraria::time_logger::s())._int_format.ptr();
        use terraria::time_logger::format_pool as FP;
        let (a, b, c) = (FP::format__double(fmt, 42.0), FP::format__double(fmt, 1234.6), FP::format__double(fmt, -3.0));
        r.probe("timelog_format", &[&a, &b, &c]);
        terraria::net_message::resync_tiles__int_rectangle(0, rect(sx - 300, sy - 200, 600, 400));
        r.state.push_str(&state_line("timelogfmt"));
    }
}
