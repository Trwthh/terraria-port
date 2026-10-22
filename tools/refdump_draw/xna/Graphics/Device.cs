// XNA stand-in: GraphicsDevice, resources, states, adapter (hand-written; the rest of the surface is generated stubs).
// Semantics follow XNA 4.0 as port\crates\terraria-host\src\gfx\device.rs / states.rs model them; every call that
// changes what is drawn is recorded (Rec.cs). Nothing is rendered.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Threading;
using Microsoft.Xna.Framework.XnaStandIn;

namespace Microsoft.Xna.Framework.Graphics
{
    // ======================================================================== GraphicsResource
    public abstract partial class GraphicsResource
    {
        internal GraphicsDevice _device;
        internal bool _disposed;
        string _name;
        object _tag;
        EventHandler<EventArgs> _disposing;
        /// <summary>The recorder's name for this resource.</summary>
        public string RecLabel;

        //@impl GraphicsResource.IsDisposed
        public bool IsDisposed { get { return _disposed; } }
        //@impl GraphicsResource.Tag
        public object Tag { get { return _tag; } set { _tag = value; } }
        //@impl GraphicsResource.Name
        public string Name { get { return _name; } set { _name = value; } }
        //@impl GraphicsResource.GraphicsDevice
        public GraphicsDevice GraphicsDevice { get { return _device; } }
        //@impl GraphicsResource.Disposing
        public event EventHandler<EventArgs> Disposing { add { _disposing += value; } remove { _disposing -= value; } }
        //@impl GraphicsResource.ToString()
        public override string ToString() { return _name ?? base.ToString(); }
        //@impl GraphicsResource.Dispose(bool)
        protected virtual void Dispose(bool disposing)
        {
            if (_disposed) return;
            if (_disposing != null) _disposing(this, EventArgs.Empty);
            _disposed = true;
        }
        //@impl GraphicsResource.Dispose()
        public void Dispose() { Dispose(true); GC.SuppressFinalize(this); }
    }

    // ======================================================================== textures
    public abstract partial class Texture
    {
        internal SurfaceFormat _format;
        internal int _levels = 1;
        //@impl Texture.LevelCount
        public int LevelCount { get { return _levels; } }
        //@impl Texture.Format
        public SurfaceFormat Format { get { return _format; } }
    }

    public partial class Texture2D
    {
        internal int _width, _height;
        internal byte[] _cpu;

        /// <summary>A content texture (Texture2DReader): no pixels kept.</summary>
        internal Texture2D(GraphicsDevice device, int width, int height, int levels, SurfaceFormat format, string label)
            : this(default(StubCtor))
        {
            _device = device; _width = width; _height = height; _levels = levels; _format = format; RecLabel = label;
            Name = null;
        }

        void Init(GraphicsDevice device, int width, int height, bool mipMap, SurfaceFormat format)
        {
            if (device == null) throw new ArgumentNullException("graphicsDevice", "The GraphicsDevice must not be null when creating new resources.");
            if (width <= 0 || height <= 0) throw new ArgumentOutOfRangeException(width <= 0 ? "width" : "height");
            _device = device; _width = width; _height = height; _format = format;
            _levels = 1;
            if (mipMap) { int s = Math.Max(width, height); while (s > 1) { s >>= 1; _levels++; } }
        }

        //@impl Texture2D..ctor(GraphicsDevice, int, int)
        public Texture2D(GraphicsDevice graphicsDevice, int width, int height) : this(default(StubCtor))
        {
            Init(graphicsDevice, width, height, false, SurfaceFormat.Color);
            RecLabel = "tex" + (Interlocked.Increment(ref Rec.NewTex) - 1);
            Rec.L("tex.new " + RecLabel + " " + width + " " + height + " 0 0");
        }
        //@impl Texture2D..ctor(GraphicsDevice, int, int, bool, SurfaceFormat)
        public Texture2D(GraphicsDevice graphicsDevice, int width, int height, bool mipMap, SurfaceFormat format) : this(default(StubCtor))
        {
            Init(graphicsDevice, width, height, mipMap, format);
            RecLabel = "tex" + (Interlocked.Increment(ref Rec.NewTex) - 1);
            Rec.L("tex.new " + RecLabel + " " + width + " " + height + " " + Rec.B(mipMap) + " " + (int)format);
        }
        /// <summary>RenderTarget2D's base construction (no label, no record).</summary>
        internal Texture2D(GraphicsDevice graphicsDevice, int width, int height, bool mipMap, SurfaceFormat format, bool forTarget) : this(default(StubCtor))
        {
            Init(graphicsDevice, width, height, mipMap, format);
        }

        //@impl Texture2D.Bounds
        public Rectangle Bounds { get { return new Rectangle(0, 0, _width, _height); } }
        //@impl Texture2D.Height
        public int Height { get { return _height; } }
        //@impl Texture2D.Width
        public int Width { get { return _width; } }

        void CheckSetData(int level, Rectangle? rect, int elementSize, int count)
        {
            if (_disposed) throw new ObjectDisposedException("Texture2D");
            if (level < 0 || level >= _levels) throw new ArgumentOutOfRangeException("level");
        }

        //@impl Texture2D.SetData<T>(T[])
        public void SetData<T>(T[] data) where T : struct
        {
            Rec.L("tex.data " + Rec.T(this) + " a 0 - 0 " + (data == null ? 0 : data.Length) + " " + Rec.Hash(data, 0, data == null ? 0 : data.Length));
            if (data == null) throw new ArgumentNullException("data");
            Keep(null, data, 0, data.Length);
        }
        //@impl Texture2D.SetData<T>(T[], int, int)
        public void SetData<T>(T[] data, int startIndex, int elementCount) where T : struct
        {
            Rec.L("tex.data " + Rec.T(this) + " b 0 - " + startIndex + " " + elementCount + " " + Rec.Hash(data, startIndex, elementCount));
            if (data == null) throw new ArgumentNullException("data");
            Keep(null, data, startIndex, elementCount);
        }
        //@impl Texture2D.SetData<T>(int, Rectangle?, T[], int, int)
        public void SetData<T>(int level, Rectangle? rect, T[] data, int startIndex, int elementCount) where T : struct
        {
            Rec.L("tex.data " + Rec.T(this) + " c " + level + " " + Rec.R(rect) + " " + startIndex + " " + elementCount + " " + Rec.Hash(data, startIndex, elementCount));
            if (data == null) throw new ArgumentNullException("data");
            if (level == 0) Keep(rect, data, startIndex, elementCount);
        }

        // a CPU copy of level 0 (RGBA bytes, 4 per pixel) for GetData; only 4-byte elements are kept
        void Keep<T>(Rectangle? rect, T[] data, int start, int count) where T : struct
        {
            int size = System.Runtime.InteropServices.Marshal.SizeOf(typeof(T));
            Rectangle r = rect ?? new Rectangle(0, 0, _width, _height);
            if (size * count != r.Width * r.Height * 4) { _cpu = null; return; }
            if (_cpu == null) _cpu = new byte[_width * _height * 4];
            byte[] b = new byte[size * count];
            var h = System.Runtime.InteropServices.GCHandle.Alloc(data, System.Runtime.InteropServices.GCHandleType.Pinned);
            try { System.Runtime.InteropServices.Marshal.Copy(new IntPtr(h.AddrOfPinnedObject().ToInt64() + (long)start * size), b, 0, b.Length); }
            finally { h.Free(); }
            for (int y = 0; y < r.Height; y++) Buffer.BlockCopy(b, y * r.Width * 4, _cpu, ((r.Y + y) * _width + r.X) * 4, r.Width * 4);
        }
        void Give<T>(T[] data, int start, int count) where T : struct
        {
            if (_cpu == null) throw new NotImplementedException("XNA stand-in: Texture2D.GetData on a texture without CPU pixels (" + RecLabel + ")");
            int size = System.Runtime.InteropServices.Marshal.SizeOf(typeof(T));
            var h = System.Runtime.InteropServices.GCHandle.Alloc(data, System.Runtime.InteropServices.GCHandleType.Pinned);
            try { System.Runtime.InteropServices.Marshal.Copy(_cpu, 0, new IntPtr(h.AddrOfPinnedObject().ToInt64() + (long)start * size), Math.Min(_cpu.Length, size * count)); }
            finally { h.Free(); }
        }
        //@impl Texture2D.GetData<T>(T[])
        public void GetData<T>(T[] data) where T : struct { Give(data, 0, data.Length); }
        //@impl Texture2D.GetData<T>(T[], int, int)
        public void GetData<T>(T[] data, int startIndex, int elementCount) where T : struct { Give(data, startIndex, elementCount); }

