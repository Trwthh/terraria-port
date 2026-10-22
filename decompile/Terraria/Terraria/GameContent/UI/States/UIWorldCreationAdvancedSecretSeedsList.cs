using System;
using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;
using Terraria.Audio;
using Terraria.GameContent.Drawing;
using Terraria.GameContent.UI.Elements;
using Terraria.Graphics.Renderers;
using Terraria.ID;
using Terraria.Localization;
using Terraria.UI;
using Terraria.UI.Gamepad;

namespace Terraria.GameContent.UI.States
{
	public class UIWorldCreationAdvancedSecretSeedsList : UIState, IHaveBackButtonCommand
	{
		private UIWorldCreationAdvanced _creationState;

		private UIElement _backButton;

		private UIList _worldList;

		private UIElement _containerPanel;

		private UIScrollbar _scrollbar;

		private bool _isScrollbarAttached;

		private UIWorldCreation _creationState2;

		private ParticleRenderer SeedParticleSystem = new ParticleRenderer();

		private UIDust SeedDust = new UIDust();

		private UIText _descriptionText;

		private UIGamepadHelper _helper;

		public UIWorldCreationAdvancedSecretSeedsList(UIWorldCreationAdvanced state, UIWorldCreation state2)
		{
			_creationState = state;
			_creationState2 = state2;
			BuildPage();
		}

		private void BuildPage()
		{
			SeedDust.Clear();
			SeedParticleSystem.Clear();
			RemoveAllChildren();
			UIElement uIElement = new UIElement
			{
				Width = StyleDimension.FromPixels(500f),
				Height = StyleDimension.FromPixelsAndPercent(-200f, 1f),
				Top = StyleDimension.FromPixels(202f),
				HAlign = 0.5f,
				VAlign = 0f
			};
			uIElement.MaxHeight = StyleDimension.FromPixels(400f);
			uIElement.SetPadding(0f);
			Append(uIElement);
			UIPanel uIPanel = new UIPanel
			{
				Width = StyleDimension.FromPercent(1f),
				Height = StyleDimension.FromPixelsAndPercent(-102f, 1f),
				BackgroundColor = new Color(33, 43, 79) * 0.8f
			};
			uIPanel.SetPadding(0f);
			uIElement.Append(uIPanel);
			MakeBackAndCreatebuttons(uIElement);
			int num = 56;
			int num2 = 4;
			UIElement uIElement2 = new UIElement
			{
				Top = StyleDimension.FromPixelsAndPercent(num2, 0f),
				Width = StyleDimension.FromPixelsAndPercent(-20f, 1f),
				Left = StyleDimension.FromPixelsAndPercent(2f, 0f),
				Height = StyleDimension.FromPixelsAndPercent(-num2 - num, 1f),
				HAlign = 0.5f
			};
			uIElement2.SetPadding(0f);
			uIElement2.PaddingTop = 8f;
			uIElement2.PaddingBottom = 12f;
			uIPanel.Append(uIElement2);
			_worldList = new UIList();
			_worldList.Width.Set(0f, 1f);
			_worldList.Height.Set(0f, 1f);
			_worldList.ListPadding = 5f;
			uIElement2.Append(_worldList);
			_containerPanel = uIElement2;
			_scrollbar = new UIScrollbar();
			_scrollbar.SetView(100f, 1000f);
			_scrollbar.Height.Set(0f, 1f);
			_scrollbar.HAlign = 1f;
			_worldList.SetScrollbar(_scrollbar);
			List<WorldGen.SecretSeed> seedsForInterface = SecretSeedsTracker.SeedsForInterface;
			_worldList.ManualSortMethod = CustomSort;
			int num3 = 0;
			foreach (WorldGen.SecretSeed item in seedsForInterface)
			{
				GroupOptionButton<WorldGen.SecretSeed> groupOptionButton = new GroupOptionButton<WorldGen.SecretSeed>(item, null, Language.GetText(item.Localization), Color.White, null)
				{
					Width = StyleDimension.FromPixelsAndPercent(0f, 1f),
					Height = new StyleDimension(40f, 0f),
					HAlign = 0f
				};
				groupOptionButton.SetSnapPoint("Seed", num3++);
				UIElement uIElement3 = new UIElement();
				groupOptionButton.Append(uIElement3);
				groupOptionButton.SetTextWithoutLocalization(item.TextThatWasUsedToUnlock, 1f, Color.White, 0f, 10f);
				groupOptionButton.OnLeftMouseDown += ClickSecretSeed;
				groupOptionButton.OnMouseOver += MouseOverSeed;
				groupOptionButton.OnMouseOut += MouseOutSeed;
				groupOptionButton.SetCurrentOption(item.Enabled ? item : null);
				uIElement3.OnDraw += DrawGlowRing;
				_worldList.Add(groupOptionButton);
			}
			UIElement uIElement4 = new UIElement
			{
				Width = StyleDimension.FromPixelsAndPercent(-20f, 1f),
				Height = StyleDimension.FromPixelsAndPercent(num + num2, 0f),
				HAlign = 0.5f,
				VAlign = 1f
			};
			uIElement4.SetPadding(0f);
			uIElement4.PaddingBottom = 12f;
			uIPanel.Append(uIElement4);
			AddDescriptionPanel(uIElement4, num, "desc");
		}

