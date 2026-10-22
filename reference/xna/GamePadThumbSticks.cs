using System.Globalization;

namespace Microsoft.Xna.Framework.Input;

public struct GamePadThumbSticks
{
	internal Vector2 _left;

	internal Vector2 _right;

	public Vector2 Left => _left;

	public Vector2 Right => _right;

	public GamePadThumbSticks(Vector2 leftThumbstick, Vector2 rightThumbstick)
	{
		_left = leftThumbstick;
		_right = rightThumbstick;
		_left = Vector2.Min(_left, Vector2.One);
		_left = Vector2.Max(_left, -Vector2.One);
		_right = Vector2.Min(_right, Vector2.One);
		_right = Vector2.Max(_right, -Vector2.One);
	}

	public override bool Equals(object obj)
	{
		if (obj == null)
		{
			return false;
		}
		if (obj.GetType() != GetType())
		{
			return false;
		}
		return this == (GamePadThumbSticks)obj;
	}

	public override int GetHashCode()
	{
		return Helpers.SmartGetHashCode(this);
	}

	public override string ToString()
	{
		return string.Format(CultureInfo.CurrentCulture, "{{Left:{0} Right:{1}}}", new object[2] { _left, _right });
	}

	public static bool operator ==(GamePadThumbSticks left, GamePadThumbSticks right)
	{
		if (left._left == right._left)
		{
			return left._right == right._right;
		}
		return false;
	}

	public static bool operator !=(GamePadThumbSticks left, GamePadThumbSticks right)
	{
		if (!(left._left != right._left))
		{
			return left._right != right._right;
		}
		return true;
	}
}
