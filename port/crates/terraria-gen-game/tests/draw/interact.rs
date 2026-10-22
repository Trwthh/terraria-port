// The interaction run (`DRAW_MODE=interact`, refdata/interact): tools/refdump_draw/Interact.cs, step for step.
// Included by tests/draw.rs.

/// Interact.cs `HeldAiStyles`.
const HELD_AI_STYLES: [i32; 13] = [15, 19, 20, 75, 99, 140, 142, 160, 161, 165, 182, 190, 191];

/// Interact.cs `InteractItems`: the same rule.
unsafe fn interact_items() -> Vec<i32> {
    let it = terraria::item::new__void();
    let pr = terraria::projectile::new();
    let sets = terraria::id::item_id::sets::s();
    let (whip, kite, yoyo) = ((*sets).summoner_weapon_that_scales_with_attack_speed, (*sets).is_a_kite, (*sets).yoyo);
    let count = (*terraria::id::item_id::s()).count as i32;
    let mut list = Vec::new();
    for t in 1..count {
        terraria::item::set_defaults(it.ptr(), t, Gc::NULL);
        if (*it.ptr()).use_style == 0 {
            continue;
        }
        let mut take = *whip.at(t) || *kite.at(t) || *yoyo.at(t) || (*it.ptr()).fishing_pole > 0;
        let shoot = (*it.ptr()).shoot;
        if !take && shoot > 0 && shoot < GALLERY_PROJ_COUNT {
            terraria::projectile::set_defaults(pr.ptr(), shoot);
            take = HELD_AI_STYLES.contains(&(*pr.ptr()).ai_style);
        }
        if take {
            list.push(t);
        }
    }
    list
}

/// Interact.cs `SetInv`.
unsafe fn set_inv(slot: i32, ty: i32, stack: i32) {
    give_item(slot, ty);
    (*(*(*local_player().ptr()).inventory.at(slot)).ptr()).stack = stack;
}
/// Interact.cs `SetArmor`.
unsafe fn set_armor(slot: i32, ty: i32) {
    terraria::item::set_defaults((*(*local_player().ptr()).armor.at(slot)).ptr(), ty, Gc::NULL);
}
/// Interact.cs `PlaceTile`.
unsafe fn place_tile(x: i32, y: i32, ty: i32, style: i32) -> bool {
    terraria::world_gen::place_tile(x, y, ty, true, true, -1, style)
}
/// Interact.cs `KillTile`.
unsafe fn kill_tile(x: i32, y: i32) {
    terraria::world_gen::kill_tile(x, y, false, false, true);
}
/// Is the tile active (sTileHeader bit 5, as the reference's snapshot reads it)?
unsafe fn tile_active(x: i32, y: i32) -> bool {
    let t = gallery_tile(x, y);
    !t.is_null() && ((*t.ptr()).s_tile_header & 0x20) != 0
}
unsafe fn tile_liquid(x: i32, y: i32) -> u8 {
    let t = gallery_tile(x, y);
    if t.is_null() {
        0
    } else {
        (*t.ptr()).liquid
    }
}
/// Interact.cs `GroundBelow`.
unsafe fn ground_below(x: i32, y0: i32) -> i32 {
    let h = (*terraria::main::s()).max_tiles_y;
    (y0..h - 10).find(|&y| tile_active(x, y)).unwrap_or(-1)
}
/// `Item.NewItem(new EntitySource_DebugCommand(), x, y, 16, 16, type, stack, false, 0, NewItemOwnership 0, null, null)`
unsafe fn interact_new_item(x: i32, y: i32, ty: i32, stack: i32) {
    let _ = terraria::item::new_item__i_entity_source_int_int_int_int_int_int_bool_int_new_item_ownership_nullable_vector2_new_item_modifier(
        terraria::data_structures::entity_source_debug_command::new().up::<rt::Object>(),
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
    );
}

