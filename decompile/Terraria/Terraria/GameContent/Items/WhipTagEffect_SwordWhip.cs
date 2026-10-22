using Microsoft.Xna.Framework;
using Terraria.GameContent.Drawing;

namespace Terraria.GameContent.Items
{
	public class WhipTagEffect_SwordWhip : WhipTagEffect
	{
		public override void ModifyProcHit(Player owner, Projectile optionalProjectile, NPC npcHit, ref TagDamageChanges changes)
		{
			MegaSlash(npcHit, ref changes);
		}

		public override bool OnProcHit(Player owner, Projectile optionalProjectile, NPC npcHit, int calcDamage)
		{
			return true;
		}

		public static void MegaSlash(NPC npcHit, ref TagDamageChanges changes)
		{
			int num = 30;
			Vector2 positionInWorld = Main.rand.NextVector2FromRectangle(npcHit.Hitbox);
			ParticleOrchestrator.RequestParticleSpawn(clientOnly: false, ParticleOrchestraType.TrueExcalibur, new ParticleOrchestraSettings
			{
				PositionInWorld = positionInWorld
			});
			changes.AddedFlatDamage += num;
		}
	}
}
