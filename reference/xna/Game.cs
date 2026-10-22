using System;
using System.Collections.Generic;
using System.Threading;
using Microsoft.Xna.Framework.Audio;
using Microsoft.Xna.Framework.Content;
using Microsoft.Xna.Framework.GamerServices;
using Microsoft.Xna.Framework.Graphics;

namespace Microsoft.Xna.Framework;

public class Game : IDisposable
{
	private readonly TimeSpan maximumElapsedTime = TimeSpan.FromMilliseconds(500.0);

	private IGraphicsDeviceManager graphicsDeviceManager;

	private IGraphicsDeviceService graphicsDeviceService;

	private GameHost host;

	private bool isActive;

	private bool exitRequested;

	private TimeSpan inactiveSleepTime;

	private bool isMouseVisible;

	private bool inRun;

	private bool endRunRequired;

	private GameTime gameTime = new GameTime();

	private GameClock clock;

	private TimeSpan totalGameTime;

	private TimeSpan targetElapsedTime;

	private TimeSpan accumulatedElapsedGameTime;

	private TimeSpan lastFrameElapsedGameTime;

	private bool isFixedTimeStep = true;

	private bool drawRunningSlowly;

	private int updatesSinceRunningSlowly1 = int.MaxValue;

	private int updatesSinceRunningSlowly2 = int.MaxValue;

	private bool doneFirstUpdate;

	private bool doneFirstDraw;

	private bool forceElapsedTimeToZero;

	private bool suppressDraw;

	private GameComponentCollection gameComponents;

	private List<IUpdateable> updateableComponents = new List<IUpdateable>();

	private List<IUpdateable> currentlyUpdatingComponents = new List<IUpdateable>();

	private List<IDrawable> drawableComponents = new List<IDrawable>();

	private List<IDrawable> currentlyDrawingComponents = new List<IDrawable>();

	private List<IGameComponent> notYetInitialized = new List<IGameComponent>();

	private GameServiceContainer gameServices = new GameServiceContainer();

	private ContentManager content;

	private LaunchParameters launchParameters;

	public LaunchParameters LaunchParameters => launchParameters;

	public GameComponentCollection Components => gameComponents;

	public GameServiceContainer Services => gameServices;

	public TimeSpan InactiveSleepTime
	{
		get
		{
			return inactiveSleepTime;
		}
		set
		{
			if (value < TimeSpan.Zero)
			{
				throw new ArgumentOutOfRangeException("value", Resources.InactiveSleepTimeCannotBeZero);
			}
			inactiveSleepTime = value;
		}
	}

	public bool IsMouseVisible
	{
		get
		{
			return isMouseVisible;
		}
		set
		{
			isMouseVisible = value;
			if (Window != null)
			{
				Window.IsMouseVisible = value;
			}
		}
	}

	public TimeSpan TargetElapsedTime
	{
		get
		{
			return targetElapsedTime;
		}
		set
		{
			if (value <= TimeSpan.Zero)
			{
				throw new ArgumentOutOfRangeException("value", Resources.TargetElaspedCannotBeZero);
			}
			targetElapsedTime = value;
		}
	}

	public bool IsFixedTimeStep
	{
		get
		{
			return isFixedTimeStep;
		}
		set
		{
			isFixedTimeStep = value;
		}
	}

	public GameWindow Window
	{
		get
		{
			if (host != null)
			{
				return host.Window;
			}
			return null;
		}
	}

	public bool IsActive
	{
		get
		{
			bool flag = false;
			if (GamerServicesDispatcher.IsInitialized)
			{
				flag = Guide.IsVisible;
			}
			if (isActive)
			{
				return !flag;
			}
			return false;
		}
	}

	public GraphicsDevice GraphicsDevice
	{
		get
		{
			IGraphicsDeviceService graphicsDeviceService = this.graphicsDeviceService;
			if (graphicsDeviceService == null)
			{
				graphicsDeviceService = Services.GetService(typeof(IGraphicsDeviceService)) as IGraphicsDeviceService;
				if (graphicsDeviceService == null)
				{
					throw new InvalidOperationException(Resources.NoGraphicsDeviceService);
				}
			}
			return graphicsDeviceService.GraphicsDevice;
		}
	}

