using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;
using Terraria.DataStructures;

namespace Terraria.Graphics.Shaders
{
	public class HairShaderData : ShaderData
	{
		protected Vector3 _uColor = Vector3.One;

		protected Vector3 _uSecondaryColor = Vector3.One;

		protected float _uSaturation = 1f;

		protected float _uOpacity = 1f;

		protected Asset<Texture2D> _uImage;

		protected bool _shaderDisabled;

		private Vector2 _uTargetPosition = Vector2.One;

		private Effect _effect;

		private EffectParameter<Vector3> uColor;

		private EffectParameter<float> uSaturation;

		private EffectParameter<Vector3> uSecondaryColor;

		private EffectParameter<float> uTime;

		private EffectParameter<float> uOpacity;

		private EffectParameter<float> uDirection;

		private EffectParameter<Vector4> uSourceRect;

		private EffectParameter<Vector2> uDrawPosition;

		private EffectParameter<Vector2> uTargetPosition;

		private EffectParameter<Vector2> uImageSize0;

		private EffectParameter<Vector2> uImageSize1;

		public bool ShaderDisabled => _shaderDisabled;

		public HairShaderData(Asset<Effect> shader, string passName)
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
				uDirection = base.Shader.GetParameter<float>("uDirection");
				uSourceRect = base.Shader.GetParameter<Vector4>("uSourceRect");
				uDrawPosition = base.Shader.GetParameter<Vector2>("uDrawPosition");
				uTargetPosition = base.Shader.GetParameter<Vector2>("uTargetPosition");
				uImageSize0 = base.Shader.GetParameter<Vector2>("uImageSize0");
				uImageSize1 = base.Shader.GetParameter<Vector2>("uImageSize1");
			}
		}

		public virtual void Apply(Player player, DrawData? drawData = null)
		{
			if (!_shaderDisabled)
			{
				CheckCachedParameters();
				uColor.SetValue(_uColor);
				uSaturation.SetValue(_uSaturation);
				uSecondaryColor.SetValue(_uSecondaryColor);
				uTime.SetValue(Main.GlobalTimeWrappedHourly);
				uOpacity.SetValue(_uOpacity);
				uTargetPosition.SetValue(_uTargetPosition);
				if (drawData.HasValue)
				{
					DrawData value = drawData.Value;
					Vector4 value2 = new Vector4(value.sourceRect.Value.X, value.sourceRect.Value.Y, value.sourceRect.Value.Width, value.sourceRect.Value.Height);
					uSourceRect.SetValue(value2);
					uDrawPosition.SetValue(value.position);
					uImageSize0.SetValue(new Vector2(value.texture.Width, value.texture.Height));
				}
				else
				{
					uSourceRect.SetValue(new Vector4(0f, 0f, 4f, 4f));
				}
				if (_uImage != null)
				{
					Main.graphics.GraphicsDevice.Textures[1] = _uImage.Value;
					uImageSize1.SetValue(new Vector2(_uImage.Width(), _uImage.Height()));
				}
				if (player != null)
				{
					uDirection.SetValue(player.direction);
				}
				Apply();
			}
		}

		public virtual Color GetColor(Player player, Color lightColor)
		{
			return new Color(lightColor.ToVector4() * player.hairColor.ToVector4());
		}

		public HairShaderData UseColor(float r, float g, float b)
		{
			return UseColor(new Vector3(r, g, b));
		}

		public HairShaderData UseColor(Color color)
		{
			return UseColor(color.ToVector3());
		}

		public HairShaderData UseColor(Vector3 color)
		{
			_uColor = color;
			return this;
		}

		public HairShaderData UseImage(string path)
		{
			if (!Main.dedServ)
			{
				_uImage = Main.Assets.Request<Texture2D>(path);
			}
			return this;
		}

		public HairShaderData UseOpacity(float alpha)
		{
			_uOpacity = alpha;
			return this;
		}

		public HairShaderData UseSecondaryColor(float r, float g, float b)
		{
			return UseSecondaryColor(new Vector3(r, g, b));
		}

		public HairShaderData UseSecondaryColor(Color color)
		{
			return UseSecondaryColor(color.ToVector3());
		}

		public HairShaderData UseSecondaryColor(Vector3 color)
		{
			_uSecondaryColor = color;
			return this;
		}

		public HairShaderData UseSaturation(float saturation)
		{
			_uSaturation = saturation;
			return this;
		}

		public HairShaderData UseTargetPosition(Vector2 position)
		{
			_uTargetPosition = position;
			return this;
		}
	}
}
