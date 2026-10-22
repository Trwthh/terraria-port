using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;

namespace Terraria.Graphics
{
	public class Camera
	{
		public Vector2 UnscaledPosition => Main.screenPosition;

		public Vector2 UnscaledSize => new Vector2(Main.screenWidth, Main.screenHeight);

		public Vector2 ScaledPosition => UnscaledPosition + GameViewMatrix.Translation;

		public Vector2 ScaledSize => UnscaledSize - GameViewMatrix.Translation * 2f;

		public float BiggerScaledAxis
		{
			get
			{
				Vector2 scaledSize = ScaledSize;
				if (!(scaledSize.X > scaledSize.Y))
				{
					return scaledSize.Y;
				}
				return scaledSize.X;
			}
		}

		public float SmallerScaledAxis
		{
			get
			{
				Vector2 scaledSize = ScaledSize;
				if (!(scaledSize.X < scaledSize.Y))
				{
					return scaledSize.Y;
				}
				return scaledSize.X;
			}
		}

		public RasterizerState Rasterizer => Main.Rasterizer;

		public SamplerState Sampler => Main.DefaultSamplerState;

		public SpriteViewMatrix GameViewMatrix => Main.GameViewMatrix;

		public SpriteBatch SpriteBatch => Main.spriteBatch;

		public Vector2 Center => UnscaledPosition + UnscaledSize * 0.5f;
	}
}