        //@impl Texture2D.Dispose(bool)
        protected override void Dispose(bool disposing)
        {
            if (!_disposed) Rec.L("tex.dispose " + Rec.T(this));
            _cpu = null;
            base.Dispose(disposing);
        }
    }

    public partial class RenderTarget2D
    {
        internal RenderTargetUsage _usage;
        internal DepthFormat _depth;
        internal int _msaa;
        EventHandler<EventArgs> _contentLost;

        RenderTarget2D(GraphicsDevice device, int width, int height, bool mipMap, SurfaceFormat format, DepthFormat depth, int msaa, RenderTargetUsage usage, bool _)
            : base(device, width, height, mipMap, format, true)
        {
            _usage = usage; _depth = depth; _msaa = msaa;
            RecLabel = "rt" + (Interlocked.Increment(ref Rec.NewRt) - 1);
            Rec.L("rt.new " + RecLabel + " " + width + " " + height + " " + Rec.B(mipMap) + " " + (int)format + " " + (int)depth + " " + (int)usage);
        }
        //@impl RenderTarget2D..ctor(GraphicsDevice, int, int, bool, SurfaceFormat, DepthFormat, int, RenderTargetUsage)
        public RenderTarget2D(GraphicsDevice graphicsDevice, int width, int height, bool mipMap, SurfaceFormat preferredFormat, DepthFormat preferredDepthFormat, int preferredMultiSampleCount, RenderTargetUsage usage)
            : this(graphicsDevice, width, height, mipMap, preferredFormat, preferredDepthFormat, preferredMultiSampleCount, usage, true) { }
        //@impl RenderTarget2D..ctor(GraphicsDevice, int, int, bool, SurfaceFormat, DepthFormat)
        public RenderTarget2D(GraphicsDevice graphicsDevice, int width, int height, bool mipMap, SurfaceFormat preferredFormat, DepthFormat preferredDepthFormat)
            : this(graphicsDevice, width, height, mipMap, preferredFormat, preferredDepthFormat, 0, RenderTargetUsage.DiscardContents, true) { }
        //@impl RenderTarget2D..ctor(GraphicsDevice, int, int)
        public RenderTarget2D(GraphicsDevice graphicsDevice, int width, int height)
            : this(graphicsDevice, width, height, false, SurfaceFormat.Color, DepthFormat.None, 0, RenderTargetUsage.DiscardContents, true) { }
        //@impl RenderTarget2D.IsContentLost
        public bool IsContentLost { get { return false; } }
        //@impl RenderTarget2D.RenderTargetUsage
        public RenderTargetUsage RenderTargetUsage { get { return _usage; } }
        //@impl RenderTarget2D.MultiSampleCount
        public int MultiSampleCount { get { return _msaa; } }
        //@impl RenderTarget2D.DepthStencilFormat
        public DepthFormat DepthStencilFormat { get { return _depth; } }
        //@impl RenderTarget2D.ContentLost
        public event EventHandler<EventArgs> ContentLost { add { _contentLost += value; } remove { _contentLost -= value; } }
        //@impl RenderTarget2D.Dispose(bool)
        protected override void Dispose(bool disposing) { base.Dispose(disposing); }
    }

    public partial struct RenderTargetBinding
    {
        //@impl RenderTargetBinding..ctor(RenderTarget2D)
        public RenderTargetBinding(RenderTarget2D renderTarget) : this()
        {
            if (renderTarget == null) throw new ArgumentNullException("renderTarget");
            _renderTarget = renderTarget; _cubeMapFace = CubeMapFace.PositiveX;
        }
        //@impl RenderTargetBinding.RenderTarget
        public Texture RenderTarget { get { return _renderTarget; } }
        //@impl RenderTargetBinding.CubeMapFace
        public CubeMapFace CubeMapFace { get { return _cubeMapFace; } }
        //@impl RenderTargetBinding.op_Implicit(RenderTarget2D)
        public static implicit operator RenderTargetBinding(RenderTarget2D renderTarget) { return new RenderTargetBinding(renderTarget); }
    }

    // ======================================================================== viewport, presentation parameters
    public partial struct Viewport
    {
        //@impl Viewport..ctor(int, int, int, int)
        public Viewport(int x, int y, int width, int height) : this() { _x = x; _y = y; _width = width; _height = height; _minZ = 0f; _maxZ = 1f; }
        //@impl Viewport..ctor(Rectangle)
        public Viewport(Rectangle bounds) : this(bounds.X, bounds.Y, bounds.Width, bounds.Height) { }
        //@impl Viewport.X
        public int X { get { return _x; } set { _x = value; } }
        //@impl Viewport.Y
        public int Y { get { return _y; } set { _y = value; } }
        //@impl Viewport.Width
        public int Width { get { return _width; } set { _width = value; } }
        //@impl Viewport.Height
        public int Height { get { return _height; } set { _height = value; } }
        //@impl Viewport.MinDepth
        public float MinDepth { get { return _minZ; } set { _minZ = value; } }
        //@impl Viewport.MaxDepth
        public float MaxDepth { get { return _maxZ; } set { _maxZ = value; } }
        //@impl Viewport.Bounds
        public Rectangle Bounds { get { return new Rectangle(_x, _y, _width, _height); } set { _x = value.X; _y = value.Y; _width = value.Width; _height = value.Height; } }
        //@impl Viewport.AspectRatio
        public float AspectRatio { get { return _width != 0 && _height != 0 ? (float)_width / (float)_height : 0f; } }
        //@impl Viewport.TitleSafeArea
        public Rectangle TitleSafeArea { get { return new Rectangle(_x, _y, _width, _height); } }
        //@impl Viewport.ToString()
        public override string ToString() { return "{X:" + _x + " Y:" + _y + " Width:" + _width + " Height:" + _height + " MinDepth:" + _minZ + " MaxDepth:" + _maxZ + "}"; }
    }

    public partial class PresentationParameters
    {
        int _bbw, _bbh, _msaa; SurfaceFormat _bbf; DepthFormat _dsf = DepthFormat.Depth24; DisplayOrientation _orient; PresentInterval _pi = PresentInterval.Default;
        RenderTargetUsage _rtu = RenderTargetUsage.DiscardContents; IntPtr _hwnd; bool _full;
        //@impl PresentationParameters..ctor()
        public PresentationParameters() : this(default(StubCtor)) { }
        //@impl PresentationParameters.BackBufferWidth
        public int BackBufferWidth { get { return _bbw; } set { _bbw = value; } }
        //@impl PresentationParameters.BackBufferHeight
        public int BackBufferHeight { get { return _bbh; } set { _bbh = value; } }
        //@impl PresentationParameters.BackBufferFormat
        public SurfaceFormat BackBufferFormat { get { return _bbf; } set { _bbf = value; } }
        //@impl PresentationParameters.DepthStencilFormat
        public DepthFormat DepthStencilFormat { get { return _dsf; } set { _dsf = value; } }
        //@impl PresentationParameters.MultiSampleCount
        public int MultiSampleCount { get { return _msaa; } set { _msaa = value; } }
        //@impl PresentationParameters.DisplayOrientation
        public DisplayOrientation DisplayOrientation { get { return _orient; } set { _orient = value; } }
        //@impl PresentationParameters.PresentationInterval
        public PresentInterval PresentationInterval { get { return _pi; } set { _pi = value; } }
        //@impl PresentationParameters.RenderTargetUsage
        public RenderTargetUsage RenderTargetUsage { get { return _rtu; } set { _rtu = value; } }
        //@impl PresentationParameters.DeviceWindowHandle
        public IntPtr DeviceWindowHandle { get { return _hwnd; } set { _hwnd = value; } }
        //@impl PresentationParameters.IsFullScreen
        public bool IsFullScreen { get { return _full; } set { _full = value; } }
        //@impl PresentationParameters.Bounds
        public Rectangle Bounds { get { return new Rectangle(0, 0, _bbw, _bbh); } }
        //@impl PresentationParameters.Clone()
        public PresentationParameters Clone() { return (PresentationParameters)MemberwiseClone(); }
    }

