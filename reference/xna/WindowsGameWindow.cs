using System;
using System.Collections;
using System.ComponentModel;
using System.Drawing;
using System.IO;
using System.Reflection;
using System.Resources;
using System.Windows.Forms;
using Microsoft.Xna.Framework.Graphics;

namespace Microsoft.Xna.Framework;

internal class WindowsGameWindow : GameWindow
{
	private WindowsGameForm mainForm;

	private bool isMouseVisible;

	private bool isGuideVisible;

	private bool inDeviceTransition;

	private Exception pendingException;

	public override IntPtr Handle
	{
		get
		{
			if (mainForm != null)
			{
				return mainForm.Handle;
			}
			return IntPtr.Zero;
		}
	}

	public override bool AllowUserResizing
	{
		get
		{
			if (mainForm != null)
			{
				return mainForm.AllowUserResizing;
			}
			return false;
		}
		set
		{
			if (mainForm != null)
			{
				mainForm.AllowUserResizing = value;
			}
		}
	}

	internal override bool IsMouseVisible
	{
		get
		{
			return isMouseVisible;
		}
		set
		{
			isMouseVisible = value;
			if (mainForm != null)
			{
				mainForm.IsMouseVisible = isMouseVisible || isGuideVisible;
			}
		}
	}

	internal bool IsGuideVisible
	{
		set
		{
			if (value != isGuideVisible)
			{
				isGuideVisible = value;
				if (mainForm != null)
				{
					mainForm.IsMouseVisible = isMouseVisible || isGuideVisible;
				}
			}
		}
	}

	public override Rectangle ClientBounds => mainForm.ClientBounds;

	public override DisplayOrientation CurrentOrientation => DisplayOrientation.Default;

	public override string ScreenDeviceName
	{
		get
		{
			if (mainForm == null)
			{
				return string.Empty;
			}
			if (mainForm.DeviceScreen == null)
			{
				return string.Empty;
			}
			return DeviceNameFromScreen(mainForm.DeviceScreen);
		}
	}

	internal override bool IsMinimized
	{
		get
		{
			if (mainForm == null)
			{
				return false;
			}
			return mainForm.IsMinimized;
		}
	}

	internal Form Form => mainForm;

	internal event EventHandler<EventArgs> Suspend;

	internal event EventHandler<EventArgs> Resume;

	public WindowsGameWindow()
	{
		mainForm = new WindowsGameForm();
		Icon defaultIcon = GetDefaultIcon();
		if (defaultIcon != null)
		{
			mainForm.Icon = defaultIcon;
		}
		base.Title = GetDefaultTitleName();
		mainForm.Suspend += mainForm_Suspend;
		mainForm.Resume += mainForm_Resume;
		mainForm.ScreenChanged += mainForm_ScreenChanged;
		mainForm.ApplicationActivated += mainForm_ApplicationActivated;
		mainForm.ApplicationDeactivated += mainForm_ApplicationDeactivated;
		mainForm.UserResized += mainForm_UserResized;
		mainForm.Closing += mainForm_Closing;
		mainForm.Paint += mainForm_Paint;
	}

	internal void Close()
	{
		if (mainForm != null)
		{
			mainForm.Close();
			mainForm = null;
		}
	}

	public override void BeginScreenDeviceChange(bool willBeFullScreen)
	{
		mainForm.BeginScreenDeviceChange(willBeFullScreen);
		inDeviceTransition = true;
	}

	public override void EndScreenDeviceChange(string screenDeviceName, int clientWidth, int clientHeight)
	{
		try
		{
			mainForm.EndScreenDeviceChange(screenDeviceName, clientWidth, clientHeight);
		}
		finally
		{
			inDeviceTransition = false;
		}
	}

	protected override void SetTitle(string title)
	{
		if (mainForm != null)
		{
			mainForm.Text = title;
		}
	}

	protected internal override void SetSupportedOrientations(DisplayOrientation orientations)
	{
	}

	protected void OnSuspend()
	{
		if (this.Suspend != null)
		{
			this.Suspend(this, EventArgs.Empty);
		}
	}

	protected void OnResume()
	{
		if (this.Resume != null)
		{
			this.Resume(this, EventArgs.Empty);
		}
	}

	private void mainForm_ApplicationActivated(object sender, EventArgs e)
	{
		OnActivated();
	}

	private void mainForm_ApplicationDeactivated(object sender, EventArgs e)
	{
		OnDeactivated();
	}

	private void mainForm_ScreenChanged(object sender, EventArgs e)
	{
		OnScreenDeviceNameChanged();
	}

	private void mainForm_UserResized(object sender, EventArgs e)
	{
		OnClientSizeChanged();
	}

	private void mainForm_Paint(object sender, PaintEventArgs e)
	{
		if (!inDeviceTransition)
		{
			try
			{
				OnPaint();
			}
			catch (Exception innerException)
			{
				pendingException = new InvalidOperationException(Resources.PreviousDrawThrew, innerException);
			}
		}
	}

