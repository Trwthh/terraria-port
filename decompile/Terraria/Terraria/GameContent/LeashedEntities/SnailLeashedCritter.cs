namespace Terraria.GameContent.LeashedEntities
{
	public class SnailLeashedCritter : CrawlerLeashedCritter
	{
		public new static SnailLeashedCritter Prototype = new SnailLeashedCritter();

		protected override void SetDefaults(Item sample)
		{
			base.SetDefaults(sample);
			if (npcType == 359)
			{
				scale = (float)Main.rand.Next(80, 111) * 0.01f;
			}
		}

		protected override void VisualEffects()
		{
			base.VisualEffects();
			switch (npcType)
			{
			case 360:
				Lighting.AddLight((int)base.Center.X / 16, (int)base.Center.Y / 16, 0.1f, 0.2f, 0.7f);
				break;
			case 655:
				Lighting.AddLight((int)base.Center.X / 16, (int)base.Center.Y / 16, 0.6f, 0.3f, 0.1f);
				break;
			}
		}
	}
}
