using System;
using System.Drawing;
using System.Windows.Forms;
using Microsoft.Xna.Framework.GamerServices;

namespace Microsoft.Xna.Framework;

internal class WindowsGameForm : Form
{
	private bool freezeOurEvents;

	private Screen screen;

	private FormWindowState resizeWindowState;

	private Size startResizeSize = Size.Empty;

	private bool hidMouse;

	private bool isMouseVisible;

	private bool allowUserResizing;

	private bool userResizing;

	private bool? deviceChangeWillBeFullScreen;

	private bool deviceChangeChangedVisible;

	private bool oldVisible;

	private Size oldClientSize;

	private bool centerScreen = true;

	private bool isFullScreenMaximized;

	private FormBorderStyle savedFormBorderStyle;

	private FormWindowState savedWindowState;

	private System.Drawing.Rectangle savedBounds;

	private System.Drawing.Rectangle savedRestoreBounds;

	private bool firstPaint = true;

	internal bool AllowUserResizing
	{
		get
		{
			return allowUserResizing;
		}
		set
		{
			if (allowUserResizing != value)
			{
				allowUserResizing = value;
				UpdateBorderStyle();
			}
		}
	}

	internal bool IsMouseVisible
	{
		get
		{
			return isMouseVisible;
		}
		set
		{
			if (isMouseVisible == value)
			{
				return;
			}
			isMouseVisible = value;
			if (isMouseVisible)
			{
				if (hidMouse)
				{
					Cursor.Show();
					hidMouse = false;
				}
			}
			else if (!hidMouse)
			{
				Cursor.Hide();
				hidMouse = true;
			}
		}
	}

	internal Screen DeviceScreen => screen;

	internal Rectangle ClientBounds
	{
		get
		{
			System.Drawing.Point point = PointToScreen(System.Drawing.Point.Empty);
			return new Rectangle(point.X, point.Y, base.ClientSize.Width, base.ClientSize.Height);
		}
	}

	internal bool IsMinimized
	{
		get
		{
			if (base.ClientSize.Width != 0)
			{
				return base.ClientSize.Height == 0;
			}
			return true;
		}
	}

	internal event EventHandler Suspend;

	internal event EventHandler Resume;

	internal event EventHandler ScreenChanged;

	internal event EventHandler UserResized;

	internal event EventHandler ApplicationActivated;

	internal event EventHandler ApplicationDeactivated;

	public WindowsGameForm()
	{
		SuspendLayout();
		base.AutoScaleDimensions = new SizeF(6f, 13f);
		base.AutoScaleMode = AutoScaleMode.Font;
		base.CausesValidation = false;
		base.ClientSize = new Size(292, 266);
		base.Name = "GameForm";
		Text = "GameForm";
		base.ResizeBegin += Form_ResizeBegin;
		base.ClientSizeChanged += Form_ClientSizeChanged;
		base.Resize += Form_Resize;
		base.LocationChanged += Form_LocationChanged;
		base.ResizeEnd += Form_ResizeEnd;
		base.MouseEnter += Form_MouseEnter;
		base.MouseLeave += Form_MouseLeave;
		ResumeLayout(performLayout: false);
		try
		{
			freezeOurEvents = true;
			resizeWindowState = base.WindowState;
			screen = WindowsGameWindow.ScreenFromHandle(base.Handle);
			SetStyle(ControlStyles.Opaque | ControlStyles.AllPaintingInWmPaint, value: false);
			base.ClientSize = new Size(GameWindow.DefaultClientWidth, GameWindow.DefaultClientHeight);
			UpdateBorderStyle();
		}
		finally
		{
			freezeOurEvents = false;
		}
	}

	private void UpdateBorderStyle()
	{
		if (!allowUserResizing)
		{
			base.MaximizeBox = false;
			if (!isFullScreenMaximized)
			{
				base.FormBorderStyle = FormBorderStyle.FixedSingle;
			}
		}
		else
		{
			base.MaximizeBox = true;
			if (!isFullScreenMaximized)
			{
				base.FormBorderStyle = FormBorderStyle.Sizable;
			}
		}
	}

	private void UpdateScreen()
	{
		if (freezeOurEvents)
		{
			return;
		}
		Screen obj = Screen.FromHandle(base.Handle);
		if (screen == null || !screen.Equals(obj))
		{
			screen = obj;
			if (screen != null)
			{
				OnScreenChanged();
			}
		}
	}

	private void OnSuspend()
	{
		if (this.Suspend != null)
		{
			this.Suspend(this, EventArgs.Empty);
		}
	}

