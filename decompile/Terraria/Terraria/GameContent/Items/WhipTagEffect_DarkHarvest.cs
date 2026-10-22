using Microsoft.Xna.Framework;
using Terraria.GameContent.Drawing;
using Terraria.ID;

namespace Terraria.GameContent.Items
{
	public class WhipTagEffect_DarkHarvest : WhipTagEffect
	{
		public override void OnTaggedHit(Player owner, Projectile optionalProjectile, NPC npcHit, int calcDamage)
		{
			SpawnBlackLightning(optionalProjectile, npcHit);
		}

		private void SpawnBlackLightning(Projectile projectile, NPC npcHit)
		{
			int damage = (int)((float)TagDamage * ProjectileID.Sets.SummonTagDamageMultiplier[projectile.type]);
			int num = Projectile.NewProjectile(projectile.GetProjectileSource_FromThis(), npcHit.Center, Vector2.Zero, 916, damage, 0f, projectile.owner);
			Main.projectile[num].localNPCImmunity[npcHit.whoAmI] = -1;
			EmitBlackLightningParticles(npcHit);
		}

		private static void EmitBlackLightningParticles(NPC targetNPC)
		{
			ParticleOrchestrator.RequestParticleSpawn(clientOnly: false, ParticleOrchestraType.BlackLightningHit, new ParticleOrchestraSettings
			{
				PositionInWorld = targetNPC.Center
			});
		}
	}
}
