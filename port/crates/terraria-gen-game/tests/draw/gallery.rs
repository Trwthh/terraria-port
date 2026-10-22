// The gallery run (`DRAW_MODE=gallery`, refdata/gallery): tools/refdump_draw/Gallery.cs, step for step. Included by
// tests/draw.rs (`include!`), so it shares its `Run` and helpers.

/// Gallery.cs `ProjCount` / `NpcCount` (ProjectileID.Count / NPCID.Count of 1.4.5.8).
const GALLERY_PROJ_COUNT: i32 = 1136;
const GALLERY_NPC_COUNT: i32 = 697;
const GALLERY_PROJ_BATCH: i32 = 30;
const GALLERY_NPC_BATCH: i32 = 20;
/// ParticleOrchestraType.Count
const PARTICLE_COUNT: i32 = 83;
/// ParticleOrchestraType.ItemTransfer
const PARTICLE_ITEM_TRANSFER: i32 = 24;
/// SkyEntityType, in order
const SKY_ENTITY_TYPES: [&str; 19] = [
    "birdsv", "wyvern", "airship", "airballoon", "eyeball", "meteor", "boneserpent", "bats", "butterflies", "lostkite", "vulture", "pixieposse", "seagulls",
    "slimeballoons", "gastropods", "pegasus", "eaterofsouls", "crimera", "hellbats",
];

/// Gallery.cs `GallerySkip`: `DRAW_GALLERY_SKIP=bg,particles,ambient,proj,npc` (diagnosis of a later part only).
fn gallery_skip(part: &str) -> bool {
    std::env::var("DRAW_GALLERY_SKIP").is_ok_and(|v| v.split(',').any(|p| p == part))
}

/// Gallery.cs `Immune`.
unsafe fn gallery_immune() {
    let pl = local_player().ptr();
    (*pl).immune = true;
    (*pl).immune_time = 100000;
}

/// Gallery.cs `NewProj` (RefDumpDraw.cs): damage 10, knockback 1, owner the local player.
unsafe fn gallery_new_proj(x: f32, y: f32, vx: f32, vy: f32, ty: i32) -> i32 {
    terraria::projectile::new_projectile__i_entity_source_vector2_vector2_int_int_float_int_float_float_float_new_projectile_modifier(
        terraria::data_structures::entity_source_debug_command::new().up::<rt::Object>(),
        terraria_core::xna::Vector2 { x, y },
        terraria_core::xna::Vector2 { x: vx, y: vy },
        ty,
        10,
        1.0,
        (*terraria::main::s()).my_player,
        0.0,
        0.0,
        0.0,
        rt::zero::<rt::Delegate<dyn Fn(Gc<terraria::projectile::Projectile>)>>(),
    )
}

/// Gallery.cs `SpawnParticle`: `ParticleOrchestrator.RequestParticleSpawn(clientOnly: true, type, settings, null)`.
unsafe fn gallery_particle(ty: i32, x: f32, y: f32, vx: f32, vy: f32, unique: i32) {
    let settings = terraria::game_content::drawing::particle_orchestra_settings::ParticleOrchestraSettings {
        position_in_world: terraria_core::xna::Vector2 { x, y },
        movement_vector: terraria_core::xna::Vector2 { x: vx, y: vy },
        unique_info_piece: unique,
        index_of_player_who_invoked_this: 0,
    };
    terraria::game_content::drawing::particle_orchestrator::request_particle_spawn(true, ty as u8, settings, rt::zero::<Nullable<i32>>());
}

/// Gallery.cs `TileAt`.
unsafe fn gallery_tile(x: i32, y: i32) -> Gc<terraria::tile::Tile> {
    *(*terraria::main::s()).tile.at(x, y)
}

