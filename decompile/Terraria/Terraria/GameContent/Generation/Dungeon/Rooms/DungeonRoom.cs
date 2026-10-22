using System;
using Microsoft.Xna.Framework;
using ReLogic.Utilities;
using Terraria.GameContent.Generation.Dungeon.Features;
using Terraria.Utilities;

namespace Terraria.GameContent.Generation.Dungeon.Rooms
{
	public abstract class DungeonRoom
	{
		public DungeonRoomSettings settings;

		public bool calculated;

		public bool generated;

		public DungeonBounds InnerBounds = new DungeonBounds();

		public DungeonBounds OuterBounds = new DungeonBounds();

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

		public Point Center => InnerBounds.Center;

		public DungeonRoom(DungeonRoomSettings settings)
		{
			this.settings = settings;
		}

		public virtual bool CanGenerateFeatureAt(DungeonData data, IDungeonFeature feature, int x, int y)
		{
			if (feature is DungeonWindow && data.Type != DungeonType.DualDungeon)
			{
				return false;
			}
			if (feature is DungeonPitTrap && ((DungeonPitTrapSettings)((DungeonPitTrap)feature).settings).ConnectedRoom != this)
			{
				return false;
			}
			return settings.StyleData.CanGenerateFeatureAt(data, this, feature, x, y);
		}

		public virtual void GeneratePreHallwaysDungeonFeaturesInRoom(DungeonData data)
		{
			if ((settings.StyleData.Style == 4 || settings.StyleData.Style == 5) && InnerBounds.Width > 10 && InnerBounds.Height > 10)
			{
				DungeonUtils.GenerateSpeleothemsInArea(data, settings.StyleData, InnerBounds.Left, InnerBounds.Top, InnerBounds.Width, InnerBounds.Height, Math.Max(3, InnerBounds.Width / 3), settings.StyleData.BrickTileType, (settings.OverridePaintTile >= 0 || !settings.StyleData.TilePaintColor.HasValue) ? settings.OverridePaintTile : settings.StyleData.TilePaintColor.Value);
			}
		}

		public virtual void GenerateEarlyDungeonFeaturesInRoom(DungeonData data)
		{
			UnifiedRandom unifiedRandom = new UnifiedRandom(settings.RandomSeed);
			if (data.Type != DungeonType.DualDungeon)
			{
				return;
			}
			if (unifiedRandom.Next(3) == 0)
			{
				int num = 1;
				DungeonWindowBasicSettings dungeonWindowBasicSettings = new DungeonWindowBasicSettings
				{
					Style = settings.StyleData,
					Closed = !((double)InnerBounds.Bottom <= Main.worldSurface)
				};
				int width = InnerBounds.Width;
				int height = InnerBounds.Height;
				bool flag = true;
				int num2 = unifiedRandom.Next(3);
				if (num2 >= 1 && num2 <= 2 && (width <= 36 || height <= 15))
				{
					num2 = 0;
				}
				if (num2 == 0 && (width <= 14 || height <= 10))
				{
					flag = false;
				}
				if (flag)
				{
					Point center = InnerBounds.Center;
					if (num2 == 0 || (uint)(num2 - 1) > 1u)
					{
						num = Math.Max(3, InnerBounds.Width / 3);
						if (num % 2 == 0)
						{
							num++;
						}
						dungeonWindowBasicSettings.Width = Math.Max(3, num);
						dungeonWindowBasicSettings.Height = Math.Max(5, InnerBounds.Height / 3);
						DungeonWindow dungeonWindow = new DungeonWindowBasic(dungeonWindowBasicSettings);
						center = GetRoomCenterForDungeonFeature(data, dungeonWindow);
						if (CanGenerateFeatureAt(data, dungeonWindow, center.X, center.Y))
						{
							dungeonWindow.GenerateFeature(data, center.X, center.Y);
						}
					}
					else
					{
						num = Math.Min(7, Math.Max(3, InnerBounds.Width / 5));
						if (num % 2 == 0)
						{
							num++;
						}
						dungeonWindowBasicSettings.Width = Math.Max(3, num);
						dungeonWindowBasicSettings.Height = Math.Max(5, InnerBounds.Height / 3);
						DungeonWindow dungeonWindow = new DungeonWindowBasic(dungeonWindowBasicSettings);
						center = GetRoomCenterForDungeonFeature(data, dungeonWindow);
						if (CanGenerateFeatureAt(data, dungeonWindow, center.X, center.Y))
						{
							dungeonWindow.GenerateFeature(data, center.X, center.Y);
						}
						dungeonWindowBasicSettings.Height -= 2;
						dungeonWindow = new DungeonWindowBasic(dungeonWindowBasicSettings);
						if (CanGenerateFeatureAt(data, dungeonWindow, center.X - num - 2, center.Y))
						{
							dungeonWindow.GenerateFeature(data, center.X - num - 2, center.Y);
						}
						dungeonWindow = new DungeonWindowBasic(dungeonWindowBasicSettings);
						if (CanGenerateFeatureAt(data, dungeonWindow, center.X + num + 2, center.Y))
						{
							dungeonWindow.GenerateFeature(data, center.X + num + 2, center.Y);
						}
					}
				}
			}
			int liquidType = settings.StyleData.LiquidType;
			if (liquidType >= 0)
			{
				FloodRoom((byte)liquidType);
			}
		}

