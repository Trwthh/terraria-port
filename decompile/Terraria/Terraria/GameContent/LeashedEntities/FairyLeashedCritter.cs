using Microsoft.Xna.Framework;

namespace Terraria.GameContent.LeashedEntities
{
	public class FairyLeashedCritter : FlyerLeashedCritter
	{
		public new static FairyLeashedCritter Prototype = new FairyLeashedCritter();

		public FairyLeashedCritter()
		{
			minWaitTime = 30;
			maxWaitTime = 90;
			maxFlySpeed = 1.1f;
			acceleration = 0.05f;
			rotationScalar = 0.25f;
			brakeDuration = 30;
		}

		protected override void VisualEffects()
		{
			base.VisualEffects();
			Color value = Color.HotPink;
			Color value2 = Color.LightPink;
			int num = 4;
			if (npcType == 584)
			{
				value = Color.LimeGreen;
				value2 = Color.LightSeaGreen;
			}
			if (npcType == 585)
			{
				value = Color.RoyalBlue;
				value2 = Color.LightBlue;
			}
			if ((int)Main.timeForVisualEffects % 4 == 0 && Main.rand.Next(4) != 0)
			{
				position += netOffset;
				Dust dust = Dust.NewDustDirect(base.Center - new Vector2(4f) + Main.rand.NextVector2Circular(2f, 2f), num, num, 278, 0f, 0f, 200, Color.Lerp(value, value2, Main.rand.NextFloat()), 0.65f);
				dust.velocity *= 0f;
				dust.velocity += velocity * 0.3f;
				dust.noGravity = true;
				dust.noLight = true;
				position -= netOffset;
			}
			Lighting.AddLight(base.Center, value.ToVector3() * 0.7f);
		}
	}
}
