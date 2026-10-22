mod content;
mod data_structures;
mod draw_context;
mod game_content;
mod gfx;
mod graphics;
mod main_animate;
mod main_backgrounds;
mod main_backgrounds_gen;
mod main_draw;
mod main_liquid;
mod pending;
mod testing;
mod waterfall_manager;

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::{PrimaryWindow, WindowResized};
use terraria_sim::Game;

use content::Content;
use draw_context::{DrawContext, MainDrawState};
use gfx::{BackBufferImage, DrawListRes, SpriteBatch, TargetId};
use graphics::tile_batch::TileBatch;
use main_draw::Renderers;

/// The image the game's back buffer is rendered into, and the sprite that shows it.
#[derive(Resource)]
struct BackBuffer {
    width: u32,
    height: u32,
}

#[derive(Component)]
struct BackBufferSprite;

/// The whole simulation state (`terraria_sim::Game`).
#[derive(Resource)]
struct GameRes(Game);

fn main() {
    let content = match Content::locate() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let game = match load_game() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()).set(WindowPlugin {
            // TERRARIA_HIDDEN=1 keeps the window off screen (automated captures)
            primary_window: Some(Window { title: "Terraria".into(), resolution: (1280u32, 720u32).into(), visible: std::env::var_os("TERRARIA_HIDDEN").is_none(), ..default() }),
            ..default()
        }))
        .add_plugins(gfx::GfxPlugin)
        .insert_resource(content)
        .insert_resource(GameRes(game))
        // the original runs Update at a fixed 60 Hz (Game.TargetElapsedTime = 1/60 s)
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .add_systems(Startup, (setup, init_draw_resources).chain())
        .add_systems(FixedUpdate, update_game)
        .add_systems(Update, (resize_back_buffer, draw_frame, capture_and_exit).chain())
        .run();
}

/// Everything the update and draw systems keep between ticks and frames: `Main.spriteBatch`, `Main.tileBatch`, the client statics
/// and the two renderers.
#[derive(Resource)]
struct DrawResources {
    sprite_batch: SpriteBatch,
    tile_batch: TileBatch,
    state: MainDrawState,
    renderers: Renderers,
}

/// World to open: `TERRARIA_WORLD=<file.wld>`. There is no menu yet, so a world file is required.
/// Optional: `TERRARIA_VIEW_TILE=<x>,<y>` centres the view on that tile instead of the spawn point;
/// `TERRARIA_UNLIT=1` draws with full-white light (debug: caves are black in the game without a light source);
/// `TERRARIA_UNPORTED_REPORT=1` prints every call into unported code with its hit count when the screenshot run exits;
/// `TERRARIA_TIME=<double>` and `TERRARIA_DAYTIME=0|1` set `Main.time` / `Main.dayTime` after the world is loaded (captures at a chosen time of day);
/// `TERRARIA_MOON_PHASE=<0..7>` sets `Main.moonPhase`.
fn load_game() -> Result<Game, String> {
    pending::UNLIT.store(std::env::var_os("TERRARIA_UNLIT").is_some(), std::sync::atomic::Ordering::Relaxed);
    let mut g = Game::new();
    let Some(path) = std::env::var_os("TERRARIA_WORLD").map(PathBuf::from) else {
        return Err("set TERRARIA_WORLD to a .wld file (the main menu is not ported yet)".into());
    };
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let world = terraria_formats::wld::read_world(&bytes).map_err(|e| format!("{}: {e:?}", path.display()))?;
    // The menu frames before a world load leave the liquid statics `Main.DoUpdate` sets (Main.cs:17287-17290, `gfxQuality` 1): `Liquid.cycles`
    // and `Liquid.curMaxLiquid`; the settle `LoadWorld` runs depends on them.
    main_liquid::do_update_liquid_statics(&mut g, MainDrawState::new().gfx_quality);
    terraria_sim::io::world_file::load_world(&mut g, world);
    g.main.game_menu = false;
    // `TextureAssets.Cloud[n].Width()/Height()` and `Star.SpawnStars()` (Main.Initialize, Main.cs:6864)
    if let Ok(c) = Content::locate() {
        g.main.cloud_texture_size = main_backgrounds::read_cloud_texture_sizes(&c.root);
    }
    terraria_sim::star::spawn_stars(&mut g, -1);
    if let Ok(v) = std::env::var("TERRARIA_TIME") {
        g.main.time = v.trim().parse::<f64>().map_err(|e| format!("TERRARIA_TIME must be a number, got {v:?}: {e}"))?;
    }
    if let Ok(v) = std::env::var("TERRARIA_DAYTIME") {
        g.main.day_time = match v.trim() {
            "1" => true,
            "0" => false,
            _ => return Err(format!("TERRARIA_DAYTIME must be 0 or 1, got {v:?}")),
        };
    }
    if let Ok(v) = std::env::var("TERRARIA_MOON_PHASE") {
        g.main.moon_phase = v.trim().parse::<i32>().map_err(|e| format!("TERRARIA_MOON_PHASE must be 0..7, got {v:?}: {e}"))?;
    }
    // No player yet: start the view centred on the spawn point, as the camera would be on a fresh spawn.
    g.main.screen_width = 1280;
    g.main.screen_height = 720;
    let (view_x, view_y) = match std::env::var("TERRARIA_VIEW_TILE") {
        Ok(v) => {
            let mut it = v.split(',').map(|n| n.trim().parse::<i32>());
            match (it.next(), it.next()) {
                (Some(Ok(x)), Some(Ok(y))) => (x, y),
                _ => return Err(format!("TERRARIA_VIEW_TILE must be <x>,<y>, got {v:?}")),
            }
        }
        Err(_) => (g.main.spawn_tile_x, g.main.spawn_tile_y),
    };
    g.main.screen_position = (
        view_x as f32 * 16.0 + 8.0 - g.main.screen_width as f32 / 2.0,
        view_y as f32 * 16.0 - g.main.screen_height as f32 / 2.0,
    );
    Ok(g)
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut list: ResMut<DrawListRes>, window: Single<&Window, With<PrimaryWindow>>) {
    let (w, h) = (window.physical_width().max(1), window.physical_height().max(1));
    let image = images.add(gfx::new_target_image(w, h));
    list.0.set_back_buffer_size(w, h);
    commands.insert_resource(BackBufferImage(image.clone()));
    commands.spawn(Camera2d);
    commands.spawn((Sprite { image, custom_size: Some(Vec2::new(window.width(), window.height())), ..default() }, BackBufferSprite));
    commands.insert_resource(BackBuffer { width: w, height: h });
}

