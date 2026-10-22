//! The world clock and the sky of `Terraria.Main`: `UpdateTime` and its helpers, the day / night turnovers, the sunrise and
//! sunset tint of `NextHorizonRenderer`, the background variant tables. Event, invasion, NPC and weather side effects the clock
//! triggers are `unported!` at the place the original makes them. The sky colour itself is `sky_gen::set_back_color`
//! (generated from `Main.SetBackColor`).

use terraria_core::xna::Color;

use crate::game::Game;
use crate::star;
use crate::unported;

/// `Main.IsFastForwardingTime()` (Main.cs)
pub fn is_fast_forwarding_time(g: &Game) -> bool {
    if !g.main.fast_forward_time_to_dawn {
        return g.main.fast_forward_time_to_dusk;
    }
    true
}

/// `Main.ShouldNormalEventsBeAbleToStart()`: `!NPC.LunarApocalypseIsUp && !NPC.AnyNPCs(398) && NPC.MoonLordCountdown <= 0` then `LanternNight.LanternsUp`.
/// The NPC state is not ported (no Lunar Apocalypse, no Moon Lord, countdown 0).
pub fn should_normal_events_be_able_to_start(_g: &Game) -> bool {
    let lunar_apocalypse_is_up = unported!("NPC.LunarApocalypseIsUp", false);
    let any_npcs_398 = unported!("NPC.AnyNPCs(398)", false);
    let moon_lord_countdown = unported!("NPC.MoonLordCountdown", 0);
    if !lunar_apocalypse_is_up && !any_npcs_398 && moon_lord_countdown <= 0 {
        return lantern_night_lanterns_up();
    }
    true
}

/// `LanternNight.LanternsUp` (not ported: no lantern night)
fn lantern_night_lanterns_up() -> bool {
    unported!("LanternNight.LanternsUp", false)
}

/// `WorldGen.GetWorldUpdateRate()` (WorldGen.cs): `CreativePowers.FreezeTime` is not ported (never enabled).
pub fn world_gen_get_world_update_rate(g: &Game) -> i32 {
    let mut result = g.main.desired_world_tiles_update_rate.min(24);
    if unported!("CreativePowerManager.Instance.GetPower<FreezeTime>().Enabled", false) {
        result = 0;
    }
    result
}

/// `Main.UpdateTimeRate()` (Main.cs:6387)
pub fn update_time_rate(g: &mut Game) {
    if is_fast_forwarding_time(g) {
        g.main.day_rate = 60;
        g.main.desired_world_tiles_update_rate = 1;
        return;
    }
    let enabled = unported!("CreativePowerManager.Instance.GetPower<FreezeTime>().Enabled", false);
    let target_time_rate = unported!("CreativePowerManager.Instance.GetPower<ModifyTimeRate>().TargetTimeRate", 1);
    // `CurrentFrameFlags.SleepingPlayersCount == CurrentFrameFlags.ActivePlayersCount && SleepingPlayersCount > 0`: no players
    let flag = unported!("CurrentFrameFlags.SleepingPlayersCount == ActivePlayersCount", false);
    let mut num = target_time_rate;
    if !g.main.game_menu && flag {
        num *= 5;
    }
    if enabled {
        num = 0;
    }
    g.main.day_rate = num;
    g.main.desired_world_tiles_update_rate = num;
    if g.main.game_menu {
        g.main.day_rate = 1;
        g.main.desired_world_tiles_update_rate = 1;
    }
}

