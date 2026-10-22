using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using Microsoft.Xna.Framework;
using Terraria.GameContent.Generation;
using Terraria.Utilities;
using Terraria.WorldBuilding;

namespace Terraria.GameContent.Biomes.CaveHouse
{
	public class HouseBuilder
	{
		private const int VERTICAL_EXIT_WIDTH = 3;

		public static readonly HouseBuilder Invalid = new HouseBuilder();

		public readonly HouseType Type;

		public readonly bool IsValid;

		protected ushort[] SkipTilesDuringWallAging = new ushort[5] { 245, 246, 240, 241, 242 };

		public double ChestChance { get; set; }

		public ushort TileType { get; protected set; }

		public ushort WallType { get; protected set; }

		public ushort BeamType { get; protected set; }

		public byte BeamPaint { get; protected set; }

		public int PlatformStyle { get; protected set; }

		public int DoorStyle { get; protected set; }

		public int TableStyle { get; protected set; }

		public bool UsesTables2 { get; protected set; }

		public int WorkbenchStyle { get; protected set; }

		public int PianoStyle { get; protected set; }

		public int BookcaseStyle { get; protected set; }

		public int ChairStyle { get; protected set; }

		public int ChestStyle { get; protected set; }

		public bool UsesContainers2 { get; protected set; }

		public ReadOnlyCollection<Rectangle> Rooms { get; private set; }

		public Rectangle TopRoom => Rooms.First();

		public Rectangle BottomRoom => Rooms.Last();

		private UnifiedRandom _random => WorldGen.genRand;

		private Tile[,] _tiles => Main.tile;

		private HouseBuilder()
		{
			IsValid = false;
		}

		protected HouseBuilder(HouseType type, IEnumerable<Rectangle> rooms)
		{
			Type = type;
			IsValid = true;
			List<Rectangle> list = rooms.ToList();
			list.Sort((Rectangle lhs, Rectangle rhs) => lhs.Top.CompareTo(rhs.Top));
			Rooms = list.AsReadOnly();
		}

		protected virtual void AgeRoom(Rectangle room)
		{
		}

		public void PotentiallyConvertToRainbowMossBlock()
		{
			if (WorldGen.SecretSeed.rainbowStuff.Enabled && WorldGen.genRand.Next(2) == 0)
			{
				TileType = 692;
				WallType = 346;
				PlatformStyle = 43;
				DoorStyle = 44;
			}
		}

		public void PotentiallyConvertToRainbowBrick()
		{
			if (!Main.tenthAnniversaryWorld)
			{
				return;
			}
			if (Main.getGoodWorld)
			{
				if (WorldGen.genRand.Next(7) == 0)
				{
					TileType = 160;
					WallType = 44;
				}
			}
			else if (WorldGen.genRand.Next(2) == 0)
			{
				TileType = 160;
				WallType = 44;
			}
		}

		public void RainbowifyOnTenthAnniversaryWorlds()
		{
			if (!Main.tenthAnniversaryWorld || (TileType == 160 && WorldGen.genRand.Next(2) == 0))
			{
				return;
			}
			foreach (Rectangle room in Rooms)
			{
				WorldUtils.Gen(new Point(room.X, room.Y), new Shapes.Rectangle(room.Width, room.Height), new Actions.SetTileAndWallRainbowPaint());
			}
		}

