using System;
using Microsoft.Xna.Framework;
using Newtonsoft.Json;
using ReLogic.Utilities;
using Terraria.GameContent.Generation.Dungeon;

namespace Terraria.GameContent.Biomes
{
	public class DungeonControlLine
	{
		public int Index;

		public DungeonControlLine Next;

		public DungeonControlLine Prev;

		public Vector2D Start;

		public Vector2D End;

		public Vector2D StartTangent;

		public Vector2D EndTangent;

		public Vector2D StartNormal;

		public Vector2D EndNormal;

		public double CrossTangent;

		public double StartRadius;

		public double EndRadius;

		public static double NormalizedDistanceSafeFromDither;

		private const double StyleTransitionDitherWidth = 0.5;

		private const int BorderWidth = 4;

		public Vector2D NormalizedLineDirection;

		public double LineLength;

		public DungeonGenerationStyleData Style;

		public int ProgressionStage;

		public bool CurveLine;

		public Vector2D Center => (End + Start) / 2.0;

		[JsonConstructor]
		private DungeonControlLine()
		{
		}

		public DungeonControlLine(Vector2D start, Vector2D end, double startRadius, double endRadius, int progressionStage, DungeonGenerationStyleData style)
		{
			Start = start;
			End = end;
			StartRadius = startRadius;
			EndRadius = endRadius;
			ProgressionStage = progressionStage;
			Style = style;
			Vector2D v = End - Start;
			LineLength = v.Length();
			NormalizedLineDirection = v.SafeNormalize(Vector2D.UnitX);
		}

		private void CacheNormals()
		{
			StartNormal = new Vector2D(StartTangent.Y, 0.0 - StartTangent.X);
			EndNormal = new Vector2D(0.0 - EndTangent.Y, EndTangent.X);
			CrossTangent = Vector2D.Cross(StartTangent, EndTangent);
		}

		public bool CanPaint(int x, int y, out double distance, out double normalizedLineProgress)
		{
			distance = 0.0;
			normalizedLineProgress = 0.0;
			Vector2D vector2D = new Vector2D(x, y);
			Vector2D value = vector2D - Start;
			double num = Vector2D.Dot(value, StartTangent);
			if (num < 0.0)
			{
				if (Prev != null)
				{
					return false;
				}
				normalizedLineProgress = 0.0;
				distance = value.Length();
				return true;
			}
			Vector2D value2 = vector2D - End;
			double num2 = Vector2D.Dot(value2, EndTangent);
			if (num2 < 0.0)
			{
				if (Next != null)
				{
					return false;
				}
				normalizedLineProgress = 1.0;
				distance = value2.Length();
				return true;
			}
			double num3 = Vector2D.Dot(value, StartNormal);
			double num4 = Vector2D.Dot(value2, EndNormal);
			double num5 = (num + num2) / 2.0;
			num *= num;
			num2 *= num2;
			double num6 = num / (num + num2);
			double value3 = num3 * (1.0 - num6) + num4 * num6 - num5 * CrossTangent * num6 * (1.0 - num6);
			distance = Math.Abs(value3);
			normalizedLineProgress = num6;
			return true;
		}

		public void Paint(int x, int y)
		{
			if (!CanPaint(x, y, out var distance, out var normalizedLineProgress))
			{
				return;
			}
			double num = Utils.Lerp(StartRadius, EndRadius, normalizedLineProgress);
			double num2 = distance / num;
			if (num2 > 1.0)
			{
				return;
			}
			DungeonGenerationStyleData styleWithDitheredTransition = GetStyleWithDitheredTransition(x, y, normalizedLineProgress);
			if (SkipPaintForEdge(x, y, styleWithDitheredTransition, num2))
			{
				return;
			}
			Tile tile = Main.tile[x, y];
			tile.ClearEverything();
			tile.active(active: true);
			tile.type = styleWithDitheredTransition.BrickTileType;
			tile.wall = styleWithDitheredTransition.BrickWallType;
			if (styleWithDitheredTransition.UnbreakableWallProgressionTier <= DualDungeonUnbreakableWallTiers.EarlyGame)
			{
				return;
			}
			int num3 = (int)(num * NormalizedDistanceSafeFromDither);
			double num4 = distance - (double)num3;
			if (num4 >= -4.0 && num4 <= 0.0)
			{
				int num5 = styleWithDitheredTransition.UnbreakableWallProgressionTier;
				if (num4 <= -2.0)
				{
					num5 += 16;
				}
				tile.wall = 350;
				tile.wallColor((byte)num5);
			}
		}

