using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.DataStructures;
using Terraria.GameContent;

namespace Terraria.Graphics.Renderers
{
	public class ShockIconParticle : ABasicParticle
	{
		public float FadeInNormalizedTime = 0.25f;

		public float FadeOutNormalizedTime = 0.75f;

		public float TimeToLive = 20f;

		public float Opacity;

		public float InitialScale = 1f;

		public Color ColorTint = Color.White;

		public ProjectileKey ParentProjectileKey;

		public Vector2 OffsetFromParent;

		private Vector2 initialPosition;

		private float _timeSinceSpawn;

		public override void FetchFromPool()
		{
			base.FetchFromPool();
			_timeSinceSpawn = 0f;
			Opacity = 0f;
			FadeInNormalizedTime = 0.1f;
			FadeOutNormalizedTime = 0.9f;
			TimeToLive = 20f;
			InitialScale = 1f;
			ColorTint = Color.White;
		}

		public override void Update(ref ParticleRendererSettings settings)
		{
			if (_timeSinceSpawn == 0f)
			{
				initialPosition = LocalPosition;
			}
			base.Update(ref settings);
			_timeSinceSpawn += 1f;
			float num = _timeSinceSpawn / TimeToLive;
			Scale = Vector2.One * InitialScale * Utils.MultiLerp(num, 0.2f, 0.9f, 1.3f, 0.9f);
			Opacity = MathHelper.Clamp(Utils.Remap(num, 0f, FadeInNormalizedTime, 0f, 1f) * Utils.Remap(num, FadeOutNormalizedTime, 1f, 1f, 0f), 0f, 1f) * 0.5f;
			if (ParentProjectileKey.TryGetActive(out var proj))
			{
				LocalPosition = proj.Top + num * OffsetFromParent;
			}
			else
			{
				LocalPosition = initialPosition + num * OffsetFromParent;
			}
			if (_timeSinceSpawn >= TimeToLive)
			{
				base.ShouldBeRemovedFromRenderer = true;
			}
		}

		public override void Draw(ref ParticleRendererSettings settings, SpriteBatch spritebatch)
		{
			Vector2 position = settings.AnchorPosition + LocalPosition;
			Texture2D value = TextureAssets.Extra[268].Value;
			Vector2 origin = new Vector2(value.Width / 2, value.Height / 2);
			Vector2 scale = Scale;
			Color color = Color.Lerp(Lighting.GetColor(LocalPosition.ToTileCoordinates()).MultiplyRGBA(ColorTint), ColorTint, 0.75f) * Opacity;
			spritebatch.Draw(value, position, value.Frame(), color, Rotation, origin, scale, SpriteEffects.None, 0f);
		}
	}
}
