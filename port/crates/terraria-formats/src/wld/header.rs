//! `SaveWorldHeader` / `SaveWorldFlags` (WorldFile.cs 1233-1445) and `LoadHeader` / `LoadWorldFlags`
//! (1996-2532). For the versions this reader accepts every `versionNumber >= N` branch of the loader is
//! taken, so the layout equals the writer's sequence; the field order below is that sequence.
//!
//! Fields named `temp_*` are `WorldFile._temp*`: copies of `Main` state that the loader parks in
//! `WorldFile` and applies later in `WorldFile.LoadWorld`.

use super::binio::{Reader, Writer};
use super::error::WldError;
use super::wire::{wire_struct, Wire};

/// `BannerSystem.Save` / `BannerSystem.Load` (GameContent/BannerSystem.cs 172-212).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct BannerSystem {
    /// `BannerSystem.killCount` as stored: `short` count, then `int` each.
    pub kill_count: Vec<i32>,
    /// `BannerSystem.claimableBanners` as stored: `short` count, then `ushort` each.
    pub claimable_banners: Vec<u16>,
}

impl Wire for BannerSystem {
    const MIN_SIZE: usize = 4;
    fn read(r: &mut Reader) -> Result<Self, WldError> {
        let at = r.pos();
        let n = r.i16()?;
        let n = r.checked_count(at, n as i64, 4, "banner kill count")?;
        let mut kill_count = Vec::with_capacity(n);
        for _ in 0..n {
            kill_count.push(r.i32()?);
        }
        let at = r.pos();
        let n = r.i16()?;
        let n = r.checked_count(at, n as i64, 2, "claimable banner")?;
        let mut claimable_banners = Vec::with_capacity(n);
        for _ in 0..n {
            claimable_banners.push(r.u16()?);
        }
        Ok(BannerSystem { kill_count, claimable_banners })
    }
    fn write(&self, w: &mut Writer) -> Result<(), WldError> {
        w.len_i16("WorldHeader", self.kill_count.len(), "banner kill count")?;
        for v in &self.kill_count {
            w.i32(*v);
        }
        w.len_i16("WorldHeader", self.claimable_banners.len(), "claimable banner")?;
        for v in &self.claimable_banners {
            w.u16(*v);
        }
        Ok(())
    }
}

/// `TreeTopsInfo.Save` / `Load` (GameContent/TreeTopsInfo.cs): `int` count, then `int` each.
/// The game keeps `AreaId.Count` = 13 variations; `Load` reads `min(count, 13)` values and would leave
/// the rest unread (a bad section pointer), so more than 13 is rejected.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TreeTops {
    pub variations: Vec<i32>,
}

pub const TREE_TOPS_AREA_COUNT: usize = 13;

impl Wire for TreeTops {
    const MIN_SIZE: usize = 4;
    fn read(r: &mut Reader) -> Result<Self, WldError> {
        let at = r.pos();
        let n = r.count(4, "tree top variation")?;
        if n > TREE_TOPS_AREA_COUNT {
            return Err(r.invalid_at(at, format!("{n} tree top variations, the game has {TREE_TOPS_AREA_COUNT}")));
        }
        let mut variations = Vec::with_capacity(n);
        for _ in 0..n {
            variations.push(r.i32()?);
        }
        Ok(TreeTops { variations })
    }
    fn write(&self, w: &mut Writer) -> Result<(), WldError> {
        w.len_i32("WorldHeader", self.variations.len(), "tree top variation")?;
        for v in &self.variations {
            w.i32(*v);
        }
        Ok(())
    }
}

/// `ExtraSpawnPointManager.Write` / `Read` (GameContent/ExtraSpawnPointManager.cs): `byte` count, then
/// `short x, short y` each.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ExtraSpawnPoints {
    /// `ExtraSpawnPointManager.extraSpawnPoints` as (x, y) tile coordinates.
    pub points: Vec<(i16, i16)>,
}

impl Wire for ExtraSpawnPoints {
    const MIN_SIZE: usize = 1;
    fn read(r: &mut Reader) -> Result<Self, WldError> {
        let n = r.u8()?;
        let mut points = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let x = r.i16()?;
            let y = r.i16()?;
            points.push((x, y));
        }
        Ok(ExtraSpawnPoints { points })
    }
    fn write(&self, w: &mut Writer) -> Result<(), WldError> {
        match u8::try_from(self.points.len()) {
            Ok(n) => w.u8(n),
            Err(_) => {
                return Err(WldError::Unrepresentable {
                    section: "WorldHeader",
                    reason: format!("{} extra spawn points do not fit in a byte", self.points.len()),
                })
            }
        }
        for (x, y) in &self.points {
            w.i16(*x);
            w.i16(*y);
        }
        Ok(())
    }
}

