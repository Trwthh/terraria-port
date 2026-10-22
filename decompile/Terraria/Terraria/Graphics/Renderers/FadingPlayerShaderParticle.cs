using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.DataStructures;

namespace Terraria.Graphics.Renderers
{
	public class FadingPlayerShaderParticle : FadingParticle
	{
		private Player _player;

		private int _shader;

		public override void FetchFromPool()
		{
			base.FetchFromPool();
			_player = null;
			_shader = 0;
		}

		public void SetTypeInfo(float timeToLive, Player player, int shader, bool fullbright = true)
		{
			SetTypeInfo(timeToLive, fullbright);
			_player = player;
			_shader = shader;
		}

		public override void Draw(ref ParticleRendererSettings settings, SpriteBatch spritebatch)
		{
			if (_player == null || _shader == 0)
			{
				base.Draw(ref settings, spritebatch);
				return;
			}
			Effect pixelShader = Main.pixelShader;
			Color color = (fullbright ? ColorTint : ColorTint.MultiplyRGB(Lighting.GetColor(LocalPosition.ToTileCoordinates()))) * Utils.GetLerpValue(0f, FadeInNormalizedTime, timeSinceSpawn / timeTolive, clamped: true) * Utils.GetLerpValue(1f, FadeOutNormalizedTime, timeSinceSpawn / timeTolive, clamped: true);
			DrawData cdd = new DrawData
			{
				texture = _texture.Value,
				sourceRect = _texture.Frame(),
				shader = _shader
			};
			PlayerDrawHelper.SetShaderForData(_player, _shader, ref cdd);
			spritebatch.Draw(_texture.Value, settings.AnchorPosition + LocalPosition, _frame, color, Rotation, _origin, Scale, SpriteEffects.None, 0f);
			pixelShader.CurrentTechnique.Passes[0].Apply();
		}
	}
}