		public void PotentiallyConvertToSeedHouse()
		{
			if (WorldGen.SecretSeed.errorWorld.Enabled)
			{
				PlatformStyle = WorldGen.genRand.Next(49);
				DoorStyle = WorldGen.genRand.Next(49);
				TableStyle = WorldGen.genRand.Next(35);
				WorkbenchStyle = WorldGen.genRand.Next(44);
				PianoStyle = WorldGen.genRand.Next(39);
				BookcaseStyle = WorldGen.genRand.Next(40);
				ChairStyle = WorldGen.genRand.Next(44);
				switch (WorldGen.genRand.Next(20))
				{
				default:
					TileType = 159;
					WallType = 43;
					break;
				case 1:
					TileType = 422;
					WallType = 225;
					break;
				case 2:
					TileType = 194;
					WallType = 75;
					break;
				case 3:
					TileType = 541;
					WallType = 318;
					PlatformStyle = 48;
					break;
				case 4:
					TileType = 137;
					WallType = 147;
					break;
				case 5:
					TileType = 48;
					WallType = 245;
					break;
				case 6:
					TileType = 370;
					WallType = 182;
					break;
				case 7:
					TileType = 140;
					WallType = 33;
					break;
				case 8:
					TileType = 347;
					WallType = 174;
					break;
				case 9:
					TileType = 508;
					WallType = 243;
					break;
				case 10:
					TileType = 507;
					WallType = 242;
					break;
				case 11:
					TileType = 546;
					WallType = 167;
					break;
				case 12:
					TileType = 329;
					WallType = 169;
					break;
				case 13:
					TileType = 326;
					WallType = 136;
					break;
				case 14:
					TileType = 327;
					WallType = 137;
					break;
				case 15:
					TileType = 345;
					WallType = 172;
					break;
				case 16:
					TileType = 708;
					WallType = 347;
					break;
				case 17:
					TileType = 501;
					WallType = 238;
					break;
				case 18:
					TileType = 272;
					WallType = 225;
					break;
				case 19:
					TileType = 421;
					WallType = 225;
					break;
				}
			}
			else
			{
				if (WorldGen.genRand.NextFloat() > 0.4f)
				{
					return;
				}
				bool num = Type == HouseType.Wood;
				bool flag = Type == HouseType.Desert;
				bool num2 = Type == HouseType.Jungle;
				bool flag2 = Type == HouseType.Ice;
				List<ushort> list = new List<ushort>();
				if (num2 && Main.notTheBeesWorld && Main.tenthAnniversaryWorld)
				{
					list.Add(562);
					list.Add(563);
					list.Add(229);
				}
				if ((num || flag2) && Main.drunkWorld && Main.tenthAnniversaryWorld)
				{
					if (flag2)
					{
						list.Add(197);
					}
					else
					{
						list.Add(193);
					}
				}
				if (flag2 && WorldGen.SecretSeed.worldIsFrozen.Enabled && WorldGen.genRand.Next(3) == 0)
				{
					list.Add(145);
					list.Add(146);
				}
				if (flag && Main.remixWorld && Main.getGoodWorld)
				{
					list.Add(188);
				}
				if (list.Count > 0)
				{
					ushort num3 = list[WorldGen.genRand.Next(list.Count)];
					switch (num3)
					{
					case 562:
						TileType = num3;
						WallType = 312;
						BeamType = 575;
						BeamPaint = 16;
						PlatformStyle = 44;
						DoorStyle = 45;
						TableStyle = 8;
						UsesTables2 = true;
						WorkbenchStyle = 40;
						PianoStyle = 39;
						BookcaseStyle = 40;
						ChairStyle = 44;
						ChestStyle = 11;
						UsesContainers2 = true;
						break;
					case 563:
						TileType = num3;
						WallType = 313;
						BeamType = 575;
						BeamPaint = 16;
						PlatformStyle = 44;
						DoorStyle = 45;
						TableStyle = 8;
						UsesTables2 = true;
						WorkbenchStyle = 40;
						PianoStyle = 39;
						BookcaseStyle = 40;
						ChairStyle = 44;
						ChestStyle = 11;
						UsesContainers2 = true;
						break;
					case 229:
						TileType = num3;
						WallType = 86;
						BeamType = 575;
						BeamPaint = 15;
						PlatformStyle = 24;
						DoorStyle = 22;
						TableStyle = 19;
						UsesTables2 = false;
						WorkbenchStyle = 19;
						PianoStyle = 9;
						BookcaseStyle = 9;
						ChairStyle = 22;
						ChestStyle = 29;
						UsesContainers2 = false;
						break;
					case 188:
						TileType = num3;
						WallType = 72;
						BeamType = 124;
						BeamPaint = 17;
						PlatformStyle = 25;
						DoorStyle = 4;
						TableStyle = 30;
						UsesTables2 = false;
						WorkbenchStyle = 5;
						PianoStyle = 17;
						BookcaseStyle = 6;
						ChairStyle = 6;
						ChestStyle = 42;
						UsesContainers2 = false;
						break;
					case 193:
						TileType = num3;
						WallType = 76;
						BeamType = 124;
						BeamPaint = 19;
						PlatformStyle = 20;
						DoorStyle = 31;
						TableStyle = 29;
						UsesTables2 = false;
						WorkbenchStyle = 8;
						PianoStyle = 24;
						BookcaseStyle = 26;
						ChairStyle = 31;
						ChestStyle = 34;
						UsesContainers2 = false;
						break;
					case 197:
						TileType = num3;
						WallType = 76;
						BeamType = 574;
						BeamPaint = 26;
						PlatformStyle = 20;
						DoorStyle = 31;
						TableStyle = 29;
						UsesTables2 = false;
						WorkbenchStyle = 8;
						PianoStyle = 24;
						BookcaseStyle = 26;
						ChairStyle = 31;
						ChestStyle = 34;
						UsesContainers2 = false;
						break;
					case 145:
						TileType = num3;
						WallType = 29;
						BeamType = 574;
						BeamPaint = 26;
						break;
					case 146:
						TileType = num3;
						WallType = 30;
						BeamType = 574;
						BeamPaint = 26;
						break;
					}
				}
			}
		}

