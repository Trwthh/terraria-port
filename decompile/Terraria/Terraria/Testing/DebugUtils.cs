using Microsoft.Xna.Framework;
using Terraria.ID;
using Terraria.Testing.ChatCommands;
using Terraria.UI.Chat;

namespace Terraria.Testing
{
	public static class DebugUtils
	{
		private static char[] _slopeIcons = new char[5] { '⬓', '◣', '◢', '◤', '◥' };

		internal static string GetTileDescription(int x, int y)
		{
			Tile tile = Main.tile[x, y];
			if (tile == null)
			{
				return "";
			}
			Point point = (Main.LocalPlayer.Bottom + new Vector2(-8f, 8f)).ToTileCoordinates();
			if (!TileID.Search.TryGetName(tile.type, out var name))
			{
				name = "Unknown";
			}
			if (!WallID.Search.TryGetName(tile.wall, out var name2))
			{
				name2 = "Unknown";
			}
			string text = "   ";
			return text + "Pos: " + x + ", " + y + "\n" + text + "Type: " + tile.type + ((tile.blockType() == 0) ? "" : (" " + _slopeIcons[tile.blockType() - 1])) + " (" + name + ")\n" + text + "Frame: " + tile.frameX + ", " + tile.frameY + "\n" + text + "FrameImportant: " + Main.tileFrameImportant[tile.type].ToString() + "\n" + text + "Liquid: " + tile.liquid + " (" + tile.liquidType() + ")\n" + text + "Wall: " + tile.wall + " (" + name2 + ")\n" + text + "Compare Spot: " + point.X + ", " + point.Y + "\n" + text + "Chunk: " + x / 200 + ", " + y / 150 + "\n" + text + "Paints: " + tile.color() + (tile.fullbrightBlock() ? " fullbright" : "") + (tile.invisibleBlock() ? " echo" : "") + ", " + tile.wallColor() + (tile.fullbrightWall() ? " fullbright" : "") + (tile.invisibleWall() ? " echo" : "") + "\n" + text + "Light: " + Lighting.GetColor(x, y);
		}

		public static void QuickSPMessage(string message)
		{
			ChatManager.DebugCommands.Process(new DebugMessage((byte)Main.myPlayer, message));
		}
	}
}
