using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.DataStructures;
using Terraria.UI;

namespace Terraria.Map
{
	public struct MapOverlayDrawContext
	{
		public struct DrawResult
		{
			public static readonly DrawResult Culled = new DrawResult(isMouseOver: false);

			public readonly bool IsMouseOver;

			public DrawResult(bool isMouseOver)
			{
				IsMouseOver = isMouseOver;
			}
		}

		private readonly Vector2 _mapPosition;

		private readonly Vector2 _mapOffset;

		private readonly Rectangle? _clippingRect;

		private readonly float _mapScale;

		private readonly float _drawScale;

		private readonly float _opacity;

		public MapOverlayDrawContext(Vector2 mapPosition, Vector2 mapOffset, Rectangle? clippingRect, float mapScale, float drawScale, float opacity)
		{
			_mapPosition = mapPosition;
			_mapOffset = mapOffset;
			_clippingRect = clippingRect;
			_mapScale = mapScale;
			_drawScale = drawScale;
			_opacity = opacity;
		}

		public DrawResult Draw(Texture2D texture, Vector2 position, Alignment alignment)
		{
			return Draw(texture, position, new SpriteFrame(1, 1), alignment);
		}

		public DrawResult Draw(Texture2D texture, Vector2 position, SpriteFrame frame, Alignment alignment)
		{
			if (_opacity == 0f)
			{
				return new DrawResult(isMouseOver: false);
			}
			position = (position - _mapPosition) * _mapScale + _mapOffset;
			if (_clippingRect.HasValue && !_clippingRect.Value.Contains(position.ToPoint()))
			{
				return DrawResult.Culled;
			}
			Rectangle sourceRectangle = frame.GetSourceRectangle(texture);
			Vector2 vector = sourceRectangle.Size() * alignment.OffsetMultiplier;
			Main.spriteBatch.Draw(texture, position, sourceRectangle, Color.White * _opacity, 0f, vector, _drawScale, SpriteEffects.None, 0f);
			position -= vector * _drawScale;
			return new DrawResult(new Rectangle((int)position.X, (int)position.Y, (int)((float)sourceRectangle.Width * _drawScale), (int)((float)sourceRectangle.Height * _drawScale)).Contains(Main.MouseScreen.ToPoint()));
		}

		public Rectangle GetUnclampedDrawRegion(Texture2D texture, Vector2 position, SpriteFrame frame, float scale, Alignment alignment)
		{
			position = (position - _mapPosition) * _mapScale + _mapOffset;
			Rectangle sourceRectangle = frame.GetSourceRectangle(texture);
			Vector2 vector = sourceRectangle.Size() * alignment.OffsetMultiplier;
			float num = _drawScale * scale;
			Vector2 vector2 = position - vector * num;
			return new Rectangle((int)vector2.X, (int)vector2.Y, (int)((float)sourceRectangle.Width * num), (int)((float)sourceRectangle.Height * num));
		}

		public Rectangle GetClampedDrawRegion(Texture2D texture, Vector2 position, SpriteFrame frame, float scale, Alignment alignment, int screenBorderRegion)
		{
			position = (position - _mapPosition) * _mapScale + _mapOffset;
			Rectangle sourceRectangle = frame.GetSourceRectangle(texture);
			Vector2 vector = sourceRectangle.Size() * alignment.OffsetMultiplier;
			float num = _drawScale * scale;
			Vector2 vector2 = position - vector * num;
			Rectangle result = new Rectangle((int)vector2.X, (int)vector2.Y, (int)((float)sourceRectangle.Width * num), (int)((float)sourceRectangle.Height * num));
			int num2 = Main.screenWidth - screenBorderRegion;
			int num3 = Main.screenHeight - screenBorderRegion;
			int num4 = (screenBorderRegion + num2) / 2;
			int num5 = (screenBorderRegion + num3) / 2;
			if (result.X < screenBorderRegion)
			{
				float num6 = result.X - num4;
				float num7 = result.Y - num5;
				int num8 = result.X - screenBorderRegion;
				int num9 = (int)((float)num8 / num6 * num7);
				result.X -= num8;
				result.Y -= num9;
			}
			else if (result.X + result.Width > num2)
			{
				float num10 = result.X - num4;
				float num11 = result.Y - num5;
				int num12 = result.X + result.Width - num2;
				int num13 = (int)((float)num12 / num10 * num11);
				result.X -= num12;
				result.Y -= num13;
			}
			if (result.Y < screenBorderRegion)
			{
				float num14 = result.X - num4;
				float num15 = result.Y - num5;
				int num16 = result.Y - screenBorderRegion;
				int num17 = (int)((float)num16 / num15 * num14);
				result.X -= num17;
				result.Y -= num16;
			}
			else if (result.Y + result.Height > num3)
			{
				float num18 = result.X - num4;
				float num19 = result.Y - num5;
				int num20 = result.Y + result.Height - num3;
				int num21 = (int)((float)num20 / num19 * num18);
				result.X -= num21;
				result.Y -= num20;
			}
			return result;
		}