		public void PaintSeedHouses()
		{
			if (TileType != 197 || !Main.drunkWorld || !Main.tenthAnniversaryWorld)
			{
				return;
			}
			foreach (Rectangle room in Rooms)
			{
				WorldUtils.Gen(new Point(room.X, room.Y), new Shapes.Rectangle(room.Width, room.Height), Actions.Chain(new Modifiers.OnlyTiles(19, 10, 11, 14, 18, 87, 101, 15, 21), new Actions.SetTilePaint(7)));
				WorldUtils.Gen(new Point(room.X, room.Y), new Shapes.Rectangle(room.Width, room.Height), Actions.Chain(new Modifiers.OnlyWalls(WallType), new Actions.SetWallPaint(7)));
			}
		}

		public virtual void Place(HouseBuilderContext context, StructureMap structures)
		{
			PlaceEmptyRooms();
			foreach (Rectangle room in Rooms)
			{
				structures.AddProtectedStructure(room, 8);
			}
			PlaceStairs();
			PlaceDoors();
			PlacePlatforms();
			PlaceSupportBeams();
			PlaceBiomeSpecificPriorityTool(context);
			FillRooms();
			foreach (Rectangle room2 in Rooms)
			{
				AgeRoom(room2);
			}
			PlaceChests();
			PlaceBiomeSpecificTool(context);
			PaintSeedHouses();
		}

		private void PlaceEmptyRooms()
		{
			foreach (Rectangle room in Rooms)
			{
				WorldUtils.Gen(new Point(room.X, room.Y), new Shapes.Rectangle(room.Width, room.Height), Actions.Chain(new Actions.SetTileKeepWall(TileType), new Actions.SetFrames(frameNeighbors: true)));
				WorldUtils.Gen(new Point(room.X + 1, room.Y + 1), new Shapes.Rectangle(room.Width - 2, room.Height - 2), Actions.Chain(new Actions.ClearTile(frameNeighbors: true), new Actions.PlaceWall(WallType)));
			}
		}