	public ContentManager Content
	{
		get
		{
			return content;
		}
		set
		{
			if (value == null)
			{
				throw new ArgumentNullException();
			}
			content = value;
		}
	}

	internal bool IsActiveIgnoringGuide => isActive;

	private bool ShouldExit => exitRequested;

	public event EventHandler<EventArgs> Activated;

	public event EventHandler<EventArgs> Deactivated;

	public event EventHandler<EventArgs> Exiting;

	public event EventHandler<EventArgs> Disposed;

	public Game()
	{
		FrameworkDispatcher.Update();
		EnsureHost();
		launchParameters = new LaunchParameters();
		gameComponents = new GameComponentCollection();
		gameComponents.ComponentAdded += GameComponentAdded;
		gameComponents.ComponentRemoved += GameComponentRemoved;
		content = new ContentManager(gameServices);
		host.Window.Paint += Paint;
		clock = new GameClock();
		totalGameTime = TimeSpan.Zero;
		accumulatedElapsedGameTime = TimeSpan.Zero;
		lastFrameElapsedGameTime = TimeSpan.Zero;
		targetElapsedTime = TimeSpan.FromTicks(166667L);
		inactiveSleepTime = TimeSpan.FromMilliseconds(20.0);
	}

	public void Run()
	{
		RunGame(useBlockingRun: true);
	}

	internal void StartGameLoop()
	{
		RunGame(useBlockingRun: false);
	}

	private void RunGame(bool useBlockingRun)
	{
		try
		{
			graphicsDeviceManager = Services.GetService(typeof(IGraphicsDeviceManager)) as IGraphicsDeviceManager;
			if (graphicsDeviceManager != null)
			{
				graphicsDeviceManager.CreateDevice();
			}
			Initialize();
			inRun = true;
			BeginRun();
			gameTime.ElapsedGameTime = TimeSpan.Zero;
			gameTime.TotalGameTime = totalGameTime;
			gameTime.IsRunningSlowly = false;
			Update(gameTime);
			doneFirstUpdate = true;
			if (useBlockingRun)
			{
				if (host != null)
				{
					host.Run();
				}
				EndRun();
			}
			else
			{
				if (host != null)
				{
					host.StartGameLoop();
				}
				endRunRequired = true;
			}
		}
		catch (NoSuitableGraphicsDeviceException exception)
		{
			if (!ShowMissingRequirementMessage(exception))
			{
				throw;
			}
		}
		catch (NoAudioHardwareException exception2)
		{
			if (!ShowMissingRequirementMessage(exception2))
			{
				throw;
			}
		}
		finally
		{
			if (!endRunRequired)
			{
				inRun = false;
			}
		}
	}

	public void RunOneFrame()
	{
		if (host != null)
		{
			host.RunOneFrame();
		}
	}

