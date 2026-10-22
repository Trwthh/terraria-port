using Microsoft.Xna.Framework;
using Newtonsoft.Json;
using ReLogic.Utilities;
using Terraria.Utilities;

namespace Terraria.GameContent.Generation.Dungeon
{
	public class DungeonBounds
	{
		[JsonProperty]
		private Rectangle? _hitbox;

		private int _boundsLeft;

		private int _boundsRight;

		private int _boundsTop;

		private int _boundsBottom;

		public Rectangle Hitbox
		{
			get
			{
				if (_hitbox.HasValue)
				{
					return _hitbox.Value;
				}
				return Rectangle.Empty;
			}
		}

		public int X => _boundsLeft;

		public int Y => _boundsTop;

		public int Width => _boundsRight - _boundsLeft;

		public int Height => _boundsBottom - _boundsTop;

		public int Size
		{
			get
			{
				if (Width <= Height)
				{
					return Height;
				}
				return Width;
			}
		}

		public int Left
		{
			get
			{
				return _boundsLeft;
			}
			set
			{
				_boundsLeft = (int)MathHelper.Clamp(value, 10f, Main.maxTilesX - 10);
			}
		}

		public int Right
		{
			get
			{
				return _boundsRight;
			}
			set
			{
				_boundsRight = (int)MathHelper.Clamp(value, 10f, Main.maxTilesX - 10);
			}
		}

		public int Top
		{
			get
			{
				return _boundsTop;
			}
			set
			{
				_boundsTop = (int)MathHelper.Clamp(value, 10f, Main.maxTilesY - 10);
			}
		}

		public int Bottom
		{
			get
			{
				return _boundsBottom;
			}
			set
			{
				_boundsBottom = (int)MathHelper.Clamp(value, 10f, Main.maxTilesY - 10);
			}
		}

		public Point Center => new Point((Left + Right) / 2, (Top + Bottom) / 2);

		public Point RandomPointInBounds(UnifiedRandom genRand)
		{
			return new Point(genRand.Next(Left, Right + 1), genRand.Next(Top, Bottom + 1));
		}

		public void Inflate(int amount)
		{
			SetBounds(Left - amount, Top - amount, Right + amount, Bottom + amount);
		}

		public void Shrink(int amount)
		{
			SetBounds(Left + amount, Top + amount, Right - amount, Bottom - amount);
		}

		public bool ContainsWithFluff(Vector2 point, int fluff)
		{
			if (fluff == 0)
			{
				return Contains((int)point.X, (int)point.Y);
			}
			return ContainsWithFluff((int)point.X, (int)point.Y, fluff);
		}

		public bool ContainsWithFluff(Vector2D point, int fluff)
		{
			if (fluff == 0)
			{
				return Contains((int)point.X, (int)point.Y);
			}
			return ContainsWithFluff((int)point.X, (int)point.Y, fluff);
		}

		public bool ContainsWithFluff(Point point, int fluff)
		{
			if (fluff == 0)
			{
				return Contains(point.X, point.Y);
			}
			return ContainsWithFluff(point.X, point.Y, fluff);
		}

		public bool ContainsWithFluff(int x, int y, int fluff)
		{
			if (fluff == 0)
			{
				return Contains(x, y);
			}
			if (!_hitbox.HasValue)
			{
				return false;
			}
			Rectangle rectangle = new Rectangle(_hitbox.Value.Left - fluff, _hitbox.Value.Top - fluff, _hitbox.Value.Width + fluff * 2, _hitbox.Value.Height + fluff * 2);
			return rectangle.Contains(x, y);
		}

		public bool Contains(Vector2D point)
		{
			return Contains((int)point.X, (int)point.Y);
		}

		public bool Contains(Point point)
		{
			return Contains(point.X, point.Y);
		}

		public bool Contains(int x, int y)
		{
			if (!_hitbox.HasValue)
			{
				return false;
			}
			return _hitbox.Value.Contains(x, y);
		}

		public bool Intersects(DungeonBounds bounds)
		{
			if (!bounds.HasHitbox())
			{
				return false;
			}
			return Intersects(bounds.Hitbox);
		}

		public bool Intersects(Rectangle hitbox)
		{
			if (!_hitbox.HasValue)
			{
				return false;
			}
			return _hitbox.Value.Intersects(hitbox);
		}

		public bool IntersectsWithLineThreePointCheck(Point startPoint, Point endPoint)
		{
			return IntersectsWithLineThreePointCheck(startPoint.ToVector2D(), endPoint.ToVector2D());
		}

		public bool IntersectsWithLineThreePointCheck(int startPointX, int startPointY, int endPointX, int endPointY)
		{
			return IntersectsWithLineThreePointCheck(new Vector2D(startPointX, startPointY), new Vector2D(endPointX, endPointY));
		}

		public bool IntersectsWithLineThreePointCheck(Vector2D startPoint, Vector2D endPoint)
		{
			if (!_hitbox.HasValue)
			{
				return false;
			}
			if (Contains(startPoint) || Contains(endPoint) || Contains((startPoint + endPoint) / 2.0))
			{
				return true;
			}
			return false;
		}

		public bool HasHitbox()
		{
			return _hitbox.HasValue;
		}

		public void SetBoundsLeft(int minX)
		{
			Left = minX;
		}

		public void SetBoundsRight(int maxX)
		{
			Right = maxX;
		}

		public void SetBoundsTop(int minY)
		{
			Top = minY;
		}

		public void SetBoundsBottom(int maxY)
		{
			Bottom = maxY;
		}

		public void SetBounds(Rectangle rect)
		{
			SetBounds(rect.Left, rect.Top, rect.Right, rect.Bottom);
		}

		public void SetBounds(int minX, int minY, int maxX, int maxY)
		{
			Left = minX;
			Right = maxX;
			Top = minY;
			Bottom = maxY;
			CalculateHitbox();
		}

		public void UpdateBounds(int x, int y)
		{
			if (x < _boundsLeft)
			{
				Left = x;
			}
			if (x > _boundsRight)
			{
				Right = x;
			}
			if (y < _boundsTop)
			{
				Top = y;
			}
			if (y > _boundsBottom)
			{
				Bottom = y;
			}
		}

		public void UpdateBounds(DungeonBounds bounds)
		{
			if (Width == 0 || Height == 0)
			{
				SetBounds(bounds.Left, bounds.Top, bounds.Right, bounds.Bottom);
			}
			else
			{
				UpdateBounds(bounds.Left, bounds.Top, bounds.Right, bounds.Bottom);
			}
		}

		public void UpdateBounds(int minX, int minY, int maxX, int maxY)
		{
			if (minX < _boundsLeft)
			{
				Left = minX;
			}
			if (maxX > _boundsRight)
			{
				Right = maxX;
			}
			if (minY < _boundsTop)
			{
				Top = minY;
			}
			if (maxY > _boundsBottom)
			{
				Bottom = maxY;
			}
		}

		public Rectangle CalculateHitbox()
		{
			if (Right <= Left)
			{
				Right = Left + 1;
			}
			if (Bottom <= Top)
			{
				Bottom = Top + 1;
			}
			_hitbox = new Rectangle(X, Y, Width, Height);
			return _hitbox.Value;
		}

		public void Reset()
		{
			_hitbox = null;
			Left = 0;
			Right = 0;
			Top = 0;
			Bottom = 0;
		}

		public override string ToString()
		{
			return $"Bounds: {Left}, {Top}, {Right}, {Bottom} | {Width} x {Height}";
		}
	}
}
