using System;
using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;

namespace Terraria.GameContent.Drawing
{
	public class BackgroundGradientDrawer
	{
		private Color _color;

		private GetBackgroundDrawWeightMethod _weightGetter;

		private BackgroundArrayGetterMethod _textureGetter;

		private int[] _textureIndexesToCheck;

		private static Asset<Texture2D> _sunflareGradientDitherTexture;

		public BackgroundGradientDrawer(Color gradientColor, GetBackgroundDrawWeightMethod weightGetter, BackgroundArrayGetterMethod textureGetter, params int[] textureIndexesToCheck)
		{
			_color = gradientColor;
			_weightGetter = weightGetter;
			_textureGetter = textureGetter;
			_textureIndexesToCheck = textureIndexesToCheck;
		}

		public void Draw()
		{
			if (!Main.BackgroundEnabled)
			{
				return;
			}
			float num = _weightGetter();
			if (!(num <= 0f) && ShouldDrawForTextures() && Main.ShouldDrawSurfaceBackground())
			{
				if (_sunflareGradientDitherTexture == null)
				{
					_sunflareGradientDitherTexture = Main.Assets.Request<Texture2D>("Images/Misc/Sunflare/colorgradientdither");
				}
				Main.spriteBatch.Draw(color: new Color(_color.ToVector3() * Main.ColorOfSurfaceBackgroundsBase.ToVector3()) * num, texture: _sunflareGradientDitherTexture.Value, destinationRectangle: GetGradientRect(), sourceRectangle: null, rotation: 0f, origin: Vector2.Zero, effects: SpriteEffects.None, layerDepth: 0f);
			}
		}

		private static Rectangle GetGradientRect()
		{
			int num = 400;
			int y = Math.Max(0, (int)((Main.worldSurface * 16.0 - (double)Main.screenPosition.Y - 2400.0) * 0.10000000149011612)) - num;
			return new Rectangle(0, y, Main.screenWidth, Main.screenHeight + num);
		}

		private bool ShouldDrawForTextures()
		{
			IEnumerable<int> enumerable = _textureGetter();
			int[] textureIndexesToCheck = _textureIndexesToCheck;
			foreach (int num in textureIndexesToCheck)
			{
				foreach (int item in enumerable)
				{
					if (num == item)
					{
						return true;
					}
				}
			}
			return false;
		}
	}
}
