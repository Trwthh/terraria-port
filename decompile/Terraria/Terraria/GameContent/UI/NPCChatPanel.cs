using System;
using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Graphics;
using ReLogic.Localization.IME;
using ReLogic.OS;
using Terraria.Audio;
using Terraria.GameContent.UI.States;
using Terraria.GameInput;
using Terraria.Localization;
using Terraria.UI;
using Terraria.UI.Chat;
using Terraria.UI.Gamepad;

namespace Terraria.GameContent.UI
{
	public class NPCChatPanel
	{
		private int textBlinkerCount;

		private int textBlinkerState;

		private List<NPCInteraction> _interactions = new List<NPCInteraction>();

		private TextDisplayCache _textDisplayCache = new TextDisplayCache();

		private int _neededInteractionLines;

		public const int AllowedInteractionsPerLine = 4;

		private int _lastHovered = -1;

		private Player LocalPlayer => Main.LocalPlayer;

		private byte mouseTextColor => Main.mouseTextColor;

		public bool allowRichText => LocalPlayer.talkNPC != -1;

		public bool InVirtualKeyboard
		{
			get
			{
				if (Main.InGameUI.CurrentState is UIVirtualKeyboard)
				{
					return PlayerInput.UsingGamepad;
				}
				return false;
			}
		}

		public void Draw()
		{
			if (!CanHoldConversation())
			{
				Close();
				return;
			}
			PrepareText();
			PrepareInteractions();
			PrepareVirtualKeyboard();
			Color color = new Color(200, 200, 200, 200);
			int num = (mouseTextColor * 2 + 255) / 3;
			Color color2 = new Color(num, num, num, num);
			Point point = new Point(500, 500);
			Rectangle rectangle = new Rectangle(Main.screenWidth / 2 - point.X / 2, 100, point.X, 30);
			rectangle.Height += 30 * _textDisplayCache.AmountOfLines;
			rectangle.Height += 30 * _neededInteractionLines + Math.Max(0, 2 * (_neededInteractionLines - 1));
			if (true)
			{
				Main.spriteBatch.Draw(TextureAssets.ChatBack.Value, rectangle.TopLeft(), new Rectangle(0, 0, TextureAssets.ChatBack.Width(), rectangle.Height - 30), color, 0f, default(Vector2), 1f, SpriteEffects.None, 0f);
				Main.spriteBatch.Draw(TextureAssets.ChatBack.Value, new Vector2(rectangle.TopLeft().X, rectangle.Y + rectangle.Height - 30), new Rectangle(0, TextureAssets.ChatBack.Height() - 30, TextureAssets.ChatBack.Width(), 30), color, 0f, default(Vector2), 1f, SpriteEffects.None, 0f);
			}
			else
			{
				Utils.DrawInvBG(Main.spriteBatch, rectangle);
			}
			DrawText(color2, rectangle);
			Main.DrawNPCPortrait(color, rectangle.TopLeft());
			Main.DrawNPCChatBottomRightItem(rectangle.BottomRight());
			if (!PlayerInput.IgnoreMouseInterface && rectangle.Contains(new Point(Main.mouseX, Main.mouseY)))
			{
				LocalPlayer.mouseInterface = true;
			}
			DrawButtons(rectangle, color2);
		}

