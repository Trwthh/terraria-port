using Microsoft.Xna.Framework;
using Terraria.DataStructures;
using Terraria.GameContent.Generation.Dungeon.Features;
using Terraria.Utilities;
using Terraria.WorldBuilding;

namespace Terraria.GameContent.Generation.Dungeon.Entrances
{
	public class TowerDungeonEntrance : DungeonEntrance
	{
		public TowerDungeonEntrance(DungeonEntranceSettings settings)
			: base(settings)
		{
		}

		public override void CalculateEntrance(DungeonData data, int x, int y)
		{
			calculated = false;
			TowerEntrance(data, x, y, generating: false);
			calculated = true;
		}

		public override bool GenerateEntrance(DungeonData data, int x, int y)
		{
			generated = false;
			TowerEntrance(data, x, y, generating: true);
			generated = true;
			return true;
		}

		public override bool CanGenerateFeatureAt(DungeonData data, IDungeonFeature feature, int x, int y)
		{
			if (feature is DungeonGlobalBookshelves || feature is DungeonGlobalPaintings || feature is DungeonGlobalSpikes)
			{
				return false;
			}
			return base.CanGenerateFeatureAt(data, feature, x, y);
		}

		public void TowerEntrance(DungeonData data, int i, int j, bool generating)
		{
			UnifiedRandom unifiedRandom = new UnifiedRandom(((TowerDungeonEntranceSettings)settings).RandomSeed);
			ushort brickTileType = settings.StyleData.BrickTileType;
			ushort brickWallType = settings.StyleData.BrickWallType;
			byte? tilePaintColor = settings.StyleData.TilePaintColor;
			byte? wallPaintColor = settings.StyleData.WallPaintColor;
			WindowType windowType = WindowType.RegularWindows;
			switch (unifiedRandom.Next(3))
			{
			default:
				windowType = WindowType.RegularWindows;
				break;
			case 1:
				windowType = WindowType.SkeletronMosaic;
				break;
			case 2:
				windowType = WindowType.MoonLordMosaic;
				break;
			}
			bool dungeonEntranceIsBuried = SpecialSeedFeatures.DungeonEntranceIsBuried;
			bool dungeonEntranceIsUnderground = SpecialSeedFeatures.DungeonEntranceIsUnderground;
			bool flag = data.genVars.dungeonSide == DungeonSide.Left;
			if (Main.drunkWorld)
			{
				flag = !flag;
			}
			Bounds.SetBounds(i, j, i, j);
			if (generating)
			{
				int num = 60;
				for (int k = i - num; k < i + num; k++)
				{
					for (int l = j - num; l < j + num; l++)
					{
						if (WorldGen.InWorld(k, l))
						{
							Main.tile[k, l].liquid = 0;
							Main.tile[k, l].lava(lava: false);
							Main.tile[k, l].Clear(TileDataType.Slope);
						}
					}
				}
			}
			int num2 = 5;
			int num3 = 35;
			int num4 = num3 + num2;
			int num5 = 100;
			int num6 = 30;
			int num7 = j - num6;
			int num8 = 30;
			int num9 = 25;
			int num10 = num9 + num2;
			int num11 = 20;
			int num12 = num8 + num11;
			int num13 = 15;
			int num14 = num13 + num2;
			int num15 = 40;
			int num16 = num8 + num11 + num15;
			int minY = num7 - num16;
			int maxY = num7 + 10;
			int m = 10;
			int num17 = 50;
			if (data.Type == DungeonType.DualDungeon)
			{
				num5 = DungeonUtils.GetDualDungeonBrickSupportCutoffY(data) - num7;
			}
			else if (dungeonEntranceIsUnderground)
			{
				num5 = num17 - m + 5;
			}
			if (generating && !dungeonEntranceIsBuried && !dungeonEntranceIsUnderground)
			{
				int x = i - num3 + 1;
				if (flag)
				{
					x = i + num3 - 1;
				}
				WorldUtils.Gen(new Point(x, num7 - 15), new Shapes.Circle(15, 15), Actions.Chain(new Actions.Clear()));
			}
			Bounds.UpdateBounds(i - num4, minY, i + num4 + 1, maxY);
			if (generating)
			{
				int num18 = -5;
				int num19 = num5;
				for (int n = -num4; n <= num4; n++)
				{
					for (int num20 = num18; num20 < num19; num20++)
					{
						int num21 = i + n;
						int num22 = num7 + num20;
						if (!WorldGen.InWorld(num21, num22))
						{
							continue;
						}
						Tile tile = Main.tile[num21, num22];
						bool flag2 = tile.active() && !settings.StyleData.TileIsInStyle(tile.type);
						bool flag3 = !settings.StyleData.WallIsInStyle(tile.wall);
						bool flag4 = DungeonUtils.IsConsideredDungeonWall(tile.wall);
						if (num20 < 0)
						{
							tile.ClearEverything();
						}
						else if (num20 >= 0 && num20 < 5)
						{
							if ((n >= -num4 + num2 && n <= -num4 + num2 * 2 - 1) || (n >= num4 - num2 * 2 + 1 && n <= num4 - num2))
							{
								tile.ClearEverything();
								if (!flag4)
								{
									tile.wall = brickWallType;
									if (wallPaintColor.HasValue)
									{
										tile.wallColor(wallPaintColor.Value);
									}
								}
							}
							else if (!flag4)
							{
								tile.liquid = 0;
								tile.active(active: true);
								tile.type = brickTileType;
								if (tilePaintColor.HasValue)
								{
									tile.color(tilePaintColor.Value);
								}
								if (n != -num4 && n != num4)
								{
									tile.wall = brickWallType;
									if (wallPaintColor.HasValue)
									{
										tile.wallColor(wallPaintColor.Value);
									}
								}
							}
						}
						else if (num20 >= 5 && num20 < 10)
						{
							if (n >= -num4 + num2 && n <= num4 - num2)
							{
								tile.ClearEverything();
								tile.wall = brickWallType;
								if (wallPaintColor.HasValue)
								{
									tile.wallColor(wallPaintColor.Value);
								}
							}
							else if (!flag4)
							{
								tile.liquid = 0;
								tile.active(active: true);
								tile.type = brickTileType;
								if (tilePaintColor.HasValue)
								{
									tile.color(tilePaintColor.Value);
								}
								if (n != -num4 && n != num4)
								{
									tile.wall = brickWallType;
									if (wallPaintColor.HasValue)
									{
										tile.wallColor(wallPaintColor.Value);
									}
								}
							}
						}
						else if ((tile.active() && flag2) || !flag4)
						{
							tile.liquid = 0;
							tile.active(active: true);
							tile.type = brickTileType;
							if (tilePaintColor.HasValue)
							{
								tile.color(tilePaintColor.Value);
							}
							if (n != -num4 && n != num4)
							{
								tile.wall = brickWallType;
								if (wallPaintColor.HasValue)
								{
									tile.wallColor(wallPaintColor.Value);
								}
							}
						}
						else if (flag3)
						{
							tile.liquid = 0;
							if (n != -num4 && n != num4)
							{
								tile.wall = brickWallType;
								if (wallPaintColor.HasValue)
								{
									tile.wallColor(wallPaintColor.Value);
								}
							}
						}
						if (num20 == 1 && (n == -num4 + num2 || n == num4 - num2 * 2))
						{
							DungeonPlatformData item = new DungeonPlatformData
							{
								Position = new Point(num21, num22),
								OverrideHeightFluff = 0,
								ForcePlacement = true,
								PlacePotsChance = 0.33000001311302185
							};
							data.dungeonPlatformData.Add(item);
						}
						if (num20 == 10 && n == 0)
						{
							DungeonPlatformData item2 = new DungeonPlatformData
							{
								Position = new Point(num21, num22),
								OverrideHeightFluff = 0,
								ForcePlacement = true,
								PlacePotsChance = 0.33000001311302185
							};
							data.dungeonPlatformData.Add(item2);
						}
					}
				}
				int num23 = -1;
				int num24 = 6;
				for (; m < num17; m++)
				{
					Tile tile2 = Main.tile[i, num7 + m];
					if (num23 == -1 && !tile2.active())
					{
						num23 = 15;
					}
					if (num23 > 0)
					{
						num23--;
						if (num23 <= 0)
						{
							break;
						}
						if (num23 <= 5)
						{
							num24--;
						}
					}
					for (int num25 = -num24; num25 <= num24; num25++)
					{
						Tile tile3 = Main.tile[i + num25, num7 + m];
						tile3.ClearEverything();
						if (!DungeonUtils.IsConsideredDungeonWall(tile3.wall))
						{
							tile3.wall = brickWallType;
							if (wallPaintColor.HasValue)
							{
								tile3.wallColor(wallPaintColor.Value);
							}
						}
					}
				}
			}
			if (generating)
			{
				for (int num26 = -num4; num26 <= num4; num26++)
				{
					int num27 = i + num26;
					for (int num28 = 0; num28 <= num16; num28++)
					{
						int num29 = num7 - num28;
						if (!WorldGen.InWorld(num27, num29, 5))
						{
							continue;
						}
						Tile tile4 = Main.tile[num27, num29];
						if (num28 >= 0 && num28 <= num8)
						{
							if (num26 >= -num3 && num26 <= num3)
							{
								DungeonUtils.ChangeWallType(tile4, brickWallType, resetTile: true, wallPaintColor);
							}
							else
							{
								if (num26 > -num4 && num26 < num4)
								{
									DungeonUtils.ChangeWallType(tile4, brickWallType, resetTile: true, wallPaintColor);
								}
								DungeonUtils.ChangeTileType(tile4, brickTileType, resetTile: false, tilePaintColor);
							}
							if (num28 >= num8 - num2 && (num26 < -num9 || num26 > num9))
							{
								DungeonUtils.ChangeTileType(tile4, brickTileType, resetTile: false, tilePaintColor);
							}
						}
						else if (num28 >= num8 - num2 && num28 <= num12 && num26 >= -num10 && num26 <= num10)
						{
							if (num26 >= -num9 && num26 <= num9)
							{
								DungeonUtils.ChangeWallType(tile4, brickWallType, resetTile: true, wallPaintColor);
							}
							else
							{
								if (num26 > -num10 && num26 < num10)
								{
									DungeonUtils.ChangeWallType(tile4, brickWallType, resetTile: true, wallPaintColor);
								}
								DungeonUtils.ChangeTileType(tile4, brickTileType, resetTile: false, tilePaintColor);
							}
							if (num28 >= num12 - num2 && (num26 < -num13 || num26 > num13))
							{
								DungeonUtils.ChangeTileType(tile4, brickTileType, resetTile: false, tilePaintColor);
							}
						}
						else
						{
							if (num28 < num12 - num2 || num28 > num16 || num26 < -num14 || num26 > num14)
							{
								continue;
							}
							if (num26 >= -num13 && num26 <= num13)
							{
								DungeonUtils.ChangeWallType(tile4, brickWallType, resetTile: true, wallPaintColor);
							}
							else
							{
								if (num26 > -num14 && num26 < num14)
								{
									DungeonUtils.ChangeWallType(tile4, brickWallType, resetTile: true, wallPaintColor);
								}
								DungeonUtils.ChangeTileType(tile4, brickTileType, resetTile: false, tilePaintColor);
							}
							if (num28 >= num16 - num2)
							{
								DungeonUtils.ChangeTileType(tile4, brickTileType, resetTile: false, tilePaintColor);
							}
						}
					}
				}
			}
			DungeonPillarSettings dungeonPillarSettings = new DungeonPillarSettings
			{
				Style = settings.StyleData,
				PillarType = PillarType.Block,
				Width = 3,
				Height = 0,
				CrowningOnTop = true,
				CrowningOnBottom = true,
				CrowningStopsAtPillar = false,
				AlwaysPlaceEntirePillar = true
			};
			if (generating)
			{
				dungeonPillarSettings.PillarType = PillarType.BlockActuated;
				new DungeonPillar(dungeonPillarSettings).GenerateFeature(data, i - num9 - 3, num7);
				new DungeonPillar(dungeonPillarSettings).GenerateFeature(data, i + num9 + 3, num7);
				new DungeonPillar(dungeonPillarSettings).GenerateFeature(data, i - num13 - 3, num7);
				new DungeonPillar(dungeonPillarSettings).GenerateFeature(data, i + num13 + 3, num7);
			}
			if (generating)
			{
				DungeonUtils.GenerateBottomWedge(i - num4 - 4, num7 - num8, 5, brickTileType, left: true, wall: false, actuated: false, crowningBottom: true, -1);
				TowerEntrance_OuterPillar(data, i - num4 - 4, num7 - num8, brickTileType);
				DungeonUtils.GenerateBottomWedge(i - num10 - 4, num7 - num12, 5, brickTileType, left: true, wall: false, actuated: false, crowningBottom: true, -1);
				TowerEntrance_OuterPillar(data, i - num10 - 4, num7 - num12, brickTileType);
				DungeonUtils.GenerateBottomWedge(i - num14 - 4, num7 - num16, 5, brickTileType, left: true, wall: false, actuated: false, crowningBottom: true, -1);
				TowerEntrance_OuterPillar(data, i - num14 - 4, num7 - num16, brickTileType);
				DungeonUtils.GenerateBottomWedge(i + num4 + 3, num7 - num8, 5, brickTileType, left: false, wall: false, actuated: false, crowningBottom: true, -1);
				TowerEntrance_OuterPillar(data, i + num4 + 4, num7 - num8, brickTileType);
				DungeonUtils.GenerateBottomWedge(i + num10 + 3, num7 - num12, 5, brickTileType, left: false, wall: false, actuated: false, crowningBottom: true, -1);
				TowerEntrance_OuterPillar(data, i + num10 + 4, num7 - num12, brickTileType);
				DungeonUtils.GenerateBottomWedge(i + num14 + 3, num7 - num16, 5, brickTileType, left: false, wall: false, actuated: false, crowningBottom: true, -1);
				TowerEntrance_OuterPillar(data, i + num14 + 4, num7 - num16, brickTileType);
			}
			if (generating)
			{
				dungeonPillarSettings.PillarType = PillarType.Block;
				dungeonPillarSettings.CrowningOnTop = false;
				dungeonPillarSettings.CrowningOnBottom = false;
				dungeonPillarSettings.Width = 5;
				dungeonPillarSettings.Height = 2;
				new DungeonPillar(dungeonPillarSettings).GenerateFeature(data, i - num4 + 5, num7 - num8 - 1);
				TowerEntrance_LineOfFence(i - num4 - 2, i - num10 + 1, num7 - num8 - 1);
				new DungeonPillar(dungeonPillarSettings).GenerateFeature(data, i - num10 + 5, num7 - num12 - 1);
				TowerEntrance_LineOfFence(i - num10 - 2, i - num14 + 1, num7 - num12 - 1);
				new DungeonPillar(dungeonPillarSettings).GenerateFeature(data, i - num14 + 5, num7 - num16 - 1);
				new DungeonPillar(dungeonPillarSettings).GenerateFeature(data, i - num14 + 13, num7 - num16 - 1);
				new DungeonPillar(dungeonPillarSettings).GenerateFeature(data, i + num4 - 5, num7 - num8 - 1);
				TowerEntrance_LineOfFence(i + num10 - 1, i + num4 + 2, num7 - num8 - 1);
				new DungeonPillar(dungeonPillarSettings).GenerateFeature(data, i + num10 - 5, num7 - num12 - 1);
				TowerEntrance_LineOfFence(i + num14 - 1, i + num10 + 2, num7 - num12 - 1);
				new DungeonPillar(dungeonPillarSettings).GenerateFeature(data, i + num14 - 5, num7 - num16 - 1);
				new DungeonPillar(dungeonPillarSettings).GenerateFeature(data, i + num14 - 13, num7 - num16 - 1);
				TowerEntrance_LineOfFence(i - num14 - 2, i + num14 + 2, num7 - num16 - 1);
				DungeonUtils.GenerateBottomWedge(i - num13, num7 - num16 + num2, 3, brickTileType, left: false, wall: false, actuated: false, crowningBottom: true, -1);
				DungeonUtils.GenerateBottomWedge(i + num13 - 1, num7 - num16 + num2, 3, brickTileType, left: true, wall: false, actuated: false, crowningBottom: true, -1);
			}
			if (generating)
			{
				TowerEntrance_AddPlatform(data, new Point(i - num10 - 2, num7 - num8 + 15));
				TowerEntrance_AddPlatform(data, new Point(i - num10 - 2, num7 - num8 + 21));
				TowerEntrance_AddPlatform(data, new Point(i - num14 - 2, num7 - num12 + 15));
				TowerEntrance_AddPlatform(data, new Point(i - num14 - 2, num7 - num12 + 21));
				TowerEntrance_AddPlatform(data, new Point(i + num10 + 2, num7 - num8 + 15));
				TowerEntrance_AddPlatform(data, new Point(i + num10 + 2, num7 - num8 + 21));
				TowerEntrance_AddPlatform(data, new Point(i + num14 + 2, num7 - num12 + 15));
				TowerEntrance_AddPlatform(data, new Point(i + num14 + 2, num7 - num12 + 21));
				TowerEntrance_AddPlatform(data, new Point(i, num7 - num12 + num2 - 3));
			}
			if (generating)
			{
				int num30 = num7 - num16 + 20;
				DungeonWindowBasicSettings dungeonWindowBasicSettings = new DungeonWindowBasicSettings
				{
					Style = settings.StyleData,
					Width = 5,
					Height = 24,
					Closed = dungeonEntranceIsUnderground
				};
				DungeonWindowMosaicSettings dungeonWindowMosaicSettings = new DungeonWindowMosaicSettings
				{
					Style = settings.StyleData,
					Closed = dungeonEntranceIsUnderground,
					MosaicType = windowType
				};
				switch (windowType)
				{
				case WindowType.RegularWindows:
					new DungeonWindowBasic(dungeonWindowBasicSettings).GenerateFeature(data, i - 9, num30 + 4);
					new DungeonWindowBasic(dungeonWindowBasicSettings).GenerateFeature(data, i + 9, num30 + 4);
					dungeonWindowBasicSettings.Height = 28;
					new DungeonWindowBasic(dungeonWindowBasicSettings).GenerateFeature(data, i, num30 + 3);
					break;
				case WindowType.SkeletronMosaic:
					if (!dungeonEntranceIsUnderground)
					{
						dungeonWindowMosaicSettings.OverrideGlassType = 89;
					}
					dungeonWindowMosaicSettings.OverrideGlassPaint = 26;
					new DungeonWindowMosaic(dungeonWindowMosaicSettings).GenerateFeature(data, i, num30 - 1);
					break;
				case WindowType.MoonLordMosaic:
					if (!dungeonEntranceIsUnderground)
					{
						dungeonWindowMosaicSettings.OverrideGlassType = 91;
					}
					new DungeonWindowMosaic(dungeonWindowMosaicSettings).GenerateFeature(data, i, num30 + 5);
					break;
				}
				dungeonWindowBasicSettings.Width = 9;
				dungeonWindowBasicSettings.Height = 24;
				new DungeonWindowBasic(dungeonWindowBasicSettings).GenerateFeature(data, i - 8, num7 - 16);
				new DungeonWindowBasic(dungeonWindowBasicSettings).GenerateFeature(data, i + 8, num7 - 16);
				dungeonWindowBasicSettings.Width = 7;
				dungeonWindowBasicSettings.Height = 11;
				new DungeonWindowBasic(dungeonWindowBasicSettings).GenerateFeature(data, i - 10, num7 - 37);
				new DungeonWindowBasic(dungeonWindowBasicSettings).GenerateFeature(data, i + 10, num7 - 37);
				dungeonWindowBasicSettings.Height = 13;
				new DungeonWindowBasic(dungeonWindowBasicSettings).GenerateFeature(data, i, num7 - 39);
			}
			if (generating)
			{
				TowerEntrance_Door(data, i, num7, num4, num3, flag, dungeonEntranceIsBuried);
				TowerEntrance_Door(data, i, num7, num4, num3, !flag, dungeonEntranceIsBuried);
			}
			OldManSpawn = DungeonUtils.SetOldManSpawnAndSpawnOldManIfDefaultDungeon(i, num7, generating);
			if (generating && SpecialSeedFeatures.DungeonEntranceHasATree)
			{
				DungeonUtils.GenerateDungeonTree(data, i, (int)Main.worldSurface, num7 - num16 + 8, generateRoots: false);
			}
			if (generating && SpecialSeedFeatures.DungeonEntranceHasStairs)
			{
				int i2 = i + num4;
				DungeonUtils.GenerateDungeonStairs(data, i2, num7, 1, settings.StyleData, num5);
				i2 = i - num4;
				DungeonUtils.GenerateDungeonStairs(data, i2, num7, -1, settings.StyleData, num5);
			}
			Bounds.CalculateHitbox();
		}

