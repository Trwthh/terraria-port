using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;

namespace Terraria.GameContent.Events
{
	public class ScreenDarkness
	{
		public static float screenObstruction;

		public static Color frontColor = new Color(0, 0, 120);

		public static void Update(SceneState sceneState, SceneMetrics metrics)
		{
			float target = 0f;
			float amount = 1f / 60f;
			Vector2 center = metrics.Center;
			for (int i = 0; i < Main.maxNPCs; i++)
			{
				if (Main.npc[i].active && Main.npc[i].type == 370 && Main.npc[i].Distance(center) < 3000f && (Main.npc[i].ai[0] >= 10f || (Main.npc[i].ai[0] == 9f && Main.npc[i].ai[2] > 120f)))
				{
					target = 0.95f;
					frontColor = new Color(0, 0, 120) * 0.3f;
					amount = 0.03f;
				}
				if (Main.npc[i].active && Main.npc[i].type == 113 && Main.npc[i].Distance(center) < 3000f)
				{
					float num = Utils.Remap(Main.npc[i].Distance(center), 2000f, 3000f, 1f, 0f);
					target = Main.npc[i].localAI[1] * num;
					amount = 1f;
					frontColor = Color.Black;
				}
			}
			sceneState.MoveTowards(ref screenObstruction, target, amount);
		}

		public static void DrawBack(SpriteBatch spriteBatch)
		{
			if (screenObstruction != 0f)
			{
				Color color = Color.Black * screenObstruction;
				spriteBatch.Draw(TextureAssets.MagicPixel.Value, new Rectangle(-2, -2, Main.screenWidth + 4, Main.screenHeight + 4), new Rectangle(0, 0, 1, 1), color);
			}
		}

		public static void DrawFront(SpriteBatch spriteBatch)
		{
			if (screenObstruction != 0f)
			{
				Color color = frontColor * screenObstruction;
				spriteBatch.Draw(TextureAssets.MagicPixel.Value, new Rectangle(-2, -2, Main.screenWidth + 4, Main.screenHeight + 4), new Rectangle(0, 0, 1, 1), color);
			}
		}
	}
}
