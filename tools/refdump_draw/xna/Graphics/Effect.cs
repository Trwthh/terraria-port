// XNA stand-in: Effect and its collections. The compiled effect (D3DX fx_2_0, optionally behind XNA's 0xBCF00BCF
// header) is read for names only: parameters, techniques, passes (port\crates\terraria-host\src\gfx\fx_binary.rs
// parse_full walks the same tables). Parameter values, CurrentTechnique and EffectPass.Apply are recorded.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Text;
using Microsoft.Xna.Framework.XnaStandIn;

namespace Microsoft.Xna.Framework.Graphics
{
    public partial class Effect
    {
        internal EffectParameterCollection _params;
        internal EffectTechniqueCollection _techniques;
        EffectTechnique _current;

        //@impl Effect..ctor(GraphicsDevice, byte[])
        public Effect(GraphicsDevice graphicsDevice, byte[] effectCode) : this(default(StubCtor))
        {
            if (graphicsDevice == null) throw new ArgumentNullException("graphicsDevice");
            _device = graphicsDevice;
            var ps = new List<EffectParameter>();
            var ts = new List<EffectTechnique>();
            Parse(effectCode, ps, ts);
            _params = new EffectParameterCollection(ps);
            _techniques = new EffectTechniqueCollection(ts);
            _current = ts[0];
        }

        static uint U(byte[] b, int at) { return BitConverter.ToUInt32(b, at); }
        static string Str(byte[] b, int bas, uint off)
        {
            int at = bas + (int)off;
            int len = (int)U(b, at);
            if (len == 0) return "";
            int n = len; if (b[at + 4 + n - 1] == 0) n--;
            return Encoding.ASCII.GetString(b, at + 4, n);
        }
        void Parse(byte[] code, List<EffectParameter> ps, List<EffectTechnique> ts)
        {
            int start = 0;
            if (U(code, 0) == 0xBCF00BCF) start = (int)U(code, 4);
            if (U(code, start) != 0xFEFF0901) throw new ArgumentException("not a D3DX effect");
            int bas = start + 8;
            int c = bas + (int)U(code, start + 4);
            uint np = U(code, c), nt = U(code, c + 4); c += 16;
            for (uint i = 0; i < np; i++)
            {
                uint typeOff = U(code, c), na = U(code, c + 12); c += 16 + 8 * (int)na;
                int t = bas + (int)typeOff;
                string name = Str(code, bas, U(code, t + 8));
                ps.Add(new EffectParameter(this, name, (EffectParameterType)U(code, t), (EffectParameterClass)U(code, t + 4)));
            }
            for (uint i = 0; i < nt; i++)
            {
                uint nameOff = U(code, c), na = U(code, c + 4), npass = U(code, c + 8); c += 12 + 8 * (int)na;
                var passes = new List<EffectPass>();
                var tech = new EffectTechnique(Str(code, bas, nameOff));
                for (uint k = 0; k < npass; k++)
                {
                    uint pn = U(code, c), pa = U(code, c + 4), states = U(code, c + 8); c += 12 + 8 * (int)pa + 16 * (int)states;
                    passes.Add(new EffectPass(this, tech, Str(code, bas, pn)));
                }
                tech._passes = new EffectPassCollection(passes);
                ts.Add(tech);
            }
        }

        //@impl Effect.Parameters
        public EffectParameterCollection Parameters { get { return _params; } }
        //@impl Effect.Techniques
        public EffectTechniqueCollection Techniques { get { return _techniques; } }
        //@impl Effect.CurrentTechnique
        public EffectTechnique CurrentTechnique
        {
            get { return _current; }
            set
            {
                Rec.L("fx.technique " + Rec.E(this) + " " + (value == null ? "-" : value.Name));
                if (value == null) throw new InvalidOperationException("CurrentTechnique must be set to a valid technique of this effect.");
                _current = value;
            }
        }
        //@impl Effect.Dispose(bool)
        protected override void Dispose(bool disposing) { base.Dispose(disposing); }
    }