		private void DrawButtons(Rectangle panelArea, Color chatColor)
		{
			UILinkPointNavigator.Shortcuts.NPCCHAT_ButtonsNew = true;
			UILinkPointNavigator.Shortcuts.NPCCHAT_ButtonsCount = _interactions.Count;
			DynamicSpriteFont value = FontAssets.MouseText.Value;
			Vector2 vector = panelArea.BottomLeft() + new Vector2(30f, -22 * _neededInteractionLines + Math.Max(0, 2 * (_neededInteractionLines - 1)) - 4);
			int num = -1;
			int num2 = -1;
			float num3 = 0.9f;
			Rectangle rectangle = new Rectangle((int)vector.X, (int)vector.Y, 100, 22);
			foreach (NPCInteraction interaction in _interactions)
			{
				num++;
				byte b = mouseTextColor;
				chatColor = new Color(b, (int)((double)(int)b / 1.1), b / 2, b);
				if (num % 4 == 0)
				{
					rectangle.X = (int)vector.X;
					rectangle.Y = num / 4 * 22 + (int)vector.Y;
				}
				string text = interaction.GetText();
				int coinValue = 0;
				bool num4 = interaction.TryAddCoins(ref chatColor, out coinValue);
				float num5 = 1f;
				Vector2 stringSize = ChatManager.GetStringSize(value, text, new Vector2(num3));
				if (stringSize.X > 260f)
				{
					num5 *= 260f / stringSize.X;
				}
				rectangle.Width = (int)(stringSize.X * num5);
				bool flag = rectangle.Contains(new Point(Main.mouseX, Main.mouseY));
				Vector2 vector2 = new Vector2(flag ? 1.2f : num3);
				Vector2 origin = new Vector2(0f, stringSize.Y * 0.5f);
				Color baseColor = (flag ? Color.Brown : Color.Black);
				Vector2 vector3 = new Vector2(rectangle.Left, rectangle.Center.Y);
				if (flag)
				{
					vector3.X -= (int)((1.2f - num3) * (float)rectangle.Width * 0.5f);
					stringSize *= 1.2f / num3;
				}
				if (flag)
				{
					num2 = num;
				}
				ChatManager.DrawColorCodedStringShadow(Main.spriteBatch, value, text, vector3, baseColor, 0f, origin, vector2 * num5);
				ChatManager.DrawColorCodedString(Main.spriteBatch, value, text, vector3, chatColor, 0f, origin, vector2 * num5);
				UILinkPointNavigator.SetPosition(2500 + num, rectangle.Center.ToVector2());
				rectangle.X += rectangle.Width + 30;
				if (interaction.ShowExclamation)
				{
					Utils.DrawNotificationIcon(Main.spriteBatch, vector3 + new Vector2(stringSize.X * num5, 0f) + new Vector2(8f, 0f), 0f);
				}
				if (num4)
				{
					ItemSlot.DrawMoney(Main.spriteBatch, "", rectangle.X - 45, rectangle.Y - 44, Utils.CoinsSplit(coinValue), horizontal: true);
					rectangle.X += 106;
				}
				if (!PlayerInput.IgnoreMouseInterface && flag)
				{
					LocalPlayer.mouseInterface = true;
					LocalPlayer.releaseUseItem = false;
					num2 = num;
					if (Main.mouseLeft && Main.mouseLeftRelease)
					{
						Main.mouseLeftRelease = false;
						interaction.Interact();
					}
				}
			}
			if (_lastHovered != num2 && (!PlayerInput.UsingGamepad || num2 != -1))
			{
				SoundEngine.PlaySound(12);
			}
			_lastHovered = num2;
		}

		private void PrepareInteractions()
		{
			_interactions.Clear();
			foreach (NPCInteraction item in NPCInteractions.All)
			{
				if (item.Condition())
				{
					_interactions.Add(item);
				}
			}
			int count = _interactions.Count;
			_neededInteractionLines = (int)Math.Ceiling((float)count / 4f);
		}

		private void PrepareVirtualKeyboard()
		{
			int num = 120 + _textDisplayCache.AmountOfLines * 30 + 30;
			num -= 235;
			UIVirtualKeyboard.ShouldHideText = !PlayerInput.SettingsForUI.ShowGamepadHints;
			if (!PlayerInput.UsingGamepad)
			{
				num = 9999;
			}
			UIVirtualKeyboard.OffsetDown = num;
		}

		private void PrepareText()
		{
			string chatTextToShow = Main.npcChatText;
			OverrideChatTextWithShenanigans(ref chatTextToShow);
			_textDisplayCache.PrepareCache(chatTextToShow);
		}

