using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;
using Terraria.DataStructures;

namespace Terraria.Graphics.Shaders
{
	public class ArmorShaderData : ShaderData
	{
		private Vector3 _uColor = Vector3.One;

		private Vector3 _uSecondaryColor = Vector3.One;

		private float _uSaturation = 1f;

		private float _uOpacity = 1f;

		private Asset<Texture2D> _uImage;

		private Vector2 _uTargetPosition = Vector2.One;

		private Effect _effect;

		private EffectParameter<Vector3> uColor;

		private EffectParameter<float> uSaturation;

		private EffectParameter<Vector3> uSecondaryColor;

		private EffectParameter<float> uTime;

		private EffectParameter<float> uOpacity;

		private EffectParameter<Vector2> uTargetPosition;

		private EffectParameter<Vector4> uSourceRect;

		private EffectParameter<Vector4> uLegacyArmorSourceRect;

		private EffectParameter<Vector2> uLegacyArmorSheetSize;

		private EffectParameter<Vector2> uDrawPosition;

		private EffectParameter<float> uRotation;

		private EffectParameter<float> uDirection;

		private EffectParameter<Vector2> uImageSize0;

		private EffectParameter<Vector2> uImageSize1;

		public ArmorShaderData(Asset<Effect> shader, string passName)
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
				uTargetPosition = base.Shader.GetParameter<Vector2>("uTargetPosition");
				uSourceRect = base.Shader.GetParameter<Vector4>("uSourceRect");
				uLegacyArmorSourceRect = base.Shader.GetParameter<Vector4>("uLegacyArmorSourceRect");
				uLegacyArmorSheetSize = base.Shader.GetParameter<Vector2>("uLegacyArmorSheetSize");
				uDrawPosition = base.Shader.GetParameter<Vector2>("uDrawPosition");
				uRotation = base.Shader.GetParameter<float>("uRotation");
				uDirection = base.Shader.GetParameter<float>("uDirection");
				uImageSize0 = base.Shader.GetParameter<Vector2>("uImageSize0");
				uImageSize1 = base.Shader.GetParameter<Vector2>("uImageSize1");
			}
		}

		public virtual void Apply(Entity entity, DrawData? drawData = null)
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
				Vector4 value2 = ((!value.sourceRect.HasValue) ? new Vector4(0f, 0f, value.texture.Width, value.texture.Height) : new Vector4(value.sourceRect.Value.X, value.sourceRect.Value.Y, value.sourceRect.Value.Width, value.sourceRect.Value.Height));
				uSourceRect.SetValue(value2);
				uLegacyArmorSourceRect.SetValue(value2);
				uDrawPosition.SetValue(value.position);
				uImageSize0.SetValue(new Vector2(value.texture.Width, value.texture.Height));
				uLegacyArmorSheetSize.SetValue(new Vector2(value.texture.Width, value.texture.Height));
				uRotation.SetValue(value.rotation * (((value.effect & SpriteEffects.FlipHorizontally) != SpriteEffects.None) ? (-1f) : 1f));
				uDirection.SetValue(((value.effect & SpriteEffects.FlipHorizontally) == 0) ? 1 : (-1));
			}
			else
			{
				Vector4 value3 = new Vector4(0f, 0f, 4f, 4f);
				uSourceRect.SetValue(value3);
				uLegacyArmorSourceRect.SetValue(value3);
				uRotation.SetValue(0f);
			}
			if (_uImage != null)
			{
				Main.graphics.GraphicsDevice.Textures[1] = _uImage.Value;
				uImageSize1.SetValue(new Vector2(_uImage.Width(), _uImage.Height()));
			}
			if (entity != null)
			{
				uDirection.SetValue(entity.direction);
			}
			if (entity is Player player)
			{
				Rectangle bodyFrame = player.bodyFrame;
				uLegacyArmorSourceRect.SetValue(new Vector4(bodyFrame.X, bodyFrame.Y, bodyFrame.Width, bodyFrame.Height));
				uLegacyArmorSheetSize.SetValue(new Vector2(40f, 1120f));
			}
			Apply();
		}

		public ArmorShaderData UseColor(float r, float g, float b)
		{
			return UseColor(new Vector3(r, g, b));
		}

		public ArmorShaderData UseColor(Color color)
		{
			return UseColor(color.ToVector3());
		}

		public ArmorShaderData UseColor(Vector3 color)
		{
			_uColor = color;
			return this;
		}

		public ArmorShaderData UseImage(string path)
		{
			if (!Main.dedServ)
			{
				_uImage = Main.Assets.Request<Texture2D>(path);
			}
			return this;
		}

		public ArmorShaderData UseOpacity(float alpha)
		{
			_uOpacity = alpha;
			return this;
		}

		public ArmorShaderData UseTargetPosition(Vector2 position)
		{
			_uTargetPosition = position;
			return this;
		}

		public ArmorShaderData UseSecondaryColor(float r, float g, float b)
		{
			return UseSecondaryColor(new Vector3(r, g, b));
		}

		public ArmorShaderData UseSecondaryColor(Color color)
		{
			return UseSecondaryColor(color.ToVector3());
		}

		public ArmorShaderData UseSecondaryColor(Vector3 color)
		{
			_uSecondaryColor = color;
			return this;
		}

		public ArmorShaderData UseSaturation(float saturation)
		{
			_uSaturation = saturation;
			return this;
		}

		public virtual ArmorShaderData GetSecondaryShader(Entity entity)
		{
			return this;
		}
	}
}