	private void OnResume()
	{
		if (this.Resume != null)
		{
			this.Resume(this, EventArgs.Empty);
		}
	}

	private void OnScreenChanged()
	{
		if (this.ScreenChanged != null)
		{
			this.ScreenChanged(this, EventArgs.Empty);
		}
	}

	private void OnUserResized(bool forceEvent)
	{
		if ((!freezeOurEvents || forceEvent) && this.UserResized != null)
		{
			this.UserResized(this, EventArgs.Empty);
		}
	}

	private void OnActivateApp(bool active)
	{
		if (active)
		{
			firstPaint = true;
			freezeOurEvents = false;
			if (isFullScreenMaximized)
			{
				base.TopMost = true;
			}
			if (this.ApplicationActivated != null)
			{
				this.ApplicationActivated(this, EventArgs.Empty);
			}
		}
		else
		{
			if (this.ApplicationDeactivated != null)
			{
				this.ApplicationDeactivated(this, EventArgs.Empty);
			}
			freezeOurEvents = true;
		}
	}

	protected override void OnPaintBackground(PaintEventArgs e)
	{
		if (firstPaint)
		{
			base.OnPaintBackground(e);
			firstPaint = false;
		}
	}

	protected override void WndProc(ref Message m)
	{
		if (m.Msg == 28)
		{
			bool active = ((m.WParam != IntPtr.Zero) ? true : false);
			OnActivateApp(active);
		}
		base.WndProc(ref m);
	}

	protected override bool ProcessDialogKey(Keys keyData)
	{
		Keys keys = keyData & Keys.KeyCode;
		Keys keys2 = keyData & Keys.Alt;
		if (keys2 == Keys.Alt && (keys == Keys.F4 || keys == Keys.None))
		{
			return base.ProcessDialogKey(keyData);
		}
		if (GamerServicesDispatcher.IsInitialized && (keys == Keys.Home || Guide.IsVisible))
		{
			return base.ProcessDialogKey(keyData);
		}
		return true;
	}

	protected override void Dispose(bool disposing)
	{
		base.Dispose(disposing);
	}

	private void Form_ResizeBegin(object sender, EventArgs e)
	{
		startResizeSize = base.ClientSize;
		userResizing = true;
		OnSuspend();
	}

	private void Form_Resize(object sender, EventArgs e)
	{
		if (resizeWindowState != base.WindowState)
		{
			resizeWindowState = base.WindowState;
			firstPaint = true;
			OnUserResized(forceEvent: false);
			Invalidate();
		}
		if (userResizing && base.ClientSize != startResizeSize)
		{
			Invalidate();
		}
	}

	private void Form_ResizeEnd(object sender, EventArgs e)
	{
		userResizing = false;
		if (base.ClientSize != startResizeSize)
		{
			centerScreen = false;
			OnUserResized(forceEvent: false);
		}
		firstPaint = true;
		OnResume();
	}

	private void Form_ClientSizeChanged(object sender, EventArgs e)
	{
		UpdateScreen();
	}

	private void Form_LocationChanged(object sender, EventArgs e)
	{
		if (userResizing)
		{
			centerScreen = false;
		}
		UpdateScreen();
	}

	private void Form_MouseEnter(object sender, EventArgs e)
	{
		if (!isMouseVisible && !hidMouse)
		{
			Cursor.Hide();
			hidMouse = true;
		}
	}

	private void Form_MouseLeave(object sender, EventArgs e)
	{
		if (hidMouse)
		{
			Cursor.Show();
			hidMouse = false;
		}
	}

	internal void BeginScreenDeviceChange(bool willBeFullScreen)
	{
		oldClientSize = base.ClientSize;
		if (willBeFullScreen && !isFullScreenMaximized)
		{
			savedFormBorderStyle = base.FormBorderStyle;
			savedWindowState = base.WindowState;
			savedBounds = base.Bounds;
			if (base.WindowState == FormWindowState.Maximized)
			{
				savedRestoreBounds = base.RestoreBounds;
			}
		}
		if (willBeFullScreen != isFullScreenMaximized)
		{
			deviceChangeChangedVisible = true;
			oldVisible = base.Visible;
			base.Visible = false;
		}
		else
		{
			deviceChangeChangedVisible = false;
		}
		if (!willBeFullScreen && isFullScreenMaximized)
		{
			base.TopMost = false;
			base.FormBorderStyle = savedFormBorderStyle;
			if (savedWindowState == FormWindowState.Maximized)
			{
				SetBoundsCore(screen.Bounds.X, screen.Bounds.Y, savedRestoreBounds.Width, savedRestoreBounds.Height, BoundsSpecified.Size);
			}
			else
			{
				SetBoundsCore(screen.Bounds.X, screen.Bounds.Y, savedBounds.Width, savedBounds.Height, BoundsSpecified.Size);
			}
		}
		if (willBeFullScreen != isFullScreenMaximized)
		{
			SendToBack();
		}
		deviceChangeWillBeFullScreen = willBeFullScreen;
	}

