using System.Collections.Generic;
using System.Linq;
using Microsoft.Xna.Framework;
using Terraria.Utilities;
using Terraria.WorldBuilding;

namespace Terraria.GameContent.Generation.Dungeon.Rooms
{
	public class LivingTreeDungeonRoom : DungeonRoom
	{
		private ShapeData _innerShapeData = new ShapeData();

		private ShapeData _outerShapeData = new ShapeData();

		private int _floodedTileCount;

		public Point[] Positions;

		public LivingTreeDungeonRoom(DungeonRoomSettings settings)
			: base(settings)
		{
		}

		public override void CalculateRoom(DungeonData data)
		{
			calculated = false;
			int x = settings.RoomPosition.X;
			int y = settings.RoomPosition.Y;
			LivingTreeRoom(data, x, y, generating: false);
			calculated = true;
		}

		public override bool GenerateRoom(DungeonData data)
		{
			generated = false;
			int x = settings.RoomPosition.X;
			int y = settings.RoomPosition.Y;
			LivingTreeRoom(data, x, y, generating: true);
			generated = true;
			return true;
		}

		public override int GetFloodedRoomTileCount()
		{
			return _floodedTileCount;
		}

		public override void FloodRoom(byte liquidType)
		{
			if (_innerShapeData == null || Positions == null)
			{
				base.FloodRoom(liquidType);
				return;
			}
			_ = (WormlikeDungeonRoomSettings)settings;
			WorldUtils.Gen(Positions[0], new ModShapes.All(_innerShapeData), Actions.Chain(new Modifiers.IsBelowHeight(base.Center.Y, inclusive: true), new Modifiers.IsNotSolid(), new Actions.SetLiquid(liquidType)));
		}

		public override ProtectionType GetProtectionTypeFromPoint(int x, int y)
		{
			if (_innerShapeData == null || _outerShapeData == null || Positions == null || (calculated && !OuterBounds.Contains(x, y)))
			{
				return base.GetProtectionTypeFromPoint(x, y);
			}
			Point point = Positions[0];
			if (!_outerShapeData.Contains(x - point.X, y - point.Y))
			{
				return ProtectionType.None;
			}
			return ProtectionType.Walls;
		}

		public override bool IsInsideRoom(int x, int y)
		{
			if (Positions == null)
			{
				return base.IsInsideRoom(x, y);
			}
			Point point = Positions[0];
			if (base.IsInsideRoom(x, y))
			{
				return _innerShapeData.Contains(x - point.X, y - point.Y);
			}
			return false;
		}

		public override void GeneratePreHallwaysDungeonFeaturesInRoom(DungeonData data)
		{
			UnifiedRandom unifiedRandom = new UnifiedRandom(settings.RandomSeed);
			ushort brickTileType = settings.StyleData.BrickTileType;
			ushort brickCrackedTileType = settings.StyleData.BrickCrackedTileType;
			byte? b = ((settings.OverridePaintTile > -1) ? new byte?((byte)settings.OverridePaintTile) : settings.StyleData.TilePaintColor);
			if (settings.OverridePaintWall <= -1)
			{
				_ = settings.StyleData.WallPaintColor;
			}
			else
			{
				_ = settings.OverridePaintWall;
			}
			int growthLength = (int)((float)InnerBounds.Height * 0.1f) + unifiedRandom.Next(4);
			int branchDensity = 2 + unifiedRandom.Next(2);
			int leafDensity = 3 + unifiedRandom.Next(4);
			DungeonUtils.GenerateHangingLeafCluster(startPoint: new Point(InnerBounds.Center.X, InnerBounds.Top), data: data, genRand: unifiedRandom, bounds: OuterBounds, growthLength: growthLength, branchDensity: branchDensity, leafDensity: leafDensity, leafType: brickCrackedTileType, woodType: brickTileType, leafPaintColor: b, woodPaintColor: b);
			growthLength = (int)((float)InnerBounds.Height * 0.15f) + unifiedRandom.Next(5);
			branchDensity = 3 + unifiedRandom.Next(2);
			leafDensity = 4 + unifiedRandom.Next(4);
			DungeonUtils.GenerateHangingLeafCluster(startPoint: new Point(InnerBounds.Left + 2 + unifiedRandom.Next(3), InnerBounds.Top), data: data, genRand: unifiedRandom, bounds: OuterBounds, growthLength: growthLength, branchDensity: branchDensity, leafDensity: leafDensity, leafType: brickCrackedTileType, woodType: brickTileType, leafPaintColor: b, woodPaintColor: b);
			growthLength = (int)((float)InnerBounds.Height * 0.15f) + unifiedRandom.Next(5);
			branchDensity = 3 + unifiedRandom.Next(2);
			leafDensity = 4 + unifiedRandom.Next(4);
			DungeonUtils.GenerateHangingLeafCluster(startPoint: new Point(InnerBounds.Right - 2 - unifiedRandom.Next(3), InnerBounds.Top), data: data, genRand: unifiedRandom, bounds: OuterBounds, growthLength: growthLength, branchDensity: branchDensity, leafDensity: leafDensity, leafType: brickCrackedTileType, woodType: brickTileType, leafPaintColor: b, woodPaintColor: b);
			base.GeneratePreHallwaysDungeonFeaturesInRoom(data);
		}

