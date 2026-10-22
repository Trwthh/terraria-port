using System;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.Audio;
using Terraria.GameContent;
using Terraria.GameContent.Shaders;
using Terraria.Graphics.Effects;
using Terraria.ID;

namespace Terraria.Graphics.Renderers
{
	public class FakeFishParticle : IPooledParticle, IParticle, IParticleRepel
	{
		private enum State
		{
			FreeRoaming,
			InterestedInBobber,
			Latched,
			Jumping
		}

		public Vector2 Position;

		public Vector2 Velocity;

		public float Rotation;

		private int _latchedProjectileType;

		private Projectile _latchedProjectile;

		private Item _itemInstance;

		private int _lifeTimeCounted;

		private int _lifeTimeTotal;

		private float _totalScale;

		private int _waitTime;

		private State _state;

		private int _jumpTimeLeft;

		private bool _itemLooksDiagonal;

		private Vector2 _targetVelocity;

		private Vector2 _bobberLocation;

		private int _steerTime;

		private bool _isInanimate;

		private Color _sonarColor;

		private int _sonarTimeleft;

		private int _sonarStartTime;

		private bool _isSelectedSonarType;

		private bool _wasWet;

		private int _delayTime;

		private bool _gotRepelled;

		public bool ShouldBeRemovedFromRenderer { get; private set; }

		public bool IsRestingInPool { get; private set; }

		public FakeFishParticle()
		{
			_itemInstance = new Item();
		}

		public bool TryToMagnetizeTo(Projectile projectile, int requiredItemType = -1)
		{
			if (!CanHit(projectile, requiredItemType))
			{
				return false;
			}
			_latchedProjectile = projectile;
			_latchedProjectileType = projectile.type;
			_state = State.Latched;
			SetAndGetLatchDetails(out var _, out var _);
			Rotate(instant: true);
			return true;
		}

		public bool TryToPushAway(Projectile projectile, int requiredItemType = -1)
		{
			if (_state == State.Jumping)
			{
				return false;
			}
			if (requiredItemType == _itemInstance.type)
			{
				return false;
			}
			if (!CanHit(projectile, -1, 160f))
			{
				return false;
			}
			Velocity += projectile.DirectionTo(Position).SafeNormalize(Vector2.UnitY) * (1f + 1f * Main.rand.NextFloat()) * 1f;
			return true;
		}

		public bool TryToBePinged(Projectile projectile, int requiredItemType = -1)
		{
			if (!CanHit(projectile, -1, 1000f))
			{
				return false;
			}
			int num = (int)projectile.Center.Distance(Position) / 10;
			_sonarTimeleft = (_sonarStartTime = 60) + num;
			_isSelectedSonarType = requiredItemType == _itemInstance.type;
			_sonarColor = (_isSelectedSonarType ? new Color(255, 220, 30, 0) : (new Color(30, 200, 255, 0) * 0.05f));
			return true;
		}

		public bool CanHit(Projectile projectile, int requiredItemType = -1, float allowedRange = 80f)
		{
			if (requiredItemType != -1 && _itemInstance.type != requiredItemType)
			{
				return false;
			}
			Vector2 center = projectile.Center;
			int num = 80;
			if (center.Distance(Position) > (float)num)
			{
				return false;
			}
			if (!Collision.CanHitLine(center, 0, 0, Position, 0, 0))
			{
				return false;
			}
			return true;
		}

		private void CheckLatch()
		{
			if (_state != State.Latched)
			{
				return;
			}
			int num = 80;
			if (!_latchedProjectile.active || _latchedProjectile.type != _latchedProjectileType || Position.Distance(_latchedProjectile.Center) > (float)num)
			{
				RemoveLatch();
			}
			else if (_latchedProjectile.ai[0] == 0f && _latchedProjectile.ai[1] >= 0f)
			{
				RemoveLatch();
			}
			else if (_latchedProjectile.ai[0] == 1f)
			{
				RemoveLatch();
				if (_latchedProjectile.ai[1] == (float)_itemInstance.type)
				{
					ShouldBeRemovedFromRenderer = true;
				}
			}
		}

