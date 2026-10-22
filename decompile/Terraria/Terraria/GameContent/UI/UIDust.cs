using System;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.GameContent.Events;

namespace Terraria.GameContent.UI
{
	public class UIDust
	{
		public const int maxDust = 200;

		public Dust[] dust = new Dust[201];

		public void Clear()
		{
			for (int i = 0; i < 201; i++)
			{
				dust[i] = new Dust();
			}
		}

		public Dust NewDustPerfect(Vector2 Position, int Type, Vector2? Velocity = null, int Alpha = 0, Color newColor = default(Color), float Scale = 1f)
		{
			Dust dust = NewDust(Position, 0, 0, Type, 0f, 0f, Alpha, newColor, Scale);
			dust.position = Position;
			if (Velocity.HasValue)
			{
				dust.velocity = Velocity.Value;
			}
			return dust;
		}

		public Dust NewDustDirect(Vector2 Position, int Width, int Height, int Type, float SpeedX = 0f, float SpeedY = 0f, int Alpha = 0, Color newColor = default(Color), float Scale = 1f)
		{
			Dust dust = NewDust(Position, Width, Height, Type, SpeedX, SpeedY, Alpha, newColor, Scale);
			if (dust.velocity.HasNaNs())
			{
				dust.velocity = Vector2.Zero;
			}
			return dust;
		}

		public Dust NewDust(Vector2 Position, int Width, int Height, int Type, float SpeedX = 0f, float SpeedY = 0f, int Alpha = 0, Color newColor = default(Color), float Scale = 1f)
		{
			if (Type != 6 && Type != 267)
			{
				throw new Exception();
			}
			int num = 200;
			for (int i = 0; i < 200; i++)
			{
				Dust dust = this.dust[i];
				if (!dust.active)
				{
					int num2 = Width;
					int num3 = Height;
					if (num2 < 5)
					{
						num2 = 5;
					}
					if (num3 < 5)
					{
						num3 = 5;
					}
					num = i;
					dust.fadeIn = 0f;
					dust.active = true;
					dust.type = Type;
					dust.noGravity = false;
					dust.color = newColor;
					dust.alpha = Alpha;
					dust.position.X = Position.X + (float)Main.rand.Next(num2 - 4) + 4f;
					dust.position.Y = Position.Y + (float)Main.rand.Next(num3 - 4) + 4f;
					dust.velocity.X = (float)Main.rand.Next(-20, 21) * 0.1f + SpeedX;
					dust.velocity.Y = (float)Main.rand.Next(-20, 21) * 0.1f + SpeedY;
					dust.frame.X = 10 * Type;
					dust.frame.Y = 10 * Main.rand.Next(3);
					dust.shader = null;
					dust.customData = null;
					dust.noLightEmittance = false;
					dust.fullBright = false;
					int num4 = Type;
					while (num4 >= 100)
					{
						num4 -= 100;
						dust.frame.X -= 1000;
						dust.frame.Y += 30;
					}
					dust.frame.Width = 8;
					dust.frame.Height = 8;
					dust.rotation = 0f;
					dust.scale = 1f + (float)Main.rand.Next(-20, 21) * 0.01f;
					dust.scale *= Scale;
					dust.noLight = false;
					dust.firstFrame = true;
					if (dust.type == 6)
					{
						dust.velocity.Y = (float)Main.rand.Next(-10, 6) * 0.1f;
						dust.velocity.X *= 0.3f;
						dust.scale *= 0.7f;
					}
					break;
				}
			}
			return this.dust[num];
		}

		public Dust CloneDust(int dustIndex)
		{
			Dust rf = dust[dustIndex];
			return CloneDust(rf);
		}

		public Dust CloneDust(Dust rf)
		{
			Dust obj = NewDust(rf.position, 0, 0, rf.type);
			obj.position = rf.position;
			obj.velocity = rf.velocity;
			obj.fadeIn = rf.fadeIn;
			obj.noGravity = rf.noGravity;
			obj.scale = rf.scale;
			obj.rotation = rf.rotation;
			obj.noLight = rf.noLight;
			obj.active = rf.active;
			obj.type = rf.type;
			obj.color = rf.color;
			obj.alpha = rf.alpha;
			obj.frame = rf.frame;
			obj.shader = rf.shader;
			obj.customData = rf.customData;
			return obj;
		}

		public void QuickBox(Vector2 topLeft, Vector2 bottomRight, int divisions, Color color, Action<Dust> manipulator)
		{
			float num = divisions + 2;
			for (float num2 = 0f; num2 <= (float)(divisions + 2); num2 += 1f)
			{
				Dust obj = QuickDust(new Vector2(MathHelper.Lerp(topLeft.X, bottomRight.X, num2 / num), topLeft.Y), color);
				manipulator?.Invoke(obj);
				obj = QuickDust(new Vector2(MathHelper.Lerp(topLeft.X, bottomRight.X, num2 / num), bottomRight.Y), color);
				manipulator?.Invoke(obj);
				obj = QuickDust(new Vector2(topLeft.X, MathHelper.Lerp(topLeft.Y, bottomRight.Y, num2 / num)), color);
				manipulator?.Invoke(obj);
				obj = QuickDust(new Vector2(bottomRight.X, MathHelper.Lerp(topLeft.Y, bottomRight.Y, num2 / num)), color);
				manipulator?.Invoke(obj);
			}
		}