		private void AddDescriptionPanel(UIElement container, float accumulatedHeight, string tagGroup)
		{
			float num = 0f;
			UISlicedImage uISlicedImage = new UISlicedImage(Main.Assets.Request<Texture2D>("Images/UI/CharCreation/CategoryPanelHighlight"))
			{
				HAlign = 0.5f,
				VAlign = 1f,
				Width = StyleDimension.FromPixelsAndPercent((0f - num) * 2f, 1f),
				Left = StyleDimension.FromPixels(0f - num),
				Height = StyleDimension.FromPixelsAndPercent(accumulatedHeight, 0f),
				Top = StyleDimension.FromPixels(2f)
			};
			uISlicedImage.SetSliceDepths(10);
			uISlicedImage.Color = Color.LightGray * 0.7f;
			container.Append(uISlicedImage);
			UIText uIText = new UIText(Language.GetText("UI.WorldDescriptionDefault"), 0.7f)
			{
				HAlign = 0f,
				VAlign = 0f,
				Width = StyleDimension.FromPixelsAndPercent(0f, 1f),
				Height = StyleDimension.FromPixelsAndPercent(0f, 1f),
				Top = StyleDimension.FromPixelsAndPercent(2f, 0f)
			};
			uIText.IsWrapped = true;
			uIText.PaddingLeft = 20f;
			uIText.PaddingRight = 20f;
			uIText.PaddingTop = 4f;
			uISlicedImage.Append(uIText);
			_descriptionText = uIText;
		}

		public void MouseOutSeed(UIMouseEvent evt, UIElement listeningElement)
		{
			ClearOptionDescription(evt, listeningElement);
		}

		public void MouseOverSeed(UIMouseEvent evt, UIElement listeningElement)
		{
			if (evt.Target is GroupOptionButton<WorldGen.SecretSeed> groupOptionButton)
			{
				_ = groupOptionButton.IsSelected;
				if (Main.mouseLeft)
				{
					listeningElement.LeftMouseDown(evt);
				}
				ShowOptionDescription(evt, listeningElement);
			}
		}

		public void DrawGlowRing(UIElement listeningElement, SpriteBatch spriteBatch)
		{
			GroupOptionButton<WorldGen.SecretSeed> groupOptionButton = (GroupOptionButton<WorldGen.SecretSeed>)listeningElement.Parent;
			if (groupOptionButton.OptionValue.Enabled)
			{
				Asset<Texture2D> asset = Main.Assets.Request<Texture2D>("Images/UI/WorldCreation/IconRandomSeed");
				CalculatedStyle dimensions = groupOptionButton.GetDimensions();
				Vector2 position = dimensions.ToRectangle().TopRight() + new Vector2(-22f, 22f);
				Texture2D value = asset.Value;
				Rectangle r = new Rectangle(0, 0, 4, 4);
				Vector2 vector = new Vector2((float)value.Width * 0.45f, (float)value.Height * 0.95f);
				float num = 0.25f * (float)Math.Sin(Main.GlobalTimeWrappedHourly * 1.3f + dimensions.Position().Y);
				num = (float)Math.PI / 4f;
				vector = r.Size() / 2f;
				float num2 = 1.5f;
				_ = num2 + 1f;
				Math.Sin(Main.GlobalTimeWrappedHourly * 1.3f + dimensions.Position().Y * 0.00153178f);
				num2 = 1f;
				num = 0f;
				r = value.Frame();
				vector = r.Size() / 2f;
				spriteBatch.Draw(value, position, r, Color.White, num, vector, num2, SpriteEffects.None, 0f);
			}
		}

