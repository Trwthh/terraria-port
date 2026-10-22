using System;
using System.Diagnostics;

namespace Microsoft.Xna.Framework.Input;

public static class GamePad
{
	internal const string XinputNativeDll = "xinput1_3.dll";

	private static bool[] _disconnected = new bool[4];

	private static long[] _lastReadTime = new long[4];

	public static GamePadState GetState(PlayerIndex playerIndex)
	{
		return GetState(playerIndex, GamePadDeadZone.IndependentAxes);
	}

	public static GamePadState GetState(PlayerIndex playerIndex, GamePadDeadZone deadZoneMode)
	{
		XINPUT_STATE pState = default(XINPUT_STATE);
		ErrorCodes errorCodes = ErrorCodes.Success;
		if (ThrottleDisconnectedRetries(playerIndex))
		{
			errorCodes = ErrorCodes.NotConnected;
		}
		else
		{
			errorCodes = UnsafeNativeMethods.GetState(playerIndex, out pState);
			ResetThrottleState(playerIndex, errorCodes);
		}
		if (errorCodes != ErrorCodes.Success && errorCodes != ErrorCodes.NotConnected)
		{
			throw new InvalidOperationException(FrameworkResources.InvalidController);
		}
		return new GamePadState(ref pState, errorCodes, deadZoneMode);
	}

	public static GamePadCapabilities GetCapabilities(PlayerIndex playerIndex)
	{
		XINPUT_CAPABILITIES pCaps = default(XINPUT_CAPABILITIES);
		ErrorCodes errorCodes = ErrorCodes.Success;
		if (ThrottleDisconnectedRetries(playerIndex))
		{
			errorCodes = ErrorCodes.NotConnected;
		}
		else
		{
			errorCodes = UnsafeNativeMethods.GetCaps(playerIndex, 1u, out pCaps);
			ResetThrottleState(playerIndex, errorCodes);
		}
		if (errorCodes != ErrorCodes.Success && errorCodes != ErrorCodes.NotConnected)
		{
			throw new InvalidOperationException(FrameworkResources.InvalidController);
		}
		return new GamePadCapabilities(ref pCaps, errorCodes);
	}

	public static bool SetVibration(PlayerIndex playerIndex, float leftMotor, float rightMotor)
	{
		XINPUT_VIBRATION pVibration = default(XINPUT_VIBRATION);
		pVibration.LeftMotorSpeed = (short)(leftMotor * 65535f);
		pVibration.RightMotorSpeed = (short)(rightMotor * 65535f);
		ErrorCodes errorCodes = ErrorCodes.Success;
		if (ThrottleDisconnectedRetries(playerIndex))
		{
			errorCodes = ErrorCodes.NotConnected;
		}
		else
		{
			errorCodes = UnsafeNativeMethods.SetState(playerIndex, ref pVibration);
			ResetThrottleState(playerIndex, errorCodes);
		}
		if (errorCodes == ErrorCodes.Success)
		{
			return true;
		}
		if (errorCodes != ErrorCodes.Success && errorCodes != ErrorCodes.NotConnected && errorCodes != ErrorCodes.Busy)
		{
			throw new InvalidOperationException(FrameworkResources.InvalidController);
		}
		return false;
	}

	private static bool ThrottleDisconnectedRetries(PlayerIndex playerIndex)
	{
		if (playerIndex < PlayerIndex.One || playerIndex > PlayerIndex.Four)
		{
			return false;
		}
		if (!_disconnected[(int)playerIndex])
		{
			return false;
		}
		long timestamp = Stopwatch.GetTimestamp();
		for (int i = 0; i < 4; i++)
		{
			if (_disconnected[i])
			{
				long num = timestamp - _lastReadTime[i];
				long num2 = Stopwatch.Frequency;
				if (i != (int)playerIndex)
				{
					num2 /= 4;
				}
				if (num >= 0 && num <= num2)
				{
					return true;
				}
			}
		}
		return false;
	}

	private static void ResetThrottleState(PlayerIndex playerIndex, ErrorCodes result)
	{
		if (playerIndex >= PlayerIndex.One && playerIndex <= PlayerIndex.Four)
		{
			if (result == ErrorCodes.NotConnected)
			{
				_disconnected[(int)playerIndex] = true;
				_lastReadTime[(int)playerIndex] = Stopwatch.GetTimestamp();
			}
			else
			{
				_disconnected[(int)playerIndex] = false;
			}
		}
	}
}
