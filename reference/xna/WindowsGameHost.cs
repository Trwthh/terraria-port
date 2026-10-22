using System;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using Microsoft.Xna.Framework.Audio;
using Microsoft.Xna.Framework.GamerServices;
using Microsoft.Xna.Framework.Graphics;
using Microsoft.Xna.Framework.Input;
using Microsoft.Xna.Framework.Input.Touch;

namespace Microsoft.Xna.Framework;

internal class WindowsGameHost : GameHost
{
	private Game game;

	private WindowsGameWindow gameWindow;

	private bool doneRun;

	private bool exitRequested;

	internal override GameWindow Window => gameWindow;

	public WindowsGameHost(Game game)
	{
		this.game = game;
		LockThreadToProcessor();
		gameWindow = new WindowsGameWindow();
		Mouse.WindowHandle = gameWindow.Handle;
		TouchPanel.WindowHandle = gameWindow.Handle;
		gameWindow.IsMouseVisible = game.IsMouseVisible;
		gameWindow.Activated += GameWindowActivated;
		gameWindow.Deactivated += GameWindowDeactivated;
		gameWindow.Suspend += GameWindowSuspend;
		gameWindow.Resume += GameWindowResume;
	}

	private void GameWindowSuspend(object sender, EventArgs e)
	{
		OnSuspend();
	}

	private void GameWindowResume(object sender, EventArgs e)
	{
		OnResume();
	}

	private void GameWindowDeactivated(object sender, EventArgs e)
	{
		OnDeactivated();
	}

	private void GameWindowActivated(object sender, EventArgs e)
	{
		OnActivated();
	}

	private void ApplicationIdle(object sender, EventArgs e)
	{
		NativeMethods.Message msg;
		while (!NativeMethods.PeekMessage(out msg, IntPtr.Zero, 0u, 0u, 0u))
		{
			if (exitRequested)
			{
				gameWindow.Close();
			}
			else
			{
				RunOneFrame();
			}
		}
	}

	internal override void Run()
	{
		if (doneRun)
		{
			throw new InvalidOperationException(Resources.NoMultipleRuns);
		}
		try
		{
			Application.Idle += ApplicationIdle;
			Application.Run(gameWindow.Form);
		}
		finally
		{
			Application.Idle -= ApplicationIdle;
			doneRun = true;
			OnExiting();
		}
	}

	internal override void RunOneFrame()
	{
		gameWindow.Tick();
		OnIdle();
		if (GamerServicesDispatcher.IsInitialized)
		{
			gameWindow.IsGuideVisible = Guide.IsVisible;
		}
	}

	internal override void StartGameLoop()
	{
	}

	internal override void Exit()
	{
		exitRequested = true;
	}

	private void LockThreadToProcessor()
	{
		UIntPtr lpProcessAffinityMask = UIntPtr.Zero;
		UIntPtr lpSystemAffinityMask = UIntPtr.Zero;
		if (GetProcessAffinityMask(GetCurrentProcess(), out lpProcessAffinityMask, out lpSystemAffinityMask) && lpProcessAffinityMask != UIntPtr.Zero)
		{
			UIntPtr dwThreadAffinityMask = (UIntPtr)(lpProcessAffinityMask.ToUInt64() & (~lpProcessAffinityMask.ToUInt64() + 1));
			SetThreadAffinityMask(GetCurrentThread(), dwThreadAffinityMask);
		}
	}

	[DllImport("kernel32.dll")]
	private static extern IntPtr GetCurrentThread();

	[DllImport("kernel32.dll")]
	private static extern IntPtr GetCurrentProcess();

	[DllImport("kernel32.dll")]
	private static extern UIntPtr SetThreadAffinityMask(IntPtr hThread, UIntPtr dwThreadAffinityMask);

	[DllImport("kernel32.dll", SetLastError = true)]
	private static extern bool GetProcessAffinityMask(IntPtr hProcess, out UIntPtr lpProcessAffinityMask, out UIntPtr lpSystemAffinityMask);

	internal override bool ShowMissingRequirementMessage(Exception exception)
	{
		string text;
		if (exception is NoSuitableGraphicsDeviceException)
		{
			text = Resources.NoSuitableGraphicsDevice + "\n\n" + exception.Message;
		}
		else
		{
			if (!(exception is NoAudioHardwareException))
			{
				return base.ShowMissingRequirementMessage(exception);
			}
			text = Resources.NoAudioHardware;
		}
		MessageBox.Show(gameWindow.Form, text, gameWindow.Title, MessageBoxButtons.OK, System.Windows.Forms.MessageBoxIcon.Hand);
		return true;
	}
}
