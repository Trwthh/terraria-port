using System.Collections.Generic;
using Microsoft.Xna.Framework;

namespace Terraria.GameContent
{
	public static class NearbyChests
	{
		private static List<PositionedChest> _scratch = new List<PositionedChest>();

		public static List<PositionedChest> GetChestsInRangeOf(Vector2 position, float range = 0f)
		{
			if (range <= 0f)
			{
				range = 600f;
			}
			List<PositionedChest> scratch = _scratch;
			scratch.Clear();
			for (int i = 0; i < 8000; i++)
			{
				Chest chest = Main.chest[i];
				if (chest != null)
				{
					Vector2 vector = new Vector2(chest.x * 16 + 16, chest.y * 16 + 16);
					if (!(Vector2.Distance(vector, position) > range))
					{
						scratch.Add(new PositionedChest(chest, vector));
					}
				}
			}
			return scratch;
		}

		public static List<PositionedChest> GetBanksInRangeOf(Player player, float range = 0f)
		{
			if (range <= 0f)
			{
				range = 600f;
			}
			List<PositionedChest> scratch = _scratch;
			scratch.Clear();
			int num = (int)(range / 16f + 2f);
			Point point = player.Center.ToTileCoordinates();
			Rectangle rectangle = new Rectangle(point.X - num, point.Y - num, num * 2 + 1, num * 2 + 1);
			for (int i = 0; i < 1000; i++)
			{
				Projectile projectile = Main.projectile[i];
				if (!projectile.active)
				{
					continue;
				}
				int containerIndex = -1;
				if (projectile.TryGetContainerIndex(out containerIndex))
				{
					Vector2 vec = projectile.Hitbox.ClosestPointInRect(player.Center);
					if (rectangle.Contains(vec.ToTileCoordinates()) && ContainerIndexToPlayerBank(player, containerIndex, out var bank) && !scratch.Contains(bank))
					{
						scratch.Add(new PositionedChest(bank, projectile.Center));
					}
				}
			}
			for (int j = rectangle.Left; j < rectangle.Right; j++)
			{
				for (int k = rectangle.Top; k < rectangle.Bottom; k++)
				{
					if (WorldGen.InWorld(j, k))
					{
						int container = 0;
						switch (Main.tile[j, k].type)
						{
						case 29:
							container = -2;
							break;
						case 97:
							container = -3;
							break;
						case 463:
							container = -4;
							break;
						case 491:
							container = -5;
							break;
						}
						if (ContainerIndexToPlayerBank(player, container, out var bank2) && !scratch.Contains(bank2))
						{
							scratch.Add(new PositionedChest(bank2, new Vector2(j * 16 + 16, k * 16 + 16)));
						}
					}
				}
			}
			return scratch;
		}

		private static bool Contains(this List<PositionedChest> list, Chest chest)
		{
			foreach (PositionedChest item in list)
			{
				if (item.chest == chest)
				{
					return true;
				}
			}
			return false;
		}

		private static bool ContainerIndexToPlayerBank(Player player, int container, out Chest bank)
		{
			bank = null;
			switch (container)
			{
			case -2:
				bank = player.bank;
				return true;
			case -3:
				bank = player.bank2;
				return true;
			case -4:
				bank = player.bank3;
				return true;
			case -5:
			{
				bank = player.bank4;
				for (int i = 0; i < 58; i++)
				{
					if (player.inventory[i].stack > 0 && player.inventory[i].type == 5325)
					{
						return false;
					}
				}
				return true;
			}
			default:
				return false;
			}
		}
	}
}
