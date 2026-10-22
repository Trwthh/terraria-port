// XNA stand-in: Microsoft.Xna.Framework.Game.dll. No window, no device, no loop of its own: the harness drives the
// frame loop through Host, which mirrors port\crates\terraria-host\src\game.rs (init, begin_frame, begin_update,
// end_update, begin_draw, end_draw) so both sides see the same GameTime values, window sizes and device resets.
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using Microsoft.Xna.Framework.Content;
using Microsoft.Xna.Framework.Graphics;
using Microsoft.Xna.Framework.XnaStandIn;

namespace Microsoft.Xna.Framework.XnaStandIn
{
    /// <summary>The process's one game host (port: terraria_host::game::HostState).</summary>
    public static class Host
    {
        public const long TargetTicks = 166667;
        public static GraphicsDevice Device;
        public static GraphicsDeviceManager Manager;
        public static StandInWindow Window;
        public static GameServiceContainer Services;
        public static GameTime Time;
        public static Game TheGame;
        public static bool IsActive = true, IsFixedTimeStep = true, IsMouseVisible, ShouldExit, SuppressDrawFlag;
        public static TimeSpan TargetElapsed = TimeSpan.FromTicks(TargetTicks), InactiveSleep = TimeSpan.FromTicks(200000);
        public static EventHandler<EventArgs> Exiting, Activated, Deactivated;
        public static int WinW, WinH;
        static long accumulated, total, lastFrameElapsed;
        static int slow1 = int.MaxValue, slow2 = int.MaxValue;
        static bool drawRunningSlowly, allSuppressed = true, ranUpdate;
        static int pendingN; static long pendingTicks;
        static readonly List<EventHandler<EventArgs>> events = new List<EventHandler<EventArgs>>();

        /// <summary>port game::init: device, manager state, window, services, before `new Main()`.</summary>
        public static void Init(int width, int height, IntPtr handle, int displayWidth, int displayHeight)
        {
            GraphicsAdapter.DisplayWidth = displayWidth; GraphicsAdapter.DisplayHeight = displayHeight;
            var pp = new PresentationParameters();
            pp.BackBufferWidth = width; pp.BackBufferHeight = height; pp.BackBufferFormat = SurfaceFormat.Color;
            pp.DepthStencilFormat = DepthFormat.Depth24; pp.RenderTargetUsage = RenderTargetUsage.DiscardContents;
            pp.PresentationInterval = PresentInterval.One; pp.DeviceWindowHandle = handle;
            Device = new GraphicsDevice(GraphicsAdapter.DefaultAdapter, GraphicsProfile.HiDef, pp);
            Window = new StandInWindow(handle, width, height);
            Services = new GameServiceContainer();
            Time = new GameTime(TimeSpan.Zero, TimeSpan.Zero);
            WinW = width; WinH = height;
        }

