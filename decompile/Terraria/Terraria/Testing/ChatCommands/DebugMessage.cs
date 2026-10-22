using System;
using System.IO;
using Microsoft.Xna.Framework;
using Terraria.Chat;
using Terraria.Localization;

namespace Terraria.Testing.ChatCommands
{
	public class DebugMessage
	{
		private const char COMMAND_PREFIX = '/';

		public readonly byte Author;

		public readonly string CommandName = "";

		public readonly string Arguments = "";

		public readonly Vector2 MousePosition;

		public DebugMessage(byte author, string message)
			: this(author, message, new Vector2(Main.mouseX, Main.mouseY) + Main.screenPosition)
		{
		}

		private DebugMessage(byte author, string message, Vector2 mousePosition)
		{
			MousePosition = mousePosition;
			Author = author;
			if (message[0] != '/')
			{
				return;
			}
			string text = message.ToLower();
			int num = text.Length;
			for (int i = 0; i < text.Length; i++)
			{
				if (text[i] == ' ')
				{
					num = i;
					break;
				}
			}
			if ((CommandName = text.Substring(1, num - 1)).Length != 0 && num < message.Length - 1)
			{
				Arguments = message.Substring(num + 1);
			}
		}

		private DebugMessage(byte author, string commandName, string arguments, Vector2 mousePosition)
		{
			Author = author;
			CommandName = commandName;
			Arguments = arguments;
			MousePosition = mousePosition;
		}

		public void Reply(string message)
		{
			DisplayMessage(message, ChatColors.Command);
		}

		public void ReplyError(string message)
		{
			DisplayMessage(message, ChatColors.Error);
		}

		private void DisplayMessage(string message, Color color)
		{
			if (Main.dedServ && Author == byte.MaxValue)
			{
				Console.WriteLine(message);
			}
			else
			{
				ChatHelper.DisplayMessageOnClient(NetworkText.FromLiteral(message), color, Author);
			}
		}

		public void Serialize(BinaryWriter writer)
		{
			writer.Write(CommandName);
			writer.Write(Arguments);
			writer.WriteVector2(MousePosition);
		}

		public static DebugMessage Deserialize(byte author, BinaryReader reader)
		{
			string commandName = reader.ReadString();
			string arguments = reader.ReadString();
			Vector2 mousePosition = reader.ReadVector2();
			return new DebugMessage(author, commandName, arguments, mousePosition);
		}

		public DebugMessage CreateSubMessage(string newMessage)
		{
			return new DebugMessage(Author, newMessage, MousePosition);
		}
	}
}
