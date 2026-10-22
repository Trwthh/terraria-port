using Microsoft.Xna.Framework;
using Terraria.GameContent;

namespace Terraria.Chat.Commands
{
	[ChatCommand("BossDamage")]
	public class BossDamageCommand : IChatCommand
	{
		private static readonly Color RESPONSE_COLOR = ChatColors.World;

		public void ProcessIncomingMessage(string text, byte clientId)
		{
			foreach (NPCDamageTracker item in NPCDamageTracker.RecentAttempts())
			{
				for (int i = 0; i < 255; i++)
				{
					if (Main.player[i].active)
					{
						ChatHelper.SendChatMessageToClient(item.GetReport(Main.player[i]), RESPONSE_COLOR, i);
					}
				}
			}
		}

		public void ProcessOutgoingMessage(ChatMessage message)
		{
		}
	}
}
