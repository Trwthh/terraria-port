// The probe run, part O: tools/refdump_draw/ProbeO.cs, step for step. Included by tests/draw.rs.

/// ProbeO.cs `UIPlace`.
unsafe fn ui_place(e: Gc<terraria::ui::ui_element::UIElement>, x: f32, y: f32, w: f32, h: f32) {
    use terraria::ui::style_dimension::StyleDimension;
    let px = |v: f32| StyleDimension { pixels: v, precent: 0.0 };
    (*e.ptr()).left = px(x);
    (*e.ptr()).top = px(y);
    (*e.ptr()).width = px(w);
    (*e.ptr()).height = px(h);
    terraria::ui::ui_element::recalculate__v(e.ptr());
}

/// ProbeO.cs `ProbeO`.
unsafe fn probe_o(r: &mut Run, sx: i32, sy: i32, _x0: i32, _gy: i32) {
    use terraria_core::xna::Color;
    type UIE = terraria::ui::ui_element::UIElement;
    let m = terraria::main::s();
    let pl = local_player();
    let p_ = pl.ptr();
    if probe_group(r, "uicrafting") {
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        set_inv(10, 9, 99);
        set_inv(11, 3, 99);
        terraria::game_content::ui::new_crafting_ui::open(true, Gc::NULL);
        r.frames(4);
        r.record("newcrafting", 1);
        terraria::game_content::ui::new_crafting_ui::close(true, false);
        terraria::ui::crafting_ui::scroll_crafting_grid((*m).crafting_ui.ptr(), -1, 5);
        terraria::ui::crafting_ui::scroll_crafting_grid((*m).crafting_ui.ptr(), 1, 5);
        let banner = (*m).banner_ui.ptr();
        terraria::ui::banner_claiming_ui::scroll_crafting_grid(banner, -1, 5);
        terraria::ui::banner_claiming_ui::navigate_pips_list(banner, 1);
        terraria::ui::banner_claiming_ui::navigate_pips_grid(banner, 1, 1);
        (*m).player_inventory = false;
        give_item(10, 0);
        give_item(11, 0);
        r.frames(2);
        r.state.push_str(&state_line("uicrafting"));
    }
    if probe_group(r, "uicreative") {
        use terraria::game_content::creative::creative_ui as CU;
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let diff = (*p_).difficulty;
        (*p_).difficulty = 3;
        (*m).player_inventory = true;
        let cm = (*m).creative_menu.ptr();
        CU::toggle_menu(cm);
        r.frames(3);
        let st = (*cm)._ui_state.ptr();
        terraria::game_content::ui::states::ui_creative_powers_menu::toggle_main_category(st, 1);
        r.frames(3);
        r.record("creative_items", 1);
        terraria::game_content::ui::states::ui_creative_powers_menu::toggle_main_category(st, 2);
        r.frames(2);
        let it = cover_new_item(9);
        (*it.ptr()).stack = 99;
        CU::set_itemby_index(cm, it, 0);
        CU::sacrifice_item_in_sacrifice_slot(cm);
        r.frames(6);
        r.record("creative_research", 1);
        CU::toggle_menu(cm);
        (*m).player_inventory = false;
        (*p_).difficulty = diff;
        clear_items();
        for k in 0..58 {
            give_item(k, 0);
        }
        r.frames(2);
        r.state.push_str(&state_line("uicreative"));
    }
    if probe_group(r, "uielements") {
        use terraria::game_content::ui::elements as E;
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let hair: Gc<UIE> = E::ui_hair_style_button::new(pl, 5).up();
        ui_place(hair, 100.0, 100.0, 44.0, 44.0);
        let icon_b = E::ui_icon_text_button::new(terraria::localization::language::get_text(s!("UI.Back")), Color::WHITE, s!("Images/UI/Workshop/Publish"), 1.0, 0.5, 10.0);
        let icon: Gc<UIE> = icon_b.up();
        ui_place(icon, 160.0, 100.0, 200.0, 40.0);
        E::ui_icon_text_button::set_text(icon_b.ptr(), terraria::localization::language::get_text(s!("UI.Cancel")), 0.9, Color::YELLOW);
        let ta = terraria::game_content::texture_assets::s();
        let border: Gc<UIE> = E::ui_image_with_border::new__asset_texture2_d_asset_texture2_d((*ta).inventory_back, (*ta).inventory_back2).up();
        ui_place(border, 380.0, 100.0, 40.0, 40.0);
        let bar_b = E::ui_progress_bar::new();
        let bar: Gc<UIE> = bar_b.up();
        ui_place(bar, 100.0, 160.0, 300.0, 20.0);
        E::ui_progress_bar::set_progress(bar_b.ptr(), 0.37);
        let sv = std::rc::Rc::new(std::cell::Cell::new(0.6f32));
        let (s1, s2) = (sv.clone(), sv.clone());
        let slider: Gc<UIE> = E::ui_vertical_slider::new(
            Delegate::<dyn Fn() -> f32>::new(Box::new(move || s1.get())),
            Delegate::<dyn Fn(f32)>::new(Box::new(move |f| s2.set(f))),
            Delegate::<dyn Fn()>::new(Box::new(|| {})),
            Color::RED,
        )
        .up();
        ui_place(slider, 420.0, 160.0, 20.0, 120.0);
        let scroll_b = E::ui_scrollbar::new(0);
        let scroll: Gc<UIE> = scroll_b.up();
        ui_place(scroll, 460.0, 160.0, 20.0, 200.0);
        E::ui_scrollbar::set_view(scroll_b.ptr(), 50.0, 400.0);
        let diag = terraria::ui::net_diagnostics_ui::new();
        let _roots = (rt::Rooted::new(hair), rt::Rooted::new(icon), rt::Rooted::new(border), rt::Rooted::new(bar), rt::Rooted::new(slider), rt::Rooted::new(scroll), rt::Rooted::new(diag));
        let sb = (*m).sprite_batch;
        r.direct("uielements", |_| {
            in_batch(|| {
                for e in [hair, icon, border, bar, slider, scroll] {
                    terraria::ui::ui_element::draw__v(e.ptr(), sb);
                }
                terraria::ui::net_diagnostics_ui::draw(diag.ptr(), sb);
                terraria::game_content::ui::custom_currency_manager::draw_savings(sb, (*terraria::id::custom_currency_id::s()).defender_medals, 500.0, 300.0, false);
            })
        });
        r.input.x = 470;
        r.input.y = 250;
        r.frames(1);
        E::ui_scrollbar::left_mouse_down(scroll_b.ptr(), mouse_evt(scroll));
        r.probe("scrollbar", &[&E::ui_scrollbar::get_value(scroll_b.ptr())]);
        r.input.x = 640;
        r.input.y = 360;
        let lines = Arr::<Str>::new(4);
        let mut cl = 0i32;
        terraria::game_content::ui::custom_currency_manager::get_price_text((*terraria::id::custom_currency_id::s()).defender_medals, lines, &mut cl, 25);
        r.probe("currencyprice", &[&cl, &*lines.at(0)]);
        let frame = (*(*m).minimap_frame_manager_instance.ptr()).base.active_selection;
        let ms = (*m).map_minimap_scale;
        terraria::game_content::ui::minimap::minimap_frame::zoom_in_button(frame.ptr());
        terraria::game_content::ui::minimap::minimap_frame::zoom_out_button(frame.ptr());
        r.probe("minimapzoom", &[&(*m).map_minimap_scale]);
        (*m).map_minimap_scale = ms;
        let anchor = terraria::game_content::ui::world_ui_anchor::new__entity(pl.up());
        let bubble = terraria::game_content::ui::emote_bubble::new(0, anchor, 180);
        let biomes = List::<i32>::new_gc();
        terraria::game_content::ui::emote_bubble::probe_biomes(bubble.ptr(), biomes, pl);
        r.probe("emotebiomes", &[&(*biomes.ptr()).count()]);
        terraria::ui::ingame_fancy_ui::open_virtual_keyboard(1);
        r.frames(2);
        r.record("virtualkeyboard", 1);
        terraria::ui::ingame_fancy_ui::close(false);
        r.frames(2);
        r.state.push_str(&state_line("uielements"));
    }
}
