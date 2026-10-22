using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;

namespace Terraria.GameContent.Drawing
{
	public class EmptyHorizonRenderer : IHorizonRenderer
	{
		public void DrawHorizon()
		{
			if (!Main.ShouldDrawSurfaceBackground())
			{
				return;
			}
			foreach (BackgroundGradientDrawer backgroundDrawer in SunGradients.BackgroundDrawers)
			{
				backgroundDrawer.Draw();
			}
		}

		public void DrawLensFlare()
		{
		}

		public void ModifyHorizonLight(ref Color color)
		{
		}

		public void DrawSun(Vector2 sunPosition)
		{
		}

		public void DrawSurfaceLayer(int layerIndex)
		{
		}

		public void CloudsStart()
		{
		}

		public void DrawCloud(float globalCloudAlpha, Cloud theCloud, int cloudPass, float cY)
		{
			Asset<Texture2D> asset = TextureAssets.Cloud[theCloud.type];
			Color color = theCloud.cloudColor(Main.ColorOfTheSkies);
			if (cloudPass == 1)
			{
				float num = theCloud.scale * 0.8f;
				float num2 = (theCloud.scale + 1f) / 2f * 0.9f;
				color.R = (byte)((float)(int)color.R * num);
				color.G = (byte)((float)(int)color.G * num2);
			}
			if (Main.atmo < 1f)
			{
				color *= Main.atmo;
			}
			Main.spriteBatch.Draw(asset.Value, new Vector2(theCloud.position.X, cY) + asset.Size() / 2f, null, color * globalCloudAlpha, theCloud.rotation, asset.Size() / 2f, theCloud.scale, theCloud.spriteDir, 0f);
		}

		public void CloudsEnd()
		{
		}
	}
}
