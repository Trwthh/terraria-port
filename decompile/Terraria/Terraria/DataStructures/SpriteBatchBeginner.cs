using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.Graphics;

namespace Terraria.DataStructures
{
	public struct SpriteBatchBeginner
	{
		private SpriteSortMode sortMode;

		private BlendState blendState;

		private SamplerState samplerState;

		private DepthStencilState depthStencilState;

		private RasterizerState rasterizerState;

		private Effect effect;

		public Matrix transformMatrix;

		public SpriteBatchBeginner(SpriteSortMode sortMode, BlendState blendState, SamplerState samplerState, DepthStencilState depthStencilState, RasterizerState rasterizerState, Effect effect, Matrix transformMatrix)
		{
			this.sortMode = sortMode;
			this.blendState = blendState;
			this.samplerState = samplerState;
			this.depthStencilState = depthStencilState;
			this.rasterizerState = rasterizerState;
			this.effect = effect;
			this.transformMatrix = transformMatrix;
		}

		public void Begin(SpriteBatch spriteBatch)
		{
			spriteBatch.Begin(sortMode, blendState, samplerState, depthStencilState, rasterizerState, effect, transformMatrix);
		}

		public void Begin(SpriteBatch spriteBatch, SpriteSortMode sortMode)
		{
			spriteBatch.Begin(sortMode, blendState, samplerState, depthStencilState, rasterizerState, effect, transformMatrix);
		}

		public void Begin(TileBatch tileBatch)
		{
			tileBatch.Begin(rasterizerState, transformMatrix);
		}
	}
}
