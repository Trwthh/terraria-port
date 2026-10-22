using System;
using Microsoft.Xna.Framework;
using ReLogic.Utilities;
using Terraria.GameContent.Generation.Dungeon.Features;
using Terraria.Utilities;
using Terraria.WorldBuilding;

namespace Terraria.GameContent.Generation.Dungeon.Rooms
{
	public class GenShapeDungeonRoom : DungeonRoom
	{
		private ShapeData _innerShapeData = new ShapeData();

		private ShapeData _outerShapeData = new ShapeData();

		private int _floodedTileCount;

		public GenShapeDungeonRoom(DungeonRoomSettings settings)
			: base(settings)
		{
			_ = (GenShapeDungeonRoomSettings)settings;
		}

		public override void CalculateRoom(DungeonData data)
		{
			calculated = false;
			int x = settings.RoomPosition.X;
			int y = settings.RoomPosition.Y;
			GenShapeRoom(data, x, y, generating: false);
			calculated = true;
		}

		public override bool GenerateRoom(DungeonData data)
		{
			generated = false;
			int x = settings.RoomPosition.X;
			int y = settings.RoomPosition.Y;
			GenShapeRoom(data, x, y, generating: true);
			generated = true;
			return true;
		}

		public override bool CanGenerateFeatureAt(DungeonData data, IDungeonFeature feature, int x, int y)
		{
			GenShapeType shapeType = ((GenShapeDungeonRoomSettings)settings).ShapeType;
			if ((uint)(shapeType - 2) <= 1u && feature is DungeonWindow)
			{
				return false;
			}
			return base.CanGenerateFeatureAt(data, feature, x, y);
		}

		public override void GenerateEarlyDungeonFeaturesInRoom(DungeonData data)
		{
			GenShapeDungeonRoomSettings genShapeDungeonRoomSettings = (GenShapeDungeonRoomSettings)settings;
			if (genShapeDungeonRoomSettings.ShapeType == GenShapeType.Doughnut)
			{
				byte? paintID = ((settings.OverridePaintTile > -1) ? new byte?((byte)settings.OverridePaintTile) : settings.StyleData.TilePaintColor);
				DungeonShapes.CircleRoom circleRoom = (DungeonShapes.CircleRoom)genShapeDungeonRoomSettings.InnerShape;
				Actions.SetTilePaint setTilePaint = new Actions.SetTilePaint(paintID);
				DungeonShapes.CircleRoom shape = new DungeonShapes.CircleRoom(Math.Max(1, circleRoom.HorizontalRadius / 2), Math.Max(1, circleRoom.VerticalRadius / 2));
				WorldUtils.Gen(InnerBounds.Center, shape, Actions.Chain(new Actions.ClearTile(), new Actions.SetTile(genShapeDungeonRoomSettings.StyleData.BrickTileType, setSelfFrames: false, setNeighborFrames: false, clearTile: false), setTilePaint));
			}
			base.GenerateEarlyDungeonFeaturesInRoom(data);
		}

		public override Point GetRoomCenterForDungeonFeature(DungeonData data, DungeonFeature feature)
		{
			GenShapeDungeonRoomSettings genShapeDungeonRoomSettings = (GenShapeDungeonRoomSettings)settings;
			Point roomCenterForDungeonFeature = base.GetRoomCenterForDungeonFeature(data, feature);
			if (feature is DungeonWindow && genShapeDungeonRoomSettings.ShapeType == GenShapeType.Mound)
			{
				roomCenterForDungeonFeature.Y += InnerBounds.Height / 5;
			}
			return roomCenterForDungeonFeature;
		}

		public override Point GetRoomCenterForHallway(Vector2D otherRoomPos)
		{
			GenShapeDungeonRoomSettings obj = (GenShapeDungeonRoomSettings)settings;
			Vector2D vector2D = base.GetRoomCenterForHallway(otherRoomPos).ToVector2D();
			Vector2D unitX = Vector2D.UnitX;
			DungeonRoomType roomType = obj.RoomType;
			if (roomType != DungeonRoomType.GenShapeDoughnut)
			{
				return vector2D.ToPoint();
			}
			unitX = (otherRoomPos - vector2D).SafeNormalize(Vector2D.UnitX);
			Point result = (vector2D + unitX * (InnerBounds.Size / 2)).ToPoint();
			result.X = (int)(vector2D.X + unitX.X * (double)(InnerBounds.Width / 2));
			result.Y = (int)(vector2D.Y + unitX.Y * (double)(InnerBounds.Height / 2));
			return result;
		}

