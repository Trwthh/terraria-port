//! What the ported drawing code needs besides the game state: the render resources (`Main.spriteBatch`,
//! `Main.tileBatch`, the texture cache behind `TextureAssets`) and the client-only statics of `Main` that drawing reads.

use std::collections::HashSet;

use terraria_core::xna::Vector3;
use terraria_data::main_arrays::{TILE_COUNT, WALL_COUNT};
use terraria_sim::Game;

use crate::content::Content;
use crate::gfx::{DrawList, SpriteBatch, Texture2D};
use crate::graphics::tile_batch::TileBatch;
use crate::pending::SectionManager;

/// `Main.cageFrames` (Main.cs:1519)
pub const CAGE_FRAMES: usize = 25;

/// Client-only static fields of `Terraria.Main` that the drawing code reads (they are not simulation state, so they are
/// not in `terraria_sim::game::MainState`). Initial values are the declarations' in Main.cs.
pub struct MainDrawState {
    /// `Main.gfxQuality` (Main.cs:1029)
    pub gfx_quality: f32,
    /// `Main.drawToScreen` (Main.cs:595). `DoDraw` sets it from `Lighting.UpdateEveryFrame`; the port always draws straight
    /// to the screen (no render-target caching yet), so it is set to true at start-up.
    pub draw_to_screen: bool,
    /// `Main.offScreenRange` (Main.cs:839)
    pub off_screen_range: i32,
    /// `Main.martianLight` (Main.cs:1307)
    pub martian_light: f32,
    /// `Main.shimmerAlpha` (Main.cs:993)
    pub shimmer_alpha: f32,
    /// `Main.shimmerShine` (Main.cs:2448)
    pub shimmer_shine: Vector3,
    /// `Main.critterCage` (Main.cs:1521)
    pub critter_cage: bool,
    /// `Main.weatherVaneBobframe` (Main.cs:2392)
    pub weather_vane_bobframe: i32,
    /// `Main.tileFrame` (Main.cs:1671): the animation frame per tile type. The animation task drives these.
    pub tile_frame: Vec<i32>,
    /// `Main.tileFrameCounter` (Main.cs:1673)
    pub tile_frame_counter: Vec<i32>,
    /// `Main.wallFrame` (Main.cs:1675)
    pub wall_frame: Vec<u8>,
    /// `Main.wallFrameCounter` (Main.cs:1677)
    pub wall_frame_counter: Vec<u8>,
    /// `Main.cageFrames` (Main.cs:1519)
    pub cage_frames: i32,
    /// the critter cage / jar frame arrays (Main.cs:1523-1661), `new int[cageFrames]` (or `[n, cageFrames]`)
    pub bunny_cage_frame: Vec<i32>,
    pub squirrel_cage_frame: Vec<i32>,
    pub mallard_cage_frame: Vec<i32>,
    pub duck_cage_frame: Vec<i32>,
    pub grebe_cage_frame: Vec<i32>,
    pub seagull_cage_frame: Vec<i32>,
    pub bird_cage_frame: Vec<i32>,
    pub red_bird_cage_frame: Vec<i32>,
    pub blue_bird_cage_frame: Vec<i32>,
    pub macaw_cage_frame: Vec<i32>,
    pub butterfly_cage_frame: Vec<Vec<i32>>,
    pub dragonfly_jar_frame: Vec<Vec<i32>>,
    pub scorpion_cage_frame: Vec<Vec<i32>>,
    pub snail_cage_frame: Vec<i32>,
    pub snail2_cage_frame: Vec<i32>,
    pub fish_bowl_frame: Vec<i32>,
    pub lava_fish_bowl_frame: Vec<i32>,
    pub frog_cage_frame: Vec<i32>,
    pub mouse_cage_frame: Vec<i32>,
    pub turtle_cage_frame: Vec<i32>,
    pub fairy_jar_frame: Vec<i32>,
    pub jellyfish_cage_frame: Vec<Vec<i32>>,
    pub worm_cage_frame: Vec<i32>,
    pub maggot_cage_frame: Vec<i32>,
    pub rat_cage_frame: Vec<i32>,
    pub ladybug_cage_frame: Vec<i32>,
    pub penguin_cage_frame: Vec<i32>,
    pub water_strider_cage_frame: Vec<i32>,
    pub seahorse_cage_frame: Vec<i32>,
    pub slug_cage_frame: Vec<Vec<i32>>,
    pub owl_cage_frame: Vec<i32>,
    pub grasshopper_cage_frame: Vec<i32>,
    pub pufferfish_cage_frame: Vec<i32>,
    /// `Main.bunnyCageFrameCounter`
    pub bunny_cage_frame_counter: Vec<i32>,
    /// `Main.squirrelCageFrameCounter`
    pub squirrel_cage_frame_counter: Vec<i32>,
    /// `Main.mallardCageFrameCounter`
    pub mallard_cage_frame_counter: Vec<i32>,
    /// `Main.duckCageFrameCounter`
    pub duck_cage_frame_counter: Vec<i32>,
    /// `Main.grebeCageFrameCounter`
    pub grebe_cage_frame_counter: Vec<i32>,
    /// `Main.seagullCageFrameCounter`
    pub seagull_cage_frame_counter: Vec<i32>,
    /// `Main.birdCageFrameCounter`
    pub bird_cage_frame_counter: Vec<i32>,
    /// `Main.redBirdCageFrameCounter`
    pub red_bird_cage_frame_counter: Vec<i32>,
    /// `Main.blueBirdCageFrameCounter`
    pub blue_bird_cage_frame_counter: Vec<i32>,
    /// `Main.macawCageFrameCounter`
    pub macaw_cage_frame_counter: Vec<i32>,
    /// `Main.snailCageFrameCounter`
    pub snail_cage_frame_counter: Vec<i32>,
    /// `Main.snail2CageFrameCounter`
    pub snail2_cage_frame_counter: Vec<i32>,
    /// `Main.frogCageFrameCounter`
    pub frog_cage_frame_counter: Vec<i32>,
    /// `Main.mouseCageFrameCounter`
    pub mouse_cage_frame_counter: Vec<i32>,
    /// `Main.turtleCageFrameCounter`
    pub turtle_cage_frame_counter: Vec<i32>,
    /// `Main.wormCageFrameCounter`
    pub worm_cage_frame_counter: Vec<i32>,
    /// `Main.maggotCageFrameCounter`
    pub maggot_cage_frame_counter: Vec<i32>,
    /// `Main.ratCageFrameCounter`
    pub rat_cage_frame_counter: Vec<i32>,
    /// `Main.ladybugCageFrameCounter`
    pub ladybug_cage_frame_counter: Vec<i32>,
    /// `Main.penguinCageFrameCounter`
    pub penguin_cage_frame_counter: Vec<i32>,
    /// `Main.waterStriderCageFrameCounter`
    pub water_strider_cage_frame_counter: Vec<i32>,
    /// `Main.seahorseCageFrameCounter`
    pub seahorse_cage_frame_counter: Vec<i32>,
    /// `Main.owlCageFrameCounter`
    pub owl_cage_frame_counter: Vec<i32>,
    /// `Main.grasshopperCageFrameCounter`
    pub grasshopper_cage_frame_counter: Vec<i32>,
    /// `Main.pufferfishCageFrameCounter`
    pub pufferfish_cage_frame_counter: Vec<i32>,
    /// `Main.squirrelCageFrameOrange` (`new int[cageFrames]`)
    pub squirrel_cage_frame_orange: Vec<i32>,
    /// `Main.squirrelCageFrameCounterOrange` (`new int[cageFrames]`)
    pub squirrel_cage_frame_counter_orange: Vec<i32>,
    /// `Main.fishBowlFrameCounter` (`new int[cageFrames]`)
    pub fish_bowl_frame_counter: Vec<i32>,
    /// `Main.lavaFishBowlFrameCounter` (`new int[cageFrames]`)
    pub lava_fish_bowl_frame_counter: Vec<i32>,
    /// `Main.fairyJarFrameCounter` (`new int[cageFrames]`)
    pub fairy_jar_frame_counter: Vec<i32>,
    /// `Main.fishBowlFrameMode` (`new byte[cageFrames]`)
    pub fish_bowl_frame_mode: Vec<u8>,
    /// `Main.butterflyCageMode` (`new byte[9, cageFrames]`)
    pub butterfly_cage_mode: Vec<Vec<u8>>,
    /// `Main.dragonflyJarFrameCounter` (`new int[7, cageFrames]`)
    pub dragonfly_jar_frame_counter: Vec<Vec<i32>>,
    /// `Main.butterflyCageFrameCounter` (`new int[9, cageFrames]`)
    pub butterfly_cage_frame_counter: Vec<Vec<i32>>,
    /// `Main.scorpionCageFrameCounter` (`new int[2, cageFrames]`)
    pub scorpion_cage_frame_counter: Vec<Vec<i32>>,
    /// `Main.jellyfishCageFrameCounter` (`new int[3, cageFrames]`)
    pub jellyfish_cage_frame_counter: Vec<Vec<i32>>,
    /// `Main.slugCageFrameCounter` (`new int[3, cageFrames]`)
    pub slug_cage_frame_counter: Vec<Vec<i32>>,
    /// `Main.mouseTextColorChange` (Main.cs:1761, 1), `Main.masterColor` (1f), `Main.masterColorDir` (1), `Main.demonTorchDir` (1),
    /// `Main.martianLightDir` (1), `Main.DiscoStyle` (0): the direction / phase of the colour pulses `DoUpdate_Animate*` runs.
    pub mouse_text_color_change: i32,
    pub master_color: f32,
    pub master_color_dir: i32,
    pub demon_torch_dir: i32,
    pub martian_light_dir: i32,
    pub disco_style: i32,
    /// `Main.sunCircle` (Main.cs:555, `float`, 0). Advanced by `Main.DoDraw` every frame (`main_draw::do_draw`).
    pub sun_circle: f32,
    /// `Main.TileFrameSeed` (Main.cs:420, `(ulong)Guid.NewGuid().GetHashCode()`: random per run; a fixed value here).
    /// Advanced by `Main.DoDraw` every fifth frame (`main_draw::do_draw`).
    pub tile_frame_seed: u64,
    /// `Main._drawCycleCounter` (Main.cs, `long`, 0)
    pub draw_cycle_counter: i64,
    /// `Main.SettingsEnabled_TilesSwayInWind` (Main.cs:1353)
    pub settings_enabled_tiles_sway_in_wind: bool,
    /// `Main.sectionManager` (see `pending::SectionManager`)
    pub section_manager: SectionManager,
    /// the liquid drawing's client statics, `LiquidRenderer.Instance` and `waterfallManager` (see `main_liquid.rs`)
    pub liquid: crate::main_liquid::MainLiquidState,
    /// names of texture files that failed to load (each is logged once; nothing is drawn in their place)
    missing_textures: HashSet<String>,
}

