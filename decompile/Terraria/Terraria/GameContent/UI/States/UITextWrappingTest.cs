using System;
using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Microsoft.Xna.Framework.Input;
using Terraria.Audio;
using Terraria.GameContent.UI.Elements;
using Terraria.Localization;
using Terraria.UI;
using Terraria.UI.Chat;

namespace Terraria.GameContent.UI.States
{
	public class UITextWrappingTest : UIState
	{
		private enum Mode
		{
			UIText,
			SignsAndNPCChat,
			WordwrapStringLegacy,
			DrawColorCodedStringWithShadow,
			DrawColorCodedStringLegacy,
			MultilineChat
		}

		private class TestElement : UIElement
		{
			private readonly string text;

			private readonly float scale;

			private readonly Mode mode;

			public Action OnHeightUpdate;

			public TestElement(string text, float scale, Mode mode)
			{
				this.text = text;
				this.scale = scale;
				this.mode = mode;
			}

			protected override void DrawSelf(SpriteBatch spriteBatch)
			{
				Vector2 vector = GetDimensions().Position();
				float num = GetInnerDimensions().Width;
				if (num <= 0f)
				{
					num = 1000f;
				}
				switch (mode)
				{
				case Mode.SignsAndNPCChat:
				{
					int lineAmount;
					string[] array = Utils.WordwrapString(text, FontAssets.MouseText.Value, (int)(num / scale), 10, out lineAmount);
					float num3 = 30f * scale;
					MinHeight.Set((float)lineAmount * num3, 0f);
					OnHeightUpdate();
					for (int j = 0; j < lineAmount; j++)
					{
						Utils.DrawBorderStringFourWay(spriteBatch, FontAssets.MouseText.Value, array[j], vector.X, vector.Y + (float)j * num3, Color.White, Color.Black, Vector2.Zero, scale);
					}
					break;
				}
				case Mode.WordwrapStringLegacy:
				{
					int lineAmount2;
					string[] array2 = Utils.WordwrapStringLegacy(text, FontAssets.MouseText.Value, (int)(num / scale), 10, out lineAmount2);
					float num4 = 30f * scale;
					MinHeight.Set((float)lineAmount2 * num4, 0f);
					OnHeightUpdate();
					for (int k = 0; k < lineAmount2; k++)
					{
						Utils.DrawBorderStringFourWay(spriteBatch, FontAssets.MouseText.Value, array2[k], vector.X, vector.Y + (float)k * num4, Color.White, Color.Black, Vector2.Zero, scale);
					}
					break;
				}
				case Mode.MultilineChat:
				{
					List<List<TextSnippet>> list = Utils.WordwrapStringSmart(text, Color.White, FontAssets.MouseText.Value, (int)(num / scale), 10);
					float num2 = 30f * scale;
					MinHeight.Set((float)list.Count * num2, 0f);
					OnHeightUpdate();
					for (int i = 0; i < list.Count; i++)
					{
						ChatManager.DrawColorCodedStringWithShadow(spriteBatch, FontAssets.MouseText.Value, list[i].ToArray(), vector + new Vector2(0f, (float)i * num2), 0f, Vector2.Zero, new Vector2(scale), out var _);
					}
					break;
				}
				case Mode.DrawColorCodedStringWithShadow:
				{
					ChatManager.DrawColorCodedStringWithShadow(spriteBatch, FontAssets.MouseText.Value, text, vector, Color.White, 0f, Vector2.Zero, new Vector2(scale), num);
					Vector2 stringSize2 = ChatManager.GetStringSize(FontAssets.MouseText.Value, text, new Vector2(scale), num);
					MinHeight.Set(stringSize2.Y, 0f);
					OnHeightUpdate();
					break;
				}
				case Mode.DrawColorCodedStringLegacy:
				{
					ChatManager.DrawColorCodedStringShadow(spriteBatch, FontAssets.MouseText.Value, text, vector, Color.Black, 0f, Vector2.Zero, new Vector2(scale), num);
					ChatManager.DrawColorCodedString(spriteBatch, FontAssets.MouseText.Value, text, vector, Color.White, 0f, Vector2.Zero, new Vector2(scale), num);
					Vector2 stringSize = ChatManager.GetStringSize(FontAssets.MouseText.Value, text, new Vector2(scale), num);
					MinHeight.Set(stringSize.Y, 0f);
					OnHeightUpdate();
					break;
				}
				}
			}
		}

		private static readonly float TextPadding = 12f;

		private UIList list;

		private UIText modeText;

		private UIText scaleText;

		private UIText langText;

		private Mode mode;

		private int scale = 100;

		private string ScaleText => "Up/Down to change scale. Current: " + scale + "%";

		private string LangText => "Current Language: " + Language.ActiveCulture.CultureInfo.DisplayName;