		public virtual void GenerateLateDungeonFeaturesInRoom(DungeonData data)
		{
		}

		public virtual Point GetRoomCenterForDungeonFeature(DungeonData data, DungeonFeature feature)
		{
			return Center;
		}

		public virtual Point GetRoomCenterForHallway(Vector2D otherRoomPos)
		{
			return Center;
		}

		public abstract void CalculateRoom(DungeonData data);

		public virtual void CalculatePlatformsAndDoors(DungeonData data)
		{
			DungeonUtils.CalculatePlatformsAndDoorsOnEdgesOfRoom(data, InnerBounds, settings.ForceStyleForDoorsAndPlatforms ? settings.StyleData : null, 3, 3);
		}

		public virtual ConnectionPointQuality GetHallwayConnectionPoint(Vector2D otherRoomPos, out Vector2D connectionPoint)
		{
			if (settings.HallwayConnectionPointOverride != null)
			{
				ConnectionPointQuality result = settings.HallwayConnectionPointOverride(this, otherRoomPos, out connectionPoint);
				if (settings.HallwayPointAdjuster.HasValue)
				{
					Vector2D vector2D = (otherRoomPos - connectionPoint).SafeNormalize(Vector2D.UnitX);
					connectionPoint -= vector2D * settings.HallwayPointAdjuster.Value;
				}
				return result;
			}
			connectionPoint = GetRoomCenterForHallway(otherRoomPos);
			Vector2D vector2D2 = (otherRoomPos - connectionPoint).SafeNormalize(Vector2D.UnitX);
			if (-0.5 < vector2D2.Y && vector2D2.Y < 0.7 && WorldGen.genRand.Next(2) == 0)
			{
				while (IsInsideRoom(connectionPoint.ToPoint()))
				{
					connectionPoint.Y += 1.0;
				}
				connectionPoint.Y -= 3.0;
			}
			else if (-0.7 < vector2D2.Y && vector2D2.Y < 0.5 && WorldGen.genRand.Next(3) == 0)
			{
				while (IsInsideRoom(connectionPoint.ToPoint()))
				{
					connectionPoint.Y -= 1.0;
				}
				connectionPoint.Y += 3.0;
			}
			else
			{
				connectionPoint += WorldGen.genRand.NextVector2DCircularEdge(4.0, 4.0);
			}
			vector2D2 = (otherRoomPos - connectionPoint).SafeNormalize(Vector2D.UnitX);
			while (IsInsideRoom(connectionPoint.ToPoint()))
			{
				connectionPoint += vector2D2;
			}
			if (settings.HallwayPointAdjuster.HasValue)
			{
				connectionPoint -= vector2D2 * settings.HallwayPointAdjuster.Value;
			}
			return ConnectionPointQuality.Good;
		}

		public abstract bool GenerateRoom(DungeonData data);

		public virtual bool TryGenerateChestInRoom(DungeonData data, DungeonGlobalBasicChests feature)
		{
			return DungeonUtils.GenerateDungeonRegularChest(data, feature, settings.StyleData, InnerBounds);
		}

		public virtual bool DualDungeons_TryGenerateBiomeChestInRoom(DungeonData data, DungeonGlobalBiomeChests feature)
		{
			return DungeonUtils.GenerateDungeonBiomeChest(data, feature, settings.StyleData, InnerBounds);
		}

