using Microsoft.Xna.Framework;
using Terraria.DataStructures;
using Terraria.GameContent;
using Terraria.Localization;
using Terraria.UI;

namespace Terraria.Map
{
	public class DigtoiseMapLayer : IMapLayer
	{
		public void Draw(ref MapOverlayDrawContext context, ref string text)
		{
			Player localPlayer = Main.LocalPlayer;
			for (int i = 0; i < 1000; i++)
			{
				Projectile projectile = Main.projectile[i];
				if (projectile.active && projectile.owner == localPlayer.whoAmI && (projectile.type == 1098 || projectile.type == 1123))
				{
					Vector2 vector = new Vector2(0.5f, 0f);
					Vector2 vector2 = projectile.Bottom / 16f;
					if (context.Draw(TextureAssets.Extra[305].Value, vector2 + vector, new SpriteFrame(2, 1, (byte)((projectile.spriteDirection <= 0) ? 1u : 0u), 0), Alignment.Bottom).IsMouseOver)
					{
						text = Language.GetTextValue("UI.Digtoise");
					}
					break;
				}
			}
		}
	}
}