		public override int GetFloodedRoomTileCount()
		{
			return _floodedTileCount;
		}

		public override void FloodRoom(byte liquidType)
		{
			GenShapeDungeonRoomSettings genShapeDungeonRoomSettings = (GenShapeDungeonRoomSettings)settings;
			WorldUtils.Gen(InnerBounds.Center, genShapeDungeonRoomSettings.InnerShape, Actions.Chain(new Modifiers.IsBelowHeight(InnerBounds.Center.Y, inclusive: true), new Modifiers.IsNotSolid(), new Actions.SetLiquid(liquidType)));
		}

		public override ProtectionType GetProtectionTypeFromPoint(int x, int y)
		{
			if (_innerShapeData == null || _outerShapeData == null || (calculated && !OuterBounds.Contains(x, y)))
			{
				return base.GetProtectionTypeFromPoint(x, y);
			}
			if (!_outerShapeData.Contains(x - base.Center.X, y - base.Center.Y))
			{
				return ProtectionType.None;
			}
			return ProtectionType.Walls;
		}

		public override bool IsInsideRoom(int x, int y)
		{
			if (base.IsInsideRoom(x, y))
			{
				return _innerShapeData.Contains(x - base.Center.X, y - base.Center.Y);
			}
			return false;
		}

		public void GenShapeRoom(DungeonData data, int i, int j, bool generating)
		{
			new UnifiedRandom(settings.RandomSeed);
			GenShapeDungeonRoomSettings genShapeDungeonRoomSettings = (GenShapeDungeonRoomSettings)settings;
			ushort brickTileType = settings.StyleData.BrickTileType;
			ushort brickWallType = settings.StyleData.BrickWallType;
			byte? paintID = ((settings.OverridePaintTile > -1) ? new byte?((byte)settings.OverridePaintTile) : settings.StyleData.TilePaintColor);
			byte? paintID2 = ((settings.OverridePaintWall > -1) ? new byte?((byte)settings.OverridePaintWall) : settings.StyleData.WallPaintColor);
			Vector2D v = new Vector2D(i, j);
			if (base.Processed)
			{
				v = base.Center;
			}
			InnerBounds.SetBounds((int)v.X, (int)v.Y, (int)v.X, (int)v.Y);
			OuterBounds.SetBounds((int)v.X, (int)v.Y, (int)v.X, (int)v.Y);
			Actions.SetTilePaint setTilePaint = new Actions.SetTilePaint(paintID);
			Actions.SetWallPaint setWallPaint = new Actions.SetWallPaint(paintID2);
			WorldUtils.Gen(v.ToPoint(), genShapeDungeonRoomSettings.OuterShape, Actions.Chain(new Modifiers.Expand(1), new Actions.UpdateBounds(data.dungeonBounds).Output(_outerShapeData), new Actions.UpdateBounds(OuterBounds), new Modifiers.Conditions(new Conditions.BoolCheck(generating)), new Actions.ClearTile(), new Actions.SetTile(brickTileType, setSelfFrames: false, setNeighborFrames: false, clearTile: false), setTilePaint));
			if (generating)
			{
				WorldUtils.Gen(v.ToPoint(), genShapeDungeonRoomSettings.OuterShape, Actions.Chain(new Actions.SetWall(brickWallType, setSelfFrames: false, setNeighborFrames: false, clearTile: false), setWallPaint));
			}
			WorldUtils.Gen(v.ToPoint(), genShapeDungeonRoomSettings.InnerShape, Actions.Chain(new Actions.UpdateBounds(data.dungeonBounds).Output(_innerShapeData), new Actions.UpdateBounds(InnerBounds), new Modifiers.Conditions(new Conditions.BoolCheck(generating)), new Actions.Clear(), new Actions.SetWall(brickWallType, setSelfFrames: false, setNeighborFrames: false, clearTile: false), setWallPaint));
			InnerBounds.CalculateHitbox();
			OuterBounds.CalculateHitbox();
			_floodedTileCount = DungeonUtils.CalculateFloodedTileCountFromShapeData(InnerBounds, _innerShapeData);
		}
	}
}