	public void Tick()
	{
		if (ShouldExit)
		{
			return;
		}
		if (!isActive)
		{
			Thread.Sleep((int)inactiveSleepTime.TotalMilliseconds);
		}
		clock.Step();
		bool flag = true;
		TimeSpan timeSpan = clock.ElapsedAdjustedTime;
		if (timeSpan < TimeSpan.Zero)
		{
			timeSpan = TimeSpan.Zero;
		}
		if (forceElapsedTimeToZero)
		{
			timeSpan = TimeSpan.Zero;
			forceElapsedTimeToZero = false;
		}
		if (timeSpan > maximumElapsedTime)
		{
			timeSpan = maximumElapsedTime;
		}
		if (isFixedTimeStep)
		{
			if (Math.Abs(timeSpan.Ticks - targetElapsedTime.Ticks) < targetElapsedTime.Ticks >> 6)
			{
				timeSpan = targetElapsedTime;
			}
			accumulatedElapsedGameTime += timeSpan;
			long num = accumulatedElapsedGameTime.Ticks / targetElapsedTime.Ticks;
			accumulatedElapsedGameTime = TimeSpan.FromTicks(accumulatedElapsedGameTime.Ticks % targetElapsedTime.Ticks);
			lastFrameElapsedGameTime = TimeSpan.Zero;
			if (num == 0)
			{
				return;
			}
			TimeSpan timeSpan2 = targetElapsedTime;
			if (num > 1)
			{
				updatesSinceRunningSlowly2 = updatesSinceRunningSlowly1;
				updatesSinceRunningSlowly1 = 0;
			}
			else
			{
				if (updatesSinceRunningSlowly1 < int.MaxValue)
				{
					updatesSinceRunningSlowly1++;
				}
				if (updatesSinceRunningSlowly2 < int.MaxValue)
				{
					updatesSinceRunningSlowly2++;
				}
			}
			drawRunningSlowly = updatesSinceRunningSlowly2 < 20;
			while (num > 0 && !ShouldExit)
			{
				num--;
				try
				{
					gameTime.ElapsedGameTime = timeSpan2;
					gameTime.TotalGameTime = totalGameTime;
					gameTime.IsRunningSlowly = drawRunningSlowly;
					Update(gameTime);
					flag &= suppressDraw;
					suppressDraw = false;
				}
				finally
				{
					lastFrameElapsedGameTime += timeSpan2;
					totalGameTime += timeSpan2;
				}
			}
		}
		else
		{
			TimeSpan timeSpan3 = timeSpan;
			drawRunningSlowly = false;
			updatesSinceRunningSlowly1 = int.MaxValue;
			updatesSinceRunningSlowly2 = int.MaxValue;
			if (!ShouldExit)
			{
				try
				{
					gameTime.ElapsedGameTime = (lastFrameElapsedGameTime = timeSpan3);
					gameTime.TotalGameTime = totalGameTime;
					gameTime.IsRunningSlowly = false;
					Update(gameTime);
					flag &= suppressDraw;
					suppressDraw = false;
				}
				finally
				{
					totalGameTime += timeSpan3;
				}
			}
		}
		if (!flag)
		{
			DrawFrame();
		}
	}

	public void SuppressDraw()
	{
		suppressDraw = true;
	}

	public void Exit()
	{
		exitRequested = true;
		host.Exit();
		if (inRun && endRunRequired)
		{
			EndRun();
			inRun = false;
		}
	}

	protected virtual void BeginRun()
	{
	}

	protected virtual void EndRun()
	{
	}

	protected virtual void Update(GameTime gameTime)
	{
		for (int i = 0; i < updateableComponents.Count; i++)
		{
			currentlyUpdatingComponents.Add(updateableComponents[i]);
		}
		for (int j = 0; j < currentlyUpdatingComponents.Count; j++)
		{
			IUpdateable updateable = currentlyUpdatingComponents[j];
			if (updateable.Enabled)
			{
				updateable.Update(gameTime);
			}
		}
		currentlyUpdatingComponents.Clear();
		FrameworkDispatcher.Update();
		doneFirstUpdate = true;
	}

	protected virtual bool BeginDraw()
	{
		if (graphicsDeviceManager != null && !graphicsDeviceManager.BeginDraw())
		{
			return false;
		}
		return true;
	}

	protected virtual void Draw(GameTime gameTime)
	{
		for (int i = 0; i < drawableComponents.Count; i++)
		{
			currentlyDrawingComponents.Add(drawableComponents[i]);
		}
		for (int j = 0; j < currentlyDrawingComponents.Count; j++)
		{
			IDrawable drawable = currentlyDrawingComponents[j];
			if (drawable.Visible)
			{
				drawable.Draw(gameTime);
			}
		}
		currentlyDrawingComponents.Clear();
	}

	protected virtual void EndDraw()
	{
		if (graphicsDeviceManager != null)
		{
			graphicsDeviceManager.EndDraw();
		}
	}

	private void Paint(object sender, EventArgs e)
	{
		if (doneFirstDraw)
		{
			DrawFrame();
		}
	}

