using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;

namespace Terraria.Graphics.Renderers
{
	public class FadingParticle : ABasicParticle
	{
		public float FadeInNormalizedTime;

		public float FadeOutNormalizedTime = 1f;

		public Color ColorTint = Color.White;

		public int Delay;

		protected float timeTolive;

		protected float timeSinceSpawn;

		protected bool fullbright = true;

		public int followPlayerIndex = -1;

		public override void FetchFromPool()
		{
			base.FetchFromPool();
			FadeInNormalizedTime = 0f;
			FadeOutNormalizedTime = 1f;
			ColorTint = Color.White;
			timeTolive = 0f;
			timeSinceSpawn = 0f;
			followPlayerIndex = -1;
			Delay = 0;
		}

		public void SetTypeInfo(float timeToLive, bool fullbright = true)
		{
			timeTolive = timeToLive;
			this.fullbright = fullbright;
		}

		public override void Update(ref ParticleRendererSettings settings)
		{
			if (Delay > 0)
			{
				Delay--;
				return;
			}
			base.Update(ref settings);
			timeSinceSpawn += 1f;
			if (timeSinceSpawn >= timeTolive)
			{
				base.ShouldBeRemovedFromRenderer = true;
			}
		}

		public override void Draw(ref ParticleRendererSettings settings, SpriteBatch spritebatch)
		{
			Vector2 position = settings.AnchorPosition + LocalPosition;
			if (followPlayerIndex != -1)
			{
				position += Main.player[followPlayerIndex].MountedCenter;
			}
			Color color = (fullbright ? ColorTint : ColorTint.MultiplyRGB(Lighting.GetColor(LocalPosition.ToTileCoordinates()))) * Utils.GetLerpValue(0f, FadeInNormalizedTime, timeSinceSpawn / timeTolive, clamped: true) * Utils.GetLerpValue(1f, FadeOutNormalizedTime, timeSinceSpawn / timeTolive, clamped: true);
			spritebatch.Draw(_texture.Value, position, _frame, color, Rotation, _origin, Scale, SpriteEffects.None, 0f);
		}
	}
}
