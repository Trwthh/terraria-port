using System;
using ReLogic.Utilities;
using Terraria.GameContent.Generation.Dungeon.Features;
using Terraria.Utilities;
using Terraria.WorldBuilding;

namespace Terraria.GameContent.Generation.Dungeon.Rooms
{
	public class LegacyDungeonRoom : DungeonRoom
	{
		private ShapeData _innerShapeData = new ShapeData();

		private ShapeData _outerShapeData = new ShapeData();

		private int _floodedTileCount;

		public Vector2D StartPosition;

		public Vector2D EndPosition;

		public int Strength;

		public LegacyDungeonRoom(DungeonRoomSettings settings)
			: base(settings)
		{
		}

		public override void CalculateRoom(DungeonData data)
		{
			calculated = false;
			int x = settings.RoomPosition.X;
			int y = settings.RoomPosition.Y;
			LegacyRoom(data, x, y, generating: false);
			calculated = true;
		}

		public override bool GenerateRoom(DungeonData data)
		{
			generated = false;
			int x = settings.RoomPosition.X;
			int y = settings.RoomPosition.Y;
			LegacyRoom(data, x, y, generating: true);
			generated = true;
			return true;
		}

		public override int GetFloodedRoomTileCount()
		{
			return _floodedTileCount;
		}

		public override void FloodRoom(byte liquidType)
		{
			if (generated && _innerShapeData != null)
			{
				WorldUtils.Gen(StartPosition.ToPoint(), new ModShapes.All(_innerShapeData), Actions.Chain(new Modifiers.IsBelowHeight(InnerBounds.Center.Y, inclusive: true), new Modifiers.IsNotSolid(), new Actions.SetLiquid(liquidType)));
			}
		}

		public override ProtectionType GetProtectionTypeFromPoint(int x, int y)
		{
			if (_innerShapeData == null || _outerShapeData == null || (calculated && !OuterBounds.Contains(x, y)))
			{
				return base.GetProtectionTypeFromPoint(x, y);
			}
			if (!_outerShapeData.Contains(x - (int)StartPosition.X, y - (int)StartPosition.Y))
			{
				return ProtectionType.None;
			}
			return ProtectionType.Walls;
		}

		public override bool IsInsideRoom(int x, int y)
		{
			if (base.IsInsideRoom(x, y))
			{
				return _innerShapeData.Contains(x - (int)StartPosition.X, y - (int)StartPosition.Y);
			}
			return false;
		}

		public override bool TryGenerateChestInRoom(DungeonData data, DungeonGlobalBasicChests feature)
		{
			Vector2D endPosition = EndPosition;
			int num = (int)((float)Strength * 0.4f);
			return DungeonUtils.GenerateDungeonRegularChest(data, feature, settings.StyleData, (int)endPosition.X - num, (int)endPosition.Y - num, (int)endPosition.X + num, (int)endPosition.Y + num);
		}

		public override bool DualDungeons_TryGenerateBiomeChestInRoom(DungeonData data, DungeonGlobalBiomeChests feature)
		{
			Vector2D endPosition = EndPosition;
			int num = (int)((float)Strength * 0.4f);
			return DungeonUtils.GenerateDungeonBiomeChest(data, feature, settings.StyleData, (int)endPosition.X - num, (int)endPosition.Y - num, (int)endPosition.X + num, (int)endPosition.Y + num);
		}

