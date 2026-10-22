using System;

namespace Microsoft.Xna.Framework.Input;

public struct KeyboardState
{
	private static uint stateMask0;

	private static uint stateMask1;

	private static uint stateMask2;

	private static uint stateMask3;

	private static uint stateMask4;

	private static uint stateMask5;

	private static uint stateMask6;

	private static uint stateMask7;

	private uint currentState0;

	private uint currentState1;

	private uint currentState2;

	private uint currentState3;

	private uint currentState4;

	private uint currentState5;

	private uint currentState6;

	private uint currentState7;

	public KeyState this[Keys key]
	{
		get
		{
			uint num;
			switch ((int)key >> 5)
			{
			case 0:
				num = currentState0;
				break;
			case 1:
				num = currentState1;
				break;
			case 2:
				num = currentState2;
				break;
			case 3:
				num = currentState3;
				break;
			case 4:
				num = currentState4;
				break;
			case 5:
				num = currentState5;
				break;
			case 6:
				num = currentState6;
				break;
			case 7:
				num = currentState7;
				break;
			default:
				return KeyState.Up;
			}
			uint num2 = (uint)(1 << (int)(key & (Keys)31));
			if ((num & num2) == 0)
			{
				return KeyState.Up;
			}
			return KeyState.Down;
		}
	}

	static KeyboardState()
	{
		stateMask0 = uint.MaxValue;
		stateMask1 = uint.MaxValue;
		stateMask2 = uint.MaxValue;
		stateMask3 = uint.MaxValue;
		stateMask4 = uint.MaxValue;
		stateMask5 = uint.MaxValue;
		stateMask6 = uint.MaxValue;
		stateMask7 = uint.MaxValue;
		KeyboardState keyboardState = default(KeyboardState);
		foreach (int value in Enum.GetValues(typeof(Keys)))
		{
			keyboardState.AddPressedKey(value);
		}
		stateMask0 = keyboardState.currentState0;
		stateMask1 = keyboardState.currentState1;
		stateMask2 = keyboardState.currentState2;
		stateMask3 = keyboardState.currentState3;
		stateMask4 = keyboardState.currentState4;
		stateMask5 = keyboardState.currentState5;
		stateMask6 = keyboardState.currentState6;
		stateMask7 = keyboardState.currentState7;
	}

	public KeyboardState(params Keys[] keys)
	{
		currentState0 = (currentState1 = (currentState2 = (currentState3 = (currentState4 = (currentState5 = (currentState6 = (currentState7 = 0u)))))));
		if (keys != null)
		{
			for (int i = 0; i < keys.Length; i++)
			{
				AddPressedKey((int)keys[i]);
			}
		}
	}

	internal void AddPressedKey(int key)
	{
		uint num = (uint)(1 << (key & 0x1F));
		switch (key >> 5)
		{
		case 0:
			currentState0 |= num & stateMask0;
			break;
		case 1:
			currentState1 |= num & stateMask1;
			break;
		case 2:
			currentState2 |= num & stateMask2;
			break;
		case 3:
			currentState3 |= num & stateMask3;
			break;
		case 4:
			currentState4 |= num & stateMask4;
			break;
		case 5:
			currentState5 |= num & stateMask5;
			break;
		case 6:
			currentState6 |= num & stateMask6;
			break;
		case 7:
			currentState7 |= num & stateMask7;
			break;
		}
	}

	internal void RemovePressedKey(int key)
	{
		uint num = (uint)(1 << (key & 0x1F));
		switch (key >> 5)
		{
		case 0:
			currentState0 &= ~(num & stateMask0);
			break;
		case 1:
			currentState1 &= ~(num & stateMask1);
			break;
		case 2:
			currentState2 &= ~(num & stateMask2);
			break;
		case 3:
			currentState3 &= ~(num & stateMask3);
			break;
		case 4:
			currentState4 &= ~(num & stateMask4);
			break;
		case 5:
			currentState5 &= ~(num & stateMask5);
			break;
		case 6:
			currentState6 &= ~(num & stateMask6);
			break;
		case 7:
			currentState7 &= ~(num & stateMask7);
			break;
		}
	}

	public bool IsKeyDown(Keys key)
	{
		return this[key] == KeyState.Down;
	}

	public bool IsKeyUp(Keys key)
	{
		return this[key] == KeyState.Up;
	}

	public Keys[] GetPressedKeys()
	{
		int index = 0;
		CheckPressedKeys(currentState0, 0, null, ref index);
		CheckPressedKeys(currentState1, 0, null, ref index);
		CheckPressedKeys(currentState2, 0, null, ref index);
		CheckPressedKeys(currentState3, 0, null, ref index);
		CheckPressedKeys(currentState4, 0, null, ref index);
		CheckPressedKeys(currentState5, 0, null, ref index);
		CheckPressedKeys(currentState6, 0, null, ref index);
		CheckPressedKeys(currentState7, 0, null, ref index);
		Keys[] array = new Keys[index];
		if (index > 0)
		{
			int index2 = 0;
			CheckPressedKeys(currentState0, 0, array, ref index2);
			CheckPressedKeys(currentState1, 1, array, ref index2);
			CheckPressedKeys(currentState2, 2, array, ref index2);
			CheckPressedKeys(currentState3, 3, array, ref index2);
			CheckPressedKeys(currentState4, 4, array, ref index2);
			CheckPressedKeys(currentState5, 5, array, ref index2);
			CheckPressedKeys(currentState6, 6, array, ref index2);
			CheckPressedKeys(currentState7, 7, array, ref index2);
		}
		return array;
	}

	private static void CheckPressedKeys(uint packedState, int packedOffset, Keys[] pressedKeys, ref int index)
	{
		if (packedState == 0)
		{
			return;
		}
		for (int i = 0; i < 32; i++)
		{
			if ((packedState & (1 << i)) != 0)
			{
				if (pressedKeys != null)
				{
					pressedKeys[index] = (Keys)(packedOffset * 32 + i);
				}
				index++;
			}
		}
	}

	public override int GetHashCode()
	{
		return currentState0.GetHashCode() ^ currentState1.GetHashCode() ^ currentState2.GetHashCode() ^ currentState3.GetHashCode() ^ currentState4.GetHashCode() ^ currentState5.GetHashCode() ^ currentState6.GetHashCode() ^ currentState7.GetHashCode();
	}

	public override bool Equals(object obj)
	{
		if (obj is KeyboardState)
		{
			return this == (KeyboardState)obj;
		}
		return false;
	}

	public static bool operator ==(KeyboardState a, KeyboardState b)
	{
		if (a.currentState0 == b.currentState0 && a.currentState1 == b.currentState1 && a.currentState2 == b.currentState2 && a.currentState3 == b.currentState3 && a.currentState4 == b.currentState4 && a.currentState5 == b.currentState5 && a.currentState6 == b.currentState6)
		{
			return a.currentState7 == b.currentState7;
		}
		return false;
	}

	public static bool operator !=(KeyboardState a, KeyboardState b)
	{
		return !(a == b);
	}
}
