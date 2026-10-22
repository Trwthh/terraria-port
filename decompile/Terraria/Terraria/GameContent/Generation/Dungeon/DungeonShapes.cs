using System;
using Microsoft.Xna.Framework;
using Terraria.WorldBuilding;

namespace Terraria.GameContent.Generation.Dungeon
{
	public class DungeonShapes
	{
		public class CircleRoom : GenShape
		{
			private int _verticalRadius;

			private int _horizontalRadius;

			public int VerticalRadius => _verticalRadius;

			public int HorizontalRadius => _horizontalRadius;

			public CircleRoom(int radius)
			{
				_verticalRadius = radius;
				_horizontalRadius = radius;
			}

			public CircleRoom(int horizontalRadius, int verticalRadius)
			{
				_horizontalRadius = horizontalRadius;
				_verticalRadius = verticalRadius;
			}

			public void SetRadius(int radius)
			{
				_verticalRadius = radius;
				_horizontalRadius = radius;
			}

			public override bool Perform(Point origin, GenAction action)
			{
				int num = (_horizontalRadius + 1) * (_horizontalRadius + 1);
				for (int i = origin.Y - _verticalRadius; i <= origin.Y + _verticalRadius; i++)
				{
					double num2 = (double)_horizontalRadius / (double)_verticalRadius * (double)(i - origin.Y);
					int num3 = Math.Min(_horizontalRadius, (int)Math.Sqrt((double)num - num2 * num2));
					for (int j = origin.X - num3; j <= origin.X + num3; j++)
					{
						if (!UnitApply(action, origin, j, i) && _quitOnFail)
						{
							return false;
						}
					}
				}
				return true;
			}
		}

		public class MoundRoom : GenShape
		{
			private int _halfWidth;

			private int _height;

			public MoundRoom(int halfWidth, int height)
			{
				_halfWidth = halfWidth;
				_height = height;
			}

			public override bool Perform(Point origin, GenAction action)
			{
				_ = _height;
				float num = _halfWidth;
				int num2 = _height / 2;
				for (int i = -_halfWidth; i <= _halfWidth; i++)
				{
					int num3 = Math.Min(_height, (int)((0f - (float)(_height + 1) / (num * num)) * ((float)i + num) * ((float)i - num)));
					for (int j = 0; j < num3; j++)
					{
						if (!UnitApply(action, origin, i + origin.X, origin.Y - j + num2) && _quitOnFail)
						{
							return false;
						}
					}
				}
				return true;
			}
		}

		public class HourglassRoom : GenShape
		{
			private int _width;

			private int _height;

			private float _percentileAddon;

			public HourglassRoom(int width, int height, float percentileAddon)
			{
				_width = width;
				_height = height;
				_percentileAddon = percentileAddon;
			}

			public override bool Perform(Point origin, GenAction action)
			{
				int num = _height / 2;
				for (int i = -num; i <= num; i++)
				{
					int y = origin.Y + i;
					float percent = ((float)i + (float)num) / (float)_height;
					float num2 = Math.Max(0f, Math.Min(1f, Utils.MultiLerp(Utils.WrappedLerp(0f, 1f, percent), 1f, 1f, 0.75f, 0.65f, 0.45f, 0.4f, 0.35f, 0.35f) + _percentileAddon));
					int num3 = (int)((float)_width * num2) / 2;
					for (int j = -num3; j <= num3; j++)
					{
						int x = origin.X + j;
						if (!UnitApply(action, origin, x, y) && _quitOnFail)
						{
							return false;
						}
					}
				}
				return true;
			}
		}

		public class QuadCircleRoom : GenShape
		{
			private int _radius;

			private int _distanceBetweenSpheres;

			public int Radius => _radius;

			public QuadCircleRoom(int radius, int distanceBetweenSpheres)
			{
				_radius = radius;
				_distanceBetweenSpheres = distanceBetweenSpheres;
			}

			public void SetRadius(int radius)
			{
				_radius = radius;
			}

			public override bool Perform(Point origin, GenAction action)
			{
				int num = (_radius + 1) * (_radius + 1);
				Point point = origin;
				int num2 = 3;
				for (int i = 0; i < 5; i++)
				{
					switch (i)
					{
					default:
						point = new Vector2(origin.X, origin.Y - _distanceBetweenSpheres + num2).ToPoint();
						break;
					case 1:
						point = new Vector2(origin.X, origin.Y + _distanceBetweenSpheres - num2).ToPoint();
						break;
					case 2:
						point = new Vector2(origin.X - _distanceBetweenSpheres + num2, origin.Y).ToPoint();
						break;
					case 3:
						point = new Vector2(origin.X + _distanceBetweenSpheres - num2, origin.Y).ToPoint();
						break;
					case 4:
						point = origin;
						break;
					}
					for (int j = point.Y - _radius; j <= point.Y + _radius; j++)
					{
						double num3 = (double)_radius / (double)_radius * (double)(j - point.Y);
						int num4 = Math.Min(_radius, (int)Math.Sqrt((double)num - num3 * num3));
						for (int k = point.X - num4; k <= point.X + num4; k++)
						{
							if (!UnitApply(action, origin, k, j) && _quitOnFail)
							{
								return false;
							}
						}
					}
				}
				return true;
			}
		}
	}
}