    // ======================================================================== adapter, display modes
    public partial class DisplayMode
    {
        internal int _w, _h;
        internal DisplayMode(int w, int h) : this(default(StubCtor)) { _w = w; _h = h; }
        //@impl DisplayMode.Format
        public SurfaceFormat Format { get { return SurfaceFormat.Color; } }
        //@impl DisplayMode.Height
        public int Height { get { return _h; } }
        //@impl DisplayMode.Width
        public int Width { get { return _w; } }
        //@impl DisplayMode.AspectRatio
        public float AspectRatio { get { return (float)_w / (float)_h; } }
        //@impl DisplayMode.TitleSafeArea
        public Rectangle TitleSafeArea { get { return new Rectangle(0, 0, _w, _h); } }
        //@impl DisplayMode.ToString()
        public override string ToString() { return "{Width:" + _w + " Height:" + _h + " Format:Color}"; }
    }

    public partial class DisplayModeCollection
    {
        internal List<DisplayMode> _modes = new List<DisplayMode>();
        internal DisplayModeCollection(List<DisplayMode> modes) : this(default(StubCtor)) { _modes = modes; }
        //@impl DisplayModeCollection.this[SurfaceFormat]
        public IEnumerable<DisplayMode> this[SurfaceFormat format] { get { return format == SurfaceFormat.Color ? _modes : new List<DisplayMode>(); } }
        //@impl DisplayModeCollection.GetEnumerator()
        public IEnumerator<DisplayMode> GetEnumerator() { return _modes.GetEnumerator(); }
        //@impl DisplayModeCollection.IEnumerable.GetEnumerator()
        IEnumerator IEnumerable.GetEnumerator() { return _modes.GetEnumerator(); }
    }

    public sealed partial class GraphicsAdapter
    {
        static GraphicsAdapter _default;
        /// <summary>The display the stand-in reports (the port's test window state: 1920 x 1080).</summary>
        public static int DisplayWidth = 1920, DisplayHeight = 1080;
        GraphicsAdapter() : this(default(StubCtor)) { }
        //@impl GraphicsAdapter.DefaultAdapter
        public static GraphicsAdapter DefaultAdapter { get { if (_default == null) _default = new GraphicsAdapter(); return _default; } }
        //@impl GraphicsAdapter.Adapters
        public static ReadOnlyCollection<GraphicsAdapter> Adapters { get { return new ReadOnlyCollection<GraphicsAdapter>(new List<GraphicsAdapter> { DefaultAdapter }); } }
        //@impl GraphicsAdapter.CurrentDisplayMode
        public DisplayMode CurrentDisplayMode { get { return new DisplayMode(DisplayWidth, DisplayHeight); } }
        //@impl GraphicsAdapter.SupportedDisplayModes
        public DisplayModeCollection SupportedDisplayModes
        {
            get
            {
                // port\crates\terraria-host\src\winforms.rs display_modes(): the standard sizes up to the display, plus the display's own
                int[,] std = { { 640, 480 }, { 800, 600 }, { 1024, 768 }, { 1152, 864 }, { 1280, 720 }, { 1280, 800 }, { 1280, 1024 }, { 1360, 768 }, { 1366, 768 }, { 1600, 900 }, { 1680, 1050 }, { 1920, 1080 }, { 2560, 1440 }, { 3840, 2160 } };
                var l = new List<DisplayMode>();
                bool has = false;
                for (int i = 0; i < std.GetLength(0); i++)
                    if (std[i, 0] <= DisplayWidth && std[i, 1] <= DisplayHeight) { l.Add(new DisplayMode(std[i, 0], std[i, 1])); if (std[i, 0] == DisplayWidth && std[i, 1] == DisplayHeight) has = true; }
                if (!has) l.Add(new DisplayMode(DisplayWidth, DisplayHeight));
                return new DisplayModeCollection(l);
            }
        }
        //@impl GraphicsAdapter.DeviceName
        public string DeviceName { get { return "\\\\.\\DISPLAY1"; } }
        //@impl GraphicsAdapter.Description
        public string Description { get { return "XNA stand-in"; } }
        //@impl GraphicsAdapter.IsDefaultAdapter
        public bool IsDefaultAdapter { get { return true; } }
        //@impl GraphicsAdapter.IsWideScreen
        public bool IsWideScreen { get { return true; } }
        //@impl GraphicsAdapter.IsProfileSupported(GraphicsProfile)
        public bool IsProfileSupported(GraphicsProfile graphicsProfile) { return graphicsProfile == GraphicsProfile.Reach || graphicsProfile == GraphicsProfile.HiDef; }
    }

    // ======================================================================== GraphicsDevice
    public sealed partial class SamplerStateCollection
    {
        readonly GraphicsDevice _d;
        internal SamplerStateCollection(GraphicsDevice d) : this(default(StubCtor)) { _d = d; }
        //@impl SamplerStateCollection.this[int]
        public SamplerState this[int index]
        {
            get { return _d._samplers[index]; }
            set
            {
                Rec.L("gd.sampler " + index + " " + Rec.S(value));
                if (value == null) throw new ArgumentNullException("value");
                value._bound = true; _d._samplers[index] = value;
            }
        }
    }
    public sealed partial class TextureCollection
    {
        readonly GraphicsDevice _d;
        internal TextureCollection(GraphicsDevice d) : this(default(StubCtor)) { _d = d; }
        //@impl TextureCollection.this[int]
        public Texture this[int index]
        {
            get { return _d._textures[index]; }
            set { Rec.L("gd.texture " + index + " " + Rec.T(value)); _d._textures[index] = value; }
        }
    }

    public partial class GraphicsDevice
    {
        internal PresentationParameters _pp;
        internal BlendState _blend;
        internal DepthStencilState _depthState;
        internal RasterizerState _raster;
        internal SamplerState[] _samplers = new SamplerState[16];
        internal Texture[] _textures = new Texture[16];
        Rectangle _scissor;
        Viewport _viewport;
        RenderTarget2D _target;
        GraphicsProfile _profile;
        bool _disposed;
        VertexBufferBinding[] _vbs = new VertexBufferBinding[0];
        IndexBuffer _indices;
        SamplerStateCollection _samplerColl;
        TextureCollection _textureColl;
        EventHandler<EventArgs> _lost, _reset, _resetting, _disposing;
        GraphicsAdapter _adapter;

        //@impl GraphicsDevice..ctor(GraphicsAdapter, GraphicsProfile, PresentationParameters)
        public GraphicsDevice(GraphicsAdapter adapter, GraphicsProfile graphicsProfile, PresentationParameters presentationParameters) : this(default(StubCtor))
        {
            _adapter = adapter; _profile = graphicsProfile; _pp = presentationParameters;
            _blend = BlendState.Opaque; _blend._bound = true;
            _depthState = DepthStencilState.Default; _depthState._bound = true;
            _raster = RasterizerState.CullCounterClockwise; _raster._bound = true;
            for (int i = 0; i < 16; i++) _samplers[i] = SamplerState.LinearWrap;
            SamplerState.LinearWrap._bound = true;
            _scissor = new Rectangle(0, 0, _pp.BackBufferWidth, _pp.BackBufferHeight);
            _viewport = new Viewport(0, 0, _pp.BackBufferWidth, _pp.BackBufferHeight);
            _samplerColl = new SamplerStateCollection(this);
            _textureColl = new TextureCollection(this);
        }