		public UITextWrappingTest()
		{
			UIPanel uIPanel = new UIPanel
			{
				Top = StyleDimension.FromPixels(100f),
				Left = StyleDimension.FromPixelsAndPercent(-400f, 0.5f),
				Width = StyleDimension.FromPixels(300f),
				Height = StyleDimension.FromPixels(40f),
				BackgroundColor = new Color(43, 56, 101),
				BorderColor = Color.Transparent
			};
			modeText = new UIText(mode.ToString(), 0.8f)
			{
				TextOriginX = 0f,
				Width = StyleDimension.FromPercent(1f),
				Height = StyleDimension.FromPercent(1f)
			};
			uIPanel.Append(modeText);
			uIPanel.OnLeftClick += delegate
			{
				CycleMode(1);
			};
			uIPanel.OnRightClick += delegate
			{
				CycleMode(-1);
			};
			Append(uIPanel);
			scaleText = new UIText(ScaleText, 0.8f)
			{
				TextOriginX = 0f,
				Top = StyleDimension.FromPixels(150f),
				Left = StyleDimension.FromPixelsAndPercent(-400f, 0.5f),
				Width = StyleDimension.FromPixels(300f),
				Height = StyleDimension.FromPixels(40f)
			};
			Append(scaleText);
			langText = new UIText(LangText, 0.8f)
			{
				TextOriginX = 1f,
				HAlign = 1f,
				Top = StyleDimension.FromPixels(150f),
				Left = StyleDimension.FromPixelsAndPercent(400f, -0.5f),
				Width = StyleDimension.FromPixels(300f),
				Height = StyleDimension.FromPixels(40f)
			};
			Append(langText);
			list = new UIList
			{
				Top = StyleDimension.FromPixels(200f),
				Left = StyleDimension.FromPixelsAndPercent(-400f, 0.5f),
				Width = StyleDimension.FromPixels(300f),
				Height = StyleDimension.FromPixelsAndPercent(-200f, 1f),
				ListPadding = 5f,
				ManualSortMethod = delegate
				{
				}
			};
			list.SetPadding(0f);
			Append(list);
			UIScrollbar uIScrollbar = new UIScrollbar();
			uIScrollbar.SetView(100f, 1000f);
			uIScrollbar.Height.Set(-20f, 1f);
			uIScrollbar.HAlign = 1f;
			uIScrollbar.VAlign = 0.5f;
			uIScrollbar.Left.Set(6f, 0f);
			list.SetScrollbar(uIScrollbar);
			ResetList();
		}

		private void CycleMode(int offset)
		{
			int length = Enum.GetValues(typeof(Mode)).Length;
			mode = (Mode)((int)(mode + offset + length) % length);
			ResetList();
		}

		private void ResetList()
		{
			modeText.SetText(mode.ToString());
			list.Clear();
			list.Add(MakeElement("A test string in english.\nSecond line.\n\n^ Double line break\nLooooooooooooooonglinewithnospaces"));
			list.Add(MakeElement("Ends with newline\n"));
			list.Add(MakeElement("Non-breaking space: с\u00a0микротранзакциями\n"));
			list.Add(MakeElement("Thin\u2009Space\nHair\u200aSpace\nZero\u200bWidth\u200bSpace"));
			list.Add(NewSeparator());
			list.Add(MakeElement("せいなる スライムが がったいして できた 生き物。ごうまんで 力づよく きらめく けっしょうに おおわれている。つばさが 生える という うわさも ある。"));
			list.Add(MakeElement("정화된 슬라임들이 모두 통합되어, 눈부신 수정으로 장식된 거만하고 압도적인 힘이 되었습니다. 날개가 돋아난다는 소문도 있습니다. "));
			list.Add(MakeElement("Святые слизни объединяются в величественную всесокрушающую массу, украшенную превосходными кристаллами. Говорят, она даже может отрастить крылья."));
			list.Add(MakeElement("神圣史莱姆合并成了一种高傲的粉碎性力量，这种力量佩戴着闪耀的水晶。传说她会长出翅膀。"));
			list.Add(MakeElement("神聖史萊姆融合後，會點綴著閃耀的水晶，擁有傲視一切的粉碎性力量。傳說她會長出翅膀。"));
			list.Add(NewSeparator());
			list.Add(MakeElement("fullwidth terminators。bang！comma，fullstop。rcomma、colon：question？"));
			list.Add(MakeElement("Chinese separation〈聖聖聖聖〉《聖聖》「聖聖」『聖聖』【聖聖〔聖聖】〖聖聖〗!%),.:;?]}$100,25.24%"));
			list.Add(NewSeparator());
			list.Add(MakeElement(new LocalizedText("", "Keybind glyph support {InputTrigger_UseOrAttack} and {InputTrigger_InteractWithTile}").Value));
			list.Add(MakeElement("[c/FF0000:SomeRedText] [c/00FF00:SomeGreenText] [c/0000FF:SomeBlueText]"));
			list.Add(MakeElement("[c/FF0000:SomeRedText][c/00FF00:SomeGreenText][c/0000FF:SomeBlueText]"));
			list.Add(MakeElement("[c/0000FF:Long colored text, with escaped square brackets [\\] inside]"));
			list.Add(MakeElement("Items[i:1][i:2][i:3][i:4][i:5][i:6][i:7][i:100][i:1000]"));
			list.Add(MakeElement("ItemsOnSeparateLines\n[i:1]\n[i:2]\n[i:3]"));
			list.Add(MakeElement("Items and text [i:1] then stuff [i:2] and some more [i:3] etc"));
			list.Add(MakeElement("nospacebetweenitems[i:6]andtext[i:7]nospacebetweenitems[i:8]andtext[i:9]"));
			list.Add(MakeElement("[g:0][g:1][g:2][g:3][g:4][g:5][g:6][g:7][g:8][g:9][g:10][g:11][g:12][g:13][g:14][g:15][g:16][g:17][g:18][g:19][g:20][g:21][g:22][g:23][g:24][g:25]"));
			list.Add(MakeElement(Language.GetTextValue("Achievements.Completed", "[a:TRANSMUTE_ITEM]")));
			list.Add(MakeElement("[a:TO_INFINITY_AND_BEYOND][a:PURIFY_ENTIRE_WORLD][a:TO_INFINITY_AND_BEYOND][a:TRANSMUTE_ITEM][a:OBTAIN_HAMMER][a:BENCHED][a:HEAVY_METAL][a:GET_GOLDEN_DELIGHT][a:MINER_FOR_FIRE][a:HEAD_IN_THE_CLOUDS][a:GET_TERRASPARK_BOOTS]"));
		}

