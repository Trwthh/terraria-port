//! Debug-state commands of the input script language (`terraria-app/src/shell/script.rs`): reading the translated
//! game's statics as JSON lines, pointing the scripted mouse at a place of the world, and the few writes a test
//! needs. Hand-written; used by the app's `terraria` binary and by `tests/client.rs`, so a scripted session
//! behaves the same under both.
//!
//! NOT a module of the `terraria-gen-game` library: the two users include this file with `#[path]`, so that a
//! change here does not recompile the 1.13M-line generated crate.
//!
//! ```text
//! frame N dump player             the local player: position, velocity, life, selected item, inventory, buffs ...
//! frame N dump world              time, day / night, menu and UI flags, screen position, entity counts
//! frame N dump tile X Y           one tile (tile coordinates)
//! frame N dump tile rel DX DY     the tile DX, DY tiles from the tile under the player's centre
//! frame N dump tiles X0 Y0 X1 Y1  the tile types of a region, one string per row ("." = no tile, otherwise the
//!                                 type in hex, 3 digits); `dump tiles rel DX0 DY0 DX1 DY1` relative to the player
//! frame N dump recipes            the recipes the player can craft now (Main.availableRecipe) and the focused one
//! frame N dump item SLOT          one inventory slot of the local player (0..58)
//! frame N dump npcs               every active NPC
//! frame N dump items              every active item lying in the world
//! frame N dump combat             the active combat texts (damage numbers)
//! frame N aim tile X Y            the scripted mouse moves onto the centre of tile X, Y (through Main.screenPosition)
//! frame N aim rel DX DY           the same, relative to the tile under the player's centre
//! frame N game give ITEM [STACK]  the game's own `Player.QuickSpawnItem` for the local player (item id, stack 1)
//! frame N aim slot SLOT           the scripted mouse moves onto inventory slot SLOT (0..49: 10 columns x 5 rows)
//! frame N aim enemy [RANGE]       the scripted mouse moves onto the nearest hostile NPC within RANGE pixels of the
//!                                 player (default 320); the mouse stays where it is when there is none
//! frame N aim ui X Y              the scripted mouse moves onto interface position X, Y (times Main.UIScale)
//! frame N aim recipe K            the scripted mouse moves onto entry K of the crafting list (CraftingUI.DrawRecipesList)
//! frame N aim settings            the scripted mouse moves onto the "Settings" button (Main.RefreshSettingsButtonStatus)
//! frame N dump ui                 the menu interface: Main.menuMode, status text, the current UIState's snap points
//!                                 and element tree (type id, outer dimensions)
//! frame N aim snap NAME [ID]      the scripted mouse moves onto a snap point of the menu's current UIState
//! frame N dump menu               the legacy text menu (title, multiplayer, settings ...): the positions of its
//!                                 selectable items as Main.DrawMenu recorded them for the gamepad
//!                                 (GamepadMainMenuHandler.MenuItemPositions), the menu mode
//! frame N aim menu K              the scripted mouse moves onto selectable item K of that menu (0 = the first)
//! frame N dump players            every active player: slot, name, position, life, held item
//! frame N dump net                Main.netMode, Netplay.Disconnect, connection state, status text, the server
//!                                 address and port in use
//! frame N dump chat               the chat lines on screen and in the history (Main.chatMonitor), newest first
//! frame N dump input              what the game made of the input: PlayerInput.CurrentInputMode (0 Keyboard,
//!                                 1 KeyboardUI, 2 Mouse, 3 XBoxGamepad, 4 XBoxGamepadUI) and UsingGamepad, the
//!                                 thumbsticks as PlayerInput keeps them, the mouse, the keys of Main.keyState,
//!                                 FocusHelper.IsSelectedApplication, Game.IsActive, the form's window state and
//!                                 border, the window mode statics (screenBorderless, screenMaximized, full screen,
//!                                 back buffer and screen size, UI scale), the local player's control flags
//! frame N game moondial           calls the game's own Main.Moondialing() (the Enchanted Moondial: time runs
//!                                 fast until dusk); `game sundial` is Main.Sundialing() (until dawn)
//! frame N set time T day|night    WRITES Main.time / Main.dayTime (not something the game does: say so in a report)
//! ```
//!
//! `dump` reads only. `aim` only computes a mouse position; the click still goes through the game's input code.
//! `game ...` calls a public method of the game. `set ...` is the only command that writes game state directly.

use terraria_gen_game::gen::terraria;
use terraria_gen_game::rt::{Gc, Str};

/// What a command asks the caller to do.
#[derive(Default, Debug)]
pub struct Reply {
    /// One JSON object (no newline) for the state log.
    pub json: Option<String>,
    /// Move the scripted mouse here (client pixels).
    pub mouse: Option<(i32, i32)>,
}

/// Is `verb` one of this module's commands? (The script parser asks.)
pub fn is_command(verb: &str) -> bool {
    matches!(verb, "dump" | "aim" | "game" | "set")
}

struct J(String);
impl J {
    fn new() -> J {
        J(String::from("{"))
    }
    fn key(&mut self, k: &str) {
        if self.0.len() > 1 {
            self.0.push(',');
        }
        self.0.push('"');
        self.0.push_str(k);
        self.0.push_str("\":");
    }
    fn raw(&mut self, k: &str, v: &str) -> &mut J {
        self.key(k);
        self.0.push_str(v);
        self
    }
    fn i(&mut self, k: &str, v: i64) -> &mut J {
        self.raw(k, &v.to_string())
    }
    fn b(&mut self, k: &str, v: bool) -> &mut J {
        self.raw(k, if v { "true" } else { "false" })
    }
    /// A float as the shortest text that reads back to the same `f32` / `f64` (non-finite values as strings).
    fn f(&mut self, k: &str, v: f64) -> &mut J {
        if v.is_finite() {
            self.raw(k, &format!("{v:?}"))
        } else {
            self.raw(k, &format!("\"{v:?}\""))
        }
    }
    fn f32(&mut self, k: &str, v: f32) -> &mut J {
        if v.is_finite() {
            self.raw(k, &format!("{v:?}"))
        } else {
            self.raw(k, &format!("\"{v:?}\""))
        }
    }
    fn s(&mut self, k: &str, v: &str) -> &mut J {
        self.raw(k, &quote(v))
    }
    fn str(&mut self, k: &str, v: Str) -> &mut J {
        match v.get() {
            Some(t) => self.s(k, t),
            None => self.raw(k, "null"),
        }
    }
    fn v2(&mut self, k: &str, v: terraria_core::xna::Vector2) -> &mut J {
        let t = format!("[{},{}]", num32(v.x), num32(v.y));
        self.raw(k, &t)
    }
    fn end(mut self) -> String {
        self.0.push('}');
        self.0
    }
}

