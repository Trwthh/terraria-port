using System;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;
using ReLogic.Graphics;
using Terraria.Audio;
using Terraria.GameContent;
using Terraria.GameInput;
using Terraria.ID;
using Terraria.Localization;
using Terraria.UI.Gamepad;

namespace Terraria.UI
{
	public class BannerClaimingUI : IPipsUI
	{
		private struct Entry
		{
			public int Index;

			public int Type;

			public int Stack;
		}

		private int _focusedElementIndex;

		private int _gridSelectionCandidateIndex;

		private float _currentElementOffset;

		private int _availableItemsCount;

		private int _gridStartingElementIndex;

		private int _bannerStackRequestLimiter;

		private Entry[] _squashedItemTypesToShow = new Entry[400];

		private int previousFocusBanner = -1;

		private int previousFocusTime;

		public bool AnyAvailableBanners { get; private set; }

		private float inventoryScale
		{
			get
			{
				return Main.inventoryScale;
			}
			set
			{
				Main.inventoryScale = value;
			}
		}

		private Color inventoryBack
		{
			get
			{
				return Main.inventoryBack;
			}
			set
			{
				Main.inventoryBack = value;
			}
		}

		public int FocusedItemType
		{
			get
			{
				if (_focusedElementIndex >= 0 && _focusedElementIndex < _availableItemsCount)
				{
					return _squashedItemTypesToShow[_focusedElementIndex].Type;
				}
				return 0;
			}
		}

		public void DrawBannersList(SpriteBatch spriteBatch, int adjY, int middleY, Color craftingTipColor)
		{
			UILinkPointNavigator.Shortcuts.CRAFT_CurrentRecipeBig = -1;
			UILinkPointNavigator.Shortcuts.CRAFT_CurrentRecipeSmall = -1;
			int num = UpdateAndGetClaimableItemsCount();
			BannerSystem.AnyNewClaimableBanners = false;
			if (num <= 0)
			{
				Main.TryChangePipsPage(Main.PipPage.Recipes);
				return;
			}
			spriteBatch.DrawString(FontAssets.MouseText.Value, Language.GetTextValue("GameUI.BannersTitle"), new Vector2(76f, 414 + adjY), craftingTipColor, 0f, default(Vector2), 1f, SpriteEffects.None, 0f);
			DrawBannersList_Move(num);
			if (Main.PipsFastScroll)
			{
				_currentElementOffset = _focusedElementIndex;
				Main.PipsFastScroll = false;
			}
			bool flag = Main.mouseLeft || Main.mouseRight;
			if (!flag)
			{
				_bannerStackRequestLimiter = 0;
			}
			for (int i = 0; i < num; i++)
			{
				float num2 = ((float)i - _currentElementOffset) * 65f;
				if (Math.Abs(num2) > (float)middleY)
				{
					continue;
				}
				Entry bannerEntry = _squashedItemTypesToShow[i];
				if (!ContentSamples.ItemsByType.TryGetValue(bannerEntry.Type, out var value))
				{
					continue;
				}
				inventoryScale = 100f / (Math.Abs(num2) + 100f);
				if ((double)inventoryScale < 0.75)
				{
					inventoryScale = 0.75f;
				}
				GetItemSlotColors(num2, middleY, 100f, i, out var inventoryAlpha, out var inventoryColor);
				int num3 = (int)(46f - 26f * inventoryScale);
				int num4 = (int)(410f + num2 * inventoryScale - 30f * inventoryScale + (float)adjY);
				if (!Main.LocalPlayer.creativeInterface && Main.mouseX >= num3 && (float)Main.mouseX <= (float)num3 + (float)TextureAssets.InventoryBack.Width() * inventoryScale && Main.mouseY >= num4 && (float)Main.mouseY <= (float)num4 + (float)TextureAssets.InventoryBack.Height() * inventoryScale && !PlayerInput.IgnoreMouseInterface)
				{
					Main.craftingHide = true;
					Main.LocalPlayer.mouseInterface = true;
					BannerSystem.ClearNewBanner(bannerEntry.Index);
					if (flag)
					{
						if (_focusedElementIndex != i)
						{
							if ((Main.mouseLeft && Main.mouseLeftRelease) || (Main.mouseRight && Main.mouseRightRelease))
							{
								_focusedElementIndex = i;
								_bannerStackRequestLimiter = -1;
							}
						}
						else if (num2 == 0f)
						{
							TryClaimingBanner(bannerEntry);
						}
					}
					ItemSlot.MouseHover(value, 35);
				}
				inventoryAlpha -= 50.0;
				if (inventoryAlpha < 0.0)
				{
					inventoryAlpha = 0.0;
				}
				if (i == _focusedElementIndex)
				{
					UILinkPointNavigator.Shortcuts.CRAFT_CurrentRecipeSmall = 0;
					if (PlayerInput.SettingsForUI.HighlightThingsForMouse)
					{
						ItemSlot.DrawGoldBGForCraftingMaterial = true;
					}
				}
				else
				{
					UILinkPointNavigator.Shortcuts.CRAFT_CurrentRecipeSmall = -1;
				}
				Color color = inventoryBack;
				inventoryBack = new Color((byte)inventoryAlpha, (byte)inventoryAlpha, (byte)inventoryAlpha, (byte)inventoryAlpha);
				int stack = value.stack;
				value.stack = bannerEntry.Stack;
				ItemSlot.DrawNewAndShinyHighlightOverride = !ItemSlot.DrawGoldBGForCraftingMaterial && BannerSystem.IsNewBanner(bannerEntry.Index);
				ItemSlot.Draw(spriteBatch, ref value, 35, new Vector2(num3, num4), inventoryColor);
				value.stack = stack;
				ItemSlot.DrawNewAndShinyHighlightOverride = false;
				inventoryBack = color;
			}
		}