impl MainDrawState {
    pub fn new() -> Self {
        let one = || vec![0; CAGE_FRAMES];
        let two = |n: usize| vec![vec![0; CAGE_FRAMES]; n];
        Self {
            gfx_quality: 1.0,
            draw_to_screen: true,
            off_screen_range: 200,
            martian_light: 1.0,
            shimmer_alpha: 0.0,
            shimmer_shine: Vector3::new(1.0, 0.5, 1.0),
            critter_cage: false,
            weather_vane_bobframe: 0,
            tile_frame: vec![0; TILE_COUNT],
            tile_frame_counter: vec![0; TILE_COUNT],
            wall_frame: vec![0; WALL_COUNT],
            wall_frame_counter: vec![0; WALL_COUNT],
            cage_frames: CAGE_FRAMES as i32,
            bunny_cage_frame: one(),
            squirrel_cage_frame: one(),
            mallard_cage_frame: one(),
            duck_cage_frame: one(),
            grebe_cage_frame: one(),
            seagull_cage_frame: one(),
            bird_cage_frame: one(),
            red_bird_cage_frame: one(),
            blue_bird_cage_frame: one(),
            macaw_cage_frame: one(),
            butterfly_cage_frame: two(9),
            dragonfly_jar_frame: two(7),
            scorpion_cage_frame: two(2),
            snail_cage_frame: one(),
            snail2_cage_frame: one(),
            fish_bowl_frame: one(),
            lava_fish_bowl_frame: one(),
            frog_cage_frame: one(),
            mouse_cage_frame: one(),
            turtle_cage_frame: one(),
            fairy_jar_frame: one(),
            jellyfish_cage_frame: two(3),
            worm_cage_frame: one(),
            maggot_cage_frame: one(),
            rat_cage_frame: one(),
            ladybug_cage_frame: one(),
            penguin_cage_frame: one(),
            water_strider_cage_frame: one(),
            seahorse_cage_frame: one(),
            slug_cage_frame: two(3),
            owl_cage_frame: one(),
            grasshopper_cage_frame: one(),
            pufferfish_cage_frame: one(),
            bunny_cage_frame_counter: one(),
            squirrel_cage_frame_counter: one(),
            mallard_cage_frame_counter: one(),
            duck_cage_frame_counter: one(),
            grebe_cage_frame_counter: one(),
            seagull_cage_frame_counter: one(),
            bird_cage_frame_counter: one(),
            red_bird_cage_frame_counter: one(),
            blue_bird_cage_frame_counter: one(),
            macaw_cage_frame_counter: one(),
            snail_cage_frame_counter: one(),
            snail2_cage_frame_counter: one(),
            frog_cage_frame_counter: one(),
            mouse_cage_frame_counter: one(),
            turtle_cage_frame_counter: one(),
            worm_cage_frame_counter: one(),
            maggot_cage_frame_counter: one(),
            rat_cage_frame_counter: one(),
            ladybug_cage_frame_counter: one(),
            penguin_cage_frame_counter: one(),
            water_strider_cage_frame_counter: one(),
            seahorse_cage_frame_counter: one(),
            owl_cage_frame_counter: one(),
            grasshopper_cage_frame_counter: one(),
            pufferfish_cage_frame_counter: one(),
            squirrel_cage_frame_orange: one(),
            squirrel_cage_frame_counter_orange: one(),
            fish_bowl_frame_counter: one(),
            lava_fish_bowl_frame_counter: one(),
            fairy_jar_frame_counter: one(),
            fish_bowl_frame_mode: vec![0; CAGE_FRAMES],
            butterfly_cage_mode: vec![vec![0; CAGE_FRAMES]; 9],
            dragonfly_jar_frame_counter: two(7),
            butterfly_cage_frame_counter: two(9),
            scorpion_cage_frame_counter: two(2),
            jellyfish_cage_frame_counter: two(3),
            slug_cage_frame_counter: two(3),
            mouse_text_color_change: 1,
            master_color: 1.0,
            master_color_dir: 1,
            demon_torch_dir: 1,
            martian_light_dir: 1,
            disco_style: 0,
            sun_circle: 0.0,
            tile_frame_seed: 0x5EED_1234,
            draw_cycle_counter: 0,
            settings_enabled_tiles_sway_in_wind: true,
            section_manager: SectionManager::default(),
            liquid: crate::main_liquid::MainLiquidState::new(),
            missing_textures: HashSet::new(),
        }
    }
}