		private void RemoveLatch()
		{
			_state = State.FreeRoaming;
			PickNewVelocity();
		}

		private void EmitWaterPulse()
		{
			if (Main.netMode != 2)
			{
				Vector2 position = Position - new Vector2(4f, 4f);
				int width = 8;
				int height = 8;
				bool lavaWet = Collision.LavaCollision(position, width, height);
				Collision.WetCollision(position, width, height);
				DoStandardWaterSplash(Position, Collision.shimmer, Collision.honey, lavaWet);
				WaterShaderData obj = (WaterShaderData)Filters.Scene["WaterDistortion"].GetShader();
				float value = 1.4f;
				obj.QueueRipple(Position, new Color(0.5f, 0.1f * (float)Math.Sign(value) + 0.5f, 0f, 1f) * Math.Abs(value), new Vector2(4f, 4f), RippleShape.Circle);
			}
		}

		private void DoStandardWaterSplash(Vector2 castPosition, bool shimmerWet, bool honeyWet, bool lavaWet)
		{
			Vector2 vector = castPosition - new Vector2(4f, 4f);
			int num = 8;
			int num2 = 8;
			if (shimmerWet)
			{
				for (int i = 0; i < 10; i++)
				{
					int num3 = Dust.NewDust(new Vector2(vector.X - 6f, vector.Y + (float)(num2 / 2) - 8f), num + 12, 24, 308);
					Main.dust[num3].velocity.Y -= 4f;
					Main.dust[num3].velocity.X *= 2.5f;
					Main.dust[num3].scale = 1.3f;
					Main.dust[num3].noGravity = true;
					switch (Main.rand.Next(6))
					{
					case 0:
						Main.dust[num3].color = new Color(255, 255, 210);
						break;
					case 1:
						Main.dust[num3].color = new Color(190, 245, 255);
						break;
					case 2:
						Main.dust[num3].color = new Color(255, 150, 255);
						break;
					default:
						Main.dust[num3].color = new Color(190, 175, 255);
						break;
					}
					SoundEngine.PlaySound(SoundID.FishSplash, (int)vector.X, (int)vector.Y, 5f);
				}
			}
			else if (honeyWet)
			{
				for (int j = 0; j < 10; j++)
				{
					int num4 = Dust.NewDust(new Vector2(vector.X - 6f, vector.Y + (float)(num2 / 2) - 8f), num + 12, 24, 152);
					Main.dust[num4].velocity.Y -= 1f;
					Main.dust[num4].velocity.X *= 2.5f;
					Main.dust[num4].scale = 1.3f;
					Main.dust[num4].alpha = 100;
					Main.dust[num4].noGravity = true;
				}
				SoundEngine.PlaySound(SoundID.FishSplash, (int)vector.X, (int)vector.Y, 1f);
			}
			else if (lavaWet)
			{
				for (int k = 0; k < 10; k++)
				{
					int num5 = Dust.NewDust(new Vector2(vector.X - 6f, vector.Y + (float)(num2 / 2) - 8f), num + 12, 24, 35);
					Main.dust[num5].velocity.Y -= 1.5f;
					Main.dust[num5].velocity.X *= 2.5f;
					Main.dust[num5].scale = 1.3f;
					Main.dust[num5].alpha = 100;
					Main.dust[num5].noGravity = true;
				}
				SoundEngine.PlaySound(SoundID.FishSplash, (int)vector.X, (int)vector.Y, 1f);
			}
			else
			{
				for (int l = 0; l < 10; l++)
				{
					int num6 = Dust.NewDust(new Vector2(vector.X - 6f, vector.Y + (float)(num2 / 2)), num + 12, 24, Dust.dustWater());
					Main.dust[num6].velocity.Y -= 4f;
					Main.dust[num6].velocity.X *= 2.5f;
					Main.dust[num6].scale = 1.3f;
					Main.dust[num6].alpha = 100;
					Main.dust[num6].noGravity = true;
				}
				SoundEngine.PlaySound(SoundID.FishSplash, (int)vector.X, (int)vector.Y, 1f);
			}
		}

