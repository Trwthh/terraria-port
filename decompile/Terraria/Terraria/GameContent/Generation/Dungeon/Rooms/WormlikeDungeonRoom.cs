using System;
using System.Collections.Generic;
using System.Linq;
using Microsoft.Xna.Framework;
using Terraria.Utilities;
using Terraria.WorldBuilding;

namespace Terraria.GameContent.Generation.Dungeon.Rooms
{
	public class WormlikeDungeonRoom : DungeonRoom
	{
		private ShapeData _innerShapeData = new ShapeData();

		private ShapeData _outerShapeData = new ShapeData();

		private int _floodedTileCount;

		public int InnerBoundsSizeMin;

		public int InnerBoundsSizeMax;

		public Vector2[] Positions;

		public WormlikeDungeonRoom(DungeonRoomSettings settings)
			: base(settings)
		{
		}

		public override void CalculateRoom(DungeonData data)
		{
			calculated = false;
			int x = settings.RoomPosition.X;
			int y = settings.RoomPosition.Y;
			WormlikeRoom(data, x, y, generating: false);
			calculated = true;
		}

		public override bool GenerateRoom(DungeonData data)
		{
			generated = false;
			int x = settings.RoomPosition.X;
			int y = settings.RoomPosition.Y;
			WormlikeRoom(data, x, y, generating: true);
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
			WorldUtils.Gen(Positions[0].ToPoint(), new ModShapes.All(_innerShapeData), Actions.Chain(new Modifiers.IsBelowHeight(base.Center.Y, inclusive: true), new Modifiers.IsNotSolid(), new Actions.SetLiquid(liquidType)));
		}

		public override ProtectionType GetProtectionTypeFromPoint(int x, int y)
		{
			if (_innerShapeData == null || _outerShapeData == null || Positions == null || (calculated && !OuterBounds.Contains(x, y)))
			{
				return base.GetProtectionTypeFromPoint(x, y);
			}
			Point point = Positions[0].ToPoint();
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
			Point point = Positions[0].ToPoint();
			if (base.IsInsideRoom(x, y))
			{
				return _innerShapeData.Contains(x - point.X, y - point.Y);
			}
			return false;
		}

		public void WormlikeRoom(DungeonData data, int i, int j, bool generating)
		{
			UnifiedRandom unifiedRandom = new UnifiedRandom(settings.RandomSeed);
			WormlikeDungeonRoomSettings wormlikeDungeonRoomSettings = (WormlikeDungeonRoomSettings)settings;
			Point p = new Point(i, j);
			if (base.Processed)
			{
				p = Positions[0].ToPoint();
			}
			int num = 9 + unifiedRandom.Next(3);
			int num2 = Math.Max(4, num / 5);
			if (base.Processed)
			{
				num = InnerBoundsSizeMax;
				num2 = InnerBoundsSizeMin;
			}
			int num3 = num;
			int num4 = 8;
			int num5 = num + num4;
			InnerBounds.SetBounds(p.X, p.Y, p.X, p.Y);
			OuterBounds.SetBounds(p.X, p.Y, p.X, p.Y);
			_outerShapeData.Clear();
			_innerShapeData.Clear();
			Vector2 vector = p.ToVector2();
			Vector2 vector2 = vector;
			List<Vector2> list = new List<Vector2>();
			vector = vector2;
			Vector2 vector3 = unifiedRandom.NextVector2CircularEdge(1f, 1f);
			Vector2 spinningpoint = vector3;
			int firstSideIterations = wormlikeDungeonRoomSettings.FirstSideIterations;
			int num6 = 0;
			for (int k = 0; k < firstSideIterations; k++)
			{
				float num7 = (float)k / (float)firstSideIterations;
				num3 = (int)Utils.Lerp(num, num2, num7);
				num5 = num3 + num4;
				Point point = vector.ToPoint();
				_outerShapeData.AddBounds(point.X - num5 - (int)vector2.X, point.Y - num5 - (int)vector2.Y, point.X + num5 - (int)vector2.X, point.Y + num5 - (int)vector2.Y);
				_innerShapeData.AddBounds(point.X - num3 - (int)vector2.X, point.Y - num3 - (int)vector2.Y, point.X + num3 - (int)vector2.X, point.Y + num3 - (int)vector2.Y);
				if (!base.Processed)
				{
					list.Add(vector);
				}
				GenerateDungeonSquareRoom(data, InnerBounds, OuterBounds, vector.ToVector2D(), settings.StyleData, num3, num5, generating, generating);
				if (base.Processed)
				{
					num6++;
					if (num6 < Positions.Length)
					{
						vector = Positions[num6];
					}
				}
				else
				{
					vector += vector3;
					vector3 = spinningpoint.RotatedBy(Utils.Lerp(0.0, 1.5707963705062866, num7));
				}
			}
			vector = vector2;
			vector3 = spinningpoint.RotatedBy(3.1415927410125732, Vector2.Zero).RotatedByRandom(0.7853981852531433);
			spinningpoint = vector3;
			firstSideIterations = wormlikeDungeonRoomSettings.SecondSideIterations;
			for (int l = 0; l < firstSideIterations; l++)
			{
				float num8 = (float)l / (float)firstSideIterations;
				num3 = (int)Utils.Lerp(num, num2, num8);
				num5 = num3 + num4;
				Point point2 = vector.ToPoint();
				_outerShapeData.AddBounds(point2.X - num5 - (int)vector2.X, point2.Y - num5 - (int)vector2.Y, point2.X + num5 - (int)vector2.X, point2.Y + num5 - (int)vector2.Y);
				_innerShapeData.AddBounds(point2.X - num3 - (int)vector2.X, point2.Y - num3 - (int)vector2.Y, point2.X + num3 - (int)vector2.X, point2.Y + num3 - (int)vector2.Y);
				if (!base.Processed)
				{
					list.Add(vector);
				}
				GenerateDungeonSquareRoom(data, InnerBounds, OuterBounds, vector.ToVector2D(), settings.StyleData, num3, num5, generating, generating);
				if (base.Processed)
				{
					num6++;
					if (num6 < Positions.Length)
					{
						vector = Positions[num6];
					}
				}
				else
				{
					vector += vector3;
					vector3 = spinningpoint.RotatedBy(Utils.Lerp(0.0, 1.5707963705062866, num8));
				}
			}
			if (!base.Processed)
			{
				Positions = Enumerable.ToArray(list);
			}
			InnerBoundsSizeMin = num2;
			InnerBoundsSizeMax = num;
			InnerBounds.CalculateHitbox();
			OuterBounds.CalculateHitbox();
			_floodedTileCount = DungeonUtils.CalculateFloodedTileCountFromShapeData(InnerBounds, _innerShapeData);
		}
	}
}
