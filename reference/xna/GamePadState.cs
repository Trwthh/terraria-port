using System.Globalization;

namespace Microsoft.Xna.Framework.Input;

public struct GamePadState
{
	private const int _normalButtonMask = 64511;

	private bool _connected;

	private int _packet;

	private GamePadThumbSticks _thumbs;

	private GamePadTriggers _triggers;

	private GamePadButtons _buttons;

	private GamePadDPad _dpad;

	private XINPUT_STATE _state;

	public GamePadButtons Buttons => _buttons;

	public GamePadDPad DPad => _dpad;

	public bool IsConnected => _connected;

	public int PacketNumber => _packet;

	public GamePadThumbSticks ThumbSticks => _thumbs;

	public GamePadTriggers Triggers => _triggers;

	public GamePadState(GamePadThumbSticks thumbSticks, GamePadTriggers triggers, GamePadButtons buttons, GamePadDPad dPad)
	{
		_packet = 0;
		_connected = true;
		_thumbs = thumbSticks;
		_triggers = triggers;
		_buttons = buttons;
		_dpad = dPad;
		_state = default(XINPUT_STATE);
		FillInternalState();
	}

	public GamePadState(Vector2 leftThumbStick, Vector2 rightThumbStick, float leftTrigger, float rightTrigger, params Buttons[] buttons)
	{
		_packet = 0;
		_connected = true;
		_thumbs = new GamePadThumbSticks(leftThumbStick, rightThumbStick);
		_triggers = new GamePadTriggers(leftTrigger, rightTrigger);
		Buttons buttons2 = (Buttons)0;
		if (buttons != null)
		{
			for (int i = 0; i < buttons.Length; i++)
			{
				buttons2 |= buttons[i];
			}
		}
		_buttons = new GamePadButtons(buttons2);
		_dpad = default(GamePadDPad);
		_dpad._down = (((buttons2 & Microsoft.Xna.Framework.Input.Buttons.DPadDown) != 0) ? ButtonState.Pressed : ButtonState.Released);
		_dpad._up = (((buttons2 & Microsoft.Xna.Framework.Input.Buttons.DPadUp) != 0) ? ButtonState.Pressed : ButtonState.Released);
		_dpad._left = (((buttons2 & Microsoft.Xna.Framework.Input.Buttons.DPadLeft) != 0) ? ButtonState.Pressed : ButtonState.Released);
		_dpad._right = (((buttons2 & Microsoft.Xna.Framework.Input.Buttons.DPadRight) != 0) ? ButtonState.Pressed : ButtonState.Released);
		_state = default(XINPUT_STATE);
		FillInternalState();
	}

	private void FillInternalState()
	{
		_state.PacketNumber = 0;
		if (Buttons.A == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.A;
		}
		if (Buttons.B == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.B;
		}
		if (Buttons.X == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.X;
		}
		if (Buttons.Y == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.Y;
		}
		if (Buttons.Back == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.Back;
		}
		if (Buttons.LeftShoulder == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.LeftShoulder;
		}
		if (Buttons.LeftStick == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.LeftThumb;
		}
		if (Buttons.RightShoulder == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.RightShoulder;
		}
		if (Buttons.RightStick == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.RightThumb;
		}
		if (Buttons.Start == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.Start;
		}
		if (Buttons.BigButton == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.BigButton;
		}
		if (DPad.Up == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.Up;
		}
		if (DPad.Down == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.Down;
		}
		if (DPad.Right == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.Right;
		}
		if (DPad.Left == ButtonState.Pressed)
		{
			_state.GamePad.Buttons |= ButtonValues.Left;
		}
		_state.GamePad.LeftTrigger = (byte)(_triggers._left * 255f);
		_state.GamePad.RightTrigger = (byte)(_triggers._right * 255f);
		_state.GamePad.ThumbLX = (short)(_thumbs._left.X * 32767f);
		_state.GamePad.ThumbLY = (short)(_thumbs._left.Y * 32767f);
		_state.GamePad.ThumbRX = (short)(_thumbs._right.X * 32767f);
		_state.GamePad.ThumbRY = (short)(_thumbs._right.Y * 32767f);
	}