        /// <summary>Driver: the back buffer got a new size (GraphicsDeviceManager.ApplyChanges / a window resize).</summary>
        public void ResetBackBuffer(int width, int height)
        {
            _pp.BackBufferWidth = width; _pp.BackBufferHeight = height;
            _target = null;
            _viewport = new Viewport(0, 0, width, height);
            _scissor = new Rectangle(0, 0, width, height);
        }

        //@impl GraphicsDevice.IsDisposed
        public bool IsDisposed { get { return _disposed; } }
        //@impl GraphicsDevice.ScissorRectangle
        public Rectangle ScissorRectangle
        {
            get { return _scissor; }
            set
            {
                Rec.L("gd.scissor " + Rec.R(value));
                int tw, th; TargetSize(out tw, out th);
                if (value.X < 0 || value.Y < 0 || value.Width < 0 || value.Height < 0 || value.X + value.Width > tw || value.Y + value.Height > th)
                    throw new ArgumentException("The scissor rectangle is invalid. It cannot be larger than or outside of the current render target bounds.");
                _scissor = value;
            }
        }
        //@impl GraphicsDevice.Indices
        public IndexBuffer Indices { get { return _indices; } set { Rec.L("gd.ib " + (value == null ? "-" : value.RecLabel)); _indices = value; } }
        //@impl GraphicsDevice.Viewport
        public Viewport Viewport
        {
            get { return _viewport; }
            set
            {
                Rec.L("gd.viewport " + value.X + " " + value.Y + " " + value.Width + " " + value.Height + " " + Rec.F(value.MinDepth) + " " + Rec.F(value.MaxDepth));
                int tw, th; TargetSize(out tw, out th);
                if (value.X < 0 || value.Y < 0 || value.Width < 0 || value.Height < 0 || value.X + value.Width > tw || value.Y + value.Height > th)
                    throw new ArgumentException("The viewport is invalid. The viewport cannot be larger than or outside of the current render target bounds.");
                _viewport = value;
            }
        }
        //@impl GraphicsDevice.DisplayMode
        public DisplayMode DisplayMode { get { return GraphicsAdapter.DefaultAdapter.CurrentDisplayMode; } }
        //@impl GraphicsDevice.GraphicsDeviceStatus
        public GraphicsDeviceStatus GraphicsDeviceStatus { get { return GraphicsDeviceStatus.Normal; } }
        //@impl GraphicsDevice.GraphicsProfile
        public GraphicsProfile GraphicsProfile { get { return _profile; } }
        /// <summary>Driver: GraphicsDeviceManager.ApplyChanges sets the profile.</summary>
        public void SetProfile(GraphicsProfile p) { _profile = p; }
        //@impl GraphicsDevice.Adapter
        public GraphicsAdapter Adapter { get { return _adapter; } }
        //@impl GraphicsDevice.PresentationParameters
        public PresentationParameters PresentationParameters { get { return _pp; } }
        //@impl GraphicsDevice.RasterizerState
        public RasterizerState RasterizerState
        {
            get { return _raster; }
            set { Rec.L("gd.raster " + Rec.S(value)); if (value == null) throw new ArgumentNullException("value"); value._bound = true; _raster = value; }
        }
        //@impl GraphicsDevice.DepthStencilState
        public DepthStencilState DepthStencilState
        {
            get { return _depthState; }
            set { Rec.L("gd.depth " + Rec.S(value)); if (value == null) throw new ArgumentNullException("value"); value._bound = true; _depthState = value; }
        }
        //@impl GraphicsDevice.BlendState
        public BlendState BlendState
        {
            get { return _blend; }
            set { Rec.L("gd.blend " + Rec.S(value)); if (value == null) throw new ArgumentNullException("value"); value._bound = true; _blend = value; }
        }
        //@impl GraphicsDevice.Textures
        public TextureCollection Textures { get { return _textureColl; } }
        //@impl GraphicsDevice.SamplerStates
        public SamplerStateCollection SamplerStates { get { return _samplerColl; } }
        //@impl GraphicsDevice.Disposing
        public event EventHandler<EventArgs> Disposing { add { _disposing += value; } remove { _disposing -= value; } }
        //@impl GraphicsDevice.DeviceLost
        public event EventHandler<EventArgs> DeviceLost { add { _lost += value; } remove { _lost -= value; } }
        //@impl GraphicsDevice.DeviceReset
        public event EventHandler<EventArgs> DeviceReset { add { _reset += value; } remove { _reset -= value; } }
        //@impl GraphicsDevice.DeviceResetting
        public event EventHandler<EventArgs> DeviceResetting { add { _resetting += value; } remove { _resetting -= value; } }

        void TargetSize(out int w, out int h)
        {
            if (_target == null) { w = _pp.BackBufferWidth; h = _pp.BackBufferHeight; }
            else { w = _target._width; h = _target._height; }
        }

        //@impl GraphicsDevice.Present()
        public void Present()
        {
            Rec.L("gd.present");
            if (_target != null) throw new InvalidOperationException("Cannot call Present when a render target is active.");
        }
        //@impl GraphicsDevice.Clear(Color)
        public void Clear(Color color) { Rec.L("gd.clear " + Rec.C(color)); }

        //@impl GraphicsDevice.SetRenderTarget(RenderTarget2D)
        public void SetRenderTarget(RenderTarget2D renderTarget)
        {
            Rec.L("gd.target " + Rec.T(renderTarget));
            if (renderTarget == _target) return;
            if (renderTarget != null && renderTarget._disposed) throw new ObjectDisposedException("RenderTarget2D");
            _target = renderTarget;
            int w, h; TargetSize(out w, out h);
            _viewport = new Viewport(0, 0, w, h);
        }
        //@impl GraphicsDevice.SetRenderTargets(RenderTargetBinding[])
        public void SetRenderTargets(params RenderTargetBinding[] renderTargets)
        {
            var sb = new System.Text.StringBuilder("gd.targets " + (renderTargets == null ? 0 : renderTargets.Length));
            if (renderTargets != null) foreach (var b in renderTargets) sb.Append(" " + Rec.T(b.RenderTarget));
            Rec.L(sb.ToString());
            int n = renderTargets == null ? 0 : renderTargets.Length;
            RenderTarget2D t = n == 0 ? null : (RenderTarget2D)renderTargets[0].RenderTarget;
            if (n > 1) throw new NotImplementedException("XNA stand-in: SetRenderTargets with more than one target");
            if (t == _target) return;
            _target = t;
            int w, h; TargetSize(out w, out h);
            _viewport = new Viewport(0, 0, w, h);
        }
        //@impl GraphicsDevice.GetRenderTargets()
        public RenderTargetBinding[] GetRenderTargets()
        {
            return _target == null ? new RenderTargetBinding[0] : new RenderTargetBinding[] { new RenderTargetBinding(_target) };
        }

