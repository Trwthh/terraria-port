using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;
using Terraria.Enums;
using Terraria.Localization;
using Terraria.Testing.ChatCommands;
using Terraria.UI;

namespace Terraria.GameContent.UI.Elements
{
	public class UIDebugCommandItem : UIPanel
	{
		public readonly IDebugCommand Command;

		private readonly Asset<Texture2D> _dividerTexture;

		private readonly Asset<Texture2D> _innerPanelTexture;

		private readonly UIText _hoverInfoLabel;

		private ItemTooltip _preparedTooltip;

		public int Order { get; set; }

		public UIDebugCommandItem(IDebugCommand command, int order)
		{
			Command = command;
			Order = order;
			Height.Set(30f, 0f);
			Width.Set(0f, 1f);
			SetPadding(6f);
			BorderColor = Color.Transparent;
			BackgroundColor = Color.Transparent;
			_dividerTexture = Main.Assets.Request<Texture2D>("Images/UI/Divider");
			_innerPanelTexture = Main.Assets.Request<Texture2D>("Images/UI/InnerPanelBackground");
			_hoverInfoLabel = new UIText("");
			_hoverInfoLabel.VAlign = 1f;
			_hoverInfoLabel.Left.Set(80f, 0f);
			_hoverInfoLabel.Top.Set(-3f, 0f);
			Append(_hoverInfoLabel);
		}

		protected override void DrawSelf(SpriteBatch spriteBatch)
		{
			string name = Command.Name;
			string text = Command.Description ?? "";
			_ = Command.HelpText;
			_ = "Authority:  " + Command.Requirements;
			base.DrawSelf(spriteBatch);
			if (base.IsMouseHovering)
			{
				Item item = Main.DisplayAndGetFakeItem(ItemRarityColor.StrongRed10);
				item.SetNameOverride(name);
				item.ToolTip = _preparedTooltip;
			}
			CalculatedStyle innerDimensions = GetInnerDimensions();
			Vector2 vector = innerDimensions.Position() - innerDimensions.Position();
			float num = 6f;
			float num2 = vector.X + num;
			float num3 = 21f;
			FontAssets.MouseText.Value.MeasureString(name);
			Color color = Color.White;
			Color color2 = Color.Gold;
			if (!CanCurrentlyBeUsed())
			{
				color = Color.DarkGray;
				color2 = Color.DarkGray;
			}
			Utils.DrawBorderString(spriteBatch, name, innerDimensions.Position() + new Vector2(num2 + 6f, vector.Y - 2f), color, 1.1f);
			Utils.DrawBorderString(spriteBatch, text, innerDimensions.Position() + new Vector2(num2 + 6f + 180f + 16f, vector.Y + 2f + num3), color2, 0.8f, 0f, 1f);
		}

		private bool CanCurrentlyBeUsed()
		{
			if ((Command.Requirements & ~CommandRequirement.SinglePlayer) == 0 && Main.netMode != 0)
			{
				return false;
			}
			return true;
		}

		public override int CompareTo(object obj)
		{
			return Order.CompareTo(((UIDebugCommandItem)obj).Order);
		}

		public override void MouseOver(UIMouseEvent evt)
		{
			base.MouseOver(evt);
			BackgroundColor = new Color(76, 90, 149);
			BorderColor = new Color(50, 60, 86);
			_ = _preparedTooltip;
			string item = FontAssets.ItemStack.Value.CreateWrappedText((Command.Description ?? "").Replace("\n", " "), 480f, Language.ActiveCulture.CultureInfo);
			List<string> list = new List<string> { item };
			list.Add(" ");
			list.Add("Authority:  " + Command.Requirements);
			_preparedTooltip = ItemTooltip.FromHardcodedText(list.ToArray());
		}

		public override void MouseOut(UIMouseEvent evt)
		{
			base.MouseOut(evt);
			BackgroundColor = (BorderColor = Color.Transparent);
		}

		public override void LeftClick(UIMouseEvent evt)
		{
			IngameFancyUI.Close();
			Main.drawingPlayerChat = true;
			Main.chatText = "/" + Command.Name.ToLower() + " ";
			Main.NewText("Chat has been set to \"" + Main.chatText + "\"", byte.MaxValue, byte.MaxValue, 0);
		}

		private void DrawPanel(SpriteBatch spriteBatch, Vector2 position, float width)
		{
			spriteBatch.Draw(_innerPanelTexture.Value, position, new Rectangle(0, 0, 8, _innerPanelTexture.Height()), Color.White);
			spriteBatch.Draw(_innerPanelTexture.Value, new Vector2(position.X + 8f, position.Y), new Rectangle(8, 0, 8, _innerPanelTexture.Height()), Color.White, 0f, Vector2.Zero, new Vector2((width - 16f) / 8f, 1f), SpriteEffects.None, 0f);
			spriteBatch.Draw(_innerPanelTexture.Value, new Vector2(position.X + width - 8f, position.Y), new Rectangle(16, 0, 8, _innerPanelTexture.Height()), Color.White);
		}
	}
}