		private void FillRooms()
		{
			int x = 14;
			if (UsesTables2)
			{
				x = 469;
			}
			Point[] choices = new Point[7]
			{
				new Point(x, TableStyle),
				new Point(16, 0),
				new Point(18, WorkbenchStyle),
				new Point(86, 0),
				new Point(87, PianoStyle),
				new Point(94, 0),
				new Point(101, BookcaseStyle)
			};
			foreach (Rectangle room in Rooms)
			{
				int num = room.Width / 8;
				int num2 = room.Width / (num + 1);
				int num3 = _random.Next(2);
				for (int i = 0; i < num; i++)
				{
					int num4 = (i + 1) * num2 + room.X;
					switch (i + num3 % 2)
					{
					case 0:
					{
						int num5 = room.Y + Math.Min(room.Height / 2, room.Height - 5);
						PaintingEntry paintingEntry = ((Type == HouseType.Desert) ? WorldGen.RandHousePictureDesert() : WorldGen.RandHousePicture());
						WorldGen.PlaceTile(num4, num5, paintingEntry.tileType, mute: true, forced: false, -1, paintingEntry.style);
						break;
					}
					case 1:
					{
						int num5 = room.Y + 1;
						WorldGen.PlaceTile(num4, num5, 34, mute: true, forced: false, -1, _random.Next(6));
						for (int j = -1; j < 2; j++)
						{
							for (int k = 0; k < 3; k++)
							{
								_tiles[j + num4, k + num5].frameX += 54;
							}
						}
						break;
					}
					}
				}
				int num6 = room.Width / 8 + 3;
				WorldGen.SetupStatueList();
				while (num6 > 0)
				{
					int num7 = _random.Next(room.Width - 3) + 1 + room.X;
					int num8 = room.Y + room.Height - 2;
					switch (_random.Next(4))
					{
					case 0:
						WorldGen.PlaceSmallPile(num7, num8, _random.Next(31, 34), 1, 185);
						break;
					case 1:
						WorldGen.PlaceTile(num7, num8, 186, mute: true, forced: false, -1, _random.Next(22, 26));
						break;
					case 2:
					{
						int num9 = _random.Next(2, GenVars.statueList.Length);
						WorldGen.PlaceTile(num7, num8, GenVars.statueList[num9].X, mute: true, forced: false, -1, GenVars.statueList[num9].Y);
						if (GenVars.StatuesWithTraps.Contains(num9))
						{
							WorldGen.PlaceStatueTrap(num7, num8);
						}
						break;
					}
					case 3:
					{
						Point point = Utils.SelectRandom(_random, choices);
						WorldGen.PlaceTile(num7, num8, point.X, mute: true, forced: false, -1, point.Y);
						break;
					}
					}
					num6--;
				}
			}
		}

		private void PlaceStairs()
		{
			foreach (Tuple<Point, Point> item3 in CreateStairsList())
			{
				Point item = item3.Item1;
				Point item2 = item3.Item2;
				int num = ((item2.X > item.X) ? 1 : (-1));
				ShapeData shapeData = new ShapeData();
				for (int i = 0; i < item2.Y - item.Y; i++)
				{
					shapeData.Add(num * (i + 1), i);
				}
				WorldUtils.Gen(item, new ModShapes.All(shapeData), Actions.Chain(new Actions.PlaceTile(19, PlatformStyle), new Actions.SetSlope((num == 1) ? 1 : 2), new Actions.SetFrames(frameNeighbors: true)));
				WorldUtils.Gen(new Point(item.X + ((num == 1) ? 1 : (-4)), item.Y - 1), new Shapes.Rectangle(4, 1), Actions.Chain(new Actions.Clear(), new Actions.PlaceWall(WallType), new Actions.PlaceTile(19, PlatformStyle), new Actions.SetFrames(frameNeighbors: true)));
			}
		}

		private List<Tuple<Point, Point>> CreateStairsList()
		{
			List<Tuple<Point, Point>> list = new List<Tuple<Point, Point>>();
			for (int i = 1; i < Rooms.Count; i++)
			{
				Rectangle rectangle = Rooms[i];
				Rectangle rectangle2 = Rooms[i - 1];
				int num = rectangle2.X - rectangle.X;
				int num2 = rectangle.X + rectangle.Width - (rectangle2.X + rectangle2.Width);
				if (num > num2)
				{
					list.Add(new Tuple<Point, Point>(new Point(rectangle.X + rectangle.Width - 1, rectangle.Y + 1), new Point(rectangle.X + rectangle.Width - rectangle.Height + 1, rectangle.Y + rectangle.Height - 1)));
				}
				else
				{
					list.Add(new Tuple<Point, Point>(new Point(rectangle.X, rectangle.Y + 1), new Point(rectangle.X + rectangle.Height - 1, rectangle.Y + rectangle.Height - 1)));
				}
			}
			return list;
		}

