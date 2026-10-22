using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;

namespace Terraria.Graphics.Shaders
{
	public class ScreenShaderData : ShaderData
	{
		private Vector3 _uColor = Vector3.One;

		private Vector3 _uSecondaryColor = Vector3.One;

		private float _uOpacity = 1f;

		private float _globalOpacity = 1f;

		private float _uIntensity = 1f;

		private Vector2 _uTargetPosition = Vector2.One;

		private Vector2 _uDirection = new Vector2(0f, 1f);

		private float _uProgress;

		private Vector2 _uImageOffset = Vector2.Zero;

		private Vector2 _uSceneSize;

		private Vector2 _uSceneOffset;

		private Vector2 _uImageSize0;

		private Asset<Texture2D>[] _uAssetImages = new Asset<Texture2D>[3];

		private Texture2D[] _uCustomImages = new Texture2D[3];

		private SamplerState[] _samplerStates = new SamplerState[3];

		private Vector2[] _imageScales = new Vector2[3]
		{
			Vector2.One,
			Vector2.One,
			Vector2.One
		};

		public static bool MultiChunkCapture;

		private Effect _effect;

		private EffectParameter<Vector3> uColor;

		private EffectParameter<float> uOpacity;

		private EffectParameter<Vector3> uSecondaryColor;

		private EffectParameter<float> uTime;

		private EffectParameter<Vector2> uScreenResolution;

		private EffectParameter<Vector2> uScreenPosition;

		private EffectParameter<Vector2> uTargetPosition;

		private EffectParameter<Vector2> uImageOffset;

		private EffectParameter<Vector2> uSceneSize;

		private EffectParameter<Vector2> uSceneOffset;

		private EffectParameter<float> uIntensity;

		private EffectParameter<float> uProgress;

		private EffectParameter<Vector2> uDirection;

		private EffectParameter<Vector2> uZoom;

		private EffectParameter<Vector2>[] uImageSize = new EffectParameter<Vector2>[4];

		private EffectParameter<bool> uMultiChunkScene;

		public float Intensity => _uIntensity;

		public float CombinedOpacity => _uOpacity * _globalOpacity;

		public static Vector2 UnscaledScreenPosition
		{
			get
			{
				Matrix effectMatrix = Main.GameViewMatrix.EffectMatrix;
				Matrix transformationMatrix = Main.GameViewMatrix.TransformationMatrix;
				return Main.screenPosition + new Vector2(effectMatrix.M41 - transformationMatrix.M41, effectMatrix.M42 - transformationMatrix.M42) / new Vector2(transformationMatrix.M11, transformationMatrix.M22);
			}
		}

		public static Vector2 UnscaledScreenSize => new Vector2(Main.screenWidth, Main.screenHeight) / Main.GameViewMatrix.RenderZoom;

		public ScreenShaderData(string passName)
			: base(Main.ScreenShaderRef, passName)
		{
		}

		public ScreenShaderData(Asset<Effect> shader, string passName)
			: base(shader, passName)
		{
		}

		public virtual void Update(GameTime gameTime)
		{
		}

		private void CheckCachedParameters()
		{
			if (_effect == null || _effect != base.Shader)
			{
				_effect = base.Shader;
				uColor = base.Shader.GetParameter<Vector3>("uColor");
				uOpacity = base.Shader.GetParameter<float>("uOpacity");
				uSecondaryColor = base.Shader.GetParameter<Vector3>("uSecondaryColor");
				uTime = base.Shader.GetParameter<float>("uTime");
				uScreenResolution = base.Shader.GetParameter<Vector2>("uScreenResolution");
				uScreenPosition = base.Shader.GetParameter<Vector2>("uScreenPosition");
				uTargetPosition = base.Shader.GetParameter<Vector2>("uTargetPosition");
				uImageOffset = base.Shader.GetParameter<Vector2>("uImageOffset");
				uSceneSize = base.Shader.GetParameter<Vector2>("uSceneSize");
				uSceneOffset = base.Shader.GetParameter<Vector2>("uSceneOffset");
				uIntensity = base.Shader.GetParameter<float>("uIntensity");
				uProgress = base.Shader.GetParameter<float>("uProgress");
				uDirection = base.Shader.GetParameter<Vector2>("uDirection");
				uZoom = base.Shader.GetParameter<Vector2>("uZoom");
				uMultiChunkScene = base.Shader.GetParameter<bool>("uMultiChunkScene");
				for (int i = 0; i < uImageSize.Length; i++)
				{
					uImageSize[i] = base.Shader.GetParameter<Vector2>("uImageSize" + i);
				}
			}
		}