/// `Main.Initialize` for the client statics: the batches and renderers, and `Initialize_TileAndNPCData2`'s cage randomisation.
fn init_draw_resources(
    mut commands: Commands,
    mut game: ResMut<GameRes>,
    mut content: ResMut<Content>,
    mut list: ResMut<DrawListRes>,
) {
    let mut res = DrawResources { sprite_batch: SpriteBatch::new(), tile_batch: TileBatch::new(), state: MainDrawState::new(), renderers: Renderers::new() };
    {
        let mut ctx = DrawContext {
            g: &mut game.0,
            content: &mut content,
            list: &mut list.0,
            sprite_batch: &mut res.sprite_batch,
            tile_batch: &mut res.tile_batch,
            state: &mut res.state,
        };
        main_animate::initialize_critter_cages(&mut ctx);
    }
    commands.insert_resource(res);
}

fn resize_back_buffer(
    mut resized: MessageReader<WindowResized>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut back: ResMut<BackBuffer>,
    mut images: ResMut<Assets<Image>>,
    mut list: ResMut<DrawListRes>,
    mut back_image: ResMut<BackBufferImage>,
    mut sprite: Single<&mut Sprite, With<BackBufferSprite>>,
    mut game: ResMut<GameRes>,
) {
    if resized.read().last().is_none() {
        return;
    }
    let (w, h) = (window.physical_width().max(1), window.physical_height().max(1));
    if (w, h) == (back.width, back.height) {
        return;
    }
    let image = images.add(gfx::new_target_image(w, h));
    list.0.set_back_buffer_size(w, h);
    back_image.0 = image.clone();
    sprite.image = image;
    sprite.custom_size = Some(Vec2::new(window.width(), window.height()));
    *back = BackBuffer { width: w, height: h };
    game.0.main.screen_width = w as i32;
    game.0.main.screen_height = h as i32;
}