		private void PlaceDoors()
		{
			foreach (Point item in CreateDoorList())
			{
				WorldUtils.Gen(item, new Shapes.Rectangle(1, 3), new Actions.ClearTile(frameNeighbors: true));
				WorldGen.PlaceTile(item.X, item.Y, 10, mute: true, forced: true, -1, DoorStyle);
			}
		}

		private List<Point> CreateDoorList()
		{
			List<Point> list = new List<Point>();
			foreach (Rectangle room in Rooms)
			{
				if (FindSideExit(new Rectangle(room.X + room.Width, room.Y + 1, 1, room.Height - 2), isLeft: false, out var exitY))
				{
					list.Add(new Point(room.X + room.Width - 1, exitY));
				}
				if (FindSideExit(new Rectangle(room.X, room.Y + 1, 1, room.Height - 2), isLeft: true, out exitY))
				{
					list.Add(new Point(room.X, exitY));
				}
			}
			return list;
		}

		private void PlacePlatforms()
		{
			foreach (Point item in CreatePlatformsList())
			{
				WorldUtils.Gen(item, new Shapes.Rectangle(3, 1), Actions.Chain(new Actions.ClearMetadata(), new Actions.PlaceTile(19, PlatformStyle), new Actions.SetFrames(frameNeighbors: true)));
			}
		}

		private List<Point> CreatePlatformsList()
		{
			List<Point> list = new List<Point>();
			Rectangle topRoom = TopRoom;
			Rectangle bottomRoom = BottomRoom;
			if (FindVerticalExit(new Rectangle(topRoom.X + 2, topRoom.Y, topRoom.Width - 4, 1), isUp: true, out var exitX))
			{
				list.Add(new Point(exitX, topRoom.Y));
			}
			if (FindVerticalExit(new Rectangle(bottomRoom.X + 2, bottomRoom.Y + bottomRoom.Height - 1, bottomRoom.Width - 4, 1), isUp: false, out exitX))
			{
				list.Add(new Point(exitX, bottomRoom.Y + bottomRoom.Height - 1));
			}
			return list;
		}

		private void PlaceSupportBeams()
		{
			foreach (Rectangle item in CreateSupportBeamList())
			{
				if (item.Height > 1 && _tiles[item.X, item.Y - 1].type != 19)
				{
					WorldUtils.Gen(new Point(item.X, item.Y), new Shapes.Rectangle(item.Width, item.Height), Actions.Chain(new Actions.SetTileKeepWall(BeamType), new Actions.SetFrames(frameNeighbors: true), new Actions.SetTilePaint(BeamPaint)));
					Tile tile = _tiles[item.X, item.Y + item.Height];
					tile.slope(0);
					tile.halfBrick(halfBrick: false);
				}
			}
		}

		private List<Rectangle> CreateSupportBeamList()
		{
			List<Rectangle> list = new List<Rectangle>();
			int num = Rooms.Min((Rectangle room) => room.Left);
			int num2 = Rooms.Max((Rectangle room) => room.Right) - 1;
			int num3 = 6;
			while (num3 > 4 && (num2 - num) % num3 != 0)
			{
				num3--;
			}
			for (int num4 = num; num4 <= num2; num4 += num3)
			{
				for (int num5 = 0; num5 < Rooms.Count; num5++)
				{
					Rectangle rectangle = Rooms[num5];
					if (num4 < rectangle.X || num4 >= rectangle.X + rectangle.Width)
					{
						continue;
					}
					int num6 = rectangle.Y + rectangle.Height;
					int num7 = 50;
					for (int num8 = num5 + 1; num8 < Rooms.Count; num8++)
					{
						if (num4 >= Rooms[num8].X && num4 < Rooms[num8].X + Rooms[num8].Width)
						{
							num7 = Math.Min(num7, Rooms[num8].Y - num6);
						}
					}
					if (num7 > 0)
					{
						Point result;
						bool flag = WorldUtils.Find(new Point(num4, num6), Searches.Chain(new Searches.Down(num7), new Conditions.IsSolid()), out result);
						if (num7 < 50 && !WorldGen.SecretSeed.GenerateBiggerAbandonedHouses)
						{
							flag = true;
							result = new Point(num4, num6 + num7);
						}
						if (flag)
						{
							list.Add(new Rectangle(num4, num6, 1, result.Y - num6));
						}
					}
				}
			}
			return list;
		}

