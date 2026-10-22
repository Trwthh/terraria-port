using System;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Graphics;
using Terraria.GameContent;
using Terraria.Localization;

namespace Terraria
{
	public class PopupText
	{
		public const int maxItemText = 20;

		public static PopupText[] popupText = new PopupText[20];

		public Vector2 position;

		public Vector2 velocity;

		public float alpha;

		public int alphaDir = 1;

		public string name;

		public string displayText;

		public long stack;

		public float scale = 1f;

		public float rotation;

		public Color color;

		public bool active;

		public int lifeTime;

		public int framesSinceSpawn;

		public static int activeTime = 60;

		public static int numActive;

		public bool NoStack;

		public bool coinText;

		public long coinValue;

		public static int sonarText = -1;

		public bool expert;

		public bool master;

		public bool sonar;

		public PopupTextContext context;

		public int npcNetID;

		public bool freeAdvanced;

		public Vector2[] charOffsets;

		public Color[] charColors;

		public PopupEffectStyle effectStyle;

		public int effectIntensity;

		public bool AnyEffect => effectStyle != PopupEffectStyle.None;

		public bool notActuallyAnItem
		{
			get
			{
				if (npcNetID == 0)
				{
					return freeAdvanced;
				}
				return true;
			}
		}

		public static float TargetScale => Main.UIScale / Main.GameViewMatrix.RenderZoom.X;

		public static void ClearSonarText()
		{
			if (sonarText >= 0 && popupText[sonarText].sonar)
			{
				popupText[sonarText].active = false;
				sonarText = -1;
			}
		}

		public static void ResetText(PopupText text)
		{
			text.NoStack = false;
			text.coinText = false;
			text.coinValue = 0L;
			text.sonar = false;
			text.npcNetID = 0;
			text.expert = false;
			text.master = false;
			text.freeAdvanced = false;
			text.scale = 0f;
			text.rotation = 0f;
			text.alpha = 1f;
			text.alphaDir = -1;
			text.framesSinceSpawn = 0;
			text.effectStyle = PopupEffectStyle.None;
			text.effectIntensity = 0;
		}

