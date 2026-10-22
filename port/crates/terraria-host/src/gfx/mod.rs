//! Graphics: the backend-neutral draw list and the XNA object model that records into it.

pub mod batcher;
pub mod device;
pub mod draw_list;
pub mod effect;
pub mod fx_binary;
pub mod sprite_batch;
pub mod states;
pub mod trace;

pub use batcher::{QuadLayout, QueuedSprite, SpriteBatcher, SpriteSortMode};
pub use device::{GraphicsDevice, IndexBuffer, PresentationParameters, Texture2D, VertexBuffer, VertexDeclaration, VertexElement, VertexPositionColorTexture, Viewport};
pub use draw_list::{
    Blend, BlendDesc, BlendFunction, ColoredQuad, Command, CullMode, DrawList, EffectId, EffectSnapshot, Frame, IRect, ParamValue, RawVertex, SamplerDesc, TargetId, TextureAddressMode, TextureFilter,
    TextureId, Topology, Vertex, DISCARD_COLOR,
};
pub use effect::{Effect, EffectDesc, EffectParameter, EffectPass, EffectTechnique};
pub use sprite_batch::SpriteBatch;
pub use states::{presets, BlendState, DepthStencilState, Presets, RasterizerState, SamplerState};