		private static bool FindVerticalExit(Rectangle wall, bool isUp, out int exitX)
		{
			Point result2;
			bool result = WorldUtils.Find(new Point(wall.X + wall.Width - 3, wall.Y + (isUp ? (-5) : 0)), Searches.Chain(new Searches.Left(wall.Width - 3), new Conditions.IsSolid().Not().AreaOr(3, 5)), out result2);
			exitX = result2.X;
			return result;
		}

		private static bool FindSideExit(Rectangle wall, bool isLeft, out int exitY)
		{
			Point result2;
			bool result = WorldUtils.Find(new Point(wall.X + (isLeft ? (-4) : 0), wall.Y + wall.Height - 3), Searches.Chain(new Searches.Up(wall.Height - 3), new Conditions.IsSolid().Not().AreaOr(4, 3)), out result2);
			exitY = result2.Y;
			return result;
		}

		private void PlaceChests()
		{
			if (_random.NextDouble() > ChestChance)
			{
				return;
			}
			bool flag = false;
			foreach (Rectangle room in Rooms)
			{
				int num = room.Height - 1 + room.Y;
				bool num2 = num > (int)Main.worldSurface;
				ushort chestTileType = (ushort)((num2 && UsesContainers2) ? 467 : 21);
				int chestStyle = (num2 ? ChestStyle : 0);
				for (int i = 0; i < 10; i++)
				{
					if (flag = WorldGen.AddBuriedChest(_random.Next(2, room.Width - 2) + room.X, num, 0, notNearOtherChests: false, chestStyle, trySlope: false, chestTileType))
					{
						break;
					}
				}
				if (flag)
				{
					break;
				}
				for (int j = room.X + 2; j <= room.X + room.Width - 2; j++)
				{
					if (flag = WorldGen.AddBuriedChest(j, num, 0, notNearOtherChests: false, chestStyle, trySlope: false, chestTileType))
					{
						break;
					}
				}
				if (flag)
				{
					break;
				}
			}
			if (!flag)
			{
				foreach (Rectangle room2 in Rooms)
				{
					int num3 = room2.Y - 1;
					bool num4 = num3 > (int)Main.worldSurface;
					ushort chestTileType2 = (ushort)((num4 && UsesContainers2) ? 467 : 21);
					int chestStyle2 = (num4 ? ChestStyle : 0);
					for (int k = 0; k < 10; k++)
					{
						if (flag = WorldGen.AddBuriedChest(_random.Next(2, room2.Width - 2) + room2.X, num3, 0, notNearOtherChests: false, chestStyle2, trySlope: false, chestTileType2))
						{
							break;
						}
					}
					if (flag)
					{
						break;
					}
					for (int l = room2.X + 2; l <= room2.X + room2.Width - 2; l++)
					{
						if (flag = WorldGen.AddBuriedChest(l, num3, 0, notNearOtherChests: false, chestStyle2, trySlope: false, chestTileType2))
						{
							break;
						}
					}
					if (flag)
					{
						break;
					}
				}
			}
			if (flag)
			{
				return;
			}
			for (int m = 0; m < 1000; m++)
			{
				int i2 = _random.Next(Rooms[0].X - 30, Rooms[0].X + 30);
				int num5 = _random.Next(Rooms[0].Y - 30, Rooms[0].Y + 30);
				bool num6 = num5 > (int)Main.worldSurface;
				ushort chestTileType3 = (ushort)((num6 && UsesContainers2) ? 467 : 21);
				int chestStyle3 = (num6 ? ChestStyle : 0);
				if (flag = WorldGen.AddBuriedChest(i2, num5, 0, notNearOtherChests: false, chestStyle3, trySlope: false, chestTileType3))
				{
					break;
				}
			}
		}

