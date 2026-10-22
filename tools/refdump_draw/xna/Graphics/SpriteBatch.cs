// XNA stand-in: SpriteBatch. Records Begin / Draw / End with the arguments as given; keeps XNA's observable state
// (nesting rules, the device states SpriteBatch sets in Begin(Immediate) and End, Textures[0] after a flush, the
// private queue the game reads by reflection: Terraria.Graphics.GraphicsUtils).
using System;
using System.Text;
using Microsoft.Xna.Framework.XnaStandIn;

namespace Microsoft.Xna.Framework.Graphics
{
    public partial class SpriteBatch
    {
        // read by the game through reflection (GraphicsUtils.PendingDrawCallCount)
        int spriteQueueCount;
        Texture2D[] spriteTextures = new Texture2D[2048];

        SpriteSortMode _sort;
        BlendState _blend;
        SamplerState _sampler;
        DepthStencilState _depth;
        RasterizerState _raster;
        Effect _effect;
        Matrix _matrix;
        bool _inBegin;
        static int _beginCount, _immediateCount;

        //@impl SpriteBatch..ctor(GraphicsDevice)
        public SpriteBatch(GraphicsDevice graphicsDevice) : this(default(StubCtor))
        {
            if (graphicsDevice == null) throw new ArgumentNullException("graphicsDevice", "The GraphicsDevice must not be null when creating new resources.");
            _device = graphicsDevice;
            RecLabel = "sb" + (System.Threading.Interlocked.Increment(ref Rec.NewSb) - 1);
            Rec.L("sb.new " + RecLabel);
        }
        //@impl SpriteBatch.Dispose(bool)
        protected override void Dispose(bool disposing) { base.Dispose(disposing); }

        void BeginImpl(int ov, SpriteSortMode sortMode, BlendState blendState, SamplerState samplerState, DepthStencilState depthStencilState, RasterizerState rasterizerState, Effect effect, Matrix transformMatrix)
        {
            Rec.L("sb.begin " + RecLabel + " " + ov + " " + (int)sortMode + " " + Rec.S(blendState) + " " + Rec.S(samplerState) + " " + Rec.S(depthStencilState) + " " + Rec.S(rasterizerState) + " "
                + Rec.E(effect) + " " + (ov == 4 ? Rec.M(transformMatrix) : "-"));
            if (_inBegin) throw new InvalidOperationException("Begin cannot be called again until End has been successfully called.");
            if (sortMode < SpriteSortMode.Deferred || sortMode > SpriteSortMode.FrontToBack) throw new ArgumentOutOfRangeException("sortMode");
            _sort = sortMode; _blend = blendState; _sampler = samplerState; _depth = depthStencilState; _raster = rasterizerState; _effect = effect; _matrix = transformMatrix;
            if (sortMode == SpriteSortMode.Immediate)
            {
                if (_beginCount > 0) throw new InvalidOperationException("Cannot nest Begin calls on a single SpriteBatch when using SpriteSortMode.Immediate.");
                SetRenderState();
                _immediateCount++;
            }
            else if (_immediateCount > 0) throw new InvalidOperationException("Cannot nest Begin calls on a single SpriteBatch when using SpriteSortMode.Immediate.");
            _beginCount++;
            _inBegin = true;
        }
        //@impl SpriteBatch.Begin()
        public void Begin() { BeginImpl(0, SpriteSortMode.Deferred, null, null, null, null, null, Matrix.Identity); }
        //@impl SpriteBatch.Begin(SpriteSortMode, BlendState)
        public void Begin(SpriteSortMode sortMode, BlendState blendState) { BeginImpl(1, sortMode, blendState, null, null, null, null, Matrix.Identity); }
        //@impl SpriteBatch.Begin(SpriteSortMode, BlendState, SamplerState, DepthStencilState, RasterizerState)
        public void Begin(SpriteSortMode sortMode, BlendState blendState, SamplerState samplerState, DepthStencilState depthStencilState, RasterizerState rasterizerState)
        { BeginImpl(2, sortMode, blendState, samplerState, depthStencilState, rasterizerState, null, Matrix.Identity); }
        //@impl SpriteBatch.Begin(SpriteSortMode, BlendState, SamplerState, DepthStencilState, RasterizerState, Effect)
        public void Begin(SpriteSortMode sortMode, BlendState blendState, SamplerState samplerState, DepthStencilState depthStencilState, RasterizerState rasterizerState, Effect effect)
        { BeginImpl(3, sortMode, blendState, samplerState, depthStencilState, rasterizerState, effect, Matrix.Identity); }
        //@impl SpriteBatch.Begin(SpriteSortMode, BlendState, SamplerState, DepthStencilState, RasterizerState, Effect, Matrix)
        public void Begin(SpriteSortMode sortMode, BlendState blendState, SamplerState samplerState, DepthStencilState depthStencilState, RasterizerState rasterizerState, Effect effect, Matrix transformMatrix)
        { BeginImpl(4, sortMode, blendState, samplerState, depthStencilState, rasterizerState, effect, transformMatrix); }

        //@impl SpriteBatch.End()
        public void End()
        {
            Rec.L("sb.end " + RecLabel);
            if (!_inBegin) throw new InvalidOperationException("Begin must be called before calling End.");
            if (_sort != SpriteSortMode.Immediate) SetRenderState();
            else _immediateCount--;
            if (spriteQueueCount > 0) Flush();
            _inBegin = false;
            _beginCount--;
        }