	internal void EndScreenDeviceChange(string screenDeviceName, int clientWidth, int clientHeight)
	{
		if (!deviceChangeWillBeFullScreen.HasValue)
		{
			throw new InvalidOperationException(Resources.MustCallBeginDeviceChange);
		}
		bool flag = false;
		if (deviceChangeWillBeFullScreen.Value)
		{
			Screen screen = WindowsGameWindow.ScreenFromDeviceName(screenDeviceName);
			System.Drawing.Rectangle bounds = Screen.GetBounds(new System.Drawing.Point(screen.Bounds.X, screen.Bounds.Y));
			if (!isFullScreenMaximized)
			{
				flag = true;
				base.TopMost = true;
				base.FormBorderStyle = FormBorderStyle.None;
				base.WindowState = FormWindowState.Normal;
				BringToFront();
			}
			base.Location = new System.Drawing.Point(bounds.X, bounds.Y);
			base.ClientSize = new Size(bounds.Width, bounds.Height);
			isFullScreenMaximized = true;
		}
		else
		{
			if (isFullScreenMaximized)
			{
				flag = true;
				BringToFront();
			}
			ResizeWindow(screenDeviceName, clientWidth, clientHeight, centerScreen);
		}
		if (deviceChangeChangedVisible)
		{
			base.Visible = oldVisible;
		}
		if (flag && oldClientSize != base.ClientSize)
		{
			OnUserResized(forceEvent: true);
		}
		deviceChangeWillBeFullScreen = null;
	}

	private void ResizeWindow(string screenDeviceName, int clientWidth, int clientHeight, bool center)
	{
		Screen screen = WindowsGameWindow.ScreenFromDeviceName(screenDeviceName);
		System.Drawing.Rectangle bounds = Screen.GetBounds(new System.Drawing.Point(screen.Bounds.X, screen.Bounds.Y));
		int num;
		int num2;
		if (screenDeviceName != WindowsGameWindow.DeviceNameFromScreen(DeviceScreen))
		{
			num = bounds.X;
			num2 = bounds.Y;
		}
		else
		{
			num = this.screen.Bounds.X;
			num2 = this.screen.Bounds.Y;
		}
		if (isFullScreenMaximized)
		{
			Size size = SizeFromClientSize(new Size(clientWidth, clientHeight));
			if (savedWindowState == FormWindowState.Maximized)
			{
				int num3 = savedRestoreBounds.X - this.screen.Bounds.X + num;
				int num4 = savedRestoreBounds.Y - this.screen.Bounds.Y + num2;
				SetBoundsCore(num3, num4, savedRestoreBounds.Width, savedRestoreBounds.Height, BoundsSpecified.All);
			}
			else if (center)
			{
				int num5 = num + bounds.Width / 2 - size.Width / 2;
				int num6 = num2 + bounds.Height / 2 - size.Height / 2;
				SetBoundsCore(num5, num6, size.Width, size.Height, BoundsSpecified.All);
			}
			else
			{
				int num7 = savedBounds.X - this.screen.Bounds.X + num;
				int num8 = savedBounds.Y - this.screen.Bounds.Y + num2;
				SetBoundsCore(num7, num8, size.Width, size.Height, BoundsSpecified.All);
			}
			base.WindowState = savedWindowState;
			isFullScreenMaximized = false;
		}
		else if (base.WindowState == FormWindowState.Normal)
		{
			int num9;
			int num10;
			if (center)
			{
				Size size2 = SizeFromClientSize(new Size(clientWidth, clientHeight));
				num9 = num + bounds.Width / 2 - size2.Width / 2;
				num10 = num2 + bounds.Height / 2 - size2.Height / 2;
			}
			else
			{
				num9 = num + base.Bounds.X - this.screen.Bounds.X;
				num10 = num2 + base.Bounds.Y - this.screen.Bounds.Y;
			}
			if (num9 != base.Location.X || num10 != base.Location.Y)
			{
				base.Location = new System.Drawing.Point(num9, num10);
			}
			if (base.ClientSize.Width != clientWidth || base.ClientSize.Height != clientHeight)
			{
				base.ClientSize = new Size(clientWidth, clientHeight);
			}
		}
	}
}
