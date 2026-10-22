// The probe run, part N: tools/refdump_draw/ProbeN.cs, step for step. Included by tests/draw.rs.

/// ProbeN.cs `UIWalk`.
unsafe fn ui_walk(e: Gc<terraria::ui::ui_element::UIElement>, into: &mut Vec<Gc<terraria::ui::ui_element::UIElement>>) {
    into.push(e);
    let els = (*e.ptr()).elements;
    for k in 0..(*els.ptr()).count() {
        ui_walk((*els.ptr()).get_item(k), into);
    }
}

/// ProbeN.cs `ProbeN`.
unsafe fn probe_n(r: &mut Run, sx: i32, sy: i32, _x0: i32, _gy: i32) {
    use terraria::ui::ingame_fancy_ui as IFU;
    let m = terraria::main::s();
    if probe_group(r, "uikeybinds") {
        use terraria::game_content::ui::states::ui_manage_controls as UMC;
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        IFU::open_keybinds();
        r.frames(3);
        let mc = (*m).manage_controls_menu;
        let mce: Gc<terraria::ui::ui_element::UIElement> = mc.up();
        UMC::gamepad_button_click(mc.ptr(), mouse_evt(mce), mce);
        r.frames(2);
        r.record("keybinds_gamepad", 1);
        UMC::vs_menu_button_click(mc.ptr(), mouse_evt(mce), mce);
        r.frames(2);
        r.record("keybinds_gamepad_ui", 1);
        let mut all = Vec::new();
        ui_walk(mce, &mut all);
        let mut n = 0;
        use terraria::game_content::ui::elements::ui_keybinding_slider_item as SL;
        for e in all {
            if !rt::obj_is::<SL::UIKeybindingSliderItem>(e.up()) {
                continue;
            }
            let s: Gc<SL::UIKeybindingSliderItem> = e.up();
            let st = (*s.ptr())._get_status_function.invoke(|f| f());
            (*s.ptr())._slide_keyboard_action.invoke(|f| f(st));
            (*s.ptr())._slide_gamepad_action.invoke(|f| f());
            let text = (*s.ptr())._text_display_function.invoke(|f| f());
            r.probe(&format!("slider_{n}"), &[&text, &st]);
            n += 1;
        }
        UMC::keyboard_button_click(mc.ptr(), mouse_evt(mce), mce);
        UMC::vs_gameplay_button_click(mc.ptr(), mouse_evt(mce), mce);
        IFU::close(false);
        r.frames(2);
        r.state.push_str(&state_line("uikeybinds"));
    }
    if probe_group(r, "uiachieve") {
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        IFU::open_achievements();
        r.frames(3);
        let menu = (*m).achievements_menu;
        let ach = terraria::achievements::achievement_manager::get_achievement(terraria::main::achievements().ptr(), s!("TIMBER"));
        terraria::game_content::ui::states::ui_achievements_menu::goto_achievement(menu.ptr(), ach);
        r.frames(2);
        r.record("achievements_goto", 1);
        let list = (*menu.ptr())._achievements_list;
        terraria::game_content::ui::elements::ui_list::scroll_wheel(list.ptr(), terraria::ui::ui_scroll_wheel_event::new(list.up(), v2(640.0, 360.0), -240));
        r.frames(2);
        r.record("achievements_scrolled", 1);
        IFU::close(false);
        r.frames(2);
        r.state.push_str(&state_line("uiachieve"));
    }
    if probe_group(r, "uibestiary") {
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let z = npc_near(3, 200.0, 0.0);
        let kills = (*(*m).bestiary_tracker.ptr()).kills;
        for _ in 0..60 {
            terraria::game_content::bestiary::npc_kills_tracker::register_kill(kills.ptr(), z);
        }
        clear_entities();
        let ui = (*m).bestiary_ui;
        IFU::open_ui_state__ui_state(ui.up());
        r.frames(3);
        let entry = terraria::game_content::bestiary::bestiary_database::find_entry_by_npcid((*m).bestiary_db.ptr(), 3);
        let info = (*ui.ptr())._info_space;
        let mut extra: terraria::game_content::ui::elements::extra_bestiary_info_page_information::ExtraBestiaryInfoPageInformation = rt::zero();
        extra.bestiary_progress_report = (*ui.ptr())._progress_report;
        terraria::game_content::ui::elements::ui_bestiary_entry_info_page::fill_info_for_entry(info.ptr(), entry, extra);
        r.frames(2);
        r.record("bestiary_entry", 1);
        terraria::game_content::ui::elements::ui_bestiary_entry_info_page::update_scrollbar(info.ptr(), -120);
        r.frames(2);
        r.record("bestiary_scrolled", 1);
        IFU::close(false);
        r.frames(2);
        r.state.push_str(&state_line("uibestiary"));
    }
}
