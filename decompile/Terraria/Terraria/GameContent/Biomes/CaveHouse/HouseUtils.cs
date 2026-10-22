using System;
using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Terraria.ID;
using Terraria.WorldBuilding;

namespace Terraria.GameContent.Biomes.CaveHouse
{
	public static class HouseUtils
	{
		private static readonly bool[] BlacklistedTiles = TileID.Sets.Factory.CreateBoolSet(true, 225, 41, 43, 44, 226, 203, 112, 25, 151, 21, 467);

		private static readonly bool[] BeelistedTiles = TileID.Sets.Factory.CreateBoolSet(true, 41, 43, 44, 226, 203, 112, 25, 151, 21, 467);

		public static HouseBuilder CreateBuilder(Point origin, StructureMap structures)
		{
			List<Rectangle> list = CreateRooms(origin);
			if (list.Count == 0 || !AreRoomLocationsValid(list))
			{
				return HouseBuilder.Invalid;
			}
			HouseType houseType = GetHouseType(list);
			if (!AreRoomsValid(list, structures, houseType))
			{
				return HouseBuilder.Invalid;
			}
			switch (houseType)
			{
			case HouseType.Wood:
				return new WoodHouseBuilder(list);
			case HouseType.Desert:
				return new DesertHouseBuilder(list);
			case HouseType.Granite:
				return new GraniteHouseBuilder(list);
			case HouseType.Ice:
				return new IceHouseBuilder(list);
			case HouseType.Jungle:
				return new JungleHouseBuilder(list);
			case HouseType.Marble:
				return new MarbleHouseBuilder(list);
			case HouseType.Mushroom:
				return new MushroomHouseBuilder(list);
			default:
				return new WoodHouseBuilder(list);
			}
		}

		public static int GetMaxPossibleRoomsInABigAbandonedHouse()
		{
			if (WorldGen.SecretSeed.errorWorld.Enabled)
			{
				return 30;
			}
			return 7;
		}

		public static int GetRandomizedRoomCountInABigAbandonedHouse()
		{
			int num = 7;
			if (WorldGen.SecretSeed.errorWorld.Enabled)
			{
				num = WorldGen.genRand.Next(7, 31);
			}
			return Math.Max(0, num - WorldGen.genRand.Next(4));
		}

		private static List<Rectangle> CreateRooms_BigAbandonedHouses(Point origin)
		{
			List<Rectangle> list = new List<Rectangle>();
			if (!WorldUtils.Find(origin, Searches.Chain(new Searches.Down(200), new Conditions.IsSolid()), out var result) || result == origin)
			{
				return new List<Rectangle>();
			}
			Rectangle lastRoom = FindRoom(result);
			int randomizedRoomCountInABigAbandonedHouse = GetRandomizedRoomCountInABigAbandonedHouse();
			if (randomizedRoomCountInABigAbandonedHouse == 0)
			{
				return list;
			}
			for (int i = 0; i < randomizedRoomCountInABigAbandonedHouse; i++)
			{
				Rectangle rectangle = FindRoom_BigAbandonedHouses(new Point(lastRoom.Center.X, lastRoom.Y + 1), lastRoom);
				list.Add(rectangle);
				lastRoom = rectangle;
			}
			for (int j = 0; j < list.Count; j++)
			{
				Rectangle rectangle2 = list[j];
				rectangle2.Y += 3;
			}
			return list;
		}

		private static List<Rectangle> CreateRooms(Point origin)
		{
			if (WorldGen.SecretSeed.GenerateBiggerAbandonedHouses)
			{
				return CreateRooms_BigAbandonedHouses(origin);
			}
			if (!WorldUtils.Find(origin, Searches.Chain(new Searches.Down(200), new Conditions.IsSolid()), out var result) || result == origin)
			{
				return new List<Rectangle>();
			}
			Rectangle item = FindRoom(result);
			Rectangle rectangle = FindRoom(new Point(item.Center.X, item.Y + 1));
			Rectangle rectangle2 = FindRoom(new Point(item.Center.X, item.Y + item.Height + 10));
			rectangle2.Y = item.Y + item.Height - 1;
			double roomSolidPrecentage = GetRoomSolidPrecentage(rectangle);
			double roomSolidPrecentage2 = GetRoomSolidPrecentage(rectangle2);
			item.Y += 3;
			rectangle.Y += 3;
			rectangle2.Y += 3;
			List<Rectangle> list = new List<Rectangle>();
			if (WorldGen.genRand.NextDouble() > roomSolidPrecentage + 0.2)
			{
				list.Add(rectangle);
			}
			list.Add(item);
			if (WorldGen.genRand.NextDouble() > roomSolidPrecentage2 + 0.2)
			{
				list.Add(rectangle2);
			}
			return list;
		}

		private static Rectangle FindRoom(Point origin)
		{
			Point result;
			bool flag = WorldUtils.Find(origin, Searches.Chain(new Searches.Left(25), new Conditions.IsSolid()), out result);
			Point result2;
			bool num = WorldUtils.Find(origin, Searches.Chain(new Searches.Right(25), new Conditions.IsSolid()), out result2);
			if (!flag)
			{
				result = new Point(origin.X - 25, origin.Y);
			}
			if (!num)
			{
				result2 = new Point(origin.X + 25, origin.Y);
			}
			Rectangle result3 = new Rectangle(origin.X, origin.Y, 0, 0);
			if (origin.X - result.X > result2.X - origin.X)
			{
				result3.X = result.X;
				result3.Width = Utils.Clamp(result2.X - result.X, 15, 30);
			}
			else
			{
				result3.Width = Utils.Clamp(result2.X - result.X, 15, 30);
				result3.X = result2.X - result3.Width;
			}
			Point result4;
			bool flag2 = WorldUtils.Find(result, Searches.Chain(new Searches.Up(10), new Conditions.IsSolid()), out result4);
			Point result5;
			bool num2 = WorldUtils.Find(result2, Searches.Chain(new Searches.Up(10), new Conditions.IsSolid()), out result5);
			if (!flag2)
			{
				result4 = new Point(origin.X, origin.Y - 10);
			}
			if (!num2)
			{
				result5 = new Point(origin.X, origin.Y - 10);
			}
			result3.Height = Utils.Clamp(Math.Max(origin.Y - result4.Y, origin.Y - result5.Y), 8, 12);
			result3.Y -= result3.Height;
			return result3;
		}

