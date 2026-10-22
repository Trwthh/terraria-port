using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Microsoft.Xna.Framework.Input;
using Steamworks;
using Terraria.Social;
using Terraria.UI.Chat;

namespace Terraria.GameContent.UI.Chat
{
	public class GlyphTagHandler : ITagHandler
	{
		public class GlyphXboxTagHandler : ITagHandler
		{
			TextSnippet ITagHandler.Parse(string text, Color baseColor, string options)
			{
				if (!int.TryParse(text, out var result) || result >= 26)
				{
					return new TextSnippet(text);
				}
				return new GlyphSnippet(result)
				{
					ForcedStyle = 0,
					DeleteWhole = true,
					Text = "[gx:" + result + "]"
				};
			}
		}

		public class GlyphPSTagHandler : ITagHandler
		{
			TextSnippet ITagHandler.Parse(string text, Color baseColor, string options)
			{
				if (!int.TryParse(text, out var result) || result >= 26)
				{
					return new TextSnippet(text);
				}
				return new GlyphSnippet(result)
				{
					ForcedStyle = 1,
					DeleteWhole = true,
					Text = "[gp:" + result + "]"
				};
			}
		}

		public class GlyphSwitchTagHandler : ITagHandler
		{
			TextSnippet ITagHandler.Parse(string text, Color baseColor, string options)
			{
				if (!int.TryParse(text, out var result) || result >= 26)
				{
					return new TextSnippet(text);
				}
				return new GlyphSnippet(result)
				{
					ForcedStyle = 2,
					DeleteWhole = true,
					Text = "[gn:" + result + "]"
				};
			}
		}

		public class GlyphSnippet : TextSnippet
		{
			public int ForcedStyle = -1;

			private int _glyphIndex;

			public GlyphSnippet(int index)
			{
				_glyphIndex = index;
				Color = Color.White;
			}

			public GlyphSnippet(string keyName)
			{
				GlyphIndexes.TryGetValue(keyName, out _glyphIndex);
				Color = Color.White;
			}

			private static int GetAutoRow()
			{
				if (SocialAPI.Mode != SocialMode.Steam)
				{
					return 0;
				}
				SteamInput.RunFrame();
				int result = 0;
				InputHandle_t controllerForGamepadIndex = SteamInput.GetControllerForGamepadIndex(0);
				if (controllerForGamepadIndex.m_InputHandle != 0L)
				{
					switch (SteamInput.GetInputTypeForHandle(controllerForGamepadIndex))
					{
					case ESteamInputType.k_ESteamInputType_PS4Controller:
					case ESteamInputType.k_ESteamInputType_PS3Controller:
					case ESteamInputType.k_ESteamInputType_PS5Controller:
						result = 1;
						break;
					case ESteamInputType.k_ESteamInputType_SwitchJoyConPair:
					case ESteamInputType.k_ESteamInputType_SwitchJoyConSingle:
					case ESteamInputType.k_ESteamInputType_SwitchProController:
						result = 2;
						break;
					}
				}
				return result;
			}

			public override bool UniqueDraw(bool justCheckingString, out Vector2 size, SpriteBatch spriteBatch, Vector2 position = default(Vector2), Color color = default(Color), float scale = 1f)
			{
				scale *= GlyphsScale;
				if (!justCheckingString && color != Color.Black)
				{
					int num = ForcedStyle;
					if (num == -1)
					{
						num = GlyphStyle;
					}
					if (num == -1)
					{
						num = GetAutoRow();
					}
					int frameX = _glyphIndex;
					int glyphIndex = _glyphIndex;
					if (glyphIndex == 25)
					{
						frameX = ((Main.GlobalTimeWrappedHourly % 0.6f < 0.3f) ? 17 : 18);
					}
					Texture2D value = TextureAssets.TextGlyph[0].Value;
					spriteBatch.Draw(value, position + GlyphsOffset, value.Frame(25, 3, frameX, num), color, 0f, Vector2.Zero, scale, SpriteEffects.None, 0f);
				}
				size = new Vector2(26f) * scale;
				return true;
			}
		}

		private const int GlyphsPerLine = 25;

		private const int MaxGlyphs = 26;

		public static float GlyphsScale = 1f;

		public static Vector2 GlyphsOffset = Vector2.Zero;

		public const int DefaultGlyphStyle = -1;

		public static int GlyphStyle = -1;

		private static Dictionary<string, int> GlyphIndexes = new Dictionary<string, int>
		{
			{
				Buttons.A.ToString(),
				0
			},
			{
				Buttons.B.ToString(),
				1
			},
			{
				Buttons.Back.ToString(),
				4
			},
			{
				Buttons.DPadDown.ToString(),
				15
			},
			{
				Buttons.DPadLeft.ToString(),
				14
			},
			{
				Buttons.DPadRight.ToString(),
				13
			},
			{
				Buttons.DPadUp.ToString(),
				16
			},
			{
				Buttons.LeftShoulder.ToString(),
				6
			},
			{
				Buttons.LeftStick.ToString(),
				10
			},
			{
				Buttons.LeftThumbstickDown.ToString(),
				20
			},
			{
				Buttons.LeftThumbstickLeft.ToString(),
				17
			},
			{
				Buttons.LeftThumbstickRight.ToString(),
				18
			},
			{
				Buttons.LeftThumbstickUp.ToString(),
				19
			},
			{
				Buttons.LeftTrigger.ToString(),
				8
			},
			{
				Buttons.RightShoulder.ToString(),
				7
			},
			{
				Buttons.RightStick.ToString(),
				11
			},
			{
				Buttons.RightThumbstickDown.ToString(),
				24
			},
			{
				Buttons.RightThumbstickLeft.ToString(),
				21
			},
			{
				Buttons.RightThumbstickRight.ToString(),
				22
			},
			{
				Buttons.RightThumbstickUp.ToString(),
				23
			},
			{
				Buttons.RightTrigger.ToString(),
				9
			},
			{
				Buttons.Start.ToString(),
				5
			},
			{
				Buttons.X.ToString(),
				2
			},
			{
				Buttons.Y.ToString(),
				3
			},
			{ "RightStickAxis", 12 },
			{ "LR", 25 }
		};

		public static TextSnippet GetGlyph(string keyName)
		{
			return new GlyphSnippet(keyName);
		}

		TextSnippet ITagHandler.Parse(string text, Color baseColor, string options)
		{
			if (!int.TryParse(text, out var result) || result >= 26)
			{
				return new TextSnippet(text);
			}
			return new GlyphSnippet(result)
			{
				DeleteWhole = true,
				Text = "[g:" + result + "]"
			};
		}

		public static string GenerateTag(int index)
		{
			string text = "[g";
			return text + ":" + index + "]";
		}

		public static string GenerateTag(string keyname)
		{
			if (GlyphIndexes.TryGetValue(keyname, out var value))
			{
				return GenerateTag(value);
			}
			return keyname;
		}
	}
}