/// Interact.cs `InteractWorld`.
unsafe fn interact_world(r: &mut Run) {
    let m = terraria::main::s();
    let (sx, sy) = ((*m).spawn_tile_x, (*m).spawn_tile_y);
    let pl = local_player();
    set_time(true, 27000.0);
    place_player(sx, sy);
    (*pl.ptr()).stat_mana_max = 200;
    (*pl.ptr()).stat_mana = 200;
    set_inv(54, 40, 999);
    set_inv(55, 97, 999);
    set_inv(56, 2676, 99);
    r.frames(30);
    r.press(KEY_D1);
    let items = interact_items();
    let s = format!("use gallery: {} items: {}", items.len(), items.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","));
    r.note(s);
    for t in items {
        clear_entities();
        place_player(sx, sy);
        gallery_immune();
        set_inv(0, t, 1);
        r.input.x = 900;
        r.input.y = 300;
        r.input.left = true;
        r.frames(20);
        r.record(&format!("use_{t}"), 1);
        r.input.left = false;
        r.frames(4);
    }
    r.input.x = 640;
    r.input.y = 360;
    clear_entities();
    place_player(sx, sy);
    r.frames(20);
    interact_built(r, sx, sy);
    interact_player(r, sx, sy);
}

const BUILD_DX: i32 = 8;
const BUILD_W: i32 = 70;

