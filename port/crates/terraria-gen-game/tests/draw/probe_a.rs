// The probe run, part A: tools/refdump_draw/ProbeA.cs, step for step. Included by tests/draw.rs.

/// ProbeA.cs `PPlayerStats`.
unsafe fn probe_player_stats(r: &mut Run, key: &str) {
    let p = &*local_player().ptr();
    r.probe(
        key,
        &[
            &p.stat_defense, &p.melee_damage, &p.ranged_damage, &p.magic_damage, &p.minion_damage, &p.melee_crit, &p.ranged_crit, &p.magic_crit, &p.endurance,
            &p.move_speed, &p.melee_speed, &p.whip_range_multiplier, &p.whip_use_time_multiplier, &p.max_minions, &p.mana_cost, &p.life_regen, &p.stat_life_max2,
            &p.stat_mana_max2, &p.dash_type, &p.thorns, &p.pick_speed, &p.armor_penetration,
        ],
    );
}

/// ProbeA.cs `TownTypes`: the same rule.
unsafe fn probe_town_types() -> Vec<i32> {
    let n = terraria::npc::new();
    let pet = (*terraria::id::npcid::sets::s()).is_town_pet;
    let mut list = Vec::new();
    for t in 1..GALLERY_NPC_COUNT {
        terraria::npc::set_defaults(n.ptr(), t, rt::zero());
        if (*n.ptr()).town_npc && !*pet.at(t) {
            list.push(t);
        }
    }
    list
}

/// ProbeA.cs `ProbeA`.
unsafe fn probe_a(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    let m = terraria::main::s();
    let pl = local_player();
    // ---- every armour set worn
    if probe_group(r, "armor") {
        let all = (*terraria::data_structures::armor_set_bonuses::s()).all;
        let n = (*all.ptr()).count();
        r.note(format!("armour sets: {n}"));
        place_player(sx, sy);
        for k in 0..n {
            let s = (*all.ptr()).get_item(k);
            set_armor(0, (*s.ptr()).head);
            set_armor(1, (*s.ptr()).body);
            set_armor(2, (*s.ptr()).legs);
            r.frames(2);
            probe_player_stats(r, &format!("armor_{k}"));
            if k % 8 == 0 {
                r.record(&format!("armor_{k}"), 1);
            }
        }
        for k in 0..3 {
            set_armor(k, 0);
        }
        r.frames(3);
    }
    // ---- town NPCs asked for their prices, in nine biomes
    if probe_group(r, "shop") {
        let town = probe_town_types();
        r.note(format!("town NPCs: {}", town.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(",")));
        let shop = (*m).shop_helper;
        for (name, bx, by) in BIOMES {
            clear_entities();
            place_player(bx, by);
            let mut ids = Vec::new();
            for (k, &ty) in town.iter().enumerate() {
                let k = k as i32;
                let tx = bx + if k < 8 { k - 4 } else { (k - 8) * 3 - 30 };
                let id = new_npc_at(tx * 16 + 8, by * 16, ty);
                let n = npc_at(id).ptr();
                (*n).homeless = false;
                (*n).home_tile_x = tx;
                (*n).home_tile_y = by;
                ids.push(id);
            }
            r.frames(4);
            for id in ids {
                let n = npc_at(id);
                let res = terraria::game_content::shop_helper::get_shopping_settings(shop.ptr(), pl, n);
                r.probe(&format!("shop_{name}_{}", (*n.ptr()).type_), &[&res.price_adjustment, &res.happiness_report]);
            }
            r.record(&format!("shop_{name}"), 1);
        }
        clear_entities();
        place_player(sx, sy);
        r.frames(5);
    }
    // ---- a desert floating island
    if probe_group(r, "island") {
        let (ix, iy) = (sx - 300, 160);
        r.probe_tiles("island_before", ix - 200, iy - 80, 400, 200);
        terraria::world_gen::desert_cloud_island(ix, iy);
        r.probe_tiles("island_after", ix - 200, iy - 80, 400, 200);
        place_player(ix, iy - 20);
        r.frames(10);
        r.record("island", 1);
        place_player(sx, sy);
        r.frames(10);
    }
}