wire_struct! {
    /// `SaveWorldFlags` (loader: `LoadWorldFlags`), in file order.
    pub struct WorldFlags {
        /// `Main.GameMode`
        pub game_mode: i32,
        /// `Main.drunkWorld`
        pub drunk_world: bool,
        /// `Main.getGoodWorld`
        pub get_good_world: bool,
        /// `Main.tenthAnniversaryWorld`
        pub tenth_anniversary_world: bool,
        /// `Main.dontStarveWorld`
        pub dont_starve_world: bool,
        /// `Main.notTheBeesWorld`
        pub not_the_bees_world: bool,
        /// `Main.remixWorld`
        pub remix_world: bool,
        /// `Main.noTrapsWorld`
        pub no_traps_world: bool,
        /// `Main.zenithWorld`
        pub zenith_world: bool,
        /// `Main.skyblockWorld`
        pub skyblock_world: bool,
        /// `Main.ActiveWorldFileData.CreationTime.ToBinary()` (raw `DateTime` binary form)
        pub creation_time: i64,
        /// `Main.ActiveWorldFileData.LastPlayed` as `DateTime.ToBinary()`; the original writes `DateTime.UtcNow.ToBinary()` on every save
        pub last_played: i64,
        /// `Main.moonType`
        pub moon_type: u8,
        /// `Main.treeX`
        pub tree_x: [i32; 3],
        /// `Main.treeStyle`
        pub tree_style: [i32; 4],
        /// `Main.caveBackX`
        pub cave_back_x: [i32; 3],
        /// `Main.caveBackStyle`
        pub cave_back_style: [i32; 4],
        /// `Main.iceBackStyle`
        pub ice_back_style: i32,
        /// `Main.jungleBackStyle`
        pub jungle_back_style: i32,
        /// `Main.hellBackStyle`
        pub hell_back_style: i32,
        /// `Main.spawnTileX`
        pub spawn_tile_x: i32,
        /// `Main.spawnTileY`
        pub spawn_tile_y: i32,
        /// `Main.worldSurface`
        pub world_surface: f64,
        /// `Main.rockLayer`
        pub rock_layer: f64,
        /// `WorldFile._tempTime` (`Main.time`)
        pub temp_time: f64,
        /// `WorldFile._tempDayTime` (`Main.dayTime`)
        pub temp_day_time: bool,
        /// `WorldFile._tempMoonPhase` (`Main.moonPhase`)
        pub temp_moon_phase: i32,
        /// `WorldFile._tempBloodMoon` (`Main.bloodMoon`)
        pub temp_blood_moon: bool,
        /// `WorldFile._tempEclipse` (`Main.eclipse`)
        pub temp_eclipse: bool,
        /// `Main.dungeonX`
        pub dungeon_x: i32,
        /// `Main.dungeonY`
        pub dungeon_y: i32,
        /// `WorldGen.crimson`
        pub crimson: bool,
        /// `NPC.downedBoss1` (Eye of Cthulhu)
        pub downed_boss1: bool,
        /// `NPC.downedBoss2` (Eater of Worlds / Brain of Cthulhu)
        pub downed_boss2: bool,
        /// `NPC.downedBoss3` (Skeletron)
        pub downed_boss3: bool,
        /// `NPC.downedQueenBee`
        pub downed_queen_bee: bool,
        /// `NPC.downedMechBoss1`
        pub downed_mech_boss1: bool,
        /// `NPC.downedMechBoss2`
        pub downed_mech_boss2: bool,
        /// `NPC.downedMechBoss3`
        pub downed_mech_boss3: bool,
        /// `NPC.downedMechBossAny`
        pub downed_mech_boss_any: bool,
        /// `NPC.downedPlantBoss`
        pub downed_plant_boss: bool,
        /// `NPC.downedGolemBoss`
        pub downed_golem_boss: bool,
        /// `NPC.downedSlimeKing`
        pub downed_slime_king: bool,
        /// `NPC.savedGoblin`
        pub saved_goblin: bool,
        /// `NPC.savedWizard`
        pub saved_wizard: bool,
        /// `NPC.savedMech`
        pub saved_mech: bool,
        /// `NPC.downedGoblins`
        pub downed_goblins: bool,
        /// `NPC.downedClown`
        pub downed_clown: bool,
        /// `NPC.downedFrost`
        pub downed_frost: bool,
        /// `NPC.downedPirates`
        pub downed_pirates: bool,
        /// `WorldGen.shadowOrbSmashed`
        pub shadow_orb_smashed: bool,
        /// `WorldGen.spawnMeteor`
        pub spawn_meteor: bool,
        /// `WorldGen.shadowOrbCount` (stored as a byte)
        pub shadow_orb_count: u8,
        /// `WorldGen.altarCount`
        pub altar_count: i32,
        /// `Main.hardMode`
        pub hard_mode: bool,
        /// `Main.afterPartyOfDoom`
        pub after_party_of_doom: bool,
        /// `Main.invasionDelay`
        pub invasion_delay: i32,
        /// `Main.invasionSize`
        pub invasion_size: i32,
        /// `Main.invasionType`
        pub invasion_type: i32,
        /// `Main.invasionX`
        pub invasion_x: f64,
        /// `Main.slimeRainTime`
        pub slime_rain_time: f64,
        /// `Main.sundialCooldown` (stored as a byte)
        pub sundial_cooldown: u8,
        /// `WorldFile._tempRaining` (`Main.raining`)
        pub temp_raining: bool,
        /// `WorldFile._tempRainTime` (`Main.rainTime`)
        pub temp_rain_time: i32,
        /// `WorldFile._tempMaxRain` (`Main.maxRaining`)
        pub temp_max_rain: f32,
        /// `WorldGen.SavedOreTiers.Cobalt`
        pub saved_ore_tiers_cobalt: i32,
        /// `WorldGen.SavedOreTiers.Mythril`
        pub saved_ore_tiers_mythril: i32,
        /// `WorldGen.SavedOreTiers.Adamantite`
        pub saved_ore_tiers_adamantite: i32,
        /// `WorldGen.treeBG1`
        pub tree_bg1: u8,
        /// `WorldGen.corruptBG`
        pub corrupt_bg: u8,
        /// `WorldGen.jungleBG`
        pub jungle_bg: u8,
        /// `WorldGen.snowBG`
        pub snow_bg: u8,
        /// `WorldGen.hallowBG`
        pub hallow_bg: u8,
        /// `WorldGen.crimsonBG`
        pub crimson_bg: u8,
        /// `WorldGen.desertBG`
        pub desert_bg: u8,
        /// `WorldGen.oceanBG`
        pub ocean_bg: u8,
        /// `Main.cloudBGActive` (stored as an int)
        pub cloud_bg_active: i32,
        /// `Main.numClouds` (stored as a short)
        pub num_clouds: i16,
        /// `Main.windSpeedTarget`
        pub wind_speed_target: f32,
        /// `Main.anglerWhoFinishedToday`
        pub angler_who_finished_today: Vec<String>,
        /// `NPC.savedAngler`
        pub saved_angler: bool,
        /// `Main.anglerQuest`
        pub angler_quest: i32,
        /// `NPC.savedStylist`
        pub saved_stylist: bool,
        /// `NPC.savedTaxCollector`
        pub saved_tax_collector: bool,
        /// `NPC.savedGolfer`
        pub saved_golfer: bool,
        /// `Main.invasionSizeStart`
        pub invasion_size_start: i32,
        /// `WorldFile._tempCultistDelay` (`CultistRitual.delay`)
        pub temp_cultist_delay: i32,
        /// `BannerSystem.killCount` and `BannerSystem.claimableBanners`
        pub banner_system: BannerSystem,
        /// `Main.fastForwardTimeToDawn`
        pub fast_forward_time_to_dawn: bool,
        /// `NPC.downedFishron`
        pub downed_fishron: bool,
        /// `NPC.downedMartians`
        pub downed_martians: bool,
        /// `NPC.downedAncientCultist`
        pub downed_ancient_cultist: bool,
        /// `NPC.downedMoonlord`
        pub downed_moonlord: bool,
        /// `NPC.downedHalloweenKing`
        pub downed_halloween_king: bool,
        /// `NPC.downedHalloweenTree`
        pub downed_halloween_tree: bool,
        /// `NPC.downedChristmasIceQueen`
        pub downed_christmas_ice_queen: bool,
        /// `NPC.downedChristmasSantank`
        pub downed_christmas_santank: bool,
        /// `NPC.downedChristmasTree`
        pub downed_christmas_tree: bool,
        /// `NPC.downedTowerSolar`
        pub downed_tower_solar: bool,
        /// `NPC.downedTowerVortex`
        pub downed_tower_vortex: bool,
        /// `NPC.downedTowerNebula`
        pub downed_tower_nebula: bool,
        /// `NPC.downedTowerStardust`
        pub downed_tower_stardust: bool,
        /// `NPC.TowerActiveSolar`
        pub tower_active_solar: bool,
        /// `NPC.TowerActiveVortex`
        pub tower_active_vortex: bool,
        /// `NPC.TowerActiveNebula`
        pub tower_active_nebula: bool,
        /// `NPC.TowerActiveStardust`
        pub tower_active_stardust: bool,
        /// `NPC.LunarApocalypseIsUp`
        pub lunar_apocalypse_is_up: bool,
        /// `WorldFile._tempPartyManual` (`BirthdayParty.ManualParty`)
        pub temp_party_manual: bool,
        /// `WorldFile._tempPartyGenuine` (`BirthdayParty.GenuineParty`)
        pub temp_party_genuine: bool,
        /// `WorldFile._tempPartyCooldown` (`BirthdayParty.PartyDaysOnCooldown`)
        pub temp_party_cooldown: i32,
        /// `WorldFile.TempPartyCelebratingNPCs`
        pub temp_party_celebrating_npcs: Vec<i32>,
        /// `WorldFile._tempSandstormHappening`
        pub temp_sandstorm_happening: bool,
        /// `WorldFile._tempSandstormTimeLeft`
        pub temp_sandstorm_time_left: i32,
        /// `WorldFile._tempSandstormSeverity`
        pub temp_sandstorm_severity: f32,
        /// `WorldFile._tempSandstormIntendedSeverity`
        pub temp_sandstorm_intended_severity: f32,
        /// `NPC.savedBartender` (written just before `DD2Event.Save`)
        pub saved_bartender: bool,
        /// `DD2Event.DownedInvasionT1`
        pub dd2_downed_invasion_t1: bool,
        /// `DD2Event.DownedInvasionT2`
        pub dd2_downed_invasion_t2: bool,
        /// `DD2Event.DownedInvasionT3`
        pub dd2_downed_invasion_t3: bool,
        /// `WorldGen.mushroomBG`
        pub mushroom_bg: u8,
        /// `WorldGen.underworldBG`
        pub underworld_bg: u8,
        /// `WorldGen.treeBG2`
        pub tree_bg2: u8,
        /// `WorldGen.treeBG3`
        pub tree_bg3: u8,
        /// `WorldGen.treeBG4`
        pub tree_bg4: u8,
        /// `NPC.combatBookWasUsed`
        pub combat_book_was_used: bool,
        /// `WorldFile._tempLanternNightCooldown`
        pub temp_lantern_night_cooldown: i32,
        /// `WorldFile._tempLanternNightGenuine`
        pub temp_lantern_night_genuine: bool,
        /// `WorldFile._tempLanternNightManual`
        pub temp_lantern_night_manual: bool,
        /// `WorldFile._tempLanternNightNextNightIsGenuine`
        pub temp_lantern_night_next_night_is_genuine: bool,
        /// `WorldGen.TreeTops` (`TreeTopsInfo._variations`)
        pub tree_tops: TreeTops,
        /// `Main.forceHalloweenForToday`
        pub force_halloween_for_today: bool,
        /// `Main.forceXMasForToday`
        pub force_x_mas_for_today: bool,
        /// `WorldGen.SavedOreTiers.Copper`
        pub saved_ore_tiers_copper: i32,
        /// `WorldGen.SavedOreTiers.Iron`
        pub saved_ore_tiers_iron: i32,
        /// `WorldGen.SavedOreTiers.Silver`
        pub saved_ore_tiers_silver: i32,
        /// `WorldGen.SavedOreTiers.Gold`
        pub saved_ore_tiers_gold: i32,
        /// `NPC.boughtCat`
        pub bought_cat: bool,
        /// `NPC.boughtDog`
        pub bought_dog: bool,
        /// `NPC.boughtBunny`
        pub bought_bunny: bool,
        /// `NPC.downedEmpressOfLight`
        pub downed_empress_of_light: bool,
        /// `NPC.downedQueenSlime`
        pub downed_queen_slime: bool,
        /// `NPC.downedDeerclops`
        pub downed_deerclops: bool,
        /// `NPC.unlockedSlimeBlueSpawn`
        pub unlocked_slime_blue_spawn: bool,
        /// `NPC.unlockedMerchantSpawn`
        pub unlocked_merchant_spawn: bool,
        /// `NPC.unlockedDemolitionistSpawn`
        pub unlocked_demolitionist_spawn: bool,
        /// `NPC.unlockedPartyGirlSpawn`
        pub unlocked_party_girl_spawn: bool,
        /// `NPC.unlockedDyeTraderSpawn`
        pub unlocked_dye_trader_spawn: bool,
        /// `NPC.unlockedTruffleSpawn`
        pub unlocked_truffle_spawn: bool,
        /// `NPC.unlockedArmsDealerSpawn`
        pub unlocked_arms_dealer_spawn: bool,
        /// `NPC.unlockedNurseSpawn`
        pub unlocked_nurse_spawn: bool,
        /// `NPC.unlockedPrincessSpawn`
        pub unlocked_princess_spawn: bool,
        /// `NPC.combatBookVolumeTwoWasUsed`
        pub combat_book_volume_two_was_used: bool,
        /// `NPC.peddlersSatchelWasUsed`
        pub peddlers_satchel_was_used: bool,
        /// `NPC.unlockedSlimeGreenSpawn`
        pub unlocked_slime_green_spawn: bool,
        /// `NPC.unlockedSlimeOldSpawn`
        pub unlocked_slime_old_spawn: bool,
        /// `NPC.unlockedSlimePurpleSpawn`
        pub unlocked_slime_purple_spawn: bool,
        /// `NPC.unlockedSlimeRainbowSpawn`
        pub unlocked_slime_rainbow_spawn: bool,
        /// `NPC.unlockedSlimeRedSpawn`
        pub unlocked_slime_red_spawn: bool,
        /// `NPC.unlockedSlimeYellowSpawn`
        pub unlocked_slime_yellow_spawn: bool,
        /// `NPC.unlockedSlimeCopperSpawn`
        pub unlocked_slime_copper_spawn: bool,
        /// `Main.fastForwardTimeToDusk`
        pub fast_forward_time_to_dusk: bool,
        /// `Main.moondialCooldown` (stored as a byte)
        pub moondial_cooldown: u8,
        /// `Main.forceHalloweenForever`
        pub force_halloween_forever: bool,
        /// `Main.forceXMasForever`
        pub force_x_mas_forever: bool,
        /// `Main.vampireSeed`
        pub vampire_seed: bool,
        /// `Main.infectedSeed`
        pub infected_seed: bool,
        /// `WorldFile._tempMeteorShowerCount`
        pub temp_meteor_shower_count: i32,
        /// `WorldFile._tempCoinRain`
        pub temp_coin_rain: i32,
        /// `Main.teamBasedSpawnsSeed`
        pub team_based_spawns_seed: bool,
        /// `ExtraSpawnPointManager.extraSpawnPoints`
        pub extra_spawn_points: ExtraSpawnPoints,
        /// `Main.dualDungeonsSeed`
        pub dual_dungeons_seed: bool,
        /// `Main.moreLightningSeed`
        pub more_lightning_seed: bool,
        /// `Main.noLightningSeed`
        pub no_lightning_seed: bool,
        /// `WorldGen.Manifest.Serialize()`: the JSON text, kept verbatim (not parsed)
        pub world_manifest: String,
    }
}

