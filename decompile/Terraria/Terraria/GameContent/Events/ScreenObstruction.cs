using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.Testing;

namespace Terraria.GameContent.Events
{
	public class ScreenObstruction
	{
		public static float lastSpeed = 0.1f;

		public static float screenObstruction;

		public static void Update(SceneState sceneState, SceneMetrics metrics)
		{
			float num = 0f;
			float amount = 0.1f;
			if (metrics.PerspectivePlayer.insideUnbreakableWalls)
			{
				int progressPlayerCanSafelyMatch = DangerousDungeonCurse.GetProgressPlayerCanSafelyMatch();
				int num2 = DangerousDungeonCurse.GetProgressPlayerNeedsToMatch(metrics.PerspectivePlayer) - progressPlayerCanSafelyMatch;
				if (num2 > 0)
				{
					float max = 0.9f;
					if (DebugOptions.devLightTilesCheat)
					{
						max = 0.25f;
					}
					num = Utils.Clamp(0.4f * (float)num2, 0f, max);
					amount = (lastSpeed = 0.01f);
				}
			}
			if (metrics.PerspectivePlayer.headcovered)
			{
				num = 0.95f;
				amount = (lastSpeed = 0.3f);
			}
			if (num == 0f && screenObstruction != 0f)
			{
				amount = lastSpeed;
			}
			else
			{
				lastSpeed = amount;
			}
			sceneState.MoveTowards(ref screenObstruction, num, amount);
		}

		public static void Draw(SpriteBatch spriteBatch)
		{
			if (screenObstruction != 0f)
			{
				Color color = Color.Black * screenObstruction;
				int num = TextureAssets.Extra[49].Width();
				int num2 = 10;
				Rectangle rect = Main.SceneMetrics.PerspectivePlayer.getRect();
				rect.Inflate((num - rect.Width) / 2, (num - rect.Height) / 2 + num2 / 2);
				rect.Offset(-(int)Main.screenPosition.X, -(int)Main.screenPosition.Y + (int)Main.player[Main.myPlayer].gfxOffY - num2);
				Rectangle destinationRectangle = Rectangle.Union(new Rectangle(0, 0, 1, 1), new Rectangle(rect.Right - 1, rect.Top - 1, 1, 1));
				Rectangle destinationRectangle2 = Rectangle.Union(new Rectangle(Main.screenWidth - 1, 0, 1, 1), new Rectangle(rect.Right, rect.Bottom - 1, 1, 1));
				Rectangle destinationRectangle3 = Rectangle.Union(new Rectangle(Main.screenWidth - 1, Main.screenHeight - 1, 1, 1), new Rectangle(rect.Left, rect.Bottom, 1, 1));
				Rectangle destinationRectangle4 = Rectangle.Union(new Rectangle(0, Main.screenHeight - 1, 1, 1), new Rectangle(rect.Left - 1, rect.Top, 1, 1));
				spriteBatch.Draw(TextureAssets.MagicPixel.Value, destinationRectangle, new Rectangle(0, 0, 1, 1), color);
				spriteBatch.Draw(TextureAssets.MagicPixel.Value, destinationRectangle2, new Rectangle(0, 0, 1, 1), color);
				spriteBatch.Draw(TextureAssets.MagicPixel.Value, destinationRectangle3, new Rectangle(0, 0, 1, 1), color);
				spriteBatch.Draw(TextureAssets.MagicPixel.Value, destinationRectangle4, new Rectangle(0, 0, 1, 1), color);
				spriteBatch.Draw(TextureAssets.Extra[49].Value, rect, color);
			}
		}
	}
}
