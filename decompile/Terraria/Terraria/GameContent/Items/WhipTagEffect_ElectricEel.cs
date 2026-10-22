using Microsoft.Xna.Framework;
using Terraria.Audio;
using Terraria.GameContent.Drawing;
using Terraria.ID;

namespace Terraria.GameContent.Items
{
	public class WhipTagEffect_ElectricEel : WhipTagEffect
	{
		public override void ModifyProcHit(Player owner, Projectile optionalProjectile, NPC npcHit, ref TagDamageChanges changes)
		{
			CreateLightningBlast(owner, npcHit, ref changes);
		}

		public override bool OnProcHit(Player owner, Projectile optionalProjectile, NPC npcHit, int calcDamage)
		{
			return true;
		}

		public static void CreateLightningBlast(Player owner, NPC npcHit, ref TagDamageChanges changes)
		{
			int num = 0;
			int num2 = 5;
			int num3 = 50;
			int num4 = 50;
			int num5 = 450;
			int num6 = 20;
			if (0 == 1)
			{
				num6 = (int)((float)num6 * owner.minionDamage);
			}
			for (int i = 0; i < Main.maxNPCs; i++)
			{
				if (num >= num2)
				{
					break;
				}
				if (i != npcHit.whoAmI)
				{
					NPC nPC = Main.npc[i];
					if (nPC.CanBeChasedBy() && (npcHit.realLife == -1 || npcHit.realLife != nPC.realLife) && !(npcHit.Distance(nPC.Hitbox.ClosestPointInRect(npcHit.Center)) > (float)num5))
					{
						num++;
						Vector2 vector = Main.rand.NextVector2FromRectangle(npcHit.Hitbox);
						Vector2 vector2 = Main.rand.NextVector2FromRectangle(nPC.Hitbox);
						ParticleOrchestrator.RequestParticleSpawn(clientOnly: false, ParticleOrchestraType.BlueLightningSmallLong, new ParticleOrchestraSettings
						{
							PositionInWorld = vector2,
							MovementVector = vector - vector2
						});
						owner.TryHittingNPC(nPC, num6, 0f, null, 5478, 0, 0);
					}
				}
			}
			SoundEngine.PlaySound(SoundID.NPCHit34, npcHit.Center);
			changes.AddedFlatDamage += num3 + num * num4;
		}
	}
}
