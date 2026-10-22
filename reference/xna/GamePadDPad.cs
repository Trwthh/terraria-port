using System.Globalization;

namespace Microsoft.Xna.Framework.Input;

public struct GamePadDPad
{
	internal ButtonState _up;

	internal ButtonState _right;

	internal ButtonState _down;

	internal ButtonState _left;

	public ButtonState Up => _up;

	public ButtonState Down => _down;

	public ButtonState Right => _right;

	public ButtonState Left => _left;

	public GamePadDPad(ButtonState upValue, ButtonState downValue, ButtonState leftValue, ButtonState rightValue)
	{
		_up = upValue;
		_right = rightValue;
		_down = downValue;
		_left = leftValue;
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
		return this == (GamePadDPad)obj;
	}

	public override int GetHashCode()
	{
		return Helpers.SmartGetHashCode(this);
	}

	public override string ToString()
	{
		string text = string.Empty;
		if (_up == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "Up";
		}
		if (_down == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "Down";
		}
		if (_left == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "Left";
		}
		if (_right == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "Right";
		}
		if (text.Length == 0)
		{
			text = "None";
		}
		return string.Format(CultureInfo.CurrentCulture, "{{DPad:{0}}}", new object[1] { text });
	}

	public static bool operator ==(GamePadDPad left, GamePadDPad right)
	{
		if (left._up == right._up && left._down == right._down && left._left == right._left)
		{
			return left._right == right._right;
		}
		return false;
	}

	public static bool operator !=(GamePadDPad left, GamePadDPad right)
	{
		if (left._up == right._up && left._down == right._down && left._left == right._left)
		{
			return left._right != right._right;
		}
		return true;
	}
}