        /// <summary>port game::begin_frame with an idle input and the window at (WinW, WinH): the number of updates to run.</summary>
        public static int BeginFrame(long elapsedTicks)
        {
            PresentationParameters pp = Device.PresentationParameters;
            if ((WinW != pp.BackBufferWidth || WinH != pp.BackBufferHeight) && WinW > 0 && WinH > 0)
            {
                Device.ResetBackBuffer(WinW, WinH);
                if (Window._clientSizeChanged != null) events.Add(Window._clientSizeChanged);
            }
            Window._bounds = new Rectangle(0, 0, WinW, WinH);
            pendingN = 0; pendingTicks = 0; allSuppressed = true; ranUpdate = false;
            if (ShouldExit) return 0;
            long elapsed = Math.Max(0, Math.Min(elapsedTicks, 5000000));
            long target = TargetElapsed.Ticks;
            if (IsFixedTimeStep)
            {
                if (Math.Abs(elapsed - target) < (target >> 6)) elapsed = target;
                accumulated += elapsed;
                long n = accumulated / target;
                accumulated %= target;
                lastFrameElapsed = 0;
                if (n == 0) return 0;
                if (n > 1) { slow2 = slow1; slow1 = 0; }
                else { slow1 = slow1 == int.MaxValue ? int.MaxValue : slow1 + 1; slow2 = slow2 == int.MaxValue ? int.MaxValue : slow2 + 1; }
                drawRunningSlowly = slow2 < 20;
                pendingN = (int)n; pendingTicks = target;
            }
            else
            {
                drawRunningSlowly = false; slow1 = slow2 = int.MaxValue; lastFrameElapsed = 0;
                pendingN = 1; pendingTicks = elapsed;
            }
            return pendingN;
        }
        public static void DispatchEvents()
        {
            var e = events.ToArray(); events.Clear();
            foreach (var h in e) h(null, null);
        }
        public static GameTime BeginUpdate()
        {
            if (pendingN == 0 || ShouldExit) return null;
            Time._elapsed = TimeSpan.FromTicks(pendingTicks); Time._total = TimeSpan.FromTicks(total); Time._slow = drawRunningSlowly;
            return Time;
        }
        public static void EndUpdate()
        {
            allSuppressed &= SuppressDrawFlag; SuppressDrawFlag = false; ranUpdate = true;
            lastFrameElapsed += pendingTicks; total += pendingTicks; pendingN = Math.Max(0, pendingN - 1);
        }
        public static GameTime BeginDraw()
        {
            if (!ranUpdate || allSuppressed || ShouldExit) return null;
            Time._elapsed = TimeSpan.FromTicks(lastFrameElapsed); Time._total = TimeSpan.FromTicks(total); Time._slow = drawRunningSlowly;
            return Time;
        }
        public static void EndDraw(bool drew) { if (drew) Device.Present(); }
    }

    public sealed class StandInWindow : GameWindow
    {
        internal Rectangle _bounds;
        internal IntPtr _handle;
        internal bool _resizing;
        internal EventHandler<EventArgs> _clientSizeChanged;
        public StandInWindow(IntPtr handle, int w, int h) : base(default(StubCtor)) { _handle = handle; _bounds = new Rectangle(0, 0, w, h); }
        public override IntPtr Handle { get { return _handle; } }
        public override bool AllowUserResizing { get { return _resizing; } set { _resizing = value; } }
        public override Rectangle ClientBounds { get { return _bounds; } }
        public override string ScreenDeviceName { get { return "\\\\.\\DISPLAY1"; } }
        public override DisplayOrientation CurrentOrientation { get { return DisplayOrientation.Default; } }
        public override void BeginScreenDeviceChange(bool willBeFullScreen) { throw new NotImplementedException("XNA stand-in: GameWindow.BeginScreenDeviceChange"); }
        public override void EndScreenDeviceChange(string screenDeviceName, int clientWidth, int clientHeight) { throw new NotImplementedException("XNA stand-in: GameWindow.EndScreenDeviceChange"); }
        protected override void SetTitle(string title) { }
        protected override void SetSupportedOrientations(DisplayOrientation orientations) { }
    }
}

namespace Microsoft.Xna.Framework
{
    public abstract partial class GameWindow
    {
        string _title = "";
        //@impl GameWindow.Title
        public string Title { get { return _title; } set { _title = value ?? ""; } }
        //@impl GameWindow.ClientSizeChanged
        public event EventHandler<EventArgs> ClientSizeChanged
        {
            add { ((StandInWindow)this)._clientSizeChanged += value; }
            remove { ((StandInWindow)this)._clientSizeChanged -= value; }
        }
    }