		private void DrawBannersList_Move(int claimableItemsCount)
		{
			_focusedElementIndex = Utils.Clamp(_focusedElementIndex, 0, claimableItemsCount - 1);
			_currentElementOffset = Utils.Clamp(_currentElementOffset, 0f, claimableItemsCount - 1);
			int num = (int)_currentElementOffset;
			_currentElementOffset = MathHelper.Lerp(_currentElementOffset, _focusedElementIndex, 0.03f);
			_currentElementOffset = Utils.MoveTowards(_currentElementOffset, _focusedElementIndex, 0.1f);
			if (num != (int)_currentElementOffset)
			{
				SoundEngine.PlaySound(12);
			}
		}

		private void TryClaimingBanner(Entry bannerEntry)
		{
			ItemSlot.HandleItemPickupAction(SendRequestForBanner, bannerEntry, bannerEntry.Type, bannerEntry.Stack, ref _bannerStackRequestLimiter);
		}

		private void SendRequestForBanner(Entry bannerEntry, int itemAmount)
		{
			BannerSystem.RequestBannerClaim(bannerEntry.Index, itemAmount);
			SoundEngine.PlaySound(7);
		}

		private int UpdateAndGetClaimableItemsCount()
		{
			ushort[] claimableBannerCounts = BannerSystem.GetClaimableBannerCounts();
			Entry[] squashedItemTypesToShow = _squashedItemTypesToShow;
			int num = 0;
			Array.Clear(squashedItemTypesToShow, 0, squashedItemTypesToShow.Length);
			for (int i = 1; i < claimableBannerCounts.Length; i++)
			{
				ushort num2 = claimableBannerCounts[i];
				if (num2 > 0)
				{
					squashedItemTypesToShow[num++] = new Entry
					{
						Index = i,
						Type = BannerSystem.BannerToItem(i),
						Stack = num2
					};
				}
			}
			AnyAvailableBanners = num > 0;
			_availableItemsCount = num;
			if (!AnyAvailableBanners)
			{
				BannerSystem.AnyNewClaimableBanners = false;
			}
			return num;
		}