impl Default for MainDrawState {
    fn default() -> Self {
        Self::new()
    }
}

/// Everything a ported draw function reads or writes: the game, the render resources, the client statics.
pub struct DrawContext<'a> {
    pub g: &'a mut Game,
    pub content: &'a mut Content,
    /// The frame's command list (what `GraphicsDevice` is in the original)
    pub list: &'a mut DrawList,
    /// `Main.spriteBatch`
    pub sprite_batch: &'a mut SpriteBatch,
    /// `Main.tileBatch`
    pub tile_batch: &'a mut TileBatch,
    pub state: &'a mut MainDrawState,
}

impl DrawContext<'_> {
    /// `TextureAssets.X[..].Value` for the file `Content/<name>.xnb`. A file that cannot be loaded is an error logged
    /// once and the texture is `None`; no other image is substituted.
    pub fn texture(&mut self, name: &str) -> Option<Texture2D> {
        if self.state.missing_textures.contains(name) {
            return None;
        }
        match self.content.texture(self.list, name) {
            Ok(t) => Some(t),
            Err(e) => {
                eprintln!("[texture] {name}: {e}");
                self.state.missing_textures.insert(name.to_owned());
                None
            }
        }
    }

    /// `TextureAssets.Tile[type].Value` (`Images/Tiles_<type>`); `Main.instance.LoadTiles(type)` is this lookup.
    pub fn tile_texture(&mut self, type_: i32) -> Option<Texture2D> {
        self.texture(&format!("Images/Tiles_{type_}"))
    }

    /// `TextureAssets.Wall[type].Value` (`Images/Wall_<type>`); `Main.instance.LoadWall(type)` is this lookup.
    pub fn wall_texture(&mut self, type_: i32) -> Option<Texture2D> {
        self.texture(&format!("Images/Wall_{type_}"))
    }

    /// `TextureAssets.GlowMask[n].Value` (`Images/Glow_<n>`)
    pub fn glow_mask(&mut self, n: i32) -> Option<Texture2D> {
        self.texture(&format!("Images/Glow_{n}"))
    }

    /// `TextureAssets.XmasTree[n].Value` (`Images/Xmas_<n>`)
    pub fn xmas_tree(&mut self, n: i32) -> Option<Texture2D> {
        self.texture(&format!("Images/Xmas_{n}"))
    }

    /// `TextureAssets.BlackTile.Value`
    pub fn black_tile(&mut self) -> Option<Texture2D> {
        self.texture("Images/Black_Tile")
    }

    /// `TextureAssets.WallOutline.Value`
    pub fn wall_outline(&mut self) -> Option<Texture2D> {
        self.texture("Images/Wall_Outline")
    }

    /// `TextureAssets.CageTop[n].Value` (`Images/CageTop_<n>`)
    pub fn cage_top(&mut self, n: i32) -> Option<Texture2D> {
        self.texture(&format!("Images/CageTop_{n}"))
    }

    /// `TextureAssets.ShroomCap.Value`
    pub fn shroom_cap(&mut self) -> Option<Texture2D> {
        self.texture("Images/Shroom_Tops")
    }

    /// `TextureAssets.Timer.Value` (AssetInitializer.cs:312)
    pub fn timer(&mut self) -> Option<Texture2D> {
        self.texture("Images/Timer")
    }

    /// `TextureAssets.SunAltar.Value` (AssetInitializer.cs:576)
    pub fn sun_altar(&mut self) -> Option<Texture2D> {
        self.texture("Images/SunAltar")
    }

    /// `TextureAssets.TreeTop[n].Value` (`Images/Tree_Tops_<n>`, AssetInitializer.cs:526)
    pub fn tree_top(&mut self, n: i32) -> Option<Texture2D> {
        self.texture(&format!("Images/Tree_Tops_{n}"))
    }

    /// `TextureAssets.TreeBranch[n].Value` (`Images/Tree_Branches_<n>`, AssetInitializer.cs:530)
    pub fn tree_branch(&mut self, n: i32) -> Option<Texture2D> {
        self.texture(&format!("Images/Tree_Branches_{n}"))
    }

    /// `TextureAssets.Flames[n].Value` (`Images/Flame_<n>`, AssetInitializer.cs:384)
    pub fn flame(&mut self, n: i32) -> Option<Texture2D> {
        self.texture(&format!("Images/Flame_{n}"))
    }

    /// `TextureAssets.Extra[n].Value` (`Images/Extra_<n>`, AssetInitializer.cs:365)
    pub fn extra(&mut self, n: i32) -> Option<Texture2D> {
        self.texture(&format!("Images/Extra_{n}"))
    }

    /// `TextureAssets.FireflyJar.Value` (AssetInitializer.cs:324)
    pub fn firefly_jar(&mut self) -> Option<Texture2D> {
        self.texture("Images/FireflyJar")
    }

    /// `TextureAssets.LightningbugJar.Value` (AssetInitializer.cs:326)
    pub fn lightningbug_jar(&mut self) -> Option<Texture2D> {
        self.texture("Images/LightningBugJar")
    }

    /// `TextureAssets.GlowSnail.Value` (AssetInitializer.cs:331)
    pub fn glow_snail(&mut self) -> Option<Texture2D> {
        self.texture("Images/GlowSnail")
    }

    /// `TextureAssets.XmasLight.Value` (AssetInitializer.cs:577)
    pub fn xmas_light(&mut self) -> Option<Texture2D> {
        self.texture("Images/XmasLight")
    }

    /// `TextureAssets.JellyfishBowl[n].Value` (`Images/jellyfishBowl<n + 1>`, AssetInitializer.cs:329)
    pub fn jellyfish_bowl(&mut self, n: i32) -> Option<Texture2D> {
        self.texture(&format!("Images/jellyfishBowl{}", n + 1))
    }
}

/// Builds a `DrawContext` over an empty 200 x 200 world without a game install, for the unit tests of the drawing code.
#[cfg(test)]
pub mod test_support {
    use super::*;
    use terraria_core::TileMap;

    pub fn with_ctx<R>(f: impl FnOnce(&mut DrawContext) -> R) -> R {
        let dir = std::env::temp_dir().join("terraria_app_test_install");
        std::fs::create_dir_all(dir.join("Content")).unwrap();
        std::env::set_var("TERRARIA_DIR", &dir);
        let mut content = Content::locate().unwrap();
        let mut g = Game::new();
        g.main.tile = TileMap::new(200, 200);
        g.main.max_tiles_x = 200;
        g.main.max_tiles_y = 200;
        let mut list = DrawList::default();
        let mut sprite_batch = SpriteBatch::new();
        let mut tile_batch = TileBatch::new();
        let mut state = MainDrawState::new();
        let mut ctx = DrawContext { g: &mut g, content: &mut content, list: &mut list, sprite_batch: &mut sprite_batch, tile_batch: &mut tile_batch, state: &mut state };
        f(&mut ctx)
    }
}