    public partial class GameTime
    {
        internal TimeSpan _total, _elapsed;
        internal bool _slow;
        //@impl GameTime..ctor()
        public GameTime() : this(default(StubCtor)) { }
        //@impl GameTime..ctor(TimeSpan, TimeSpan, bool)
        public GameTime(TimeSpan totalGameTime, TimeSpan elapsedGameTime, bool isRunningSlowly) : this(default(StubCtor)) { _total = totalGameTime; _elapsed = elapsedGameTime; _slow = isRunningSlowly; }
        //@impl GameTime..ctor(TimeSpan, TimeSpan)
        public GameTime(TimeSpan totalGameTime, TimeSpan elapsedGameTime) : this(default(StubCtor)) { _total = totalGameTime; _elapsed = elapsedGameTime; }
        //@impl GameTime.TotalGameTime
        public TimeSpan TotalGameTime { get { return _total; } }
        //@impl GameTime.ElapsedGameTime
        public TimeSpan ElapsedGameTime { get { return _elapsed; } }
        //@impl GameTime.IsRunningSlowly
        public bool IsRunningSlowly { get { return _slow; } }
    }

    public partial class GameServiceContainer
    {
        readonly Dictionary<Type, object> _s = new Dictionary<Type, object>();
        //@impl GameServiceContainer..ctor()
        public GameServiceContainer() : this(default(StubCtor)) { }
        //@impl GameServiceContainer.AddService(Type, object)
        public void AddService(Type type, object provider)
        {
            if (type == null) throw new ArgumentNullException("type");
            if (provider == null) throw new ArgumentNullException("provider");
            if (_s.ContainsKey(type)) throw new ArgumentException("The service is already present in the container.");
            _s.Add(type, provider);
        }
        //@impl GameServiceContainer.RemoveService(Type)
        public void RemoveService(Type type) { _s.Remove(type); }
        //@impl GameServiceContainer.GetService(Type)
        public object GetService(Type type) { object o; return _s.TryGetValue(type, out o) ? o : null; }
    }

    public sealed partial class GameComponentCollection
    {
        //@impl GameComponentCollection..ctor()
        public GameComponentCollection() : this(default(StubCtor)) { }
    }

    public partial class LaunchParameters
    {
        //@impl LaunchParameters..ctor()
        public LaunchParameters() : this(default(StubCtor)) { }
    }

    public partial class GraphicsDeviceInformation
    {
        GraphicsAdapter _adapter; GraphicsProfile _profile; PresentationParameters _pp;
        //@impl GraphicsDeviceInformation..ctor()
        public GraphicsDeviceInformation() : this(default(StubCtor)) { }
        //@impl GraphicsDeviceInformation.Adapter
        public GraphicsAdapter Adapter { get { return _adapter; } set { _adapter = value; } }
        //@impl GraphicsDeviceInformation.GraphicsProfile
        public GraphicsProfile GraphicsProfile { get { return _profile; } set { _profile = value; } }
        //@impl GraphicsDeviceInformation.PresentationParameters
        public PresentationParameters PresentationParameters { get { return _pp; } set { _pp = value; } }
    }

    public partial class PreparingDeviceSettingsEventArgs
    {
        GraphicsDeviceInformation _info;
        //@impl PreparingDeviceSettingsEventArgs..ctor(GraphicsDeviceInformation)
        public PreparingDeviceSettingsEventArgs(GraphicsDeviceInformation graphicsDeviceInformation) : this(default(StubCtor)) { _info = graphicsDeviceInformation; }
        //@impl PreparingDeviceSettingsEventArgs.GraphicsDeviceInformation
        public GraphicsDeviceInformation GraphicsDeviceInformation { get { return _info; } }
    }

    public partial class GraphicsDeviceManager
    {
        int _pw, _ph; bool _full, _vsync = true, _msaa; GraphicsProfile _profile = GraphicsProfile.HiDef;
        EventHandler<PreparingDeviceSettingsEventArgs> _preparing;
        EventHandler<EventArgs> _created, _disposing, _reset, _resetting;