		private void CustomSort(List<UIElement> items)
		{
			items.Sort(delegate(UIElement a, UIElement b)
			{
				GroupOptionButton<WorldGen.SecretSeed> groupOptionButton = a as GroupOptionButton<WorldGen.SecretSeed>;
				GroupOptionButton<WorldGen.SecretSeed> groupOptionButton2 = b as GroupOptionButton<WorldGen.SecretSeed>;
				if (groupOptionButton != null && groupOptionButton2 == null)
				{
					return -1;
				}
				return (groupOptionButton == null && groupOptionButton2 != null) ? 1 : groupOptionButton.OptionValue.TextThatWasUsedToUnlock.CompareTo(groupOptionButton2.OptionValue.TextThatWasUsedToUnlock);
			});
		}

		private void ClickSecretSeed(UIMouseEvent evt, UIElement listeningElement)
		{
			GroupOptionButton<WorldGen.SecretSeed> groupOptionButton = (GroupOptionButton<WorldGen.SecretSeed>)listeningElement;
			WorldGen.SecretSeed optionValue = groupOptionButton.OptionValue;
			if (optionValue.Enabled)
			{
				groupOptionButton.SetCurrentOption(null);
				WorldGen.SecretSeed.Disable(optionValue);
				_creationState2.RemoveSeedFromSeedMenu(optionValue.TextThatWasUsedToUnlock);
			}
			else
			{
				groupOptionButton.SetCurrentOption(optionValue);
				WorldGen.SecretSeed.Enable(optionValue);
				_creationState2.AddSeedFromSeedmenu(optionValue.TextThatWasUsedToUnlock);
				SpawnParticles(groupOptionButton);
			}
		}

		private void SpawnParticles(GroupOptionButton<WorldGen.SecretSeed> element)
		{
			CalculatedStyle dimensions = element.GetDimensions();
			dimensions.Center();
			Spawn_RainbowRodHit(new ParticleOrchestraSettings
			{
				PositionInWorld = dimensions.Position() + new Vector2(dimensions.Width - 20f, dimensions.Height / 2f),
				MovementVector = new Vector2(0f, 16f) + Main.rand.NextVector2Circular(10f, 2f)
			});
			float num = 8f;
			for (int i = 0; (float)i < num + 1f; i++)
			{
				Spawn_BestReforge(new ParticleOrchestraSettings
				{
					PositionInWorld = dimensions.Position() + new Vector2(0f, dimensions.Height / 2f) + new Vector2(dimensions.Width * (1f / num) * (float)i, 0f)
				});
			}
		}