		public void QuickCircle(Vector2 center, float radius, int divisions, Color color, Action<Dust> manipulator)
		{
			float num = 1f / Math.Max(1f, divisions);
			for (float num2 = 0f; num2 < 1f; num2 += num)
			{
				float num3 = num2 * ((float)Math.PI * 2f);
				Vector2 pos = center;
				pos += new Vector2(radius, 0f).RotatedBy(num3, Vector2.Zero);
				Dust obj = QuickDust(pos, color);
				manipulator?.Invoke(obj);
			}
		}

		public Dust QuickDust(Vector2 pos, Color color)
		{
			Dust obj = NewDust(pos, 0, 0, 267);
			obj.position = pos;
			obj.velocity = Vector2.Zero;
			obj.fadeIn = 1f;
			obj.noLight = true;
			obj.noGravity = true;
			obj.color = color;
			return obj;
		}

		public Dust QuickDustSmall(Vector2 pos, Color color, bool floorPositionValues = false)
		{
			Dust dust = QuickDust(pos, color);
			dust.fadeIn = 0f;
			dust.scale = 0.35f;
			if (floorPositionValues)
			{
				dust.position = dust.position.Floor();
			}
			return dust;
		}

		public void QuickDustLine(Vector2 start, Vector2 end, float splits, Color color)
		{
			QuickDust(start, color).scale = 0.3f;
			QuickDust(end, color).scale = 0.3f;
			float num = 1f / splits;
			for (float num2 = 0f; num2 < 1f; num2 += num)
			{
				QuickDust(Vector2.Lerp(start, end, num2), color).scale = 0.3f;
			}
		}

		public void UpdateDust()
		{
			Sandstorm.ShowSandstormVisuals();
			for (int i = 0; i < 200; i++)
			{
				Dust dust = this.dust[i];
				if (!dust.active)
				{
					continue;
				}
				if (dust.scale > 10f)
				{
					dust.active = false;
				}
				dust.position += dust.velocity;
				if (dust.type == 6)
				{
					if (!dust.noGravity)
					{
						dust.velocity.Y += 0.05f;
					}
				}
				else if (dust.type == 267)
				{
					if (dust.velocity.X < 0f)
					{
						dust.rotation -= 1f;
					}
					else
					{
						dust.rotation += 1f;
					}
					dust.velocity.Y *= 0.98f;
					dust.velocity.X *= 0.98f;
					dust.scale += 0.02f;
				}
				dust.velocity.X *= 0.99f;
				dust.rotation += dust.velocity.X * 0.5f;
				if (dust.fadeIn > 0f && dust.fadeIn < 100f)
				{
					dust.scale += 0.03f;
					if (dust.scale > dust.fadeIn)
					{
						dust.fadeIn = 0f;
					}
				}
				else
				{
					dust.scale -= 0.01f;
				}
				if (dust.noGravity)
				{
					dust.velocity *= 0.92f;
					if (dust.fadeIn == 0f)
					{
						dust.scale -= 0.04f;
					}
				}
				float num = 0.1f;
				if (dust.scale < num)
				{
					dust.active = false;
				}
			}
		}

		internal void DrawDust()
		{
			SpriteBatch spriteBatch = Main.spriteBatch;
			for (int i = 0; i < 200; i++)
			{
				Dust dust = this.dust[i];
				if (!dust.active)
				{
					continue;
				}
				float visualScale = dust.GetVisualScale();
				Color newColor = Lighting.GetColor((int)((double)dust.position.X + 4.0) / 16, (int)((double)dust.position.Y + 4.0) / 16);
				if (dust.type == 6)
				{
					newColor = Color.White;
				}
				newColor = dust.GetAlpha(newColor);
				spriteBatch.Draw(TextureAssets.Dust.Value, dust.position, dust.frame, newColor, dust.GetVisualRotation(), new Vector2(4f, 4f), visualScale, SpriteEffects.None, 0f);
				if (dust.color.PackedValue != 0)
				{
					Color color = dust.GetColor(newColor);
					if (color.PackedValue != 0)
					{
						spriteBatch.Draw(TextureAssets.Dust.Value, dust.position, dust.frame, color, dust.GetVisualRotation(), new Vector2(4f, 4f), visualScale, SpriteEffects.None, 0f);
					}
				}
				if (newColor == Color.Black)
				{
					dust.active = false;
				}
			}
		}
	}
}