		public void LegacyRoom(DungeonData data, int i, int j, bool generating)
		{
			LegacyDungeonRoomSettings legacyDungeonRoomSettings = (LegacyDungeonRoomSettings)settings;
			UnifiedRandom unifiedRandom = new UnifiedRandom(legacyDungeonRoomSettings.RandomSeed);
			ushort brickTileType = settings.StyleData.BrickTileType;
			ushort brickWallType = settings.StyleData.BrickWallType;
			byte? b = ((settings.OverridePaintTile > -1) ? new byte?((byte)settings.OverridePaintTile) : settings.StyleData.TilePaintColor);
			byte? b2 = ((settings.OverridePaintWall > -1) ? new byte?((byte)settings.OverridePaintWall) : settings.StyleData.WallPaintColor);
			double num = data.roomStrengthScalar;
			if (legacyDungeonRoomSettings.StartingRoom)
			{
				num = 1.0;
			}
			double num2 = (int)(15.0 * num) + unifiedRandom.Next(15);
			Vector2D vector2D = default(Vector2D);
			vector2D.X = (double)((float)unifiedRandom.Next(-10, 11) * 0.1f) * data.roomSlantVariantScalar;
			vector2D.Y = (double)((float)unifiedRandom.Next(-10, 11) * 0.1f) * data.roomSlantVariantScalar;
			if (vector2D.X == 0.0 && vector2D.Y == 0.0)
			{
				if (unifiedRandom.Next(2) == 0)
				{
					vector2D.X = ((unifiedRandom.Next(2) != 0) ? 1 : (-1));
				}
				else
				{
					vector2D.Y = ((unifiedRandom.Next(2) != 0) ? 1 : (-1));
				}
			}
			Vector2D vector2D2 = default(Vector2D);
			vector2D2.X = i;
			vector2D2.Y = (double)j - num2 / 2.0;
			if (calculated)
			{
				vector2D2 = StartPosition;
			}
			Vector2D startPosition = vector2D2;
			double num3 = data.roomStepScalar;
			if (legacyDungeonRoomSettings.StartingRoom)
			{
				num3 = 1.0;
			}
			int num4 = (int)(10.0 * num3) + unifiedRandom.Next(10);
			double num5 = num2;
			double num6 = data.roomInteriorToExteriorRatio;
			if (legacyDungeonRoomSettings.OverrideStartPosition != default(Vector2D) && legacyDungeonRoomSettings.OverrideEndPosition != default(Vector2D))
			{
				vector2D2 = (startPosition = legacyDungeonRoomSettings.OverrideStartPosition);
				Vector2D v = legacyDungeonRoomSettings.OverrideEndPosition - vector2D2;
				vector2D = v.SafeNormalize(Vector2D.UnitX);
				num4 = (int)Math.Ceiling(v.Length() / vector2D.Length());
			}
			else if (legacyDungeonRoomSettings.OverrideVelocity != default(Vector2D))
			{
				vector2D = legacyDungeonRoomSettings.OverrideVelocity;
			}
			if (legacyDungeonRoomSettings.OverrideStrength > 0)
			{
				num2 = (num5 = legacyDungeonRoomSettings.OverrideStrength);
			}
			if (legacyDungeonRoomSettings.OverrideSteps > 0)
			{
				num4 = legacyDungeonRoomSettings.OverrideSteps;
			}
			if (legacyDungeonRoomSettings.OverrideInteriorToExteriorRatio > 0.0)
			{
				num6 = legacyDungeonRoomSettings.OverrideInteriorToExteriorRatio;
			}
			InnerBounds.SetBounds((int)vector2D2.X, (int)vector2D2.Y, (int)vector2D2.X, (int)vector2D2.Y);
			OuterBounds.SetBounds((int)vector2D2.X, (int)vector2D2.Y, (int)vector2D2.X, (int)vector2D2.Y);
			while (num4 > 0)
			{
				num4--;
				int num7 = Math.Max(0, Math.Min(Main.maxTilesX - 1, (int)(vector2D2.X - num2 * 0.800000011920929 - 5.0)));
				int num8 = Math.Max(0, Math.Min(Main.maxTilesX - 1, (int)(vector2D2.X + num2 * 0.800000011920929 + 5.0)));
				int num9 = Math.Max(0, Math.Min(Main.maxTilesY - 1, (int)(vector2D2.Y - num2 * 0.800000011920929 - 5.0)));
				int num10 = Math.Max(0, Math.Min(Main.maxTilesY - 1, (int)(vector2D2.Y + num2 * 0.800000011920929 + 5.0)));
				if (legacyDungeonRoomSettings.IsEntranceRoom && data.Type == DungeonType.DualDungeon)
				{
					num10 = Math.Max(num10, DungeonUtils.GetDualDungeonBrickSupportCutoffY(data));
				}
				data.dungeonBounds.UpdateBounds(num7, num9, num8 - 1, num10 - 1);
				OuterBounds.UpdateBounds(num7, num9, num8 - 1, num10 - 1);
				int num11 = Math.Max(0, Math.Min(Main.maxTilesX - 1, (int)(vector2D2.X - num2 * num6)));
				int num12 = Math.Max(0, Math.Min(Main.maxTilesX - 1, (int)(vector2D2.X + num2 * num6)));
				int num13 = Math.Max(0, Math.Min(Main.maxTilesY - 1, (int)(vector2D2.Y - num2 * num6)));
				int num14 = Math.Max(0, Math.Min(Main.maxTilesY - 1, (int)(vector2D2.Y + num2 * num6)));
				InnerBounds.UpdateBounds(num11, num13, num12 - 1, num14 - 1);
				for (int k = num7; k < num8; k++)
				{
					for (int l = num9; l < num10; l++)
					{
						if (!generating)
						{
							_outerShapeData.Add(k - (int)startPosition.X, l - (int)startPosition.Y);
							if (k >= num11 && k <= num12 && l >= num13 && l <= num14)
							{
								_innerShapeData.Add(k - (int)startPosition.X, l - (int)startPosition.Y);
							}
							continue;
						}
						Main.tile[k, l].liquid = 0;
						bool resetTile = k >= num7 + 1 && k < num8 - 1 && l >= num9 + 1 && l < num10 - 1;
						if (CanPlaceTileAt(data, Main.tile[k, l], brickTileType, brickWallType, settings.StyleData.Style))
						{
							DungeonUtils.ChangeTileType(Main.tile[k, l], brickTileType, resetTile, b);
						}
					}
				}
				if (generating)
				{
					for (int m = num7 + 1; m < num8 - 1; m++)
					{
						for (int n = num9 + 1; n < num10 - 1; n++)
						{
							if (CanRemoveTileAt(data, Main.tile[m, n]))
							{
								DungeonUtils.ChangeWallType(Main.tile[m, n], brickWallType, resetTile: false, b2);
							}
						}
					}
				}
				num7 = num11;
				num8 = num12;
				num9 = num13;
				num10 = num14;
				if (generating)
				{
					for (int num15 = num7; num15 < num8; num15++)
					{
						for (int num16 = num9; num16 < num10; num16++)
						{
							DungeonUtils.ChangeWallType(Main.tile[num15, num16], brickWallType, resetTile: true, b2);
						}
					}
				}
				vector2D2 += vector2D;
				vector2D.X = Math.Max(-1.0, Math.Min(1.0, vector2D.X + (double)((float)unifiedRandom.Next(-10, 11) * 0.05f) * data.roomSlantVariantScalar));
				vector2D.Y = Math.Max(-1.0, Math.Min(1.0, vector2D.Y + (double)((float)unifiedRandom.Next(-10, 11) * 0.05f) * data.roomSlantVariantScalar));
			}
			StartPosition = startPosition;
			EndPosition = vector2D2;
			Strength = (int)num5;
			InnerBounds.CalculateHitbox();
			OuterBounds.CalculateHitbox();
			_floodedTileCount = DungeonUtils.CalculateFloodedTileCountFromShapeData(InnerBounds, _innerShapeData);
		}
	}
}