	protected virtual void Initialize()
	{
		HookDeviceEvents();
		while (notYetInitialized.Count != 0)
		{
			notYetInitialized[0].Initialize();
			notYetInitialized.RemoveAt(0);
		}
		if (graphicsDeviceService != null && graphicsDeviceService.GraphicsDevice != null)
		{
			LoadContent();
		}
	}

	public void ResetElapsedTime()
	{
		forceElapsedTimeToZero = true;
		drawRunningSlowly = false;
		updatesSinceRunningSlowly1 = int.MaxValue;
		updatesSinceRunningSlowly2 = int.MaxValue;
	}

	private void DrawFrame()
	{
		try
		{
			if (!ShouldExit && doneFirstUpdate && !Window.IsMinimized && BeginDraw())
			{
				gameTime.TotalGameTime = totalGameTime;
				gameTime.ElapsedGameTime = lastFrameElapsedGameTime;
				gameTime.IsRunningSlowly = drawRunningSlowly;
				Draw(gameTime);
				EndDraw();
				doneFirstDraw = true;
			}
		}
		finally
		{
			lastFrameElapsedGameTime = TimeSpan.Zero;
		}
	}

	private void GameComponentRemoved(object sender, GameComponentCollectionEventArgs e)
	{
		if (!inRun)
		{
			notYetInitialized.Remove(e.GameComponent);
		}
		if (e.GameComponent is IUpdateable updateable)
		{
			updateableComponents.Remove(updateable);
			updateable.UpdateOrderChanged -= UpdateableUpdateOrderChanged;
		}
		if (e.GameComponent is IDrawable drawable)
		{
			drawableComponents.Remove(drawable);
			drawable.DrawOrderChanged -= DrawableDrawOrderChanged;
		}
	}

	private void GameComponentAdded(object sender, GameComponentCollectionEventArgs e)
	{
		if (inRun)
		{
			e.GameComponent.Initialize();
		}
		else
		{
			notYetInitialized.Add(e.GameComponent);
		}
		if (e.GameComponent is IUpdateable updateable)
		{
			int num = updateableComponents.BinarySearch(updateable, UpdateOrderComparer.Default);
			if (num < 0)
			{
				for (num = ~num; num < updateableComponents.Count && updateableComponents[num].UpdateOrder == updateable.UpdateOrder; num++)
				{
				}
				updateableComponents.Insert(num, updateable);
				updateable.UpdateOrderChanged += UpdateableUpdateOrderChanged;
			}
		}
		if (!(e.GameComponent is IDrawable drawable))
		{
			return;
		}
		int num2 = drawableComponents.BinarySearch(drawable, DrawOrderComparer.Default);
		if (num2 < 0)
		{
			for (num2 = ~num2; num2 < drawableComponents.Count && drawableComponents[num2].DrawOrder == drawable.DrawOrder; num2++)
			{
			}
			drawableComponents.Insert(num2, drawable);
			drawable.DrawOrderChanged += DrawableDrawOrderChanged;
		}
	}

	private void DrawableDrawOrderChanged(object sender, EventArgs e)
	{
		IDrawable drawable = sender as IDrawable;
		drawableComponents.Remove(drawable);
		int num = drawableComponents.BinarySearch(drawable, DrawOrderComparer.Default);
		if (num < 0)
		{
			for (num = ~num; num < drawableComponents.Count && drawableComponents[num].DrawOrder == drawable.DrawOrder; num++)
			{
			}
			drawableComponents.Insert(num, drawable);
		}
	}

	private void UpdateableUpdateOrderChanged(object sender, EventArgs e)
	{
		IUpdateable updateable = sender as IUpdateable;
		updateableComponents.Remove(updateable);
		int num = updateableComponents.BinarySearch(updateable, UpdateOrderComparer.Default);
		if (num < 0)
		{
			for (num = ~num; num < updateableComponents.Count && updateableComponents[num].UpdateOrder == updateable.UpdateOrder; num++)
			{
			}
			updateableComponents.Insert(num, updateable);
		}
	}