        //@impl GraphicsDevice.SetVertexBuffer(VertexBuffer)
        public void SetVertexBuffer(VertexBuffer vertexBuffer)
        {
            Rec.L("gd.vb " + (vertexBuffer == null ? "-" : vertexBuffer.RecLabel) + " 0");
            _vbs = vertexBuffer == null ? new VertexBufferBinding[0] : new VertexBufferBinding[] { new VertexBufferBinding(vertexBuffer) };
        }
        //@impl GraphicsDevice.SetVertexBuffer(VertexBuffer, int)
        public void SetVertexBuffer(VertexBuffer vertexBuffer, int vertexOffset)
        {
            Rec.L("gd.vb " + (vertexBuffer == null ? "-" : vertexBuffer.RecLabel) + " " + vertexOffset);
            _vbs = vertexBuffer == null ? new VertexBufferBinding[0] : new VertexBufferBinding[] { new VertexBufferBinding(vertexBuffer, vertexOffset) };
        }
        //@impl GraphicsDevice.SetVertexBuffers(VertexBufferBinding[])
        public void SetVertexBuffers(params VertexBufferBinding[] vertexBuffers)
        {
            var sb = new System.Text.StringBuilder("gd.vbs " + (vertexBuffers == null ? 0 : vertexBuffers.Length));
            if (vertexBuffers != null) foreach (var b in vertexBuffers) sb.Append(" " + (b.VertexBuffer == null ? "-" : b.VertexBuffer.RecLabel) + "," + b.VertexOffset + "," + b.InstanceFrequency);
            Rec.L(sb.ToString());
            _vbs = vertexBuffers == null ? new VertexBufferBinding[0] : (VertexBufferBinding[])vertexBuffers.Clone();
        }
        //@impl GraphicsDevice.GetVertexBuffers()
        public VertexBufferBinding[] GetVertexBuffers() { return (VertexBufferBinding[])_vbs.Clone(); }

        //@impl GraphicsDevice.DrawPrimitives(PrimitiveType, int, int)
        public void DrawPrimitives(PrimitiveType primitiveType, int startVertex, int primitiveCount)
        {
            Rec.L("gd.dp " + (int)primitiveType + " " + startVertex + " " + primitiveCount);
        }
        //@impl GraphicsDevice.DrawIndexedPrimitives(PrimitiveType, int, int, int, int, int)
        public void DrawIndexedPrimitives(PrimitiveType primitiveType, int baseVertex, int minVertexIndex, int numVertices, int startIndex, int primitiveCount)
        {
            Rec.L("gd.dip " + (int)primitiveType + " " + baseVertex + " " + minVertexIndex + " " + numVertices + " " + startIndex + " " + primitiveCount);
        }
        static int IndexCount(PrimitiveType t, int prims)
        {
            switch (t)
            {
                case PrimitiveType.TriangleList: return prims * 3;
                case PrimitiveType.TriangleStrip: return prims + 2;
                case PrimitiveType.LineList: return prims * 2;
                case PrimitiveType.LineStrip: return prims + 1;
            }
            return 0;
        }
        //@impl GraphicsDevice.DrawUserIndexedPrimitives<T>(PrimitiveType, T[], int, int, short[], int, int)
        public void DrawUserIndexedPrimitives<T>(PrimitiveType primitiveType, T[] vertexData, int vertexOffset, int numVertices, short[] indexData, int indexOffset, int primitiveCount) where T : struct, IVertexType
        {
            Rec.L("gd.duip " + (int)primitiveType + " " + vertexOffset + " " + numVertices + " " + indexOffset + " " + primitiveCount + " "
                + Rec.Hash(vertexData, vertexOffset, numVertices) + " " + Rec.Hash(indexData, indexOffset, IndexCount(primitiveType, primitiveCount)));
        }
        //@impl GraphicsDevice.DrawUserPrimitives<T>(PrimitiveType, T[], int, int)
        public void DrawUserPrimitives<T>(PrimitiveType primitiveType, T[] vertexData, int vertexOffset, int primitiveCount) where T : struct, IVertexType
        {
            Rec.L("gd.dup " + (int)primitiveType + " " + vertexOffset + " " + primitiveCount + " " + Rec.Hash(vertexData, vertexOffset, IndexCount(primitiveType, primitiveCount)));
        }
        //@impl GraphicsDevice.Dispose()
        public void Dispose() { _disposed = true; }
        //@impl GraphicsDevice.Dispose(bool)
        protected virtual void Dispose(bool disposing) { _disposed = true; }
    }

    // ======================================================================== state objects
    public partial class BlendState
    {
        internal bool _bound;
        Blend _cs = Blend.One, _cd = Blend.Zero, _as = Blend.One, _ad = Blend.Zero;
        BlendFunction _cf = BlendFunction.Add, _af = BlendFunction.Add;
        Color _factor = Color.White;
        ColorWriteChannels _w0 = ColorWriteChannels.All, _w1 = ColorWriteChannels.All, _w2 = ColorWriteChannels.All, _w3 = ColorWriteChannels.All;
        int _msMask = -1;

        //@impl BlendState.Opaque
        public static readonly BlendState Opaque = Preset(Blend.One, Blend.Zero, "BlendState.Opaque");
        //@impl BlendState.AlphaBlend
        public static readonly BlendState AlphaBlend = Preset(Blend.One, Blend.InverseSourceAlpha, "BlendState.AlphaBlend");
        //@impl BlendState.Additive
        public static readonly BlendState Additive = Preset(Blend.SourceAlpha, Blend.One, "BlendState.Additive");
        //@impl BlendState.NonPremultiplied
        public static readonly BlendState NonPremultiplied = Preset(Blend.SourceAlpha, Blend.InverseSourceAlpha, "BlendState.NonPremultiplied");
        static BlendState Preset(Blend src, Blend dst, string name)
        {
            var b = new BlendState(); b._cs = b._as = src; b._cd = b._ad = dst; b.Name = name; b._bound = true; return b;
        }
        void Set() { if (_bound) throw new InvalidOperationException("You cannot modify a default (built-in) state object, or one that has already been bound to the device."); }
        //@impl BlendState..ctor()
        public BlendState() : this(default(StubCtor)) { }
        //@impl BlendState.MultiSampleMask
        public int MultiSampleMask { get { return _msMask; } set { Set(); _msMask = value; } }
        //@impl BlendState.BlendFactor
        public Color BlendFactor { get { return _factor; } set { Set(); _factor = value; } }
        //@impl BlendState.ColorWriteChannels3
        public ColorWriteChannels ColorWriteChannels3 { get { return _w3; } set { Set(); _w3 = value; } }
        //@impl BlendState.ColorWriteChannels2
        public ColorWriteChannels ColorWriteChannels2 { get { return _w2; } set { Set(); _w2 = value; } }
        //@impl BlendState.ColorWriteChannels1
        public ColorWriteChannels ColorWriteChannels1 { get { return _w1; } set { Set(); _w1 = value; } }
        //@impl BlendState.ColorWriteChannels
        public ColorWriteChannels ColorWriteChannels { get { return _w0; } set { Set(); _w0 = value; } }
        //@impl BlendState.AlphaBlendFunction
        public BlendFunction AlphaBlendFunction { get { return _af; } set { Set(); _af = value; } }
        //@impl BlendState.AlphaDestinationBlend
        public Blend AlphaDestinationBlend { get { return _ad; } set { Set(); _ad = value; } }
        //@impl BlendState.AlphaSourceBlend
        public Blend AlphaSourceBlend { get { return _as; } set { Set(); _as = value; } }
        //@impl BlendState.ColorBlendFunction
        public BlendFunction ColorBlendFunction { get { return _cf; } set { Set(); _cf = value; } }
        //@impl BlendState.ColorDestinationBlend
        public Blend ColorDestinationBlend { get { return _cd; } set { Set(); _cd = value; } }
        //@impl BlendState.ColorSourceBlend
        public Blend ColorSourceBlend { get { return _cs; } set { Set(); _cs = value; } }
        //@impl BlendState.Dispose(bool)
        protected override void Dispose(bool disposing) { base.Dispose(disposing); }
    }

