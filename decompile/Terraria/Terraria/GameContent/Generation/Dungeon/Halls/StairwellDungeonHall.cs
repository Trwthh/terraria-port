using System;
using System.Collections.Generic;
using ReLogic.Utilities;
using Terraria.GameContent.Generation.Dungeon.Rooms;
using Terraria.Utilities;

namespace Terraria.GameContent.Generation.Dungeon.Halls
{
	public class StairwellDungeonHall : DungeonHall
	{
		public StairwellDungeonHall(StairwellDungeonHallSettings settings)
			: base(settings)
		{
		}

		public override void CalculatePlatformsAndDoors(DungeonData data)
		{
			if (base.Processed)
			{
				DungeonUtils.CalculatePlatformAndDoorsOnHallway(data, StartPosition, StartDirection.Y, settings.ForceStyleForDoorsAndPlatforms ? settings.StyleData : null);
				DungeonUtils.CalculatePlatformAndDoorsOnHallway(data, EndPosition, EndDirection.Y, settings.ForceStyleForDoorsAndPlatforms ? settings.StyleData : null);
			}
		}

		public override void CalculateHall(DungeonData data, Vector2D startPoint, Vector2D endPoint)
		{
			calculated = false;
			Stairwell(data, startPoint, endPoint);
			calculated = true;
		}

		public override void GenerateHall(DungeonData data)
		{
			generated = false;
			Stairwell(data, StartPosition, EndPosition, generating: true);
			generated = true;
		}

		public void Stairwell(DungeonData data, Vector2D startPoint, Vector2D endPoint, bool generating = false)
		{
			StairwellDungeonHallSettings stairwellDungeonHallSettings = (StairwellDungeonHallSettings)settings;
			UnifiedRandom unifiedRandom = new UnifiedRandom(stairwellDungeonHallSettings.RandomSeed);
			Bounds.SetBounds((int)startPoint.X, (int)startPoint.Y, (int)startPoint.X, (int)startPoint.Y);
			bool flag = false;
			if (stairwellDungeonHallSettings.CrackedBrickChance > 0.0)
			{
				flag = unifiedRandom.NextDouble() <= stairwellDungeonHallSettings.CrackedBrickChance;
			}
			int innerBoundsSize = stairwellDungeonHallSettings.InnerBoundsSize;
			int outerBoundsSize = stairwellDungeonHallSettings.OuterBoundsSize;
			int num = innerBoundsSize + outerBoundsSize;
			List<DungeonRoom> allRoomsInSpots = DungeonUtils.GetAllRoomsInSpots(data.dungeonRooms, startPoint, endPoint, new DungeonRoomSearchSettings
			{
				Fluff = num + stairwellDungeonHallSettings.MaxDistFromLine
			});
			Vector2D vector2D = endPoint - startPoint;
			Vector2D vector2D2 = CalculateZigZagSlope(startPoint, endPoint, stairwellDungeonHallSettings.Gradient);
			double num2 = Math.Ceiling(Math.Abs(Vector2D.Cross(vector2D2, vector2D.SafeNormalize(default(Vector2D)))) / (double)stairwellDungeonHallSettings.MaxDistFromLine);
			vector2D2 /= num2;
			Vector2D startPoint2 = startPoint;
			for (int i = 0; (double)i < num2; i++)
			{
				Vector2D vector2D3 = startPoint + vector2D * i / num2;
				Vector2D vector2D4 = startPoint + vector2D * (i + 1) / num2;
				Vector2D vector2D5 = vector2D3 + vector2D2 + unifiedRandom.NextVector2DCircular(stairwellDungeonHallSettings.PointVariance, stairwellDungeonHallSettings.PointVariance);
				Vector2D vector2D6 = vector2D4 - vector2D2 + unifiedRandom.NextVector2DCircular(stairwellDungeonHallSettings.PointVariance, stairwellDungeonHallSettings.PointVariance);
				GenerateHallway(data, startPoint2, vector2D5, allRoomsInSpots, flag, generating);
				GenerateHallway(data, vector2D5, vector2D6, allRoomsInSpots, flag, generating);
				startPoint2 = vector2D6;
			}
			GenerateHallway(data, startPoint2, endPoint, allRoomsInSpots, flag, generating);
			data.genVars.generatingDungeonPositionX = (int)endPoint.X;
			data.genVars.generatingDungeonPositionY = (int)endPoint.Y;
			StartPosition = startPoint;
			EndPosition = endPoint;
			StartDirection = (EndDirection = vector2D2.SafeNormalize(default(Vector2D)));
			CrackedBrick = flag;
			Bounds.CalculateHitbox();
		}

		private Vector2D CalculateZigZagSlope(Vector2D startPoint, Vector2D endPoint, double gradient)
		{
			Vector2D vector2D = (endPoint - startPoint).SafeNormalize(Vector2D.UnitY);
			Vector2D vector2D2 = new Vector2D((vector2D.X > 0.0) ? 1 : (-1), gradient).SafeNormalize(default(Vector2D));
			double num = Vector2D.Distance(startPoint, endPoint) * (vector2D.X / vector2D2.X + vector2D.Y / vector2D2.Y) / 4.0;
			return vector2D2 * num;
		}

		private void GenerateHallway(DungeonData data, Vector2D startPoint, Vector2D endPoint, List<DungeonRoom> roomsInArea, bool crackedBricks, bool generating)
		{
			StairwellDungeonHallSettings stairwellDungeonHallSettings = (StairwellDungeonHallSettings)settings;
			int innerBoundsSize = stairwellDungeonHallSettings.InnerBoundsSize;
			int outerBoundsSize = stairwellDungeonHallSettings.OuterBoundsSize;
			int num = innerBoundsSize + outerBoundsSize;
			Vector2D vector2D = endPoint - startPoint;
			double num2 = Math.Ceiling(Math.Max(Math.Abs(vector2D.X), Math.Abs(vector2D.Y)));
			for (int i = 0; (double)i < num2; i++)
			{
				Vector2D currentPoint = startPoint + vector2D * ((double)i / num2);
				if (!base.Processed)
				{
					data.dungeonBounds.UpdateBounds((int)currentPoint.X - num, (int)currentPoint.Y - num, (int)currentPoint.Y + num, (int)currentPoint.Y + num);
					Bounds.UpdateBounds((int)currentPoint.X - num, (int)currentPoint.Y - num, (int)currentPoint.Y + num, (int)currentPoint.Y + num);
				}
				if (generating)
				{
					GenerateDungeonSquareHall(data, roomsInArea, currentPoint, settings.StyleData, innerBoundsSize, outerBoundsSize, stairwellDungeonHallSettings.PlaceOverProtectedBricks, crackedBricks, stairwellDungeonHallSettings.IsEntranceHall);
				}
			}
		}

		public override bool CanPlaceTileAt(DungeonData data, Tile tile, int tileType, int tileCrackedType)
		{
			if (((StairwellDungeonHallSettings)settings).IsEntranceHall && ((tile.active() && Main.tileDungeon[tile.type]) || Main.wallDungeon[tile.wall]))
			{
				return false;
			}
			return base.CanPlaceTileAt(data, tile, tileType, tileCrackedType);
		}
	}
}
