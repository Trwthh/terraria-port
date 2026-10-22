using System.Globalization;

namespace Microsoft.Xna.Framework.Input;

public struct MouseState
{
	internal int x;

	internal int y;

	internal ButtonState leftButton;

	internal ButtonState rightButton;

	internal ButtonState middleButton;

	internal ButtonState xb1;

	internal ButtonState xb2;

	internal int wheel;

	public int X => x;

	public int Y => y;

	public ButtonState LeftButton => leftButton;

	public ButtonState RightButton => rightButton;

	public ButtonState MiddleButton => middleButton;

	public ButtonState XButton1 => xb1;

	public ButtonState XButton2 => xb2;

	public int ScrollWheelValue => wheel;

	public MouseState(int x, int y, int scrollWheel, ButtonState leftButton, ButtonState middleButton, ButtonState rightButton, ButtonState xButton1, ButtonState xButton2)
	{
		this.x = x;
		this.y = y;
		wheel = scrollWheel;
		this.leftButton = leftButton;
		this.rightButton = rightButton;
		this.middleButton = middleButton;
		xb1 = xButton1;
		xb2 = xButton2;
	}

	public override int GetHashCode()
	{
		return x.GetHashCode() ^ y.GetHashCode() ^ leftButton.GetHashCode() ^ rightButton.GetHashCode() ^ middleButton.GetHashCode() ^ xb1.GetHashCode() ^ xb2.GetHashCode() ^ wheel.GetHashCode();
	}

	public override string ToString()
	{
		string text = string.Empty;
		if (leftButton == ButtonState.Pressed)
		{
			text = text + (string.IsNullOrEmpty(text) ? "" : " ") + "Left";
		}
		if (rightButton == ButtonState.Pressed)
		{
			text = text + (string.IsNullOrEmpty(text) ? "" : " ") + "Right";
		}
		if (middleButton == ButtonState.Pressed)
		{
			text = text + (string.IsNullOrEmpty(text) ? "" : " ") + "Middle";
		}
		if (xb1 == ButtonState.Pressed)
		{
			text = text + (string.IsNullOrEmpty(text) ? "" : " ") + "XButton1";
		}
		if (xb2 == ButtonState.Pressed)
		{
			text = text + (string.IsNullOrEmpty(text) ? "" : " ") + "XButton2";
		}
		if (string.IsNullOrEmpty(text))
		{
			text = "None";
		}
		return string.Format(CultureInfo.CurrentCulture, "{{X:{0} Y:{1} Buttons:{2} Wheel:{3}}}", x, y, text, wheel);
	}

	public override bool Equals(object obj)
	{
		if (obj is MouseState)
		{
			return this == (MouseState)obj;
		}
		return false;
	}

	public static bool operator ==(MouseState left, MouseState right)
	{
		if (left.x == right.x && left.y == right.y && left.leftButton == right.leftButton && left.rightButton == right.rightButton && left.middleButton == right.middleButton && left.xb1 == right.xb1 && left.xb2 == right.xb2)
		{
			return left.wheel == right.wheel;
		}
		return false;
	}

	public static bool operator !=(MouseState left, MouseState right)
	{
		return !(left == right);
	}
}