	internal GamePadState(ref XINPUT_STATE pState, ErrorCodes result, GamePadDeadZone deadZoneMode)
	{
		_state = pState;
		_connected = result != ErrorCodes.NotConnected;
		_packet = pState.PacketNumber;
		_buttons._a = (((pState.GamePad.Buttons & ButtonValues.A) == ButtonValues.A) ? ButtonState.Pressed : ButtonState.Released);
		_buttons._b = (((pState.GamePad.Buttons & ButtonValues.B) == ButtonValues.B) ? ButtonState.Pressed : ButtonState.Released);
		_buttons._x = (((pState.GamePad.Buttons & ButtonValues.X) == ButtonValues.X) ? ButtonState.Pressed : ButtonState.Released);
		_buttons._y = (((pState.GamePad.Buttons & ButtonValues.Y) == ButtonValues.Y) ? ButtonState.Pressed : ButtonState.Released);
		_buttons._start = (((pState.GamePad.Buttons & ButtonValues.Start) == ButtonValues.Start) ? ButtonState.Pressed : ButtonState.Released);
		_buttons._back = (((pState.GamePad.Buttons & ButtonValues.Back) == ButtonValues.Back) ? ButtonState.Pressed : ButtonState.Released);
		_buttons._leftStick = (((pState.GamePad.Buttons & ButtonValues.LeftThumb) == ButtonValues.LeftThumb) ? ButtonState.Pressed : ButtonState.Released);
		_buttons._rightStick = (((pState.GamePad.Buttons & ButtonValues.RightThumb) == ButtonValues.RightThumb) ? ButtonState.Pressed : ButtonState.Released);
		_buttons._leftShoulder = (((pState.GamePad.Buttons & ButtonValues.LeftShoulder) == ButtonValues.LeftShoulder) ? ButtonState.Pressed : ButtonState.Released);
		_buttons._rightShoulder = (((pState.GamePad.Buttons & ButtonValues.RightShoulder) == ButtonValues.RightShoulder) ? ButtonState.Pressed : ButtonState.Released);
		_buttons._bigButton = (((pState.GamePad.Buttons & ButtonValues.BigButton) == ButtonValues.BigButton) ? ButtonState.Pressed : ButtonState.Released);
		_triggers._left = GamePadDeadZoneUtils.ApplyTriggerDeadZone(pState.GamePad.LeftTrigger, deadZoneMode);
		_triggers._right = GamePadDeadZoneUtils.ApplyTriggerDeadZone(pState.GamePad.RightTrigger, deadZoneMode);
		_thumbs._left = GamePadDeadZoneUtils.ApplyLeftStickDeadZone(pState.GamePad.ThumbLX, pState.GamePad.ThumbLY, deadZoneMode);
		_thumbs._right = GamePadDeadZoneUtils.ApplyRightStickDeadZone(pState.GamePad.ThumbRX, pState.GamePad.ThumbRY, deadZoneMode);
		_dpad._down = (((pState.GamePad.Buttons & ButtonValues.Down) == ButtonValues.Down) ? ButtonState.Pressed : ButtonState.Released);
		_dpad._up = (((pState.GamePad.Buttons & ButtonValues.Up) == ButtonValues.Up) ? ButtonState.Pressed : ButtonState.Released);
		_dpad._left = (((pState.GamePad.Buttons & ButtonValues.Left) == ButtonValues.Left) ? ButtonState.Pressed : ButtonState.Released);
		_dpad._right = (((pState.GamePad.Buttons & ButtonValues.Right) == ButtonValues.Right) ? ButtonState.Pressed : ButtonState.Released);
	}

