using System;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;
using Terraria.DataStructures;

namespace Terraria.Graphics.Shaders
{
	public class MiscShaderData : ShaderData
	{
		private Vector3 _uColor = Vector3.One;

		private Vector3 _uSecondaryColor = Vector3.One;

		private float _uSaturation = 1f;

		private float _uOpacity = 1f;

		private Asset<Texture2D> _uImage0;

		private Asset<Texture2D> _uImage1;

		private Asset<Texture2D> _uImage2;

		private Texture _uImage0Tex;

		private Texture _uImage1Tex;

		private Texture _uImage2Tex;

		private bool _useProjectionMatrix;

		private Vector4 _shaderSpecificData = Vector4.Zero;

		private SamplerState _customSamplerState;

		private Matrix? _transformMatrix;

		private Effect _effect;

		private EffectParameter<Vector3> uColor;

		private EffectParameter<float> uSaturation;

		private EffectParameter<Vector3> uSecondaryColor;

		private EffectParameter<float> uTime;

		private EffectParameter<float> uOpacity;

		private EffectParameter<Vector4> uShaderSpecificData;

		private EffectParameter<Vector4> uSourceRect;

		private EffectParameter<Vector2> uDrawPosition;

		private EffectParameter<Vector2> uImageSize0;

		private EffectParameter<Vector2> uImageSize1;

		private EffectParameter<Vector2> uImageSize2;

		private EffectParameter<Matrix> MatrixTransform;

		public MiscShaderData(Asset<Effect> shader, string passName)
			: base(shader, passName)
		{
		}

		private void CheckCachedParameters()
		{
			if (_effect == null || _effect != base.Shader)
			{
				_effect = base.Shader;
				uColor = base.Shader.GetParameter<Vector3>("uColor");
				uSaturation = base.Shader.GetParameter<float>("uSaturation");
				uSecondaryColor = base.Shader.GetParameter<Vector3>("uSecondaryColor");
				uTime = base.Shader.GetParameter<float>("uTime");
				uOpacity = base.Shader.GetParameter<float>("uOpacity");
				uShaderSpecificData = base.Shader.GetParameter<Vector4>("uShaderSpecificData");
				uSourceRect = base.Shader.GetParameter<Vector4>("uSourceRect");
				uDrawPosition = base.Shader.GetParameter<Vector2>("uDrawPosition");
				uImageSize0 = base.Shader.GetParameter<Vector2>("uImageSize0");
				uImageSize1 = base.Shader.GetParameter<Vector2>("uImageSize1");
				uImageSize2 = base.Shader.GetParameter<Vector2>("uImageSize2");
				MatrixTransform = base.Shader.GetParameter<Matrix>("MatrixTransform");
			}
		}

		public virtual void Apply(DrawData? drawData = null)
		{
			CheckCachedParameters();
			uColor.SetValue(_uColor);
			uSaturation.SetValue(_uSaturation);
			uSecondaryColor.SetValue(_uSecondaryColor);
			uTime.SetValue(Main.GlobalTimeWrappedHourly);
			uOpacity.SetValue(_uOpacity);
			uShaderSpecificData.SetValue(_shaderSpecificData);
			if (drawData.HasValue)
			{
				DrawData value = drawData.Value;
				Vector4 value2 = Vector4.Zero;
				if (drawData.Value.sourceRect.HasValue)
				{
					value2 = new Vector4(value.sourceRect.Value.X, value.sourceRect.Value.Y, value.sourceRect.Value.Width, value.sourceRect.Value.Height);
				}
				uSourceRect.SetValue(value2);
				uDrawPosition.SetValue(value.position);
				uImageSize0.SetValue(new Vector2(value.texture.Width, value.texture.Height));
			}
			else
			{
				uSourceRect.SetValue(new Vector4(0f, 0f, 4f, 4f));
			}
			SamplerState value3 = SamplerState.LinearWrap;
			if (_customSamplerState != null)
			{
				value3 = _customSamplerState;
			}
			Texture texture = ((_uImage0 != null) ? _uImage0.Value : _uImage0Tex);
			if (texture != null)
			{
				Main.graphics.GraphicsDevice.Textures[0] = texture;
				Main.graphics.GraphicsDevice.SamplerStates[0] = value3;
				if (texture is Texture2D)
				{
					uImageSize0.SetValue(((Texture2D)texture).Size());
				}
			}
			texture = ((_uImage1 != null) ? _uImage1.Value : _uImage1Tex);
			if (texture != null)
			{
				Main.graphics.GraphicsDevice.Textures[1] = texture;
				Main.graphics.GraphicsDevice.SamplerStates[1] = value3;
				if (texture is Texture2D)
				{
					uImageSize1.SetValue(((Texture2D)texture).Size());
				}
			}
			texture = ((_uImage2 != null) ? _uImage2.Value : _uImage2Tex);
			if (texture != null)
			{
				Main.graphics.GraphicsDevice.Textures[2] = texture;
				Main.graphics.GraphicsDevice.SamplerStates[2] = value3;
				if (texture is Texture2D)
				{
					uImageSize2.SetValue(((Texture2D)texture).Size());
				}
			}
			if (_useProjectionMatrix)
			{
				MatrixTransform.SetValue(Main.GameViewMatrix.NormalizedTransformationMatrix);
			}
			if (_transformMatrix.HasValue)
			{
				MatrixTransform.SetValue(_transformMatrix.Value);
			}
			base.Apply();
		}

