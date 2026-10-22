using System;

namespace Microsoft.Xna.Framework.Input;

internal static class GamePadDeadZoneUtils
{
	private const int LeftStickDeadZoneSize = 7849;

	private const int RightStickDeadZoneSize = 8689;

	private const int TriggerDeadZoneSize = 30;

	internal static Vector2 ApplyLeftStickDeadZone(int x, int y, GamePadDeadZone deadZoneMode)
	{
		return ApplyStickDeadZone(x, y, deadZoneMode, 7849);
	}

	internal static Vector2 ApplyRightStickDeadZone(int x, int y, GamePadDeadZone deadZoneMode)
	{
		return ApplyStickDeadZone(x, y, deadZoneMode, 8689);
	}

	private static Vector2 ApplyStickDeadZone(int x, int y, GamePadDeadZone deadZoneMode, int deadZoneSize)
	{
		Vector2 result = default(Vector2);
		switch (deadZoneMode)
		{
		case GamePadDeadZone.IndependentAxes:
			result.X = ApplyLinearDeadZone(x, 32767f, deadZoneSize);
			result.Y = ApplyLinearDeadZone(y, 32767f, deadZoneSize);
			break;
		case GamePadDeadZone.Circular:
		{
			float num = (float)Math.Sqrt(x * x + y * y);
			float num2 = ApplyLinearDeadZone(num, 32767f, deadZoneSize);
			float num3 = ((num2 > 0f) ? (num2 / num) : 0f);
			result.X = MathHelper.Clamp((float)x * num3, -1f, 1f);
			result.Y = MathHelper.Clamp((float)y * num3, -1f, 1f);
			break;
		}
		default:
			result.X = ApplyLinearDeadZone(x, 32767f, 0f);
			result.Y = ApplyLinearDeadZone(y, 32767f, 0f);
			break;
		}
		return result;
	}

	internal static float ApplyTriggerDeadZone(int value, GamePadDeadZone deadZoneMode)
	{
		if (deadZoneMode == GamePadDeadZone.None)
		{
			return ApplyLinearDeadZone(value, 255f, 0f);
		}
		return ApplyLinearDeadZone(value, 255f, 30f);
	}

	private static float ApplyLinearDeadZone(float value, float maxValue, float deadZoneSize)
	{
		if (value < 0f - deadZoneSize)
		{
			value += deadZoneSize;
		}
		else
		{
			if (!(value > deadZoneSize))
			{
				return 0f;
			}
			value -= deadZoneSize;
		}
		float value2 = value / (maxValue - deadZoneSize);
		return MathHelper.Clamp(value2, -1f, 1f);
	}
}
