using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using System.Text.RegularExpressions;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Graphics;
using Terraria.Chat;
using Terraria.GameContent.UI.Chat;
using Terraria.Graphics;
using Terraria.Localization;
using Terraria.Testing.ChatCommands;

namespace Terraria.UI.Chat
{
	public static class ChatManager
	{
		public static class Regexes
		{
			public static readonly Regex Format = new Regex("(?<!\\\\)\\[(?<tag>[a-zA-Z]{1,10})(\\/(?<options>[^:]+))?:(?<text>.+?)(?<!\\\\)\\]", RegexOptions.Compiled | RegexOptions.Singleline);
		}

		public static readonly DebugCommandProcessor DebugCommands = new DebugCommandProcessor();

		public static readonly ChatCommandProcessor Commands = new ChatCommandProcessor();

		private static ConcurrentDictionary<string, ITagHandler> _handlers = new ConcurrentDictionary<string, ITagHandler>();

		public static readonly Vector2[] ShadowDirections = new Vector2[4]
		{
			-Vector2.UnitX,
			Vector2.UnitX,
			-Vector2.UnitY,
			Vector2.UnitY
		};

		public static Color WaveColor(Color color)
		{
			float num = (float)(int)Main.mouseTextColor / 255f;
			color = Color.Lerp(color, Color.Black, 1f - num);
			color.A = Main.mouseTextColor;
			return color;
		}

		public static void ConvertNormalSnippets(List<TextSnippet> snippets)
		{
			for (int i = 0; i < snippets.Count; i++)
			{
				TextSnippet textSnippet = snippets[i];
				if (textSnippet.GetType() == typeof(TextSnippet))
				{
					snippets[i] = new PlainTagHandler.PlainSnippet(textSnippet.Text, textSnippet.Color);
				}
			}
		}

		public static void Register<T>(params string[] names) where T : ITagHandler, new()
		{
			T val = new T();
			for (int i = 0; i < names.Length; i++)
			{
				_handlers[names[i].ToLower()] = val;
			}
		}

		private static ITagHandler GetHandler(string tagName)
		{
			string key = tagName.ToLower();
			if (_handlers.ContainsKey(key))
			{
				return _handlers[key];
			}
			return null;
		}

		public static bool MayNeedParsing(string text)
		{
			if (text.IndexOf('\r') < 0)
			{
				return Regexes.Format.IsMatch(text);
			}
			return true;
		}

		public static List<TextSnippet> ParseMessage(string text, Color baseColor)
		{
			text = text.Replace("\r", "");
			MatchCollection matchCollection = Regexes.Format.Matches(text);
			List<TextSnippet> list = new List<TextSnippet>();
			int num = 0;
			foreach (Match item in matchCollection)
			{
				if (item.Index > num)
				{
					list.Add(new TextSnippet(text.Substring(num, item.Index - num), baseColor));
				}
				num = item.Index + item.Length;
				string value = item.Groups["tag"].Value;
				string text2 = item.Groups["text"].Value.Replace("\\]", "]");
				string value2 = item.Groups["options"].Value;
				ITagHandler handler = GetHandler(value);
				if (handler != null)
				{
					list.Add(handler.Parse(text2, baseColor, value2));
					list[list.Count - 1].TextOriginal = item.ToString();
				}
				else
				{
					list.Add(new TextSnippet(text2, baseColor));
				}
			}
			if (text.Length > num)
			{
				list.Add(new TextSnippet(text.Substring(num, text.Length - num), baseColor));
			}
			return list;
		}

		public static bool AddChatText(DynamicSpriteFont font, string text, Vector2 baseScale)
		{
			int num = 470;
			num = Main.screenWidth - 330;
			if (GetStringSize(font, Main.chatText + text, baseScale).X > (float)num)
			{
				return false;
			}
			Main.chatText += text;
			return true;
		}

