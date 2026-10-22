using System;
using System.Collections.Generic;
using System.Linq;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Graphics;
using Terraria.Localization;
using Terraria.UI;
using Terraria.UI.Chat;

namespace Terraria.GameContent.UI.Elements
{
	public class UIText : UIElement
	{
		private object _text = "";

		private float _textScale = 1f;

		private Vector2 _textSize = Vector2.Zero;

		private bool _isLarge;

		private Color _color = Color.White;

		private Color _shadowColor = Color.Black;

		private bool _isWrapped;

		public bool DynamicallyScaleDownToWidth;

		private List<PositionedSnippet> _textLayout;

		private string _lastTextReference;

		public string Text => _text.ToString();

		public float TextOriginX { get; set; }

		public float TextOriginY { get; set; }

		public float WrappedTextBottomPadding { get; set; }

		public bool IsWrapped
		{
			get
			{
				return _isWrapped;
			}
			set
			{
				_isWrapped = value;
				InternalSetText(_text, _textScale, _isLarge);
			}
		}

		public Color TextColor
		{
			get
			{
				return _color;
			}
			set
			{
				_color = value;
			}
		}

		public Color ShadowColor
		{
			get
			{
				return _shadowColor;
			}
			set
			{
				_shadowColor = value;
			}
		}

		public event Action OnInternalTextChange;

		public UIText(string text, float textScale = 1f, bool large = false)
		{
			TextOriginX = 0.5f;
			TextOriginY = 0f;
			IsWrapped = false;
			WrappedTextBottomPadding = 20f;
			InternalSetText(text, textScale, large);
		}

		public UIText(LocalizedText text, float textScale = 1f, bool large = false)
		{
			TextOriginX = 0.5f;
			TextOriginY = 0f;
			IsWrapped = false;
			WrappedTextBottomPadding = 20f;
			InternalSetText(text, textScale, large);
		}

		public override void Recalculate()
		{
			InternalSetText(_text, _textScale, _isLarge);
			base.Recalculate();
		}

		public void SetText(string text)
		{
			InternalSetText(text, _textScale, _isLarge);
		}

		public void SetText(LocalizedText text)
		{
			InternalSetText(text, _textScale, _isLarge);
		}

		public void SetText(string text, float textScale, bool large)
		{
			InternalSetText(text, textScale, large);
		}

		public void SetText(LocalizedText text, float textScale, bool large)
		{
			InternalSetText(text, textScale, large);
		}

		protected override void DrawSelf(SpriteBatch spriteBatch)
		{
			base.DrawSelf(spriteBatch);
			VerifyTextState();
			CalculatedStyle innerDimensions = GetInnerDimensions();
			Vector2 position = innerDimensions.Position();
			if (_isLarge)
			{
				position.Y -= 10f * _textScale;
			}
			else
			{
				position.Y -= 2f * _textScale;
			}
			List<PositionedSnippet> textLayout = _textLayout;
			Vector2 scale = new Vector2(_textScale);
			Vector2 textSize = _textSize;
			if (DynamicallyScaleDownToWidth && textSize.X > innerDimensions.Width)
			{
				float num = innerDimensions.Width / textSize.X;
				textLayout = new List<PositionedSnippet>();
				for (int i = 0; i < textLayout.Count; i++)
				{
					textLayout[i].Scale(num);
				}
				scale *= num;
				textSize *= num;
			}
			position.X += (innerDimensions.Width - textSize.X) * TextOriginX;
			position.Y += (innerDimensions.Height - textSize.Y) * TextOriginY;
			Color shadowColor = _shadowColor * ((float)(int)_color.A / 255f);
			DynamicSpriteFont font = (_isLarge ? FontAssets.DeathText.Value : FontAssets.MouseText.Value);
			ChatManager.DrawColorCodedStringShadow(spriteBatch, font, _textLayout, position, shadowColor, 0f, Vector2.Zero, scale, 1.5f);
			ChatManager.DrawColorCodedString(spriteBatch, font, _textLayout, position, 0f, Vector2.Zero, scale, out var _);
		}

		private void VerifyTextState()
		{
			if ((object)_lastTextReference != Text)
			{
				InternalSetText(_text, _textScale, _isLarge);
			}
		}

		private void InternalSetText(object text, float textScale, bool large)
		{
			_text = text;
			_isLarge = large;
			_textScale = textScale;
			_lastTextReference = _text.ToString();
			List<TextSnippet> snippets = ChatManager.ParseMessage(_lastTextReference, _color);
			ChatManager.ConvertNormalSnippets(snippets);
			DynamicSpriteFont font = (large ? FontAssets.DeathText.Value : FontAssets.MouseText.Value);
			_textLayout = ChatManager.LayoutSnippets(font, snippets, new Vector2(_textScale), IsWrapped ? GetInnerDimensions().Width : (-1f)).ToList();
			_textSize = ChatManager.GetStringSize(_textLayout);
			if (IsWrapped)
			{
				_textSize.Y += WrappedTextBottomPadding * _textScale;
			}
			else
			{
				_textSize.Y = (large ? 32f : 16f) * _textScale;
			}
			MinWidth.Set((IsWrapped || DynamicallyScaleDownToWidth) ? 0f : (_textSize.X + PaddingLeft + PaddingRight), 0f);
			MinHeight.Set(_textSize.Y + PaddingTop + PaddingBottom, 0f);
			if (this.OnInternalTextChange != null)
			{
				this.OnInternalTextChange();
			}
		}
	}
}