    public partial class SamplerState
    {
        internal bool _bound;
        TextureFilter _filter = TextureFilter.Linear;
        TextureAddressMode _u = TextureAddressMode.Wrap, _v = TextureAddressMode.Wrap, _w = TextureAddressMode.Wrap;
        int _aniso = 4, _maxMip;
        float _bias;
        //@impl SamplerState.PointWrap
        public static readonly SamplerState PointWrap = Preset(TextureFilter.Point, TextureAddressMode.Wrap, "SamplerState.PointWrap");
        //@impl SamplerState.PointClamp
        public static readonly SamplerState PointClamp = Preset(TextureFilter.Point, TextureAddressMode.Clamp, "SamplerState.PointClamp");
        //@impl SamplerState.LinearWrap
        public static readonly SamplerState LinearWrap = Preset(TextureFilter.Linear, TextureAddressMode.Wrap, "SamplerState.LinearWrap");
        //@impl SamplerState.LinearClamp
        public static readonly SamplerState LinearClamp = Preset(TextureFilter.Linear, TextureAddressMode.Clamp, "SamplerState.LinearClamp");
        //@impl SamplerState.AnisotropicWrap
        public static readonly SamplerState AnisotropicWrap = Preset(TextureFilter.Anisotropic, TextureAddressMode.Wrap, "SamplerState.AnisotropicWrap");
        //@impl SamplerState.AnisotropicClamp
        public static readonly SamplerState AnisotropicClamp = Preset(TextureFilter.Anisotropic, TextureAddressMode.Clamp, "SamplerState.AnisotropicClamp");
        static SamplerState Preset(TextureFilter f, TextureAddressMode a, string name)
        {
            var s = new SamplerState(); s._filter = f; s._u = s._v = s._w = a; s.Name = name; s._bound = true; return s;
        }
        void Set() { if (_bound) throw new InvalidOperationException("You cannot modify a default (built-in) state object, or one that has already been bound to the device."); }
        //@impl SamplerState..ctor()
        public SamplerState() : this(default(StubCtor)) { }
        //@impl SamplerState.MipMapLevelOfDetailBias
        public float MipMapLevelOfDetailBias { get { return _bias; } set { Set(); _bias = value; } }
        //@impl SamplerState.MaxMipLevel
        public int MaxMipLevel { get { return _maxMip; } set { Set(); _maxMip = value; } }
        //@impl SamplerState.MaxAnisotropy
        public int MaxAnisotropy { get { return _aniso; } set { Set(); _aniso = value; } }
        //@impl SamplerState.AddressW
        public TextureAddressMode AddressW { get { return _w; } set { Set(); _w = value; } }
        //@impl SamplerState.AddressV
        public TextureAddressMode AddressV { get { return _v; } set { Set(); _v = value; } }
        //@impl SamplerState.AddressU
        public TextureAddressMode AddressU { get { return _u; } set { Set(); _u = value; } }
        //@impl SamplerState.Filter
        public TextureFilter Filter { get { return _filter; } set { Set(); _filter = value; } }
        //@impl SamplerState.Dispose(bool)
        protected override void Dispose(bool disposing) { base.Dispose(disposing); }
    }

    public partial class RasterizerState
    {
        internal bool _bound;
        CullMode _cull = CullMode.CullCounterClockwiseFace;
        FillMode _fill = FillMode.Solid;
        bool _scissor, _msaa = true;
        float _bias, _slope;
        //@impl RasterizerState.CullNone
        public static readonly RasterizerState CullNone = Preset(CullMode.None, "RasterizerState.CullNone");
        //@impl RasterizerState.CullClockwise
        public static readonly RasterizerState CullClockwise = Preset(CullMode.CullClockwiseFace, "RasterizerState.CullClockwise");
        //@impl RasterizerState.CullCounterClockwise
        public static readonly RasterizerState CullCounterClockwise = Preset(CullMode.CullCounterClockwiseFace, "RasterizerState.CullCounterClockwise");
        static RasterizerState Preset(CullMode c, string name) { var r = new RasterizerState(); r._cull = c; r.Name = name; r._bound = true; return r; }
        void Set() { if (_bound) throw new InvalidOperationException("You cannot modify a default (built-in) state object, or one that has already been bound to the device."); }
        //@impl RasterizerState..ctor()
        public RasterizerState() : this(default(StubCtor)) { }
        //@impl RasterizerState.SlopeScaleDepthBias
        public float SlopeScaleDepthBias { get { return _slope; } set { Set(); _slope = value; } }
        //@impl RasterizerState.DepthBias
        public float DepthBias { get { return _bias; } set { Set(); _bias = value; } }
        //@impl RasterizerState.MultiSampleAntiAlias
        public bool MultiSampleAntiAlias { get { return _msaa; } set { Set(); _msaa = value; } }
        //@impl RasterizerState.ScissorTestEnable
        public bool ScissorTestEnable { get { return _scissor; } set { Set(); _scissor = value; } }
        //@impl RasterizerState.FillMode
        public FillMode FillMode { get { return _fill; } set { Set(); _fill = value; } }
        //@impl RasterizerState.CullMode
        public CullMode CullMode { get { return _cull; } set { Set(); _cull = value; } }
        //@impl RasterizerState.Dispose(bool)
        protected override void Dispose(bool disposing) { base.Dispose(disposing); }
    }

    public partial class DepthStencilState
    {
        internal bool _bound;
        bool _enable = true, _write = true, _stencil;
        CompareFunction _func = CompareFunction.LessEqual;
        //@impl DepthStencilState.None
        public static readonly DepthStencilState None = Preset(false, false, "DepthStencilState.None");
        //@impl DepthStencilState.Default
        public static readonly DepthStencilState Default = Preset(true, true, "DepthStencilState.Default");
        //@impl DepthStencilState.DepthRead
        public static readonly DepthStencilState DepthRead = Preset(true, false, "DepthStencilState.DepthRead");
        static DepthStencilState Preset(bool e, bool w, string name) { var d = new DepthStencilState(); d._enable = e; d._write = w; d.Name = name; d._bound = true; return d; }
        void Set() { if (_bound) throw new InvalidOperationException("You cannot modify a default (built-in) state object, or one that has already been bound to the device."); }
        //@impl DepthStencilState..ctor()
        public DepthStencilState() : this(default(StubCtor)) { }
        //@impl DepthStencilState.StencilEnable
        public bool StencilEnable { get { return _stencil; } set { Set(); _stencil = value; } }
        //@impl DepthStencilState.DepthBufferFunction
        public CompareFunction DepthBufferFunction { get { return _func; } set { Set(); _func = value; } }
        //@impl DepthStencilState.DepthBufferWriteEnable
        public bool DepthBufferWriteEnable { get { return _write; } set { Set(); _write = value; } }
        //@impl DepthStencilState.DepthBufferEnable
        public bool DepthBufferEnable { get { return _enable; } set { Set(); _enable = value; } }
        //@impl DepthStencilState.Dispose(bool)
        protected override void Dispose(bool disposing) { base.Dispose(disposing); }
    }

    // ======================================================================== vertex / index data
    public partial struct VertexElement
    {
        //@impl VertexElement..ctor(int, VertexElementFormat, VertexElementUsage, int)
        public VertexElement(int offset, VertexElementFormat elementFormat, VertexElementUsage elementUsage, int usageIndex) : this()
        { _offset = offset; _format = elementFormat; _usage = elementUsage; _usageIndex = usageIndex; }
        //@impl VertexElement.Offset
        public int Offset { get { return _offset; } set { _offset = value; } }
        //@impl VertexElement.VertexElementFormat
        public VertexElementFormat VertexElementFormat { get { return _format; } set { _format = value; } }
        //@impl VertexElement.VertexElementUsage
        public VertexElementUsage VertexElementUsage { get { return _usage; } set { _usage = value; } }
        //@impl VertexElement.UsageIndex
        public int UsageIndex { get { return _usageIndex; } set { _usageIndex = value; } }
        //@impl VertexElement.GetHashCode()
        public override int GetHashCode() { return _offset ^ ((int)_format << 8) ^ ((int)_usage << 16) ^ (_usageIndex << 24); }
        //@impl VertexElement.ToString()
        public override string ToString() { return "{Offset:" + _offset + " Format:" + _format + " Usage:" + _usage + " UsageIndex:" + _usageIndex + "}"; }
        //@impl VertexElement.Equals(object)
        public override bool Equals(object obj) { return obj is VertexElement && this == (VertexElement)obj; }
        //@impl VertexElement.op_Equality(VertexElement, VertexElement)
        public static bool operator ==(VertexElement left, VertexElement right) { return left._offset == right._offset && left._format == right._format && left._usage == right._usage && left._usageIndex == right._usageIndex; }
        //@impl VertexElement.op_Inequality(VertexElement, VertexElement)
        public static bool operator !=(VertexElement left, VertexElement right) { return !(left == right); }
    }

