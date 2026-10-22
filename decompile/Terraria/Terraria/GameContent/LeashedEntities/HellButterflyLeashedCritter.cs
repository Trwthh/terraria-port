namespace Terraria.GameContent.LeashedEntities
{
	public class HellButterflyLeashedCritter : FlyLeashedCritter
	{
		public new static HellButterflyLeashedCritter Prototype = new HellButterflyLeashedCritter();

		protected override void VisualEffects()
		{
			base.VisualEffects();
			position += netOffset;
			Lighting.AddLight((int)base.Center.X / 16, (int)base.Center.Y / 16, 0.6f, 0.3f, 0.1f);
			if (Main.rand.Next(60) == 0)
			{
				int num = Dust.NewDust(position, width, height, 6, 0f, 0f, 254);
				Main.dust[num].velocity *= 0f;
			}
			position -= netOffset;
		}
	}
}
