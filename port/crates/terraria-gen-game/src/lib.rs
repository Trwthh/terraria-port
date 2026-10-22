//! `terraria-gen-game`: the whole game (Terraria + ReLogic) translated mechanically from the decompiled C# by
//! `tools/cs2rs` (config `tools/cs2rs/game.json`). Never edit `gen` or `host::api`; change the tool or its config.
//!
//! Contract (`tools/cs2rs/DESIGN.md` section 1): one thread runs generated code; every generated function is
//! `unsafe fn` because it reads and writes game objects through raw pointers.

pub use terraria_gen_rt as rt;
pub use terraria_gen_rt::s;

pub mod gen;
pub mod host;
