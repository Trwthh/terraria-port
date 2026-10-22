namespace Terraria.WorldBuilding
{
	public class WorldSeedOption_Anniversary : AWorldGenerationOption
	{
		protected override string KeyName => "Seed_Celebration";

		public override string ServerConfigName => "celebration";

		public WorldSeedOption_Anniversary()
		{
			base.SpecialSeedNames = new string[1] { "celebrationmk10" };
			base.SpecialSeedValues = new int[2] { 5162021, 5162011 };
		}
	}
}
