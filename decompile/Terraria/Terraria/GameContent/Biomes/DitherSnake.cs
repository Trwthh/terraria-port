using System;
using System.Collections.Generic;
using System.Linq;
using Microsoft.Xna.Framework;
using ReLogic.Utilities;
using Terraria.GameContent.Generation.Dungeon;

namespace Terraria.GameContent.Biomes
{
	public class DitherSnake : List<DungeonControlLine>
	{
		private static readonly Vector2D[] CircleTestPoints = (from i in Enumerable.Range(0, 12)
			select Vector2D.UnitX.RotatedBy(Math.PI * 2.0 * (double)i / 12.0)).ToArray();

		private static readonly double ExtraBuffer = 1.0 / Math.Cos(Math.PI / 6.0);

		public new void Add(DungeonControlLine line)
		{
			if (base.Count > 0)
			{
				DungeonControlLine dungeonControlLine = this.Last();
				dungeonControlLine.Next = line;
				line.Prev = dungeonControlLine;
			}
			line.Index = base.Count;
			base.Add(line);
		}

		public DungeonControlLine GetClosestLineTo(Vector2D pos)
		{
			DungeonControlLine result = null;
			double num = double.MaxValue;
			using (Enumerator enumerator = GetEnumerator())
			{
				while (enumerator.MoveNext())
				{
					DungeonControlLine current = enumerator.Current;
					double num2 = current.Center.Distance(pos);
					if (num2 < num)
					{
						result = current;
						num = num2;
					}
				}
				return result;
			}
		}

		public DungeonControlLine GetLineContaining(Vector2D pos, DungeonControlLine initialGuess = null, int depth = 0)
		{
			if (initialGuess == null)
			{
				initialGuess = GetClosestLineTo(pos);
			}
			if (depth == 3)
			{
				return null;
			}
			if (Vector2D.Dot(pos - initialGuess.Start, initialGuess.StartTangent) < 0.0 && initialGuess.Prev != null)
			{
				return GetLineContaining(pos, initialGuess.Prev, depth + 1);
			}
			if (Vector2D.Dot(pos - initialGuess.End, initialGuess.EndTangent) < 0.0 && initialGuess.Next != null)
			{
				return GetLineContaining(pos, initialGuess.Next, depth + 1);
			}
			return initialGuess;
		}

		public double GetPositionAlongSnake(Vector2D pos)
		{
			DungeonControlLine lineContaining = GetLineContaining(pos);
			if (!lineContaining.CanPaint((int)pos.X, (int)pos.Y, out var _, out var normalizedLineProgress))
			{
				normalizedLineProgress = 0.5;
			}
			return (double)lineContaining.Index + normalizedLineProgress;
		}

		public bool IsCircleInsideBorderWithStyle(DungeonGenerationStyleData style, Vector2D center, int radius)
		{
			DungeonControlLine closestLineTo = GetClosestLineTo(center);
			if (closestLineTo.Style == style)
			{
				return IsCircleInsideBorderWithMatchingStyle(closestLineTo, center, radius);
			}
			return false;
		}

		public bool IsCircleInsideBorderWithMatchingStyle(DungeonControlLine nearbyLine, Vector2D center, int radius)
		{
			double num = (double)radius * ExtraBuffer;
			Vector2D[] circleTestPoints = CircleTestPoints;
			foreach (Vector2D vector2D in circleTestPoints)
			{
				Vector2D vector2D2 = center + vector2D * num;
				DungeonControlLine lineContaining = GetLineContaining(vector2D2, nearbyLine);
				if (lineContaining == null || lineContaining.Style != nearbyLine.Style)
				{
					return false;
				}
				if (!lineContaining.IsInsideBorder(vector2D2.ToPoint()))
				{
					return false;
				}
			}
			return true;
		}

		public Vector2D GetRoomPositionInsideBorder(DungeonControlLine line, double normalizedDistanceAlong, double normalizedDistanceFrom, int roomRadius, out SnakeOrientation orientation)
		{
			orientation = SnakeOrientation.Unknown;
			Vector2D target = Vector2D.Lerp(line.Start, line.End, normalizedDistanceAlong);
			Vector2D potentialRoomPosition = line.GetPotentialRoomPosition(normalizedDistanceAlong, -1.0, roomRadius);
			Vector2D potentialRoomPosition2 = line.GetPotentialRoomPosition(normalizedDistanceAlong, 1.0, roomRadius);
			Vector2D target2 = ((potentialRoomPosition.Y < potentialRoomPosition2.Y) ? potentialRoomPosition : potentialRoomPosition2);
			Vector2D target3 = ((potentialRoomPosition.Y > potentialRoomPosition2.Y) ? potentialRoomPosition : potentialRoomPosition2);
			for (int i = 0; i < 4; i++)
			{
				Vector2D potentialRoomPosition3 = line.GetPotentialRoomPosition(normalizedDistanceAlong, normalizedDistanceFrom, roomRadius);
				if (IsCircleInsideBorderWithMatchingStyle(line, potentialRoomPosition3, roomRadius))
				{
					double num = potentialRoomPosition3.Distance(target);
					double num2 = potentialRoomPosition3.Distance(target2);
					double num3 = potentialRoomPosition3.Distance(target3);
					if (num < num2 && num < num3)
					{
						orientation = SnakeOrientation.Center;
					}
					else if (num2 < num3)
					{
						orientation = SnakeOrientation.Top;
					}
					else
					{
						orientation = SnakeOrientation.Bottom;
					}
					return potentialRoomPosition3;
				}
				normalizedDistanceFrom *= 0.8;
			}
			orientation = SnakeOrientation.Center;
			return line.Center;
		}

		public void SetTangents()
		{
			DungeonControlLine dungeonControlLine = base[0];
			dungeonControlLine.StartTangent = dungeonControlLine.NormalizedLineDirection;
			while (dungeonControlLine.Next != null)
			{
				DungeonControlLine next = dungeonControlLine.Next;
				dungeonControlLine.EndTangent = -(next.StartTangent = (dungeonControlLine.NormalizedLineDirection + next.NormalizedLineDirection).SafeNormalize(default(Vector2D)));
				dungeonControlLine = next;
			}
			dungeonControlLine.EndTangent = -dungeonControlLine.NormalizedLineDirection;
		}

		public void AdjustTangentsToPreventSelfIntersection()
		{
			for (int i = 0; i < 100; i++)
			{
				bool flag = false;
				using (Enumerator enumerator = GetEnumerator())
				{
					while (enumerator.MoveNext())
					{
						if (enumerator.Current.AdjustTangentsToPreventSelfIntersection())
						{
							flag = true;
						}
					}
				}
				if (!flag)
				{
					break;
				}
			}
		}

		public bool IsLineInsideBorder(Vector2D from, Vector2D to, int halfWidth)
		{
			Vector2D vector2D = (to - from).SafeNormalize(Vector2D.UnitX).RotatedBy(Math.PI / 2.0) * halfWidth;
			if (IsLineInsideBorder(from + vector2D, to + vector2D))
			{
				return IsLineInsideBorder(from - vector2D, to - vector2D);
			}
			return false;
		}

		public bool IsLineInsideBorder(Vector2D from, Vector2D to)
		{
			DungeonControlLine line = GetClosestLineTo(from);
			return Utils.PlotLine(from.ToPoint(), to.ToPoint(), delegate(int x, int y)
			{
				line = GetLineContaining(new Vector2D(x, y), line);
				return (line != null && line.IsInsideBorder(new Point(x, y))) ? true : false;
			});
		}
	}
}
