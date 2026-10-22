using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;

namespace Terraria.Graphics.Shaders
{
	public class ShaderData
	{
		public class EffectParameter<T>
		{
			private readonly Action<T> _setValue;

			private T _value;

			private bool _hasValue;

			private static ConditionalWeakTable<EffectParameter, object> _cachedParameters = new ConditionalWeakTable<EffectParameter, object>();

			private EffectParameter(Action<T> setValue)
			{
				_setValue = setValue;
			}

			public void SetValue(T value)
			{
				if (!_hasValue || !EqualityComparer<T>.Default.Equals(_value, value))
				{
					_hasValue = true;
					_value = value;
					_setValue(value);
				}
			}

			public static EffectParameter<T> Get(EffectParameter param)
			{
				if (param == null)
				{
					return null;
				}
				return (EffectParameter<T>)_cachedParameters.GetValue(param, _Create);
			}

			private static object _Create(EffectParameter param)
			{
				return ((typeof(T) == typeof(Matrix)) ? new EffectParameter<Matrix>(param.SetValue) : ((typeof(T) == typeof(Quaternion)) ? new EffectParameter<Quaternion>(param.SetValue) : ((typeof(T) == typeof(Vector4)) ? new EffectParameter<Vector4>(param.SetValue) : ((typeof(T) == typeof(Vector3)) ? new EffectParameter<Vector3>(param.SetValue) : ((typeof(T) == typeof(Vector2)) ? new EffectParameter<Vector2>(param.SetValue) : ((typeof(T) == typeof(float)) ? new EffectParameter<float>(param.SetValue) : ((typeof(T) == typeof(int)) ? new EffectParameter<int>(param.SetValue) : ((typeof(T) == typeof(bool)) ? new EffectParameter<bool>(param.SetValue) : ((typeof(T) == typeof(string)) ? ((object)new EffectParameter<string>(param.SetValue)) : ((object)((typeof(T) == typeof(Texture)) ? new EffectParameter<Texture>(param.SetValue) : null))))))))))) ?? throw new ArgumentOutOfRangeException("Unsupported type: " + typeof(T));
			}
		}

		private readonly Asset<Effect> _shader;

		private readonly string _passName;

		private Effect _effect;

		private EffectPass _effectPass;

		public Effect Shader
		{
			get
			{
				if (_shader != null)
				{
					return _shader.Value;
				}
				return null;
			}
		}

		public ShaderData(Asset<Effect> shader, string passName)
		{
			_passName = passName;
			_shader = shader;
		}

		public virtual void Apply()
		{
			if (_effect == null || _effect != Shader)
			{
				_effect = Shader;
				_effectPass = Shader.CurrentTechnique.Passes[_passName];
			}
			_effectPass.Apply();
		}
	}
}