		public void TowerEntrance_Door(DungeonData data, int i, int entranceFloor, int outerSize, int innerSize, bool leftDungeonDoor, bool buried)
		{
			int num = (leftDungeonDoor ? (innerSize - 1) : (-outerSize - 2));
			int num2 = (leftDungeonDoor ? (outerSize + 2) : (-innerSize + 1));
			if (buried)
			{
				num += 2 * ((!leftDungeonDoor) ? 1 : 0);
				num2 += 2 * (leftDungeonDoor ? (-1) : 0);
			}
			Point point = new Point(i + (leftDungeonDoor ? (outerSize - 1) : (-outerSize + 1)), entranceFloor);
			Point point2 = new Point(i + (leftDungeonDoor ? (innerSize + 1) : (-innerSize - 1)), entranceFloor);
			for (int j = num; j <= num2; j++)
			{
				for (int k = -3; k <= 1; k++)
				{
					int num3 = j + i;
					int num4 = k + entranceFloor;
					Tile tile = Main.tile[num3, num4];
					if (!buried && ((leftDungeonDoor && num3 >= point.X) || (!leftDungeonDoor && num3 <= point.X)))
					{
						tile.wall = 0;
					}
					if (k >= -2 && k <= 0)
					{
						tile.ClearTile();
					}
				}
			}
			WorldGen.PlaceTile(point.X, point.Y, 10, mute: true, forced: true, -1, 13);
			WorldGen.PlaceTile(point2.X, point2.Y, 10, mute: true, forced: true, -1, 13);
		}