		private UIElement NewSeparator()
		{
			return new UIHorizontalSeparator
			{
				Width = StyleDimension.FromPixelsAndPercent(0f, 1f),
				Color = new Color(89, 116, 213, 255) * 0.9f
			};
		}

		private UIElement MakeElement(string value)
		{
			UIElement container = new UIPanel
			{
				Width = StyleDimension.FromPercent(1f),
				Height = StyleDimension.FromPixels(50 * scale),
				BackgroundColor = new Color(43, 56, 101),
				BorderColor = Color.Transparent
			};
			container.SetPadding(TextPadding);
			if (mode == Mode.UIText)
			{
				UIText text = new UIText(value, (float)scale / 100f)
				{
					TextOriginX = 0f,
					HAlign = 0f,
					VAlign = 0f,
					Width = StyleDimension.FromPercent(1f),
					Height = StyleDimension.FromPercent(1f),
					IsWrapped = true
				};
				text.OnInternalTextChange += delegate
				{
					container.Height = new StyleDimension(text.MinHeight.Pixels, 0f);
				};
				container.Append(text);
			}
			else
			{
				TestElement text2 = new TestElement(value, (float)scale / 100f, mode)
				{
					Width = StyleDimension.FromPercent(1f)
				};
				TestElement testElement = text2;
				testElement.OnHeightUpdate = (Action)Delegate.Combine(testElement.OnHeightUpdate, (Action)delegate
				{
					container.Height = new StyleDimension(text2.MinHeight.Pixels + container.PaddingTop + container.PaddingBottom, 0f);
				});
				container.Append(text2);
			}
			return container;
		}

		public override void Draw(SpriteBatch spriteBatch)
		{
			base.Draw(spriteBatch);
			CalculatedStyle dimensions = list.GetDimensions();
			int x = (int)(dimensions.X + TextPadding);
			int x2 = (int)(dimensions.X + dimensions.Width - TextPadding);
			spriteBatch.Draw(TextureAssets.MagicPixel.Value, new Rectangle(x, (int)dimensions.Y, 1, (int)dimensions.Height), Color.Green);
			spriteBatch.Draw(TextureAssets.MagicPixel.Value, new Rectangle(x2, (int)dimensions.Y, 1, (int)dimensions.Height), Color.Green);
		}

		public override void Update(GameTime gameTime)
		{
			if (Main.keyState.IsKeyDown(Keys.Escape))
			{
				SoundEngine.PlaySound(11);
				Main.menuMode = 0;
			}
			int num = 0;
			if (Main.keyState.IsKeyDown(Keys.Down) && Main.oldKeyState.IsKeyUp(Keys.Down))
			{
				num = -10;
			}
			if (Main.keyState.IsKeyDown(Keys.Up) && Main.oldKeyState.IsKeyUp(Keys.Up))
			{
				num = 10;
			}
			if (num != 0)
			{
				scale = Utils.Clamp(scale + num, 50, 150);
				ResetList();
				scaleText.SetText(ScaleText);
			}
			langText.SetText(LangText);
			if (Main.mouseLeft)
			{
				Point point = Main.MouseScreen.ToPoint();
				CalculatedStyle dimensions = list.GetDimensions();
				if ((float)point.X > dimensions.X && (float)point.Y > dimensions.Y)
				{
					list.Width = StyleDimension.FromPixels((float)point.X - dimensions.X);
				}
			}
			base.Update(gameTime);
		}
	}
}
