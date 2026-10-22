using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;
using Terraria.Audio;
using Terraria.UI;

namespace Terraria.GameContent.UI.Elements
{
	public class UIImageButton : UIElement
	{
		private Asset<Texture2D> _texture;

		protected float _visibilityActive = 1f;

		protected float _visibilityInactive = 0.4f;

		private Asset<Texture2D> _borderTexture;

		private Rectangle? _frame;

		private Rectangle? _borderFrame;

		public Color Color = Color.White;

		public Color BorderColor = Color.White;

		public UIImageButton(Asset<Texture2D> texture, Rectangle? frame = null)
		{
			_texture = texture;
			_frame = frame;
			Width.Set(frame.HasValue ? frame.Value.Width : _texture.Width(), 0f);
			Height.Set(frame.HasValue ? frame.Value.Height : _texture.Height(), 0f);
		}

		public void SetHoverImage(Asset<Texture2D> texture, Rectangle? frame = null)
		{
			_borderTexture = texture;
			_borderFrame = frame;
		}

		public void SetImage(Asset<Texture2D> texture, Rectangle? frame = null)
		{
			_texture = texture;
			Width.Set(_frame.HasValue ? _frame.Value.Width : _texture.Width(), 0f);
			Height.Set(_frame.HasValue ? _frame.Value.Height : _texture.Height(), 0f);
		}

		protected override void DrawSelf(SpriteBatch spriteBatch)
		{
			CalculatedStyle dimensions = GetDimensions();
			spriteBatch.Draw(_texture.Value, dimensions.Position(), _frame, Color * (base.IsMouseHovering ? _visibilityActive : _visibilityInactive));
			if (_borderTexture != null && base.IsMouseHovering)
			{
				spriteBatch.Draw(_borderTexture.Value, dimensions.Position(), _borderFrame, BorderColor);
			}
		}

		public override void MouseOver(UIMouseEvent evt)
		{
			base.MouseOver(evt);
			SoundEngine.PlaySound(12);
		}

		public override void MouseOut(UIMouseEvent evt)
		{
			base.MouseOut(evt);
		}

		public void SetVisibility(float whenActive, float whenInactive)
		{
			_visibilityActive = MathHelper.Clamp(whenActive, 0f, 1f);
			_visibilityInactive = MathHelper.Clamp(whenInactive, 0f, 1f);
		}
	}
}