		public static IEnumerable<PositionedSnippet> LayoutSnippets(DynamicSpriteFont font, IEnumerable<TextSnippet> snippets, Vector2 scale, float maxWidth = -1f)
		{
			int line = 0;
			Vector2 pos = Vector2.Zero;
			float uniqueDrawScale = Math.Min(scale.X, scale.Y);
			int i = 0;
			foreach (TextSnippet snippet in snippets)
			{
				if (snippet.UniqueDraw(justCheckingSize: true, out var size, null, default(Vector2), default(Color), uniqueDrawScale))
				{
					if (maxWidth >= 0f && pos.X + size.X > maxWidth)
					{
						pos.X = 0f;
						pos.Y += (float)font.LineSpacing * scale.Y;
						line++;
					}
					yield return new PositionedSnippet(snippet, i, line, pos, size);
					pos.X += size.X;
				}
				else
				{
					string text = font.CreateWrappedText(snippet.Text, scale.X, maxWidth, pos.X, Language.ActiveCulture.CultureInfo);
					int num = 0;
					while (true)
					{
						int sep = text.IndexOf('\n', num);
						int num2 = ((sep < 0) ? text.Length : sep) - num;
						if (num2 > 0)
						{
							string text2 = text.Substring(num, num2);
							size = font.MeasureString(text2) * scale;
							yield return new PositionedSnippet(snippet.CopyMorph(text2), i, line, pos, size);
							pos.X += size.X;
						}
						if (sep < 0)
						{
							break;
						}
						pos.X = 0f;
						pos.Y += (float)font.LineSpacing * scale.Y;
						line++;
						num = sep + 1;
					}
				}
				i++;
				size = default(Vector2);
			}
		}

		public static Vector2 GetStringSize(DynamicSpriteFont font, string text, Vector2 baseScale, float maxWidth = -1f)
		{
			return GetStringSize(font, ParseMessage(text, Color.White), baseScale, maxWidth);
		}

		public static Vector2 GetStringSize(DynamicSpriteFont font, IEnumerable<TextSnippet> snippets, Vector2 scale, float maxWidth = -1f)
		{
			return GetStringSize(LayoutSnippets(font, snippets, scale, maxWidth));
		}

		public static Vector2 GetStringSize(IEnumerable<PositionedSnippet> snippets)
		{
			Vector2 zero = Vector2.Zero;
			foreach (PositionedSnippet snippet in snippets)
			{
				zero.X = Math.Max(zero.X, snippet.Position.X + snippet.Size.X);
				zero.Y = Math.Max(zero.Y, snippet.Position.Y + snippet.Size.Y);
			}
			return zero;
		}

		public static void DrawColorCodedStringShadow(SpriteBatch spriteBatch, DynamicSpriteFont font, List<PositionedSnippet> snippets, Vector2 position, Color shadowColor, float rotation, Vector2 origin, Vector2 scale, float spread = 2f)
		{
			for (int i = 0; i < ShadowDirections.Length; i++)
			{
				DrawColorCodedString(spriteBatch, font, snippets, position + ShadowDirections[i] * spread, rotation, origin, scale, out var _, shadowColor);
			}
		}

		public static void DrawColorCodedString(SpriteBatch spriteBatch, DynamicSpriteFont font, IEnumerable<TextSnippet> snippets, Vector2 position, Color baseColor, float rotation, Vector2 origin, Vector2 scale, out int hoveredSnippet, float maxWidth = -1f, bool ignoreColors = false)
		{
			DrawColorCodedString(spriteBatch, font, LayoutSnippets(font, snippets, scale, maxWidth), position, rotation, origin, scale, out hoveredSnippet, ignoreColors ? new Color?(baseColor) : ((Color?)null));
		}

		public static void DrawColorCodedString(SpriteBatch spriteBatch, DynamicSpriteFont font, IEnumerable<TextSnippet> snippets, Vector2 position, float rotation, Vector2 origin, Vector2 scale, out int hoveredSnippet, float maxWidth = -1f)
		{
			DrawColorCodedString(spriteBatch, font, LayoutSnippets(font, snippets, scale, maxWidth), position, rotation, origin, scale, out hoveredSnippet);
		}

