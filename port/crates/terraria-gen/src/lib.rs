//! `terraria-gen`: game code translated mechanically from the decompiled C# by `tools/cs2rs`.
//!
//! - [`rt`]: the hand-written runtime (crate `terraria-gen-rt`: handles, collector, strings, collections, exceptions).
//! - [`gen`]: generated modules, one per C# class, path by namespace. Never edit; change the tool or its
//!   config (`tools/cs2rs/core.json`) and run it again (`sh tools/cs2rs/run.sh`).
//! - [`host`]: the boundary to everything that is not translated game code: `host::api` (generated: one function per
//!   library member the translated code calls, `tools/cs2rs/HOST_API.md`) and the hand-written providers the
//!   config maps natively (localisation tables).
//!
//! Contract (see `tools/cs2rs/DESIGN.md` section 1): one thread runs generated code; every generated function
//! is `unsafe fn` because it reads and writes game objects through raw pointers.

pub use terraria_gen_rt as rt;
pub use terraria_gen_rt::s;

pub mod gen;
pub mod host;
