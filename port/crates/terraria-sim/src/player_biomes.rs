//! `Terraria.Player.UpdateBiomes()` (Player.cs:16950): the `Zone*` flags of the player, copied from the scene metrics the
//! tile census (`scene_metrics::scan`) produced. The background code reads the scene metrics directly; the player's own flags
//! are what the NPC spawner, the music and the buffs read. `Player` itself is not ported, so the flags live in `MainState::player_zones`.

use crate::scene_metrics::SceneMetrics;

/// The `Zone*` fields of `Player` that `UpdateBiomes` sets from the scene metrics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerZones {
    pub zone_dungeon: bool,
    pub zone_lihzhard_temple: bool,
    pub zone_granite: bool,
    pub zone_marble: bool,
    pub zone_hive: bool,
    pub zone_gem_cave: bool,
    pub zone_underground_desert: bool,
    pub zone_shimmer: bool,
    pub zone_corrupt: bool,
    pub zone_crimson: bool,
    pub zone_hallow: bool,
    pub zone_jungle: bool,
    pub zone_snow: bool,
    pub zone_desert: bool,
    pub zone_glowshroom: bool,
    pub zone_meteor: bool,
    pub zone_graveyard: bool,
    pub zone_water_candle: bool,
    pub zone_peace_candle: bool,
    pub zone_shadow_candle: bool,
    pub zone_beach: bool,
    pub zone_rain: bool,
    pub zone_sandstorm: bool,
    pub zone_underworld_height: bool,
    pub zone_rock_layer_height: bool,
    pub zone_dirt_layer_height: bool,
    pub zone_overworld_height: bool,
    pub zone_sky_height: bool,
}

/// The zone part of `Player.UpdateBiomes()` (Player.cs:16950-16993). The tower / old-ones-army zones, `townNPCs`,
/// `HasGardenGnomeNearby`, `behindBackWall` and the achievement checks need NPCs and the player and are not ported.
pub fn update_biomes(scene: &SceneMetrics) -> PlayerZones {
    crate::unported!("Player.UpdateBiomes: townNPCs / ZoneTower* / ZoneOldOneArmy / HasGardenGnomeNearby / behindBackWall / achievements");
    PlayerZones {
        zone_dungeon: scene.zone_dungeon,
        zone_lihzhard_temple: scene.zone_lihzhard_temple,
        zone_granite: scene.zone_granite,
        zone_marble: scene.zone_marble,
        zone_hive: scene.zone_hive,
        zone_gem_cave: scene.zone_gem_cave,
        zone_underground_desert: scene.zone_underground_desert,
        zone_shimmer: scene.zone_shimmer,
        zone_corrupt: scene.zone_corrupt,
        zone_crimson: scene.zone_crimson,
        zone_hallow: scene.zone_hallow,
        zone_jungle: scene.zone_jungle,
        zone_snow: scene.zone_snow,
        zone_desert: scene.zone_desert,
        zone_glowshroom: scene.zone_glowshroom,
        zone_meteor: scene.zone_meteor,
        zone_graveyard: scene.zone_graveyard,
        zone_water_candle: scene.zone_water_candle,
        zone_peace_candle: scene.zone_peace_candle,
        zone_shadow_candle: scene.zone_shadow_candle,
        zone_beach: scene.zone_beach,
        zone_rain: scene.zone_rain,
        zone_sandstorm: scene.zone_sandstorm,
        zone_underworld_height: scene.zone_underworld_height,
        zone_rock_layer_height: scene.zone_rock_layer_height,
        zone_dirt_layer_height: scene.zone_dirt_layer_height,
        zone_overworld_height: scene.zone_overworld_height,
        zone_sky_height: scene.zone_sky_height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_are_copied_one_to_one() {
        let mut m = SceneMetrics::new();
        m.zone_snow = true;
        m.zone_underworld_height = true;
        m.zone_beach = true;
        let z = update_biomes(&m);
        assert!(z.zone_snow && z.zone_underworld_height && z.zone_beach);
        assert!(!z.zone_corrupt && !z.zone_sky_height);
    }
}
