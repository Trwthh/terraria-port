using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.GameContent;

namespace Terraria.Graphics.Renderers
{
	public class BloodyExplosionParticle : ABasicParticle
	{
		public float FadeInNormalizedTime = 0.25f;

		public float FadeOutNormalizedTime = 0.75f;

		public float TimeToLive = 20f;

		public float Opacity;

		public float InnerOpacity;

		public float InitialScale = 1f;

		public Color ColorTint = Color.White;

		public Color LightColorTint = Color.Transparent;

		private float _timeSinceSpawn;

		public override void FetchFromPool()
		{
			base.FetchFromPool();
			_timeSinceSpawn = 0f;
			Opacity = 0f;
			InnerOpacity = 0f;
			FadeInNormalizedTime = 0.1f;
			FadeOutNormalizedTime = 0.9f;
			TimeToLive = 20f;
			InitialScale = 1f;
			ColorTint = Color.White;
			LightColorTint = Color.Transparent;
		}

		public override void Update(ref ParticleRendererSettings settings)
		{
			base.Update(ref settings);
			_timeSinceSpawn += 1f;
			float fromValue = _timeSinceSpawn / TimeToLive;
			Scale = Vector2.One * InitialScale * Utils.Remap(fromValue, 0f, 0.3f, 0.5f, 1f);
			float num = 0.4f;
			Opacity = MathHelper.Clamp(Utils.Remap(fromValue, 0f, FadeInNormalizedTime, 0f, 1f) * Utils.Remap(fromValue, FadeOutNormalizedTime, 1f, 1f, 0f), 0f, 1f) * num;
			InnerOpacity = MathHelper.Clamp(Utils.Remap(fromValue, 0f, FadeInNormalizedTime * 0.75f, 0f, 1f) * Utils.Remap(fromValue, 0.3f, 0.45f, 1f, 0f), 0f, 1f) * num;
			if (_timeSinceSpawn == 3f)
			{
				Rectangle r = Utils.CenteredRectangle(LocalPosition, new Vector2(16f, 16f));
				for (int i = 0; i < 50; i++)
				{
					Vector2 velocity = Main.rand.NextVector2CircularEdge(4f, 4f);
					if (i % 2 == 0)
					{
						velocity *= 0.5f;
					}
					Dust obj = Main.dust[Dust.NewDust(r.TopLeft(), r.Width, r.Height, 5, 0f, 0f, 100, default(Color), 1.25f + Main.rand.NextFloat() * 0.5f)];
					obj.velocity = velocity;
					obj.noGravity = i % 3 == 0;
				}
			}
			if (LightColorTint != Color.Transparent)
			{
				Color color = LightColorTint * Opacity;
				Lighting.AddLight(LocalPosition, (float)(int)color.R / 255f, (float)(int)color.G / 255f, (float)(int)color.B / 255f);
			}
			if (_timeSinceSpawn >= TimeToLive)
			{
				base.ShouldBeRemovedFromRenderer = true;
			}
		}

		public override void Draw(ref ParticleRendererSettings settings, SpriteBatch spritebatch)
		{
			float num = _timeSinceSpawn / TimeToLive;
			Vector2 position = settings.AnchorPosition + LocalPosition;
			Color color = Color.Lerp(Lighting.GetColor(LocalPosition.ToTileCoordinates()).MultiplyRGBA(ColorTint), ColorTint, 0.65f);
			Texture2D value = TextureAssets.Extra[174].Value;
			Vector2 origin = new Vector2(value.Width / 2, value.Height / 2);
			Vector2 scale = Scale * (1.1f + 0.15f * num);
			Color color2 = color * Opacity;
			Texture2D value2 = TextureAssets.Extra[267].Value;
			Vector2 origin2 = new Vector2(value2.Width / 2, value2.Height / 2);
			Vector2 scale2 = Scale * (1f + 0.05f * num);
			Color color3 = color * InnerOpacity;
			spritebatch.Draw(value, position, value.Frame(), color2, Rotation, origin, scale, SpriteEffects.None, 0f);
			spritebatch.Draw(value2, position, value2.Frame(), color3, Rotation, origin2, scale2, SpriteEffects.None, 0f);
		}
	}
}
