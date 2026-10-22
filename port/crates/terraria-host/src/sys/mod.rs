//! `System.*` as the game uses it. Streams, `BinaryReader` / `BinaryWriter` and `TimeSpan` are the runtime's
//! (`terraria_gen_rt::rt::io`, `rt::time`); everything here builds on them.

pub mod crypto;
pub mod cultures;
pub mod csv;
pub mod drawing;
pub mod io;
pub mod misc;
pub mod net;
pub mod regex;
pub mod regex_engine;
pub mod text;
pub mod zip;
pub mod zlib_deflate;
pub mod threading;
pub mod time;