        //@impl GraphicsDeviceManager..ctor(Game)
        public GraphicsDeviceManager(Game game) : this(default(StubCtor))
        {
            if (game == null) throw new ArgumentNullException("game");
            if (Host.Manager != null) throw new InvalidOperationException("XNA stand-in: a second GraphicsDeviceManager");
            _pw = Host.WinW; _ph = Host.WinH;
            Host.Manager = this;
            Host.Services.AddService(typeof(IGraphicsDeviceService), this);
            Host.Services.AddService(typeof(IGraphicsDeviceManager), this);
        }
        //@impl GraphicsDeviceManager.GraphicsDevice
        public GraphicsDevice GraphicsDevice { get { return Host.Device; } }
        //@impl GraphicsDeviceManager.PreferredBackBufferWidth
        public int PreferredBackBufferWidth { get { return _pw; } set { if (value <= 0) throw new ArgumentOutOfRangeException("value"); _pw = value; } }
        //@impl GraphicsDeviceManager.PreferredBackBufferHeight
        public int PreferredBackBufferHeight { get { return _ph; } set { if (value <= 0) throw new ArgumentOutOfRangeException("value"); _ph = value; } }
        //@impl GraphicsDeviceManager.IsFullScreen
        public bool IsFullScreen { get { return _full; } set { _full = value; } }
        //@impl GraphicsDeviceManager.SynchronizeWithVerticalRetrace
        public bool SynchronizeWithVerticalRetrace { get { return _vsync; } set { _vsync = value; } }
        //@impl GraphicsDeviceManager.PreferMultiSampling
        public bool PreferMultiSampling { get { return _msaa; } set { _msaa = value; } }
        //@impl GraphicsDeviceManager.GraphicsProfile
        public GraphicsProfile GraphicsProfile { get { return _profile; } set { _profile = value; } }
        //@impl GraphicsDeviceManager.PreparingDeviceSettings
        public event EventHandler<PreparingDeviceSettingsEventArgs> PreparingDeviceSettings { add { _preparing += value; } remove { _preparing -= value; } }
        //@impl GraphicsDeviceManager.DeviceCreated
        public event EventHandler<EventArgs> DeviceCreated { add { _created += value; } remove { _created -= value; } }
        //@impl GraphicsDeviceManager.DeviceDisposing
        public event EventHandler<EventArgs> DeviceDisposing { add { _disposing += value; } remove { _disposing -= value; } }
        //@impl GraphicsDeviceManager.DeviceReset
        public event EventHandler<EventArgs> DeviceReset { add { _reset += value; } remove { _reset -= value; } }
        //@impl GraphicsDeviceManager.DeviceResetting
        public event EventHandler<EventArgs> DeviceResetting { add { _resetting += value; } remove { _resetting -= value; } }
        //@impl GraphicsDeviceManager.ToggleFullScreen()
        public void ToggleFullScreen() { _full = !_full; ApplyChanges(); }
        /// <summary>port GraphicsDeviceManager::apply_changes: PreparingDeviceSettings with the device's parameters, then the back buffer takes the preferred size.</summary>
        //@impl GraphicsDeviceManager.ApplyChanges()
        public void ApplyChanges()
        {
            PresentationParameters pp = Host.Device.PresentationParameters;
            if (_preparing != null)
            {
                var info = new GraphicsDeviceInformation();
                info.Adapter = GraphicsAdapter.DefaultAdapter; info.GraphicsProfile = _profile; info.PresentationParameters = pp;
                _preparing(null, new PreparingDeviceSettingsEventArgs(info));
            }
            pp.IsFullScreen = _full;
            pp.PresentationInterval = _vsync ? PresentInterval.One : PresentInterval.Immediate;
            Host.Device.SetProfile(_profile);
            if (pp.BackBufferWidth != _pw || pp.BackBufferHeight != _ph) Host.Device.ResetBackBuffer(_pw, _ph);
            Host.Window._bounds = new Rectangle(Host.Window._bounds.X, Host.Window._bounds.Y, _pw, _ph);
        }
        //@impl GraphicsDeviceManager.IDisposable.Dispose()
        void IDisposable.Dispose() { }
        //@impl GraphicsDeviceManager.Dispose(bool)
        protected virtual void Dispose(bool disposing) { }
    }

