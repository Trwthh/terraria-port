//! `Terraria.IO.WorldFile`: moving a parsed world file into the running game state.
//! The file format itself is `terraria_formats::wld`.

use terraria_core::TileMap;
use terraria_formats::wld::World;

use crate::game::Game;
use crate::unported;

/// The state changes of `WorldFile.LoadWorld` / `LoadHeader` / `LoadWorldTiles` (WorldFile.cs:650, 1967, 2534).
///
/// Only the fields the ported code reads so far are applied; the rest of the header stays in `world`.
/// Not done yet: chests, signs, NPCs, tile entities, town manager, bestiary, creative powers,
/// `WorldGen.tileCounts` and the post-load fix-ups other than the liquid settle.
pub fn load_world(g: &mut Game, world: World) -> World {
    let h = &world.header;
    g.main.world_name = h.world_name.clone();
    g.main.left_world = h.left_world as f32;
    g.main.right_world = h.right_world as f32;
    g.main.top_world = h.top_world as f32;
    g.main.bottom_world = h.bottom_world as f32;
    g.main.max_tiles_y = h.max_tiles_y;
    g.main.max_tiles_x = h.max_tiles_x;
    g.main.game_mode = h.flags.game_mode;
    g.main.spawn_tile_x = h.flags.spawn_tile_x;
    g.main.spawn_tile_y = h.flags.spawn_tile_y;
    g.main.world_surface = h.flags.world_surface;
    g.main.rock_layer = h.flags.rock_layer;
    g.main.dungeon_x = h.flags.dungeon_x;
    g.main.dungeon_y = h.flags.dungeon_y;
    g.main.hard_mode = h.flags.hard_mode;
    g.main.remix_world = h.flags.remix_world;
    g.main.tree_x = h.flags.tree_x;
    g.main.tree_style = h.flags.tree_style;
    // `WorldGen.TreeTops.Load` (TreeTopsInfo.cs:50): reads `min(count, 13)` values (version >= 211)
    for (i, v) in h.flags.tree_tops.variations.iter().enumerate().take(13) {
        g.world_gen.tree_tops[i] = *v;
    }
    g.world_gen.hallow_bg = h.flags.hallow_bg as i32;
    // WorldFile.cs:2161-2162: `Main.windSpeedTarget = reader.ReadSingle(); Main.windSpeedCurrent = Main.windSpeedTarget;`
    g.main.wind_speed_current = h.flags.wind_speed_target;
    // WorldFile.LoadHeader (WorldFile.cs:2065-2082): the sky and background values
    g.main.moon_type = h.flags.moon_type as i32;
    for i in 0..3 {
        g.main.cave_back_x[i] = h.flags.cave_back_x[i];
    }
    g.main.cave_back_style = h.flags.cave_back_style;
    g.main.ice_back_style = h.flags.ice_back_style;
    g.main.jungle_back_style = h.flags.jungle_back_style;
    g.main.hell_back_style = h.flags.hell_back_style;
    // WorldFile.LoadHeader (WorldFile.cs:2085-2092) -> `WorldFile.SetOngoingToTemps` (WorldFile.cs:152-162)
    g.main.time = h.flags.temp_time;
    g.main.day_time = h.flags.temp_day_time;
    g.main.moon_phase = h.flags.temp_moon_phase;
    g.main.blood_moon = h.flags.temp_blood_moon;
    g.main.eclipse = h.flags.temp_eclipse;
    g.main.raining = h.flags.temp_raining;
    unported!("Main.rainTime = _tempRainTime");
    g.main.max_raining = h.flags.temp_max_rain;
    g.main.cloud_alpha = h.flags.temp_max_rain;
    // WorldFile.LoadWorldFlags (WorldFile.cs:1996-2018)
    g.main.drunk_world = h.flags.drunk_world;
    g.main.tenth_anniversary_world = h.flags.tenth_anniversary_world;
    g.main.invasion_delay = h.flags.invasion_delay;
    g.main.sundial_cooldown = h.flags.sundial_cooldown as i32;
    // WorldFile.cs:2149-2156, 2302-2326: `WorldGen.setBG(bg, style)` for every background, in the file's order
    crate::world_gen_bg::set_bg(g, 0, h.flags.tree_bg1 as i32);
    crate::world_gen_bg::set_bg(g, 1, h.flags.corrupt_bg as i32);
    crate::world_gen_bg::set_bg(g, 2, h.flags.jungle_bg as i32);
    crate::world_gen_bg::set_bg(g, 3, h.flags.snow_bg as i32);
    crate::world_gen_bg::set_bg(g, 4, h.flags.hallow_bg as i32);
    crate::world_gen_bg::set_bg(g, 5, h.flags.crimson_bg as i32);
    crate::world_gen_bg::set_bg(g, 6, h.flags.desert_bg as i32);
    crate::world_gen_bg::set_bg(g, 7, h.flags.ocean_bg as i32);
    // WorldFile.cs:2157-2161
    g.main.cloud_bg_active = h.flags.cloud_bg_active as f32;
    g.main.cloud_bg_alpha = if (g.main.cloud_bg_active as f64) < 1.0 { 0.0 } else { 1.0 };
    g.main.cloud_bg_active = -(g.main.rand.next_range(8640, 86400) as f32);
    g.main.num_clouds = h.flags.num_clouds as i32;
    g.main.wind_speed_target = h.flags.wind_speed_target;
    g.main.fast_forward_time_to_dawn = h.flags.fast_forward_time_to_dawn;
    crate::world_gen_bg::set_bg(g, 8, h.flags.mushroom_bg as i32);
    crate::world_gen_bg::set_bg(g, 9, h.flags.underworld_bg as i32);
    crate::world_gen_bg::set_bg(g, 10, h.flags.tree_bg2 as i32);
    crate::world_gen_bg::set_bg(g, 11, h.flags.tree_bg3 as i32);
    crate::world_gen_bg::set_bg(g, 12, h.flags.tree_bg4 as i32);
    crate::unported!("WorldFile.LoadChests/LoadSigns/LoadNPCs/LoadTileEntities -> game state");
    let mut world = world;
    g.main.tile = std::mem::replace(&mut world.tiles, TileMap::new(1, 1));
    // WorldFile.cs:742-769 (`isGeneratingOrLoadingWorld = true` .. `Liquid.quickSettle = false`): the liquid settle
    crate::liquid::load_world_settle(g);
    // WorldFile.cs:770 `Main.weatherCounter = WorldGen.genRand.Next(3600, 18000)`: the draw is made so the generator is in the state the original leaves it in
    let _weather_counter = g.main.rand.next_range(3600, 18000);
    crate::unported!("Main.weatherCounter (store of WorldFile.LoadWorld)");
    crate::unported!("Cloud.resetClouds");
    // WorldFile.cs:772-773: `WorldGen.WaterCheck(); WorldGen.isGeneratingOrLoadingWorld = false;`
    crate::liquid::load_world_settle_finish(g);
    world
}