    public partial class EffectParameter
    {
        readonly Effect _owner;
        readonly string _name;
        readonly EffectParameterType _type;
        readonly EffectParameterClass _class;
        internal EffectParameter(Effect owner, string name, EffectParameterType type, EffectParameterClass cls) : this(default(StubCtor))
        { _owner = owner; _name = name; _type = type; _class = cls; }
        //@impl EffectParameter.Name
        public string Name { get { return _name; } }
        //@impl EffectParameter.ParameterType
        public EffectParameterType ParameterType { get { return _type; } }
        //@impl EffectParameter.ParameterClass
        public EffectParameterClass ParameterClass { get { return _class; } }
        void P(string kind, string v) { Rec.L("fx.param " + Rec.E(_owner) + " " + _name + " " + kind + " " + v); }
        //@impl EffectParameter.SetValue(Texture)
        public void SetValue(Texture value) { P("t", Rec.T(value)); }
        //@impl EffectParameter.SetValue(Matrix)
        public void SetValue(Matrix value) { P("m", Rec.M(value)); }
        //@impl EffectParameter.SetValue(Vector4)
        public void SetValue(Vector4 value) { P("v4", Rec.V(value)); }
        //@impl EffectParameter.SetValue(Vector3)
        public void SetValue(Vector3 value) { P("v3", Rec.V(value)); }
        //@impl EffectParameter.SetValue(Vector2)
        public void SetValue(Vector2 value) { P("v2", Rec.V(value)); }
        //@impl EffectParameter.SetValue(float)
        public void SetValue(float value) { P("f", Rec.F(value)); }
        //@impl EffectParameter.SetValue(int)
        public void SetValue(int value) { P("i", value.ToString()); }
        //@impl EffectParameter.SetValue(bool)
        public void SetValue(bool value) { P("b", Rec.B(value)); }
        //@impl EffectParameter.SetValue(float[])
        public void SetValue(float[] value)
        {
            var sb = new StringBuilder();
            for (int i = 0; i < value.Length; i++) { if (i > 0) sb.Append(','); sb.Append(Rec.F(value[i])); }
            P("fa", sb.ToString());
        }
        //@impl EffectParameter.SetValue(Vector2[])
        public void SetValue(Vector2[] value)
        {
            var sb = new StringBuilder();
            for (int i = 0; i < value.Length; i++) { if (i > 0) sb.Append(';'); sb.Append(Rec.V(value[i])); }
            P("v2a", sb.ToString());
        }
        //@impl EffectParameter.SetValue(Vector4[])
        public void SetValue(Vector4[] value)
        {
            var sb = new StringBuilder();
            for (int i = 0; i < value.Length; i++) { if (i > 0) sb.Append(';'); sb.Append(Rec.V(value[i])); }
            P("v4a", sb.ToString());
        }
    }

    public partial class EffectParameterCollection
    {
        readonly List<EffectParameter> _l;
        internal EffectParameterCollection(List<EffectParameter> l) : this(default(StubCtor)) { _l = l; }
        //@impl EffectParameterCollection.this[int]
        public EffectParameter this[int index] { get { return index >= 0 && index < _l.Count ? _l[index] : null; } }
        //@impl EffectParameterCollection.this[string]
        public EffectParameter this[string name] { get { foreach (var p in _l) if (p.Name == name) return p; return null; } }
        //@impl EffectParameterCollection.Count
        public int Count { get { return _l.Count; } }
        //@impl EffectParameterCollection.GetEnumerator()
        public List<EffectParameter>.Enumerator GetEnumerator() { return _l.GetEnumerator(); }
        //@impl EffectParameterCollection.IEnumerable<EffectParameter>.GetEnumerator()
        IEnumerator<EffectParameter> IEnumerable<EffectParameter>.GetEnumerator() { return _l.GetEnumerator(); }
        //@impl EffectParameterCollection.IEnumerable.GetEnumerator()
        IEnumerator IEnumerable.GetEnumerator() { return _l.GetEnumerator(); }
    }

    public partial class EffectTechnique
    {
        readonly string _name;
        internal EffectPassCollection _passes;
        internal EffectTechnique(string name) : this(default(StubCtor)) { _name = name; }
        //@impl EffectTechnique.Name
        public string Name { get { return _name; } }
        //@impl EffectTechnique.Passes
        public EffectPassCollection Passes { get { return _passes; } }
    }

