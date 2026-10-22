// The draw-call recorder of the XNA stand-in (compiled into the stand-in Microsoft.Xna.Framework.Graphics.dll).
//
// Every call the game makes at the XNA graphics API boundary that changes what is drawn is written as one text line
// (port side: port\crates\terraria-host\src\gfx\trace.rs writes the same lines; the format is defined in both files
// and in tools\cs2rs\VERIFICATION.md "Draw output"). Lines are written only on the recording thread (the game's main
// thread) and only while On; resource labels are assigned always (textures created before recording starts keep
// their names).
//
// Values: int decimal; float = its 32 bits as 8 lower-case hex digits; Color = PackedValue (R in the low byte) as 8 hex
// digits; Vector2 = x,y; Rectangle = x,y,w,h; a missing (null) value = '-'.
// Resources: content textures and effects by their name in the game's asset repository (ReLogic AssetRepository,
// '/' separators; the pages of a DynamicSpriteFont <font>#<k>, k in order of first use in its character table), textures the game creates tex<k>, render targets rt<k>, sprite batches sb<k>,
// vertex / index buffers vb<k> / ib<k> (k = creation order from 0), effects by asset path.
// States: blend b:cs,cd,cf,as,ad,af,factor,mask ; sampler s:filter,u,v,w,aniso,maxmip,bias ;
//         rasterizer r:cull,fill,scissor,msaa,depthbias,slope ; depth z:enable,write,func,stencil
using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using Microsoft.Xna.Framework.Graphics;

namespace Microsoft.Xna.Framework.XnaStandIn
{
    public static class Rec
    {
        public static TextWriter Out;
        /// <summary>Lines are produced (digest of the frame); Full: also written to Out.</summary>
        public static bool Active, Full;
        public static ulong FrameHash = FnvBasis;
        public static long FrameLines;
        public static int MainThread = -1;
        /// <summary>The content file the current thread opened last (TitleContainer.OpenStream, set by the harness).</summary>
        [ThreadStatic] public static string CurrentAsset;
        [ThreadStatic] public static int AssetObjects;
        public static int NewTex, NewRt, NewSb, NewVb, NewIb;
        public static long Lines;

        public static bool OnMain { get { return Thread.CurrentThread.ManagedThreadId == MainThread; } }

        public static void L(string s)
        {
            if (!Active || !OnMain) return;
            ulong h = FrameHash;
            for (int i = 0; i < s.Length; i++) { h ^= (byte)s[i]; h *= FnvPrime; }
            h ^= 10; h *= FnvPrime;
            FrameHash = h;
            FrameLines++;
            if (!Full) return;
            Out.Write(s);
            Out.Write('\n');
            Lines++;
        }
        /// <summary>Starts a frame's digest.</summary>
        public static void FrameStart() { FrameHash = FnvBasis; FrameLines = 0; }

        /// <summary>The label of the next content object read from the current asset file.</summary>
        public static string ContentLabel()
        {
            string a = CurrentAsset;
            string label = a == null ? "content?" : a;
            int k = AssetObjects++;
            return k == 0 ? label : label + "#" + k;
        }

        /// <summary>A path below the install's Content folder as an asset label.</summary>
        public static string AssetLabel(string path)
        {
            string p = path.Replace('\\', '/');
            int i = p.LastIndexOf("Content/", StringComparison.OrdinalIgnoreCase);
            if (i >= 0) p = p.Substring(i + 8);
            if (p.EndsWith(".xnb", StringComparison.OrdinalIgnoreCase)) p = p.Substring(0, p.Length - 4);
            return p;
        }