		private void OverrideChatTextWithShenanigans(ref string chatTextToShow)
		{
			bool num = LocalPlayer.talkNPC != -1 && Main.CanDryadPlayStardewAnimation(LocalPlayer, Main.npc[LocalPlayer.talkNPC]);
			int num2 = 24;
			if (LocalPlayer.talkNPC != -1 && Main.npc[LocalPlayer.talkNPC].ai[0] == (float)num2 && NPC.RerollDryadText == 2)
			{
				NPC.RerollDryadText = 1;
			}
			if (num && NPC.RerollDryadText == 1 && Main.npc[LocalPlayer.talkNPC].ai[0] != (float)num2 && LocalPlayer.talkNPC != -1 && Main.npc[LocalPlayer.talkNPC].active && Main.npc[LocalPlayer.talkNPC].type == 20)
			{
				NPC.RerollDryadText = 0;
				chatTextToShow = (Main.npcChatText = Main.npc[LocalPlayer.talkNPC].GetChat());
				NPC.PreventJojaColaDialog = true;
			}
			if (num && !NPC.PreventJojaColaDialog)
			{
				chatTextToShow = Language.GetTextValue("StardewTalk.PlayerHasColaAndIsHoldingIt");
			}
		}

		private void DrawText(Color textColor, Rectangle textArea)
		{
			Vector2 vector = textArea.TopLeft() + new Vector2(20f, 20f);
			DynamicSpriteFont value = FontAssets.MouseText.Value;
			string[] textLines = _textDisplayCache.TextLines;
			int amountOfLines = _textDisplayCache.AmountOfLines;
			for (int i = 0; i < amountOfLines; i++)
			{
				string text = textLines[i];
				if (text != null)
				{
					if (allowRichText)
					{
						ChatManager.DrawColorCodedStringWithShadow(Main.spriteBatch, value, text, vector + new Vector2(0f, i * 30), textColor, 0f, Vector2.Zero, Vector2.One);
					}
					else
					{
						Utils.DrawBorderStringFourWay(Main.spriteBatch, value, text, vector.X, vector.Y + (float)(i * 30), textColor, Color.Black, Vector2.Zero);
					}
				}
			}
			if (!Main.editSign || textLines[amountOfLines - 1] == null)
			{
				return;
			}
			Vector2 vector2 = vector + new Vector2(0f, (amountOfLines - 1) * 30);
			vector2.X += value.MeasureString(textLines[amountOfLines - 1]).X;
			string compositionString = Platform.Get<IImeService>().CompositionString;
			if (compositionString != null && compositionString.Length > 0)
			{
				float x = value.MeasureString(compositionString).X;
				if (x + vector2.X - vector.X > 460f)
				{
					vector2 = vector + new Vector2(0f, amountOfLines * 30);
				}
				ChatManager.DrawColorCodedStringWithShadow(Main.spriteBatch, value, compositionString, vector2, Main.imeCompositionStringColor, 0f, Vector2.Zero, Vector2.One);
				Main.instance.SetIMEPanelAnchor(vector2 + new Vector2(0f, 54f), 0f);
				vector2.X += x;
			}
			if (++textBlinkerCount >= 20)
			{
				textBlinkerState = ((textBlinkerState == 0) ? 1 : 0);
				textBlinkerCount = 0;
			}
			if (textBlinkerState == 1)
			{
				ChatManager.DrawColorCodedStringWithShadow(Main.spriteBatch, value, "|", vector2, textColor, 0f, Vector2.Zero, Vector2.One);
			}
		}

		public void Close()
		{
			_lastHovered = -1;
			ClearNPCChatText();
		}

		private void ClearNPCChatText()
		{
			Main.npcChatText = "";
		}

		public bool CanHoldConversation()
		{
			if (LocalPlayer.talkNPC < 0)
			{
				return LocalPlayer.sign != -1;
			}
			return true;
		}
	}
}
