using Steamworks;

namespace Terraria.Net
{
	public class SteamAddress : RemoteAddress
	{
		public readonly CSteamID SteamId;

		private string _friendlyName;

		public SteamAddress(CSteamID steamId)
		{
			Type = AddressType.Steam;
			SteamId = steamId;
		}

		public override string ToString()
		{
			return GetIdentifier() + " (" + GetFriendlyName() + ")";
		}

		public override string GetIdentifier()
		{
			string text = (SteamId.m_SteamID % 2).ToString();
			string text2 = ((SteamId.m_SteamID - (76561197960265728L + SteamId.m_SteamID % 2)) / 2).ToString();
			return "STEAM_0:" + text + ":" + text2;
		}

		public override bool IsLocalHost()
		{
			if (Program.LaunchParameters.ContainsKey("-localsteamid"))
			{
				return Program.LaunchParameters["-localsteamid"].Equals(SteamId.m_SteamID.ToString());
			}
			return false;
		}

		public override string GetFriendlyName()
		{
			if (_friendlyName == null)
			{
				_friendlyName = SteamFriends.GetFriendPersonaName(SteamId);
			}
			return _friendlyName;
		}
	}
}
