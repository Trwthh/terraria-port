using System;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.GameContent.UI.Chat;
using Terraria.UI;
using Terraria.UI.Chat;

namespace Terraria.GameContent.UI.Elements
{
	public class UIKeybindingCycleListItem : UIElement
	{
		private Color _color;

		private Func<string> _getTitle;

		private Func<string> _getValue;

		public UIKeybindingCycleListItem(Func<string> getTitle, Func<string> getValue, Color color)
		{
			_color = color;
			_getTitle = ((getTitle != null) ? getTitle : ((Func<string>)(() => "???")));
			_getValue = ((getValue != null) ? getValue : ((Func<string>)(() => "???")));
		}

		protected override void DrawSelf(SpriteBatch spriteBatch)
		{
			float num = 6f;
			base.DrawSelf(spriteBatch);
			CalculatedStyle dimensions = GetDimensions();
			float num2 = dimensions.Width + 1f;
			Vector2 vector = new Vector2(dimensions.X, dimensions.Y);
			Vector2 vector2 = new Vector2(0.8f);
			Color value = (base.IsMouseHovering ? Color.White : Color.Silver);
			value = Color.Lerp(value, Color.White, base.IsMouseHovering ? 0.5f : 0f);
			Color color = (base.IsMouseHovering ? _color : _color.MultiplyRGBA(new Color(180, 180, 180)));
			Vector2 position = vector;
			Utils.DrawSettingsPanel(spriteBatch, position, num2, color);
			position.X += 8f;
			position.Y += 2f + num;
			ChatManager.DrawColorCodedStringWithShadow(spriteBatch, FontAssets.ItemStack.Value, _getTitle(), position, value, 0f, Vector2.Zero, vector2, num2);
			position.X -= 17f;
			string text = _getValue();
			Vector2 stringSize = ChatManager.GetStringSize(FontAssets.ItemStack.Value, text, vector2);
			position = new Vector2(dimensions.X + dimensions.Width - stringSize.X - 10f, dimensions.Y + 2f + num);
			GlyphTagHandler.GlyphsScale = 0.85f;
			ChatManager.DrawColorCodedStringWithShadow(spriteBatch, FontAssets.ItemStack.Value, text, position, value, 0f, Vector2.Zero, vector2, num2);
			GlyphTagHandler.GlyphsScale = 1f;
		}
	}
}