wire_struct! {
    /// `SaveWorldHeader` up to and including `Main.maxTilesX`, then the flags.
    pub struct WorldHeader {
        /// `Main.worldName`
        pub world_name: String,
        /// `Main.ActiveWorldFileData.SeedText`
        pub seed_text: String,
        /// `Main.ActiveWorldFileData.WorldGeneratorVersion`
        pub world_generator_version: u64,
        /// `Main.ActiveWorldFileData.UniqueId.ToByteArray()` (Guid in .NET byte order)
        pub unique_id: [u8; 16],
        /// `Main.ActiveWorldFileData.WorldId`
        pub world_id: i32,
        /// `Main.leftWorld` (stored as int)
        pub left_world: i32,
        /// `Main.rightWorld` (stored as int)
        pub right_world: i32,
        /// `Main.topWorld` (stored as int)
        pub top_world: i32,
        /// `Main.bottomWorld` (stored as int)
        pub bottom_world: i32,
        /// `Main.maxTilesY`
        pub max_tiles_y: i32,
        /// `Main.maxTilesX`
        pub max_tiles_x: i32,
        /// `WorldFile.SaveWorldFlags`
        pub flags: WorldFlags,
    }
}

impl Wire for WorldFlags {
    const MIN_SIZE: usize = 1;
    fn read(r: &mut Reader) -> Result<Self, WldError> {
        WorldFlags::read_fields(r)
    }
    fn write(&self, w: &mut Writer) -> Result<(), WldError> {
        self.write_fields(w)
    }
}
