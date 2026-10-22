using System;
using System.Collections.Generic;
using ReLogic.Utilities;
using Terraria.GameContent.Generation.Dungeon.Rooms;
using Terraria.Utilities;

namespace Terraria.GameContent.Generation.Dungeon.Halls
{
	public class RegularDungeonHall : DungeonHall
	{
		public RegularDungeonHall(DungeonHallSettings settings)
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
			RegularHall(data, startPoint, endPoint);
			calculated = true;
		}

		public override void GenerateHall(DungeonData data)
		{
			generated = false;
			RegularHall(data, StartPosition, EndPosition, generating: true);
			generated = true;
		}

		public void RegularHall(DungeonData data, Vector2D startPoint, Vector2D endPoint, bool generating = false)
		{
			RegularDungeonHallSettings regularDungeonHallSettings = (RegularDungeonHallSettings)settings;
			UnifiedRandom unifiedRandom = new UnifiedRandom(regularDungeonHallSettings.RandomSeed);
			Vector2D currentPoint = startPoint;
			bool flag = false;
			if (regularDungeonHallSettings.CrackedBrickChance > 0.0)
			{
				flag = unifiedRandom.NextDouble() <= regularDungeonHallSettings.CrackedBrickChance;
			}
			int num = 3;
			int num2 = 8;
			if (regularDungeonHallSettings.OverrideInnerBoundsSize > 0)
			{
				num = regularDungeonHallSettings.OverrideInnerBoundsSize;
			}
			if (regularDungeonHallSettings.OverrideOuterBoundsSize > 0)
			{
				num2 = regularDungeonHallSettings.OverrideOuterBoundsSize;
			}
			int num3 = num + num2;
			Vector2D v = endPoint - startPoint;
			Vector2D vector2D = v.SafeNormalize(Vector2D.UnitX);
			int num4 = (int)Math.Ceiling(v.Length() / vector2D.Length());
			Bounds.SetBounds((int)startPoint.X, (int)startPoint.Y, (int)startPoint.X, (int)startPoint.Y);
			Vector2D startPos = startPoint;
			Vector2D endPos = endPoint;
			DungeonRoomSearchSettings dungeonRoomSearchSettings = new DungeonRoomSearchSettings
			{
				Fluff = num3
			};
			List<DungeonRoom> allRoomsInSpots = DungeonUtils.GetAllRoomsInSpots(data.dungeonRooms, startPos, endPos, dungeonRoomSearchSettings);
			Vector2D vector2D2 = vector2D;
			while (num4 > 0 && WorldGen.InWorld((int)(currentPoint.X + vector2D.X), (int)(currentPoint.Y + vector2D.Y), 10))
			{
				if (!base.Processed)
				{
					data.dungeonBounds.UpdateBounds((int)currentPoint.X - num3, (int)currentPoint.Y - num3, (int)currentPoint.Y + num3, (int)currentPoint.Y + num3);
					Bounds.UpdateBounds((int)currentPoint.X - num3, (int)currentPoint.Y - num3, (int)currentPoint.Y + num3, (int)currentPoint.Y + num3);
				}
				if (generating)
				{
					GenerateDungeonSquareHall(data, allRoomsInSpots, currentPoint, settings.StyleData, num, num2, regularDungeonHallSettings.PlaceOverProtectedBricks, flag);
				}
				currentPoint += vector2D;
				num4--;
			}
			data.genVars.generatingDungeonPositionX = (int)endPoint.X;
			data.genVars.generatingDungeonPositionY = (int)endPoint.Y;
			StartPosition = startPoint;
			EndPosition = endPoint;
			StartDirection = new Vector2D(vector2D2.X, vector2D2.Y);
			EndDirection = new Vector2D(vector2D.X, vector2D.Y);
			CrackedBrick = flag;
			Bounds.CalculateHitbox();
		}
	}
}
