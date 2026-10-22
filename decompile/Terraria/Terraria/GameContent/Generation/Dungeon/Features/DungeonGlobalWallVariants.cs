using System;
using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Terraria.Utilities;

namespace Terraria.GameContent.Generation.Dungeon.Features
{
	public class DungeonGlobalWallVariants : GlobalDungeonFeature
	{
		public DungeonGlobalWallVariants(DungeonFeatureSettings settings)
			: base(settings)
		{
			DungeonCrawler.CurrentDungeonData.dungeonFeatures.Add(this);
		}

		public override bool GenerateFeature(DungeonData data)
		{
			generated = false;
			WallVariants(data);
			generated = true;
			return true;
		}

		public void WallVariants(DungeonData data)
		{
			UnifiedRandom genRand = WorldGen.genRand;
			int[] wallVariants = data.wallVariants;
			int num = wallVariants.Length;
			for (int i = 0; i < 5; i++)
			{
				for (int j = 0; j < num; j++)
				{
					int num2 = genRand.Next(40, 240);
					int num3 = genRand.Next(data.dungeonBounds.Left, data.dungeonBounds.Right);
					int num4 = genRand.Next(data.dungeonBounds.Top, data.dungeonBounds.Bottom);
					for (int k = num3 - num2; k < num3 + num2; k++)
					{
						for (int l = num4 - num2; l < num4 + num2; l++)
						{
							if (!((double)l <= Main.worldSurface) && WorldGen.InWorld(k, l, 2))
							{
								int num5 = Math.Abs(num3 - k);
								int num6 = Math.Abs(num4 - l);
								if (!(Math.Sqrt(num5 * num5 + num6 * num6) >= (double)((float)num2 * 0.4f)) && Main.wallDungeon[Main.tile[k, l].wall])
								{
									SpreadWallDungeon(data, k, l, (ushort)wallVariants[j]);
								}
							}
						}
					}
				}
			}
		}

		public void SpreadWallDungeon(DungeonData data, int x, int y, ushort wallType, bool dungeonWallOnly = true)
		{
			if (!WorldGen.InWorld(x, y))
			{
				return;
			}
			ushort num = wallType;
			List<Point> list = new List<Point>();
			List<Point> list2 = new List<Point>();
			HashSet<Point> hashSet = new HashSet<Point>();
			list2.Add(new Point(x, y));
			while (list2.Count > 0)
			{
				list.Clear();
				list.AddRange(list2);
				list2.Clear();
				while (list.Count > 0)
				{
					Point item = list[0];
					if (!WorldGen.InWorld(item.X, item.Y, 1))
					{
						list.Remove(item);
						continue;
					}
					hashSet.Add(item);
					list.Remove(item);
					Tile tile = Main.tile[item.X, item.Y];
					if (tile.wall == 0 || tile.wall == num || tile.wall == 244 || tile.wall == 62 || !data.CanGenerateFeatureAt(this, item.X, item.Y))
					{
						continue;
					}
					if (data.dungeonEntrance.Bounds.Contains(item.X, item.Y))
					{
						if (tile.wall != data.dungeonEntrance.settings.StyleData.BrickWallType)
						{
							continue;
						}
					}
					else if (dungeonWallOnly && tile.wall != data.genVars.brickWallType)
					{
						continue;
					}
					if (!WorldGen.SolidTile(item.X, item.Y))
					{
						tile.wall = num;
						Point item2 = new Point(item.X - 1, item.Y);
						if (!hashSet.Contains(item2))
						{
							list2.Add(item2);
						}
						item2 = new Point(item.X + 1, item.Y);
						if (!hashSet.Contains(item2))
						{
							list2.Add(item2);
						}
						item2 = new Point(item.X, item.Y - 1);
						if (!hashSet.Contains(item2))
						{
							list2.Add(item2);
						}
						item2 = new Point(item.X, item.Y + 1);
						if (!hashSet.Contains(item2))
						{
							list2.Add(item2);
						}
					}
					else if (tile.active())
					{
						tile.wall = num;
					}
				}
			}
		}
	}
}