		public void Update(ref ParticleRendererSettings settings)
		{
			if (_delayTime-- >= 0)
			{
				return;
			}
			if (++_lifeTimeCounted >= _lifeTimeTotal)
			{
				ShouldBeRemovedFromRenderer = true;
			}
			if (--_sonarTimeleft < 0)
			{
				_sonarTimeleft = 0;
			}
			bool flag = Collision.WetCollision(Position, 2, 2);
			if (flag != _wasWet)
			{
				EmitWaterPulse();
			}
			_wasWet = flag;
			if (_state == State.Jumping)
			{
				Velocity.Y += 0.2f;
				Rotation = Velocity.ToRotation();
				Position += Velocity;
				if (--_jumpTimeLeft > 0)
				{
					return;
				}
				_state = State.FreeRoaming;
				PickNewVelocity();
				Velocity *= 0.15f;
			}
			if ((float)_lifeTimeCounted / (float)_lifeTimeTotal >= 0.15f && Velocity.Length() < 0.1f && --_waitTime <= 0)
			{
				_waitTime = Main.rand.Next(30, 121);
				PickNewVelocity();
				_steerTime = Main.rand.Next(30, 121);
			}
			if (--_steerTime > 0)
			{
				Velocity = Vector2.Lerp(Velocity, Rotation.ToRotationVector2() * 1f, 1f / 30f);
				Velocity = Vector2.Lerp(Velocity, _targetVelocity, 0.05f);
			}
			else if (Velocity.Length() > 0.3f)
			{
				Velocity *= 0.975f;
			}
			if (_state == State.InterestedInBobber && Position.Distance(_bobberLocation) < 16f)
			{
				Velocity *= 0.9f;
				if (Velocity.Length() < 0.02f)
				{
					float num = 0.3f + 1.4f * Main.rand.NextFloat();
					Velocity = (_bobberLocation - Position).SafeNormalize(Vector2.Zero) * (0f - num);
				}
			}
			CheckLatch();
			if (_state == State.Latched)
			{
				SetAndGetLatchDetails(out var idealBobberPosition, out var idealOffset);
				Position = Vector2.Lerp(Position, idealBobberPosition - idealOffset, 0.05f);
			}
			Position += Velocity;
			Rotate();
			int num2 = 20;
			if (_lifeTimeTotal - _lifeTimeCounted > num2 && !flag)
			{
				_lifeTimeCounted = _lifeTimeTotal - num2;
			}
			if (Velocity.Y < 0f && !Collision.WetCollision(Position + new Vector2(0f, -20f), 0, 0))
			{
				_steerTime = 0;
				Velocity.Y *= 0.92f;
			}
			if (Velocity != Vector2.Zero && !Collision.WetCollision(Position + Velocity.SafeNormalize(Vector2.Zero) * 32f, 0, 0))
			{
				_steerTime = 0;
				PickNewVelocity();
				Velocity *= 0.92f;
			}
			if (!(Velocity != Vector2.Zero))
			{
				return;
			}
			for (float num3 = 0f; num3 < 1f; num3 += 0.25f)
			{
				Vector2 vector = ((float)Math.PI * 2f * num3).ToRotationVector2() * 16f;
				if (!(Vector2.Dot(vector, Velocity.SafeNormalize(Vector2.UnitY)) < 0f) && !Collision.WetCollision(Position + vector, 0, 0))
				{
					_steerTime = 0;
					PickNewVelocity();
					Velocity *= 0.92f;
					break;
				}
			}
		}

		private void SetAndGetLatchDetails(out Vector2 idealBobberPosition, out Vector2 idealOffset)
		{
			idealBobberPosition = _latchedProjectile.Center + new Vector2(0f, 8f) + new Vector2(8f, 5f);
			Vector2 v = idealBobberPosition - Position;
			idealOffset = v.SafeNormalize(-Vector2.UnitY) * 6f;
			if (idealOffset.Y > 0f)
			{
				idealOffset.Y *= -1f;
			}
			idealOffset.Y -= 3f;
			idealOffset += new Vector2(Math.Sign(v.X) * -4, 0f);
			_targetVelocity = idealOffset;
			_bobberLocation = idealBobberPosition;
			int num = (int)((float)_lifeTimeTotal * 0.85f);
			if (_lifeTimeCounted > num)
			{
				_lifeTimeCounted = num;
			}
		}