/// Gallery.cs `GalleryWorld`.
unsafe fn gallery_world(r: &mut Run) {
    let m = terraria::main::s();
    let (sx, sy) = ((*m).spawn_tile_x, (*m).spawn_tile_y);
    let pl = local_player();
    assert_eq!(((*terraria::id::projectile_id::s()).count as i32, (*terraria::id::npcid::s()).count as i32), (GALLERY_PROJ_COUNT, GALLERY_NPC_COUNT), "ProjectileID.Count / NPCID.Count differ from the gallery's");
    set_time(true, 27000.0);
    place_player(sx, sy);
    r.frames(30);
    // ---- 1. the old background drawing
    if !gallery_skip("bg") {
    (*m).background_enabled = false;
    for (name, x, y, day) in [
        ("day", sx, sy, true),
        ("night", sx, sy, false),
        ("cave", 1999, 547, true),
        ("desert", 1720, 352, true),
        ("snow", 2499, 298, true),
        ("crimson", 1408, 471, true),
        ("jungle", 309, 340, true),
        ("ocean", 150, 249, true),
        ("underworld", 480, 1009, true),
    ] {
        set_time(day, if day { 27000.0 } else { 16200.0 });
        place_player(x, y);
        r.frames(30);
        r.record(&format!("bgoff_{name}"), 2);
    }
    (*m).background_enabled = true;
    }
    set_time(true, 27000.0);
    place_player(sx, sy);
    r.frames(30);
    // ---- 2. every particle
    let mut b = 0;
    while b * 8 < PARTICLE_COUNT && !gallery_skip("particles") {
        let c = (*pl.ptr()).base.position;
        for k in 0..8 {
            let t = b * 8 + k;
            if t >= PARTICLE_COUNT {
                break;
            }
            gallery_particle(t, c.x - 280.0 + 70.0 * k as f32, c.y - 60.0, 2.0, -1.5, if t == PARTICLE_ITEM_TRANSFER { 4 } else { 0 });
        }
        r.frames(12);
        r.record(&format!("particles_{b}"), 1);
        r.frames(20);
        b += 1;
    }
    // ---- 3. every ambient sky entity
    if !gallery_skip("ambient") {
        let sky = terraria::graphics::effects::effect_manager__custom_sky::get_item(&mut (*(*terraria::graphics::effects::sky_manager::s()).instance.ptr()).base, s!("Ambience"));
        for (t, name) in SKY_ENTITY_TYPES.iter().enumerate() {
            let night = matches!(t, 4 | 6 | 7 | 16 | 17 | 18);
            set_time(!night, if night { 16200.0 } else { 27000.0 });
            terraria::game_content::skies::ambient_sky::spawn(sky.0 as *mut terraria::game_content::skies::ambient_sky::AmbientSky, pl, t as u8, 1000 + t as i32);
            r.frames(60);
            r.record(&format!("ambient_{name}"), 1);
        }
        set_time(true, 27000.0);
        r.frames(30);
    }
    // ---- 4. every projectile type
    let mut b = 0;
    while 1 + b * GALLERY_PROJ_BATCH < GALLERY_PROJ_COUNT && !gallery_skip("proj") {
        clear_entities();
        place_player(sx, sy);
        gallery_immune();
        let c = (*pl.ptr()).base.position;
        for k in 0..GALLERY_PROJ_BATCH {
            let ty = 1 + b * GALLERY_PROJ_BATCH + k;
            if ty >= GALLERY_PROJ_COUNT {
                break;
            }
            gallery_new_proj(c.x - 450.0 + 90.0 * (k % 10) as f32, c.y - 180.0 + 70.0 * (k / 10) as f32, 1.5, -0.5, ty);
        }
        r.frames(8);
        r.record(&format!("proj_{b}"), 1);
        b += 1;
    }
    clear_entities();
    place_player(sx, sy);
    r.frames(20);
    // ---- 5. every NPC type
    let mut b = 0;
    while 1 + b * GALLERY_NPC_BATCH < GALLERY_NPC_COUNT && !gallery_skip("npc") {
        clear_entities();
        place_player(sx, sy);
        gallery_immune();
        let c = (*pl.ptr()).base.position;
        for k in 0..GALLERY_NPC_BATCH {
            let ty = 1 + b * GALLERY_NPC_BATCH + k;
            if ty >= GALLERY_NPC_COUNT {
                break;
            }
            new_npc_at((c.x - 500.0 + 100.0 * (k % 10) as f32) as i32, (c.y - 160.0 + 120.0 * (k / 10) as f32) as i32, ty);
        }
        r.frames(8);
        r.record(&format!("npc_{b}"), 1);
        b += 1;
    }
    clear_entities();
    set_time(true, 27000.0);
    place_player(sx, sy);
    r.frames(30);
    gallery_world_events(r);
    clear_entities();
    r.frames(10);
    r.record("gallery_end", 2);
}

/// Gallery.cs `GalleryWorldEvents`.
unsafe fn gallery_world_events(r: &mut Run) {
    let m = terraria::main::s();
    let (sx, sy) = ((*m).spawn_tile_x, (*m).spawn_tile_y);
    let pl = local_player();
    let _ = terraria::world_gen::meteor(sx + 60, sy, true);
    place_player(sx + 60, sy - 20);
    r.frames(30);
    r.record("event_meteor", 2);
    let (mut tx, mut ty) = (-1, -1);
    'find: for x in sx + 2..sx + 400 {
        for y in sy - 60..sy + 20 {
            let t = gallery_tile(x, y);
            if terraria::tile::active__void(t.ptr()) && (*t.ptr()).type_ == 5 && (*t.ptr()).frame_y < 198 {
                (tx, ty) = (x, y);
                break 'find;
            }
        }
    }
    r.note(format!("tree at {tx},{ty}"));
    place_player(tx - 3, sy);
    terraria::world_gen::shake_tree(tx, ty);
    r.frames(30);
    r.record("event_shaketree", 2);
    (*m).hard_mode = true;
    terraria::world_gen::initialize_hard_mode();
    place_player(sx, sy);
    r.frames(30);
    r.record("event_hardmode", 2);
    let (mut ax, mut ay) = (-1, -1);
    'altar: for x in 50..(*m).max_tiles_x - 50 {
        for y in 50..(*m).max_tiles_y - 50 {
            let t = gallery_tile(x, y);
            if terraria::tile::active__void(t.ptr()) && (*t.ptr()).type_ == 26 {
                (ax, ay) = (x, y);
                break 'altar;
            }
        }
    }
    r.note(format!("altar at {ax},{ay}"));
    place_player(ax, ay);
    r.frames(20);
    terraria::world_gen::smash_altar(ax, ay);
    r.frames(30);
    r.record("event_altar", 2);
    place_player(sx, sy);
    r.frames(20);
    terraria::world_gen::trigger_lunar_apocalypse();
    r.frames(60);
    r.record("event_pillars", 2);
    clear_entities();
    (*m).hard_mode = false;
    (*pl.ptr()).immune = false;
    (*pl.ptr()).immune_time = 0;
    let reason = terraria::data_structures::player_death_reason::by_custom_reason(s!("the gallery"));
    terraria::player::kill_me(pl.ptr(), reason, 1000.0, 1, false);
    r.frames(30);
    r.record("player_dead", 2);
    let mut i = 0;
    while i < 1200 && (*pl.ptr()).dead {
        r.frames(1);
        i += 1;
    }
    r.note(format!("respawned: dead {} life {}", if (*pl.ptr()).dead { "True" } else { "False" }, (*pl.ptr()).stat_life));
    r.frames(30);
    r.record("player_respawn", 2);
}
