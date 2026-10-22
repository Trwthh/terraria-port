using System.Linq;
using Microsoft.Xna.Framework;
using Terraria.Localization;

namespace Terraria.Chat.Commands
{
	[ChatCommand("AllPVPDeath")]
	public class AllPVPDeathCommand : IChatCommand
	{
		private static readonly Color RESPONSE_COLOR = ChatColors.Death;

		public void ProcessIncomingMessage(string text, byte clientId)
		{
			foreach (Player item in from x in Main.player
				where x?.active ?? false
				orderby x.numberOfDeathsPVP descending
				select x)
			{
				NetworkText text2 = NetworkText.FromKey("LegacyMultiplayer.24", item.name, item.numberOfDeathsPVP);
				if (item.numberOfDeathsPVP == 1)
				{
					text2 = NetworkText.FromKey("LegacyMultiplayer.26", item.name, item.numberOfDeathsPVP);
				}
				ChatHelper.BroadcastChatMessage(text2, RESPONSE_COLOR);
			}
		}

		public void ProcessOutgoingMessage(ChatMessage message)
		{
		}
	}
}
