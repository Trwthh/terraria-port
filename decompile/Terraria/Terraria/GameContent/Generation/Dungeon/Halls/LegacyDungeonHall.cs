using System;
using System.Collections.Generic;
using ReLogic.Utilities;
using Terraria.GameContent.Generation.Dungeon.Rooms;
using Terraria.Utilities;

namespace Terraria.GameContent.Generation.Dungeon.Halls
{
	public class LegacyDungeonHall : DungeonHall
	{
		public Vector2D LastHall;

		public int Strength;

		public int Steps;

		protected Vector2D OverrideStartPosition;

		protected Vector2D OverrideEndPosition;

		public LegacyDungeonHall(DungeonHallSettings settings)
			: base(settings)
		{
		}

		public override void CalculatePlatformsAndDoors(DungeonData data)
		{
			if (base.Processed)
			{
				DungeonUtils.CalculatePlatformAndDoorsOnHallway(data, StartPosition, StartDirection.Y, settings.ForceStyleForDoorsAndPlatforms ? settings.StyleData : null);
				DungeonUtils.CalculatePlatformAndDoorsOnHallway(data, EndPosition, EndDirection.Y, settings.ForceStyleForDoorsAndPlatforms ? settings.StyleData : null);
				AddExtraPlatformsIfNeeded(data);
			}
		}

		public override void CalculateHall(DungeonData data, Vector2D startPoint, Vector2D endPoint)
		{
			calculated = false;
			OverrideStartPosition = startPoint;
			OverrideEndPosition = endPoint;
			LegacyHall(data, 0, 0);
			calculated = true;
		}

		public override void GenerateHall(DungeonData data)
		{
			generated = false;
			LegacyHall(data, 0, 0, generating: true);
			generated = true;
		}

		public bool GenerateHall(DungeonData data, int x, int y)
		{
			generated = false;
			LegacyHall(data, x, y, generating: true);
			generated = true;
			return true;
		}

