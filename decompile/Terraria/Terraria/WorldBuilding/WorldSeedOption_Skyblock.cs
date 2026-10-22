namespace Terraria.WorldBuilding
{
	public class WorldSeedOption_Skyblock : AWorldGenerationOption
	{
		protected override string KeyName => "Seed_Skyblock";

		public override string ServerConfigName => "skyblock";

		public WorldSeedOption_Skyblock()
		{
			base.SpecialSeedNames = new string[1] { "skyblock" };
			base.SpecialSeedValues = new int[0];
		}
	}
}
