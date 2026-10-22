using System;
using System.Runtime.CompilerServices;

namespace Microsoft.Xna.Framework.Input;

public static class Mouse
{
	internal static MouseMessageHooker mouseMessageHooker = new MouseMessageHooker();

	private unsafe static HWND__* hHookedHandle = null;

	public unsafe static IntPtr WindowHandle
	{
		get
		{
			return new IntPtr(hHookedHandle);
		}
		set
		{
			hHookedHandle = (HWND__*)value.ToPointer();
			mouseMessageHooker.WindowHandle = value;
		}
	}

	public unsafe static MouseState GetState()
	{
		MouseState result = default(MouseState);
		System.Runtime.CompilerServices.Unsafe.SkipInit(out tagPOINT tagPOINT);
		global::<Module>.GetCursorPos(&tagPOINT);
		if (hHookedHandle != null)
		{
			global::<Module>.ScreenToClient(hHookedHandle, &tagPOINT);
		}
		result.x = *(int*)(&tagPOINT);
		result.y = System.Runtime.CompilerServices.Unsafe.As<tagPOINT, int>(ref System.Runtime.CompilerServices.Unsafe.AddByteOffset(ref tagPOINT, 4));
		ButtonState leftButton = (ButtonState)((ushort)global::<Module>.GetAsyncKeyState(1) >>> 15);
		result.leftButton = leftButton;
		ButtonState middleButton = (ButtonState)((ushort)global::<Module>.GetAsyncKeyState(4) >>> 15);
		result.middleButton = middleButton;
		ButtonState rightButton = (ButtonState)((ushort)global::<Module>.GetAsyncKeyState(2) >>> 15);
		result.rightButton = rightButton;
		ButtonState xb = (ButtonState)((ushort)global::<Module>.GetAsyncKeyState(5) >>> 15);
		result.xb1 = xb;
		ButtonState xb2 = (ButtonState)((ushort)global::<Module>.GetAsyncKeyState(6) >>> 15);
		result.xb2 = xb2;
		result.wheel = MouseMessageHooker.CurrentWheel;
		return result;
	}

	public unsafe static void SetPosition(int x, int y)
	{
		System.Runtime.CompilerServices.Unsafe.SkipInit(out tagPOINT tagPOINT);
		*(int*)(&tagPOINT) = x;
		System.Runtime.CompilerServices.Unsafe.As<tagPOINT, int>(ref System.Runtime.CompilerServices.Unsafe.AddByteOffset(ref tagPOINT, 4)) = y;
		if (hHookedHandle != null)
		{
			global::<Module>.ClientToScreen(hHookedHandle, &tagPOINT);
		}
		global::<Module>.SetCursorPos(*(int*)(&tagPOINT), System.Runtime.CompilerServices.Unsafe.As<tagPOINT, int>(ref System.Runtime.CompilerServices.Unsafe.AddByteOffset(ref tagPOINT, 4)));
	}
}