fn num32(v: f32) -> String {
    if v.is_finite() {
        format!("{v:?}")
    } else {
        format!("\"{v:?}\"")
    }
}

fn quote(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn list_json(items: Vec<String>) -> String {
    list(items)
}
fn list(items: Vec<String>) -> String {
    format!("[{}]", items.join(","))
}

unsafe fn local_player() -> Result<Gc<terraria::player::Player>, String> {
    let ms = terraria::main::s();
    if (*ms).player.is_null() {
        return Err("Main.player is null".to_owned());
    }
    let p = *(*ms).player.at((*ms).my_player);
    if p.is_null() {
        return Err("the local player is null".to_owned());
    }
    Ok(p)
}

/// The tile under the centre of the local player's hitbox.
unsafe fn player_tile() -> Result<(i32, i32), String> {
    let p = local_player()?;
    let e = &(*p.ptr()).base;
    Ok((((e.position.x + e.width as f32 / 2.0) / 16.0) as i32, ((e.position.y + e.height as f32 / 2.0) / 16.0) as i32))
}

unsafe fn item_json(it: Gc<terraria::item::Item>) -> String {
    if it.is_null() {
        return "null".to_owned();
    }
    let i = &*it.ptr();
    let mut j = J::new();
    j.i("type", i.type_ as i64).i("stack", i.stack as i64).i("prefix", i.prefix as i64);
    if i.type_ != 0 {
        j.str("name", terraria::item::name(it.ptr()));
        j.i("damage", i.damage as i64).i("pick", i.pick as i64).i("axe", i.axe as i64).i("hammer", i.hammer as i64).i("createTile", i.create_tile as i64);
    }
    j.end()
}

unsafe fn dump_player() -> Result<String, String> {
    let ms = terraria::main::s();
    let p = local_player()?;
    let pl = &*p.ptr();
    let e = &pl.base;
    let (tx, ty) = player_tile()?;
    let mut j = J::new();
    j.i("whoAmI", e.who_am_i as i64).str("name", pl.name).b("active", pl.active).b("dead", pl.dead).b("ghost", pl.ghost);
    j.v2("position", e.position).v2("velocity", e.velocity).i("direction", e.direction as i64).i("width", e.width as i64).i("height", e.height as i64);
    j.raw("centerTile", &format!("[{tx},{ty}]"));
    j.b("wet", e.wet).b("lavaWet", e.lava_wet).b("honeyWet", e.honey_wet);
    j.i("statLife", pl.stat_life as i64).i("statLifeMax", pl.stat_life_max as i64).i("statLifeMax2", pl.stat_life_max2 as i64);
    j.i("statMana", pl.stat_mana as i64).i("statManaMax", pl.stat_mana_max as i64).i("statDefense", pl.stat_defense as i64);
    j.i("selectedItem", terraria::player::selected_item(p.ptr()) as i64);
    j.raw("heldItem", &item_json(terraria::player::held_item(p.ptr())));
    j.i("itemAnimation", pl.item_animation as i64).i("itemAnimationMax", pl.item_animation_max as i64).i("itemTime", pl.item_time as i64);
    j.i("jump", pl.jump as i64).f32("wingTime", pl.wing_time).i("rocketTime", pl.rocket_time as i64).i("fallStart", pl.fall_start as i64).i("fallStart2", pl.fall_start2 as i64);
    j.b("justJumped", pl.just_jumped).b("releaseJump", pl.release_jump).b("sliding", pl.sliding);
    j.f32("gravity", pl.gravity).f32("maxFallSpeed", pl.max_fall_speed).f32("maxRunSpeed", pl.max_run_speed).f32("runAcceleration", pl.run_acceleration).f32("gravDir", pl.grav_dir);
    j.b("immune", pl.immune).i("immuneTime", pl.immune_time as i64).i("breath", pl.breath as i64).i("respawnTimer", pl.respawn_timer as i64);
    j.b("controlLeft", pl.control_left).b("controlRight", pl.control_right).b("controlUp", pl.control_up).b("controlDown", pl.control_down);
    j.b("controlJump", pl.control_jump).b("controlUseItem", pl.control_use_item).b("controlInv", pl.control_inv).b("mouseInterface", pl.mouse_interface);
    j.raw("tileTarget", &format!("[{},{}]", (*terraria::player::s()).tile_target_x, (*terraria::player::s()).tile_target_y));
    j.raw("tileRange", &format!("[{},{}]", (*terraria::player::s()).tile_range_x, (*terraria::player::s()).tile_range_y));
    j.i("chest", pl.chest as i64).raw("spawn", &format!("[{},{}]", pl.spawn_x, pl.spawn_y));
    let mut buffs = Vec::new();
    if !pl.buff_type.is_null() {
        for k in 0..pl.buff_type.len() {
            let t = *pl.buff_type.at(k);
            if t != 0 {
                buffs.push(format!("{{\"type\":{t},\"time\":{}}}", *pl.buff_time.at(k)));
            }
        }
    }
    j.raw("buffs", &list(buffs));
    let mut inv = Vec::new();
    if !pl.inventory.is_null() {
        for k in 0..pl.inventory.len() {
            let it = *pl.inventory.at(k);
            if !it.is_null() && (*it.ptr()).type_ != 0 && (*it.ptr()).stack > 0 {
                inv.push(format!("{{\"slot\":{k},\"item\":{}}}", item_json(it)));
            }
        }
    }
    j.raw("inventory", &list(inv));
    j.raw("mouseItem", &item_json((*ms).mouse_item));
    Ok(j.end())
}

unsafe fn dump_world() -> Result<String, String> {
    let ms = &*terraria::main::s();
    let mut j = J::new();
    j.b("gameMenu", ms.game_menu).i("menuMode", ms.menu_mode as i64).b("gamePaused", ms.game_paused).i("netMode", ms.net_mode as i64).i("myPlayer", ms.my_player as i64);
    j.str("worldName", ms.world_name).i("maxTilesX", ms.max_tiles_x as i64).i("maxTilesY", ms.max_tiles_y as i64);
    j.raw("spawnTile", &format!("[{},{}]", ms.spawn_tile_x, ms.spawn_tile_y)).f("worldSurface", ms.world_surface);
    j.f("time", ms.time).b("dayTime", ms.day_time).i("dayRate", ms.day_rate as i64).b("bloodMoon", ms.blood_moon);
    j.b("fastForwardTimeToDawn", ms.fast_forward_time_to_dawn).b("fastForwardTimeToDusk", ms.fast_forward_time_to_dusk).i("sundialCooldown", ms.sundial_cooldown as i64);
    j.i("gameUpdateCount", ms._game_update_count as i64);
    j.b("playerInventory", ms.player_inventory).b("mapFullscreen", ms.map_fullscreen).i("mapStyle", ms.map_style as i64).b("ingameOptionsWindow", ms.ingame_options_window);
    j.v2("screenPosition", ms.screen_position).i("screenWidth", ms.screen_width as i64).i("screenHeight", ms.screen_height as i64);
    j.i("mouseX", ms.mouse_x as i64).i("mouseY", ms.mouse_y as i64).f32("uiScale", terraria::main::ui_scale()).f32("gameZoomTarget", ms.game_zoom_target);
    j.i("numAvailableRecipes", ms.num_available_recipes as i64).i("focusRecipe", ms.focus_recipe as i64);
    let count = |n: i32| n as i64;
    let mut npcs = 0;
    if !ms.npc.is_null() {
        for k in 0..ms.npc.len() {
            let n = *ms.npc.at(k);
            if !n.is_null() && (*n.ptr()).active {
                npcs += 1;
            }
        }
    }
    let mut items = 0;
    if !ms.item.is_null() {
        for k in 0..ms.item.len() {
            let w = *ms.item.at(k);
            if !w.is_null() && terraria::world_item::active(w.ptr()) {
                items += 1;
            }
        }
    }
    let mut projectiles = 0;
    if !ms.projectile.is_null() {
        for k in 0..ms.projectile.len() {
            let w = *ms.projectile.at(k);
            if !w.is_null() && (*w.ptr()).active {
                projectiles += 1;
            }
        }
    }
    j.i("activeNpcs", count(npcs)).i("activeItems", count(items)).i("activeProjectiles", count(projectiles));
    Ok(j.end())
}

unsafe fn dump_tile(x: i32, y: i32) -> Result<String, String> {
    let ms = &*terraria::main::s();
    let mut j = J::new();
    j.i("x", x as i64).i("y", y as i64);
    if ms.tile.is_null() || x < 0 || y < 0 || x >= ms.tile.dim(0) || y >= ms.tile.dim(1) {
        j.b("outside", true);
        return Ok(j.end());
    }
    let t = *ms.tile.at(x, y);
    if t.is_null() {
        j.b("null", true);
        return Ok(j.end());
    }
    let tp = t.ptr();
    let tl = &*tp;
    j.b("active", terraria::tile::active__void(tp)).i("type", tl.type_ as i64).i("wall", tl.wall as i64);
    j.i("liquid", tl.liquid as i64).i("liquidType", terraria::tile::liquid_type__void(tp) as i64);
    j.i("frameX", tl.frame_x as i64).i("frameY", tl.frame_y as i64);
    j.i("slope", terraria::tile::slope__void(tp) as i64).b("halfBrick", terraria::tile::half_brick__void(tp)).b("inActive", terraria::tile::in_active__void(tp));
    j.i("sTileHeader", tl.s_tile_header as i64).i("bTileHeader", tl.b_tile_header as i64).i("bTileHeader2", tl.b_tile_header2 as i64).i("bTileHeader3", tl.b_tile_header3 as i64);
    Ok(j.end())
}

unsafe fn dump_npcs() -> Result<String, String> {
    let ms = &*terraria::main::s();
    let mut out = Vec::new();
    if !ms.npc.is_null() {
        for k in 0..ms.npc.len() {
            let n = *ms.npc.at(k);
            if n.is_null() || !(*n.ptr()).active {
                continue;
            }
            let np = &*n.ptr();
            let e = &np.base;
            let mut j = J::new();
            j.i("index", k as i64).i("type", np.type_ as i64).i("netID", np.net_id as i64).str("name", terraria::npc::type_name(n.ptr()));
            j.i("life", np.life as i64).i("lifeMax", np.life_max as i64).v2("position", e.position).v2("velocity", e.velocity).i("direction", e.direction as i64);
            j.i("width", e.width as i64).i("height", e.height as i64);
            let ai: Vec<String> = if np.ai.is_null() { Vec::new() } else { (0..np.ai.len()).map(|a| num32(*np.ai.at(a))).collect() };
            j.raw("ai", &list(ai));
            j.i("aiStyle", np.ai_style as i64).i("target", np.target as i64).b("justHit", np.just_hit).i("damage", np.damage as i64).i("defense", np.defense as i64);
            j.f32("knockBackResist", np.knock_back_resist).b("friendly", np.friendly).b("townNPC", np.town_npc).f32("value", np.value);
            out.push(j.end());
        }
    }
    let mut j = J::new();
    j.raw("npcs", &list(out));
    Ok(j.end())
}

unsafe fn dump_items() -> Result<String, String> {
    let ms = &*terraria::main::s();
    let mut out = Vec::new();
    if !ms.item.is_null() {
        for k in 0..ms.item.len() {
            let w = *ms.item.at(k);
            if w.is_null() || !terraria::world_item::active(w.ptr()) {
                continue;
            }
            let e = &(*w.ptr()).base;
            let mut j = J::new();
            j.i("index", k as i64).raw("item", &item_json(terraria::world_item::inner(w.ptr()))).v2("position", e.position).v2("velocity", e.velocity);
            j.i("timeSinceItemSpawned", (*w.ptr()).time_since_item_spawned as i64).b("beingGrabbed", (*w.ptr()).being_grabbed);
            out.push(j.end());
        }
    }
    let mut j = J::new();
    j.raw("items", &list(out));
    Ok(j.end())
}

unsafe fn dump_combat() -> Result<String, String> {
    let ms = &*terraria::main::s();
    let mut out = Vec::new();
    if !ms.combat_text.is_null() {
        for k in 0..ms.combat_text.len() {
            let c = *ms.combat_text.at(k);
            if c.is_null() || !(*c.ptr()).active {
                continue;
            }
            let ct = &*c.ptr();
            let mut j = J::new();
            j.i("index", k as i64).str("text", ct.text).v2("position", ct.position).v2("velocity", ct.velocity);
            j.raw("color", &format!("[{},{},{},{}]", ct.color.r(), ct.color.g(), ct.color.b(), ct.color.a())).b("crit", ct.crit).b("dot", ct.dot).i("lifeTime", ct.life_time as i64);
            out.push(j.end());
        }
    }
    let mut j = J::new();
    j.raw("combatText", &list(out));
    Ok(j.end())
}

/// Client pixel of a world position, through `Main.screenPosition` (`Main.MouseWorld` is the mouse plus the screen
/// position when the world zoom is 1; the UI scale does not enter). Refused when the game zoom is not 1: another
/// zoom needs the game's own view transform here.
unsafe fn screen_of_world(wx: f32, wy: f32) -> Result<(i32, i32), String> {
    let ms = &*terraria::main::s();
    if ms.game_zoom_target != 1.0 {
        return Err(format!("aim needs world zoom 1 (zoom target {})", ms.game_zoom_target));
    }
    Ok(((wx - ms.screen_position.x).round() as i32, (wy - ms.screen_position.y).round() as i32))
}

/// The snap points of the menu interface's current state (`Main.MenuUI.CurrentState.GetSnapPoints()`: the game's
/// own list, the one its gamepad navigation uses): type id of the state, then (name, id, position in interface
/// coordinates). An error when the menu shows no `UIState` (the title menu is drawn from `Main.menuMode` alone).
unsafe fn menu_snap_points() -> Result<(u32, Vec<(String, i32, f32, f32)>), String> {
    let ms = &*terraria::main::s();
    if ms.menu_ui.is_null() {
        return Err("Main.MenuUI is null".to_owned());
    }
    let state = terraria::ui::user_interface::current_state(ms.menu_ui.ptr());
    if state.is_null() {
        return Err(format!("Main.MenuUI has no current state (menuMode {})", ms.menu_mode));
    }
    let element = state.ptr() as *mut terraria::ui::ui_element::UIElement;
    let list = terraria::ui::ui_element::get_snap_points__v(element);
    let mut out = Vec::new();
    for k in 0..(*list.ptr()).count() {
        let p = (*list.ptr()).get_item(k);
        let pos = terraria::ui::snap_point::position(p.ptr());
        out.push(((*p.ptr()).name.get().unwrap_or("").to_owned(), terraria::ui::snap_point::id(p.ptr()), pos.x, pos.y));
    }
    Ok((terraria_gen_game::rt::type_id_of(element as *const u8), out))
}

/// The element tree of the menu's current UI state, depth first: `[depth, type id, x, y, width, height]` per
/// element (outer dimensions in interface coordinates), at most 400 elements.
unsafe fn menu_element_tree() -> Result<Vec<String>, String> {
    let ms = &*terraria::main::s();
    if ms.menu_ui.is_null() {
        return Err("Main.MenuUI is null".to_owned());
    }
    let state = terraria::ui::user_interface::current_state(ms.menu_ui.ptr());
    if state.is_null() {
        return Err(format!("Main.MenuUI has no current state (menuMode {})", ms.menu_mode));
    }
    unsafe fn walk(e: *mut terraria::ui::ui_element::UIElement, depth: i32, out: &mut Vec<String>) {
        if out.len() >= 400 {
            return;
        }
        let d = terraria::ui::ui_element::get_outer_dimensions(e);
        out.push(format!("[{depth},{},{},{},{},{}]", terraria_gen_game::rt::type_id_of(e as *const u8), num32(d.x), num32(d.y), num32(d.width), num32(d.height)));
        let children = (*e).elements;
        if children.is_null() {
            return;
        }
        for k in 0..(*children.ptr()).count() {
            let c = (*children.ptr()).get_item(k);
            if !c.is_null() {
                walk(c.ptr(), depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(state.ptr() as *mut terraria::ui::ui_element::UIElement, 0, &mut out);
    Ok(out)
}

fn int(words: &[&str], i: usize, what: &str) -> Result<i32, String> {
    words.get(i).ok_or_else(|| format!("{what}: a number is missing"))?.parse::<i32>().map_err(|_| format!("{what}: not a number: {:?}", words[i]))
}

/// A script's `wait <condition>`: does it hold now?
///
/// ```text
/// ingame                    a world is loaded and the local player is active (Main.gameMenu == false)
/// menu                      the menus are up (Main.gameMenu)
/// netmode K                 Main.netMode == K (0 single player, 1 client, 2 server)
/// menumode K                the menus are up and Main.menuMode == K (31: the server asked for its password)
/// players K                 exactly K players are active
/// remote-moved P            another active player is at least P pixels away from the local player horizontally
/// tile X Y inactive         the tile has no block; `tile rel DX DY inactive` relative to the local player's tile
/// chat TEXT                 a chat line contains TEXT
/// status TEXT               Main.statusText contains TEXT
/// ```
///
/// # Safety
/// As [`command`].
pub unsafe fn condition(cond: &str) -> Result<bool, String> {
    let words: Vec<&str> = cond.split_whitespace().collect();
    let ms = &*terraria::main::s();
    let active_players = || -> Vec<Gc<terraria::player::Player>> {
        let mut v = Vec::new();
        if !ms.player.is_null() {
            for k in 0..ms.player.len() {
                let p = *ms.player.at(k);
                if !p.is_null() && (*p.ptr()).active {
                    v.push(p);
                }
            }
        }
        v
    };
    let tile_inactive = |x: i32, y: i32| -> bool {
        let inside = !ms.tile.is_null() && x >= 0 && y >= 0 && x < ms.tile.dim(0) && y < ms.tile.dim(1);
        if !inside {
            return false;
        }
        let t = *ms.tile.at(x, y);
        t.is_null() || !terraria::tile::active__void(t.ptr())
    };
    Ok(match words.as_slice() {
        ["ingame"] => !ms.game_menu && local_player().is_ok_and(|p| (*p.ptr()).active),
        ["menu"] => ms.game_menu,
        ["netmode", k] => ms.net_mode == k.parse::<i32>().map_err(|_| "netmode K".to_owned())?,
        ["menumode", k] => ms.game_menu && ms.menu_mode == k.parse::<i32>().map_err(|_| "menumode K".to_owned())?,
        ["players", k] => active_players().len() == k.parse::<usize>().map_err(|_| "players K".to_owned())?,
        ["remote-moved", px] => {
            let px: f32 = px.parse().map_err(|_| "remote-moved PIXELS".to_owned())?;
            match local_player() {
                Ok(me) => active_players().iter().any(|p| *p != me && ((*p.ptr()).base.position.x - (*me.ptr()).base.position.x).abs() >= px),
                Err(_) => false,
            }
        }
        ["tile", "rel", dx, dy, "inactive"] => match player_tile() {
            Ok((px, py)) => tile_inactive(px + dx.parse::<i32>().map_err(|_| "tile rel DX DY inactive".to_owned())?, py + dy.parse::<i32>().map_err(|_| "tile rel DX DY inactive".to_owned())?),
            Err(_) => false,
        },
        ["tile", x, y, "inactive"] => tile_inactive(x.parse().map_err(|_| "tile X Y inactive".to_owned())?, y.parse().map_err(|_| "tile X Y inactive".to_owned())?),
        ["chat", rest @ ..] => {
            let want = rest.join(" ");
            let mon = ms.chat_monitor;
            let mut hit = false;
            if !mon.is_null() && terraria_gen_game::rt::type_id_of(mon.0 as *const u8) == terraria::game_content::ui::chat::remade_chat_monitor::TYPE_ID {
                let m = &*(mon.0 as *const terraria::game_content::ui::chat::remade_chat_monitor::RemadeChatMonitor);
                if !m._messages.is_null() {
                    for k in 0..(*m._messages.ptr()).count() {
                        let c = (*m._messages.ptr()).get_item(k);
                        if !c.is_null() && (*c.ptr()).original_text.get().is_some_and(|t| t.contains(&want)) {
                            hit = true;
                        }
                    }
                }
            }
            hit
        }
        ["status", rest @ ..] => ms.status_text.get().is_some_and(|t| t.contains(&rest.join(" "))),
        _ => return Err(format!("unknown condition {cond:?}")),
    })
}

/// Runs one command (`cmd` is the text after `frame N`).
///
/// # Safety
/// Reads (and for `game` / `set`, writes) translated game state: call it on the game thread, between `Update` calls.
pub unsafe fn command(cmd: &str) -> Result<Reply, String> {
    let words: Vec<&str> = cmd.split_whitespace().collect();
    let mut reply = Reply::default();
    match words.as_slice() {
        ["dump", "player"] => reply.json = Some(dump_player()?),
        ["dump", "world"] => reply.json = Some(dump_world()?),
        ["dump", "input"] => {
            let ms = &*terraria::main::s();
            let pi = &*terraria::game_input::player_input::s();
            let mut j = J::new();
            j.i("currentInputMode", terraria::game_input::player_input::current_input_mode() as i64).b("usingGamepad", terraria::game_input::player_input::using_gamepad()).b("usingGamepadUI", terraria::game_input::player_input::using_gamepad_ui());
            j.v2("gamepadThumbstickLeft", pi.gamepad_thumbstick_left).v2("gamepadThumbstickRight", pi.gamepad_thumbstick_right);
            // the gamepad cursor of the interface: the navigator page and point
            let nav = &*terraria::ui::gamepad::ui_link_point_navigator::s();
            j.i("linkPage", nav.current_page as i64).i("linkPoint", terraria::ui::gamepad::ui_link_point_navigator::current_point() as i64).b("linkNavigatorInUse", nav.in_use);
            j.raw("mouse", &format!("[{},{}]", ms.mouse_x, ms.mouse_y)).b("mouseLeft", ms.mouse_left).i("scrollWheelValue", pi.scroll_wheel_value as i64);
            let pressed = ms.key_state.get_pressed_keys();
            j.raw("keys", &list_json(pressed.as_slice().iter().map(|k| k.to_string()).collect()));
            j.b("isSelectedApplication", (*terraria::focus_helper::s()).is_selected_application).b("gameIsActive", terraria_host::game::is_active()).b("gamePaused", ms.game_paused);
            let form = &*terraria_host::winforms::Form::from_handle(0).ptr();
            j.i("formWindowState", form.window_state() as i64).i("formBorderStyle", form.form_border_style() as i64);
            let b = form.bounds();
            j.raw("formBounds", &format!("[{},{},{},{}]", b.x, b.y, b.width, b.height));
            j.b("screenBorderless", ms.screen_borderless).b("screenMaximized", ms.screen_maximized);
            if !ms.graphics.is_null() {
                let g = &*ms.graphics.ptr();
                j.b("isFullScreen", g.is_full_screen()).raw("backBuffer", &format!("[{},{}]", g.preferred_back_buffer_width(), g.preferred_back_buffer_height()));
            }
            let d = terraria_host::game::display_mode();
            j.raw("screen", &format!("[{},{}]", ms.screen_width, ms.screen_height)).raw("display", &format!("[{},{}]", d.0, d.1)).f32("uiScale", terraria::main::ui_scale());
            if let Ok(p) = local_player() {
                let pl = &*p.ptr();
                j.b("controlLeft", pl.control_left).b("controlRight", pl.control_right).b("controlUp", pl.control_up).b("controlDown", pl.control_down).b("controlJump", pl.control_jump).b("controlUseItem", pl.control_use_item).b("controlInv", pl.control_inv);
            }
            reply.json = Some(j.end())
        }
        ["dump", "npcs"] => reply.json = Some(dump_npcs()?),
        ["dump", "items"] => reply.json = Some(dump_items()?),
        ["dump", "combat"] => reply.json = Some(dump_combat()?),
        ["dump", "recipes"] => {
            let ms = &*terraria::main::s();
            let mut out = Vec::new();
            for k in 0..ms.num_available_recipes {
                let ri = *ms.available_recipe.at(k);
                let r = *ms.recipe.at(ri);
                out.push(format!("{{\"index\":{k},\"recipe\":{ri},\"creates\":{}}}", item_json((*r.ptr()).create_item)));
            }
            let mut j = J::new();
            j.i("numAvailableRecipes", ms.num_available_recipes as i64).i("focusRecipe", ms.focus_recipe as i64).raw("recipes", &list(out));
            reply.json = Some(j.end())
        }
        ["dump", "tiles", rest @ ..] => {
            let (rel, o) = if rest.first() == Some(&"rel") { (true, 3) } else { (false, 2) };
            let (mut x0, mut y0, mut x1, mut y1) = (int(&words, o, "dump tiles")?, int(&words, o + 1, "dump tiles")?, int(&words, o + 2, "dump tiles")?, int(&words, o + 3, "dump tiles")?);
            if rel {
                let (px, py) = player_tile()?;
                (x0, y0, x1, y1) = (px + x0, py + y0, px + x1, py + y1);
            }
            if x1 < x0 || y1 < y0 || (x1 - x0) > 200 || (y1 - y0) > 100 {
                return Err("dump tiles: an ordered region of at most 200 x 100 tiles".to_owned());
            }
            let ms = &*terraria::main::s();
            let mut rows = Vec::new();
            for y in y0..=y1 {
                let mut row = String::new();
                for x in x0..=x1 {
                    let inside = !ms.tile.is_null() && x >= 0 && y >= 0 && x < ms.tile.dim(0) && y < ms.tile.dim(1);
                    let t = if inside { *ms.tile.at(x, y) } else { Gc::NULL };
                    if t.is_null() || !terraria::tile::active__void(t.ptr()) {
                        row.push_str("  . ");
                    } else {
                        row.push_str(&format!("{:03x} ", (*t.ptr()).type_));
                    }
                }
                rows.push(quote(row.trim_end()));
            }
            let mut j = J::new();
            j.raw("from", &format!("[{x0},{y0}]")).raw("to", &format!("[{x1},{y1}]")).raw("rows", &list(rows));
            reply.json = Some(j.end())
        }
        ["dump", "tile", "rel", ..] => {
            let (px, py) = player_tile()?;
            reply.json = Some(dump_tile(px + int(&words, 3, "dump tile rel")?, py + int(&words, 4, "dump tile rel")?)?)
        }
        ["dump", "tile", ..] => reply.json = Some(dump_tile(int(&words, 2, "dump tile")?, int(&words, 3, "dump tile")?)?),
        ["dump", "item", ..] => {
            let slot = int(&words, 2, "dump item")?;
            let p = local_player()?;
            let inv = (*p.ptr()).inventory;
            if inv.is_null() || slot < 0 || slot >= inv.len() {
                return Err(format!("dump item: no inventory slot {slot}"));
            }
            let mut j = J::new();
            j.i("slot", slot as i64).raw("item", &item_json(*inv.at(slot)));
            reply.json = Some(j.end())
        }
        ["aim", "tile", ..] => {
            let (x, y) = (int(&words, 2, "aim tile")?, int(&words, 3, "aim tile")?);
            reply.mouse = Some(screen_of_world(x as f32 * 16.0 + 8.0, y as f32 * 16.0 + 8.0)?);
            let mut j = J::new();
            j.raw("tile", &format!("[{x},{y}]")).raw("mouse", &format!("[{},{}]", reply.mouse.unwrap().0, reply.mouse.unwrap().1));
            reply.json = Some(j.end())
        }
        ["aim", "rel", ..] => {
            let (px, py) = player_tile()?;
            let (x, y) = (px + int(&words, 2, "aim rel")?, py + int(&words, 3, "aim rel")?);
            reply.mouse = Some(screen_of_world(x as f32 * 16.0 + 8.0, y as f32 * 16.0 + 8.0)?);
            let mut j = J::new();
            j.raw("tile", &format!("[{x},{y}]")).raw("mouse", &format!("[{},{}]", reply.mouse.unwrap().0, reply.mouse.unwrap().1));
            reply.json = Some(j.end())
        }
        ["aim", "slot", ..] => {
            // Main.DrawInventory (Main.cs): slot (column i, row j) is drawn at 20 + i * 56 * inventoryScale,
            // 20 + j * 56 * inventoryScale with inventoryScale 0.85 and a 52-pixel back texture
            let slot = int(&words, 2, "aim slot")?;
            if !(0..50).contains(&slot) {
                return Err(format!("aim slot: {slot} is not one of the 50 main inventory slots"));
            }
            // the interface is drawn under the UI scale matrix: a UI position p is at client pixel p * UIScale
            let ui = terraria::main::ui_scale();
            let scale = 0.85f32;
            let (i, jrow) = (slot % 10, slot / 10);
            let x = ((20.0 + i as f32 * 56.0 * scale + 26.0 * scale) * ui) as i32;
            let y = ((20.0 + jrow as f32 * 56.0 * scale + 26.0 * scale) * ui) as i32;
            reply.mouse = Some((x, y));
            let mut j = J::new();
            j.i("slot", slot as i64).raw("mouse", &format!("[{x},{y}]"));
            reply.json = Some(j.end())
        }
        ["aim", "enemy", ..] => {
            let range = if words.len() > 2 { int(&words, 2, "aim enemy")? as f32 } else { 320.0 };
            let ms = &*terraria::main::s();
            let p = local_player()?;
            let pe = &(*p.ptr()).base;
            let (px, py) = (pe.position.x + pe.width as f32 / 2.0, pe.position.y + pe.height as f32 / 2.0);
            let mut best: Option<(f32, i32, f32, f32)> = None;
            if !ms.npc.is_null() {
                for k in 0..ms.npc.len() {
                    let n = *ms.npc.at(k);
                    if n.is_null() {
                        continue;
                    }
                    let np = &*n.ptr();
                    if !np.active || np.friendly || np.town_npc || np.life <= 0 || np.damage <= 0 {
                        continue;
                    }
                    let (nx, ny) = (np.base.position.x + np.base.width as f32 / 2.0, np.base.position.y + np.base.height as f32 / 2.0);
                    let d = ((nx - px) * (nx - px) + (ny - py) * (ny - py)).sqrt();
                    if d <= range && best.is_none_or(|b| d < b.0) {
                        best = Some((d, k, nx, ny));
                    }
                }
            }
            let mut j = J::new();
            match best {
                Some((d, k, nx, ny)) => {
                    let m = screen_of_world(nx, ny)?;
                    reply.mouse = Some(m);
                    j.i("npc", k as i64).f32("distance", d).raw("mouse", &format!("[{},{}]", m.0, m.1));
                }
                None => {
                    j.raw("npc", "null");
                }
            }
            reply.json = Some(j.end())
        }
        ["aim", "ui", ..] => {
            let ui = terraria::main::ui_scale();
            let (x, y) = ((int(&words, 2, "aim ui")? as f32 * ui) as i32, (int(&words, 3, "aim ui")? as f32 * ui) as i32);
            reply.mouse = Some((x, y));
            let mut j = J::new();
            j.f32("uiScale", ui).raw("mouse", &format!("[{x},{y}]"));
            reply.json = Some(j.end())
        }
        ["dump", "menu"] => {
            let ms = &*terraria::main::s();
            let list = (*terraria::ui::gamepad::gamepad_main_menu_handler::s()).menu_item_positions;
            let mut items = Vec::new();
            if !list.is_null() {
                for k in 0..(*list.ptr()).count() {
                    let v = (*list.ptr()).get_item(k);
                    items.push(format!("[{},{}]", num32(v.x), num32(v.y)));
                }
            }
            let mut j = J::new();
            j.i("menuMode", ms.menu_mode as i64).b("gameMenu", ms.game_menu).str("statusText", ms.status_text).raw("items", &list_json(items));
            reply.json = Some(j.end())
        }
        // `aim menu K`: Main.DrawMenu draws item K centred on that position (FontAssets.DeathText); the hit box is
        // the text around it
        ["aim", "menu", ..] => {
            let k = int(&words, 2, "aim menu")?;
            let list = (*terraria::ui::gamepad::gamepad_main_menu_handler::s()).menu_item_positions;
            if list.is_null() || k < 0 || k >= (*list.ptr()).count() {
                return Err(format!("aim menu: no selectable item {k} (the menu recorded {} positions; menuMode {})", if list.is_null() { 0 } else { (*list.ptr()).count() }, (*terraria::main::s()).menu_mode));
            }
            let v = (*list.ptr()).get_item(k);
            let ui = terraria::main::ui_scale();
            let (x, y) = ((v.x * ui) as i32, (v.y * ui) as i32);
            reply.mouse = Some((x, y));
            let mut j = J::new();
            j.i("item", k as i64).raw("at", &format!("[{},{}]", num32(v.x), num32(v.y))).raw("mouse", &format!("[{x},{y}]"));
            reply.json = Some(j.end())
        }
        ["dump", "players"] => {
            let ms = &*terraria::main::s();
            let mut out = Vec::new();
            if !ms.player.is_null() {
                for k in 0..ms.player.len() {
                    let p = *ms.player.at(k);
                    if p.is_null() || !(*p.ptr()).active {
                        continue;
                    }
                    let pl = &*p.ptr();
                    let mut j = J::new();
                    j.i("slot", k as i64).str("name", pl.name).v2("position", pl.base.position).v2("velocity", pl.base.velocity).i("direction", pl.base.direction as i64);
                    j.i("statLife", pl.stat_life as i64).b("dead", pl.dead).i("selectedItem", terraria::player::selected_item(p.ptr()) as i64);
                    j.raw("centerTile", &format!("[{},{}]", ((pl.base.position.x + pl.base.width as f32 / 2.0) / 16.0) as i32, ((pl.base.position.y + pl.base.height as f32 / 2.0) / 16.0) as i32));
                    out.push(j.end());
                }
            }
            let mut j = J::new();
            j.i("myPlayer", ms.my_player as i64).i("netMode", ms.net_mode as i64).raw("players", &list_json(out));
            reply.json = Some(j.end())
        }
        ["dump", "net"] => {
            let ms = &*terraria::main::s();
            let ns = &*terraria::netplay::s();
            let mut j = J::new();
            j.i("netMode", ms.net_mode as i64).b("gameMenu", ms.game_menu).i("menuMode", ms.menu_mode as i64).i("myPlayer", ms.my_player as i64).str("statusText", ms.status_text);
            j.b("disconnect", ns.disconnect).b("isHostAndPlay", ns.is_host_and_play).i("listenPort", ns.listen_port as i64).str("serverIPText", ns.server_ip_text).b("clientLoopThreadRunning", ns.client_loop_thread_running);
            j.b("hasFullyConnectedClients", ns.has_fully_connected_clients);
            if !ns.connection.is_null() {
                let c = &*ns.connection.ptr();
                j.i("connectionState", c.state as i64).b("connectionIsActive", c.is_active);
            }
            j.i("pendingSocketOperations", terraria_host::sys::net::pending_operations() as i64);
            reply.json = Some(j.end())
        }
        ["dump", "chat"] => {
            let ms = &*terraria::main::s();
            let mut lines = Vec::new();
            let mon = ms.chat_monitor;
            if !mon.is_null() && terraria_gen_game::rt::type_id_of(mon.0 as *const u8) == terraria::game_content::ui::chat::remade_chat_monitor::TYPE_ID {
                let m = &*(mon.0 as *const terraria::game_content::ui::chat::remade_chat_monitor::RemadeChatMonitor);
                if !m._messages.is_null() {
                    for k in 0..(*m._messages.ptr()).count().min(40) {
                        let c = (*m._messages.ptr()).get_item(k);
                        if !c.is_null() {
                            lines.push(quote((*c.ptr()).original_text.get().unwrap_or("")));
                        }
                    }
                }
            }
            let mut j = J::new();
            j.b("drawingPlayerChat", ms.drawing_player_chat).str("chatText", ms.chat_text).raw("lines", &list_json(lines));
            reply.json = Some(j.end())
        }
        ["dump", "ui"] => {
            let ms = &*terraria::main::s();
            let mut j = J::new();
            j.i("menuMode", ms.menu_mode as i64).b("gameMenu", ms.game_menu).str("statusText", ms.status_text).f32("uiScale", terraria::main::ui_scale());
            match menu_snap_points() {
                Ok((type_id, points)) => {
                    let known = [
                        (terraria::game_content::ui::states::ui_world_creation::TYPE_ID, "UIWorldCreation"),
                        (terraria::game_content::ui::states::ui_world_select::TYPE_ID, "UIWorldSelect"),
                        (terraria::game_content::ui::states::ui_character_select::TYPE_ID, "UICharacterSelect"),
                        (terraria::game_content::ui::states::ui_character_creation::TYPE_ID, "UICharacterCreation"),
                        (terraria::game_content::ui::states::ui_virtual_keyboard::TYPE_ID, "UIVirtualKeyboard"),
                        (terraria::game_content::ui::states::ui_world_load::TYPE_ID, "UIWorldLoad"),
                    ];
                    let name = known.iter().find(|k| k.0 == type_id).map(|k| k.1).unwrap_or("");
                    j.i("stateTypeId", type_id as i64).s("state", name);
                    j.raw("snapPoints", &list(points.iter().map(|p| format!("{{\"name\":{},\"id\":{},\"at\":[{},{}]}}", quote(&p.0), p.1, num32(p.2), num32(p.3))).collect()));
                }
                Err(e) => {
                    j.s("state", &e);
                }
            }
            if let Ok(tree) = menu_element_tree() {
                j.raw("elements", &list(tree));
            }
            reply.json = Some(j.end())
        }
        // `aim snap NAME [ID]`: the scripted mouse moves onto a snap point of the menu's current UI state (the
        // element's own `SetSnapPoint(name, id)`: "Create", "Back", "Name", "Seed", "size" 0..2, "difficulty" 0..3,
        // "evil" 0..2 on the world creation page; "Play" / "New" ... on the lists)
        ["aim", "snap", name, rest @ ..] => {
            let id = match rest.first() {
                Some(t) => Some(t.parse::<i32>().map_err(|_| format!("aim snap: not a number: {t:?}"))?),
                None => None,
            };
            let (_, points) = menu_snap_points()?;
            let hits: Vec<_> = points.iter().filter(|p| p.0 == *name && id.is_none_or(|i| i == p.1)).collect();
            let p = match hits.as_slice() {
                [p] => *p,
                [] => return Err(format!("aim snap: no snap point {name:?} {id:?}; the state has {:?}", points.iter().map(|p| format!("{} {}", p.0, p.1)).collect::<Vec<_>>())),
                _ => return Err(format!("aim snap: {} snap points are called {name:?}: give the id", hits.len())),
            };
            let ui = terraria::main::ui_scale();
            let (x, y) = ((p.2 * ui) as i32, (p.3 * ui) as i32);
            reply.mouse = Some((x, y));
            let mut j = J::new();
            j.s("snap", &p.0).i("id", p.1 as i64).raw("at", &format!("[{},{}]", num32(p.2), num32(p.3))).raw("mouse", &format!("[{x},{y}]"));
            reply.json = Some(j.end())
        }
        ["aim", "recipe", ..] => {
            // CraftingUI.DrawRecipesList (CraftingUI.cs:160-195): the slot of entry i is at
            // x = (int)(46 - 26 * s), y = (int)(410 + availableRecipeY[i] * s - 30 * s + adjY), s = max(0.75, 100 / (|Y| + 100)),
            // adjY = (Main.screenHeight - 600) / 2, or (Main.screenHeight - 508) / 2 below 700, with the interface's
            // screen height (Main.cs:42295-42301)
            let k = int(&words, 2, "aim recipe")?;
            let ms = &*terraria::main::s();
            if k < 0 || k >= ms.num_available_recipes {
                return Err(format!("aim recipe: {k} is not one of the {} available recipes", ms.num_available_recipes));
            }
            let ui = terraria::main::ui_scale();
            let ry = *(*terraria::ui::crafting_ui::s()).available_recipe_y.at(k);
            let s = (100.0f32 / (ry.abs() + 100.0)).max(0.75);
            let ui_height = terraria::game_input::player_input::ui_screen_height();
            let adj_y = if ui_height < 700 { (ui_height - 508) / 2 } else { (ui_height - 600) / 2 };
            let x = (46.0 - 26.0 * s) as i32;
            let y = (410.0 + ry * s - 30.0 * s + adj_y as f32) as i32;
            let (mx, my) = (((x as f32 + 26.0 * s) * ui) as i32, ((y as f32 + 26.0 * s) * ui) as i32);
            reply.mouse = Some((mx, my));
            let mut j = J::new();
            j.i("recipe", k as i64).f32("availableRecipeY", ry).raw("slot", &format!("[{x},{y}]")).raw("mouse", &format!("[{mx},{my}]"));
            reply.json = Some(j.end())
        }
        ["aim", "settings"] => {
            let ui = terraria::main::ui_scale();
            let (mut x, mut y) = (0i32, 0i32);
            terraria::main::refresh_settings_button_status(&mut x, &mut y);
            // the button's hit box is the text around (x, y), its lower 10 pixels excluded (Main.DrawSettingButton)
            let (mx, my) = ((x as f32 * ui) as i32, ((y - 6) as f32 * ui) as i32);
            reply.mouse = Some((mx, my));
            let mut j = J::new();
            j.raw("button", &format!("[{x},{y}]")).raw("mouse", &format!("[{mx},{my}]"));
            reply.json = Some(j.end())
        }
        ["game", "moondial"] => {
            terraria::main::moondialing();
            reply.json = Some(dump_world()?)
        }
        // `game give ITEM [STACK]`: the game's own item spawn, `Player.QuickSpawnItem(source, type, stack)` with
        // `GetItemSource_Misc(0)`: the item appears at the player and is picked up like any dropped item
        ["game", "give", ..] => {
            let item = int(&words, 2, "game give")?;
            let stack = if words.len() > 3 { int(&words, 3, "game give")? } else { 1 };
            let p = local_player()?;
            let source = terraria::player::get_item_source_misc(p.ptr(), 0);
            terraria::player::quick_spawn_item__i_entity_source_int_int(p.ptr(), source, item, stack);
            let mut j = J::new();
            j.i("item", item as i64).i("stack", stack as i64);
            reply.json = Some(j.end())
        }
        ["game", "sundial"] => {
            terraria::main::sundialing();
            reply.json = Some(dump_world()?)
        }
        ["set", "time", t, phase] => {
            let time: f64 = t.parse().map_err(|_| format!("set time: not a number: {t:?}"))?;
            let day = match *phase {
                "day" => true,
                "night" => false,
                _ => return Err("set time T day|night".to_owned()),
            };
            let ms = terraria::main::s();
            (*ms).time = time;
            (*ms).day_time = day;
            let mut j = J::new();
            j.s("wrote", "Main.time, Main.dayTime").f("time", time).b("dayTime", day);
            reply.json = Some(j.end())
        }
        _ => return Err(format!("unknown debug-state command: {cmd:?}")),
    }
    Ok(reply)
}