	protected virtual void OnActivated(object sender, EventArgs args)
	{
		if (this.Activated != null)
		{
			this.Activated(this, args);
		}
	}

	protected virtual void OnDeactivated(object sender, EventArgs args)
	{
		if (this.Deactivated != null)
		{
			this.Deactivated(this, args);
		}
	}

	protected virtual void OnExiting(object sender, EventArgs args)
	{
		if (this.Exiting != null)
		{
			this.Exiting(null, args);
		}
	}

	private void EnsureHost()
	{
		if (host == null)
		{
			host = new WindowsGameHost(this);
			host.Activated += HostActivated;
			host.Deactivated += HostDeactivated;
			host.Suspend += HostSuspend;
			host.Resume += HostResume;
			host.Idle += HostIdle;
			host.Exiting += HostExiting;
		}
	}

	private void HostSuspend(object sender, EventArgs e)
	{
		clock.Suspend();
	}

	private void HostResume(object sender, EventArgs e)
	{
		clock.Resume();
	}

	private void HostExiting(object sender, EventArgs e)
	{
		OnExiting(this, EventArgs.Empty);
	}

	private void HostIdle(object sender, EventArgs e)
	{
		Tick();
	}

	private void HostDeactivated(object sender, EventArgs e)
	{
		if (isActive)
		{
			isActive = false;
			OnDeactivated(this, EventArgs.Empty);
		}
	}

	private void HostActivated(object sender, EventArgs e)
	{
		if (!isActive)
		{
			isActive = true;
			OnActivated(this, EventArgs.Empty);
		}
	}

	private void HookDeviceEvents()
	{
		graphicsDeviceService = Services.GetService(typeof(IGraphicsDeviceService)) as IGraphicsDeviceService;
		if (graphicsDeviceService != null)
		{
			graphicsDeviceService.DeviceCreated += DeviceCreated;
			graphicsDeviceService.DeviceResetting += DeviceResetting;
			graphicsDeviceService.DeviceReset += DeviceReset;
			graphicsDeviceService.DeviceDisposing += DeviceDisposing;
		}
	}

	private void UnhookDeviceEvents()
	{
		if (graphicsDeviceService != null)
		{
			graphicsDeviceService.DeviceCreated -= DeviceCreated;
			graphicsDeviceService.DeviceResetting -= DeviceResetting;
			graphicsDeviceService.DeviceReset -= DeviceReset;
			graphicsDeviceService.DeviceDisposing -= DeviceDisposing;
		}
	}

	private void DeviceResetting(object sender, EventArgs e)
	{
	}

	private void DeviceReset(object sender, EventArgs e)
	{
	}

	private void DeviceCreated(object sender, EventArgs e)
	{
		LoadContent();
	}

	private void DeviceDisposing(object sender, EventArgs e)
	{
		content.Unload();
		UnloadContent();
	}

	protected virtual void LoadContent()
	{
	}

	protected virtual void UnloadContent()
	{
	}

	public void Dispose()
	{
		Dispose(disposing: true);
		GC.SuppressFinalize(this);
	}

	~Game()
	{
		Dispose(disposing: false);
	}

	protected virtual void Dispose(bool disposing)
	{
		if (!disposing)
		{
			return;
		}
		lock (this)
		{
			IGameComponent[] array = new IGameComponent[gameComponents.Count];
			gameComponents.CopyTo(array, 0);
			for (int i = 0; i < array.Length; i++)
			{
				if (array[i] is IDisposable disposable)
				{
					disposable.Dispose();
				}
			}
			if (graphicsDeviceManager is IDisposable disposable2)
			{
				disposable2.Dispose();
			}
			UnhookDeviceEvents();
			if (this.Disposed != null)
			{
				this.Disposed(this, EventArgs.Empty);
			}
		}
	}

	protected virtual bool ShowMissingRequirementMessage(Exception exception)
	{
		if (host != null)
		{
			return host.ShowMissingRequirementMessage(exception);
		}
		return false;
	}
}
