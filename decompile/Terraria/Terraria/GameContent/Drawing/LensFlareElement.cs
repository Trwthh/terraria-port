using System;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;

namespace Terraria.GameContent.Drawing
{
	public struct LensFlareElement
	{
		public Asset<Texture2D> Texture;

		public int RepeatTimes;

		public float ScaleStart;

		public float ScaleOverIndex;

		public float DistanceStart;

		public float DistanceAlongIndex;

		public Color Color;

		public float IntensityOverIndex;

		public float Rotation;

		public void Draw(SpriteBatch spriteBatch, Vector2 sunPosition, Vector2 screenCenterPosition, float intensity)
		{
			if (intensity == 0f)
			{
				return;
			}
			Player localPlayer = Main.LocalPlayer;
			int availableAdvancedShadowsCount = localPlayer.availableAdvancedShadowsCount;
			Vector2 v = localPlayer.GetAdvancedShadow(0).Position - localPlayer.GetAdvancedShadow(Math.Min(4, availableAdvancedShadowsCount - 1)).Position;
			float num = Vector2.Dot(v.SafeNormalize(Vector2.UnitX), (sunPosition - screenCenterPosition).SafeNormalize(-Vector2.UnitY)) * v.Length();
			for (int i = 0; i < RepeatTimes; i++)
			{
				float scale = ScaleStart + ScaleOverIndex * (float)i;
				Color color = Color * (1f + IntensityOverIndex * (float)i) * intensity;
				float num2 = DistanceStart + DistanceAlongIndex * (float)i;
				num2 += num * -0.0002f;
				num2 %= 1f;
				Vector2 position = Vector2.Lerp(sunPosition, screenCenterPosition, num2 * 2f);
				float num3 = (screenCenterPosition - sunPosition).ToRotation() + Rotation;
				if (Rotation == 0f)
				{
					num3 += Main.screenPosition.Y * 0.001f;
				}
				spriteBatch.Draw(Texture.Value, position, null, color, num3, Texture.Size() / 2f, scale, SpriteEffects.None, 0f);
			}
		}
	}
}
