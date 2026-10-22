using System;
using Microsoft.Xna.Framework;

namespace Terraria.GameContent.Items
{
	public class WhipTagEffect_Starcrash : WhipTagEffect
	{
		public override bool OnProcHit(Player owner, Projectile optionalProjectile, NPC npcHit, int calcDamage)
		{
			SpawnMeteorWhipMeteorOn(optionalProjectile, npcHit, calcDamage);
			return true;
		}

		private void SpawnMeteorWhipMeteorOn(Projectile projectile, NPC targetNPC, int calcDamage)
		{
			int num = 200;
			int num2 = 600;
			int damage = (int)((float)calcDamage * 1.33f);
			Vector2 vector = new Vector2(-num + Main.rand.Next(num * 2), -num2);
			Vector2 vector2 = targetNPC.Center + vector;
			Vector2 vector3 = vector.SafeNormalize(Vector2.Zero) * -12f;
			int num3 = 8;
			int num4 = 35;
			vector2 = targetNPC.Center + new Vector2(0f, -num3 * num4).RotatedBy(Main.rand.NextFloatDirection() * ((float)Math.PI * 2f) * 0.125f);
			vector3 = targetNPC.DirectionFrom(vector2) * num3;
			Projectile.NewProjectile(projectile.GetProjectileSource_FromThis(), vector2, vector3, 1037, damage, projectile.knockBack, projectile.owner, Main.rand.Next(3), targetNPC.position.Y);
		}
	}
}
