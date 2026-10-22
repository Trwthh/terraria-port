using System;
using System.Globalization;
using System.Runtime.CompilerServices;
using <CppImplementationDetails>;

namespace Microsoft.Xna.Framework.Input;

public static class Keyboard
{
	internal static bool DisableHomeKey;

	public static KeyboardState GetState()
	{
		return GetState((PlayerIndex)255);
	}

	public unsafe static KeyboardState GetState(PlayerIndex playerIndex)
	{
		System.Runtime.CompilerServices.Unsafe.SkipInit(out $ArrayType$$$BY0BAA@E $ArrayType$$$BY0BAA@E);
		if (global::<Module>.GetKeyboardState((byte*)(&$ArrayType$$$BY0BAA@E)) == 0)
		{
			throw new InvalidOperationException(string.Format(args: new object[1] { (int)global::<Module>.GetLastError() }, provider: CultureInfo.CurrentCulture, format: FrameworkResources.CouldNotReadKeyboard));
		}
		if (DisableHomeKey)
		{
			System.Runtime.CompilerServices.Unsafe.As<$ArrayType$$$BY0BAA@E, sbyte>(ref System.Runtime.CompilerServices.Unsafe.AddByteOffset(ref $ArrayType$$$BY0BAA@E, 36)) = 0;
		}
		KeyboardState result = default(KeyboardState);
		int num = 0;
		do
		{
			if ((byte)(*(byte*)((ref *(?*)num) + (ref *(?*)(&$ArrayType$$$BY0BAA@E))) & 0x80) != 0)
			{
				result.AddPressedKey(num);
			}
			num++;
		}
		while (num < 256);
		return result;
	}
}