/// `Main.Update` at 60 Hz. Until `Player` is ported the view is moved with the game's own
/// `Main.DebugCameraPan` (Main.cs:63657) on the arrow keys, then clamped by `Main.ClampScreenPositionToWorld` (Main.cs:64956).
/// The tile / wall / colour animation and the renderers' `Update` run in the order `Main.DoUpdate` calls them (Main.cs:17047-18029).
#[allow(clippy::too_many_arguments)]
fn update_game(
    mut game: ResMut<GameRes>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut res: ResMut<DrawResources>,
    mut content: ResMut<Content>,
    mut list: ResMut<DrawListRes>,
) {
    {
        let res = &mut *res;
        let mut ctx = DrawContext {
            g: &mut game.0,
            content: &mut content,
            list: &mut list.0,
            sprite_batch: &mut res.sprite_batch,
            tile_batch: &mut res.tile_batch,
            state: &mut res.state,
        };
        // Main.cs:17056 `GlobalTimeWrappedHourly = (float)(gameTime.TotalGameTime.TotalSeconds % 3600.0)`
        ctx.g.main.global_time_wrapped_hourly = (time.elapsed_secs_f64() % 3600.0) as f32;
        main_backgrounds::do_update_animate_backgrounds(&mut ctx); // Main.cs:17263
        main_animate::do_update_animate(&mut ctx);
        // Main.cs:17373 `if ((timeForVisualEffects += 1.0) >= 216000.0) timeForVisualEffects = 0.0`
        ctx.g.main.time_for_visual_effects += 1.0;
        if ctx.g.main.time_for_visual_effects >= 216000.0 {
            ctx.g.main.time_for_visual_effects = 0.0;
        }
        main_backgrounds::do_update_sky(&mut ctx); // Main.cs:17429-17488
        main_backgrounds::do_update_world_time(&mut ctx); // Main.cs:17962, 18013
        // Main.cs:18028-18029, inside UpdateWorld
        res.renderers.tiles_renderer.update(&mut ctx);
        res.renderers.walls_renderer.update(&mut ctx);
        main_liquid::update(&mut ctx); // LiquidRenderer.Update (Main.cs:17254), waterfallManager.UpdateFrame (19782), Liquid.UpdateLiquid (WorldGen.cs:72368)
    }
    let g = &mut game.0;
    // DebugCameraPan
    let mut num = 32;
    if keys.pressed(KeyCode::ShiftLeft) {
        num *= 2;
    }
    if keys.pressed(KeyCode::ControlLeft) {
        num /= 6;
    }
    let (mut x, mut y) = g.main.screen_position;
    if keys.pressed(KeyCode::ArrowLeft) {
        x -= num as f32;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        x += num as f32;
    }
    if keys.pressed(KeyCode::ArrowUp) {
        y -= num as f32;
    }
    if keys.pressed(KeyCode::ArrowDown) {
        y += num as f32;
    }
    // ClampScreenPositionToWorld with WorldUtils.GetWorldPlayArea (640 px inside each world edge).
    // GameViewMatrix.Translation is zero and Camera.ScaledSize is the screen size at zoom 1 (zoom is not ported yet).
    let num = 640;
    let (left, top) = (g.main.left_world as i32 + num, g.main.top_world as i32 + num);
    let (right, bottom) = (g.main.right_world as i32 - num, g.main.bottom_world as i32 - num);
    let (min_x, min_y) = (left as f32, top as f32);
    let (max_x, max_y) = ((right - g.main.screen_width) as f32, (bottom - g.main.screen_height) as f32);
    // Vector2.Clamp (reference/xna/Vector2.cs:198): the upper bound is applied first, so the lower bound wins when the range is inverted
    g.main.screen_position = (x.min(max_x).max(min_x), y.min(max_y).max(min_y));
}

/// `Main.Draw`: records the frame's draw list with the ported draw order (see `main_draw::do_draw`).
fn draw_frame(
    mut list: ResMut<DrawListRes>,
    mut content: ResMut<Content>,
    mut res: ResMut<DrawResources>,
    mut game: ResMut<GameRes>,
) {
    let res = &mut *res;
    let list = &mut list.0;
    list.begin_frame();
    list.set_render_target(TargetId::BACK_BUFFER, false);
    let mut ctx = DrawContext {
        g: &mut game.0,
        content: &mut content,
        list,
        sprite_batch: &mut res.sprite_batch,
        tile_batch: &mut res.tile_batch,
        state: &mut res.state,
    };
    main_draw::do_draw(&mut ctx, &mut res.renderers);
}

/// `TERRARIA_SCREENSHOT=<file.png>`: save the window contents after a few seconds, then quit.
fn capture_and_exit(mut commands: Commands, mut frame: Local<u32>, mut exit: MessageWriter<AppExit>) {
    let Some(path) = std::env::var_os("TERRARIA_SCREENSHOT") else { return };
    *frame += 1;
    let at: u32 = std::env::var("TERRARIA_SCREENSHOT_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(120);
    if *frame == at {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(PathBuf::from(path)));
    } else if *frame == at + 30 {
        // TERRARIA_UNPORTED_REPORT=1: print every call into unported code reached so far, with hit counts
        if std::env::var_os("TERRARIA_UNPORTED_REPORT").is_some() {
            eprintln!("[unported report]");
            for (name, hits) in terraria_sim::unported::report() {
                eprintln!("{hits:>10}  {name}");
            }
        }
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod main_animate_tests;