    public partial class Game
    {
        ContentManager _content;
        GameComponentCollection _components = new GameComponentCollection();
        LaunchParameters _launch = new LaunchParameters();

        //@impl Game..ctor()
        public Game() : this(default(StubCtor))
        {
            if (Host.Device == null) throw new InvalidOperationException("XNA stand-in: Host.Init before new Game()");
            Host.TheGame = this;
            _content = new ContentManager(Host.Services);
        }
        //@impl Game.LaunchParameters
        public LaunchParameters LaunchParameters { get { return _launch; } }
        //@impl Game.Components
        public GameComponentCollection Components { get { return _components; } }
        //@impl Game.Services
        public GameServiceContainer Services { get { return Host.Services; } }
        //@impl Game.InactiveSleepTime
        public TimeSpan InactiveSleepTime { get { return Host.InactiveSleep; } set { Host.InactiveSleep = value; } }
        //@impl Game.IsMouseVisible
        public bool IsMouseVisible { get { return Host.IsMouseVisible; } set { Host.IsMouseVisible = value; } }
        //@impl Game.TargetElapsedTime
        public TimeSpan TargetElapsedTime { get { return Host.TargetElapsed; } set { if (value <= TimeSpan.Zero) throw new ArgumentOutOfRangeException("value"); Host.TargetElapsed = value; } }
        //@impl Game.IsFixedTimeStep
        public bool IsFixedTimeStep { get { return Host.IsFixedTimeStep; } set { Host.IsFixedTimeStep = value; } }
        //@impl Game.Window
        public GameWindow Window { get { return Host.Window; } }
        //@impl Game.IsActive
        public bool IsActive { get { return Host.IsActive; } }
        //@impl Game.GraphicsDevice
        public GraphicsDevice GraphicsDevice { get { return Host.Device; } }
        //@impl Game.Content
        public ContentManager Content { get { return _content; } set { if (value == null) throw new ArgumentNullException(); _content = value; } }
        //@impl Game.Activated
        public event EventHandler<EventArgs> Activated { add { Host.Activated += value; } remove { Host.Activated -= value; } }
        //@impl Game.Deactivated
        public event EventHandler<EventArgs> Deactivated { add { Host.Deactivated += value; } remove { Host.Deactivated -= value; } }
        //@impl Game.Exiting
        public event EventHandler<EventArgs> Exiting { add { Host.Exiting += value; } remove { Host.Exiting -= value; } }
        //@impl Game.SuppressDraw()
        public void SuppressDraw() { Host.SuppressDrawFlag = true; }
        //@impl Game.Exit()
        public void Exit() { Host.ShouldExit = true; }
        //@impl Game.Update(GameTime)
        protected virtual void Update(GameTime gameTime) { }
        //@impl Game.Draw(GameTime)
        protected virtual void Draw(GameTime gameTime) { }
        //@impl Game.BeginDraw()
        protected virtual bool BeginDraw() { return true; }
        //@impl Game.EndDraw()
        protected virtual void EndDraw() { }
        //@impl Game.Initialize()
        protected virtual void Initialize() { LoadContent(); }
        //@impl Game.LoadContent()
        protected virtual void LoadContent() { }
        //@impl Game.UnloadContent()
        protected virtual void UnloadContent() { }
        //@impl Game.Dispose()
        public void Dispose() { Dispose(true); }
        //@impl Game.Dispose(bool)
        protected virtual void Dispose(bool disposing) { }

        /// <summary>The harness calls the game's protected overrides through these.</summary>
        public void StandInInitialize() { Initialize(); }
        public void StandInUpdate(GameTime t) { Update(t); }
        public void StandInDraw(GameTime t) { Draw(t); }
    }
}
