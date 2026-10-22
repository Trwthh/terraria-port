namespace Terraria.WorldBuilding
{
	public class WorldSeedOption_DontStarve : AWorldGenerationOption
	{
		protected override string KeyName => "Seed_TheConstant";

		public override string ServerConfigName => "theconstant";

		public WorldSeedOption_DontStarve()
		{
			base.SpecialSeedNames = new string[4] { "constant", "theconstant", "eye4aneye", "eyeforaneye" };
			base.SpecialSeedValues = new int[0];
		}
	}
}