/// Interact.cs `InteractBuilt`.
unsafe fn interact_built(r: &mut Run, sx: i32, sy: i32) {
    let m = terraria::main::s();
    let pl = local_player();
    let x0 = sx + BUILD_DX;
    let gy = ground_below(x0, sy - 12);
    r.note(format!("build ground at {x0},{gy}"));
    for x in x0 - 2..=x0 + BUILD_W {
        for y in gy - 10..gy {
            kill_tile(x, y);
        }
        for y in gy..=gy + 1 {
            place_tile(x, y, 0, 0);
        }
    }
    r.frames(5);
    let devices: [[i32; 3]; 15] = [
        [136, 0, 0],
        [4, 0, 2],
        [93, 0, 4],
        [137, 0, 6],
        [105, 2, 8],
        [144, 0, 11],
        [207, 0, 13],
        [216, 0, 16],
        [335, 0, 18],
        [338, 0, 21],
        [405, 0, 23],
        [244, 0, 27],
        [455, 0, 31],
        [565, 0, 35],
        [130, 0, 38],
    ];
    for d in devices {
        let ok = place_tile(x0 + d[2], gy - 1, d[0], d[1]);
        r.note(format!("place {} at +{}: {}", d[0], d[2], if ok { "True" } else { "False" }));
    }
    for x in x0..=x0 + 39 {
        let _ = terraria::world_gen::place_wire(x, gy - 1);
    }
    place_player(x0 - 1, gy - 1);
    r.frames(10);
    terraria::wiring::hit_switch(x0, gy - 1);
    r.frames(30);
    r.record("wired_on", 2);
    terraria::wiring::hit_switch(x0, gy - 1);
    r.frames(30);
    r.record("wired_off", 2);
    clear_entities();
    {
        let chests = (*m).chest;
        let mut ci = -1i32;
        for (i, c) in chests.as_slice().iter().enumerate() {
            if !c.is_null() && (*c.ptr()).y < 300 {
                ci = i as i32;
                break;
            }
        }
        let ch = *chests.at(ci);
        let citems = (*ch.ptr()).item;
        let mut k = 0;
        for it in citems.as_slice() {
            if k >= 5 {
                break;
            }
            if it.is_null() || (*it.ptr()).type_ == 0 {
                continue;
            }
            set_inv(10 + k, (*it.ptr()).type_, 1);
            k += 1;
        }
        set_inv(20, 73, 7);
        place_player((*ch.ptr()).x, (*ch.ptr()).y + 2);
        r.frames(10);
        terraria::player::open_chest(pl.ptr(), (*ch.ptr()).x, (*ch.ptr()).y, ci);
        r.frames(5);
        terraria::ui::chest_ui::quick_stack(false);
        r.frames(5);
        r.record("chest_quickstack", 1);
        terraria::ui::chest_ui::loot_all();
        r.frames(5);
        r.record("chest_lootall", 1);
        terraria::ui::chest_ui::deposit_all();
        r.frames(5);
        r.record("chest_depositall", 1);
        terraria::ui::chest_ui::restock();
        r.frames(5);
        r.record("chest_restock", 1);
        (*pl.ptr()).chest = -1;
        (*m).player_inventory = false;
        r.frames(5);
    }
    let cx = x0 + 50;
    for d in 1..=3 {
        clear_entities();
        place_player(cx - 6, gy - 1);
        gallery_immune();
        terraria::game_content::events::dd2_event::start_invasion(d);
        new_npc_at(cx * 16 + 8, gy * 16, 548);
        terraria::game_content::events::dd2_event::drop_starter_crystals();
        r.frames(420);
        let s = format!("dd2 difficulty {d}: ongoing {}", if (*terraria::game_content::events::dd2_event::s()).ongoing { "True" } else { "False" });
        r.note(s);
        r.record(&format!("dd2_{d}"), 1);
        terraria::game_content::events::dd2_event::stop_invasion(false);
        r.frames(10);
    }
    clear_entities();
    {
        let px = x0 + 58;
        for x in px..px + 6 {
            for y in gy..=gy + 1 {
                kill_tile(x, y);
                let t = gallery_tile(x, y);
                (*t.ptr()).liquid = 255;
                terraria::tile::liquid_type__int(t.ptr(), 3);
            }
        }
        for x in px - 1..=px + 6 {
            place_tile(x, gy + 2, 0, 0);
        }
        place_tile(px - 1, gy + 1, 0, 0);
        place_tile(px + 6, gy + 1, 0, 0);
        for ty in [9, 3, 29, 75] {
            interact_new_item((px + 2) * 16, gy * 16, ty, 3);
        }
        gallery_new_proj(((px + 1) * 16) as f32, ((gy - 3) * 16) as f32, 0.0, 3.0, 1);
        place_player(px - 3, gy - 1);
        r.frames(60);
        r.record("shimmer_items", 1);
        place_player(px + 2, gy);
        r.frames(90);
        r.record("shimmer_player", 1);
        place_player(x0, gy - 1);
        r.frames(60);
    }
    {
        let chx = x0 + 44;
        let ok = place_tile(chx, gy - 1, 15, 0);
        r.note(format!("chair: {}", if ok { "True" } else { "False" }));
        place_player(chx + 1, gy - 1);
        r.frames(5);
        terraria::game_content::player_sitting_helper::sit_down(&mut (*pl.ptr()).sitting, pl, chx, gy - 1);
        r.frames(20);
        r.record("player_sit", 1);
        terraria::game_content::player_sitting_helper::sit_up(&mut (*pl.ptr()).sitting, pl, true);
        r.frames(10);
    }
    {
        for n in (*m).npc.as_slice() {
            let n = n.ptr();
            if (*n).active && (*n).type_ == 22 {
                (*n).homeless = false;
                (*n).home_tile_x = sx;
                (*n).home_tile_y = sy;
                break;
            }
        }
        set_time(true, 27000.0);
        terraria::world_gen::spawn_travel_npc();
        r.frames(30);
        r.record("npc_travel", 1);
        (*terraria::world_gen::s()).prioritized_town_npc_type = 17;
        terraria::world_gen::spawn_homeless_npc();
        r.frames(30);
        r.record("npc_homeless", 1);
    }
    place_player(sx, sy);
    r.frames(20);
}