		public MiscShaderData UseColor(float r, float g, float b)
		{
			return UseColor(new Vector3(r, g, b));
		}

		public MiscShaderData UseColor(Color color)
		{
			return UseColor(color.ToVector3());
		}

		public MiscShaderData UseColor(Vector3 color)
		{
			_uColor = color;
			return this;
		}

		public MiscShaderData UseSamplerState(SamplerState state)
		{
			_customSamplerState = state;
			return this;
		}

		public MiscShaderData UseImage0(string path)
		{
			if (Main.dedServ)
			{
				return this;
			}
			_uImage0Tex = null;
			_uImage0 = Main.Assets.Request<Texture2D>(path);
			return this;
		}

		public MiscShaderData UseImage1(string path)
		{
			if (Main.dedServ)
			{
				return this;
			}
			_uImage1Tex = null;
			_uImage1 = Main.Assets.Request<Texture2D>(path);
			return this;
		}

		public MiscShaderData UseImage2(string path)
		{
			if (Main.dedServ)
			{
				return this;
			}
			_uImage2Tex = null;
			_uImage2 = Main.Assets.Request<Texture2D>(path);
			return this;
		}

		public MiscShaderData UseImage0(Texture texture)
		{
			if (Main.dedServ)
			{
				return this;
			}
			_uImage0Tex = texture;
			_uImage0 = null;
			return this;
		}

		public MiscShaderData UseImage1(Texture texture)
		{
			if (Main.dedServ)
			{
				return this;
			}
			_uImage1Tex = texture;
			_uImage1 = null;
			return this;
		}

		public MiscShaderData UseImage2(Texture texture)
		{
			if (Main.dedServ)
			{
				return this;
			}
			_uImage2Tex = texture;
			_uImage2 = null;
			return this;
		}

		private static bool IsPowerOfTwo(int n)
		{
			return (int)Math.Ceiling(Math.Log(n) / Math.Log(2.0)) == (int)Math.Floor(Math.Log(n) / Math.Log(2.0));
		}

		public MiscShaderData UseOpacity(float alpha)
		{
			_uOpacity = alpha;
			return this;
		}

		public MiscShaderData UseSecondaryColor(float r, float g, float b)
		{
			return UseSecondaryColor(new Vector3(r, g, b));
		}

		public MiscShaderData UseSecondaryColor(Color color)
		{
			return UseSecondaryColor(color.ToVector3());
		}

		public MiscShaderData UseSecondaryColor(Vector3 color)
		{
			_uSecondaryColor = color;
			return this;
		}

		public MiscShaderData UseProjectionMatrix(bool doUse)
		{
			_useProjectionMatrix = doUse;
			return this;
		}

		public MiscShaderData UseSpriteTransformMatrix(Matrix? transform)
		{
			if (!transform.HasValue)
			{
				_transformMatrix = null;
				return this;
			}
			Viewport viewport = Main.graphics.GraphicsDevice.Viewport;
			float num = ((viewport.Width > 0) ? (1f / (float)viewport.Width) : 0f);
			float num2 = ((viewport.Height > 0) ? (-1f / (float)viewport.Height) : 0f);
			Matrix matrix = new Matrix
			{
				M11 = num * 2f,
				M22 = num2 * 2f,
				M33 = 1f,
				M44 = 1f,
				M41 = -1f - num,
				M42 = 1f - num2
			};
			_transformMatrix = transform.Value * matrix;
			return this;
		}

		public MiscShaderData UseSaturation(float saturation)
		{
			_uSaturation = saturation;
			return this;
		}

		public virtual MiscShaderData GetSecondaryShader(Entity entity)
		{
			return this;
		}

		public MiscShaderData UseShaderSpecificData(Vector4 specificData)
		{
			_shaderSpecificData = specificData;
			return this;
		}
	}
}