		private void GetItemSlotColors(float offset, int middleY, float fadeInValue, int recipeIndex, out double inventoryAlpha, out Color inventoryColor2)
		{
			inventoryAlpha = inventoryBack.A + 50;
			double num = 255.0;
			if (Math.Abs(offset) > (float)middleY - fadeInValue)
			{
				inventoryAlpha = (double)(150f * (fadeInValue - (Math.Abs(offset) - ((float)middleY - fadeInValue)))) * 0.01;
				num = (double)(255f * (fadeInValue - (Math.Abs(offset) - ((float)middleY - fadeInValue)))) * 0.01;
			}
			new Color((byte)inventoryAlpha, (byte)inventoryAlpha, (byte)inventoryAlpha, (byte)inventoryAlpha);
			inventoryColor2 = new Color((byte)num, (byte)num, (byte)num, (byte)num);
		}

		public void DrawGridToggle(SpriteBatch spriteBatch, int adjY)
		{
			if (Main.PipsCurrentPage != Main.PipPage.Banners)
			{
				previousFocusBanner = -1;
			}
			if (UpdateAndGetClaimableItemsCount() == 0)
			{
				if (Main.PipsCurrentPage == Main.PipPage.Banners)
				{
					Main.PipsUseGrid = false;
				}
				return;
			}
			int num = 128;
			int num2 = 450 + adjY;
			if (Main.InGuideCraftMenu)
			{
				num2 -= 150;
			}
			UILinkPointNavigator.SetPosition(11002, new Vector2(num, num2));
			bool flag = Main.mouseX > num - 15 && Main.mouseX < num + 15 && Main.mouseY > num2 - 15 && Main.mouseY < num2 + 15 && !PlayerInput.IgnoreMouseInterface;
			if (Main.PipsCurrentPage == Main.PipPage.Banners)
			{
				Utils.DrawSelectedCraftingBarIndicator(spriteBatch, num, num2);
			}
			int num3 = 2 - (Main.PipsCurrentPage == Main.PipPage.Banners && Main.PipsUseGrid).ToInt() * 2 + flag.ToInt();
			Asset<Texture2D> asset = TextureAssets.BannerToggle[num3];
			spriteBatch.Draw(asset.Value, new Vector2(num, num2), null, Color.White, 0f, asset.Size() / 2f, 1f, SpriteEffects.None, 0f);
			if (flag)
			{
				Main.instance.MouseTextNoOverride(Language.GetTextValue("GameUI.BannersWindow"), 0, 0);
				Main.player[Main.myPlayer].mouseInterface = true;
				if (Main.mouseLeft && Main.mouseLeftRelease)
				{
					if (!Main.TryChangePipsPage(Main.PipPage.Banners))
					{
						Main.PipsUseGrid = !Main.PipsUseGrid;
					}
					SoundEngine.PlaySound(12);
				}
			}
			Main.DoStatefulTickSound(ref Main.GridToggleMouseOverBanners, flag);
			if (BannerSystem.AnyNewClaimableBanners)
			{
				Utils.DrawNotificationIcon(spriteBatch, new Vector2(num, num2 - 7));
			}
		}

