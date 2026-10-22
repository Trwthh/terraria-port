//! XNA-style immediate rendering on top of Bevy's GPU device.
//!
//! Terraria draws every frame through `SpriteBatch.Begin / Draw / End` and `GraphicsDevice.SetRenderTarget`.
//! The recording half lives in `terraria_host::gfx` (no Bevy): a [`DrawList`] of ordered commands and vertices.
//! This module is the backend: once per frame the list's [`terraria_host::gfx::Frame`] is moved to the render
//! world and replayed with plain wgpu render passes ([`render`]). The back buffer is an ordinary Bevy `Image`
//! shown by a full-window sprite; every other texture and render target is a wgpu texture owned by the replay.
//!
//! [`spritebatch`] is the hand-ported prototype's deferred `SpriteBatch` (kept so the prototype's images do not
//! change); the translated game uses `terraria_host::gfx::SpriteBatch`.
//!
//! Colour handling matches XNA 4 (HiDef, no sRGB): textures and targets are `Rgba8Unorm`, blending
//! happens on the stored byte values, textures are premultiplied alpha as the content pipeline wrote them.

#![allow(unused_imports, dead_code)] // the full XNA surface is here before every caller is ported
mod render;
mod spritebatch;

pub use render::{new_target_image, BackBufferImage, DrawListRes, GfxPlugin};
pub use spritebatch::{SpriteBatch, SpriteEffects, Texture2D};
pub use terraria_core::xna::{Color, Matrix, Rectangle, Vector2};
pub use terraria_host::gfx::{BlendDesc as BlendState, ColoredQuad, DrawList, SamplerDesc as SamplerState, TargetId, TextureId, Vertex};
