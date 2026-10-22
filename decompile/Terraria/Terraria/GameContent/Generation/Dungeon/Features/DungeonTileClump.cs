using System;
using ReLogic.Utilities;
using Terraria.Utilities;

namespace Terraria.GameContent.Generation.Dungeon.Features
{
	public class DungeonTileClump : DungeonFeature
	{
		public DungeonTileClump(DungeonFeatureSettings settings, bool addToFeatures = false)
			: base(settings)
		{
			if (addToFeatures)
			{
				DungeonCrawler.CurrentDungeonData.dungeonFeatures.Add(this);
			}
		}

		public override bool GenerateFeature(DungeonData data, int x, int y)
		{
			generated = false;
			DungeonTileClumpSettings dungeonTileClumpSettings = (DungeonTileClumpSettings)settings;
			if (TileClump(data, x, y, dungeonTileClumpSettings.Strength, dungeonTileClumpSettings.Steps, dungeonTileClumpSettings.TileType, dungeonTileClumpSettings.WallType, generating: true))
			{
				generated = true;
				return true;
			}
			return false;
		}

		public bool TileClump(DungeonData data, int i, int j, double strength, int steps, int tileType = -1, int wallType = -1, bool generating = false)
		{
			DungeonTileClumpSettings dungeonTileClumpSettings = (DungeonTileClumpSettings)settings;
			UnifiedRandom unifiedRandom = new UnifiedRandom(dungeonTileClumpSettings.RandomSeed);
			ushort? onlyReplaceThisTileType = dungeonTileClumpSettings.OnlyReplaceThisTileType;
			ushort? onlyReplaceThisWallType = dungeonTileClumpSettings.OnlyReplaceThisWallType;
			double num = strength;
			double num2 = steps;
			Vector2D vector2D = default(Vector2D);
			vector2D.X = i;
			vector2D.Y = j;
			Vector2D vector2D2 = default(Vector2D);
			vector2D2.X = (double)unifiedRandom.Next(-10, 11) * 0.1;
			vector2D2.Y = (double)unifiedRandom.Next(-10, 11) * 0.1;
			Bounds.SetBounds(i, j, i, j);
			while (num > 0.0 && num2 > 0.0)
			{
				if (vector2D.Y < 0.0 && num2 > 0.0 && tileType == 59)
				{
					num2 = 0.0;
				}
				num = strength * (num2 / (double)steps);
				num2 -= 1.0;
				int num3 = Math.Max(0, Math.Min(Main.maxTilesX, (int)(vector2D.X - num * 0.5)));
				int num4 = Math.Max(0, Math.Min(Main.maxTilesX, (int)(vector2D.X + num * 0.5)));
				int num5 = Math.Max(0, Math.Min(Main.maxTilesY, (int)(vector2D.Y - num * 0.5)));
				int num6 = Math.Max(0, Math.Min(Main.maxTilesY, (int)(vector2D.Y + num * 0.5)));
				Bounds.UpdateBounds(num3, num4, num5, num6);
				if (generating)
				{
					for (int k = num3; k < num4; k++)
					{
						for (int l = num5; l < num6; l++)
						{
							if (!(Math.Abs((double)k - vector2D.X) + Math.Abs((double)l - vector2D.Y) < strength * 0.5 * (1.0 + (double)unifiedRandom.Next(-10, 11) * 0.015)) || (dungeonTileClumpSettings.AreaToGenerateIn != null && !dungeonTileClumpSettings.AreaToGenerateIn.Contains(k, l)))
							{
								continue;
							}
							Tile tile = Main.tile[k, l];
							if (tile.active() && (tile.actuator() || tile.anyWire()))
							{
								continue;
							}
							if (tileType != -1 && tile.active())
							{
								if (onlyReplaceThisTileType.HasValue && tile.type != onlyReplaceThisTileType.Value)
								{
									continue;
								}
								Main.tile[k, l].type = (ushort)tileType;
								WorldGen.paintTile(k, l, 0, broadCast: false, paintEffects: false);
							}
							if (wallType != -1 && (!onlyReplaceThisWallType.HasValue || tile.wall == onlyReplaceThisWallType.Value))
							{
								Main.tile[k, l].wall = (ushort)wallType;
								WorldGen.paintWall(k, l, 0, broadCast: false, paintEffects: false);
							}
						}
					}
				}
				vector2D += vector2D2;
				vector2D2.X += (float)unifiedRandom.Next(-10, 11) * 0.05f;
				if (vector2D2.X > 1.0)
				{
					vector2D2.X = 1.0;
				}
				if (vector2D2.X < -1.0)
				{
					vector2D2.X = -1.0;
				}
			}
			Bounds.CalculateHitbox();
			return true;
		}
	}
}
