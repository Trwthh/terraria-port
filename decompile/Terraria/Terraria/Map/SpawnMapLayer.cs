using Microsoft.Xna.Framework;
using Terraria.GameContent;
using Terraria.Localization;
using Terraria.UI;

namespace Terraria.Map
{
	public class SpawnMapLayer : IMapLayer
	{
		public void Draw(ref MapOverlayDrawContext context, ref string text)
		{
			Player localPlayer = Main.LocalPlayer;
			Vector2 position = new Vector2((float)localPlayer.SpawnX + 0.5f, localPlayer.SpawnY);
			Vector2 position2 = new Vector2((float)Main.spawnTileX + 0.5f, Main.spawnTileY);
			if (!Main.teamBasedSpawnsSeed && context.Draw(TextureAssets.SpawnPoint.Value, position2, Alignment.Bottom).IsMouseOver)
			{
				text = Language.GetTextValue("UI.SpawnPoint");
			}
			if (localPlayer.SpawnX != -1 && context.Draw(TextureAssets.SpawnBed.Value, position, Alignment.Bottom).IsMouseOver)
			{
				text = Language.GetTextValue("UI.SpawnBed");
			}
		}
	}
}