/// `Main.UpdateTime()` (Main.cs:65856), called by `Main.UpdateWorld_Time` (Main.cs:18299) from `Main.UpdateWorld` every tick.
pub fn update_time(g: &mut Game) {
    if lantern_night_lanterns_up() {
        g.main.cloud_bg_active = 0.0;
        if g.main.num_clouds > 30 {
            g.main.num_clouds = 30;
        }
    }
    if g.main.lady_bug_rain_boost > 0 {
        g.main.lady_bug_rain_boost -= g.main.day_rate;
    }
    if g.main.pumpkin_moon {
        g.main.blood_moon = false;
        g.main.snow_moon = false;
    }
    if g.main.snow_moon {
        g.main.blood_moon = false;
    }
    if ((g.main.net_mode != 1 && !g.main.game_menu) || g.main.net_mode == 2) && ((g.main.world_surface > 50.0) || g.main.remix_world) {
        unported!("Main.UpdateTime slime rain / rain (StopSlimeRain, StopRain, ChangeRain, StartRain, StartSlimeRain and their rand.Next calls)");
    }
    if g.main.max_raining != g.main.old_max_raining {
        if g.main.net_mode == 2 {
            unported!("NetMessage.SendData(7)");
        }
        g.main.old_max_raining = g.main.max_raining;
    }
    update_time_rate(g);
    let num4 = g.main.time;
    g.main.time += g.main.day_rate as f64;
    unported!("CultistRitual.UpdateTime");
    unported!("BirthdayParty.UpdateTime");
    unported!("LanternNight.UpdateTime");
    unported!("Sandstorm.UpdateTime");
    unported!("DD2Event.UpdateTime");
    unported!("CreditsRollEvent.UpdateTime");
    unported!("WorldGen.mysticLogsEvent.UpdateTime");
    unported!("PylonSystem.Update");
    if !g.main.ded_serv {
        unported!("NPC.MoonLordCountdown / Filters.Scene[MoonLordShake]");
    }
    unported!("NPC.MoonLordCountdown countdown and NPC.SpawnOnPlayer(398)");
    if unported!("NPC.taxCollector", false) && g.main.net_mode != 2 && !g.main.game_menu {
        unported!("Player.taxTimer / Player.CollectTaxes");
    }
    if g.main.net_mode != 1 {
        unported!("Main.UpdateSlimeRainWarning");
    }
    if g.main.net_mode != 1 {
        if unported!("NPC.travelNPC", false) {
            if !g.main.day_time || g.main.time > 48600.0 {
                unported!("WorldGen.UnspawnTravelNPC");
            }
        } else if !is_fast_forwarding_time(g) && g.main.day_time && g.main.time < 27000.0 {
            let mut num6 = g.main.day_rate;
            if num6 < 1 {
                num6 = 1;
            }
            let mut num7 = (27000.0_f64 / num6 as f64) as i32;
            num7 *= 4;
            if g.main.rand.next(num7) == 0 {
                let num8 = unported!("Main.npc[] town NPC count (active && townNPC && type != 37 && type != 453)", 0);
                if num8 >= 2 {
                    unported!("WorldGen.SpawnTravelNPC");
                }
            }
        }
        unported!("NPC.travelNPC = false");
    }
    if g.main.net_mode != 1 {
        if !g.main.day_time || g.main.time > 48600.0 {
            unported!("WorldGen.UnspawnHomelessNPC");
        } else if g.main.day_time && g.main.time < 27000.0 && !is_fast_forwarding_time(g) && world_gen_get_world_update_rate(g) > 0 {
            let mut num9 = g.main.day_rate;
            if num9 < 1 {
                num9 = 1;
            }
            let max_value = (27000.0_f64 / num9 as f64) as i32;
            if g.main.rand.next(max_value) == 0 {
                unported!("WorldGen.SpawnHomelessNPC");
            }
        }
    }
    let mut stop_events = should_normal_events_be_able_to_start(g);
    if !g.main.day_time {
        g.main.eclipse = false;
        if !is_fast_forwarding_time(g) && !stop_events {
            unported!("Main.UpdateTime night events (WorldGen.spawnEye / spawnHardBoss NPC.SpawnOnPlayer, Deerclops spawn)");
            let _ = num4;
        }
        if g.main.time > 32400.0 {
            update_time_start_day(g, &mut stop_events);
        }
        unported!("Main.HandleMeteorFall");
    } else {
        unported!("WorldGen.spawnHardBoss = 0 / WorldGen.spawnEye = false");
        g.main.blood_moon = false;
        unported!("Main.stopMoonEvent");
        if g.main.time > 54000.0 {
            update_time_start_night(g, &mut stop_events);
        }
        unported!("Main.UpdateTime_SpawnTownNPCs");
    }
    if g.main.day_time {
        g.main.tile_block_light[718] = false;
    } else {
        g.main.tile_block_light[718] = true;
    }
}