/// Interact.cs `InteractPlayer`.
unsafe fn interact_player(r: &mut Run, sx: i32, sy: i32) {
    let m = terraria::main::s();
    let pl = local_player();
    set_armor(3, 3016);
    r.frames(5);
    for _ in 0..30 {
        (*pl.ptr()).immune = false;
        (*pl.ptr()).immune_time = 0;
        (*pl.ptr()).stat_life = 100;
        let _ = terraria::player::hurt(pl.ptr(), terraria::data_structures::player_death_reason::legacy_default(), 1, 1, false, false, false, -1, true);
        r.frames(2);
    }
    r.record("player_dodge", 1);
    set_armor(3, 0);
    {
        let hook = *(*pl.ptr()).misc_equips.at(4);
        terraria::item::set_defaults(hook.ptr(), 84, Gc::NULL);
        r.input.x = 800;
        r.input.y = 200;
        r.frames(2);
        terraria::player::quick_grapple(pl.ptr());
        r.frames(30);
        r.record("player_grapple", 1);
        r.input.x = 640;
        r.input.y = 360;
        clear_entities();
        place_player(sx, sy);
        r.frames(20);
    }
    set_armor(4, 128);
    r.input.keys = vec![KEY_SPACE];
    r.frames(60);
    r.record("player_rocketboots", 1);
    r.input.keys.clear();
    r.frames(60);
    set_armor(4, 0);
    place_player(sx, sy);
    for set in [[231, 232, 233], [2763, 2764, 2765]] {
        for k in 0..3 {
            set_armor(k as i32, set[k]);
        }
        r.frames(20);
        r.record(&format!("player_armor_{}", set[0]), 1);
    }
    for k in 0..3 {
        set_armor(k, 0);
    }
    (*pl.ptr()).ghost = true;
    r.frames(30);
    r.record("player_ghost", 1);
    (*pl.ptr()).ghost = false;
    place_player(sx, sy);
    r.frames(10);
    set_time(true, 54000.0 - 60.0);
    r.frames(90);
    r.record("time_dusk", 1);
    set_time(false, 32400.0 - 60.0);
    r.frames(90);
    r.record("time_dawn", 1);
    set_time(true, 27000.0);
    terraria::player::add_buff(pl.ptr(), 1, 3600, false);
    terraria::player::add_buff(pl.ptr(), 2, 3600, false);
    r.input.x = 40;
    r.input.y = 90;
    r.frames(10);
    r.record("ui_bufftip", 1);
    r.input.x = 640;
    r.input.y = 360;
    r.frames(5);
    {
        let mgr = (*m).resource_sets_manager;
        for k in 0..6 {
            terraria::game_content::ui::resource_sets::player_resource_sets_manager::cycle_resource_set(mgr.ptr());
            r.frames(5);
            r.record(&format!("ui_resources_{k}"), 1);
        }
    }
    {
        let p = (*pl.ptr()).base.position;
        interact_new_item(p.x as i32, p.y as i32, 73, 5);
        r.frames(20);
        r.record("ui_pickup", 1);
    }
    set_inv(0, 36, 5);
    r.frames(2);
    let (fx, fy) = feet_tile();
    r.aim(((fx + 2) * 16 + 8) as f32, (fy * 16 - 8) as f32);
    r.frames(5);
    r.record("ui_placepreview", 1);
    r.input.x = 640;
    r.input.y = 360;
    {
        let h = (*m).max_tiles_y;
        let (mut bx, mut by) = (-1, -1);
        let mut water = false;
        for x in 60..700 {
            if bx >= 0 {
                break;
            }
            let mut y = 50;
            while y < h - 50 && !tile_active(x, y) && tile_liquid(x, y) == 0 {
                y += 1;
            }
            let wet = tile_liquid(x, y) != 0;
            if wet {
                water = true;
            } else if water {
                bx = x;
                by = y;
            }
        }
        r.note(format!("beach at {bx},{by}"));
        set_inv(0, 2289, 1);
        place_player(bx + 1, by - 1);
        r.frames(10);
        r.aim(((bx - 8) * 16) as f32, ((by - 1) * 16) as f32);
        r.click();
        r.frames(400);
        r.record("fishing_wait", 1);
        r.click();
        r.frames(30);
        r.record("fishing_reel", 1);
        r.input.x = 640;
        r.input.y = 360;
        clear_entities();
    }
    place_player(sx, sy);
    r.frames(20);
    interact_player2(r, sx, sy);
}

