using System;
using System.Globalization;

namespace Microsoft.Xna.Framework.Input;

public struct GamePadTriggers
{
	internal float _left;

	internal float _right;

	public float Left => _left;

	public float Right => _right;

	public GamePadTriggers(float leftTrigger, float rightTrigger)
	{
		_left = leftTrigger;
		_right = rightTrigger;
		_left = Math.Min(_left, 1f);
		_left = Math.Max(_left, 0f);
		_right = Math.Min(_right, 1f);
		_right = Math.Max(_right, 0f);
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
		return this == (GamePadTriggers)obj;
	}

	public override int GetHashCode()
	{
		return Helpers.SmartGetHashCode(this);
	}

	public override string ToString()
	{
		return string.Format(CultureInfo.CurrentCulture, "{{Left:{0} Right:{1}}}", new object[2] { _left, _right });
	}

	public static bool operator ==(GamePadTriggers left, GamePadTriggers right)
	{
		if (left._left == right._left)
		{
			return left._right == right._right;
		}
		return false;
	}

	public static bool operator !=(GamePadTriggers left, GamePadTriggers right)
	{
		if (left._left == right._left)
		{
			return left._right != right._right;
		}
		return true;
	}
}
