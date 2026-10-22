using Microsoft.Xna.Framework;
using Terraria.DataStructures;
using Terraria.GameContent;
using Terraria.Localization;
using Terraria.UI;

namespace Terraria.Map
{
	public class TeamBasedSpawnMapLayer : IMapLayer
	{
		public void Draw(ref MapOverlayDrawContext context, ref string text)
		{
			if (Main.teamBasedSpawnsSeed)
			{
				int team = Main.LocalPlayer.team;
				Point spawnPoint = Point.Zero;
				if (ExtraSpawnPointManager.TryGetExtraSpawnPointForTeam(team, out spawnPoint) && context.Draw(TextureAssets.Extra[282].Value, spawnPoint.ToVector2(), new SpriteFrame(6, 1, (byte)team, 0), Alignment.Bottom).IsMouseOver)
				{
					text = Language.GetTextValue("UI.TeamSpawnPoint");
				}
			}
		}
	}
}
