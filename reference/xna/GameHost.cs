using System;

namespace Microsoft.Xna.Framework;

internal abstract class GameHost
{
	internal abstract GameWindow Window { get; }

	internal event EventHandler<EventArgs> Suspend;

	internal event EventHandler<EventArgs> Resume;

	internal event EventHandler<EventArgs> Activated;

	internal event EventHandler<EventArgs> Deactivated;

	internal event EventHandler<EventArgs> Idle;

	internal event EventHandler<EventArgs> Exiting;

	internal abstract void Run();

	internal abstract void RunOneFrame();

	internal abstract void StartGameLoop();

	internal abstract void Exit();

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

	protected void OnIdle()
	{
		if (this.Idle != null)
		{
			this.Idle(this, EventArgs.Empty);
		}
	}

	protected void OnExiting()
	{
		if (this.Exiting != null)
		{
			this.Exiting(this, EventArgs.Empty);
		}
	}

	internal virtual bool ShowMissingRequirementMessage(Exception exception)
	{
		return false;
	}
}