		private static Rectangle FindRoom_BigAbandonedHouses(Point origin, Rectangle lastRoom)
		{
			int minValue = 15;
			int num = 30;
			int minValue2 = 8;
			int num2 = 12;
			Point result;
			bool flag = WorldUtils.Find(origin, Searches.Chain(new Searches.Left(25), new Conditions.IsSolid()), out result);
			Point result2;
			bool num3 = WorldUtils.Find(origin, Searches.Chain(new Searches.Right(25), new Conditions.IsSolid()), out result2);
			if (!flag)
			{
				result = new Point(origin.X - 25, origin.Y);
			}
			if (!num3)
			{
				result2 = new Point(origin.X + 25, origin.Y);
			}
			Rectangle result3 = new Rectangle(origin.X, origin.Y, 0, 0);
			if (WorldGen.genRand.Next(2) == 0)
			{
				if (result.X < lastRoom.Left)
				{
					result.X = lastRoom.Left;
				}
				result3.X = result.X;
				result3.Width = WorldGen.genRand.Next(minValue, num + 1);
				if (result3.Left <= 10)
				{
					result3.X = 10;
				}
				if (result3.Right >= Main.maxTilesX - 10)
				{
					result3.X = Main.maxTilesX - 10 - result3.Width;
				}
			}
			else
			{
				if (result2.X > lastRoom.Right)
				{
					result2.X = lastRoom.Right;
				}
				result3.Width = WorldGen.genRand.Next(minValue, num + 1);
				result3.X = result2.X - result3.Width;
				if (result3.Left <= 10)
				{
					result3.X = 10;
				}
				if (result3.Right >= Main.maxTilesX - 10)
				{
					result3.X = Main.maxTilesX - 10 - result3.Width;
				}
			}
			result3.Height = WorldGen.genRand.Next(minValue2, num2 + 1);
			result3.Y -= result3.Height;
			return result3;
		}

		private static double GetRoomSolidPrecentage(Rectangle room)
		{
			double num = room.Width * room.Height;
			Ref<int> obj = new Ref<int>(0);
			WorldUtils.Gen(new Point(room.X, room.Y), new Shapes.Rectangle(room.Width, room.Height), Actions.Chain(new Modifiers.IsSolid(), new Actions.Count(obj)));
			return (double)obj.Value / num;
		}

		private static bool AreRoomLocationsValid(IEnumerable<Rectangle> rooms)
		{
			foreach (Rectangle room in rooms)
			{
				if (!WorldGen.InWorld(room, 10))
				{
					return false;
				}
				if (room.Y + room.Height > Main.maxTilesY - 220)
				{
					return false;
				}
			}
			return true;
		}

		private static HouseType GetHouseType(IEnumerable<Rectangle> rooms)
		{
			Dictionary<ushort, int> dictionary = new Dictionary<ushort, int>();
			foreach (Rectangle room in rooms)
			{
				WorldUtils.Gen(new Point(room.X - 10, room.Y - 10), new Shapes.Rectangle(room.Width + 20, room.Height + 20), new Actions.TileScanner(0, 59, 147, 1, 161, 53, 396, 397, 368, 367, 60, 70).Output(dictionary));
			}
			List<Tuple<HouseType, int>> list = new List<Tuple<HouseType, int>>
			{
				Tuple.Create(HouseType.Wood, dictionary[0] + dictionary[1]),
				Tuple.Create(HouseType.Jungle, dictionary[59] + dictionary[60] * 10),
				Tuple.Create(HouseType.Mushroom, dictionary[59] + dictionary[70] * 10),
				Tuple.Create(HouseType.Ice, dictionary[147] + dictionary[161]),
				Tuple.Create(HouseType.Desert, dictionary[397] + dictionary[396] + dictionary[53]),
				Tuple.Create(HouseType.Granite, dictionary[368]),
				Tuple.Create(HouseType.Marble, dictionary[367])
			};
			Tuple<HouseType, int> tuple = list[0];
			for (int i = 1; i < list.Count; i++)
			{
				if (tuple.Item2 < list[i].Item2)
				{
					tuple = list[i];
				}
			}
			return tuple.Item1;
		}

		private static bool AreRoomsValid(IEnumerable<Rectangle> rooms, StructureMap structures, HouseType style)
		{
			foreach (Rectangle room in rooms)
			{
				if (style != HouseType.Granite && WorldUtils.Find(new Point(room.X - 2, room.Y - 2), Searches.Chain(new Searches.Rectangle(room.Width + 4, room.Height + 4).RequireAll(mode: false), new Conditions.HasLava()), out var _))
				{
					return false;
				}
				if (WorldGen.notTheBees)
				{
					if (!structures.CanPlace(room, BeelistedTiles, 5))
					{
						return false;
					}
				}
				else if (!structures.CanPlace(room, BlacklistedTiles, 5))
				{
					return false;
				}
			}
			return true;
		}
	}
}