		public DrawResult DrawClamped(Texture2D texture, Texture2D offscreenTexture, Vector2 position, Color color, SpriteFrame frame, float scaleIfNotSelected, float scaleIfSelected, float scaleIfOffscreen, Alignment alignment, int screenBorderRegion, out bool onScreen)
		{
			onScreen = true;
			if (_opacity == 0f)
			{
				return new DrawResult(isMouseOver: false);
			}
			position = (position - _mapPosition) * _mapScale + _mapOffset;
			Rectangle sourceRectangle = frame.GetSourceRectangle(texture);
			Vector2 vector = sourceRectangle.Size() * alignment.OffsetMultiplier;
			Vector2 vector2 = position;
			float num = _drawScale * scaleIfNotSelected;
			float num2 = _drawScale * scaleIfOffscreen;
			Vector2 vector3 = position - vector * num;
			Rectangle rectangle = new Rectangle((int)vector3.X, (int)vector3.Y, (int)((float)sourceRectangle.Width * num), (int)((float)sourceRectangle.Height * num));
			int num3 = Main.screenWidth - screenBorderRegion;
			int num4 = Main.screenHeight - screenBorderRegion;
			int num5 = (screenBorderRegion + num3) / 2;
			int num6 = (screenBorderRegion + num4) / 2;
			if (rectangle.X < screenBorderRegion)
			{
				float num7 = rectangle.X - num5;
				float num8 = rectangle.Y - num6;
				int num9 = rectangle.X - screenBorderRegion;
				int num10 = (int)((float)num9 / num7 * num8);
				vector2.X -= num9;
				rectangle.X -= num9;
				vector2.Y -= num10;
				rectangle.Y -= num10;
				onScreen = false;
			}
			else if (rectangle.X + rectangle.Width > num3)
			{
				onScreen = false;
				float num11 = rectangle.X - num5;
				float num12 = rectangle.Y - num6;
				int num13 = rectangle.X + rectangle.Width - num3;
				int num14 = (int)((float)num13 / num11 * num12);
				vector2.X -= num13;
				rectangle.X -= num13;
				vector2.Y -= num14;
				rectangle.Y -= num14;
			}
			if (rectangle.Y < screenBorderRegion)
			{
				onScreen = false;
				float num15 = rectangle.X - num5;
				float num16 = rectangle.Y - num6;
				int num17 = rectangle.Y - screenBorderRegion;
				int num18 = (int)((float)num17 / num16 * num15);
				vector2.X -= num18;
				rectangle.X -= num18;
				vector2.Y -= num17;
				rectangle.Y -= num17;
			}
			else if (rectangle.Y + rectangle.Height > num4)
			{
				onScreen = false;
				float num19 = rectangle.X - num5;
				float num20 = rectangle.Y - num6;
				int num21 = rectangle.Y + rectangle.Height - num4;
				int num22 = (int)((float)num21 / num20 * num19);
				vector2.X -= num22;
				rectangle.X -= num22;
				vector2.Y -= num21;
				rectangle.Y -= num21;
			}
			bool flag = rectangle.Contains(Main.MouseScreen.ToPoint());
			float num23 = num;
			if (!onScreen)
			{
				num23 = num2;
				if (flag)
				{
					num23 *= scaleIfSelected;
				}
			}
			else if (flag)
			{
				num23 = _drawScale * scaleIfSelected;
			}
			if (!onScreen && !flag)
			{
				int frameX = 2;
				int frameY = 1;
				_ = offscreenTexture.Width / 3;
				_ = offscreenTexture.Height / 3;
				Vector2 vector4 = position - vector2;
				float rotation = vector4.ToRotation();
				vector4.Normalize();
				Vector2 position2 = rectangle.Center.ToVector2();
				position2 += vector4 * ((float)(sourceRectangle.Height / 4) * num2);
				Rectangle value = offscreenTexture.Frame(3, 3, frameX, frameY);
				Vector2 origin = new Vector2(0f, (float)value.Height * 0.5f);
				Main.spriteBatch.Draw(offscreenTexture, position2, value, Color.White * _opacity, rotation, origin, num2, SpriteEffects.None, 0f);
			}
			Main.spriteBatch.Draw(texture, vector2, sourceRectangle, color * _opacity, 0f, vector, num23, SpriteEffects.None, 0f);
			return new DrawResult(flag);
		}

		public DrawResult Draw(Texture2D texture, Vector2 position, Color color, SpriteFrame frame, float scaleIfNotSelected, float scaleIfSelected, Alignment alignment)
		{
			if (_opacity == 0f)
			{
				return new DrawResult(isMouseOver: false);
			}
			position = (position - _mapPosition) * _mapScale + _mapOffset;
			if (_clippingRect.HasValue && !_clippingRect.Value.Contains(position.ToPoint()))
			{
				return DrawResult.Culled;
			}
			Rectangle sourceRectangle = frame.GetSourceRectangle(texture);
			Vector2 vector = sourceRectangle.Size() * alignment.OffsetMultiplier;
			Vector2 position2 = position;
			float num = _drawScale * scaleIfNotSelected;
			Vector2 vector2 = position - vector * num;
			bool num2 = new Rectangle((int)vector2.X, (int)vector2.Y, (int)((float)sourceRectangle.Width * num), (int)((float)sourceRectangle.Height * num)).Contains(Main.MouseScreen.ToPoint());
			float scale = num;
			if (num2)
			{
				scale = _drawScale * scaleIfSelected;
			}
			Main.spriteBatch.Draw(texture, position2, sourceRectangle, color * _opacity, 0f, vector, scale, SpriteEffects.None, 0f);
			return new DrawResult(num2);
		}
	}
}