		private void PlaceBiomeSpecificPriorityTool(HouseBuilderContext context)
		{
			if (Type != HouseType.Desert || GenVars.extraBastStatueCount >= GenVars.extraBastStatueCountMax)
			{
				return;
			}
			bool flag = false;
			foreach (Rectangle room in Rooms)
			{
				int num = room.Height - 2 + room.Y;
				if (WorldGen.remixWorldGen && (double)num > Main.rockLayer)
				{
					return;
				}
				for (int i = 0; i < 10; i++)
				{
					int num2 = _random.Next(2, room.Width - 2) + room.X;
					WorldGen.PlaceTile(num2, num, 506, mute: true, forced: true);
					if (flag = _tiles[num2, num].active() && _tiles[num2, num].type == 506)
					{
						break;
					}
				}
				if (flag)
				{
					break;
				}
				for (int j = room.X + 2; j <= room.X + room.Width - 2; j++)
				{
					if (flag = WorldGen.PlaceTile(j, num, 506, mute: true, forced: true))
					{
						break;
					}
				}
				if (flag)
				{
					break;
				}
			}
			if (!flag)
			{
				foreach (Rectangle room2 in Rooms)
				{
					int num3 = room2.Y - 1;
					for (int k = 0; k < 10; k++)
					{
						int num4 = _random.Next(2, room2.Width - 2) + room2.X;
						WorldGen.PlaceTile(num4, num3, 506, mute: true, forced: true);
						if (flag = _tiles[num4, num3].active() && _tiles[num4, num3].type == 506)
						{
							break;
						}
					}
					if (flag)
					{
						break;
					}
					for (int l = room2.X + 2; l <= room2.X + room2.Width - 2; l++)
					{
						if (flag = WorldGen.PlaceTile(l, num3, 506, mute: true, forced: true))
						{
							break;
						}
					}
					if (flag)
					{
						break;
					}
				}
			}
			if (flag)
			{
				GenVars.extraBastStatueCount++;
			}
		}

		private void PlaceBiomeSpecificTool(HouseBuilderContext context)
		{
			if (Type == HouseType.Jungle && context.SharpenerCount < _random.Next(2, 5))
			{
				bool flag = false;
				foreach (Rectangle room in Rooms)
				{
					int num = room.Height - 2 + room.Y;
					for (int i = 0; i < 10; i++)
					{
						int num2 = _random.Next(2, room.Width - 2) + room.X;
						WorldGen.PlaceTile(num2, num, 377, mute: true, forced: true);
						if (flag = _tiles[num2, num].active() && _tiles[num2, num].type == 377)
						{
							break;
						}
					}
					if (flag)
					{
						break;
					}
					for (int j = room.X + 2; j <= room.X + room.Width - 2; j++)
					{
						if (flag = WorldGen.PlaceTile(j, num, 377, mute: true, forced: true))
						{
							break;
						}
					}
					if (flag)
					{
						break;
					}
				}
				if (flag)
				{
					context.SharpenerCount++;
				}
			}
			if (Type != HouseType.Desert || context.ExtractinatorCount >= _random.Next(2, 5))
			{
				return;
			}
			ushort num3 = 219;
			if (WorldGen.SecretSeed.errorWorld.Enabled)
			{
				num3 = 642;
			}
			bool flag2 = false;
			foreach (Rectangle room2 in Rooms)
			{
				int num4 = room2.Height - 2 + room2.Y;
				for (int k = 0; k < 10; k++)
				{
					int num5 = _random.Next(2, room2.Width - 2) + room2.X;
					WorldGen.PlaceTile(num5, num4, num3, mute: true, forced: true);
					if (flag2 = _tiles[num5, num4].active() && _tiles[num5, num4].type == num3)
					{
						break;
					}
				}
				if (flag2)
				{
					break;
				}
				for (int l = room2.X + 2; l <= room2.X + room2.Width - 2; l++)
				{
					if (flag2 = WorldGen.PlaceTile(l, num4, num3, mute: true, forced: true))
					{
						break;
					}
				}
				if (flag2)
				{
					break;
				}
			}
			if (flag2)
			{
				context.ExtractinatorCount++;
			}
		}
	}
}