        // XNA SpriteBatch.SetRenderState: the batch's states (defaults for null) onto the device. Not recorded: the game did not make these calls.
        void SetRenderState()
        {
            GraphicsDevice d = _device;
            (d._blend = _blend ?? BlendState.AlphaBlend)._bound = true;
            (d._depthState = _depth ?? DepthStencilState.None)._bound = true;
            (d._raster = _raster ?? RasterizerState.CullCounterClockwise)._bound = true;
            (d._samplers[0] = _sampler ?? SamplerState.LinearClamp)._bound = true;
        }

        void Flush()
        {
            // Textures[0] is the texture of the last texture run drawn (deferred / immediate order: the last queued sprite)
            _device._textures[0] = spriteTextures[spriteQueueCount - 1];
            Array.Clear(spriteTextures, 0, spriteQueueCount);
            spriteQueueCount = 0;
        }

        void Queue(Texture2D texture)
        {
            if (texture == null) throw new ArgumentNullException("texture", "This method does not accept null for this parameter.");
            if (!_inBegin) throw new InvalidOperationException("Begin must be called successfully before a Draw can be called.");
            if (spriteQueueCount >= spriteTextures.Length) Array.Resize(ref spriteTextures, spriteTextures.Length * 2);
            spriteTextures[spriteQueueCount++] = texture;
            if (_sort == SpriteSortMode.Immediate) Flush();
        }

        string Head(string ov, Texture2D t) { return "sb.draw " + RecLabel + " " + ov + " " + Rec.T(t) + " "; }

        //@impl SpriteBatch.Draw(Texture2D, Vector2, Color)
        public void Draw(Texture2D texture, Vector2 position, Color color)
        {
            if (Rec.Active) Rec.L(Head("p", texture) + Rec.V(position) + " " + Rec.C(color));
            Queue(texture);
        }
        //@impl SpriteBatch.Draw(Texture2D, Vector2, Rectangle?, Color)
        public void Draw(Texture2D texture, Vector2 position, Rectangle? sourceRectangle, Color color)
        {
            if (Rec.Active) Rec.L(Head("ps", texture) + Rec.V(position) + " " + Rec.R(sourceRectangle) + " " + Rec.C(color));
            Queue(texture);
        }
        //@impl SpriteBatch.Draw(Texture2D, Vector2, Rectangle?, Color, float, Vector2, float, SpriteEffects, float)
        public void Draw(Texture2D texture, Vector2 position, Rectangle? sourceRectangle, Color color, float rotation, Vector2 origin, float scale, SpriteEffects effects, float layerDepth)
        {
            if (Rec.Active) Rec.L(Head("pf", texture) + Rec.V(position) + " " + Rec.R(sourceRectangle) + " " + Rec.C(color) + " " + Rec.F(rotation) + " " + Rec.V(origin) + " " + Rec.F(scale) + " " + (int)effects + " " + Rec.F(layerDepth));
            Queue(texture);
        }
        //@impl SpriteBatch.Draw(Texture2D, Vector2, Rectangle?, Color, float, Vector2, Vector2, SpriteEffects, float)
        public void Draw(Texture2D texture, Vector2 position, Rectangle? sourceRectangle, Color color, float rotation, Vector2 origin, Vector2 scale, SpriteEffects effects, float layerDepth)
        {
            if (Rec.Active) Rec.L(Head("pv", texture) + Rec.V(position) + " " + Rec.R(sourceRectangle) + " " + Rec.C(color) + " " + Rec.F(rotation) + " " + Rec.V(origin) + " " + Rec.V(scale) + " " + (int)effects + " " + Rec.F(layerDepth));
            Queue(texture);
        }
        //@impl SpriteBatch.Draw(Texture2D, Rectangle, Color)
        public void Draw(Texture2D texture, Rectangle destinationRectangle, Color color)
        {
            if (Rec.Active) Rec.L(Head("r", texture) + Rec.R(destinationRectangle) + " " + Rec.C(color));
            Queue(texture);
        }
        //@impl SpriteBatch.Draw(Texture2D, Rectangle, Rectangle?, Color)
        public void Draw(Texture2D texture, Rectangle destinationRectangle, Rectangle? sourceRectangle, Color color)
        {
            if (Rec.Active) Rec.L(Head("rs", texture) + Rec.R(destinationRectangle) + " " + Rec.R(sourceRectangle) + " " + Rec.C(color));
            Queue(texture);
        }
        //@impl SpriteBatch.Draw(Texture2D, Rectangle, Rectangle?, Color, float, Vector2, SpriteEffects, float)
        public void Draw(Texture2D texture, Rectangle destinationRectangle, Rectangle? sourceRectangle, Color color, float rotation, Vector2 origin, SpriteEffects effects, float layerDepth)
        {
            if (Rec.Active) Rec.L(Head("rf", texture) + Rec.R(destinationRectangle) + " " + Rec.R(sourceRectangle) + " " + Rec.C(color) + " " + Rec.F(rotation) + " " + Rec.V(origin) + " " + (int)effects + " " + Rec.F(layerDepth));
            Queue(texture);
        }
    }
}