/// Interact.cs `InteractPlayer2`.
unsafe fn interact_player2(r: &mut Run, sx: i32, sy: i32) {
    let m = terraria::main::s();
    let pl = local_player();
    for w in [5586, 5659, 5686] {
        place_player(sx, sy);
        set_armor(3, w);
        r.frames(5);
        r.input.keys = vec![KEY_SPACE];
        r.frames(40);
        r.record(&format!("player_wings_{w}"), 1);
        r.input.keys.clear();
        r.frames(60);
    }
    set_armor(3, 963);
    place_player(sx, sy);
    r.frames(5);
    for _ in 0..40 {
        (*pl.ptr()).immune = false;
        (*pl.ptr()).immune_time = 0;
        (*pl.ptr()).stat_life = 100;
        let _ = terraria::player::hurt(pl.ptr(), terraria::data_structures::player_death_reason::legacy_default(), 1, 1, false, false, false, -1, true);
        r.frames(2);
    }
    r.record("player_blackbelt", 1);
    set_armor(3, 3580);
    r.frames(20);
    r.record("player_yoraiz0r", 1);
    set_armor(3, 0);
    set_armor(4, 128);
    place_player(sx, sy);
    r.frames(5);
    r.input.keys = vec![KEY_SPACE];
    r.frames(8);
    r.input.keys.clear();
    r.frames(4);
    r.input.keys = vec![KEY_SPACE];
    r.frames(50);
    r.record("player_rockets", 1);
    r.input.keys.clear();
    r.frames(90);
    set_armor(4, 0);
    place_player(sx, sy);
    set_inv(11, 28, 5);
    set_inv(12, 110, 5);
    set_inv(13, 292, 3);
    set_inv(14, 290, 3);
    (*pl.ptr()).stat_life = 40;
    (*pl.ptr()).stat_mana = 10;
    r.frames(2);
    terraria::player::quick_heal(pl.ptr());
    terraria::player::quick_mana(pl.ptr());
    terraria::player::quick_buff(pl.ptr());
    r.frames(20);
    r.record("player_quick", 1);
    {
        let saddle = *(*pl.ptr()).misc_equips.at(3);
        terraria::item::set_defaults(saddle.ptr(), 2430, Gc::NULL);
        terraria::player::quick_mount(pl.ptr());
        r.frames(30);
        r.record("player_quickmount", 1);
        terraria::player::quick_mount(pl.ptr());
        r.frames(10);
    }
    {
        let p = (*pl.ptr()).base.position;
        let a = new_npc_at(p.x as i32 + 40, p.y as i32 + 40, 369);
        let angler = *(*m).npc.at(a);
        terraria::player::get_angler_reward(pl.ptr(), angler, 2450);
        r.frames(30);
        r.record("player_angler", 1);
        clear_entities();
    }
    for mt in [7, 8] {
        place_player(sx, sy);
        let mount = (*pl.ptr()).mount;
        terraria::mount::set_mount(mount.ptr(), mt, pl);
        r.frames(10);
        r.input.x = 800;
        r.input.y = 420;
        r.input.left = true;
        r.frames(40);
        r.record(&format!("player_mountability_{mt}"), 1);
        r.input.left = false;
        r.input.x = 640;
        r.input.y = 360;
        terraria::mount::dismount(mount.ptr(), pl, false);
        r.frames(10);
    }
    clear_entities();
    place_player(sx, sy);
    r.frames(20);
}
