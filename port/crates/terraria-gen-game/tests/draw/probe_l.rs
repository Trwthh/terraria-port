// The probe run, part L: tools/refdump_draw/ProbeL.cs, step for step. Included by tests/draw.rs.

impl Pv for terraria::data_structures::point16::Point16 {
    fn pv(&self) -> String {
        format!("{},{}", self.x, self.y)
    }
}
/// ProbeL.cs `InvSet`.
unsafe fn inv_set(k: i32, ty: i32, stack: i32) {
    let it = *(*local_player().ptr()).inventory.at(k);
    terraria::item::set_defaults(it.ptr(), ty, Gc::NULL);
    if ty != 0 {
        (*it.ptr()).stack = stack;
    }
}
/// ProbeL.cs `FindNpc`.
unsafe fn find_npc(ty: i32) -> i32 {
    let npcs = (*terraria::main::s()).npc;
    for i in 0..npcs.len() {
        let n = *npcs.at(i);
        if (*n.ptr()).active && (*n.ptr()).type_ == ty {
            return i;
        }
    }
    -1
}

/// ProbeL.cs `ProbeL`.
unsafe fn probe_l(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    use terraria::main as M;
    use terraria::minecart as MC;
    use terraria::mount as MT;
    use terraria::ui::item_slot as IS;
    use terraria_core::xna::Color;
    let m = M::s();
    let pl = local_player();
    let p_ = pl.ptr();
    let me = (*m).my_player;
    let inst = (*m).instance.ptr();
    // ---- mounts
    if probe_group(r, "mountdirect") {
        clear_entities();
        clear_fx();
        for x in x0 + 4..=x0 + 30 {
            place_tile(x, gy - 1, 314, 0);
        }
        place_player(x0 + 8, gy - 2);
        r.frames(2);
        let pp = (*p_).base.position;
        let ms = (*p_).minecart_settings;
        r.probe("magnet", &[&MC::get_magnet_position(pp, 20, 42, ms)]);
        let mut otp = pp;
        let ot = MC::get_on_track(x0 + 10, gy - 1, &mut otp, 20, 42, ms);
        r.probe("ontrack", &[&ot, &otp]);
        MC::hit_track_switch(pp, 20, 42, ms);
        let mt = (*p_).mount.ptr();
        MT::set_mount(mt, 11, pl);
        r.frames(2);
        let pp = (*p_).base.position;
        let dlg = MT::delegations(mt);
        let mut rot = 0.0f32;
        let tr = MC::track_rotation(pl, &mut rot, pp, 20, 42, false, false, dlg);
        r.probe("trackrot", &[&tr, &rot]);
        MC::wheel_sparks((*dlg.ptr()).minecart_dust.clone(), pp, 20, 42, 3, ms);
        r.probe("mechpoint", &[&MT::get_minecart_mech_point(pl, 20, -19)]);
        MT::meowcart_bumper_sound(pl, pp, 20, 42);
        MT::meowcart_landing_sound(pl, pp, 20, 42);
        npc_near(3, 60.0, 0.0);
        (*p_).base.velocity = v2(4.0, 0.0);
        MT::cast_super_cart_laser(mt, pl);
        (*p_).base.velocity = v2(0.0, 0.0);
        MT::do_failed_dismount_dust(mt, pl, 20);
        MT::dismount(mt, pl, false);
        MT::set_mount(mt, 9, pl);
        r.frames(1);
        r.input.x = 800;
        r.input.y = 300;
        r.frames(1);
        MT::start_ability_charge(mt, pl);
        r.probe("abilitycharge", &[&MT::ability_charge(mt)]);
        MT::dismount(mt, pl, false);
        MT::set_mount(mt, 8, pl);
        r.frames(1);
        let dw = MT::drill_smart_cursor_walls(mt, pl, (*mt)._mount_specific_data.up());
        r.probe("drillwalls", &[&dw]);
        MT::dismount(mt, pl, false);
        r.input.x = 640;
        r.input.y = 360;
        probe_fx(r, "mountdirect");
        r.state.push_str(&state_line("mountdirect"));
        r.frames(3);
        r.record("mountdirect", 1);
        clear_entities();
        clear_fx();
        clear_area(x0, x0 + 40, gy);
        r.frames(2);
    }
    // ---- Main helpers
    if probe_group(r, "maindirect") {
        clear_entities();
        clear_fx();
        place_player(sx, sy);
        r.frames(2);
        r.probe("pvpdamage", &[&M::calculate_damage_players_take_in_pvp(50, 10), &M::calculate_damage_players_take_in_pvp(5, 40)]);
        r.probe("reversegravity", &[&M::reverse_gravity_support__rectangle(rect(100, 200, 30, 40))]);
        (*m).invasion_type = 1;
        (*m).invasion_size = 130;
        M::fake_load_invasion_start();
        r.probe("invasionstart", &[&(*m).invasion_size_start]);
        (*m).invasion_type = 0;
        (*m).invasion_size = 0;
        (*m).invasion_size_start = 0;
        inv_set(49, 73, 80);
        npc_near(18, 60.0, 0.0);
        terraria::player::set_talk_npc__int_2(p_, find_npc(18));
        (*p_).stat_life = 60;
        let cost = M::get_nurse_heal_cost();
        let (mut cc, mut ci) = (Color::WHITE, cost);
        let t = M::get_coin_value_text_nurse(&mut cc, &mut ci);
        r.probe("nurse", &[&cost, &t, &cc]);
        M::npc_chat_text_do_nurse_heal(cost);
        r.probe("nurseheal", &[&(*p_).stat_life, &(*m).npc_chat_text]);
        npc_near(441, -60.0, 0.0);
        terraria::player::set_talk_npc__int_2(p_, find_npc(441));
        (*p_).tax_money = 123456;
        let (mut tc, mut ti) = (Color::WHITE, 123456i32);
        let t = M::get_coin_value_text_tax_collector(&mut tc, &mut ti);
        r.probe("taxtext", &[&t, &tc]);
        M::npc_chat_text_do_tax_collector();
        r.probe("tax", &[&(*p_).tax_money, &(*m).npc_chat_text]);
        npc_near(369, 90.0, 0.0);
        terraria::player::set_talk_npc__int_2(p_, find_npc(369));
        let quest = (*m).angler_quest;
        inv_set(10, *(*m).angler_quest_item_net_i_ds.at(quest), 1);
        M::npc_chat_text_do_angler_quest();
        r.probe("angler", &[&(*m).angler_quest_finished, &(*m).npc_chat_text]);
        (*m).angler_quest_finished = false;
        npc_near(207, -90.0, 0.0);
        terraria::player::set_talk_npc__int_2(p_, find_npc(207));
        let exotic = (*terraria::id::item_id::sets::s()).exotic_plants_for_dye_trade;
        let plant = (0..exotic.len()).find(|&k| *exotic.at(k)).unwrap_or(-1);
        inv_set(11, plant, 1);
        M::npc_chat_text_dye_trader_rare_plant();
        r.probe("dyetrader", &[&plant, &(*m).npc_chat_text]);
        terraria::player::set_talk_npc__int_2(p_, -1);
        (*m).npc_chat_text = s!("");
        let reforge = cover_new_item(3507);
        (*m).reforge_item = reforge;
        M::reforge_item_in_reforge_slot();
        r.probe("reforge", &[&(*reforge.ptr()).prefix]);
        (*m).reforge_item = cover_new_item(0);
        (*m).reforge_cooldown = 0;
        for k in 0..58 {
            inv_set(k, 0, 0);
        }
        clear_items();
        clear_player_buffs();
        (*p_).stat_life = 100;
        (*p_).tax_money = 0;
        probe_fx(r, "maindirect");
        r.state.push_str(&state_line("maindirect"));
        r.frames(3);
        r.record("maindirect", 1);
        clear_entities();
        clear_fx();
        r.frames(2);
    }
    // ---- Main draws called outside the frame
    if probe_group(r, "maindraw") {
        clear_entities();
        clear_fx();
        place_player(sx, sy);
        r.frames(2);
        let pp = (*p_).base.position;
        let z = npc_near(3, 80.0, 0.0);
        let qs = npc_near(657, -120.0, -40.0);
        let guard = gallery_new_proj(pp.x - 40.0, pp.y - 20.0, 0.0, 0.0, 623);
        (*p_).last_death_postion = v2(pp.x + 100.0, pp.y);
        (*m).guide_item = cover_new_item(9);
        let screen = (*m).screen_position;
        let sb = (*m).sprite_batch;
        r.direct("maindraw", |r| {
            in_batch(|| {
                M::draw_guide_craft_text(100, 300);
                let dm = M::draw_player_death_marker(300.0, 200.0, 2.0, 0.0, 0.0, 1.0, me, 255);
                r.probe("deathmarker", &[&dm]);
                M::draw_npc_map_icons3(300.0, 200.0, 2.0, 255, 1.0);
                M::draw_interface_healthbar_worm(inst, z, 15, 1.0);
                M::draw_npc_direct_queen_slime_wings(inst, qs, sb, screen, npc_center(qs), Color::WHITE);
                M::draw_paladins_shield_boundary(inst, pp, pp + v2(200.0, -60.0));
                M::draw_proj_stardust_guardian_punching(inst, proj_at(guard));
                M::draw_surface_bg_good_evil_desert(inst, 0.0, 1.0, 0, 0);
                let ts = M::draw_torch_biome_swap_icon(inst, 400, 300, false, 0);
                r.probe("torchswap", &[&ts]);
                (*m).wof_npc_index = (*z.ptr()).base.who_am_i;
                M::draw_wof_tongue_to_player(me);
                (*m).wof_npc_index = -1;
            })
        });
        let tb = (*m).tile_batch.ptr();
        r.direct("maindraw_surface", |_| {
            terraria::graphics::tile_batch::begin__void(tb);
            M::draw_simple_surface_background(inst, pp - v2(200.0, 150.0), 400, 300);
            terraria::graphics::tile_batch::end(tb);
        });
        (*m).guide_item = cover_new_item(0);
        r.state.push_str(&state_line("maindraw"));
        clear_entities();
        clear_fx();
        r.frames(2);
    }
    // ---- ItemSlot, ChestUI, ItemSorting
    if probe_group(r, "itemslot") {
        clear_entities();
        clear_fx();
        place_player(x0 + 2, gy - 1);
        r.frames(2);
        r.probe("trustypal", &[&IS::get_trusty_pal_glow_color()]);
        let (mut ok, mut slot) = (false, 0i32);
        let back = IS::armor_swap(cover_new_item(90), &mut ok, &mut slot);
        r.probe("armorswap", &[&ok, &slot, &(*back.ptr()).type_]);
        let dye = cover_new_item(1007);
        let (mut ok, mut slot) = (false, 0i32);
        let back = IS::dye_swap(dye, &mut ok, &mut slot);
        r.probe("dyeswap", &[&ok, &slot, &(*back.ptr()).type_]);
        inv_set(0, 3507, 1);
        let inv = (*p_).inventory;
        r.probe("gamepad", &[&IS::get_gamepad_instructions__item_arr_int_int(inv, 0, 0)]);
        let shop = *(*inst).shop.at(1);
        let shop_items = (*shop.ptr()).item;
        terraria::item::set_defaults((*shop_items.at(0)).ptr(), 8, Gc::NULL);
        inv_set(49, 73, 10);
        M::set_npc_shop(1);
        (*m).mouse_left = true;
        (*m).stack_split = 0;
        IS::handle_shop_slot(shop_items, 0, false, true);
        r.probe("shopbuy", &[&(*(*m).mouse_item.ptr()).type_, &(*(*m).mouse_item.ptr()).stack]);
        (*m).mouse_left = false;
        M::set_npc_shop(0);
        terraria::item::turn_to_air((*m).mouse_item.ptr());
        terraria::item::turn_to_air((*shop_items.at(0)).ptr());
        let co = (*m).cursor_override;
        (*m).cursor_override = 3;
        let oc = IS::override_left_click(inv, 0, 0);
        r.probe("overrideclick", &[&oc, &(*(*inv.at(0)).ptr()).favorited]);
        (*m).cursor_override = co;
        IS::pickup_item_into_mouse(inv, 0, 0, pl);
        r.probe("pickup", &[&(*(*m).mouse_item.ptr()).type_, &(*(*m).mouse_item.ptr()).stack]);
        terraria::item::turn_to_air((*m).mouse_item.ptr());
        let armor = (*p_).armor;
        terraria::item::set_defaults((*armor.at(10)).ptr(), 90, Gc::NULL);
        IS::swap_vanity_equip(armor, 8, 10, pl);
        r.probe("vanityswap", &[&(*(*armor.at(0)).ptr()).type_, &(*(*armor.at(10)).ptr()).type_]);
        inv_set(1, 3318, 1);
        IS::try_open_container(inv, 0, 1, pl);
        r.probe("container", &[&(*(*inv.at(1)).ptr()).type_, &(*(*inv.at(1)).ptr()).stack]);
        let sb = (*m).sprite_batch;
        r.direct("itemslot_money", |_| {
            in_batch(|| IS::draw_money(sb, s!("Savings"), 500.0, 300.0, Arr::<i32>::from_slice(&[12, 34, 5, 1]), false, false))
        });
        let cx = x0 + 6;
        place_tile(cx, gy - 1, 21, 0);
        let ci = terraria::chest::find_chest(cx, gy - 2);
        r.probe("chest", &[&ci]);
        if ci >= 0 {
            let chest = *(*m).chest.at(ci);
            let citems = (*chest.ptr()).item;
            terraria::item::set_defaults((*citems.at(3)).ptr(), 9, Gc::NULL);
            (*(*citems.at(3)).ptr()).stack = 5;
            terraria::item::set_defaults((*citems.at(0)).ptr(), 2, Gc::NULL);
            (*(*citems.at(0)).ptr()).stack = 3;
            inv_set(12, 9, 20);
            terraria::ui::chest_ui::deposit_all_into_local_chest(pl, chest, 12);
            (*p_).chest = ci;
            inv_set(13, 2, 7);
            r.probe("tryplace", &[&terraria::ui::chest_ui::try_placing_in_chest(inv, 13, false, 0)]);
            terraria::ui::chest_ui::scroll(1);
            terraria::ui::chest_ui::scroll(-1);
            terraria::ui::item_sorting::sort_inventory__chest_bool_bool(chest, false, false);
            let it = |k: i32| *citems.at(k);
            r.probe("chestsorted", &[&(*it(0).ptr()).type_, &(*it(0).ptr()).stack, &(*it(1).ptr()).type_, &(*it(1).ptr()).stack]);
            (*p_).chest = -1;
            for k in 0..citems.len() {
                terraria::item::turn_to_air((*citems.at(k)).ptr());
            }
        }
        inv_set(20, 40, 50);
        inv_set(21, 97, 30);
        terraria::ui::item_sorting::fill_ammo_from_inventory();
        r.probe("ammo", &[&(*(*inv.at(54)).ptr()).type_, &(*(*inv.at(55)).ptr()).type_]);
        for k in 0..58 {
            inv_set(k, 0, 0);
        }
        for k in 0..armor.len() {
            terraria::item::turn_to_air((*armor.at(k)).ptr());
        }
        let dyes = (*p_).dye;
        for k in 0..dyes.len() {
            terraria::item::turn_to_air((*dyes.at(k)).ptr());
        }
        clear_items();
        r.state.push_str(&state_line("itemslot"));
        r.frames(3);
        r.record("itemslot", 1);
        clear_entities();
        clear_fx();
        clear_area(x0, x0 + 40, gy);
        r.frames(2);
    }
}