/// `Main.UpdateTime_StartNight(ref bool stopEvents)` (Main.cs:66249)
pub fn update_time_start_night(g: &mut Game, stop_events: &mut bool) {
    if g.main.fast_forward_time_to_dusk {
        g.main.fast_forward_time_to_dusk = false;
        update_time_rate(g);
    }
    if g.main.moondial_cooldown > 0 {
        g.main.moondial_cooldown -= 1;
    }
    unported!("NPC.ResetBadgerHatTime");
    unported!("NPC.freeCake = false");
    star::night_setup(g);
    unported!("NPC.setFireFlyChance");
    unported!("BirthdayParty.CheckNight");
    unported!("LanternNight.CheckNight");
    unported!("WorldGen.mysticLogsEvent.StartNight");
    unported!("WorldGen.prioritizedTownNPCType = 0");
    g.main.check_for_spawns = 0;
    if g.main.rand.next(50) == 0 && g.main.net_mode != 1 && unported!("NPC.downedBoss2", false) {
        unported!("WorldGen.spawnMeteor = true");
    }
    if !(g.main.world_surface > 50.0) {
        unported!("WorldGen.spawnMeteor = false");
    }
    if lantern_night_lanterns_up() {
        *stop_events = true;
    }
    if g.main.eclipse && g.main.net_mode != 1 {
        unported!("AchievementsHelper.NotifyProgressionEvent(3)");
    }
    g.main.eclipse = false;
    if g.main.net_mode != 1 {
        unported!("AchievementsHelper.NotifyProgressionEvent(0)");
    }
    if !is_fast_forwarding_time(g) && !*stop_events {
        // `(!NPC.downedBoss1 || ...) && netMode != 1`: the player loop (`player[i].active && statLifeMax >= 200 && statDefense > 10`) finds nobody,
        // so `flag && rand.Next(maxValue)` does not draw; the hard-mode boss warning needs `WorldGen.altarCount` (header value not loaded yet)
        unported!("Main.UpdateTime_StartNight: Eye of Cthulhu warning (WorldGen.spawnEye) / mechanical boss warning (WorldGen.spawnHardBoss)");
        let mut max_value2 = 9;
        if g.main.tenth_anniversary_world {
            max_value2 = 6;
        }
        if !unported!("WorldGen.spawnEye", false) && g.main.moon_phase != 4 && g.main.rand.next(max_value2) == 0 && g.main.net_mode != 1 {
            unported!("Main.player[m].active && statLifeMax > 120 -> bloodMoon = true");
            if g.main.blood_moon {
                g.main.sundial_cooldown = 0;
                g.main.moondial_cooldown = 0;
                unported!("AchievementsHelper.NotifyProgressionEvent(4)");
                unported!("ChatHelper.BroadcastChatMessage(Lang.misc[8])");
            }
        }
    }
    g.main.time = 0.0;
    g.main.day_time = false;
    if g.main.net_mode == 2 {
        unported!("NetMessage.SendData(7)");
    }
}

