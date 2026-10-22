using Microsoft.Xna.Framework;

namespace Terraria.DataStructures
{
	public class MultiPointHitbox
	{
		public readonly Point PointSize;

		public readonly Vector2[] Points;

		public readonly Rectangle BoundingRect;

		public MultiPointHitbox(Point pointSize, Vector2[] points)
		{
			PointSize = pointSize;
			Points = points;
			Rectangle rectangle = Utils.CenteredRectangle(points[0], Vector2.Zero);
			foreach (Vector2 v in points)
			{
				rectangle = rectangle.Including(v.ToPoint());
			}
			BoundingRect = rectangle;
		}

		public bool Intersects(Rectangle targetRect)
		{
			targetRect.Inflate(PointSize.X / 2, PointSize.Y / 2);
			if (!BoundingRect.Intersects(targetRect))
			{
				return false;
			}
			Vector2[] points = Points;
			foreach (Vector2 v in points)
			{
				if (targetRect.Contains(v.ToPoint()))
				{
					return true;
				}
			}
			return false;
		}
	}
}