		public void DrawBannersGrid(SpriteBatch spriteBatch)
		{
			int num = UpdateAndGetClaimableItemsCount();
			BannerSystem.AnyNewClaimableBanners = false;
			UILinkPointNavigator.Shortcuts.CRAFT_CurrentRecipeBig = -1;
			UILinkPointNavigator.Shortcuts.CRAFT_CurrentRecipeSmall = -1;
			int num2 = 42;
			inventoryScale = 0.75f;
			int num3 = 340;
			int num4 = 310;
			int num5 = (Main.screenWidth - num4 - 280) / num2;
			int num6 = (Main.screenHeight - num3 - 20) / num2;
			UILinkPointNavigator.Shortcuts.CRAFT_IconsPerRow = num5;
			UILinkPointNavigator.Shortcuts.CRAFT_IconsPerColumn = num6;
			int num7 = 0;
			int num8 = 0;
			int num9 = num4;
			int num10 = num3;
			int mouseX = Main.mouseX;
			int mouseY = Main.mouseY;
			int num11 = num4 - 20;
			int num12 = num3 + 2;
			if (_gridStartingElementIndex > num - num5 * num6)
			{
				_gridStartingElementIndex = num - num5 * num6;
				if (_gridStartingElementIndex < 0)
				{
					_gridStartingElementIndex = 0;
				}
			}
			if (_gridStartingElementIndex > 0)
			{
				if (mouseX >= num11 && mouseX <= num11 + TextureAssets.CraftUpButton.Width() && mouseY >= num12 && mouseY <= num12 + TextureAssets.CraftUpButton.Height() && !PlayerInput.IgnoreMouseInterface)
				{
					Main.LocalPlayer.mouseInterface = true;
					if (Main.mouseLeftRelease && Main.mouseLeft)
					{
						_gridStartingElementIndex -= num5;
						if (_gridStartingElementIndex < 0)
						{
							_gridStartingElementIndex = 0;
						}
						SoundEngine.PlaySound(12);
						Main.mouseLeftRelease = false;
					}
				}
				spriteBatch.Draw(TextureAssets.CraftUpButton.Value, new Vector2(num11, num12), null, new Color(200, 200, 200, 200), 0f, default(Vector2), 1f, SpriteEffects.None, 0f);
			}
			if (_gridStartingElementIndex < num - num5 * num6)
			{
				num12 += 20;
				if (mouseX >= num11 && mouseX <= num11 + TextureAssets.CraftUpButton.Width() && mouseY >= num12 && mouseY <= num12 + TextureAssets.CraftUpButton.Height() && !PlayerInput.IgnoreMouseInterface)
				{
					Main.LocalPlayer.mouseInterface = true;
					if (Main.mouseLeftRelease && Main.mouseLeft)
					{
						_gridStartingElementIndex += num5;
						SoundEngine.PlaySound(12);
						if (_gridStartingElementIndex > num - num5)
						{
							_gridStartingElementIndex = num - num5;
						}
						Main.mouseLeftRelease = false;
					}
				}
				spriteBatch.Draw(TextureAssets.CraftDownButton.Value, new Vector2(num11, num12), null, new Color(200, 200, 200, 200), 0f, default(Vector2), 1f, SpriteEffects.None, 0f);
			}
			_gridStartingElementIndex = Utils.Clamp(_gridStartingElementIndex, 0, num - 1);
			_gridSelectionCandidateIndex = Utils.Clamp(_gridSelectionCandidateIndex, 0, num - 1);
			for (int i = _gridStartingElementIndex; i < num && i < num; i++)
			{
				int num13 = num9;
				int num14 = num10;
				double num15 = inventoryBack.A + 50;
				double num16 = 255.0;
				Entry entry = _squashedItemTypesToShow[i];
				if (!ContentSamples.ItemsByType.TryGetValue(entry.Type, out var value))
				{
					continue;
				}
				new Color((byte)num15, (byte)num15, (byte)num15, (byte)num15);
				new Color((byte)num16, (byte)num16, (byte)num16, (byte)num16);
				if (mouseX >= num13 && (float)mouseX <= (float)num13 + (float)TextureAssets.InventoryBack.Width() * inventoryScale && mouseY >= num14 && (float)mouseY <= (float)num14 + (float)TextureAssets.InventoryBack.Height() * inventoryScale && !PlayerInput.IgnoreMouseInterface)
				{
					Main.LocalPlayer.mouseInterface = true;
					if (Main.mouseLeftRelease && Main.mouseLeft)
					{
						_focusedElementIndex = i;
						Main.PipsFastScroll = true;
						Main.PipsUseGrid = false;
						SoundEngine.PlaySound(12);
						Main.mouseLeftRelease = false;
						if (PlayerInput.UsingGamepadUI)
						{
							UILinkPointNavigator.ChangePage(22);
							Main.LockCraftingForThisCraftClickDuration();
						}
					}
					Main.craftingHide = true;
					Main.HoverItem = value.Clone();
					Main.HoverItem.stack = entry.Stack;
					ItemSlot.MouseHover(35);
					BannerSystem.ClearNewBanner(entry.Index);
					Main.hoverItemName = value.Name;
					if (value.stack > 1)
					{
						Main.hoverItemName = Main.hoverItemName + " (" + value.stack + ")";
					}
				}
				if (num > 0)
				{
					num15 -= 50.0;
					if (num15 < 0.0)
					{
						num15 = 0.0;
					}
					UILinkPointNavigator.Shortcuts.CRAFT_CurrentRecipeBig = ((_gridSelectionCandidateIndex != i) ? (-1) : 0);
					Color color = inventoryBack;
					inventoryBack = new Color((byte)num15, (byte)num15, (byte)num15, (byte)num15);
					int stack = value.stack;
					value.stack = entry.Stack;
					ItemSlot.DrawNewAndShinyHighlightOverride = BannerSystem.IsNewBanner(entry.Index);
					ItemSlot.Draw(spriteBatch, ref value, 35, new Vector2(num13, num14));
					ItemSlot.DrawNewAndShinyHighlightOverride = false;
					value.stack = stack;
					inventoryBack = color;
				}
				num9 += num2;
				num7++;
				if (num7 >= num5)
				{
					num9 = num4;
					num10 += num2;
					num7 = 0;
					num8++;
					if (num8 >= num6)
					{
						break;
					}
				}
			}
		}