/// `Main.UpdateTime_StartDay(ref bool stopEvents)` (Main.cs:66404)
pub fn update_time_start_day(g: &mut Game, stop_events: &mut bool) {
    unported!("WorldGen.ResetTreeShakes");
    if g.main.fast_forward_time_to_dawn {
        g.main.fast_forward_time_to_dawn = false;
        update_time_rate(g);
    }
    unported!("Main.AnglerQuestSwap");
    unported!("BirthdayParty.CheckMorning");
    unported!("LanternNight.CheckMorning");
    if g.main.invasion_delay > 0 {
        g.main.invasion_delay -= 1;
    }
    unported!("WorldGen.prioritizedTownNPCType = 0");
    g.main.check_for_spawns = 0;
    g.main.time = 0.0;
    if g.main.blood_moon && g.main.net_mode != 1 {
        unported!("AchievementsHelper.NotifyProgressionEvent(5)");
    }
    g.main.blood_moon = false;
    unported!("Main.CheckForMoonEventsScoreDisplay");
    unported!("Main.CheckForMoonEventsStartingTemporarySeasons");
    unported!("Main.checkXMas");
    unported!("Main.checkHalloween");
    unported!("Main.stopMoonEvent");
    g.main.day_time = true;
    if g.main.sundial_cooldown > 0 {
        g.main.sundial_cooldown -= 1;
    }
    g.main.moon_phase += 1;
    if g.main.moon_phase >= 8 {
        g.main.moon_phase = 0;
    }
    if g.main.drunk_world && g.main.net_mode != 1 {
        unported!("WorldGen.crimson = !WorldGen.crimson");
    }
    if g.main.net_mode == 2 {
        unported!("NetMessage.SendData(7)");
    }
    if g.main.net_mode == 1 {
        return;
    }
    unported!("AchievementsHelper.NotifyProgressionEvent(1)");
    if *stop_events {
        return;
    }
    if g.main.hard_mode && unported!("NPC.downedMechBossAny", false) && g.main.rand.next(20) == 0 {
        g.main.sundial_cooldown = 0;
        g.main.moondial_cooldown = 0;
        g.main.eclipse = true;
        unported!("AchievementsHelper.NotifyProgressionEvent(2)");
        unported!("ChatHelper.BroadcastChatMessage (eclipse)");
        if g.main.net_mode == 2 {
            unported!("NetMessage.SendData(7)");
        }
    } else {
        if g.main.snow_moon || g.main.pumpkin_moon || unported!("DD2Event.Ongoing", false) {
            return;
        }
        unported!("Main.UpdateTime_StartDay: goblin / pirate invasion starts (WorldGen.shadowOrbSmashed, NPC.downedGoblins, StartInvasion)");
    }
}

/// `Main.DoUpdate_AnimateBackgrounds()` (Main.cs:19866)
pub fn do_update_animate_backgrounds(g: &mut Game) {
    let mut set1 = g.main.tree_mnt_bg_set1;
    do_update_animate_backgrounds_update_forest(g, 0, &mut set1);
    g.main.tree_mnt_bg_set1 = set1;
    let mut set2 = g.main.tree_mnt_bg_set2;
    do_update_animate_backgrounds_update_forest(g, 10, &mut set2);
    g.main.tree_mnt_bg_set2 = set2;
    let mut set3 = g.main.tree_mnt_bg_set3;
    do_update_animate_backgrounds_update_forest(g, 11, &mut set3);
    g.main.tree_mnt_bg_set3 = set3;
    let mut set4 = g.main.tree_mnt_bg_set4;
    do_update_animate_backgrounds_update_forest(g, 12, &mut set4);
    g.main.tree_mnt_bg_set4 = set4;
}

/// `Main.DoUpdate_AnimateBackgrounds_UpdateForest(int bgIndex, int[] bgSet)` (Main.cs:19874)
fn do_update_animate_backgrounds_update_forest(g: &mut Game, bg_index: usize, bg_set: &mut [i32; 2]) {
    if bg_set[1] == 94 || (bg_set[1] >= 114 && bg_set[1] <= 116) {
        g.main.bg_frame_counter[bg_index] += 1;
        if g.main.bg_frame_counter[bg_index] >= 6 {
            g.main.bg_frame_counter[bg_index] = 0;
            g.main.bg_frame[bg_index] += 1;
            if g.main.bg_frame[bg_index] >= 4 {
                g.main.bg_frame[bg_index] = 0;
            }
        }
        if g.main.bg_frame[bg_index] == 0 {
            bg_set[1] = 94;
        } else if g.main.bg_frame[bg_index] == 1 {
            bg_set[1] = 114;
        } else if g.main.bg_frame[bg_index] == 2 {
            bg_set[1] = 115;
        } else {
            bg_set[1] = 116;
        }
        if g.main.bg_frame[bg_index] == 0 {
            bg_set[0] = 93;
        } else if g.main.bg_frame[bg_index] == 1 {
            bg_set[0] = 168;
        } else if g.main.bg_frame[bg_index] == 2 {
            bg_set[0] = 169;
        } else {
            bg_set[0] = 170;
        }
    }
    if bg_set[1] < 180 || bg_set[1] > 183 {
        return;
    }
    g.main.bg_frame_counter[bg_index] += 1;
    if g.main.bg_frame_counter[bg_index] >= 6 {
        g.main.bg_frame_counter[bg_index] = 0;
        g.main.bg_frame[bg_index] += 1;
        if g.main.bg_frame[bg_index] >= 4 {
            g.main.bg_frame[bg_index] = 0;
        }
    }
    if g.main.bg_frame[bg_index] == 0 {
        bg_set[1] = 180;
    } else if g.main.bg_frame[bg_index] == 1 {
        bg_set[1] = 181;
    } else if g.main.bg_frame[bg_index] == 2 {
        bg_set[1] = 182;
    } else {
        bg_set[1] = 183;
    }
}

