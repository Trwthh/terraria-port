using System;
using System.Collections.Generic;
using Microsoft.Xna.Framework;
using ReLogic.Utilities;
using Terraria.GameContent.Generation.Dungeon.Rooms;

namespace Terraria.GameContent.Generation.Dungeon.Halls
{
	public abstract class DungeonHall
	{
		public DungeonHallSettings settings;

		public bool calculated;

		public bool generated;

		public DungeonBounds Bounds = new DungeonBounds();

		public Vector2D StartPosition;

		public Vector2D EndPosition;

		public Vector2D StartDirection;

		public Vector2D EndDirection;

		public bool CrackedBrick;

		public bool Processed
		{
			get
			{
				if (!calculated)
				{
					return generated;
				}
				return true;
			}
		}

		public Vector2D CenterPosition => (StartPosition + EndPosition) / 2.0;

		public DungeonHall(DungeonHallSettings settings)
		{
			this.settings = settings;
		}

		public abstract void CalculateHall(DungeonData data, Vector2D startPoint, Vector2D endPoint);

		public abstract void CalculatePlatformsAndDoors(DungeonData data);

		public abstract void GenerateHall(DungeonData data);

		public virtual int GetFurnitureCount(int defaultCount)
		{
			return defaultCount;
		}

		public void AddExtraPlatformsIfNeeded(DungeonData data)
		{
			if (data.Type != DungeonType.DualDungeon)
			{
				return;
			}
			float num = 0.65f;
			int num2 = 20;
			double num3 = StartPosition.Distance(EndPosition);
			float num4 = num2;
			Vector2 vector = StartPosition.ToVector2();
			Vector2 vector2 = EndPosition.ToVector2();
			Vector2 vector3 = (vector2 - vector).SafeNormalize(Vector2.Zero);
			if (!(vector3.Y < num) || !(vector3.Y > 0f - num))
			{
				for (; (double)num4 < num3; num4 += (float)num2)
				{
					Vector2 v = Vector2.Lerp(vector, vector2, (float)((double)num4 / num3));
					DungeonUtils.CalculatePlatformAndDoorsOnHallway(data, v.ToVector2D(), vector3.Y, settings.ForceStyleForDoorsAndPlatforms ? settings.StyleData : null);
				}
			}
		}

		public void GenerateDungeonSquareHall(DungeonData data, List<DungeonRoom> roomsInArea, Vector2D currentPoint, DungeonGenerationStyleData styleData, int innerBoundsSize, int outerBoundsSize, bool placeOverProtectedBricks = false, bool crackedBricks = false, bool clearPaintFirst = false)
		{
			ushort brickTileType = styleData.BrickTileType;
			ushort brickCrackedTileType = styleData.BrickCrackedTileType;
			ushort brickWallType = styleData.BrickWallType;
			byte? b = ((settings.OverridePaintTile > -1) ? new byte?((byte)settings.OverridePaintTile) : settings.StyleData.TilePaintColor);
			byte? b2 = ((settings.OverridePaintWall > -1) ? new byte?((byte)settings.OverridePaintWall) : settings.StyleData.WallPaintColor);
			int num = innerBoundsSize + outerBoundsSize;
			for (int i = -num; i <= num; i++)
			{
				int num2 = (int)currentPoint.X + i;
				for (int j = -num; j <= num; j++)
				{
					int num3 = (int)currentPoint.Y + j;
					bool flag = true;
					bool flag2 = true;
					ProtectionType highestProtectionTypeFromPoint = DungeonUtils.GetHighestProtectionTypeFromPoint(num2, num3, roomsInArea);
					if (highestProtectionTypeFromPoint == ProtectionType.TilesAndWalls)
					{
						continue;
					}
					if (highestProtectionTypeFromPoint == ProtectionType.Tiles)
					{
						flag = false;
					}
					if (highestProtectionTypeFromPoint == ProtectionType.Walls && DungeonUtils.IsConsideredDungeonWall(Main.tile[num2, num3].wall))
					{
						flag2 = false;
					}
					Tile tile = Main.tile[num2, num3];
					if (Math.Abs(i) <= innerBoundsSize && Math.Abs(j) <= innerBoundsSize)
					{
						if (!CanRemoveTileAt(data, tile, brickCrackedTileType))
						{
							continue;
						}
						if (crackedBricks)
						{
							if ((tile.active() || !DungeonUtils.IsConsideredDungeonWall(tile.wall)) && num3 < Main.UnderworldLayer)
							{
								if (settings.CarveOnly)
								{
									tile.ClearTile();
								}
								else
								{
									if (flag)
									{
										tile.ClearTile();
									}
									if (flag2)
									{
										tile.wall = 0;
									}
									if (flag2)
									{
										if (clearPaintFirst)
										{
											WorldGen.paintWall(num2, num3, 0, broadCast: false, paintEffects: false);
										}
										DungeonUtils.ChangeWallType(tile, brickWallType, resetTile: false, b2);
									}
									if (flag)
									{
										if (clearPaintFirst)
										{
											WorldGen.paintTile(num2, num3, 0, broadCast: false, paintEffects: false);
										}
										DungeonUtils.ChangeTileType(tile, brickCrackedTileType, resetTile: false, b);
									}
								}
							}
						}
						else
						{
							tile.ClearTile();
							if (!settings.CarveOnly && flag2)
							{
								if (clearPaintFirst)
								{
									WorldGen.paintWall(num2, num3, 0, broadCast: false, paintEffects: false);
								}
								DungeonUtils.ChangeWallType(tile, brickWallType, resetTile: false, b2);
							}
						}
					}
					else if (CanPlaceTileAt(data, tile, brickTileType, brickCrackedTileType))
					{
						if (flag)
						{
							tile.ClearTile();
						}
						if (flag2)
						{
							tile.wall = 0;
						}
						if (flag)
						{
							if (clearPaintFirst)
							{
								WorldGen.paintTile(num2, num3, 0, broadCast: false, paintEffects: false);
							}
							DungeonUtils.ChangeTileType(tile, brickTileType, resetTile: false, b);
						}
						if (flag2 && i > -num && i < num && j > -num && j < num)
						{
							if (clearPaintFirst)
							{
								WorldGen.paintWall(num2, num3, 0, broadCast: false, paintEffects: false);
							}
							DungeonUtils.ChangeWallType(tile, brickWallType, resetTile: false, b2);
						}
					}
					tile.liquid = 0;
				}
			}
		}

		public virtual bool CanPlaceTileAt(DungeonData data, Tile tile, int tileType, int tileCrackedType)
		{
			if (settings.CarveOnly)
			{
				return false;
			}
			if (DungeonUtils.IsConsideredDungeonWall(tile.wall))
			{
				if (tile.active())
				{
					if (!DungeonUtils.IsHigherOrEqualTieredDungeonTile(data, tile.type, tileType, -1, settings.StyleData.Style))
					{
						return tile.type != tileCrackedType;
					}
					return false;
				}
				return false;
			}
			return true;
		}

		public virtual bool CanRemoveTileAt(DungeonData data, Tile tile, int tileCrackedType)
		{
			if (!tile.active())
			{
				return true;
			}
			if (data.Type == DungeonType.DualDungeon && tile.type == tileCrackedType)
			{
				return false;
			}
			return true;
		}
	}
}
