using System;
using System.Collections.Generic;
using Microsoft.Xna.Framework;
using ReLogic.Utilities;
using Terraria.GameContent.Generation.Dungeon.Rooms;
using Terraria.Utilities;

namespace Terraria.GameContent.Generation.Dungeon.Halls
{
	public class SineDungeonHall : DungeonHall
	{
		public List<Tuple<Vector2D, Vector2D>> PotentialPlatformPoints = new List<Tuple<Vector2D, Vector2D>>();

		public SineDungeonHall(DungeonHallSettings settings)
			: base(settings)
		{
		}

		public override void CalculatePlatformsAndDoors(DungeonData data)
		{
			if (!base.Processed)
			{
				return;
			}
			DungeonUtils.CalculatePlatformAndDoorsOnHallway(data, StartPosition, StartDirection.Y, settings.ForceStyleForDoorsAndPlatforms ? settings.StyleData : null);
			DungeonUtils.CalculatePlatformAndDoorsOnHallway(data, EndPosition, EndDirection.Y, settings.ForceStyleForDoorsAndPlatforms ? settings.StyleData : null);
			float num = 0.65f;
			for (int i = 0; i < PotentialPlatformPoints.Count; i++)
			{
				Tuple<Vector2D, Vector2D> tuple = PotentialPlatformPoints[i];
				Vector2D item = tuple.Item1;
				Vector2D item2 = tuple.Item2;
				if (!(item2.Y < (double)num) || !(item2.Y > (double)(0f - num)))
				{
					DungeonUtils.CalculatePlatformAndDoorsOnHallway(data, item, item2.Y, settings.ForceStyleForDoorsAndPlatforms ? settings.StyleData : null);
				}
			}
		}

		public override void CalculateHall(DungeonData data, Vector2D startPoint, Vector2D endPoint)
		{
			calculated = false;
			SineHall(data, startPoint, endPoint);
			calculated = true;
		}

		public override void GenerateHall(DungeonData data)
		{
			generated = false;
			SineHall(data, StartPosition, EndPosition, generating: true);
			generated = true;
		}

		public void SineHall(DungeonData data, Vector2D startPoint, Vector2D endPoint, bool generating = false)
		{
			SineDungeonHallSettings sineDungeonHallSettings = (SineDungeonHallSettings)settings;
			UnifiedRandom unifiedRandom = new UnifiedRandom(sineDungeonHallSettings.RandomSeed);
			Vector2D vector2D = startPoint;
			bool flag = false;
			if (sineDungeonHallSettings.CrackedBrickChance > 0.0)
			{
				flag = unifiedRandom.NextDouble() <= sineDungeonHallSettings.CrackedBrickChance;
			}
			int num = 3;
			int num2 = 8;
			int num3 = num + num2;
			Vector2D v = endPoint - startPoint;
			Vector2D vector2D2 = v.SafeNormalize(Vector2D.UnitX);
			int num4 = (int)Math.Ceiling(v.Length() / vector2D2.Length());
			int num5 = num4;
			Bounds.SetBounds((int)startPoint.X, (int)startPoint.Y, (int)startPoint.X, (int)startPoint.Y);
			Vector2D startPos = startPoint;
			Vector2D endPos = endPoint;
			DungeonRoomSearchSettings dungeonRoomSearchSettings = new DungeonRoomSearchSettings
			{
				Fluff = (int)((float)num * sineDungeonHallSettings.Magnitude) + num2
			};
			List<DungeonRoom> allRoomsInSpots = DungeonUtils.GetAllRoomsInSpots(data.dungeonRooms, startPos, endPos, dungeonRoomSearchSettings);
			Vector2D v2 = vector2D2;
			Vector2D v3 = (Vector2.UnitY * sineDungeonHallSettings.Magnitude).ToVector2D();
			v3 = v3.ToVector2().RotatedBy(v2.ToRotation(), Vector2.Zero).ToVector2D();
			float num6 = 0f;
			float num7 = 0f;
			float num8 = (float)sineDungeonHallSettings.Iterations / (float)num5 * ((float)Math.PI * 2f);
			while (num4 > 0)
			{
				Vector2D vector2D3 = v3 * (float)Math.Sin(num7);
				Vector2D currentPoint = (sineDungeonHallSettings.FlipSine ? (vector2D - vector2D3) : (vector2D + vector2D3));
				if (!WorldGen.InWorld((int)(currentPoint.X + vector2D2.X), (int)(currentPoint.Y + vector2D2.Y), 10))
				{
					break;
				}
				if (!base.Processed)
				{
					data.dungeonBounds.UpdateBounds((int)currentPoint.X - num3, (int)currentPoint.Y - num3, (int)currentPoint.Y + num3, (int)currentPoint.Y + num3);
					Bounds.UpdateBounds((int)currentPoint.X - num3, (int)currentPoint.Y - num3, (int)currentPoint.Y + num3, (int)currentPoint.Y + num3);
				}
				if (generating)
				{
					GenerateDungeonSquareHall(data, allRoomsInSpots, currentPoint, settings.StyleData, num, num2, sineDungeonHallSettings.PlaceOverProtectedBricks, flag);
				}
				vector2D += vector2D2;
				num6 += num8;
				num7 += num8;
				if (num6 >= 0.5f)
				{
					num6 = 0f;
					if (!base.Processed)
					{
						PotentialPlatformPoints.Add(new Tuple<Vector2D, Vector2D>(vector2D, vector2D3));
					}
				}
				num4--;
			}
			data.genVars.generatingDungeonPositionX = (int)endPoint.X;
			data.genVars.generatingDungeonPositionY = (int)endPoint.Y;
			StartPosition = startPoint;
			EndPosition = endPoint;
			StartDirection = new Vector2D(v2.X, v2.Y);
			EndDirection = new Vector2D(vector2D2.X, vector2D2.Y);
			CrackedBrick = flag;
			Bounds.CalculateHitbox();
		}
	}
}
