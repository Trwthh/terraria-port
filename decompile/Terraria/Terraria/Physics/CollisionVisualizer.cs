using System;
using Microsoft.Xna.Framework;
using Terraria.DataStructures;

namespace Terraria.Physics
{
	public static class CollisionVisualizer
	{
		public static bool enabled;

		public static Color colorSelf;

		public static Color colorTarget;

		public static void VisualizeElipticalLine(Vector2 attacker, Vector2 target, float allowedDistance, float squishX, float squishY, bool needsCanHitCheck)
		{
			if (enabled)
			{
				Vector2 vector = (target - attacker).SafeNormalize(Vector2.UnitX);
				vector.X /= squishX;
				vector.Y /= squishY;
				Vector2 vector2 = Vector2.Zero.MoveTowards(target - attacker, allowedDistance) * new Vector2(squishX, squishY);
				Utils.DrawLine(Main.spriteBatch, attacker, attacker + vector2, colorSelf, colorSelf, 8f);
			}
		}

		public static void VisualizeRects(Rectangle attacker, Rectangle target, bool needsCanHitCheck)
		{
			if (enabled)
			{
				Utils.DrawLine(Main.spriteBatch, new Vector2(attacker.Left + attacker.Width / 2, attacker.Top), new Vector2(attacker.Left + attacker.Width / 2, attacker.Bottom), colorSelf, colorSelf, attacker.Width);
			}
		}

		public static void VisualizeConeFast(Vector2 attacker, Rectangle targetRect, float correctedAngle, float maximumAngle, float allowedDistance, bool needsCanHitCheck)
		{
			if (enabled)
			{
				Color color = colorSelf * 0.3f;
				for (float num = maximumAngle; num > 0f; num -= (float)Math.PI / 180f)
				{
					Vector2 vector = (correctedAngle - num).ToRotationVector2();
					Vector2 vector2 = (correctedAngle + num).ToRotationVector2();
					Utils.DrawLine(Main.spriteBatch, attacker, attacker + vector * allowedDistance, color, color, 4f);
					Utils.DrawLine(Main.spriteBatch, attacker, attacker + vector2 * allowedDistance, color, color, 4f);
				}
			}
		}

		public static void VisualizeConeSlow(Vector2 attacker, Rectangle targetRect, float correctedAngle, float maximumAngle, float allowedDistance, bool needsCanHitCheck)
		{
			if (enabled)
			{
				Color color = colorSelf * 0.3f;
				for (float num = maximumAngle; num > 0f; num -= (float)Math.PI / 180f)
				{
					Vector2 vector = (correctedAngle - num).ToRotationVector2();
					Vector2 vector2 = (correctedAngle + num).ToRotationVector2();
					Utils.DrawLine(Main.spriteBatch, attacker, attacker + vector * allowedDistance, color, color, 4f);
					Utils.DrawLine(Main.spriteBatch, attacker, attacker + vector2 * allowedDistance, color, color, 4f);
				}
			}
		}

		public static void VisualizeAABBvLine(Rectangle targetRect, Vector2 lineStart, Vector2 lineEnd, float lineWidth)
		{
			if (enabled)
			{
				Utils.DrawLine(Main.spriteBatch, lineStart, lineEnd, colorSelf, colorSelf, lineWidth);
			}
		}

		public static void VisualizeAABBvMultiPoint(Rectangle targetRectangle, MultiPointHitbox lightning)
		{
			if (enabled)
			{
				Vector2[] points = lightning.Points;
				foreach (Vector2 center in points)
				{
					Utils.DrawRect(Main.spriteBatch, Utils.CenteredRectangle(center, new Vector2(4f, 4f)), colorSelf);
				}
			}
		}
	}
}