/// `Main.HorizonPhase` (Main.cs:2973)
pub fn horizon_phase(g: &Game) -> i32 {
    let mut num = g.main.moon_phase;
    if g.main.day_time && g.main.time < 27000.0 {
        num -= 1;
    }
    if num < 0 {
        num += 8;
    }
    num
}

/// `Main.ShouldDrawSurfaceBackground()` (Main.cs:60620)
pub fn should_draw_surface_background(g: &Game) -> bool {
    let mut result = false;
    if (!g.main.remix_world || (g.main.game_menu && !g.world_gen.remix_world_gen)) && (!g.world_gen.remix_world_gen || !g.world_gen.drunk_world_gen) {
        result = true;
    }
    if g.main.map_fullscreen {
        result = false;
    }
    result
}

/// `Math.IEEERemainder(x, y)`: `x - y * Math.Round(x / y)` with round-half-to-even; a zero result keeps the sign of `x`.
pub fn ieee_remainder(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() || x.is_infinite() || y == 0.0 {
        return f64::NAN;
    }
    if y.is_infinite() {
        return x;
    }
    let r = x - y * (x / y).round_ties_even();
    if r == 0.0 {
        return 0.0_f64.copysign(x);
    }
    r
}

/// `Main.InfoToSetBackColor` (Main.cs:200)
#[derive(Clone, Copy, Debug, Default)]
pub struct InfoToSetBackColor {
    pub is_in_game_menu_or_is_server: bool,
    pub corruption_biome_influence: f32,
    pub crimson_biome_influence: f32,
    pub jungle_biome_influence: f32,
    pub mushroom_biome_influence: f32,
    pub graveyard_influence: f32,
    pub blood_moon_active: bool,
    pub lantern_night_active: bool,
}