		public static int NewText(AdvancedPopupRequest request, Vector2 position)
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
				PopupText obj = popupText[num];
				ResetText(obj);
				obj.active = true;
				obj.position = position - vector / 2f;
				obj.name = text;
				obj.stack = 1L;
				obj.velocity = request.Velocity;
				obj.lifeTime = request.DurationInFrames;
				obj.context = PopupTextContext.Advanced;
				obj.freeAdvanced = true;
				obj.color = request.Color;
				obj.PrepareDisplayText();
			}
			return num;
		}

		public static int NewText(PopupTextContext context, int npcNetID, Vector2 position, bool stay5TimesLonger)
		{
			if (!Main.showItemText)
			{
				return -1;
			}
			if (npcNetID == 0)
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
				NPC nPC = new NPC();
				nPC.SetDefaults(npcNetID);
				string typeName = nPC.TypeName;
				Vector2 vector = FontAssets.MouseText.Value.MeasureString(typeName);
				PopupText popupText = PopupText.popupText[num];
				ResetText(popupText);
				popupText.active = true;
				popupText.position = position - vector / 2f;
				popupText.name = typeName;
				popupText.stack = 1L;
				popupText.velocity.Y = -7f;
				popupText.lifeTime = 60;
				popupText.context = context;
				if (stay5TimesLonger)
				{
					popupText.lifeTime *= 5;
				}
				popupText.npcNetID = npcNetID;
				popupText.color = Color.White;
				if (context == PopupTextContext.SonarAlert)
				{
					popupText.color = Color.Lerp(Color.White, Color.Crimson, 0.5f);
				}
				popupText.PrepareDisplayText();
			}
			return num;
		}

		public static int NewText(PopupTextContext context, Item newItem, Vector2 position, int stack, bool noStack = false, bool longText = false)
		{
			if (!Main.showItemText)
			{
				return -1;
			}
			if (newItem.Name == null)
			{
				return -1;
			}
			if (Main.netMode == 2)
			{
				return -1;
			}
			bool flag = newItem.type >= 71 && newItem.type <= 74;
			for (int i = 0; i < 20; i++)
			{
				PopupText popupText = PopupText.popupText[i];
				if (!popupText.active || popupText.notActuallyAnItem || (!(popupText.name == newItem.AffixName()) && (!flag || !popupText.coinText)) || popupText.NoStack || noStack)
				{
					continue;
				}
				string text = newItem.Name + " (" + (popupText.stack + stack) + ")";
				string text2 = newItem.Name;
				if (popupText.stack > 1)
				{
					text2 = text2 + " (" + popupText.stack + ")";
				}
				Vector2 vector = FontAssets.MouseText.Value.MeasureString(text2);
				vector = FontAssets.MouseText.Value.MeasureString(text);
				if (popupText.lifeTime < 0)
				{
					popupText.scale = 1f;
				}
				if (popupText.lifeTime < 60)
				{
					popupText.lifeTime = 60;
				}
				if (flag && popupText.coinText)
				{
					long num = 0L;
					if (newItem.type == 71)
					{
						num += stack;
					}
					else if (newItem.type == 72)
					{
						num += 100 * stack;
					}
					else if (newItem.type == 73)
					{
						num += 10000 * stack;
					}
					else if (newItem.type == 74)
					{
						num += 1000000 * stack;
					}
					popupText.AddToCoinValue(num);
					text = ValueToName(popupText.coinValue);
					vector = FontAssets.MouseText.Value.MeasureString(text);
					popupText.name = text;
					if (popupText.coinValue >= 1000000)
					{
						if (popupText.lifeTime < 300)
						{
							popupText.lifeTime = 300;
						}
						popupText.color = new Color(220, 220, 198);
					}
					else if (popupText.coinValue >= 10000)
					{
						if (popupText.lifeTime < 240)
						{
							popupText.lifeTime = 240;
						}
						popupText.color = new Color(224, 201, 92);
					}
					else if (popupText.coinValue >= 100)
					{
						if (popupText.lifeTime < 180)
						{
							popupText.lifeTime = 180;
						}
						popupText.color = new Color(181, 192, 193);
					}
					else if (popupText.coinValue >= 1)
					{
						if (popupText.lifeTime < 120)
						{
							popupText.lifeTime = 120;
						}
						popupText.color = new Color(246, 138, 96);
					}
				}
				popupText.stack += stack;
				popupText.scale = 0f;
				popupText.rotation = 0f;
				popupText.position.X = position.X + (float)newItem.width * 0.5f - vector.X * 0.5f;
				popupText.position.Y = position.Y + (float)newItem.height * 0.25f - vector.Y * 0.5f;
				popupText.velocity.Y = -7f;
				popupText.context = context;
				popupText.npcNetID = 0;
				popupText.effectStyle = PopupEffectStyle.None;
				if (popupText.coinText)
				{
					popupText.stack = 1L;
				}
				PrepareEffects(context, newItem, popupText);
				if (popupText.AnyEffect)
				{
					popupText.framesSinceSpawn = 0;
				}
				popupText.PrepareDisplayText();
				return i;
			}
			int num2 = FindNextItemTextSlot();
			if (num2 >= 0)
			{
				string text3 = newItem.AffixName();
				if (stack > 1)
				{
					text3 = text3 + " (" + stack + ")";
				}
				Vector2 vector2 = FontAssets.MouseText.Value.MeasureString(text3);
				PopupText popupText2 = PopupText.popupText[num2];
				ResetText(popupText2);
				popupText2.active = true;
				popupText2.position.X = position.X - vector2.X * 0.5f;
				popupText2.position.Y = position.Y - vector2.Y * 0.5f;
				popupText2.name = newItem.AffixName();
				popupText2.stack = stack;
				popupText2.velocity.Y = -7f;
				popupText2.lifeTime = 60;
				popupText2.context = context;
				if (longText)
				{
					popupText2.lifeTime *= 5;
				}
				popupText2.coinValue = 0L;
				popupText2.coinText = newItem.type >= 71 && newItem.type <= 74;
				if (popupText2.coinText)
				{
					long num3 = 0L;
					if (newItem.type == 71)
					{
						num3 += popupText2.stack;
					}
					else if (newItem.type == 72)
					{
						num3 += 100 * popupText2.stack;
					}
					else if (newItem.type == 73)
					{
						num3 += 10000 * popupText2.stack;
					}
					else if (newItem.type == 74)
					{
						num3 += 1000000 * popupText2.stack;
					}
					popupText2.AddToCoinValue(num3);
					popupText2.ValueToName();
					popupText2.stack = 1L;
					if (popupText2.coinValue >= 1000000)
					{
						if (popupText2.lifeTime < 300)
						{
							popupText2.lifeTime = 300;
						}
						popupText2.color = new Color(220, 220, 198);
					}
					else if (popupText2.coinValue >= 10000)
					{
						if (popupText2.lifeTime < 240)
						{
							popupText2.lifeTime = 240;
						}
						popupText2.color = new Color(224, 201, 92);
					}
					else if (popupText2.coinValue >= 100)
					{
						if (popupText2.lifeTime < 180)
						{
							popupText2.lifeTime = 180;
						}
						popupText2.color = new Color(181, 192, 193);
					}
					else if (popupText2.coinValue >= 1)
					{
						if (popupText2.lifeTime < 120)
						{
							popupText2.lifeTime = 120;
						}
						popupText2.color = new Color(246, 138, 96);
					}
				}
				PrepareEffects(context, newItem, popupText2);
				popupText2.PrepareDisplayText();
			}
			return num2;
		}

		private static void PrepareEffects(PopupTextContext context, Item newItem, PopupText somePopup)
		{
			if (newItem.rare == -13)
			{
				somePopup.master = true;
			}
			somePopup.expert = newItem.expert;
			CraftingEffectDetails effectDetails = CraftingEffects.GetEffectDetails(newItem);
			if (!somePopup.coinText)
			{
				somePopup.color = Item.GetPopupRarityColor(effectDetails.Rarity);
			}
			if (context == PopupTextContext.ItemCraft)
			{
				somePopup.effectIntensity = effectDetails.Intensity;
				somePopup.effectStyle = effectDetails.Style;
			}
		}

		private void PrepareDisplayText()
		{
			displayText = name;
			if (stack > 1)
			{
				displayText = displayText + " (" + stack + ")";
			}
			if (AnyEffect)
			{
				PrepareTextEffects();
			}
		}

		private void PrepareTextEffects()
		{
			int length = displayText.Length;
			if (charOffsets == null)
			{
				charOffsets = new Vector2[length];
			}
			Array.Resize(ref charOffsets, length);
			if (charColors == null)
			{
				charColors = new Color[length];
			}
			Array.Resize(ref charColors, length);
		}

		private static void EmitFancyFlashDust(PopupText somePopup)
		{
			Vector2 textHitbox = somePopup.GetTextHitbox();
			float num = 1f / somePopup.scale;
			textHitbox *= num;
			int num2 = 6 + somePopup.effectIntensity / 2;
			int num3 = -3 + somePopup.effectIntensity;
			num3 *= 4;
			if (num3 < 0)
			{
				num3 = 0;
			}
			num2 -= num3;
			if (somePopup.effectStyle == PopupEffectStyle.Potion)
			{
				num2 = 0;
				num3 = 0;
			}
			for (int i = 0; i < num2; i++)
			{
				float num4 = -0.1f + 1.2f * Main.rand.NextFloat();
				float x = somePopup.position.X + textHitbox.X * num4;
				float y = somePopup.position.Y + textHitbox.Y * (1f + 0.4f * (float)Math.Sin(num4 * (float)Math.PI));
				Dust dust = Dust.NewDustPerfect(new Vector2(x, y), 306, new Vector2(0f, Main.rand.NextFloatDirection()), 0, somePopup.color, 2f);
				dust.noGravity = true;
				dust.noLight = true;
				dust.noLightEmittance = true;
				dust.velocity.Y += -2f;
				dust.fadeIn = 1.4f * (1f + 0.4f * Main.rand.NextFloat());
				dust.scale = 0.6f + 0.4f * Main.rand.NextFloat();
				if (dust.scale >= 0.9f)
				{
					Dust dust2 = Dust.CloneDust(dust);
					dust2.color = new Color(255, 255, 255, 255);
					dust2.scale *= 0.65f;
					dust2.fadeIn = 1.1f;
				}
			}
			for (int j = 0; j < num3; j++)
			{
				float num5 = -0.1f + 1.2f * Main.rand.NextFloat();
				float x2 = somePopup.position.X + textHitbox.X * num5;
				float y2 = somePopup.position.Y + textHitbox.Y * (0.6f + 0.4f * (float)Math.Sin(num5 * (float)Math.PI));
				Dust dust3 = Dust.NewDustPerfect(new Vector2(x2, y2), 306, new Vector2(0f, Main.rand.NextFloatDirection()), 0, somePopup.color, 2f);
				dust3.noLight = true;
				dust3.noLightEmittance = true;
				dust3.velocity.X = dust3.velocity.RotatedBy((float)Math.PI * 2f * Main.rand.NextFloatDirection()).X;
				dust3.velocity.Y += -2f;
				dust3.fadeIn = 2.4f * (1f + 0.4f * Main.rand.NextFloat());
				dust3.scale = 0.6f + 0.4f * Main.rand.NextFloat();
				if (dust3.scale >= 0.9f)
				{
					Dust dust4 = Dust.CloneDust(dust3);
					dust4.color = new Color(255, 255, 255, 255);
					dust4.scale *= 0.65f;
					dust4.fadeIn = 1.1f;
				}
			}
		}

		private void AddToCoinValue(long addedValue)
		{
			long val = coinValue + addedValue;
			coinValue = Math.Min(9999999999L, Math.Max(0L, val));
		}

		private static int FindNextItemTextSlot()
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

		public static void AssignAsSonarText(int sonarTextIndex)
		{
			sonarText = sonarTextIndex;
			if (sonarText > -1)
			{
				popupText[sonarText].sonar = true;
			}
		}

		public static string ValueToName(long coinValue)
		{
			int num = 0;
			int num2 = 0;
			int num3 = 0;
			int num4 = 0;
			string text = "";
			long num5 = coinValue;
			while (num5 > 0)
			{
				if (num5 >= 1000000)
				{
					num5 -= 1000000;
					num++;
				}
				else if (num5 >= 10000)
				{
					num5 -= 10000;
					num2++;
				}
				else if (num5 >= 100)
				{
					num5 -= 100;
					num3++;
				}
				else if (num5 >= 1)
				{
					num5--;
					num4++;
				}
			}
			text = "";
			if (num > 0)
			{
				text = text + num + string.Format(" {0} ", Language.GetTextValue("Currency.Platinum"));
			}
			if (num2 > 0)
			{
				text = text + num2 + string.Format(" {0} ", Language.GetTextValue("Currency.Gold"));
			}
			if (num3 > 0)
			{
				text = text + num3 + string.Format(" {0} ", Language.GetTextValue("Currency.Silver"));
			}
			if (num4 > 0)
			{
				text = text + num4 + string.Format(" {0} ", Language.GetTextValue("Currency.Copper"));
			}
			if (text.Length > 1)
			{
				text = text.Substring(0, text.Length - 1);
			}
			return text;
		}

		private void ValueToName()
		{
			int num = 0;
			int num2 = 0;
			int num3 = 0;
			int num4 = 0;
			long num5 = coinValue;
			while (num5 > 0)
			{
				if (num5 >= 1000000)
				{
					num5 -= 1000000;
					num++;
				}
				else if (num5 >= 10000)
				{
					num5 -= 10000;
					num2++;
				}
				else if (num5 >= 100)
				{
					num5 -= 100;
					num3++;
				}
				else if (num5 >= 1)
				{
					num5--;
					num4++;
				}
			}
			name = "";
			if (num > 0)
			{
				name = name + num + string.Format(" {0} ", Language.GetTextValue("Currency.Platinum"));
			}
			if (num2 > 0)
			{
				name = name + num2 + string.Format(" {0} ", Language.GetTextValue("Currency.Gold"));
			}
			if (num3 > 0)
			{
				name = name + num3 + string.Format(" {0} ", Language.GetTextValue("Currency.Silver"));
			}
			if (num4 > 0)
			{
				name = name + num4 + string.Format(" {0} ", Language.GetTextValue("Currency.Copper"));
			}
			if (name.Length > 1)
			{
				name = name.Substring(0, name.Length - 1);
			}
		}

		public void Update(int whoAmI)
		{
			if (!active)
			{
				return;
			}
			framesSinceSpawn++;
			float targetScale = TargetScale;
			alpha += (float)alphaDir * 0.01f;
			if ((double)alpha <= 0.7)
			{
				alpha = 0.7f;
				alphaDir = 1;
			}
			if (alpha >= 1f)
			{
				alpha = 1f;
				alphaDir = -1;
			}
			if (expert)
			{
				color = new Color((byte)Main.DiscoR, (byte)Main.DiscoG, (byte)Main.DiscoB, Main.mouseTextColor);
			}
			else if (master)
			{
				color = new Color(255, (byte)(Main.masterColor * 200f), 0, Main.mouseTextColor);
			}
			bool flag = false;
			Vector2 textHitbox = GetTextHitbox();
			Rectangle rectangle = new Rectangle((int)(position.X - textHitbox.X / 2f), (int)(position.Y - textHitbox.Y / 2f), (int)textHitbox.X, (int)textHitbox.Y);
			if (AnyEffect && framesSinceSpawn == 8)
			{
				EmitFancyFlashDust(this);
			}
			for (int i = 0; i < 20; i++)
			{
				PopupText popupText = PopupText.popupText[i];
				if (!popupText.active || i == whoAmI)
				{
					continue;
				}
				Vector2 textHitbox2 = popupText.GetTextHitbox();
				Rectangle value = new Rectangle((int)(popupText.position.X - textHitbox2.X / 2f), (int)(popupText.position.Y - textHitbox2.Y / 2f), (int)textHitbox2.X, (int)textHitbox2.Y);
				if (rectangle.Intersects(value) && (position.Y < popupText.position.Y || (position.Y == popupText.position.Y && whoAmI < i)))
				{
					flag = true;
					int num = numActive;
					if (num > 3)
					{
						num = 3;
					}
					popupText.lifeTime = activeTime + 15 * num;
					lifeTime = activeTime + 15 * num;
				}
			}
			if (!flag)
			{
				velocity.Y *= 0.86f;
				if (scale == targetScale)
				{
					velocity.Y *= 0.4f;
				}
			}
			else if (velocity.Y > -6f)
			{
				velocity.Y -= 0.2f;
			}
			else
			{
				velocity.Y *= 0.86f;
			}
			velocity.X *= 0.93f;
			position += velocity;
			lifeTime--;
			if (lifeTime <= 0)
			{
				scale -= 0.03f * targetScale;
				if ((double)scale < 0.1 * (double)targetScale)
				{
					active = false;
					if (sonarText == whoAmI)
					{
						sonarText = -1;
					}
				}
				lifeTime = 0;
			}
			else
			{
				if (scale < targetScale)
				{
					scale += 0.1f * targetScale;
				}
				if (scale > targetScale)
				{
					scale = targetScale;
				}
			}
		}

		private Vector2 GetTextHitbox()
		{
			string text = displayText;
			Vector2 result = FontAssets.MouseText.Value.MeasureString(text);
			result *= scale;
			result.Y *= 0.8f;
			return result;
		}

		public static void UpdateItemText()
		{
			int num = 0;
			for (int i = 0; i < 20; i++)
			{
				if (popupText[i].active)
				{
					num++;
					popupText[i].Update(i);
				}
			}
			numActive = num;
		}

		public static void ClearAll()
		{
			for (int i = 0; i < 20; i++)
			{
				popupText[i] = new PopupText();
			}
			numActive = 0;
		}

		public static void DrawItemTextPopups(float scaleTarget)
		{
			SpriteBatch spriteBatch = Main.spriteBatch;
			Vector2 screenPosition = Main.screenPosition;
			int screenHeight = Main.screenHeight;
			for (int i = 0; i < 20; i++)
			{
				PopupText popupText = PopupText.popupText[i];
				if (!popupText.active)
				{
					continue;
				}
				string text = popupText.displayText;
				Vector2 vector = FontAssets.MouseText.Value.MeasureString(text);
				Vector2 vector2 = new Vector2(vector.X * 0.5f, vector.Y * 0.5f);
				float num = scaleTarget;
				float num2 = popupText.scale / num;
				int num3 = (int)(255f - 255f * num2);
				float num4 = (int)popupText.color.R;
				_ = (float)(int)popupText.color.G;
				_ = (float)(int)popupText.color.B;
				float num5 = (int)popupText.color.A;
				num4 *= num2 * popupText.alpha * 0.3f;
				_ = popupText.alpha;
				_ = popupText.alpha;
				num5 *= num2 * popupText.alpha;
				Color color = Color.Black;
				float num6 = 1f;
				Texture2D texture2D = null;
				Vector2[] array = null;
				Color[] array2 = null;
				switch (popupText.context)
				{
				case PopupTextContext.ItemPickupToVoidContainer:
					color = new Color(127, 20, 255) * 0.4f;
					num6 = 0.8f;
					break;
				case PopupTextContext.SonarAlert:
					color = Color.Blue * 0.4f;
					if (popupText.npcNetID != 0)
					{
						color = Color.Red * 0.4f;
					}
					num6 = 1f;
					break;
				case PopupTextContext.ItemReforge_Best:
					color = Main.hslToRgb(Main.GlobalTimeWrappedHourly * 0.6f % 1f, 1f, 0.6f) * 0.6f;
					num *= 0.5f;
					num6 = 0.8f;
					break;
				}
				int num7 = 40;
				float num8 = Utils.Remap(popupText.framesSinceSpawn, 0f, num7, 0f, 1f);
				float num9 = (float)Utils.EaseOutCirc(num8);
				if (popupText.effectStyle == PopupEffectStyle.Metal || popupText.effectStyle == PopupEffectStyle.MagicWeapon || popupText.effectStyle == PopupEffectStyle.RangedWeapon || popupText.effectStyle == PopupEffectStyle.SummonWeapon || popupText.effectStyle == PopupEffectStyle.MeleeWeapon)
				{
					Vector2 vector3 = new Vector2(0f, -4f);
					Vector2 vector4 = popupText.position - screenPosition + vector2;
					float num10 = popupText.scale;
					num6 = (float)Utils.Lerp(0.6000000238418579, 1.0, num8);
					Vector3 vector5 = Main.rgbToHsl(popupText.color);
					Color value = Main.hslToRgb(vector5.X, vector5.Y, 1f - num8);
					value.A = 0;
					float amount = (float)Utils.EaseInCirc(Utils.Clamp(num8 * 1.25f, 0f, 1f));
					color = Color.Lerp(value, Color.Black, amount);
					float num11 = Utils.Remap(num8, 0f, 0.1f, 0f, 1f) * Utils.Remap(num8, 0.1f, 1f, 1f, 0f);
					float num12 = Utils.Remap(num8, 0f, 0.2f, 0f, 1f) * Utils.Remap(num8, 0.2f, 0.8f, 1f, 0f);
					Texture2D value2 = TextureAssets.Extra[98].Value;
					Vector2 origin = value2.Frame().Size() / 2f;
					Vector2 vector6 = new Vector2(1f, vector.X / (float)value2.Width);
					vector6 *= num10;
					if (num11 > 0f)
					{
						Vector2 vector7 = new Vector2(Utils.Remap(num9, 0f, 1f, -20f, 20f), 0f) + vector3;
						vector7 *= num10;
						Vector2 vector8 = new Vector2(-60f, 0f);
						while (vector8.X <= 40f)
						{
							spriteBatch.Draw(value2, vector4 + vector7 + vector8 * num10, null, popupText.color * num11, (float)Math.PI / 2f, origin, vector6, SpriteEffects.None, 0f);
							vector8.X += 40f;
						}
						Vector2 vector9 = new Vector2(-20f, 0f);
						while (vector9.X <= 20f)
						{
							spriteBatch.Draw(value2, vector4 + vector7 + vector9 * num10, null, new Color(255, 255, 255, 0) * 0.5f * num12, (float)Math.PI / 2f, origin, vector6 * 0.5f, SpriteEffects.None, 0f);
							vector9.X += 20f;
						}
					}
					float num13 = (float)Math.PI * 2f * num8;
					float fromValue = (float)Utils.EaseOutCirc(Utils.Clamp(num8 * 2f, 0f, 1f));
					float num14 = Utils.Remap(num8, 0.1f, 0.3f, 0f, 1f) * Utils.Remap(num8, 0.3f, 0.6f, 1f, 0f);
					Vector2 vector10 = new Vector2(vector.X, 0f) * Utils.Remap(fromValue, 0f, 1f, -0.8f, 0.4f) + vector3;
					vector10 *= num10;
					spriteBatch.Draw(value2, vector4 + vector10, null, popupText.color * num14, 0f + num13, origin, num10, SpriteEffects.None, 0f);
					spriteBatch.Draw(value2, vector4 + vector10, null, popupText.color * num14, (float)Math.PI / 2f + num13, origin, num10 * 1.3f, SpriteEffects.None, 0f);
					spriteBatch.Draw(value2, vector4 + vector10, null, new Color(255, 255, 255, 0) * num14, 0f + num13, origin, new Vector2(0.5f, 0.5f) * num10 * 1.3f, SpriteEffects.None, 0f);
					spriteBatch.Draw(value2, vector4 + vector10, null, new Color(255, 255, 255, 0) * num14, (float)Math.PI / 2f + num13, origin, new Vector2(0.5f, 0.5f) * num10, SpriteEffects.None, 0f);
					num13 = 0f;
					spriteBatch.Draw(value2, vector4 + vector10, null, popupText.color * num14, 0f + num13, origin, num10, SpriteEffects.None, 0f);
					spriteBatch.Draw(value2, vector4 + vector10, null, popupText.color * num14, (float)Math.PI / 2f + num13, origin, num10 * 1.3f, SpriteEffects.None, 0f);
					spriteBatch.Draw(value2, vector4 + vector10, null, new Color(255, 255, 255, 0) * num14, 0f + num13, origin, new Vector2(0.5f, 0.5f) * num10 * 1.3f, SpriteEffects.None, 0f);
					spriteBatch.Draw(value2, vector4 + vector10, null, new Color(255, 255, 255, 0) * num14, (float)Math.PI / 2f + num13, origin, new Vector2(0.5f, 0.5f) * num10, SpriteEffects.None, 0f);
					array2 = popupText.charColors;
					float num15 = 1f / (float)text.Length;
					for (int j = 0; j < text.Length; j++)
					{
						float amount2 = Utils.Remap(num9, num15 * (float)j, num15 * (float)(j + 1), 0f, 1f);
						array2[j] = Color.Lerp(Color.White, popupText.color, amount2);
					}
				}
				if (popupText.effectStyle == PopupEffectStyle.MagicWeapon)
				{
					array = popupText.charOffsets;
					float num16 = 1f / (float)text.Length;
					for (int k = 0; k < text.Length; k++)
					{
						Utils.Remap(num9 * 1.25f, num16 * (float)k, num16 * (float)(k + 1), 0f, 1f);
						Vector2 value3 = new Vector2((0f - (float)Math.Sin((float)Math.PI * 2f * ((float)(k * 31) / 12f))) * 144f, 0f);
						array[k] = Vector2.Lerp(value3, Vector2.Zero, num9);
					}
				}
				if (popupText.effectStyle == PopupEffectStyle.RangedWeapon)
				{
					array = popupText.charOffsets;
					array2 = popupText.charColors;
					color = Color.Transparent;
					float num17 = 1f / (float)text.Length;
					float num18 = Utils.Clamp(num9 * 3f, 0f, 1f);
					float num19 = Utils.Clamp(num9 * 1.5f, 0f, 1f);
					for (int l = 0; l < text.Length; l++)
					{
						float num20 = Utils.Remap(num9, num17 * (float)l, num17 * (float)(l + 1), 0f, 1f);
						array2[l] = Color.Lerp(new Color(0, 0, 0, 1), popupText.color, num20);
						Vector2 value4 = new Vector2(60f * num20 - 120f * num18, ((float)Math.Sin((float)Math.PI * 2f * ((float)(l * 31) / 12f)) * 0.5f + 0.5f) * -204f * (1f - num19));
						array[l] = Vector2.Lerp(value4, Vector2.Zero, num9);
					}
				}
				if (popupText.effectStyle == PopupEffectStyle.Potion)
				{
					array = popupText.charOffsets;
					float num21 = 1f / (float)text.Length;
					for (int m = 0; m < text.Length; m++)
					{
						Utils.Remap(num9 * 1.25f, num21 * (float)m, num21 * (float)(m + 1), 0f, 1f);
						Vector2 value5 = new Vector2(0f, (float)Math.Sin((float)Math.PI * 2f * ((float)m / 12f)) * 20f);
						array[m] = Vector2.Lerp(value5, Vector2.Zero, num9);
					}
				}
				float num22 = (float)num3 / 255f;
				for (int n = 0; n < 5; n++)
				{
					Color value6 = color;
					float num23 = 0f;
					float num24 = 0f;
					switch (n)
					{
					case 0:
						num23 -= num * 2f;
						break;
					case 1:
						num23 += num * 2f;
						break;
					case 2:
						num24 -= num * 2f;
						break;
					case 3:
						num24 += num * 2f;
						break;
					default:
						value6 = popupText.color * num2 * popupText.alpha * num6;
						break;
					}
					if (n < 4)
					{
						num5 = (float)(int)popupText.color.A * num2 * popupText.alpha;
						value6 = new Color(0, 0, 0, (int)num5);
					}
					if (color != Color.Black && n < 4)
					{
						num23 *= 1.3f + 1.3f * num22;
						num24 *= 1.3f + 1.3f * num22;
					}
					float num25 = popupText.position.Y - screenPosition.Y + num24;
					if (Main.player[Main.myPlayer].gravDir == -1f)
					{
						num25 = (float)screenHeight - num25;
					}
					if (color != Color.Black && n < 4)
					{
						Color value7 = color;
						value7.A = (byte)MathHelper.Lerp(60f, 127f, Utils.GetLerpValue(0f, 255f, num5, clamped: true));
						spriteBatch.DrawString(FontAssets.MouseText.Value, text, new Vector2(popupText.position.X - screenPosition.X + num23 + vector2.X, num25 + vector2.Y), Color.Lerp(value6, value7, 0.5f), popupText.rotation, vector2, popupText.scale, SpriteEffects.None, 0f, array);
						spriteBatch.DrawString(FontAssets.MouseText.Value, text, new Vector2(popupText.position.X - screenPosition.X + num23 + vector2.X, num25 + vector2.Y), value7, popupText.rotation, vector2, popupText.scale, SpriteEffects.None, 0f, array);
					}
					else
					{
						spriteBatch.DrawString(FontAssets.MouseText.Value, text, new Vector2(popupText.position.X - screenPosition.X + num23 + vector2.X, num25 + vector2.Y), value6, popupText.rotation, vector2, popupText.scale, SpriteEffects.None, 0f, array, (n == 4) ? array2 : null);
					}
					if (texture2D != null)
					{
						float num26 = (1.3f - num22) * popupText.scale * 0.7f;
						Vector2 vector11 = new Vector2(popupText.position.X - screenPosition.X + num23 + vector2.X, num25 + vector2.Y);
						Color color2 = color * 0.6f;
						if (n == 4)
						{
							color2 = Color.White * 0.6f;
						}
						color2.A = (byte)((float)(int)color2.A * 0.5f);
						int num27 = 25;
						spriteBatch.Draw(texture2D, vector11 + new Vector2(vector2.X * -0.5f - (float)num27 - texture2D.Size().X / 2f, 0f), null, color2 * popupText.scale, 0f, texture2D.Size() / 2f, num26, SpriteEffects.None, 0f);
						spriteBatch.Draw(texture2D, vector11 + new Vector2(vector2.X * 0.5f + (float)num27 + texture2D.Size().X / 2f, 0f), null, color2 * popupText.scale, 0f, texture2D.Size() / 2f, num26, SpriteEffects.None, 0f);
					}
				}
			}
		}
	}
}
