using Microsoft.Xna.Framework;
using Terraria.Utilities;

namespace Terraria.GameContent.Generation.Dungeon.Rooms
{
	public class RegularDungeonRoom : DungeonRoom
	{
		public int _innerBoundsSize;

		public RegularDungeonRoom(DungeonRoomSettings settings)
			: base(settings)
		{
		}

		public override void CalculateRoom(DungeonData data)
		{
			calculated = false;
			int x = settings.RoomPosition.X;
			int y = settings.RoomPosition.Y;
			RegularRoom(data, x, y, generating: false);
			calculated = true;
		}

		public override bool GenerateRoom(DungeonData data)
		{
			generated = false;
			int x = settings.RoomPosition.X;
			int y = settings.RoomPosition.Y;
			RegularRoom(data, x, y, generating: true);
			generated = true;
			return true;
		}

		public void RegularRoom(DungeonData data, int i, int j, bool generating)
		{
			UnifiedRandom unifiedRandom = new UnifiedRandom(settings.RandomSeed);
			RegularDungeonRoomSettings regularDungeonRoomSettings = (RegularDungeonRoomSettings)settings;
			Point point = new Point(i, j);
			if (base.Processed)
			{
				point = InnerBounds.Center;
			}
			int num = 6 + unifiedRandom.Next(7);
			int num2 = 8;
			if (regularDungeonRoomSettings.OverrideInnerBoundsSize > 0)
			{
				num = regularDungeonRoomSettings.OverrideInnerBoundsSize;
			}
			if (regularDungeonRoomSettings.OverrideOuterBoundsSize > 0)
			{
				num2 = regularDungeonRoomSettings.OverrideOuterBoundsSize;
			}
			if (base.Processed)
			{
				num = _innerBoundsSize;
			}
			int totalBoundsSize = num + num2;
			InnerBounds.SetBounds(point.X, point.Y, point.X, point.Y);
			OuterBounds.SetBounds(point.X, point.Y, point.X, point.Y);
			GenerateDungeonSquareRoom(data, InnerBounds, OuterBounds, point, settings.StyleData, num, totalBoundsSize, generating, generating);
			_innerBoundsSize = num;
			InnerBounds.CalculateHitbox();
			OuterBounds.CalculateHitbox();
		}
	}
}
