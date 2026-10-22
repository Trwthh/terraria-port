namespace Microsoft.Xna.Framework.Input;

internal struct XINPUT_CAPABILITIES
{
	public byte Type;

	public byte SubType;

	public ushort Flags;

	public XINPUT_GAMEPAD GamePad;

	public XINPUT_VIBRATION Vibration;
}
