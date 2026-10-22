using Steamworks;
using Terraria.Social.Base;

namespace Terraria.Social.Steam
{
	public class DLCSocialModule : Terraria.Social.Base.DLCSocialModule
	{
		private const int AppId_CollectorsEdition = 4922500;

		public override void Initialize()
		{
		}

		public override void Shutdown()
		{
		}

		public override bool HasDLC(DLCName dlc)
		{
			if (dlc != DLCName.CollectorsEdition)
			{
				return false;
			}
			return SteamApps.BIsSubscribedApp(new AppId_t(4922500u));
		}
	}
}