    public partial class EffectTechniqueCollection
    {
        readonly List<EffectTechnique> _l;
        internal EffectTechniqueCollection(List<EffectTechnique> l) : this(default(StubCtor)) { _l = l; }
        //@impl EffectTechniqueCollection.this[int]
        public EffectTechnique this[int index] { get { return index >= 0 && index < _l.Count ? _l[index] : null; } }
        //@impl EffectTechniqueCollection.this[string]
        public EffectTechnique this[string name] { get { foreach (var t in _l) if (t.Name == name) return t; return null; } }
        //@impl EffectTechniqueCollection.Count
        public int Count { get { return _l.Count; } }
        //@impl EffectTechniqueCollection.GetEnumerator()
        public List<EffectTechnique>.Enumerator GetEnumerator() { return _l.GetEnumerator(); }
        //@impl EffectTechniqueCollection.IEnumerable<EffectTechnique>.GetEnumerator()
        IEnumerator<EffectTechnique> IEnumerable<EffectTechnique>.GetEnumerator() { return _l.GetEnumerator(); }
        //@impl EffectTechniqueCollection.IEnumerable.GetEnumerator()
        IEnumerator IEnumerable.GetEnumerator() { return _l.GetEnumerator(); }
    }

    public partial class EffectPass
    {
        readonly Effect _effect;
        readonly EffectTechnique _technique;
        readonly string _name;
        internal EffectPass(Effect effect, EffectTechnique technique, string name) : this(default(StubCtor)) { _effect = effect; _technique = technique; _name = name; }
        //@impl EffectPass.Name
        public string Name { get { return _name; } }
        //@impl EffectPass.Apply()
        public void Apply() { Rec.L("fx.apply " + Rec.E(_effect) + " " + _technique.Name + " " + _name); }
    }

    public partial class EffectPassCollection
    {
        readonly List<EffectPass> _l;
        internal EffectPassCollection(List<EffectPass> l) : this(default(StubCtor)) { _l = l; }
        //@impl EffectPassCollection.this[int]
        public EffectPass this[int index] { get { return index >= 0 && index < _l.Count ? _l[index] : null; } }
        //@impl EffectPassCollection.this[string]
        public EffectPass this[string name] { get { foreach (var p in _l) if (p.Name == name) return p; return null; } }
        //@impl EffectPassCollection.Count
        public int Count { get { return _l.Count; } }
        //@impl EffectPassCollection.GetEnumerator()
        public List<EffectPass>.Enumerator GetEnumerator() { return _l.GetEnumerator(); }
        //@impl EffectPassCollection.IEnumerable<EffectPass>.GetEnumerator()
        IEnumerator<EffectPass> IEnumerable<EffectPass>.GetEnumerator() { return _l.GetEnumerator(); }
        //@impl EffectPassCollection.IEnumerable.GetEnumerator()
        IEnumerator IEnumerable.GetEnumerator() { return _l.GetEnumerator(); }
    }
}

namespace Microsoft.Xna.Framework.Content
{
    using Microsoft.Xna.Framework.Graphics;

    // The XNB readers the content pipeline names for the game's textures and effects (found by XNA's ContentTypeReaderManager by name).
    internal class Texture2DReader : ContentTypeReader<Texture2D>
    {
        protected internal override Texture2D Read(ContentReader input, Texture2D existingInstance)
        {
            SurfaceFormat format = (SurfaceFormat)input.ReadInt32();
            int width = input.ReadInt32(), height = input.ReadInt32(), levels = input.ReadInt32();
            for (int i = 0; i < levels; i++) { int size = input.ReadInt32(); input.ReadBytes(size); }
            var service = (IGraphicsDeviceService)input.ContentManager.ServiceProvider.GetService(typeof(IGraphicsDeviceService));
            return new Texture2D(service.GraphicsDevice, width, height, levels, format, null);
        }
    }

    internal class EffectReader : ContentTypeReader<Effect>
    {
        protected internal override Effect Read(ContentReader input, Effect existingInstance)
        {
            int size = input.ReadInt32();
            byte[] code = input.ReadBytes(size);
            var service = (IGraphicsDeviceService)input.ContentManager.ServiceProvider.GetService(typeof(IGraphicsDeviceService));
            return new Effect(service.GraphicsDevice, code);
        }
    }
}