		public void TowerEntrance_LineOfFence(int leftX, int rightX, int y)
		{
			if (y <= 10)
			{
				return;
			}
			for (int i = leftX; i <= rightX; i++)
			{
				if (leftX >= 10 && rightX <= Main.maxTilesX - 10)
				{
					WorldGen.PlaceWall(i, y, 245, mute: true);
				}
			}
		}

		public void TowerEntrance_OuterPillar(DungeonData data, int pillarX, int pillarY, ushort tileType)
		{
			DungeonPillarSettings dungeonPillarSettings = new DungeonPillarSettings();
			dungeonPillarSettings.Style = settings.StyleData;
			dungeonPillarSettings.PillarType = PillarType.Block;
			dungeonPillarSettings.Width = 7;
			dungeonPillarSettings.Height = 3;
			new DungeonPillar(dungeonPillarSettings).GenerateFeature(data, pillarX, pillarY - 1);
			dungeonPillarSettings.Width = 5;
			dungeonPillarSettings.Height = 7;
			new DungeonPillar(dungeonPillarSettings).GenerateFeature(data, pillarX, pillarY - 4);
			if (pillarY - 11 >= 10)
			{
				WorldGen.PlaceTile(pillarX, pillarY - 11, 215, mute: true);
			}
			for (int i = 0; i < 5; i++)
			{
				WorldGen.PlaceWall(pillarX - 2 + i, pillarY - 11, 245, mute: true);
			}
			if (pillarY - 12 >= 10)
			{
				WorldGen.PlaceWall(pillarX - 2, pillarY - 12, 245, mute: true);
				WorldGen.PlaceWall(pillarX + 2, pillarY - 12, 245, mute: true);
			}
			if (pillarY - 10 >= 10)
			{
				WorldGen.PlaceWall(pillarX - 2, pillarY - 10, 245, mute: true);
				WorldGen.PlaceWall(pillarX + 2, pillarY - 10, 245, mute: true);
			}
		}

