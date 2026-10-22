using Terraria.GameContent.Drawing;

namespace Terraria.GameContent.Items
{
	public class WhipTagEffect_Kaleidoscope : WhipTagEffect
	{
		public override void OnTaggedHit(Player owner, Projectile optionalProjectile, NPC npcHit, int calcDamage)
		{
			ParticleOrchestrator.RequestParticleSpawn(clientOnly: false, ParticleOrchestraType.RainbowRodHit, new ParticleOrchestraSettings
			{
				PositionInWorld = optionalProjectile.Center
			});
		}
	}
}
