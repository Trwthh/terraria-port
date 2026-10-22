using Microsoft.Xna.Framework;

namespace Terraria.GameContent.LeashedEntities
{
	public class EmpressButterflyLeashedCritter : FlyLeashedCritter
	{
		public new static EmpressButterflyLeashedCritter Prototype = new EmpressButterflyLeashedCritter();

		private float fadeAmount;

		private const int FadeAwayCap = 50;

		private float Opacity => Utils.GetLerpValue(60f, 25f, fadeAmount, clamped: true);

		protected override void CopyToDummy()
		{
			base.CopyToDummy();
			LeashedCritter._dummy.ai[2] = fadeAmount;
			LeashedCritter._dummy.Opacity = Opacity;
		}

		protected override void VisualEffects()
		{
			base.VisualEffects();
			Vector3 rgb = Main.hslToRgb(Main.GlobalTimeWrappedHourly * 0.33f % 1f, 1f, 0.5f).ToVector3() * 0.3f;
			rgb += Vector3.One * 0.1f;
			Lighting.AddLight(base.Center, rgb);
			bool value = Main.LocalPlayer.Center.Distance(base.Center) > 300f;
			fadeAmount = MathHelper.Clamp(fadeAmount + (float)value.ToDirectionInt(), 0f, 50f);
			if (!(fadeAmount > 0f))
			{
				return;
			}
			float opacity = Opacity;
			int num = 1;
			for (int i = 0; i < num; i++)
			{
				if (Main.rand.Next(5) == 0)
				{
					float num2 = MathHelper.Lerp(0.9f, 0.6f, opacity);
					Color newColor = Main.hslToRgb(Main.GlobalTimeWrappedHourly * 0.3f % 1f, 1f, 0.5f) * 0.5f;
					int num3 = Dust.NewDust(position, width, height, 267, 0f, 0f, 0, newColor);
					Main.dust[num3].position = base.Center + Main.rand.NextVector2Circular(width, height);
					Main.dust[num3].velocity *= Main.rand.NextFloat() * 0.8f;
					Main.dust[num3].velocity += velocity * 0.6f;
					Main.dust[num3].noGravity = true;
					Main.dust[num3].fadeIn = 0.6f + Main.rand.NextFloat() * 0.7f * num2;
					Main.dust[num3].scale = 0.35f;
					if (num3 != 6000)
					{
						Dust dust = Dust.CloneDust(num3);
						dust.scale /= 2f;
						dust.fadeIn *= 0.85f;
						dust.color = new Color(255, 255, 255, 255) * 0.5f;
					}
				}
			}
		}
	}
}