		public override void GenerateLateDungeonFeaturesInRoom(DungeonData data)
		{
			UnifiedRandom unifiedRandom = new UnifiedRandom(settings.RandomSeed);
			_ = (LivingTreeDungeonRoomSettings)settings;
			ushort brickTileType = settings.StyleData.BrickTileType;
			ushort brickCrackedTileType = settings.StyleData.BrickCrackedTileType;
			ushort brickWallType = settings.StyleData.BrickWallType;
			byte? b = ((settings.OverridePaintTile > -1) ? new byte?((byte)settings.OverridePaintTile) : settings.StyleData.TilePaintColor);
			for (int i = 0; i < 50; i++)
			{
				int x = unifiedRandom.Next(InnerBounds.Left + 1, InnerBounds.Right);
				int y = unifiedRandom.Next(InnerBounds.Top + 1, InnerBounds.Bottom);
				Point point = DungeonUtils.FirstSolid(ceiling: false, new Point(x, y), InnerBounds);
				x = point.X;
				y = point.Y - 1;
				Tile tile = Main.tile[x, y];
				if (tile.active() || tile.wall != brickWallType)
				{
					continue;
				}
				if (unifiedRandom.Next(2) == 0)
				{
					WorldGen.PlaceTile(x, y, 187, mute: true, forced: false, -1, unifiedRandom.Next(47, 50));
					continue;
				}
				int num = unifiedRandom.Next(2);
				int pileStyle = 72;
				if (num == 1)
				{
					pileStyle = unifiedRandom.Next(59, 62);
				}
				WorldGen.PlaceSmallPile(x, y, pileStyle, num, 185);
			}
			for (int j = 0; j < 10; j++)
			{
				int x2 = unifiedRandom.Next(InnerBounds.Left + 1, InnerBounds.Right);
				int y2 = unifiedRandom.Next(InnerBounds.Top + 1, InnerBounds.Bottom);
				Point point2 = DungeonUtils.FirstSolid(ceiling: true, new Point(x2, y2), InnerBounds);
				x2 = point2.X;
				y2 = point2.Y + 1;
				Tile tile2 = Main.tile[x2, y2];
				Tile tile3 = Main.tile[x2, y2 - 1];
				if (tile2.active() || tile2.wall != brickWallType || !tile3.active() || tile3.type != brickCrackedTileType)
				{
					continue;
				}
				ushort type = 52;
				if (brickTileType == 383)
				{
					type = 62;
				}
				for (int num2 = unifiedRandom.Next(3, 12); num2 > 0; num2--)
				{
					Tile tile4 = Main.tile[x2, y2];
					if (tile4.active())
					{
						break;
					}
					tile4.ClearTile();
					tile4.active(active: true);
					tile4.type = type;
					if (b.HasValue && b.Value >= 0)
					{
						WorldGen.paintTile(x2, y2, b.Value, broadCast: false, paintEffects: false);
					}
					y2++;
				}
			}
		}

		public void LivingTreeRoom(DungeonData data, int i, int j, bool generating)
		{
			UnifiedRandom unifiedRandom = new UnifiedRandom(settings.RandomSeed);
			LivingTreeDungeonRoomSettings livingTreeDungeonRoomSettings = (LivingTreeDungeonRoomSettings)settings;
			_ = settings.StyleData.BrickTileType;
			_ = settings.StyleData.BrickCrackedTileType;
			_ = settings.StyleData.BrickWallType;
			if (settings.OverridePaintTile <= -1)
			{
				_ = settings.StyleData.TilePaintColor;
			}
			else
			{
				_ = settings.OverridePaintTile;
			}
			if (settings.OverridePaintWall <= -1)
			{
				_ = settings.StyleData.WallPaintColor;
			}
			else
			{
				_ = settings.OverridePaintWall;
			}
			List<Point> list = new List<Point>();
			Point item = new Point(i, j);
			if (calculated)
			{
				item = Positions[0];
			}
			else
			{
				list.Add(item);
			}
			Point point = new Point(item.X, item.Y + livingTreeDungeonRoomSettings.InnerHeight / 2);
			int num = point.Y - livingTreeDungeonRoomSettings.InnerHeight;
			int innerWidth = livingTreeDungeonRoomSettings.InnerWidth;
			int depth = livingTreeDungeonRoomSettings.Depth;
			int num2 = innerWidth;
			int num3 = num2 + depth;
			OuterBounds.SetBounds(item.X, item.Y, item.X, item.Y);
			InnerBounds.SetBounds(item.X, item.Y, item.X, item.Y);
			_outerShapeData.Clear();
			_innerShapeData.Clear();
			int num4 = 1;
			while ((!base.Processed && point.Y > num) || (base.Processed && num4 < Positions.Length))
			{
				_outerShapeData.AddBounds(point.X - num3 - item.X, point.Y - num3 - item.Y, point.X + num3 - item.X, point.Y + num3 - item.Y);
				_innerShapeData.AddBounds(point.X - num2 - item.X, point.Y - num2 - item.Y, point.X + num2 - item.X, point.Y + num2 - item.Y);
				if (!base.Processed)
				{
					list.Add(point);
				}
				GenerateDungeonSquareRoom(data, InnerBounds, OuterBounds, point.ToVector2D(), settings.StyleData, num2, num3, generating, generating);
				if (base.Processed)
				{
					num4++;
					if (num4 < Positions.Length)
					{
						point = Positions[num4];
					}
				}
				else
				{
					if (point.Y % 4 == 0)
					{
						point.X += ((unifiedRandom.Next(2) != 0) ? 1 : (-1));
					}
					point.Y--;
				}
			}
			if (!base.Processed)
			{
				Positions = Enumerable.ToArray(list);
			}
			InnerBounds.CalculateHitbox();
			OuterBounds.CalculateHitbox();
			_floodedTileCount = DungeonUtils.CalculateFloodedTileCountFromShapeData(InnerBounds, _innerShapeData);
		}
	}
}