		public static void DrawColorCodedString(SpriteBatch spriteBatch, DynamicSpriteFont font, IEnumerable<PositionedSnippet> snippets, Vector2 position, float rotation, Vector2 origin, Vector2 scale, out int hoveredSnippet, Color? colorOverride = null)
		{
			hoveredSnippet = -1;
			Vector2 vec = new Vector2(Main.mouseX, Main.mouseY);
			float scale2 = Math.Min(scale.X, scale.Y);
			foreach (PositionedSnippet snippet2 in snippets)
			{
				Vector2 vector = position + snippet2.Position;
				TextSnippet snippet = snippet2.Snippet;
				Color color = (colorOverride.HasValue ? colorOverride.Value : snippet.GetVisibleColor());
				if (!snippet.UniqueDraw(justCheckingSize: false, out var _, spriteBatch, vector, color, scale2))
				{
					spriteBatch.DrawString(font, snippet.Text, vector, color, rotation, origin, scale, SpriteEffects.None, 0f);
				}
				if (snippet2.Snippet.CheckForHover && vec.Between(vector, vector + snippet2.Size))
				{
					hoveredSnippet = snippet2.OrigIndex;
				}
			}
		}

		public static void DrawColorCodedStringWithShadow(SpriteBatch spriteBatch, DynamicSpriteFont font, TextSnippet[] snippets, Vector2 position, float rotation, Vector2 origin, Vector2 baseScale, out int hoveredSnippet, float maxWidth = -1f, float spread = 2f)
		{
			List<PositionedSnippet> snippets2 = LayoutSnippets(font, snippets, baseScale, maxWidth).ToList();
			DrawColorCodedStringShadow(spriteBatch, font, snippets2, position, Color.Black, rotation, origin, baseScale, spread);
			DrawColorCodedString(spriteBatch, font, snippets2, position, rotation, origin, baseScale, out hoveredSnippet);
		}

		public static void DrawColorCodedStringWithShadow(SpriteBatch spriteBatch, DynamicSpriteFont font, TextSnippet[] snippets, Vector2 position, Color color, float rotation, Vector2 origin, Vector2 baseScale, out int hoveredSnippet, float maxWidth = -1f, float spread = 2f)
		{
			List<PositionedSnippet> snippets2 = LayoutSnippets(font, snippets, baseScale, maxWidth).ToList();
			DrawColorCodedStringShadow(spriteBatch, font, snippets2, position, color.MultiplyRGBA(Color.Black), rotation, origin, baseScale, spread);
			DrawColorCodedString(spriteBatch, font, snippets2, position, rotation, origin, baseScale, out hoveredSnippet, color);
		}

		public static void DrawColorCodedStringShadow(SpriteBatch spriteBatch, DynamicSpriteFont font, string text, Vector2 position, Color baseColor, float rotation, Vector2 origin, Vector2 baseScale, float maxWidth = -1f, float spread = 2f)
		{
			for (int i = 0; i < ShadowDirections.Length; i++)
			{
				DrawColorCodedString(spriteBatch, font, text, position + ShadowDirections[i] * spread, baseColor, rotation, origin, baseScale, maxWidth, ignoreColors: true);
			}
		}

