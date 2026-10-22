using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Newtonsoft.Json;
using Terraria.GameContent.Generation;
using Terraria.GameContent.Generation.Dungeon;
using Terraria.ID;
using Terraria.WorldBuilding;

namespace Terraria.GameContent.Biomes
{
	public class EnchantedSwordBiome : MicroBiome
	{
		[JsonProperty("ChanceOfEntrance")]
		private double _chanceOfEntrance;

		[JsonProperty("ChanceOfRealSword")]
		private double _chanceOfRealSword;

		public override bool Place(Point origin, StructureMap structures, GenerationProgress progress)
		{
			Dictionary<ushort, int> dictionary = new Dictionary<ushort, int>();
			WorldUtils.Gen(new Point(origin.X - 25, origin.Y - 25), new Shapes.Rectangle(50, 50), new Actions.TileScanner(0, 1).Output(dictionary));
			int num = dictionary[0] + dictionary[1];
			if (WorldGen.SecretSeed.errorWorld.Enabled)
			{
				if (num < 625)
				{
					return false;
				}
			}
			else if (num < 1250)
			{
				return false;
			}
			int num2 = 55;
			if (WorldGen.SecretSeed.errorWorld.Enabled)
			{
				num2 = 105;
			}
			if (origin.Y <= num2)
			{
				return false;
			}
			int num3 = origin.Y - num2;
			int num4 = 50;
			if (num3 < num4)
			{
				num4 = num3;
			}
			if (!WorldUtils.Find(origin, Searches.Chain(new Searches.Up(num3), new Conditions.IsSolid().AreaOr(1, num4).Not()), out var result) || result.Y <= num2)
			{
				if (!WorldGen.SecretSeed.errorWorld.Enabled)
				{
					return false;
				}
				result.Y = origin.Y - 100;
			}
			if (WorldUtils.Find(origin, Searches.Chain(new Searches.Up(origin.Y - result.Y), new Conditions.IsTile(53)), out var _) && !WorldGen.SecretSeed.errorWorld.Enabled)
			{
				return false;
			}
			result.Y += 50;
			ShapeData shapeData = new ShapeData();
			ShapeData shapeData2 = new ShapeData();
			Point point = new Point(origin.X, origin.Y + 20);
			Point point2 = new Point(origin.X, origin.Y + 30);
			bool[] array = new bool[TileID.Sets.GeneralPlacementTiles.Length];
			for (int i = 0; i < array.Length; i++)
			{
				array[i] = TileID.Sets.GeneralPlacementTiles[i];
			}
			array[21] = false;
			array[467] = false;
			double num5 = 0.8 + GenBase._random.NextDouble() * 0.5;
			Rectangle rectangle = new Rectangle(point.X - (int)(20.0 * num5), point.Y - 20, (int)(40.0 * num5), 40);
			if (!structures.CanPlace(rectangle, array))
			{
				return false;
			}
			Rectangle rectangle2 = new Rectangle(origin.X, result.Y + 10, 1, origin.Y - result.Y - 9);
			if (!structures.CanPlace(rectangle2, array, 2))
			{
				return false;
			}
			if (WorldGen.SecretSeed.dualDungeons.Enabled && (DungeonUtils.IntersectsAnyPotentialDungeonBounds(rectangle) || DungeonUtils.IntersectsAnyPotentialDungeonBounds(rectangle2)))
			{
				return false;
			}
			WorldUtils.Gen(point, new Shapes.Slime(20, num5, 1.0), Actions.Chain(new Modifiers.Blotches(2, 0.4), new Actions.ClearTile(frameNeighbors: true).Output(shapeData)));
			WorldUtils.Gen(point2, new Shapes.Mound(14, 14), Actions.Chain(new Modifiers.Blotches(2, 1, 0.8), new Actions.SetTile(0), new Actions.SetFrames(frameNeighbors: true).Output(shapeData2)));
			shapeData.Subtract(shapeData2, point, point2);
			WorldUtils.Gen(point, new ModShapes.InnerOutline(shapeData), Actions.Chain(new Actions.SetTile(2), new Actions.SetFrames(frameNeighbors: true)));
			WorldUtils.Gen(point, new ModShapes.All(shapeData), Actions.Chain(new Modifiers.RectangleMask(-40, 40, 0, 40), new Modifiers.IsEmpty(), new Actions.SetLiquid()));
			WorldUtils.Gen(point, new ModShapes.All(shapeData), Actions.Chain(new Actions.PlaceWall(68), new Modifiers.OnlyTiles(2), new Modifiers.Offset(0, 1), new ActionVines(3, 5, 382)));
			if (GenBase._random.NextDouble() <= _chanceOfEntrance || WorldGen.tenthAnniversaryWorldGen)
			{
				ShapeData data = new ShapeData();
				WorldUtils.Gen(new Point(origin.X, result.Y + 10), new Shapes.Rectangle(1, origin.Y - result.Y - 9), Actions.Chain(new Modifiers.Blotches(2, 0.2), new Modifiers.SkipTiles(191, 192), new Actions.ClearTile().Output(data), new Modifiers.Expand(1), new Modifiers.OnlyTiles(53), new Actions.SetTile(397).Output(data)));
				WorldUtils.Gen(new Point(origin.X, result.Y + 10), new ModShapes.All(data), new Actions.SetFrames(frameNeighbors: true));
			}
			if (GenBase._random.NextDouble() <= _chanceOfRealSword)
			{
				WorldGen.PlaceTile(point2.X, point2.Y - 15, 187, mute: true, forced: false, -1, 17);
			}
			else
			{
				WorldGen.PlaceTile(point2.X, point2.Y - 15, 186, mute: true, forced: false, -1, 15);
			}
			WorldUtils.Gen(point2, new ModShapes.All(shapeData2), Actions.Chain(new Modifiers.Offset(0, -1), new Modifiers.OnlyTiles(2), new Modifiers.Offset(0, -1), new ActionGrass()));
			structures.AddProtectedStructure(new Rectangle(point.X - (int)(20.0 * num5), point.Y - 20, (int)(40.0 * num5), 40), 10);
			return true;
		}
	}
}
