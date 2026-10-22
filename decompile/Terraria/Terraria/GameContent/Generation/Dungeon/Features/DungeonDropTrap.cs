using Terraria.ID;
using Terraria.Utilities;

namespace Terraria.GameContent.Generation.Dungeon.Features
{
	public class DungeonDropTrap : DungeonFeature
	{
		public DungeonDropTrap(DungeonFeatureSettings settings, bool addToFeatures = true)
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
			if (DropTrap(data, x, y))
			{
				generated = true;
				return true;
			}
			return false;
		}

		public override bool CanGenerateFeatureAt(DungeonData data, IDungeonFeature feature, int x, int y)
		{
			return false;
		}

		public bool DropTrap(DungeonData data, int i, int j)
		{
			UnifiedRandom genRand = WorldGen.genRand;
			DungeonDropTrapSettings dungeonDropTrapSettings = (DungeonDropTrapSettings)settings;
			bool num = dungeonDropTrapSettings.StyleData.Style == 0;
			ushort num2 = (num ? data.genVars.dungeonStyle.BrickTileType : dungeonDropTrapSettings.StyleData.BrickTileType);
			ushort num3 = (num ? data.genVars.dungeonStyle.BrickWallType : dungeonDropTrapSettings.StyleData.BrickWallType);
			byte? b = (num ? data.genVars.dungeonStyle.TilePaintColor : dungeonDropTrapSettings.StyleData.TilePaintColor);
			if (!num)
			{
				_ = dungeonDropTrapSettings.StyleData.WallPaintColor;
			}
			else
			{
				_ = data.genVars.dungeonStyle.WallPaintColor;
			}
			ushort num4 = num2;
			int num5 = -1;
			int num6 = -1;
			switch (dungeonDropTrapSettings.DropTrapType)
			{
			default:
				num5 = 53;
				num4 = 396;
				break;
			case DungeonDropTrapType.Silt:
				num5 = 123;
				break;
			case DungeonDropTrapType.Slush:
				num5 = 224;
				num4 = 147;
				break;
			case DungeonDropTrapType.Lava:
				num6 = 1;
				break;
			}
			int num7 = 6;
			int num8 = 4;
			int num9 = 25;
			int k = j;
			if (!WorldGen.InWorld(i, k, num9))
			{
				return false;
			}
			for (; !Main.tile[i, k].active() && k < Main.UnderworldLayer; k++)
			{
			}
			if (!WorldGen.InWorld(i, k, num9))
			{
				return false;
			}
			if (Main.tile[i, k - 1].active())
			{
				return false;
			}
			if (!Main.tileSolid[Main.tile[i, k].type] || Main.tile[i, k].halfBrick() || Main.tile[i, k].topSlope())
			{
				return false;
			}
			if ((Main.tile[i, k].type != num5 && Main.tile[i, k].type != num4 && Main.tile[i, k].type != num2) || Main.tile[i, k].wall != num3)
			{
				return false;
			}
			k--;
			_ = Main.tile[i, k];
			int num10 = -1;
			int num11 = genRand.Next(6, 12);
			int num12 = genRand.Next(6, 14);
			for (int l = i - num9; l <= i + num9; l++)
			{
				for (int m = k - num9; m < k + num9; m++)
				{
					Tile tile = Main.tile[l, m];
					if (tile.wire())
					{
						return false;
					}
					if (TileID.Sets.BasicChest[tile.type])
					{
						return false;
					}
				}
			}
			for (int num13 = k; num13 > k - 30; num13--)
			{
				if (Main.tile[i, num13].active())
				{
					if (Main.tile[i, num13].type == num2)
					{
						num10 = num13;
						break;
					}
					return false;
				}
			}
			if (num10 <= -1)
			{
				return false;
			}
			if (k - num10 < num12 + num8)
			{
				return false;
			}
			int num14 = 0;
			_ = (k + num10) / 2;
			for (int n = i - num11; n <= i + num11; n++)
			{
				for (int num15 = num10 - num12; num15 <= num10; num15++)
				{
					Tile tile2 = Main.tile[n, num15];
					if (tile2.active() && Main.tileSolid[tile2.type])
					{
						num14++;
					}
				}
			}
			double num16 = (double)((num11 * 2 + 1) * (num12 + 1)) * 0.75;
			if ((double)num14 < num16)
			{
				return false;
			}
			Bounds.SetBounds(i - num11 - 1, num10 - num12, i + num11 + 1, num10 + 20);
			Bounds.CalculateHitbox();
			if (!data.CanGenerateFeatureInArea(this, Bounds))
			{
				return false;
			}
			Bounds = new DungeonBounds();
			for (int num17 = i - num11 - 1; num17 <= i + num11 + 1; num17++)
			{
				for (int num18 = num10 - num12; num18 <= num10; num18++)
				{
					Tile tile3 = Main.tile[num17, num18];
					bool flag = false;
					if (tile3.active() && Main.tileSolid[tile3.type])
					{
						flag = true;
					}
					if (num18 == num10)
					{
						tile3.slope(0);
						tile3.halfBrick(halfBrick: false);
						if (!flag)
						{
							tile3.active(active: true);
							tile3.type = num2;
							if (b.HasValue)
							{
								tile3.color(b.Value);
							}
						}
						continue;
					}
					if (num18 == num10 - num12)
					{
						tile3.ClearTile();
						tile3.active(active: true);
						if (flag && Main.tile[num17, num18 - 1].active() && Main.tileSolid[Main.tile[num17, num18 - 1].type])
						{
							tile3.type = num4;
						}
						else
						{
							tile3.type = num2;
						}
						if (b.HasValue)
						{
							tile3.color(b.Value);
						}
						continue;
					}
					if (num17 == i - num11 - 1 || num17 == i + num11 + 1)
					{
						if (!flag)
						{
							tile3.ClearTile();
							tile3.active(active: true);
							tile3.type = num2;
							if (b.HasValue)
							{
								tile3.color(b.Value);
							}
						}
						else
						{
							tile3.slope(0);
							tile3.halfBrick(halfBrick: false);
						}
						continue;
					}
					tile3.ClearTile();
					if (num6 > -1)
					{
						tile3.liquid = byte.MaxValue;
						tile3.liquidType(num6);
						continue;
					}
					tile3.active(active: true);
					tile3.type = (ushort)num5;
					if (b.HasValue)
					{
						tile3.color(b.Value);
					}
				}
			}
			for (int num19 = (int)((double)num10 - (double)num12 * 0.6600000262260437); (double)num19 <= (double)num10 - (double)num12 * 0.33000001311302185; num19++)
			{
				if ((double)num19 < (double)num10 - (double)num12 * 0.4000000059604645)
				{
					if (Main.tile[i - num11 - 2, num19].bottomSlope())
					{
						Main.tile[i - num11 - 2, num19].slope(0);
					}
				}
				else if ((double)num19 > (double)num10 - (double)num12 * 0.6000000238418579)
				{
					if (Main.tile[i - num11 - 2, num19].topSlope())
					{
						Main.tile[i - num11 - 2, num19].slope(0);
					}
					Main.tile[i - num11 - 2, num19].halfBrick(halfBrick: false);
				}
				else
				{
					Main.tile[i - num11 - 2, num19].halfBrick(halfBrick: false);
					Main.tile[i - num11 - 2, num19].slope(0);
				}
				if (!Main.tile[i - num11 - 2, num19].active() || !Main.tileSolid[Main.tile[i - num11 - 2, num19].type])
				{
					Main.tile[i - num11 - 2, num19].active(active: true);
					Main.tile[i - num11 - 2, num19].type = num2;
					if (b.HasValue)
					{
						Main.tile[i - num11 - 2, num19].color(b.Value);
					}
				}
				if (!Main.tile[i + num11 + 2, num19].active() || !Main.tileSolid[Main.tile[i + num11 + 2, num19].type])
				{
					Main.tile[i + num11 + 2, num19].active(active: true);
					Main.tile[i + num11 + 2, num19].type = num2;
					if (b.HasValue)
					{
						Main.tile[i + num11 + 2, num19].color(b.Value);
					}
				}
			}
			for (int num20 = num10 - num12; num20 <= num10; num20++)
			{
				Main.tile[i - num11 - 2, num20].slope(0);
				Main.tile[i - num11 - 2, num20].halfBrick(halfBrick: false);
				Main.tile[i - num11 - 1, num20].slope(0);
				Main.tile[i - num11 - 1, num20].halfBrick(halfBrick: false);
				Main.tile[i - num11 + 1, num20].slope(0);
				Main.tile[i - num11 + 1, num20].halfBrick(halfBrick: false);
				Main.tile[i - num11 + 2, num20].slope(0);
				Main.tile[i - num11 + 2, num20].halfBrick(halfBrick: false);
			}
			for (int num21 = i - num11 - 1; num21 < i + num11 + 1; num21++)
			{
				int num22 = k - num12 - 1;
				if (Main.tile[num21, num22].bottomSlope())
				{
					Main.tile[num21, num22].slope(0);
				}
				Main.tile[num21, num22].halfBrick(halfBrick: false);
			}
			WorldGen.KillTile(i - 2, k);
			WorldGen.KillTile(i - 1, k);
			WorldGen.KillTile(i + 1, k);
			WorldGen.KillTile(i + 2, k);
			WorldGen.PlaceTile(i, k, 135, mute: true, forced: false, -1, 7);
			for (int num23 = i - num11; num23 <= i + num11; num23++)
			{
				int num24 = k;
				if ((float)num23 < (float)i - (float)num11 * 0.8f || (float)num23 > (float)i + (float)num11 * 0.8f)
				{
					num24 = k - 3;
				}
				else if ((float)num23 < (float)i - (float)num11 * 0.6f || (float)num23 > (float)i + (float)num11 * 0.6f)
				{
					num24 = k - 2;
				}
				else if ((float)num23 < (float)i - (float)num11 * 0.4f || (float)num23 > (float)i + (float)num11 * 0.4f)
				{
					num24 = k - 1;
				}
				for (int num25 = num10; num25 <= k; num25++)
				{
					if (num23 == i && num25 <= k)
					{
						Main.tile[i, num25].wire(wire: true);
					}
					if (Main.tile[num23, num25].active() && Main.tileSolid[Main.tile[num23, num25].type])
					{
						if (num25 < num10 + num7 - 4)
						{
							Main.tile[num23, num25].actuator(actuator: true);
							Main.tile[num23, num25].wire(wire: true);
						}
						else if (num25 < num24)
						{
							WorldGen.KillTile(num23, num25);
						}
					}
				}
			}
			int num26 = k;
			for (int num27 = i - num11; num27 <= i + num11; num27++)
			{
				for (int num28 = num26; num28 < Main.UnderworldLayer; num28++)
				{
					if (num28 >= num26)
					{
						Tile tile4 = Main.tile[num27, num28];
						if (tile4.active() && !TileID.Sets.Platforms[tile4.type] && Main.tileSolid[tile4.type])
						{
							num26 = num28;
							break;
						}
					}
				}
			}
			Bounds.SetBounds(i - num11 - 1, num10 - num12, i + num11 + 1, num26);
			Bounds.CalculateHitbox();
			return true;
		}
	}
}
