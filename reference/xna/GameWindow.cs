using System;

namespace Microsoft.Xna.Framework;

public abstract class GameWindow
{
	internal static readonly int DefaultClientWidth = 800;

	internal static readonly int DefaultClientHeight = 600;

	private string title;

	public string Title
	{
		get
		{
			return title;
		}
		set
		{
			if (value == null)
			{
				throw new ArgumentNullException("value", Resources.TitleCannotBeNull);
			}
			if (title != value)
			{
				title = value;
				SetTitle(title);
			}
		}
	}

	public abstract IntPtr Handle { get; }

	public abstract bool AllowUserResizing { get; set; }

	internal abstract bool IsMouseVisible { get; set; }

	internal abstract bool IsMinimized { get; }

	public abstract Rectangle ClientBounds { get; }

	public abstract string ScreenDeviceName { get; }

	public abstract DisplayOrientation CurrentOrientation { get; }

	internal event EventHandler<EventArgs> Activated;

	internal event EventHandler<EventArgs> Deactivated;

	internal event EventHandler<EventArgs> Paint;

	public event EventHandler<EventArgs> ScreenDeviceNameChanged;

	public event EventHandler<EventArgs> ClientSizeChanged;

	public event EventHandler<EventArgs> OrientationChanged;

	internal GameWindow()
	{
		title = string.Empty;
	}

	public abstract void BeginScreenDeviceChange(bool willBeFullScreen);

	public abstract void EndScreenDeviceChange(string screenDeviceName, int clientWidth, int clientHeight);

	public void EndScreenDeviceChange(string screenDeviceName)
	{
		EndScreenDeviceChange(screenDeviceName, ClientBounds.Width, ClientBounds.Height);
	}

	protected abstract void SetTitle(string title);

	protected internal abstract void SetSupportedOrientations(DisplayOrientation orientations);

	protected void OnActivated()
	{
		if (this.Activated != null)
		{
			this.Activated(this, EventArgs.Empty);
		}
	}

	protected void OnDeactivated()
	{
		if (this.Deactivated != null)
		{
			this.Deactivated(this, EventArgs.Empty);
		}
	}

	protected void OnPaint()
	{
		if (this.Paint != null)
		{
			this.Paint(this, EventArgs.Empty);
		}
	}

	protected void OnScreenDeviceNameChanged()
	{
		if (this.ScreenDeviceNameChanged != null)
		{
			this.ScreenDeviceNameChanged(this, EventArgs.Empty);
		}
	}

	protected void OnClientSizeChanged()
	{
		if (this.ClientSizeChanged != null)
		{
			this.ClientSizeChanged(this, EventArgs.Empty);
		}
	}

	protected void OnOrientationChanged()
	{
		if (this.OrientationChanged != null)
		{
			this.OrientationChanged(this, EventArgs.Empty);
		}
	}
}