    public partial class VertexDeclaration
    {
        VertexElement[] _elements;
        int _stride;
        static int Size(VertexElementFormat f)
        {
            switch (f)
            {
                case VertexElementFormat.Single: return 4;
                case VertexElementFormat.Vector2: return 8;
                case VertexElementFormat.Vector3: return 12;
                case VertexElementFormat.Vector4: return 16;
                case VertexElementFormat.Color: return 4;
                case VertexElementFormat.Byte4: return 4;
                case VertexElementFormat.Short2: return 4;
                case VertexElementFormat.Short4: return 8;
                case VertexElementFormat.NormalizedShort2: return 4;
                case VertexElementFormat.NormalizedShort4: return 8;
                case VertexElementFormat.HalfVector2: return 4;
                case VertexElementFormat.HalfVector4: return 8;
            }
            return 0;
        }
        //@impl VertexDeclaration..ctor(VertexElement[])
        public VertexDeclaration(params VertexElement[] elements) : this(default(StubCtor))
        {
            if (elements == null || elements.Length == 0) throw new ArgumentNullException("elements");
            _elements = (VertexElement[])elements.Clone();
            foreach (var e in elements) _stride = Math.Max(_stride, e.Offset + Size(e.VertexElementFormat));
        }
        //@impl VertexDeclaration..ctor(int, VertexElement[])
        public VertexDeclaration(int vertexStride, params VertexElement[] elements) : this(default(StubCtor))
        {
            if (elements == null || elements.Length == 0) throw new ArgumentNullException("elements");
            _elements = (VertexElement[])elements.Clone(); _stride = vertexStride;
        }
        //@impl VertexDeclaration.VertexStride
        public int VertexStride { get { return _stride; } }
        //@impl VertexDeclaration.GetVertexElements()
        public VertexElement[] GetVertexElements() { return (VertexElement[])_elements.Clone(); }
        //@impl VertexDeclaration.Dispose(bool)
        protected override void Dispose(bool disposing) { base.Dispose(disposing); }
    }

    public partial struct VertexPositionColorTexture
    {
        //@impl VertexPositionColorTexture.VertexDeclaration
        public static readonly VertexDeclaration VertexDeclaration = new VertexDeclaration(
            new VertexElement(0, VertexElementFormat.Vector3, VertexElementUsage.Position, 0),
            new VertexElement(12, VertexElementFormat.Color, VertexElementUsage.Color, 0),
            new VertexElement(16, VertexElementFormat.Vector2, VertexElementUsage.TextureCoordinate, 0));
        //@impl VertexPositionColorTexture..ctor(Vector3, Color, Vector2)
        public VertexPositionColorTexture(Vector3 position, Color color, Vector2 textureCoordinate) { Position = position; Color = color; TextureCoordinate = textureCoordinate; }
        //@impl VertexPositionColorTexture.IVertexType.get_VertexDeclaration()
        VertexDeclaration IVertexType.VertexDeclaration { get { return VertexDeclaration; } }
    }

    public partial struct VertexBufferBinding
    {
        //@impl VertexBufferBinding..ctor(VertexBuffer, int, int)
        public VertexBufferBinding(VertexBuffer vertexBuffer, int vertexOffset, int instanceFrequency) : this() { _vertexBuffer = vertexBuffer; _vertexOffset = vertexOffset; _instanceFrequency = instanceFrequency; }
        //@impl VertexBufferBinding..ctor(VertexBuffer, int)
        public VertexBufferBinding(VertexBuffer vertexBuffer, int vertexOffset) : this() { _vertexBuffer = vertexBuffer; _vertexOffset = vertexOffset; }
        //@impl VertexBufferBinding..ctor(VertexBuffer)
        public VertexBufferBinding(VertexBuffer vertexBuffer) : this() { _vertexBuffer = vertexBuffer; }
        //@impl VertexBufferBinding.InstanceFrequency
        public int InstanceFrequency { get { return _instanceFrequency; } }
        //@impl VertexBufferBinding.VertexOffset
        public int VertexOffset { get { return _vertexOffset; } }
        //@impl VertexBufferBinding.VertexBuffer
        public VertexBuffer VertexBuffer { get { return _vertexBuffer; } }
        //@impl VertexBufferBinding.op_Implicit(VertexBuffer)
        public static implicit operator VertexBufferBinding(VertexBuffer vertexBuffer) { return new VertexBufferBinding(vertexBuffer); }
    }

    public partial class VertexBuffer
    {
        internal VertexDeclaration _decl;
        internal int _count;
        internal BufferUsage _bu;
        internal void InitVb(GraphicsDevice d, VertexDeclaration decl, string what, int count, BufferUsage usage, bool dynamic)
        {
            if (d == null) throw new ArgumentNullException("graphicsDevice");
            _device = d; _decl = decl; _count = count; _bu = usage;
            RecLabel = "vb" + (Interlocked.Increment(ref Rec.NewVb) - 1);
            Rec.L("vb.new " + RecLabel + " " + what + " " + count + " " + (int)usage);
        }
        static VertexDeclaration DeclOf(Type t)
        {
            if (t == null) throw new ArgumentNullException("vertexType");
            object o = Activator.CreateInstance(t);
            var vt = o as IVertexType;
            if (vt == null) throw new ArgumentException("vertexType must implement IVertexType");
            return vt.VertexDeclaration;
        }
        //@impl VertexBuffer..ctor(GraphicsDevice, Type, int, BufferUsage)
        public VertexBuffer(GraphicsDevice graphicsDevice, Type vertexType, int vertexCount, BufferUsage usage) : this(default(StubCtor))
        { InitVb(graphicsDevice, DeclOf(vertexType), vertexType.FullName, vertexCount, usage, false); }
        //@impl VertexBuffer..ctor(GraphicsDevice, VertexDeclaration, int, BufferUsage)
        public VertexBuffer(GraphicsDevice graphicsDevice, VertexDeclaration vertexDeclaration, int vertexCount, BufferUsage usage) : this(default(StubCtor))
        { InitVb(graphicsDevice, vertexDeclaration, "decl" + vertexDeclaration.VertexStride, vertexCount, usage, false); }
        internal VertexBuffer(GraphicsDevice graphicsDevice, Type vertexType, int vertexCount, BufferUsage usage, bool dynamic) : this(default(StubCtor))
        { InitVb(graphicsDevice, DeclOf(vertexType), vertexType.FullName, vertexCount, usage, dynamic); }
        internal VertexBuffer(GraphicsDevice graphicsDevice, VertexDeclaration vertexDeclaration, int vertexCount, BufferUsage usage, bool dynamic) : this(default(StubCtor))
        { InitVb(graphicsDevice, vertexDeclaration, "decl" + vertexDeclaration.VertexStride, vertexCount, usage, dynamic); }
        //@impl VertexBuffer.VertexDeclaration
        public VertexDeclaration VertexDeclaration { get { return _decl; } }
        //@impl VertexBuffer.VertexCount
        public int VertexCount { get { return _count; } }
        //@impl VertexBuffer.BufferUsage
        public BufferUsage BufferUsage { get { return _bu; } }
        //@impl VertexBuffer.SetData<T>(T[])
        public void SetData<T>(T[] data) where T : struct { Rec.L("vb.data " + RecLabel + " 0 0 " + data.Length + " 0 " + Rec.Hash(data, 0, data.Length)); }
        //@impl VertexBuffer.SetData<T>(T[], int, int)
        public void SetData<T>(T[] data, int startIndex, int elementCount) where T : struct { Rec.L("vb.data " + RecLabel + " 0 " + startIndex + " " + elementCount + " 0 " + Rec.Hash(data, startIndex, elementCount)); }
        //@impl VertexBuffer.SetData<T>(int, T[], int, int, int)
        public void SetData<T>(int offsetInBytes, T[] data, int startIndex, int elementCount, int vertexStride) where T : struct { Rec.L("vb.data " + RecLabel + " " + offsetInBytes + " " + startIndex + " " + elementCount + " " + vertexStride + " " + Rec.Hash(data, startIndex, elementCount)); }
        //@impl VertexBuffer.Dispose(bool)
        protected override void Dispose(bool disposing) { base.Dispose(disposing); }
    }
    public partial class DynamicVertexBuffer
    {
        //@impl DynamicVertexBuffer..ctor(GraphicsDevice, Type, int, BufferUsage)
        public DynamicVertexBuffer(GraphicsDevice graphicsDevice, Type vertexType, int vertexCount, BufferUsage usage) : base(graphicsDevice, vertexType, vertexCount, usage, true) { }
        //@impl DynamicVertexBuffer..ctor(GraphicsDevice, VertexDeclaration, int, BufferUsage)
        public DynamicVertexBuffer(GraphicsDevice graphicsDevice, VertexDeclaration vertexDeclaration, int vertexCount, BufferUsage usage) : base(graphicsDevice, vertexDeclaration, vertexCount, usage, true) { }
        //@impl DynamicVertexBuffer.IsContentLost
        public bool IsContentLost { get { return false; } }
        //@impl DynamicVertexBuffer.ContentLost
        public event EventHandler<EventArgs> ContentLost { add { } remove { } }
        //@impl DynamicVertexBuffer.SetData<T>(T[], int, int, SetDataOptions)
        public void SetData<T>(T[] data, int startIndex, int elementCount, SetDataOptions options) where T : struct
        { Rec.L("vb.data " + RecLabel + " 0 " + startIndex + " " + elementCount + " 0 " + Rec.Hash(data, startIndex, elementCount)); }
        //@impl DynamicVertexBuffer.SetData<T>(int, T[], int, int, int, SetDataOptions)
        public void SetData<T>(int offsetInBytes, T[] data, int startIndex, int elementCount, int vertexStride, SetDataOptions options) where T : struct
        { Rec.L("vb.data " + RecLabel + " " + offsetInBytes + " " + startIndex + " " + elementCount + " " + vertexStride + " " + Rec.Hash(data, startIndex, elementCount)); }
    }

