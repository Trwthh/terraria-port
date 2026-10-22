//! The boundary to everything that is not translated game code. `api` is generated: one function per library
//! member the translated code calls and no native table maps (`tools/cs2rs/HOST_API.md`).

pub use crate::rt::host::{as_instance, cast_instance, configured, inert, is_instance, new_opaque, reached, unimplemented, unimplemented_value};

pub mod api;
pub mod launch;

use crate::rt;

/// `Environment.TickCount`: milliseconds since the process started (the original counts from boot). Only used
/// to seed generators created with `new UnifiedRandom()`. The host's clock, so a driver that pins it
/// (`terraria_host::sys::time::set_fixed_tick_count`, tests/draw.rs) pins these seeds too.
pub fn environment_tick_count() -> i32 {
    terraria_host::sys::time::environment_tick_count()
}

/// `Program.ForceLoadAssembly(Assembly.GetExecutingAssembly(), initializeStaticMembers)` (Program.cs:109): the
/// original walks every type of the executable by reflection, JIT-compiles its methods (`ForceJITOnAssembly`:
/// `RuntimeHelpers.PrepareMethod`, counting `ThingsLoaded` per type) and then runs every non-generic type's static
/// constructor (`ForceStaticInitializers`). Ahead-of-time compiled code has nothing to JIT, so what is left is the
/// progress counters and the static initialisers, in the order of the executable's metadata
/// (`gen::types::STATIC_INITS`, generated from the reference database). Mapped in `tools/cs2rs/game.json`.
///
/// Deviation: `ThingsLoaded` jumps to its final value instead of counting up while methods are compiled (the
/// loading bar of the splash screen reads `Program.LoadedPercentage`).
///
/// # Safety
/// Generated code: the simulation thread only.
pub unsafe fn force_load_assembly(initialize_static_members: bool) {
    use crate::gen::{terraria::program, types};
    (*program::s()).things_to_load = types::ASSEMBLY_TYPE_COUNT;
    (*program::s()).things_loaded = types::ASSEMBLY_TYPE_COUNT;
    if initialize_static_members {
        for (_, init) in types::STATIC_INITS.iter() {
            init();
        }
    }
}

/// `ChromaInitializer.AddDevices()` (ChromaInitializer.cs:591) without the vendor device groups.
///
/// The original adds four device groups (Razer, Corsair, Logitech: P/Invoke wrappers of the vendors' SDK DLLs;
/// SteelSeries: an HTTP client of the SteelSeries Engine running on a thread of its own, built on the vendor's
/// `SteelSeries.GameSense` library) and enables those the settings select. The port ships none of the vendor SDKs,
/// so no group is added: the RGB engine, every shader and every condition exist and update as in the original, with
/// no device to send colours to. What is left of the method runs as written: the frame time and the special rules.
/// The `ProcessExit` / `ApplicationExit` hooks only disable device groups and are not registered.
/// Mapped in `tools/cs2rs/game.json` (`nativeMembers`).
///
/// # Safety
/// Generated code: the simulation thread only.
pub unsafe fn chroma_add_devices() {
    use crate::gen::re_logic::peripherals::rgb::chroma_engine;
    use crate::gen::terraria::initializers::chroma_initializer as ci;
    inert("ChromaInitializer.AddDevices: vendor RGB device groups (no vendor SDK in the port)");
    chroma_engine::set_frame_time_in_seconds((*ci::s())._engine.ptr(), ((1.0f64 / ((*ci::s())._rgb_update_rate as f64)) as f32));
    ci::load_special_rules_for_devices();
}

/// `ChromaInitializer.LoadSpecialRulesFor_GameSense()` (ChromaInitializer.cs:656): builds the SteelSeries Engine
/// event bindings (objects of the vendor's `SteelSeries.GameSense` library) and hands them to
/// `ChromaEngine.LoadSpecialRules`, which forwards them to the device groups. There is no SteelSeries group
/// ([`chroma_add_devices`]), so nothing would receive them. Mapped in `tools/cs2rs/game.json`.
pub fn chroma_skip_game_sense_rules() {
    inert("ChromaInitializer.LoadSpecialRulesFor_GameSense: SteelSeries Engine bindings (no SteelSeries device group in the port)");
}

/// `XnbReader.FromStream<DynamicSpriteFont>`: what `DynamicSpriteFontReader.Read` (DynamicSpriteFontReader.cs:10)
/// does with the values the host read from the XNB container (`terraria_host::content::FontData`): the translated
/// constructors, in the reader's order.
impl terraria_host::content::XnbObject for crate::gen::re_logic::graphics::dynamic_sprite_font::DynamicSpriteFont {
    fn from_xnb_stream(stream: rt::Gc<rt::io::Stream>) -> rt::Gc<Self> {
        use crate::gen::re_logic::graphics::{dynamic_sprite_font, font_page};
        use crate::rt::{Arr, List, Seq};
        // SAFETY: generated constructors on the simulation thread; the host objects are fresh.
        unsafe {
            let data = terraria_host::content::xnb_from_stream_font(terraria_host::game::graphics_device(), stream);
            let d = &*data.ptr();
            let font = dynamic_sprite_font::new(d.spacing, d.line_spacing, d.default_character);
            let pages: Vec<rt::Gc<font_page::FontPage>> = d
                .pages
                .as_slice()
                .iter()
                .map(|p| {
                    font_page::new(
                        p.texture,
                        List::from_seq_gc(Seq::from_arr(p.glyphs)),
                        List::from_seq_gc(Seq::from_arr(p.padding)),
                        List::from_seq_gc(Seq::from_arr(p.characters)),
                        List::from_seq_gc(Seq::from_arr(p.kerning)),
                    )
                })
                .collect();
            dynamic_sprite_font::set_pages(font.ptr(), Arr::from_slice(&pages));
            font
        }
    }
}