		public DungeonGenerationStyleData GetStyleWithDitheredTransition(int x, int y, double normalizedLineProgress)
		{
			if (normalizedLineProgress < 0.25)
			{
				if (Prev != null && Prev.Style != Style && Utils.Remap(normalizedLineProgress, 0.0, 0.25, 0.5, 1.0) <= DitherSnakePass._bayerDither[x % 4, y % 4])
				{
					return Prev.Style;
				}
			}
			else if (normalizedLineProgress > 0.75 && Next != null && Next.Style != Style && Utils.Remap(normalizedLineProgress, 0.75, 1.0, 0.0, 0.5) >= DitherSnakePass._bayerDither[x % 4, y % 4])
			{
				return Next.Style;
			}
			return Style;
		}

		public static bool SkipPaintForEdge(int x, int y, DungeonGenerationStyleData style, double normalizedDistanceForPoint)
		{
			if (normalizedDistanceForPoint <= NormalizedDistanceSafeFromDither)
			{
				return false;
			}
			double num = 1.0 - (normalizedDistanceForPoint - NormalizedDistanceSafeFromDither) / (1.0 - NormalizedDistanceSafeFromDither);
			if (!style.EdgeDither)
			{
				return num < 0.25;
			}
			if (!WorldGen.InWorld(x, y, 5))
			{
				return false;
			}
			Tile tile = Main.tile[x, y];
			if (tile != null && !tile.active())
			{
				return true;
			}
			if (num <= DitherSnakePass._bayerDither[x % 4, y % 4])
			{
				return true;
			}
			double num2 = Utils.Lerp(0.0, 0.949999988079071, 1.0 - num);
			if ((double)WorldGen.genRand.NextFloat() <= num2)
			{
				return true;
			}
			if (num <= 3.0 / 32.0 && WorldGen.genRand.Next(3) != 0)
			{
				return true;
			}
			if (num <= 0.125 && WorldGen.genRand.Next(2) == 0)
			{
				return true;
			}
			if (num <= 5.0 / 32.0 && WorldGen.genRand.Next(4) == 0)
			{
				return true;
			}
			return false;
		}

		public void Paint(Rectangle dungeonBounds)
		{
			CacheNormals();
			double num = Utils.Max<double>(StartRadius, EndRadius);
			Point point = Start.ToPoint();
			Point point2 = End.ToPoint();
			Rectangle value = Rectangle.Union(new Rectangle(point.X, point.Y, 1, 1), new Rectangle(point2.X, point2.Y, 1, 1));
			value.Inflate((int)num, (int)num);
			Rectangle rectangle = Rectangle.Intersect(value, dungeonBounds);
			for (int i = rectangle.Left; i <= rectangle.Right; i++)
			{
				for (int j = rectangle.Top; j <= rectangle.Bottom; j++)
				{
					Paint(i, j);
				}
			}
		}

		public bool IsSelfIntersecting()
		{
			CacheNormals();
			double num = Vector2D.Cross(StartNormal, EndNormal);
			Vector2D value = End - Start;
			double value2 = Vector2D.Cross(value, EndNormal) / num;
			double value3 = Vector2D.Cross(value, StartNormal) / num;
			if (!(Math.Abs(value2) < StartRadius))
			{
				return Math.Abs(value3) < EndRadius;
			}
			return true;
		}

		public bool AdjustTangentsToPreventSelfIntersection()
		{
			if (!IsSelfIntersecting())
			{
				return false;
			}
			Vector2D startTangent = (StartTangent - EndTangent / 2.0).SafeNormalize(default(Vector2D));
			Vector2D endTangent = (EndTangent - StartTangent / 2.0).SafeNormalize(default(Vector2D));
			if (Prev != null)
			{
				StartTangent = startTangent;
				Prev.EndTangent = -StartTangent;
			}
			if (Next != null)
			{
				EndTangent = endTangent;
				Next.StartTangent = -EndTangent;
			}
			return true;
		}

		public bool IsInsideBorder(Point point)
		{
			if (CanPaint(point.X, point.Y, out var distance, out var normalizedLineProgress))
			{
				return distance < Utils.Lerp(StartRadius, EndRadius, normalizedLineProgress) * NormalizedDistanceSafeFromDither - 4.0;
			}
			return false;
		}

		public Vector2D GetPotentialRoomPosition(double normalizedDistanceAlong, double normalizedOffset, int roomRadius)
		{
			Vector2D vector2D = Vector2D.Lerp(StartNormal, EndNormal, normalizedDistanceAlong).SafeNormalize(default(Vector2D));
			double num = Utils.Lerp(StartRadius, EndRadius, normalizedDistanceAlong) * NormalizedDistanceSafeFromDither - 4.0 - (double)roomRadius;
			return Vector2D.Lerp(Start, End, normalizedDistanceAlong) + vector2D * num * normalizedOffset;
		}
	}
}