		private void Rotate(bool instant = false)
		{
			float num = 0.1f;
			if (_isInanimate)
			{
				num *= 0.05f;
			}
			if (instant)
			{
				num = (float)Math.PI * 2f;
			}
			if (_state == State.InterestedInBobber || _state == State.Latched)
			{
				Rotation = Utils.rotateTowards(Position, Rotation.ToRotationVector2(), _bobberLocation, num).ToRotation();
			}
			else if (Velocity != Vector2.Zero)
			{
				Rotation = Utils.rotateTowards(Vector2.Zero, Rotation.ToRotationVector2(), Velocity, num).ToRotation();
			}
		}

		private void PickNewVelocity()
		{
			_targetVelocity = Main.rand.NextVector2Circular(1.5f, 0.6f);
			if (_state == State.InterestedInBobber)
			{
				_targetVelocity = (_bobberLocation - Position).SafeNormalize(-Vector2.UnitY) * (0.7f + 0.5f * Main.rand.NextFloat());
			}
			if (_state == State.Latched)
			{
				_targetVelocity = (_latchedProjectile.Center - Position).SafeNormalize(-Vector2.UnitY) * 0.7f;
			}
		}

		public void Prepare(int itemType, int lifeTimeTotal, Vector2 bobberLocation)
		{
			_itemInstance.SetDefaults(itemType);
			_totalScale = 0.6f + 0.15f * Main.rand.NextFloat();
			_bobberLocation = bobberLocation + new Vector2(0f, 8f);
			_isInanimate = false;
			_lifeTimeTotal = lifeTimeTotal;
			_state = ((Main.rand.Next(3) != 0) ? State.InterestedInBobber : State.FreeRoaming);
			_itemLooksDiagonal = ItemID.Sets.ReceivesDiagonalCorrectionAsFakeFish[itemType];
			if (ItemID.Sets.IsFishingCrate[itemType] || ItemID.Sets.FakeFishInanimate[itemType])
			{
				_state = State.FreeRoaming;
				_isInanimate = true;
				_itemLooksDiagonal = false;
			}
			PickNewVelocity();
			_steerTime = Main.rand.Next(30, 121);
			_wasWet = true;
			_delayTime = 0;
		}

		public void TryJumping()
		{
			if (!_isInanimate)
			{
				int num = (int)(Velocity.Y / -0.2f);
				_jumpTimeLeft = num * 2;
				_state = State.Jumping;
				_delayTime = Main.rand.Next(0, 11);
			}
		}