		public virtual ProtectionType GetProtectionTypeFromPoint(int x, int y)
		{
			if (!OuterBounds.Contains(x, y))
			{
				return ProtectionType.None;
			}
			return ProtectionType.Walls;
		}

		public bool IsInsideRoom(Point point)
		{
			return IsInsideRoom(point.X, point.Y);
		}

		public virtual bool IsInsideRoom(int x, int y)
		{
			return InnerBounds.Contains(x, y);
		}

		public virtual int GetFloodedRoomTileCount()
		{
			return InnerBounds.Width * InnerBounds.Height;
		}

		public virtual void FloodRoom(byte liquidType)
		{
			for (int i = InnerBounds.Left; i <= InnerBounds.Right; i++)
			{
				for (int j = InnerBounds.Center.Y; j <= InnerBounds.Bottom; j++)
				{
					Tile tile = Main.tile[i, j];
					if (!tile.active())
					{
						tile.liquid = byte.MaxValue;
						tile.liquidType(liquidType);
					}
				}
			}
		}

		public virtual int GetFurnitureCount(int defaultCount)
		{
			return defaultCount;
		}

		public void GenerateDungeonSquareRoom(DungeonData data, DungeonBounds innerBounds, DungeonBounds outerBounds, Vector2D currentPoint, DungeonGenerationStyleData styleData, int innerBoundsSize, int totalBoundsSize, bool genTiles = true, bool genWalls = true)
		{
			byte? b = ((settings.OverridePaintTile > -1) ? new byte?((byte)settings.OverridePaintTile) : styleData.TilePaintColor);
			GenerateDungeonSquareRoom(wallPaint: (settings.OverridePaintWall > -1) ? new byte?((byte)settings.OverridePaintWall) : styleData.WallPaintColor, data: data, innerBounds: innerBounds, outerBounds: outerBounds, currentPoint: currentPoint, tileType: styleData.BrickTileType, wallType: styleData.BrickWallType, newStyleID: styleData.Style, tilePaint: b, innerBoundsSize: innerBoundsSize, totalBoundsSize: totalBoundsSize, genTiles: genTiles, genWalls: genWalls);
		}

		public void GenerateDungeonSquareRoom(DungeonData data, DungeonBounds innerBounds, DungeonBounds outerBounds, Vector2D currentPoint, ushort tileType, ushort wallType, int newStyleID, int? tilePaint, int? wallPaint, int innerBoundsSize, int totalBoundsSize, bool genTiles = true, bool genWalls = true)
		{
			for (int i = -totalBoundsSize; i <= totalBoundsSize; i++)
			{
				int num = (int)currentPoint.X + i;
				for (int j = -totalBoundsSize; j <= totalBoundsSize; j++)
				{
					int num2 = (int)currentPoint.Y + j;
					Tile tile = Main.tile[num, num2];
					if (Math.Abs(i) <= innerBoundsSize && Math.Abs(j) <= innerBoundsSize)
					{
						innerBounds.UpdateBounds(num, num2);
						if (!CanRemoveTileAt(data, tile))
						{
							continue;
						}
						if (genWalls)
						{
							DungeonUtils.ChangeWallType(tile, wallType, resetTile: true, wallPaint);
						}
					}
					else
					{
						outerBounds.UpdateBounds(num, num2);
						if (CanPlaceTileAt(data, tile, tileType, wallType, newStyleID))
						{
							bool flag = genWalls && i > -totalBoundsSize && i < totalBoundsSize && j > -totalBoundsSize && j < totalBoundsSize;
							if (genTiles)
							{
								DungeonUtils.ChangeTileType(tile, tileType, flag, tilePaint);
							}
							if (flag)
							{
								DungeonUtils.ChangeWallType(tile, wallType, resetTile: false, wallPaint);
							}
						}
					}
					tile.liquid = 0;
				}
			}
		}

		public virtual bool CanPlaceTileAt(DungeonData data, Tile tile, int tileType, int wallType, int newStyleID)
		{
			if (tile.wall == 350)
			{
				return false;
			}
			return !DungeonUtils.IsHigherOrEqualTieredDungeonWall(data, tile.wall, wallType, newStyleID, newStyleID, includeWindows: false, includeExtras: true);
		}

		public virtual bool CanRemoveTileAt(DungeonData data, Tile tile)
		{
			if (tile.wall == 350)
			{
				return false;
			}
			return true;
		}
	}
}