		private void Spawn_RainbowRodHit(ParticleOrchestraSettings settings)
		{
			float num = Main.rand.NextFloat() * ((float)Math.PI * 2f);
			float num2 = 6f;
			float num3 = Main.rand.NextFloat();
			for (float num4 = 0f; num4 < 1f; num4 += 1f / num2)
			{
				Vector2 vector = settings.MovementVector * Main.rand.NextFloatDirection() * 0.15f;
				Vector2 vector2 = new Vector2(Main.rand.NextFloat() * 0.4f + 0.4f);
				float f = num + Main.rand.NextFloat() * ((float)Math.PI * 2f);
				float rotation = (float)Math.PI / 2f;
				Vector2 vector3 = 1.5f * vector2;
				float num5 = 60f;
				Vector2 vector4 = Main.rand.NextVector2Circular(8f, 8f) * vector2;
				PrettySparkleParticle prettySparkleParticle = new PrettySparkleParticle();
				prettySparkleParticle.Velocity = f.ToRotationVector2() * vector3 + vector;
				prettySparkleParticle.AccelerationPerFrame = f.ToRotationVector2() * -(vector3 / num5) - vector * 1f / 60f;
				prettySparkleParticle.ColorTint = Main.hslToRgb((num3 + Main.rand.NextFloat() * 0.33f) % 1f, 1f, 0.4f + Main.rand.NextFloat() * 0.25f);
				prettySparkleParticle.ColorTint.A = 0;
				prettySparkleParticle.LocalPosition = settings.PositionInWorld + vector4;
				prettySparkleParticle.Rotation = rotation;
				prettySparkleParticle.Scale = vector2;
				SeedParticleSystem.Add(prettySparkleParticle);
				prettySparkleParticle = new PrettySparkleParticle();
				prettySparkleParticle.Velocity = f.ToRotationVector2() * vector3 + vector;
				prettySparkleParticle.AccelerationPerFrame = f.ToRotationVector2() * -(vector3 / num5) - vector * 1f / 60f;
				prettySparkleParticle.ColorTint = new Color(255, 255, 255, 0);
				prettySparkleParticle.LocalPosition = settings.PositionInWorld + vector4;
				prettySparkleParticle.Rotation = rotation;
				prettySparkleParticle.Scale = vector2 * 0.6f;
				SeedParticleSystem.Add(prettySparkleParticle);
			}
			for (int i = 0; i < 12; i++)
			{
				Color newColor = Main.hslToRgb((num3 + Main.rand.NextFloat() * 0.12f) % 1f, 1f, 0.4f + Main.rand.NextFloat() * 0.25f);
				Dust dust = SeedDust.NewDust(settings.PositionInWorld, 0, 0, 267, 0f, 0f, 0, newColor);
				dust.velocity = Main.rand.NextVector2Circular(1f, 1f);
				dust.velocity += settings.MovementVector * Main.rand.NextFloatDirection() * 0.5f;
				dust.noGravity = true;
				dust.scale = 0.6f + Main.rand.NextFloat() * 0.9f;
				dust.fadeIn = 0.7f + Main.rand.NextFloat() * 0.8f;
				if (dust.dustIndex != 200 && dust.type != 0)
				{
					Dust dust2 = SeedDust.CloneDust(dust);
					dust2.scale /= 2f;
					dust2.fadeIn *= 0.75f;
					dust2.color = new Color(255, 255, 255, 255);
				}
			}
		}

		private void Spawn_BestReforge(ParticleOrchestraSettings settings)
		{
			Vector2 accelerationPerFrame = new Vector2(0f, 0.16350001f);
			Asset<Texture2D> textureAsset = Main.Assets.Request<Texture2D>("Images/UI/Creative/Research_Spark");
			for (int i = 0; i < 2; i++)
			{
				Vector2 vector = Main.rand.NextVector2Circular(3f, 4f);
				Vector2 vector2 = new Vector2(0f, Main.rand.NextFloatDirection() * 20f);
				SeedParticleSystem.Add(new CreativeSacrificeParticle(textureAsset, null, settings.MovementVector + vector, settings.PositionInWorld + vector2)
				{
					AccelerationPerFrame = accelerationPerFrame,
					ScaleOffsetPerFrame = -1f / 60f
				});
			}
			float num = Main.rand.NextFloat();
			for (int j = 0; j < 3; j++)
			{
				Color newColor = Main.hslToRgb((num + Main.rand.NextFloat() * 0.12f) % 1f, 1f, 0.4f + Main.rand.NextFloat() * 0.25f);
				Dust dust = SeedDust.NewDust(settings.PositionInWorld, 0, 0, 267, 0f, 0f, 0, newColor);
				dust.velocity = Main.rand.NextVector2Circular(1f, 1f);
				dust.velocity += settings.MovementVector * Main.rand.NextFloatDirection() * 0.5f;
				dust.noGravity = true;
				dust.scale = 0.6f + Main.rand.NextFloat() * 0.9f;
				dust.fadeIn = 0.7f + Main.rand.NextFloat() * 0.8f;
				Vector2 vector3 = new Vector2(0f, Main.rand.NextFloatDirection() * 20f);
				dust.position += vector3;
				if (dust.dustIndex != 200 && dust.type != 0)
				{
					Dust dust2 = SeedDust.CloneDust(dust);
					dust2.scale /= 2f;
					dust2.fadeIn *= 0.75f;
					dust2.color = new Color(255, 255, 255, 255);
				}
			}
		}

