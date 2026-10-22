using Microsoft.Xna.Framework;
using Terraria.WorldBuilding;

namespace Terraria.GameContent.Biomes
{
	public class HoneyPatchBiome : MicroBiome
	{
		public override bool Place(Point origin, StructureMap structures, GenerationProgress progress)
		{
			if (GenBase._tiles[origin.X, origin.Y].active() && WorldGen.SolidTile(origin.X, origin.Y))
			{
				return false;
			}
			if (!WorldUtils.Find(origin, Searches.Chain(new Searches.Down(80), new Conditions.IsSolid()), out var result))
			{
				return false;
			}
			result.Y += 2;
			Ref<int> obj = new Ref<int>(0);
			Ref<int> obj2 = new Ref<int>(0);
			Ref<int> obj3 = new Ref<int>(0);
			WorldUtils.Gen(result, new Shapes.Circle(15), Actions.Chain(new Modifiers.IsSolid(), new Actions.Scanner(obj), new Modifiers.OnlyTiles(60, 59), new Actions.Scanner(obj2), new Modifiers.OnlyTiles(60), new Actions.Scanner(obj3)));
			if ((double)obj2.Value / (double)obj.Value < 0.75 || obj3.Value < 2)
			{
				return false;
			}
			obj = new Ref<int>(0);
			WorldUtils.Gen(result, new Shapes.Circle(8), Actions.Chain(new Modifiers.IsSolid(), new Actions.Scanner(obj)));
			if (obj.Value < 20)
			{
				return false;
			}
			if (!structures.CanPlace(new Rectangle(result.X - 8, result.Y - 8, 16, 16)))
			{
				return false;
			}
			if (TooCloseToImportantLocations(result))
			{
				return false;
			}
			WorldUtils.Gen(result, new Shapes.Circle(8), Actions.Chain(new Modifiers.RadialDither(0.0, 10.0), new Modifiers.IsSolid(), new Actions.SetTile(229, setSelfFrames: true)));
			ShapeData data = new ShapeData();
			WorldUtils.Gen(result, new Shapes.Circle(4, 3), Actions.Chain(new Modifiers.Blotches(), new Modifiers.IsSolid(), new Actions.ClearTile(frameNeighbors: true), new Modifiers.RectangleMask(-6, 6, 0, 3).Output(data), new Actions.SetLiquid(2)));
			WorldUtils.Gen(new Point(result.X, result.Y + 1), new ModShapes.InnerOutline(data), Actions.Chain(new Modifiers.IsEmpty(), new Modifiers.RectangleMask(-6, 6, 1, 3), new Actions.SetTile(59, setSelfFrames: true)));
			WorldUtils.Gen(new Point(result.X, result.Y), new ModShapes.All(data), Actions.Chain(new Modifiers.Expand(1), new Modifiers.IsBelowHeight(result.Y, inclusive: true), new Modifiers.IsNotSolid(), new Modifiers.NoLiquid(2), new Actions.SetTile(229, setSelfFrames: true)));
			structures.AddProtectedStructure(new Rectangle(result.X - 8, result.Y - 8, 16, 16));
			return true;
		}

		private static bool TooCloseToImportantLocations(Point origin)
		{
			int x = origin.X;
			int y = origin.Y;
			if (y >= Main.UnderworldLayer - 30)
			{
				return true;
			}
			int num = 150;
			for (int i = x - num; i < x + num; i += 10)
			{
				if (i <= 0 || i > Main.maxTilesX - 1)
				{
					continue;
				}
				for (int j = y - num; j < y + num; j += 10)
				{
					if (j > 0 && j <= Main.maxTilesY - 1)
					{
						if (Main.tile[i, j].active() && Main.tile[i, j].type == 226)
						{
							return true;
						}
						if (Main.tile[i, j].wall == 83 || Main.tile[i, j].wall == 3 || Main.tile[i, j].wall == 87)
						{
							return true;
						}
					}
				}
			}
			return false;
		}
	}
}
