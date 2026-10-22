using System;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Localization.IME;
using ReLogic.OS;

namespace Terraria.GameContent.UI.Elements
{
	internal class UITextBox : UITextPanel<string>
	{
		private int _cursor;

		private int _frameCount;

		private int _maxLength = 20;

		public bool ShowInputTicker = true;

		public bool HideSelf;

		protected override Vector2 TextDrawPosition
		{
			get
			{
				Vector2 textDrawPosition = base.TextDrawPosition;
				if (ShowInputTicker)
				{
					string compositionString = Platform.Get<IImeService>().CompositionString;
					if (!string.IsNullOrEmpty(compositionString))
					{
						textDrawPosition.X -= base.Font.MeasureString(compositionString).X * base.TextScale * TextHAlign;
					}
				}
				return textDrawPosition;
			}
		}

		public UITextBox(string text, float textScale = 1f, bool large = false)
			: base(text, textScale, large)
		{
		}

		public void Write(string text)
		{
			SetText(base.Text.Insert(_cursor, text));
			_cursor += text.Length;
		}

		public override void SetText(string text, float textScale, bool large)
		{
			text = Utils.TrimUserString(text ?? "", _maxLength);
			base.SetText(text, textScale, large);
			_cursor = Math.Min(base.Text.Length, _cursor);
		}

		public void SetTextMaxLength(int maxLength)
		{
			_maxLength = maxLength;
		}

		public void Backspace()
		{
			if (_cursor != 0)
			{
				SetText(Utils.TrimLastCharacter(base.Text));
			}
		}

		public void CursorLeft()
		{
			if (_cursor != 0)
			{
				_cursor--;
			}
		}

		public void CursorRight()
		{
			if (_cursor < base.Text.Length)
			{
				_cursor++;
			}
		}

		protected override void DrawSelf(SpriteBatch spriteBatch)
		{
			if (HideSelf)
			{
				return;
			}
			_cursor = base.Text.Length;
			base.DrawSelf(spriteBatch);
			if (!ShowInputTicker)
			{
				return;
			}
			Vector2 textDrawPosition = TextDrawPosition;
			string compositionString = Platform.Get<IImeService>().CompositionString;
			if (!string.IsNullOrEmpty(compositionString))
			{
				textDrawPosition.X += base.Font.MeasureString(compositionString).X * base.TextScale;
				DrawText(spriteBatch, compositionString, TextDrawPosition + new Vector2(base.TextSize.X, 0f), Main.imeCompositionStringColor);
			}
			_frameCount++;
			if ((_frameCount %= 40) <= 20)
			{
				textDrawPosition.X += base.Font.MeasureString(base.Text.Substring(0, _cursor)).X * base.TextScale;
				textDrawPosition.X += 6f - (base.IsLarge ? 8f : 4f) * base.TextScale;
				if (base.IsLarge)
				{
					textDrawPosition.Y += 2f * base.TextScale;
				}
				DrawText(spriteBatch, "|", textDrawPosition, base.TextColor);
			}
		}
	}
}
