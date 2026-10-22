using System;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.GameContent;

namespace Terraria.Graphics.Renderers
{
	public class LittleFlyingCritterParticle : IPooledParticle, IParticle, IParticleRepel
	{
		public enum FlyType
		{
			RegularFly,
			ButterFly
		}

		private int _lifeTimeCounted;

		private int _lifeTimeTotal;

		private Vector2 _spawnPosition;

		private Vector2 _localPosition;

		private Vector2 _velocity;

		private float _neverGoBelowThis;

		private Vector2 _addedVelocity;

		private int _repelLifetimeDecay;

		private Color _overrideColor;

		private FlyType _type;

		private int _variantRow;

		private int _variantColumn;

		public bool IsRestingInPool { get; private set; }

		public bool ShouldBeRemovedFromRenderer { get; private set; }

		public void Prepare(FlyType type, Vector2 position, int duration, Color overrideColor = default(Color), int repelLifetimeDecay = 0)
		{
			_type = type;
			_variantRow = Main.rand.Next(8);
			_variantColumn = ((Main.rand.Next(5) == 0) ? 1 : 0);
			_spawnPosition = position;
			_localPosition = position + Main.rand.NextVector2Circular(4f, 8f);
			_neverGoBelowThis = position.Y + 8f;
			RandomizeVelocity();
			_lifeTimeCounted = 0;
			_lifeTimeTotal = 300 + Main.rand.Next(6) * 60;
			_overrideColor = overrideColor;
			_repelLifetimeDecay = repelLifetimeDecay;
		}

		private void RandomizeVelocity()
		{
			_velocity = Main.rand.NextVector2Circular(1f, 1f);
		}

		public void RestInPool()
		{
			IsRestingInPool = true;
		}

		public virtual void FetchFromPool()
		{
			IsRestingInPool = false;
			ShouldBeRemovedFromRenderer = false;
			_addedVelocity = Vector2.Zero;
		}

		public void Update(ref ParticleRendererSettings settings)
		{
			if (++_lifeTimeCounted >= _lifeTimeTotal)
			{
				ShouldBeRemovedFromRenderer = true;
			}
			float num = 0.02f;
			int num2 = 30;
			if (_type == FlyType.ButterFly)
			{
				num = 0.01f;
				num2 = 600;
			}
			_velocity += new Vector2((float)Math.Sign(_spawnPosition.X - _localPosition.X) * num, (float)Math.Sign(_spawnPosition.Y - _localPosition.Y) * num);
			if (_lifeTimeCounted % num2 == 0 && Main.rand.Next(2) == 0)
			{
				RandomizeVelocity();
				if (Main.rand.Next(2) == 0)
				{
					_velocity /= 2f;
				}
			}
			_addedVelocity *= 0.98f;
			if (_addedVelocity.Length() < 0.01f)
			{
				_addedVelocity = new Vector2(0f, 0f);
			}
			_localPosition += _velocity + _addedVelocity;
			if (_localPosition.Y > _neverGoBelowThis)
			{
				_localPosition.Y = _neverGoBelowThis;
				if (_velocity.Y > 0f)
				{
					_velocity.Y *= -1f;
				}
				if (_addedVelocity.Y > 0f)
				{
					_addedVelocity.Y *= -1f;
				}
			}
		}

		public void Draw(ref ParticleRendererSettings settings, SpriteBatch spritebatch)
		{
			Vector2 vector = settings.AnchorPosition + _localPosition;
			if (vector.X < -10f || vector.X > (float)(Main.screenWidth + 10) || vector.Y < -10f || vector.Y > (float)(Main.screenHeight + 10))
			{
				ShouldBeRemovedFromRenderer = true;
				return;
			}
			switch (_type)
			{
			case FlyType.RegularFly:
				Draw_Fly(ref settings, spritebatch);
				break;
			case FlyType.ButterFly:
				Draw_ButterFly(ref settings, spritebatch);
				break;
			}
		}

		private void Draw_ButterFly(ref ParticleRendererSettings settings, SpriteBatch spritebatch)
		{
			Vector2 vector = _velocity + _addedVelocity;
			Texture2D value = TextureAssets.Extra[281].Value;
			int num = _lifeTimeCounted % 10 / 5;
			int variantRow = _variantRow;
			bool flag = _variantColumn == 1;
			Rectangle rectangle = new Rectangle(flag ? 10 : 0, (variantRow * 2 + num) * 10, flag ? 14 : 8, 8);
			Vector2 origin = rectangle.Size() / 2f;
			float num2 = Utils.Remap(_lifeTimeCounted, 0f, 90f, 0f, 1f) * Utils.Remap(_lifeTimeCounted, _lifeTimeTotal - 90, _lifeTimeTotal, 1f, 0f);
			Color color = Lighting.GetColor(_localPosition.ToTileCoordinates());
			_overrideColor = Color.White;
			Vector4 vector2 = _overrideColor.ToVector4() * color.ToVector4();
			Color color2 = new Color(vector2);
			float scale = 0.75f;
			spritebatch.Draw(value, settings.AnchorPosition + _localPosition, rectangle, color2 * num2, 0f, origin, scale, (vector.X < 0f) ? SpriteEffects.FlipHorizontally : SpriteEffects.None, 0f);
		}

		private void Draw_Fly(ref ParticleRendererSettings settings, SpriteBatch spritebatch)
		{
			Vector2 vector = _velocity + _addedVelocity;
			Texture2D value = TextureAssets.Extra[262].Value;
			int frameY = _lifeTimeCounted % 6 / 3;
			Rectangle value2 = value.Frame(1, 6, 0, frameY);
			Vector2 origin = new Vector2((!(vector.X > 0f)) ? 1 : 3, 3f);
			float num = Utils.Remap(_lifeTimeCounted, 0f, 90f, 0f, 1f) * Utils.Remap(_lifeTimeCounted, _lifeTimeTotal - 90, _lifeTimeTotal, 1f, 0f);
			Color color = Lighting.GetColor(_localPosition.ToTileCoordinates());
			if (_overrideColor == default(Color))
			{
				spritebatch.Draw(value, settings.AnchorPosition + _localPosition, value2, color * num, 0f, origin, 1f, (vector.X > 0f) ? SpriteEffects.FlipHorizontally : SpriteEffects.None, 0f);
				return;
			}
			Vector4 vector2 = _overrideColor.ToVector4() * color.ToVector4();
			Color color2 = new Color(vector2);
			value2.Offset(0, 12);
			spritebatch.Draw(value, settings.AnchorPosition + _localPosition, value2, color2 * num, 0f, origin, 1f, (vector.X > 0f) ? SpriteEffects.FlipHorizontally : SpriteEffects.None, 0f);
			value2.Offset(0, 12);
			spritebatch.Draw(value, settings.AnchorPosition + _localPosition, value2, color * num, 0f, origin, 1f, (vector.X > 0f) ? SpriteEffects.FlipHorizontally : SpriteEffects.None, 0f);
		}

		public void BeRepelled(ref ParticleRepelDetails details)
		{
			float num = Utils.Remap(_localPosition.Distance(details.SourcePosition) - details.Radius, 0f, 100f, 1f, 0f);
			if (!(num <= 0f))
			{
				Vector2 vector = _localPosition.DirectionFrom(details.SourcePosition).SafeNormalize(-Vector2.UnitY).RotatedByRandom(0.5235987901687622);
				_addedVelocity = vector * 3.5f * num;
				_lifeTimeCounted += _repelLifetimeDecay;
			}
		}
	}
}