        public static string F(float f) { return ((uint)BitConverter.ToInt32(BitConverter.GetBytes(f), 0)).ToString("x8"); }
        public static string C(Color c) { return c.PackedValue.ToString("x8"); }
        public static string V(Vector2 v) { return F(v.X) + "," + F(v.Y); }
        public static string V(Vector3 v) { return F(v.X) + "," + F(v.Y) + "," + F(v.Z); }
        public static string V(Vector4 v) { return F(v.X) + "," + F(v.Y) + "," + F(v.Z) + "," + F(v.W); }
        public static string R(Rectangle r) { return r.X + "," + r.Y + "," + r.Width + "," + r.Height; }
        public static string R(Rectangle? r) { return r.HasValue ? R(r.Value) : "-"; }
        public static string M(Matrix m)
        {
            return F(m.M11) + "," + F(m.M12) + "," + F(m.M13) + "," + F(m.M14) + "," + F(m.M21) + "," + F(m.M22) + "," + F(m.M23) + "," + F(m.M24) + ","
                + F(m.M31) + "," + F(m.M32) + "," + F(m.M33) + "," + F(m.M34) + "," + F(m.M41) + "," + F(m.M42) + "," + F(m.M43) + "," + F(m.M44);
        }
        /// <summary>Names content objects that have no label yet (the harness walks the game's asset repository).</summary>
        public static Action Missing;
        public static string T(Texture t)
        {
            if (t == null) return "-";
            if (t.RecLabel == null && Missing != null) Missing();
            return t.RecLabel ?? "?";
        }
        public static string B(bool b) { return b ? "1" : "0"; }

        public static string S(BlendState s)
        {
            if (s == null) return "-";
            return "b:" + (int)s.ColorSourceBlend + "," + (int)s.ColorDestinationBlend + "," + (int)s.ColorBlendFunction + "," + (int)s.AlphaSourceBlend + ","
                + (int)s.AlphaDestinationBlend + "," + (int)s.AlphaBlendFunction + "," + C(s.BlendFactor) + "," + (int)s.ColorWriteChannels;
        }
        public static string S(SamplerState s)
        {
            if (s == null) return "-";
            return "s:" + (int)s.Filter + "," + (int)s.AddressU + "," + (int)s.AddressV + "," + (int)s.AddressW + "," + s.MaxAnisotropy + "," + s.MaxMipLevel + "," + F(s.MipMapLevelOfDetailBias);
        }
        public static string S(RasterizerState s)
        {
            if (s == null) return "-";
            return "r:" + (int)s.CullMode + "," + (int)s.FillMode + "," + B(s.ScissorTestEnable) + "," + B(s.MultiSampleAntiAlias) + "," + F(s.DepthBias) + "," + F(s.SlopeScaleDepthBias);
        }
        public static string S(DepthStencilState s)
        {
            if (s == null) return "-";
            return "z:" + B(s.DepthBufferEnable) + "," + B(s.DepthBufferWriteEnable) + "," + (int)s.DepthBufferFunction + "," + B(s.StencilEnable);
        }
        public static string E(Effect e)
        {
            if (e == null) return "-";
            if (e.RecLabel == null && Missing != null) Missing();
            return e.RecLabel ?? "?";
        }

        // ---------------------------------------------------------------- hashing (FNV-1a 64 over the element bytes)
        public const ulong FnvBasis = 14695981039346656037UL, FnvPrime = 1099511628211UL;
        public static readonly bool Raw = Environment.GetEnvironmentVariable("DRAW_RAW") == "1";
        public static string Hash<T>(T[] data, int start, int count) where T : struct
        {
            if (data == null) return "-";
            int size = Marshal.SizeOf(typeof(T));
            if (start < 0 || count < 0 || start + count > data.Length) return "range";
            byte[] b = new byte[size * count];
            if (count > 0)
            {
                GCHandle h = GCHandle.Alloc(data, GCHandleType.Pinned);
                try { Marshal.Copy(new IntPtr(h.AddrOfPinnedObject().ToInt64() + (long)start * size), b, 0, b.Length); }
                finally { h.Free(); }
            }
            ulong x = FnvBasis;
            foreach (byte v in b) { x ^= v; x *= FnvPrime; }
            // DRAW_RAW=1 (diagnosis, not comparable with the port): the bytes themselves
            if (Raw) { var sb = new StringBuilder(); foreach (byte v in b) sb.Append(v.ToString("x2")); return size + ":" + sb; }
            return size + ":" + x.ToString("x16");
        }

        // ---------------------------------------------------------------- recording-only depth guard
        /// <summary>Calls the stand-in makes on itself (none record; kept for symmetry with the port's guard).</summary>
        [ThreadStatic] public static int Inside;
    }
}