		public override void Apply()
		{
			CheckCachedParameters();
			Vector2 vector = new Vector2(Main.offScreenRange, Main.offScreenRange);
			uColor.SetValue(_uColor);
			uOpacity.SetValue(CombinedOpacity);
			uSecondaryColor.SetValue(_uSecondaryColor);
			uTime.SetValue(Main.GlobalTimeWrappedHourly);
			uScreenResolution.SetValue(UnscaledScreenSize);
			uScreenPosition.SetValue(UnscaledScreenPosition - vector);
			uTargetPosition.SetValue(_uTargetPosition - vector);
			uImageOffset.SetValue(_uImageOffset);
			uSceneSize.SetValue(_uSceneSize);
			uSceneOffset.SetValue(_uSceneOffset);
			uIntensity.SetValue(_uIntensity);
			uProgress.SetValue(_uProgress);
			uDirection.SetValue(_uDirection);
			uZoom.SetValue(Main.GameViewMatrix.RenderZoom);
			uMultiChunkScene.SetValue(MultiChunkCapture);
			uImageSize[0].SetValue(_uImageSize0);
			for (int i = 0; i < _uAssetImages.Length; i++)
			{
				Texture2D texture2D = _uCustomImages[i];
				if (_uAssetImages[i] != null && _uAssetImages[i].IsLoaded)
				{
					texture2D = _uAssetImages[i].Value;
				}
				if (texture2D != null)
				{
					Main.graphics.GraphicsDevice.Textures[i + 1] = texture2D;
					int width = texture2D.Width;
					int height = texture2D.Height;
					if (_samplerStates[i] != null)
					{
						Main.graphics.GraphicsDevice.SamplerStates[i + 1] = _samplerStates[i];
					}
					else if (Utils.IsPowerOfTwo(width) && Utils.IsPowerOfTwo(height))
					{
						Main.graphics.GraphicsDevice.SamplerStates[i + 1] = SamplerState.LinearWrap;
					}
					else
					{
						Main.graphics.GraphicsDevice.SamplerStates[i + 1] = SamplerState.AnisotropicClamp;
					}
					uImageSize[i + 1].SetValue(new Vector2(width, height) * _imageScales[i]);
				}
			}
			base.Apply();
		}

		public ScreenShaderData UseImageOffset(Vector2 offset)
		{
			_uImageOffset = offset;
			return this;
		}

		public ScreenShaderData UseIntensity(float intensity)
		{
			_uIntensity = intensity;
			return this;
		}

		public ScreenShaderData UseColor(float r, float g, float b)
		{
			return UseColor(new Vector3(r, g, b));
		}

		public ScreenShaderData UseProgress(float progress)
		{
			_uProgress = progress;
			return this;
		}

		public ScreenShaderData UseImage(Texture2D image, int index = 0, SamplerState samplerState = null)
		{
			_samplerStates[index] = samplerState;
			_uAssetImages[index] = null;
			_uCustomImages[index] = image;
			return this;
		}

		public ScreenShaderData UseImage(string path, int index = 0, SamplerState samplerState = null)
		{
			_uAssetImages[index] = Main.Assets.Request<Texture2D>(path);
			_uCustomImages[index] = null;
			_samplerStates[index] = samplerState;
			return this;
		}

		public ScreenShaderData UseSceneSize(Vector2 size)
		{
			_uSceneSize = size;
			return this;
		}

		public ScreenShaderData UseSceneOffset(Vector2 size)
		{
			_uSceneOffset = size;
			return this;
		}

		public ScreenShaderData UseImageSize0(Vector2 size)
		{
			_uImageSize0 = size;
			return this;
		}

		public ScreenShaderData UseColor(Color color)
		{
			return UseColor(color.ToVector3());
		}

		public ScreenShaderData UseColor(Vector3 color)
		{
			_uColor = color;
			return this;
		}

		public ScreenShaderData UseDirection(Vector2 direction)
		{
			_uDirection = direction;
			return this;
		}

		public ScreenShaderData UseGlobalOpacity(float opacity)
		{
			_globalOpacity = opacity;
			return this;
		}

		public ScreenShaderData UseTargetPosition(Vector2 position)
		{
			_uTargetPosition = position;
			return this;
		}

		public ScreenShaderData UseSecondaryColor(float r, float g, float b)
		{
			return UseSecondaryColor(new Vector3(r, g, b));
		}

		public ScreenShaderData UseSecondaryColor(Color color)
		{
			return UseSecondaryColor(color.ToVector3());
		}

		public ScreenShaderData UseSecondaryColor(Vector3 color)
		{
			_uSecondaryColor = color;
			return this;
		}

		public ScreenShaderData UseOpacity(float opacity)
		{
			_uOpacity = opacity;
			return this;
		}

		public ScreenShaderData UseImageScale(Vector2 scale, int index = 0)
		{
			_imageScales[index] = scale;
			return this;
		}

		public virtual ScreenShaderData GetSecondaryShader(Player player)
		{
			return this;
		}
	}
}