		public override void Recalculate()
		{
			if (_scrollbar != null)
			{
				if (_isScrollbarAttached && !_scrollbar.CanScroll)
				{
					_containerPanel.RemoveChild(_scrollbar);
					_isScrollbarAttached = false;
					_worldList.Width.Set(0f, 1f);
				}
				else if (!_isScrollbarAttached && _scrollbar.CanScroll)
				{
					_containerPanel.Append(_scrollbar);
					_isScrollbarAttached = true;
					_worldList.Width.Set(-25f, 1f);
				}
			}
			base.Recalculate();
		}

		private void MakeBackAndCreatebuttons(UIElement outerContainer)
		{
			UITextPanel<LocalizedText> uITextPanel = new UITextPanel<LocalizedText>(Language.GetText("UI.Apply"), 0.65f, large: true)
			{
				Width = StyleDimension.FromPixelsAndPercent(-10f, 0.5f),
				Height = StyleDimension.FromPixels(50f),
				VAlign = 1f,
				HAlign = 0.5f,
				Top = StyleDimension.FromPixels(-43f)
			};
			uITextPanel.OnMouseOver += FadedMouseOver;
			uITextPanel.OnMouseOut += FadedMouseOut;
			uITextPanel.OnLeftMouseDown += Click_GoBack;
			uITextPanel.SetSnapPoint("Back", 0);
			outerContainer.Append(uITextPanel);
			_backButton = uITextPanel;
		}

		private void Click_GoBack(UIMouseEvent evt, UIElement listeningElement)
		{
			GoBack();
		}

		private void GoBack()
		{
			SoundEngine.PlaySound(11);
			Main.MenuUI.SetState(_creationState);
			_creationState.RefreshSecretSeedButton();
		}

		private void FadedMouseOver(UIMouseEvent evt, UIElement listeningElement)
		{
			SoundEngine.PlaySound(12);
			((UIPanel)evt.Target).BackgroundColor = new Color(73, 94, 171);
			((UIPanel)evt.Target).BorderColor = Colors.FancyUIFatButtonMouseOver;
			ShowOptionDescription(evt, listeningElement);
		}

		private void FadedMouseOut(UIMouseEvent evt, UIElement listeningElement)
		{
			((UIPanel)evt.Target).BackgroundColor = new Color(63, 82, 151) * 0.8f;
			((UIPanel)evt.Target).BorderColor = Color.Black;
			ClearOptionDescription(evt, listeningElement);
		}

		public void HandleBackButtonUsage()
		{
			GoBack();
		}

		public void ClearOptionDescription(UIMouseEvent evt, UIElement listeningElement)
		{
			_descriptionText.SetText(Language.GetText("UI.WorldDescriptionDefault"));
		}

		public void ShowOptionDescription(UIMouseEvent evt, UIElement listeningElement)
		{
			LocalizedText localizedText = null;
			if (listeningElement is GroupOptionButton<WorldGen.SecretSeed> groupOptionButton)
			{
				localizedText = groupOptionButton.Description;
			}
			if (localizedText != null)
			{
				_descriptionText.SetText(localizedText);
			}
		}

		public override void Draw(SpriteBatch spriteBatch)
		{
			base.Draw(spriteBatch);
			SetupGamepadPoints(spriteBatch);
			DrawSeedSystems(spriteBatch);
		}

		public void DrawSeedSystems(SpriteBatch spriteBatch)
		{
			SeedDust.UpdateDust();
			SeedDust.DrawDust();
			SeedParticleSystem.Update();
			SeedParticleSystem.Draw(spriteBatch);
		}

		private void SetupGamepadPoints(SpriteBatch spriteBatch)
		{
			UILinkPointNavigator.Shortcuts.BackButtonCommand = 7;
			int num = 3000;
			int currentID = num;
			GetSnapPoints();
			UILinkPoint linkPoint = _helper.GetLinkPoint(currentID++, _backButton);
			List<SnapPoint> snapPoints = _worldList.GetSnapPoints();
			UILinkPoint[,] array = _helper.CreateUILinkPointGrid(ref currentID, snapPoints, 1, null, null, null, linkPoint);
			UILinkPoint upSide = array[0, array.GetLength(1) - 1];
			_helper.PairUpDown(upSide, linkPoint);
			_helper.MoveToVisuallyClosestPoint(num, currentID);
		}
	}
}