    public partial class IndexBuffer
    {
        internal IndexElementSize _size;
        internal int _count;
        internal BufferUsage _bu;
        internal void InitIb(GraphicsDevice d, IndexElementSize size, int count, BufferUsage usage, bool dynamic)
        {
            if (d == null) throw new ArgumentNullException("graphicsDevice");
            _device = d; _size = size; _count = count; _bu = usage;
            RecLabel = "ib" + (Interlocked.Increment(ref Rec.NewIb) - 1);
            Rec.L("ib.new " + RecLabel + " " + (int)size + " " + count + " " + (int)usage);
        }
        static IndexElementSize SizeOf(Type t)
        {
            if (t == typeof(short) || t == typeof(ushort)) return IndexElementSize.SixteenBits;
            if (t == typeof(int) || t == typeof(uint)) return IndexElementSize.ThirtyTwoBits;
            throw new ArgumentException("indexType");
        }
        //@impl IndexBuffer..ctor(GraphicsDevice, Type, int, BufferUsage)
        public IndexBuffer(GraphicsDevice graphicsDevice, Type indexType, int indexCount, BufferUsage usage) : this(default(StubCtor)) { InitIb(graphicsDevice, SizeOf(indexType), indexCount, usage, false); }
        //@impl IndexBuffer..ctor(GraphicsDevice, IndexElementSize, int, BufferUsage)
        public IndexBuffer(GraphicsDevice graphicsDevice, IndexElementSize indexElementSize, int indexCount, BufferUsage usage) : this(default(StubCtor)) { InitIb(graphicsDevice, indexElementSize, indexCount, usage, false); }
        internal IndexBuffer(GraphicsDevice graphicsDevice, IndexElementSize indexElementSize, int indexCount, BufferUsage usage, bool dynamic) : this(default(StubCtor)) { InitIb(graphicsDevice, indexElementSize, indexCount, usage, dynamic); }
        //@impl IndexBuffer.IndexCount
        public int IndexCount { get { return _count; } }
        //@impl IndexBuffer.IndexElementSize
        public IndexElementSize IndexElementSize { get { return _size; } }
        //@impl IndexBuffer.BufferUsage
        public BufferUsage BufferUsage { get { return _bu; } }
        //@impl IndexBuffer.SetData<T>(T[])
        public void SetData<T>(T[] data) where T : struct { Rec.L("ib.data " + RecLabel + " 0 0 " + data.Length + " " + Rec.Hash(data, 0, data.Length)); }
        //@impl IndexBuffer.SetData<T>(T[], int, int)
        public void SetData<T>(T[] data, int startIndex, int elementCount) where T : struct { Rec.L("ib.data " + RecLabel + " 0 " + startIndex + " " + elementCount + " " + Rec.Hash(data, startIndex, elementCount)); }
        //@impl IndexBuffer.SetData<T>(int, T[], int, int)
        public void SetData<T>(int offsetInBytes, T[] data, int startIndex, int elementCount) where T : struct { Rec.L("ib.data " + RecLabel + " " + offsetInBytes + " " + startIndex + " " + elementCount + " " + Rec.Hash(data, startIndex, elementCount)); }
        //@impl IndexBuffer.Dispose(bool)
        protected override void Dispose(bool disposing) { base.Dispose(disposing); }
    }
    public partial class DynamicIndexBuffer
    {
        //@impl DynamicIndexBuffer..ctor(GraphicsDevice, Type, int, BufferUsage)
        public DynamicIndexBuffer(GraphicsDevice graphicsDevice, Type indexType, int indexCount, BufferUsage usage)
            : base(graphicsDevice, indexType == typeof(int) || indexType == typeof(uint) ? IndexElementSize.ThirtyTwoBits : IndexElementSize.SixteenBits, indexCount, usage, true) { }
        //@impl DynamicIndexBuffer..ctor(GraphicsDevice, IndexElementSize, int, BufferUsage)
        public DynamicIndexBuffer(GraphicsDevice graphicsDevice, IndexElementSize indexElementSize, int indexCount, BufferUsage usage) : base(graphicsDevice, indexElementSize, indexCount, usage, true) { }
        //@impl DynamicIndexBuffer.IsContentLost
        public bool IsContentLost { get { return false; } }
        //@impl DynamicIndexBuffer.ContentLost
        public event EventHandler<EventArgs> ContentLost { add { } remove { } }
        //@impl DynamicIndexBuffer.SetData<T>(T[], int, int, SetDataOptions)
        public void SetData<T>(T[] data, int startIndex, int elementCount, SetDataOptions options) where T : struct
        { Rec.L("ib.data " + RecLabel + " 0 " + startIndex + " " + elementCount + " " + Rec.Hash(data, startIndex, elementCount)); }
        //@impl DynamicIndexBuffer.SetData<T>(int, T[], int, int, SetDataOptions)
        public void SetData<T>(int offsetInBytes, T[] data, int startIndex, int elementCount, SetDataOptions options) where T : struct
        { Rec.L("ib.data " + RecLabel + " " + offsetInBytes + " " + startIndex + " " + elementCount + " " + Rec.Hash(data, startIndex, elementCount)); }
    }
}