		public static Vector2 DrawColorCodedString(SpriteBatch spriteBatch, DynamicSpriteFont font, string text, Vector2 position, Color baseColor, float rotation, Vector2 origin, Vector2 baseScale, float maxWidth = -1f, bool ignoreColors = false)
		{
			Vector2 vector = position;
			Vector2 result = vector;
			string[] array = text.Split('\n');
			float x = font.MeasureString(" ").X;
			Color color = baseColor;
			float num = 1f;
			float num2 = 0f;
			string[] array2 = array;
			for (int i = 0; i < array2.Length; i++)
			{
				string[] array3 = array2[i].Split(':');
				foreach (string text2 in array3)
				{
					if (text2.StartsWith("sss"))
					{
						if (text2.StartsWith("sss1"))
						{
							if (!ignoreColors)
							{
								color = Color.Red;
							}
						}
						else if (text2.StartsWith("sss2"))
						{
							if (!ignoreColors)
							{
								color = Color.Blue;
							}
						}
						else if (text2.StartsWith("sssr") && !ignoreColors)
						{
							color = Color.White;
						}
						continue;
					}
					string[] array4 = text2.Split(' ');
					for (int k = 0; k < array4.Length; k++)
					{
						if (k != 0)
						{
							vector.X += x * baseScale.X * num;
						}
						if (maxWidth > 0f)
						{
							float num3 = font.MeasureString(array4[k]).X * baseScale.X * num;
							if (vector.X - position.X + num3 > maxWidth)
							{
								vector.X = position.X;
								vector.Y += (float)font.LineSpacing * num2 * baseScale.Y;
								result.Y = Math.Max(result.Y, vector.Y);
								num2 = 0f;
							}
						}
						if (num2 < num)
						{
							num2 = num;
						}
						spriteBatch.DrawString(font, array4[k], vector, color, rotation, origin, baseScale * num, SpriteEffects.None, 0f);
						vector.X += font.MeasureString(array4[k]).X * baseScale.X * num;
						result.X = Math.Max(result.X, vector.X);
					}
				}
				vector.X = position.X;
				vector.Y += (float)font.LineSpacing * num2 * baseScale.Y;
				result.Y = Math.Max(result.Y, vector.Y);
				num2 = 0f;
			}
			return result;
		}

		public static void DrawColorCodedStringWithShadow(SpriteBatch spriteBatch, DynamicSpriteFont font, string text, Vector2 position, Color baseColor, float rotation, Vector2 origin, Vector2 scale, float maxWidth = -1f, float spread = 2f)
		{
			Color color = baseColor.MultiplyRGBA(Color.Black);
			if (maxWidth < 0f && !MayNeedParsing(text))
			{
				Vector2[] shadowDirections = ShadowDirections;
				foreach (Vector2 vector in shadowDirections)
				{
					spriteBatch.DrawString(font, text, position + vector * spread, color, rotation, origin, scale, SpriteEffects.None, 0f);
				}
				spriteBatch.DrawString(font, text, position, baseColor, rotation, origin, scale, SpriteEffects.None, 0f);
			}
			else
			{
				List<TextSnippet> snippets = ParseMessage(text, baseColor);
				ConvertNormalSnippets(snippets);
				List<PositionedSnippet> snippets2 = LayoutSnippets(font, snippets, scale, maxWidth).ToList();
				DrawColorCodedStringShadow(spriteBatch, font, snippets2, position, color, rotation, origin, scale, spread);
				DrawColorCodedString(spriteBatch, font, snippets2, position, rotation, origin, scale, out var _);
			}
		}

		public static void DrawStringWithShadowFast(TileBatch tileBatch, DynamicSpriteFont font, string text, Vector2 position, Color baseColor, Vector2 origin, float scale, uint layer = 0u, float spread = 2f)
		{
			Color shadowColor = baseColor.MultiplyRGBA(Color.Black);
			font.DrawCustomFast(delegate(Texture2D tex, Vector2 pos, Rectangle rect, Vector2 _)
			{
				tileBatch.SetLayer(layer, 0);
				Vector2[] shadowDirections = ShadowDirections;
				foreach (Vector2 vector in shadowDirections)
				{
					tileBatch.Draw(tex, pos + vector * spread, rect, shadowColor, Vector2.Zero, scale);
				}
				tileBatch.SetLayer(layer + 1, 0);
				tileBatch.Draw(tex, pos, rect, baseColor, Vector2.Zero, scale);
			}, text, position - origin * scale, new Vector2(scale));
		}
	}
}