		public virtual void LegacyHall(DungeonData dungeonData, int i, int j, bool generating = false)
		{
			LegacyDungeonHallSettings legacyDungeonHallSettings = (LegacyDungeonHallSettings)settings;
			UnifiedRandom unifiedRandom = new UnifiedRandom(legacyDungeonHallSettings.RandomSeed);
			ushort brickTileType = settings.StyleData.BrickTileType;
			ushort brickCrackedTileType = settings.StyleData.BrickCrackedTileType;
			ushort brickWallType = settings.StyleData.BrickWallType;
			byte? b = ((settings.OverridePaintTile > -1) ? new byte?((byte)settings.OverridePaintTile) : settings.StyleData.TilePaintColor);
			byte? b2 = ((settings.OverridePaintWall > -1) ? new byte?((byte)settings.OverridePaintWall) : settings.StyleData.WallPaintColor);
			Vector2D vector2D = new Vector2D(i, j);
			Vector2D startPosition = vector2D;
			Vector2D vector2D2 = Vector2D.Zero;
			int num = (int)(4.0 * dungeonData.hallStrengthScalar) + unifiedRandom.Next(2);
			Vector2D zero = Vector2D.Zero;
			Vector2D zero2 = Vector2D.Zero;
			int num2 = 1;
			double hallStepScalar = dungeonData.hallStepScalar;
			int num3 = (int)(35.0 * hallStepScalar) + unifiedRandom.Next(45);
			bool flag = false;
			if (legacyDungeonHallSettings.CrackedBrickChance > 0.0)
			{
				flag = unifiedRandom.NextDouble() <= legacyDungeonHallSettings.CrackedBrickChance;
			}
			if (legacyDungeonHallSettings.ForceHorizontal)
			{
				num3 += (int)(20.0 * hallStepScalar);
				dungeonData.lastDungeonHall = Vector2D.Zero;
			}
			else
			{
				if (unifiedRandom.Next(5) == 0)
				{
					num *= 2;
					num3 /= 2;
				}
				if (WorldGen.SecretSeed.errorWorld.Enabled && unifiedRandom.Next(2) == 0)
				{
					num *= 2;
				}
				if (WorldGen.SecretSeed.errorWorld.Enabled && unifiedRandom.Next(2) == 0)
				{
					num3 *= 2;
				}
			}
			Vector2D lastHall = dungeonData.lastDungeonHall;
			if (calculated)
			{
				vector2D = (startPosition = StartPosition);
				vector2D2 = (EndPosition - StartPosition).SafeNormalize(Vector2D.UnitX);
				num = Strength;
				num3 = Steps;
				lastHall = LastHall;
			}
			int steps = num3;
			int num4 = num;
			double num5 = dungeonData.hallInteriorToExteriorRatio;
			if ((float)legacyDungeonHallSettings.OverrideStrength > 0f)
			{
				num = (num4 = legacyDungeonHallSettings.OverrideStrength);
			}
			if (legacyDungeonHallSettings.OverrideSteps > 0)
			{
				num3 = (steps = legacyDungeonHallSettings.OverrideSteps);
			}
			if (legacyDungeonHallSettings.OverrideInteriorToExteriorRatio > 0.0)
			{
				num5 = legacyDungeonHallSettings.OverrideInteriorToExteriorRatio;
			}
			bool flag2 = false;
			int num6 = Main.UnderworldLayer - (int)(100.0 * ((dungeonData.HallSizeScalar > dungeonData.RoomSizeScalar) ? dungeonData.HallSizeScalar : dungeonData.RoomSizeScalar));
			bool flag3 = false;
			if (OverrideStartPosition != default(Vector2D) && OverrideEndPosition != default(Vector2D))
			{
				flag3 = true;
				Vector2D overrideStartPosition = OverrideStartPosition;
				Vector2D v = OverrideEndPosition - overrideStartPosition;
				Vector2D vector2D3 = v.SafeNormalize(Vector2D.UnitX);
				num3 = (steps = (int)Math.Ceiling(v.Length() / vector2D3.Length()));
				vector2D = overrideStartPosition;
				startPosition = vector2D;
				zero.X = vector2D3.X;
				zero.Y = vector2D3.Y;
				zero2.X = 0.0 - vector2D3.X;
				zero2.Y = 0.0 - vector2D3.Y;
				vector2D2 = vector2D3;
			}
			else
			{
				bool flag4 = false;
				bool flag5 = true;
				bool flag6 = false;
				while (!flag4)
				{
					flag6 = false;
					if (flag5 && !legacyDungeonHallSettings.ForceHorizontal)
					{
						bool flag7 = true;
						bool flag8 = true;
						bool flag9 = true;
						bool flag10 = true;
						bool flag11 = false;
						int num7 = num3;
						bool flag12 = false;
						for (int num8 = j; num8 > j - num7; num8--)
						{
							if (!WorldGen.InWorld(i, num8, 50))
							{
								flag7 = false;
								break;
							}
							if (DungeonUtils.IsConsideredDungeonWall(Main.tile[i, num8].wall))
							{
								if (flag12)
								{
									flag7 = false;
									break;
								}
							}
							else
							{
								flag12 = true;
							}
						}
						flag12 = false;
						for (int k = j; k < j + num7; k++)
						{
							if (!WorldGen.InWorld(i, k, 50))
							{
								flag8 = false;
								break;
							}
							if (k >= num6)
							{
								flag11 = true;
								flag8 = false;
								break;
							}
							if (DungeonUtils.IsConsideredDungeonWall(Main.tile[i, k].wall))
							{
								if (flag12)
								{
									flag8 = false;
									break;
								}
							}
							else
							{
								flag12 = true;
							}
						}
						flag12 = false;
						for (int num9 = i; num9 > i - num7; num9--)
						{
							if (!WorldGen.InWorld(num9, j, 50))
							{
								flag9 = false;
								break;
							}
							if (DungeonUtils.IsConsideredDungeonWall(Main.tile[num9, j].wall))
							{
								if (flag12)
								{
									flag9 = false;
									break;
								}
							}
							else
							{
								flag12 = true;
							}
						}
						flag12 = false;
						for (int l = i; l < i + num7; l++)
						{
							if (!WorldGen.InWorld(l, j, 50))
							{
								flag10 = false;
								break;
							}
							if (DungeonUtils.IsConsideredDungeonWall(Main.tile[l, j].wall))
							{
								if (flag12)
								{
									flag10 = false;
									break;
								}
							}
							else
							{
								flag12 = true;
							}
						}
						if (!flag9 && !flag10 && !flag7 && !flag8)
						{
							num2 = ((unifiedRandom.Next(2) != 0) ? 1 : (-1));
							if (unifiedRandom.Next(2) == 0)
							{
								flag6 = true;
							}
							if (num2 == 1 && !flag6 && flag11)
							{
								num2 = ((unifiedRandom.Next(2) == 0) ? 1 : (-1));
								flag6 = true;
							}
						}
						else
						{
							int num10 = 0;
							int num11 = 100;
							do
							{
								num11--;
								if (num11 <= 0)
								{
									num10 = 0;
									break;
								}
								num10 = unifiedRandom.Next(4);
								if (num10 == 1 && flag11)
								{
									num10 = ((unifiedRandom.Next(2) == 0) ? 2 : 3);
								}
							}
							while (!(num10 == 0 && flag7) && !(num10 == 1 && flag8) && !(num10 == 2 && flag9) && !(num10 == 3 && flag10));
							switch (num10)
							{
							case 0:
								num2 = -1;
								break;
							case 1:
								num2 = 1;
								break;
							default:
								flag6 = true;
								num2 = ((num10 != 2) ? 1 : (-1));
								break;
							}
						}
					}
					else
					{
						num2 = ((unifiedRandom.Next(2) != 0) ? 1 : (-1));
						if (unifiedRandom.Next(2) == 0)
						{
							flag6 = true;
						}
						if (num2 == 1 && j + num3 >= num6)
						{
							num2 = ((unifiedRandom.Next(2) != 0) ? 1 : (-1));
							flag6 = true;
						}
					}
					flag5 = false;
					if (legacyDungeonHallSettings.ForceHorizontal)
					{
						flag6 = true;
					}
					if (flag6)
					{
						zero.Y = 0.0;
						zero.X = num2;
						zero2.Y = 0.0;
						zero2.X = -num2;
						vector2D2.Y = 0.0;
						vector2D2.X = num2;
						if (unifiedRandom.Next(3) == 0)
						{
							if (unifiedRandom.Next(2) == 0)
							{
								vector2D2.Y = -0.20000000298023224 * dungeonData.hallSlantVariantScalar;
							}
							else
							{
								vector2D2.Y = 0.20000000298023224 * dungeonData.hallSlantVariantScalar;
							}
						}
					}
					else
					{
						num++;
						vector2D2.Y = num2;
						vector2D2.X = 0.0;
						zero.X = 0.0;
						zero.Y = num2;
						zero2.X = 0.0;
						zero2.Y = -num2;
						if (legacyDungeonHallSettings.ZigzagChance > 0.0 && unifiedRandom.NextDouble() <= legacyDungeonHallSettings.ZigzagChance)
						{
							flag2 = true;
							if (unifiedRandom.Next(2) == 0)
							{
								vector2D2.X = (double)unifiedRandom.Next(10, 20) * 0.1 * dungeonData.hallSlantVariantScalar;
							}
							else
							{
								vector2D2.X = (double)(-unifiedRandom.Next(10, 20)) * 0.1 * dungeonData.hallSlantVariantScalar;
							}
						}
						else if (unifiedRandom.Next(2) == 0)
						{
							if (unifiedRandom.Next(2) == 0)
							{
								vector2D2.X = (double)unifiedRandom.Next(20, 40) * 0.01 * dungeonData.hallSlantVariantScalar;
							}
							else
							{
								vector2D2.X = (double)(-unifiedRandom.Next(20, 40)) * 0.01 * dungeonData.hallSlantVariantScalar;
							}
						}
						else
						{
							num3 /= 2;
						}
					}
					if (dungeonData.lastDungeonHall != zero2)
					{
						flag4 = true;
					}
				}
			}
			int num12 = 0;
			float num13 = (float)Main.maxTilesX * 0.25f;
			float num14 = (float)Main.maxTilesX * 0.75f;
			if (WorldGen.SecretSeed.errorWorld.Enabled)
			{
				num13 = (float)Main.maxTilesX * 0.4f;
				num14 = (float)Main.maxTilesX * 0.6f;
			}
			bool flag13 = vector2D.Y < Main.rockLayer + 100.0;
			if (WorldGen.remixWorldGen)
			{
				flag13 = vector2D.Y < Main.worldSurface + 100.0;
			}
			bool flag14 = vector2D.X < (double)(Main.maxTilesX / 2) && vector2D.X > (double)num13;
			bool flag15 = vector2D.X > (double)(Main.maxTilesX / 2) && vector2D.X < (double)num14;
			if (!flag3 && !legacyDungeonHallSettings.ForceHorizontal)
			{
				if (vector2D.X > (double)(Main.maxTilesX - 200))
				{
					num2 = -1;
					zero.X = num2;
					zero.Y = 0.0;
					vector2D2.X = num2;
					vector2D2.Y = 0.0;
					if (unifiedRandom.Next(3) == 0)
					{
						if (unifiedRandom.Next(2) == 0)
						{
							vector2D2.Y = -0.20000000298023224 * dungeonData.hallSlantVariantScalar;
						}
						else
						{
							vector2D2.Y = 0.20000000298023224 * dungeonData.hallSlantVariantScalar;
						}
					}
				}
				else if (vector2D.X < 200.0)
				{
					num2 = 1;
					zero.X = num2;
					zero.Y = 0.0;
					vector2D2.X = num2;
					vector2D2.Y = 0.0;
					if (unifiedRandom.Next(3) == 0)
					{
						if (unifiedRandom.Next(2) == 0)
						{
							vector2D2.Y = -0.20000000298023224 * dungeonData.hallSlantVariantScalar;
						}
						else
						{
							vector2D2.Y = 0.20000000298023224 * dungeonData.hallSlantVariantScalar;
						}
					}
				}
				else if (vector2D.Y >= (double)num6)
				{
					num2 = -1;
					num++;
					zero.X = 0.0;
					zero.Y = num2;
					vector2D2.X = 0.0;
					vector2D2.Y = num2;
					if (unifiedRandom.Next(2) == 0)
					{
						if (unifiedRandom.Next(2) == 0)
						{
							vector2D2.X = (double)((float)unifiedRandom.Next(20, 50) * 0.01f) * dungeonData.hallSlantVariantScalar;
						}
						else
						{
							vector2D2.X = (double)((float)(-unifiedRandom.Next(20, 50)) * 0.01f) * dungeonData.hallSlantVariantScalar;
						}
					}
				}
				else if (vector2D.Y < 200.0)
				{
					num2 = 1;
					num++;
					zero.X = 0.0;
					zero.Y = num2;
					vector2D2.X = 0.0;
					vector2D2.Y = num2;
					if (unifiedRandom.Next(2) == 0)
					{
						if (unifiedRandom.Next(2) == 0)
						{
							vector2D2.X = (double)((float)unifiedRandom.Next(20, 50) * 0.01f) * dungeonData.hallSlantVariantScalar;
						}
						else
						{
							vector2D2.X = (double)((float)(-unifiedRandom.Next(20, 50)) * 0.01f) * dungeonData.hallSlantVariantScalar;
						}
					}
				}
				else if (!flag3)
				{
					if (flag13)
					{
						num2 = 1;
						num++;
						zero.X = 0.0;
						zero.Y = num2;
						vector2D2.X = 0.0;
						vector2D2.Y = num2;
						if (legacyDungeonHallSettings.ZigzagChance > 0.0 && unifiedRandom.NextDouble() <= legacyDungeonHallSettings.ZigzagChance)
						{
							flag2 = true;
							if (unifiedRandom.Next(2) == 0)
							{
								vector2D2.X = (double)unifiedRandom.Next(10, 20) * 0.1 * dungeonData.hallSlantVariantScalar;
							}
							else
							{
								vector2D2.X = (double)(-unifiedRandom.Next(10, 20)) * 0.1 * dungeonData.hallSlantVariantScalar;
							}
						}
						else if (unifiedRandom.Next(2) == 0)
						{
							if (unifiedRandom.Next(2) == 0)
							{
								vector2D2.X = (double)unifiedRandom.Next(20, 50) * 0.01 * dungeonData.hallSlantVariantScalar;
							}
							else
							{
								vector2D2.X = (double)unifiedRandom.Next(20, 50) * 0.01 * dungeonData.hallSlantVariantScalar;
							}
						}
					}
					else if (flag14)
					{
						num2 = -1;
						zero.Y = 0.0;
						zero.X = num2;
						vector2D2.Y = 0.0;
						vector2D2.X = num2;
						if (unifiedRandom.Next(3) == 0)
						{
							if (unifiedRandom.Next(2) == 0)
							{
								vector2D2.Y = -0.20000000298023224 * dungeonData.hallSlantVariantScalar;
							}
							else
							{
								vector2D2.Y = 0.20000000298023224 * dungeonData.hallSlantVariantScalar;
							}
						}
					}
					else if (flag15)
					{
						num2 = 1;
						zero.Y = 0.0;
						zero.X = num2;
						vector2D2.Y = 0.0;
						vector2D2.X = num2;
						if (unifiedRandom.Next(3) == 0)
						{
							if (unifiedRandom.Next(2) == 0)
							{
								vector2D2.Y = -0.20000000298023224 * dungeonData.hallSlantVariantScalar;
							}
							else
							{
								vector2D2.Y = 0.20000000298023224 * dungeonData.hallSlantVariantScalar;
							}
						}
					}
				}
			}
			Vector2D startDirection = zero;
			dungeonData.lastDungeonHall = zero;
			if (!calculated && !flag3 && Math.Abs(vector2D2.X) > Math.Abs(vector2D2.Y) && unifiedRandom.Next(3) != 0)
			{
				num = (int)((float)num4 * ((float)unifiedRandom.Next(110, 150) * 0.01f));
			}
			if (!base.Processed)
			{
				Bounds.SetBounds((int)vector2D.X, (int)vector2D.Y, (int)vector2D.X, (int)vector2D.Y);
			}
			Vector2D startPos = vector2D;
			Vector2D endPos = vector2D + vector2D2 * num3;
			DungeonRoomSearchSettings dungeonRoomSearchSettings = new DungeonRoomSearchSettings
			{
				Fluff = num3 / 2 + num
			};
			List<DungeonRoom> allRoomsInSpots = DungeonUtils.GetAllRoomsInSpots(dungeonData.dungeonRooms, startPos, endPos, dungeonRoomSearchSettings);
			while (num3 > 0)
			{
				num12++;
				if (flag3)
				{
					if (!WorldGen.InWorld((int)(vector2D.X + zero.X), (int)(vector2D.Y + zero.Y), 10))
					{
						num3 = 0;
					}
				}
				else if (zero.X > 0.0 && vector2D.X > (double)(Main.maxTilesX - 100))
				{
					num3 = 0;
				}
				else if (zero.X < 0.0 && vector2D.X < 100.0)
				{
					num3 = 0;
				}
				else if (zero.Y > 0.0 && vector2D.Y >= (double)num6)
				{
					num3 = 0;
				}
				else if (zero.Y < 0.0 && vector2D.Y < 100.0)
				{
					num3 = 0;
				}
				else if (WorldGen.remixWorldGen && zero.Y < 0.0 && vector2D.Y < (Main.rockLayer + Main.worldSurface) / 2.0)
				{
					num3 = 0;
				}
				else if (!WorldGen.remixWorldGen && zero.Y < 0.0 && vector2D.Y < Main.rockLayer + 50.0)
				{
					num3 = 0;
				}
				num3--;
				int num15 = Math.Max(0, Math.Min(Main.maxTilesX - 1, (int)(vector2D.X - (double)num - 4.0 - (double)unifiedRandom.Next(6))));
				int num16 = Math.Max(0, Math.Min(Main.maxTilesX - 1, (int)(vector2D.X + (double)num + 4.0 + (double)unifiedRandom.Next(6))));
				int num17 = Math.Max(0, Math.Min(Main.maxTilesY - 1, (int)(vector2D.Y - (double)num - 4.0 - (double)unifiedRandom.Next(6))));
				int num18 = Math.Max(0, Math.Min(Main.maxTilesY - 1, (int)(vector2D.Y + (double)num + 4.0 + (double)unifiedRandom.Next(6))));
				if (!base.Processed)
				{
					dungeonData.dungeonBounds.UpdateBounds(num15, num17, num16, num18);
					Bounds.UpdateBounds(num15, num17, num16, num18);
				}
				if (generating && !settings.CarveOnly)
				{
					for (int m = num15; m < num16; m++)
					{
						for (int n = num17; n < num18; n++)
						{
							bool flag16 = true;
							ProtectionType highestProtectionTypeFromPoint = DungeonUtils.GetHighestProtectionTypeFromPoint(m, n, allRoomsInSpots);
							if (highestProtectionTypeFromPoint != ProtectionType.TilesAndWalls)
							{
								if (highestProtectionTypeFromPoint == ProtectionType.Tiles)
								{
									flag16 = false;
								}
								Tile tile = Main.tile[m, n];
								tile.liquid = 0;
								bool resetTile = m >= num15 + 1 && m < num16 - 1 && n >= num17 + 1 && n < num18 - 1;
								if (flag16 && n <= Main.UnderworldLayer + 7 && CanPlaceTileAt(dungeonData, tile, brickTileType, brickCrackedTileType))
								{
									DungeonUtils.ChangeTileType(tile, brickTileType, resetTile, b);
								}
							}
						}
					}
					for (int num19 = num15 + 1; num19 < num16 - 1; num19++)
					{
						for (int num20 = num17 + 1; num20 < num18 - 1; num20++)
						{
							if (num20 >= Main.UnderworldLayer + 7)
							{
								continue;
							}
							bool flag17 = true;
							ProtectionType highestProtectionTypeFromPoint2 = DungeonUtils.GetHighestProtectionTypeFromPoint(num19, num20, allRoomsInSpots);
							if (highestProtectionTypeFromPoint2 != ProtectionType.TilesAndWalls)
							{
								if (highestProtectionTypeFromPoint2 == ProtectionType.Walls && DungeonUtils.IsConsideredDungeonWall(Main.tile[num19, num20].wall))
								{
									flag17 = false;
								}
								if (flag17)
								{
									DungeonUtils.ChangeWallType(Main.tile[num19, num20], brickWallType, resetTile: false, b2);
								}
							}
						}
					}
				}
				if (generating)
				{
					int num21 = 0;
					if (vector2D2.Y == 0.0 && unifiedRandom.Next(num + 1) == 0)
					{
						num21 = unifiedRandom.Next(1, 3);
					}
					else if (vector2D2.X == 0.0 && unifiedRandom.Next(num - 1) == 0)
					{
						num21 = unifiedRandom.Next(1, 3);
					}
					else if (unifiedRandom.Next(num * 3) == 0)
					{
						num21 = unifiedRandom.Next(1, 3);
					}
					num15 = Math.Max(0, Math.Min(Main.maxTilesX - 1, (int)(vector2D.X - (double)num * num5 - (double)num21)));
					num16 = Math.Max(0, Math.Min(Main.maxTilesX - 1, (int)(vector2D.X + (double)num * num5 + (double)num21)));
					num17 = Math.Max(0, Math.Min(Main.maxTilesY - 1, (int)(vector2D.Y - (double)num * num5 - (double)num21)));
					num18 = Math.Max(0, Math.Min(Main.maxTilesY - 1, (int)(vector2D.Y + (double)num * num5 + (double)num21)));
					for (int num22 = num15; num22 < num16; num22++)
					{
						for (int num23 = num17; num23 < num18; num23++)
						{
							bool flag18 = true;
							bool flag19 = true;
							ProtectionType highestProtectionTypeFromPoint3 = DungeonUtils.GetHighestProtectionTypeFromPoint(num22, num23, allRoomsInSpots);
							if (highestProtectionTypeFromPoint3 == ProtectionType.TilesAndWalls)
							{
								continue;
							}
							if (highestProtectionTypeFromPoint3 == ProtectionType.Tiles)
							{
								flag18 = false;
							}
							if (highestProtectionTypeFromPoint3 == ProtectionType.Walls && DungeonUtils.IsConsideredDungeonWall(Main.tile[num22, num23].wall))
							{
								flag19 = false;
							}
							if (!CanRemoveTileAt(dungeonData, Main.tile[num22, num23], brickCrackedTileType))
							{
								continue;
							}
							if (flag)
							{
								if ((Main.tile[num22, num23].active() || !DungeonUtils.IsConsideredDungeonWall(Main.tile[num22, num23].wall)) && num23 < Main.UnderworldLayer)
								{
									if (settings.CarveOnly)
									{
										Main.tile[num22, num23].ClearTile();
									}
									else
									{
										Main.tile[num22, num23].ClearTile();
										if (flag18)
										{
											DungeonUtils.ChangeTileType(Main.tile[num22, num23], brickCrackedTileType, resetTile: false, b);
										}
									}
								}
							}
							else
							{
								Main.tile[num22, num23].ClearTile();
							}
							if (flag19 && num23 < Main.UnderworldLayer && !settings.CarveOnly)
							{
								DungeonUtils.ChangeWallType(Main.tile[num22, num23], brickWallType, resetTile: false, b2);
							}
						}
					}
				}
				vector2D += vector2D2;
				if (!flag3 && flag2 && num12 > unifiedRandom.Next(10, 20))
				{
					num12 = 0;
					vector2D2.X *= -1.0;
				}
			}
			dungeonData.genVars.generatingDungeonPositionX = (int)vector2D.X;
			dungeonData.genVars.generatingDungeonPositionY = (int)vector2D.Y;
			StartPosition = startPosition;
			EndPosition = vector2D;
			StartDirection = startDirection;
			EndDirection = zero;
			Strength = num4;
			Steps = steps;
			LastHall = lastHall;
			CrackedBrick = flag;
			if (!base.Processed)
			{
				Bounds.CalculateHitbox();
			}
		}
	}
}
