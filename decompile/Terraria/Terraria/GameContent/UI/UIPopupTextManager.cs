using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Graphics;

namespace Terraria.GameContent.UI
{
	public class UIPopupTextManager
	{
		public const int maxItemText = 20;

		public UIPopupText[] popupText = new UIPopupText[20];

		public int numActive;

		public void ResetText(UIPopupText text)
		{
			text.scale = 0f;
			text.rotation = 0f;
			text.alpha = 1f;
			text.alphaDir = -1;
			text.framesSinceSpawn = 0;
		}

		public int NewText(UIAdvancedPopupRequest request)
		{
			if (!Main.showItemText)
			{
				return -1;
			}
			if (Main.netMode == 2)
			{
				return -1;
			}
			int num = FindNextItemTextSlot();
			if (num >= 0)
			{
				string text = request.Text;
				Vector2 vector = FontAssets.MouseText.Value.MeasureString(text);
				UIPopupText uIPopupText = popupText[num];
				ResetText(uIPopupText);
				uIPopupText.active = true;
				uIPopupText.position = request.Position;
				if (request.Alignment >= UIPopupTextAlignment.BottomLeft)
				{
					uIPopupText.position.Y -= vector.Y;
				}
				else if (request.Alignment >= UIPopupTextAlignment.MidLeft)
				{
					uIPopupText.position.Y -= vector.Y / 2f;
				}
				switch ((int)request.Alignment % 3)
				{
				case 1:
					uIPopupText.position.X -= vector.X / 2f;
					break;
				case 2:
					uIPopupText.position.X -= vector.X;
					break;
				}
				uIPopupText.name = text;
				uIPopupText.velocity = request.Velocity;
				uIPopupText.lifeTime = request.DurationInFrames;
				uIPopupText.context = request.Context;
				uIPopupText.color = request.Color;
				uIPopupText.PrepareDisplayText();
			}
			return num;
		}

		private int FindNextItemTextSlot()
		{
			int num = -1;
			for (int i = 0; i < 20; i++)
			{
				if (!popupText[i].active)
				{
					num = i;
					break;
				}
			}
			if (num == -1)
			{
				double num2 = Main.bottomWorld;
				for (int j = 0; j < 20; j++)
				{
					if (num2 > (double)popupText[j].position.Y)
					{
						num = j;
						num2 = popupText[j].position.Y;
					}
				}
			}
			return num;
		}

		public void UpdateItemText()
		{
			int num = 0;
			for (int i = 0; i < 20; i++)
			{
				if (popupText[i].active)
				{
					num++;
					popupText[i].Update(i, this);
				}
			}
			numActive = num;
		}

		public void ClearAll()
		{
			for (int i = 0; i < 20; i++)
			{
				popupText[i] = new UIPopupText();
			}
			numActive = 0;
		}

		public void DrawItemTextPopups(float scaleTarget)
		{
			SpriteBatch spriteBatch = Main.spriteBatch;
			for (int i = 0; i < 20; i++)
			{
				UIPopupText uIPopupText = popupText[i];
				if (!uIPopupText.active)
				{
					continue;
				}
				string displayText = uIPopupText.displayText;
				Vector2 vector = FontAssets.MouseText.Value.MeasureString(displayText);
				Vector2 origin = new Vector2(vector.X * 0.5f, vector.Y * 0.5f);
				float num = scaleTarget;
				float num2 = uIPopupText.scale / num;
				int num3 = (int)(255f - 255f * num2);
				float num4 = (int)uIPopupText.color.R;
				_ = (float)(int)uIPopupText.color.G;
				_ = (float)(int)uIPopupText.color.B;
				float num5 = (int)uIPopupText.color.A;
				num4 *= num2 * uIPopupText.alpha * 0.3f;
				_ = uIPopupText.alpha;
				_ = uIPopupText.alpha;
				num5 *= num2 * uIPopupText.alpha;
				Color color = Color.Black;
				float num6 = 1f;
				Texture2D texture2D = null;
				if (uIPopupText.context == UIPopupTextContext.SpecialSeed)
				{
					color = Main.hslToRgb(Main.GlobalTimeWrappedHourly * 0.6f % 1f, 1f, 0.6f) * 0.6f;
					num *= 0.5f;
					num6 = 0.8f;
				}
				int num7 = 40;
				Utils.EaseOutCirc(Utils.Remap(uIPopupText.framesSinceSpawn, 0f, num7, 0f, 1f));
				float num8 = (float)num3 / 255f;
				for (int j = 0; j < 5; j++)
				{
					Color color2 = color;
					float num9 = 0f;
					float num10 = 0f;
					switch (j)
					{
					case 0:
						num9 -= num * 2f;
						break;
					case 1:
						num9 += num * 2f;
						break;
					case 2:
						num10 -= num * 2f;
						break;
					case 3:
						num10 += num * 2f;
						break;
					default:
						color2 = uIPopupText.color * num2 * uIPopupText.alpha * num6;
						break;
					}
					if (j < 4)
					{
						num5 = (float)(int)uIPopupText.color.A * num2 * uIPopupText.alpha;
						color2 = new Color(0, 0, 0, (int)num5);
					}
					if (color != Color.Black && j < 4)
					{
						num9 *= 1.3f + 1.3f * num8;
						num10 *= 1.3f + 1.3f * num8;
					}
					float num11 = uIPopupText.position.X + num9;
					float num12 = uIPopupText.position.Y + num10;
					if (color != Color.Black && j < 4)
					{
						Color color3 = color;
						color3.A = (byte)MathHelper.Lerp(60f, 127f, Utils.GetLerpValue(0f, 255f, num5, clamped: true));
						spriteBatch.DrawString(FontAssets.MouseText.Value, displayText, new Vector2(num11 + origin.X, num12 + origin.Y), Color.Lerp(color2, color3, 0.5f), uIPopupText.rotation, origin, uIPopupText.scale, SpriteEffects.None, 0f);
						spriteBatch.DrawString(FontAssets.MouseText.Value, displayText, new Vector2(num11 + origin.X, num12 + origin.Y), color3, uIPopupText.rotation, origin, uIPopupText.scale, SpriteEffects.None, 0f);
					}
					else
					{
						spriteBatch.DrawString(FontAssets.MouseText.Value, displayText, new Vector2(num11 + origin.X, num12 + origin.Y), color2, uIPopupText.rotation, origin, uIPopupText.scale, SpriteEffects.None, 0f);
					}
					if (texture2D != null)
					{
						float scale = (1.3f - num8) * uIPopupText.scale * 0.7f;
						Vector2 vector2 = new Vector2(num11 + origin.X, num12 + origin.Y);
						Color color4 = color * 0.6f;
						if (j == 4)
						{
							color4 = Color.White * 0.6f;
						}
						color4.A = (byte)((float)(int)color4.A * 0.5f);
						int num13 = 25;
						spriteBatch.Draw(texture2D, vector2 + new Vector2(origin.X * -0.5f - (float)num13 - texture2D.Size().X / 2f, 0f), null, color4 * uIPopupText.scale, 0f, texture2D.Size() / 2f, scale, SpriteEffects.None, 0f);
						spriteBatch.Draw(texture2D, vector2 + new Vector2(origin.X * 0.5f + (float)num13 + texture2D.Size().X / 2f, 0f), null, color4 * uIPopupText.scale, 0f, texture2D.Size() / 2f, scale, SpriteEffects.None, 0f);
					}
				}
			}
		}
	}
}
