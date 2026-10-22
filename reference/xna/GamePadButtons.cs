using System.Globalization;

namespace Microsoft.Xna.Framework.Input;

public struct GamePadButtons
{
	internal ButtonState _a;

	internal ButtonState _b;

	internal ButtonState _x;

	internal ButtonState _y;

	internal ButtonState _leftStick;

	internal ButtonState _rightStick;

	internal ButtonState _leftShoulder;

	internal ButtonState _rightShoulder;

	internal ButtonState _back;

	internal ButtonState _start;

	internal ButtonState _bigButton;

	public ButtonState A => _a;

	public ButtonState B => _b;

	public ButtonState Back => _back;

	public ButtonState X => _x;

	public ButtonState Y => _y;

	public ButtonState Start => _start;

	public ButtonState LeftShoulder => _leftShoulder;

	public ButtonState LeftStick => _leftStick;

	public ButtonState RightShoulder => _rightShoulder;

	public ButtonState RightStick => _rightStick;

	public ButtonState BigButton => _bigButton;

	public GamePadButtons(Buttons buttons)
	{
		_a = (((buttons & Buttons.A) == Buttons.A) ? ButtonState.Pressed : ButtonState.Released);
		_b = (((buttons & Buttons.B) == Buttons.B) ? ButtonState.Pressed : ButtonState.Released);
		_x = (((buttons & Buttons.X) == Buttons.X) ? ButtonState.Pressed : ButtonState.Released);
		_y = (((buttons & Buttons.Y) == Buttons.Y) ? ButtonState.Pressed : ButtonState.Released);
		_start = (((buttons & Buttons.Start) == Buttons.Start) ? ButtonState.Pressed : ButtonState.Released);
		_back = (((buttons & Buttons.Back) == Buttons.Back) ? ButtonState.Pressed : ButtonState.Released);
		_leftStick = (((buttons & Buttons.LeftStick) == Buttons.LeftStick) ? ButtonState.Pressed : ButtonState.Released);
		_rightStick = (((buttons & Buttons.RightStick) == Buttons.RightStick) ? ButtonState.Pressed : ButtonState.Released);
		_leftShoulder = (((buttons & Buttons.LeftShoulder) == Buttons.LeftShoulder) ? ButtonState.Pressed : ButtonState.Released);
		_rightShoulder = (((buttons & Buttons.RightShoulder) == Buttons.RightShoulder) ? ButtonState.Pressed : ButtonState.Released);
		_bigButton = (((buttons & Buttons.BigButton) == Buttons.BigButton) ? ButtonState.Pressed : ButtonState.Released);
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
		return this == (GamePadButtons)obj;
	}

	public override int GetHashCode()
	{
		return Helpers.SmartGetHashCode(this);
	}

	public override string ToString()
	{
		string text = string.Empty;
		if (_a == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "A";
		}
		if (_b == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "B";
		}
		if (_x == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "X";
		}
		if (_y == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "Y";
		}
		if (_leftShoulder == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "LeftShoulder";
		}
		if (_rightShoulder == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "RightShoulder";
		}
		if (_leftStick == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "LeftStick";
		}
		if (_rightStick == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "RightStick";
		}
		if (_start == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "Start";
		}
		if (_back == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "Back";
		}
		if (_bigButton == ButtonState.Pressed)
		{
			text = text + ((text.Length != 0) ? " " : "") + "BigButton";
		}
		if (text.Length == 0)
		{
			text = "None";
		}
		return string.Format(CultureInfo.CurrentCulture, "{{Buttons:{0}}}", new object[1] { text });
	}

	public static bool operator ==(GamePadButtons left, GamePadButtons right)
	{
		if (left._a == right._a && left._b == right._b && left._x == right._x && left._y == right._y && left._leftShoulder == right._leftShoulder && left._leftStick == right._leftStick && left._rightShoulder == right._rightShoulder && left._rightStick == right._rightStick && left._back == right._back && left._start == right._start)
		{
			return left._bigButton == right._bigButton;
		}
		return false;
	}

	public static bool operator !=(GamePadButtons left, GamePadButtons right)
	{
		if (left._a == right._a && left._b == right._b && left._x == right._x && left._y == right._y && left._leftShoulder == right._leftShoulder && left._leftStick == right._leftStick && left._rightShoulder == right._rightShoulder && left._rightStick == right._rightStick && left._back == right._back && left._start == right._start)
		{
			return left._bigButton != right._bigButton;
		}
		return true;
	}
}
