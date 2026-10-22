using System;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;

namespace Terraria.GameContent
{
	public abstract class AnOutlinedDrawRenderTargetContent : ARenderTargetContentByRequest
	{
		protected int width = 84;

		protected int height = 84;

		public Color _borderColor = Color.White;

		private RenderTarget2D _helperTarget;

		public void UseColor(Color color)
		{
			_borderColor = color;
		}

		protected override void HandleUseRequest(GraphicsDevice device, SpriteBatch spriteBatch)
		{
			Effect pixelShader = Main.pixelShader;
			EffectPass effectPass = pixelShader.CurrentTechnique.Passes["ColorOnly"];
			new Rectangle(0, 0, width, height);
			PrepareARenderTarget_AndListenToEvents(ref _target, device, width, height, RenderTargetUsage.PreserveContents);
			PrepareARenderTarget_WithoutListeningToEvents(ref _helperTarget, device, width, height, RenderTargetUsage.DiscardContents);
			device.SetRenderTarget(_helperTarget);
			device.Clear(Color.Transparent);
			spriteBatch.Begin(SpriteSortMode.Immediate, BlendState.AlphaBlend, SamplerState.PointClamp, DepthStencilState.None, RasterizerState.CullCounterClockwise, null);
			DrawTheContent(spriteBatch);
			spriteBatch.End();
			device.SetRenderTarget(_target);
			device.Clear(Color.Transparent);
			spriteBatch.Begin(SpriteSortMode.Immediate, BlendState.AlphaBlend, SamplerState.PointClamp, DepthStencilState.None, RasterizerState.CullCounterClockwise, null);
			effectPass.Apply();
			int num = 2;
			int num2 = num * 2;
			for (int i = -num2; i <= num2; i += num)
			{
				for (int j = -num2; j <= num2; j += num)
				{
					if (Math.Abs(i) + Math.Abs(j) == num2)
					{
						spriteBatch.Draw(_helperTarget, new Vector2(i, j), Color.Black);
					}
				}
			}
			num2 = num;
			for (int k = -num2; k <= num2; k += num)
			{
				for (int l = -num2; l <= num2; l += num)
				{
					if (Math.Abs(k) + Math.Abs(l) == num2)
					{
						spriteBatch.Draw(_helperTarget, new Vector2(k, l), _borderColor);
					}
				}
			}
			pixelShader.CurrentTechnique.Passes[0].Apply();
			spriteBatch.Draw(_helperTarget, Vector2.Zero, Color.White);
			spriteBatch.End();
			device.SetRenderTarget(null);
			_wasPrepared = true;
		}

		internal abstract void DrawTheContent(SpriteBatch spriteBatch);
	}
}
