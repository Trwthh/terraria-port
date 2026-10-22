using System.Linq;

namespace Terraria.WorldBuilding
{
	public class WorldSeedOption_Normal : AWorldGenerationOption
	{
		protected override string KeyName => "Seed_Normal";

		public override string ServerConfigName => null;

		public WorldSeedOption_Normal()
		{
			base.SpecialSeedNames = new string[0];
			base.SpecialSeedValues = new int[0];
			AWorldGenerationOption.OnOptionStateChanged += UpdateDependentState;
		}

		private void UpdateDependentState(AWorldGenerationOption changed)
		{
			base.Enabled = WorldGenerationOptions.Options.All((AWorldGenerationOption x) => x == this || !x.Enabled);
		}

		protected override void OnEnabledStateChanged()
		{
			if (!base.Enabled)
			{
				return;
			}
			foreach (AWorldGenerationOption option in WorldGenerationOptions.Options)
			{
				if (option != this)
				{
					option.Enabled = false;
				}
			}
		}
	}
}