		public void ScrollCraftingList(int mouseWheel)
		{
			_focusedElementIndex += mouseWheel;
		}

		public void ScrollCraftingGrid(int mouseWheel, int width)
		{
			_gridStartingElementIndex -= mouseWheel * width;
			if (mouseWheel > 0)
			{
				SoundEngine.PlaySound(12);
			}
		}

		public void NavigatePipsList(int yOffset)
		{
			float currentElementOffset = _currentElementOffset;
			_focusedElementIndex = Utils.Clamp(_focusedElementIndex + yOffset, 0, _availableItemsCount - 1);
			if (currentElementOffset != _currentElementOffset)
			{
				SoundEngine.PlaySound(12);
			}
		}

		public void NavigatePipsGrid(int xOffset, int yOffset)
		{
			int cRAFT_IconsPerRow = UILinkPointNavigator.Shortcuts.CRAFT_IconsPerRow;
			int num = Math.Min(UILinkPointNavigator.Shortcuts.CRAFT_IconsPerColumn, (int)Math.Ceiling((float)_availableItemsCount / (float)cRAFT_IconsPerRow));
			int num2 = _gridSelectionCandidateIndex - _gridStartingElementIndex;
			int num3 = num2 % cRAFT_IconsPerRow;
			int num4 = num2 / cRAFT_IconsPerRow;
			if (xOffset < 0 && num3 == 0)
			{
				xOffset = 0;
			}
			if (xOffset > 0 && num3 >= cRAFT_IconsPerRow)
			{
				xOffset = 0;
			}
			if (yOffset < 0 && _gridStartingElementIndex > 0 && num4 == 0)
			{
				_gridStartingElementIndex -= cRAFT_IconsPerRow;
			}
			if (yOffset > 0 && num4 == cRAFT_IconsPerRow - 1)
			{
				_gridStartingElementIndex += cRAFT_IconsPerRow;
			}
			_gridStartingElementIndex = Utils.Clamp(_gridStartingElementIndex, 0, Math.Max(0, _availableItemsCount - 1 - cRAFT_IconsPerRow * num));
			int num5 = _gridSelectionCandidateIndex - _gridStartingElementIndex;
			num3 = num5 % cRAFT_IconsPerRow;
			num4 = num5 / cRAFT_IconsPerRow;
			if (yOffset < 0 && _gridStartingElementIndex == 0 && num4 == 0)
			{
				yOffset = 0;
			}
			if (yOffset > 0 && num4 == num - 1)
			{
				yOffset = 0;
			}
			int num6 = (_availableItemsCount - _gridStartingElementIndex) % cRAFT_IconsPerRow;
			if (yOffset > 0 && num6 != 0 && num4 == num - 2 && num3 >= num6)
			{
				yOffset = 0;
			}
			int gridSelectionCandidateIndex = _gridSelectionCandidateIndex;
			int num7 = xOffset + yOffset * cRAFT_IconsPerRow;
			_gridSelectionCandidateIndex = Utils.Clamp(_gridSelectionCandidateIndex + num7, 0, _availableItemsCount - 1);
			if (gridSelectionCandidateIndex != _gridSelectionCandidateIndex)
			{
				SoundEngine.PlaySound(12);
			}
		}

		public void ResetGridSelection()
		{
			_gridSelectionCandidateIndex = 0;
		}
	}
}