		public void TowerEntrance_TreeOnPillar(UnifiedRandom genRand, int pillarX, int pillarY)
		{
			int num = 5;
			int num2 = num / 2;
			for (int i = 0; i < num; i++)
			{
				int num3 = pillarX + i - num2;
				for (int j = 0; j <= 3; j++)
				{
					int num4 = pillarY + j;
					if (num4 <= 10)
					{
						break;
					}
					if ((j != 1 || genRand.Next(2) != 0) && (j != 2 || genRand.Next(3) == 0) && (j != 3 || genRand.Next(4) == 0))
					{
						Tile tile = Main.tile[num3, num4];
						if (WorldGen.TileIsExposedToAir(num3, num4))
						{
							tile.type = 2;
						}
						else
						{
							tile.type = 0;
						}
					}
				}
			}
			if (pillarY > 10)
			{
				WorldGen.TryGrowingTreeByType(5, pillarX, pillarY, 0, ignoreWalls: true);
			}
		}

		public void TowerEntrance_AddPlatform(DungeonData data, Point position)
		{
			DungeonPlatformData item = new DungeonPlatformData
			{
				Position = position,
				OverrideHeightFluff = 0,
				ForcePlacement = true,
				PlacePotsChance = 0.33000001311302185,
				PlaceBooksChance = 0.75,
				PlacePotionBottlesChance = 0.10000000149011612,
				NoWaterbolt = true
			};
			data.dungeonPlatformData.Add(item);
		}
	}
}