	public bool IsButtonDown(Buttons button)
	{
		Buttons buttons = (Buttons)(_state.GamePad.Buttons & (ButtonValues.A | ButtonValues.B | ButtonValues.Back | ButtonValues.Down | ButtonValues.Left | ButtonValues.LeftShoulder | ButtonValues.LeftThumb | ButtonValues.Right | ButtonValues.RightShoulder | ButtonValues.RightThumb | ButtonValues.Start | ButtonValues.Up | ButtonValues.X | ButtonValues.Y | ButtonValues.BigButton));
		if ((button & Microsoft.Xna.Framework.Input.Buttons.LeftThumbstickLeft) == Microsoft.Xna.Framework.Input.Buttons.LeftThumbstickLeft && GamePadDeadZoneUtils.ApplyLeftStickDeadZone(_state.GamePad.ThumbLX, _state.GamePad.ThumbLY, GamePadDeadZone.IndependentAxes).X < 0f)
		{
			buttons |= Microsoft.Xna.Framework.Input.Buttons.LeftThumbstickLeft;
		}
		if ((button & Microsoft.Xna.Framework.Input.Buttons.LeftThumbstickRight) == Microsoft.Xna.Framework.Input.Buttons.LeftThumbstickRight && GamePadDeadZoneUtils.ApplyLeftStickDeadZone(_state.GamePad.ThumbLX, _state.GamePad.ThumbLY, GamePadDeadZone.IndependentAxes).X > 0f)
		{
			buttons |= Microsoft.Xna.Framework.Input.Buttons.LeftThumbstickRight;
		}
		if ((button & Microsoft.Xna.Framework.Input.Buttons.LeftThumbstickDown) == Microsoft.Xna.Framework.Input.Buttons.LeftThumbstickDown && GamePadDeadZoneUtils.ApplyLeftStickDeadZone(_state.GamePad.ThumbLX, _state.GamePad.ThumbLY, GamePadDeadZone.IndependentAxes).Y < 0f)
		{
			buttons |= Microsoft.Xna.Framework.Input.Buttons.LeftThumbstickDown;
		}
		if ((button & Microsoft.Xna.Framework.Input.Buttons.LeftThumbstickUp) == Microsoft.Xna.Framework.Input.Buttons.LeftThumbstickUp && GamePadDeadZoneUtils.ApplyLeftStickDeadZone(_state.GamePad.ThumbLX, _state.GamePad.ThumbLY, GamePadDeadZone.IndependentAxes).Y > 0f)
		{
			buttons |= Microsoft.Xna.Framework.Input.Buttons.LeftThumbstickUp;
		}
		if ((button & Microsoft.Xna.Framework.Input.Buttons.RightThumbstickLeft) == Microsoft.Xna.Framework.Input.Buttons.RightThumbstickLeft && GamePadDeadZoneUtils.ApplyRightStickDeadZone(_state.GamePad.ThumbRX, _state.GamePad.ThumbRY, GamePadDeadZone.IndependentAxes).X < 0f)
		{
			buttons |= Microsoft.Xna.Framework.Input.Buttons.RightThumbstickLeft;
		}
		if ((button & Microsoft.Xna.Framework.Input.Buttons.RightThumbstickRight) == Microsoft.Xna.Framework.Input.Buttons.RightThumbstickRight && GamePadDeadZoneUtils.ApplyRightStickDeadZone(_state.GamePad.ThumbRX, _state.GamePad.ThumbRY, GamePadDeadZone.IndependentAxes).X > 0f)
		{
			buttons |= Microsoft.Xna.Framework.Input.Buttons.RightThumbstickRight;
		}
		if ((button & Microsoft.Xna.Framework.Input.Buttons.RightThumbstickDown) == Microsoft.Xna.Framework.Input.Buttons.RightThumbstickDown && GamePadDeadZoneUtils.ApplyRightStickDeadZone(_state.GamePad.ThumbRX, _state.GamePad.ThumbRY, GamePadDeadZone.IndependentAxes).Y < 0f)
		{
			buttons |= Microsoft.Xna.Framework.Input.Buttons.RightThumbstickDown;
		}
		if ((button & Microsoft.Xna.Framework.Input.Buttons.RightThumbstickUp) == Microsoft.Xna.Framework.Input.Buttons.RightThumbstickUp && GamePadDeadZoneUtils.ApplyRightStickDeadZone(_state.GamePad.ThumbRX, _state.GamePad.ThumbRY, GamePadDeadZone.IndependentAxes).Y > 0f)
		{
			buttons |= Microsoft.Xna.Framework.Input.Buttons.RightThumbstickUp;
		}
		if ((button & Microsoft.Xna.Framework.Input.Buttons.LeftTrigger) == Microsoft.Xna.Framework.Input.Buttons.LeftTrigger && GamePadDeadZoneUtils.ApplyTriggerDeadZone(_state.GamePad.LeftTrigger, GamePadDeadZone.IndependentAxes) > 0f)
		{
			buttons |= Microsoft.Xna.Framework.Input.Buttons.LeftTrigger;
		}
		if ((button & Microsoft.Xna.Framework.Input.Buttons.RightTrigger) == Microsoft.Xna.Framework.Input.Buttons.RightTrigger && GamePadDeadZoneUtils.ApplyTriggerDeadZone(_state.GamePad.RightTrigger, GamePadDeadZone.IndependentAxes) > 0f)
		{
			buttons |= Microsoft.Xna.Framework.Input.Buttons.RightTrigger;
		}
		return (button & buttons) == button;
	}

	public bool IsButtonUp(Buttons button)
	{
		return !IsButtonDown(button);
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
		return this == (GamePadState)obj;
	}

	public override int GetHashCode()
	{
		return _thumbs.GetHashCode() ^ _triggers.GetHashCode() ^ (_buttons.GetHashCode() ^ _connected.GetHashCode()) ^ (_dpad.GetHashCode() ^ _packet.GetHashCode());
	}

	public override string ToString()
	{
		return string.Format(CultureInfo.CurrentCulture, "{{IsConnected:{0}}}", new object[1] { _connected });
	}

	public static bool operator ==(GamePadState left, GamePadState right)
	{
		if (left._connected == right._connected && left._packet == right._packet && left._thumbs == right._thumbs && left._triggers == right._triggers && left._buttons == right._buttons)
		{
			return left._dpad == right._dpad;
		}
		return false;
	}

	public static bool operator !=(GamePadState left, GamePadState right)
	{
		if (left._connected == right._connected && left._packet == right._packet && !(left._thumbs != right._thumbs) && !(left._triggers != right._triggers) && !(left._buttons != right._buttons))
		{
			return left._dpad != right._dpad;
		}
		return true;
	}
}