	private void mainForm_Closing(object sender, CancelEventArgs e)
	{
		OnDeactivated();
	}

	private void mainForm_Resume(object sender, EventArgs e)
	{
		OnResume();
	}

	private void mainForm_Suspend(object sender, EventArgs e)
	{
		OnSuspend();
	}

	internal void Tick()
	{
		if (pendingException != null)
		{
			Exception ex = pendingException;
			pendingException = null;
			throw ex;
		}
	}

	internal static Screen ScreenFromHandle(IntPtr windowHandle)
	{
		int num = 0;
		Screen screen = null;
		NativeMethods.GetWindowRect(windowHandle, out var rect);
		System.Drawing.Rectangle rectangle = new System.Drawing.Rectangle(rect.Left, rect.Top, rect.Right - rect.Left, rect.Bottom - rect.Top);
		Screen[] allScreens = Screen.AllScreens;
		foreach (Screen screen2 in allScreens)
		{
			System.Drawing.Rectangle rectangle2 = rectangle;
			rectangle2.Intersect(screen2.Bounds);
			int num2 = rectangle2.Width * rectangle2.Height;
			if (num2 > num)
			{
				num = num2;
				screen = screen2;
			}
		}
		if (screen == null)
		{
			screen = Screen.AllScreens[0];
		}
		return screen;
	}

	internal static string DeviceNameFromScreen(Screen screen)
	{
		string result = screen.DeviceName;
		int num = screen.DeviceName.IndexOf('\0');
		if (num != -1)
		{
			result = screen.DeviceName.Substring(0, num);
		}
		return result;
	}

	internal static Screen ScreenFromDeviceName(string screenDeviceName)
	{
		if (string.IsNullOrEmpty(screenDeviceName))
		{
			throw new ArgumentException(Resources.NullOrEmptyScreenDeviceName);
		}
		Screen[] allScreens = Screen.AllScreens;
		foreach (Screen screen in allScreens)
		{
			if (DeviceNameFromScreen(screen) == screenDeviceName)
			{
				return screen;
			}
		}
		throw new ArgumentException(Resources.InvalidScreenDeviceName, "screenDeviceName");
	}

	internal static Screen ScreenFromAdapter(GraphicsAdapter adapter)
	{
		Screen[] allScreens = Screen.AllScreens;
		foreach (Screen screen in allScreens)
		{
			if (DeviceNameFromScreen(screen) == adapter.DeviceName)
			{
				return screen;
			}
		}
		throw new ArgumentException(Resources.InvalidScreenAdapter, "adapter");
	}

	private static string GetAssemblyTitle(Assembly assembly)
	{
		if (assembly == null)
		{
			return null;
		}
		AssemblyTitleAttribute[] array = (AssemblyTitleAttribute[])assembly.GetCustomAttributes(typeof(AssemblyTitleAttribute), inherit: true);
		if (array != null && array.Length > 0)
		{
			return array[0].Title;
		}
		return null;
	}

	private static string GetDefaultTitleName()
	{
		string assemblyTitle = GetAssemblyTitle(Assembly.GetEntryAssembly());
		if (!string.IsNullOrEmpty(assemblyTitle))
		{
			return assemblyTitle;
		}
		try
		{
			Uri uri = new Uri(Application.ExecutablePath);
			return Path.GetFileNameWithoutExtension(uri.LocalPath);
		}
		catch (ArgumentNullException)
		{
		}
		catch (UriFormatException)
		{
		}
		return Resources.DefaultTitleName;
	}

	private static Icon FindFirstIcon(Assembly assembly)
	{
		if (assembly == null)
		{
			return null;
		}
		string[] manifestResourceNames = assembly.GetManifestResourceNames();
		foreach (string name in manifestResourceNames)
		{
			try
			{
				Stream manifestResourceStream = assembly.GetManifestResourceStream(name);
				return new Icon(manifestResourceStream);
			}
			catch
			{
				try
				{
					ResourceReader resourceReader = new ResourceReader(assembly.GetManifestResourceStream(name));
					IDictionaryEnumerator enumerator = resourceReader.GetEnumerator();
					while (enumerator.MoveNext())
					{
						if (enumerator.Value is Icon result)
						{
							return result;
						}
					}
				}
				catch
				{
				}
			}
		}
		return null;
	}

	private static Icon GetDefaultIcon()
	{
		Assembly entryAssembly = Assembly.GetEntryAssembly();
		Icon icon;
		if (entryAssembly != null)
		{
			try
			{
				icon = Icon.ExtractAssociatedIcon(entryAssembly.Location);
				if (icon != null)
				{
					return icon;
				}
			}
			catch
			{
			}
		}
		icon = FindFirstIcon(entryAssembly);
		if (icon != null)
		{
			return icon;
		}
		return new Icon(typeof(Game), "Game.ico");
	}
}
