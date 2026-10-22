namespace Terraria.WorldBuilding
{
	public class WorldSeedOption_NotTheBees : AWorldGenerationOption
	{
		protected override string KeyName => "Seed_NotTheBees";

		public override string ServerConfigName => "notthebees";

		public WorldSeedOption_NotTheBees()
		{
			base.SpecialSeedNames = new string[1] { "notthebees" };
			base.SpecialSeedValues = new int[0];
		}
	}
}