/// `DontStarveSeed.FixBiomeDarkness(ref Color bgColor, ref int R, ref int G, ref int B)` (DontStarveSeed.cs:53)
pub fn dont_starve_seed_fix_biome_darkness(g: &Game, bg_color: Color, r: &mut i32, g_: &mut i32, b: &mut i32) {
    if g.main.dont_starve_world {
        *r = (bg_color.r() as i32).min(*r) as u8 as i32;
        *g_ = (bg_color.g() as i32).min(*g_) as u8 as i32;
        *b = (bg_color.b() as i32).min(*b) as u8 as i32;
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// NextHorizonRenderer (GameContent/Drawing/NextHorizonRenderer.cs): the default `Main.HorizonRenderer`
// ---------------------------------------------------------------------------------------------------------------------

/// `Utils.GetLerpValue(float from, float to, float t, bool clamped)` (Utils.cs:315)
fn utils_get_lerp_value(from: f32, to: f32, t: f32, clamped: bool) -> f32 {
    if clamped {
        if from < to {
            if t < from {
                return 0.0;
            }
            if t > to {
                return 1.0;
            }
        } else {
            if t < to {
                return 1.0;
            }
            if t > from {
                return 0.0;
            }
        }
    }
    (t - from) / (to - from)
}

/// `Utils.Remap(float fromValue, float fromMin, float fromMax, float toMin, float toMax, bool clamped = true)` (Utils.cs:345)
pub fn utils_remap(from_value: f32, from_min: f32, from_max: f32, to_min: f32, to_max: f32) -> f32 {
    terraria_core::xna::math_helper::lerp(to_min, to_max, utils_get_lerp_value(from_min, from_max, from_value, true))
}

/// `SunGradients.Sunrises[n][0]` and `[Length / 2]` (SunGradients.cs): the first and the middle colour of the four sunrise gradients
/// (`Sunrise_Blue`, `Sunrise_Violet`, `Sunrise_Yellow`, `Sunrise_Aluminum`). The other 13 colours of each are not read by the ported code.
pub const SUNRISE_FIRST: [Color; 4] = [Color::new(17, 35, 67, 255), Color::new(37, 42, 58, 255), Color::new(15, 18, 28, 255), Color::new(42, 85, 135, 255)];
pub const SUNRISE_MIDDLE: [Color; 4] = [Color::new(68, 144, 149, 255), Color::new(106, 92, 142, 255), Color::new(69, 92, 102, 255), Color::new(138, 95, 155, 255)];
/// `SunGradients.Sunsets[n]` (`Sunset_Blue`, `Sunset_Dark`, `Sunset_Pink`, `Sunset_Red`)
pub const SUNSET_FIRST: [Color; 4] = [Color::new(67, 80, 117, 255), Color::new(16, 15, 33, 255), Color::new(72, 48, 93, 255), Color::new(27, 24, 39, 255)];
pub const SUNSET_MIDDLE: [Color; 4] = [Color::new(171, 103, 125, 255), Color::new(47, 23, 45, 255), Color::new(173, 93, 145, 255), Color::new(64, 32, 45, 255)];

/// `NextHorizonRenderer.GetCelestialEffectPower()` (NextHorizonRenderer.cs)
pub fn get_celestial_effect_power(g: &Game) -> f32 {
    let num = 1800.0_f32;
    let num2 = 1800.0_f32;
    let to_max = 0.0_f32;
    if g.main.day_time {
        return utils_remap(g.main.time as f32, 0.0, num * 2.0, 0.0, 1.0) * utils_remap(g.main.time as f32, 54000.0 - num, 54000.0, 1.0, to_max);
    }
    utils_remap(g.main.time as f32, 0.0, num2 * 2.0, 0.0, 1.0) * utils_remap(g.main.time as f32, 32400.0 - num2, 32400.0, 1.0, 0.0)
}

/// `NextHorizonRenderer.GetVisibilities(out float sunsetVisibility, out float sunriseVisibility, out float celestialVisibility)`;
/// returns `(sunset, sunrise, celestial)`.
pub fn get_visibilities(g: &Game) -> (f32, f32, f32) {
    let mut sunset_visibility = 1.0_f32;
    let mut sunrise_visibility = 1.0_f32;
    let celestial_visibility = get_celestial_effect_power(g);
    let mut num = 1.0_f32;
    num *= g.main.atmo;
    let num2 = 1.0_f32 - g.main.cloud_alpha;
    num *= num2 * num2;
    num *= 1.0_f32 - g.main.smoothed_mushroom_light_influence;
    sunrise_visibility *= num;
    sunset_visibility *= num;
    let time = g.main.time;
    let num3 = 54000.0_f64;
    if g.main.day_time {
        let from_min = 3600.0_f32;
        let num4 = 2700;
        let from_max = 10800.0_f32;
        let num5 = -10800.0_f32;
        let num6 = -3600.0_f32;
        sunrise_visibility *= utils_remap(time as f32, 0.0, num4 as f32, 0.0, 1.0) * utils_remap(time as f32, from_min, from_max, 1.0, 0.0);
        let num7 = utils_remap(time as f32, num3 as f32 + num5, num3 as f32 + num6, 0.0, 1.0);
        let num8 = utils_remap(time as f32, num3 as f32 + num6, num3 as f32, 1.0, 0.0);
        sunset_visibility *= num7 * num8 * num8;
        if g.main.eclipse {
            sunset_visibility = 0.0;
            sunrise_visibility = 0.0;
        }
    } else {
        sunrise_visibility = 0.0;
        sunset_visibility = 0.0;
    }
    if g.main.game_menu && g.world_gen.drunk_world_gen {
        sunrise_visibility = 0.0;
        sunset_visibility = 0.0;
    }
    (sunset_visibility, sunrise_visibility, celestial_visibility)
}

/// `NextHorizonRenderer.BlendColor(ref Color color, Color colorToChoose, float opacity)`
pub fn blend_color(color: &mut Color, color_to_choose: Color, opacity: f32) {
    if !(opacity <= 0.0) {
        let value2 = Color::new(
            color.r().max(color_to_choose.r()),
            color.g().max(color_to_choose.g()),
            color.b().max(color_to_choose.b()),
            color.a().max(color_to_choose.a()),
        );
        *color = Color::lerp(*color, value2, opacity);
    }
}

/// `NextHorizonRenderer.ModifyHorizonLight(ref Color color)` (via `Main.HorizonRenderer`)
pub fn horizon_renderer_modify_horizon_light(g: &Game, color: &mut Color) {
    if should_draw_surface_background(g) {
        let (sunset_visibility, sunrise_visibility, _celestial_visibility) = get_visibilities(g);
        let sunrise_sunset_texture_index = horizon_phase(g);
        let gradient = sunrise_sunset_texture_index as usize % SUNRISE_MIDDLE.len();
        let gradient2 = sunrise_sunset_texture_index as usize % SUNSET_MIDDLE.len();
        blend_color(color, SUNSET_MIDDLE[gradient2], sunset_visibility);
        blend_color(color, SUNRISE_MIDDLE[gradient], sunrise_visibility);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Background variant tables
// ---------------------------------------------------------------------------------------------------------------------

/// `BackgroundVariant` (DataStructures/BackgroundVariant.cs)
#[derive(Clone, Copy, Debug)]
pub struct BackgroundVariant {
    /// `_backgrounds = new int[3] { -1, -1, -1 }`
    pub backgrounds: [i32; 3],
}

impl BackgroundVariant {
    pub const fn new() -> Self {
        Self { backgrounds: [-1, -1, -1] }
    }
    /// `HasAny`
    pub fn has_any(&self) -> bool {
        if self.backgrounds[0] == -1 && self.backgrounds[1] == -1 {
            return self.backgrounds[2] != -1;
        }
        true
    }
    /// `Set(int far, int middle, int near)`
    pub fn set(&mut self, far: i32, middle: i32, near: i32) {
        self.backgrounds[0] = far;
        self.backgrounds[1] = middle;
        self.backgrounds[2] = near;
    }
    /// `Clear()`
    pub fn clear(&mut self) {
        self.set(-1, -1, -1);
    }
}

impl Default for BackgroundVariant {
    fn default() -> Self {
        Self::new()
    }
}

/// `BackgroundVariantSet` (DataStructures/BackgroundVariantSet.cs)
#[derive(Clone, Copy, Debug, Default)]
pub struct BackgroundVariantSet {
    pub pure: BackgroundVariant,
    pub corrupt: BackgroundVariant,
    pub crimson: BackgroundVariant,
    pub hallow: BackgroundVariant,
}

impl BackgroundVariantSet {
    pub const fn new() -> Self {
        Self { pure: BackgroundVariant::new(), corrupt: BackgroundVariant::new(), crimson: BackgroundVariant::new(), hallow: BackgroundVariant::new() }
    }
    /// `Clear()`
    pub fn clear(&mut self) {
        self.pure.clear();
        self.corrupt.clear();
        self.crimson.clear();
        self.hallow.clear();
    }
}

/// `BackgroundChangeFlashInfo` (GameContent/BackgroundChangeFlashInfo.cs): `WorldGen.BackgroundsCache`
#[derive(Clone, Copy, Debug)]
pub struct BackgroundChangeFlashInfo {
    /// `_variations = new int[TreeTopsInfo.AreaId.Count]` (13)
    variations: [i32; 13],
    /// `_flashPower = new float[13]`
    flash_power: [f32; 13],
}

impl BackgroundChangeFlashInfo {
    pub const fn new() -> Self {
        Self { variations: [0; 13], flash_power: [0.0; 13] }
    }
    /// `UpdateVariation(int areaId, int newVariationValue)`
    fn update_variation(&mut self, area_id: usize, new_variation_value: i32, game_menu: bool) {
        let num = self.variations[area_id];
        self.variations[area_id] = new_variation_value;
        if num != new_variation_value {
            // ValueChanged(areaId)
            if !game_menu {
                self.flash_power[area_id] = 1.0;
            }
        }
    }
    /// `UpdateFlashValues()`
    pub fn update_flash_values(&mut self) {
        for i in 0..self.flash_power.len() {
            self.flash_power[i] = terraria_core::xna::math_helper::clamp(self.flash_power[i] - 0.05, 0.0, 1.0);
        }
    }
    /// `GetFlashPower(int areaId)`
    pub fn get_flash_power(&self, area_id: i32) -> f32 {
        self.flash_power[area_id as usize]
    }
}

impl Default for BackgroundChangeFlashInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// `BackgroundChangeFlashInfo.UpdateCache()` on `WorldGen.BackgroundsCache`
pub fn background_changes_update_cache(g: &mut Game) {
    let game_menu = g.main.game_menu;
    let w = &g.world_gen;
    let values = [w.tree_bg1, w.tree_bg2, w.tree_bg3, w.tree_bg4, w.corrupt_bg, w.jungle_bg, w.snow_bg, w.hallow_bg, w.crimson_bg, w.desert_bg, w.ocean_bg, w.mushroom_bg, w.underworld_bg];
    let cache = &mut g.world_gen.backgrounds_cache;
    for (area_id, v) in values.iter().enumerate() {
        cache.update_variation(area_id, *v, game_menu);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ieee_remainder_matches_the_clr() {
        assert_eq!(ieee_remainder(10.0, 4.0), 2.0); // 10 - 4 * round(2.5) = 10 - 8 (round half to even)
        assert_eq!(ieee_remainder(11.0, 4.0), -1.0); // 11 - 4 * 3
        assert_eq!(ieee_remainder(-5.0, 2.0), -1.0); // -5 - 2 * round(-2.5) = -5 + 4
        assert!(ieee_remainder(8.0, 4.0).is_sign_positive());
        assert!(ieee_remainder(-8.0, 4.0).is_sign_negative());
    }

    #[test]
    fn day_turns_into_night_and_back_with_the_moon_phase_advancing() {
        let mut g = Game::new();
        g.main.game_menu = false;
        g.main.world_surface = 100.0;
        g.main.day_time = true;
        g.main.time = 53999.0;
        g.main.moon_phase = 7;
        update_time(&mut g); // 54000: not yet "> 54000"
        assert!(g.main.day_time);
        assert_eq!(g.main.time, 54000.0);
        update_time(&mut g); // 54001 > 54000 -> StartNight: time = 0, night
        assert!(!g.main.day_time);
        assert_eq!(g.main.time, 0.0);
        assert_eq!(g.main.moon_phase, 7);
        assert!(g.main.tile_block_light[718]);
        g.main.time = 32400.0;
        update_time(&mut g); // 32401 > 32400 -> StartDay: time = 0, day, moonPhase 7 -> 0
        assert!(g.main.day_time);
        assert_eq!(g.main.time, 0.0);
        assert_eq!(g.main.moon_phase, 0);
        assert!(!g.main.tile_block_light[718]);
        // the phase advances once per day through all eight values
        for expected in 1..=8 {
            g.main.day_time = false;
            g.main.time = 32401.0;
            update_time(&mut g);
            assert_eq!(g.main.moon_phase, expected % 8);
        }
    }

    #[test]
    fn day_rate_scales_the_clock() {
        let mut g = Game::new();
        g.main.game_menu = false;
        g.main.world_surface = 100.0;
        g.main.time = 1000.0;
        update_time(&mut g);
        assert_eq!(g.main.time, 1001.0);
        g.main.fast_forward_time_to_dawn = true; // IsFastForwardingTime: dayRate 60
        g.main.day_time = false;
        g.main.time = 0.0;
        update_time(&mut g);
        assert_eq!(g.main.time, 60.0);
    }

    #[test]
    fn horizon_phase_counts_the_morning_as_the_previous_night() {
        let mut g = Game::new();
        g.main.moon_phase = 0;
        g.main.day_time = true;
        g.main.time = 100.0;
        assert_eq!(horizon_phase(&g), 7);
        g.main.time = 30000.0;
        assert_eq!(horizon_phase(&g), 0);
        g.main.moon_phase = 3;
        g.main.time = 1.0;
        assert_eq!(horizon_phase(&g), 2);
    }
}