		public void Draw(ref ParticleRendererSettings settings, SpriteBatch spritebatch)
		{
			Main.instance.LoadItem(_itemInstance.type);
			float num = (float)_lifeTimeCounted / (float)_lifeTimeTotal;
			float toMin = Utils.Remap(num, 0.1f, 0.5f, 0f, 0.85f);
			toMin = Utils.Remap(num, 0.5f, 0.9f, toMin, 1f);
			Vector2 vector = settings.AnchorPosition + Position;
			float num2 = Utils.Remap(num, 0f, 0.25f, 0f, 1f) * Utils.Remap(num, 0.85f, 1f, 1f, 0f);
			Vector2 vector2 = new Vector2(_itemInstance.scale * _totalScale);
			Vector2 vector3 = ((float)Math.PI * 2f * num * 6f).ToRotationVector2();
			vector2.X += vector3.Y * 0.014f;
			vector2.Y += vector3.X * 0.012f;
			float num3 = Rotation;
			if (_itemLooksDiagonal)
			{
				num3 += (float)Math.PI / 4f;
			}
			SpriteEffects effects = SpriteEffects.None;
			Vector2 vector4 = Velocity;
			if (_itemLooksDiagonal)
			{
				if (_state == State.InterestedInBobber)
				{
					vector4 = _targetVelocity;
				}
				else if (_state == State.Latched)
				{
					vector4 = _targetVelocity;
				}
			}
			if (vector4.X < 0f)
			{
				effects = SpriteEffects.FlipVertically;
				if (_itemLooksDiagonal)
				{
					num3 -= (float)Math.PI / 2f;
				}
			}
			Main.instance.DrawItem_GetBasics(_itemInstance, 0, out var texture, out var frame, out var glowmaskFrame);
			Color color = Color.Lerp(Color.White, Color.Black, 0.8f) * 0.7f;
			if (_sonarStartTime > 0 && _sonarTimeleft <= _sonarStartTime && _sonarTimeleft > 0)
			{
				float fromValue = 1f - (float)_sonarTimeleft / (float)_sonarStartTime;
				float num4 = Utils.Remap(fromValue, 0f, 0.2f, 0f, 1f) * Utils.Remap(fromValue, 0.2f, 1f, 1f, 0f);
				color = Color.Lerp(color, Color.White, _isSelectedSonarType ? num4 : (num4 * 0.4f));
				Color color2 = _sonarColor * num4 * num2;
				for (float num5 = 0f; num5 < (float)Math.PI * 2f; num5 += (float)Math.PI / 2f)
				{
					spritebatch.Draw(texture, vector + (num5 + num3).ToRotationVector2() * 2f * vector2, frame, color2, num3, frame.Size() / 2f, vector2, effects, 0f);
				}
			}
			spritebatch.Draw(texture, vector, frame, color * num2, num3, frame.Size() / 2f, vector2, effects, 0f);
			if (_itemInstance.glowMask != -1)
			{
				spritebatch.Draw(TextureAssets.GlowMask[_itemInstance.glowMask].Value, vector, glowmaskFrame, Color.Lerp(Color.White, Color.Black, 0.8f) * num2 * 0.7f, num3, glowmaskFrame.Size() / 2f, vector2, effects, 0f);
			}
		}

		public void RestInPool()
		{
			IsRestingInPool = true;
		}

		public virtual void FetchFromPool()
		{
			_lifeTimeCounted = 0;
			_lifeTimeTotal = 0;
			IsRestingInPool = false;
			ShouldBeRemovedFromRenderer = false;
			Position = (Velocity = Vector2.Zero);
			Rotation = 0f;
			_gotRepelled = false;
		}

		public void BeRepelled(ref ParticleRepelDetails details)
		{
			if (_isInanimate || _gotRepelled || _state == State.Jumping || _state == State.Latched)
			{
				return;
			}
			float num = Position.Distance(details.SourcePosition) - details.Radius;
			if (num >= 100f)
			{
				return;
			}
			float toMin = 2f;
			if (!_gotRepelled || Main.rand.Next(10) == 0)
			{
				float num2 = Utils.Remap(num, 100f, 0f, toMin, 5f);
				Velocity = Position.DirectionFrom(details.SourcePosition).SafeNormalize(Velocity.ToRotation().ToRotationVector2()).RotatedByRandom(0.7853981852531433) * num2;
				Rotation = Velocity.ToRotation();
				Position -= Velocity;
				_targetVelocity = Velocity;
				if (Velocity != Vector2.Zero && !Collision.WetCollision(Position + Velocity.SafeNormalize(Vector2.Zero) * 32f, 0, 0))
				{
					_steerTime = 0;
					PickNewVelocity();
					Velocity = _targetVelocity * 0.1f;
				}
			}
			_delayTime = -1;
			_steerTime = Main.rand.Next(30, 121);
			_state = State.FreeRoaming;
			_gotRepelled = true;
			if ((float)_lifeTimeCounted < (float)_lifeTimeTotal * 0.7f)
			{
				float num3 = Utils.Remap((float)_lifeTimeCounted / (float)_lifeTimeTotal, 0f, 0.25f, 0f, 1f);
				float num4 = MathHelper.Lerp(1f, 0.85f, num3);
				if (num3 == 1f)
				{
					num4 = 0.7f;
				}
				_lifeTimeCounted = (int)((float)_lifeTimeTotal * num4);
			}
		}
	}
}
