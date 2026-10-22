using System;
using System.Collections.Generic;
using System.Linq;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;
using ReLogic.Graphics;
using Terraria.Audio;
using Terraria.GameContent.Drawing;
using Terraria.GameContent.UI.Elements;
using Terraria.Graphics;
using Terraria.Graphics.Renderers;
using Terraria.Graphics.Shaders;
using Terraria.ID;
using Terraria.IO;
using Terraria.Localization;
using Terraria.Social;
using Terraria.Testing;
using Terraria.UI;
using Terraria.UI.Gamepad;
using Terraria.WorldBuilding;

namespace Terraria.GameContent.UI.States
{
	public class UIWorldCreation : UIState, IHaveBackButtonCommand
	{
		private enum WorldSizeId
		{
			Small,
			Medium,
			Large
		}

		private enum WorldDifficultyId
		{
			Normal,
			Expert,
			Master,
			Creative
		}

		private enum WorldEvilId
		{
			Random,
			Corruption,
			Crimson
		}

		public delegate void SubmitSeedEvent();

		private string _optionwWorldName;

		private string _optionSeed;

		private bool _isSpecialSeedText;

		private List<string> _secretSeedTextsEntered = new List<string>();

		private List<string> _disabledSecretSeedTextsEntered = new List<string>();

		private ParticleRenderer SeedParticleSystem = new ParticleRenderer();

		private UIDust SeedDust = new UIDust();

		private GroupOptionButton<bool> _advancedSeedButton;

		private UICharacterNameButton _namePlate;

		private UICharacterNameButton _seedPlate;

		private UIWorldCreationPreview _previewPlate;

		private GroupOptionButton<WorldSizeId>[] _sizeButtons;

		private GroupOptionButton<WorldDifficultyId>[] _difficultyButtons;

		private GroupOptionButton<WorldEvilId>[] _evilButtons;

		private UIText _descriptionText;

		public const int MAX_NAME_LENGTH = 27;

		private UIState _goBackTarget;

		public SubmitSeedEvent SubmitSeed;

		private float ringPoint;

		private const int numSteps = 61;

		private Vector2[] oldPos = new Vector2[61];

		private Vector2[] oldTangent = new Vector2[61];

		private int specialSeedIndex;

		private static VertexStrip _vertexStrip = new VertexStrip();

		private float opacity = 0.6f;

		private float opacity2 = 0.5f;

		private float trial;

		private float animationSpeed = 0.5f;

		private float saturation = 0.5f;

		private WorldSizeId _optionSize
		{
			get
			{
				return (WorldSizeId)WorldGen.GetWorldSize();
			}
			set
			{
				WorldGen.SetWorldSize((int)value);
			}
		}

		private WorldDifficultyId _optionDifficulty
		{
			get
			{
				return (WorldDifficultyId)Main.GameMode;
			}
			set
			{
				Main.GameMode = (int)value;
			}
		}

		private WorldEvilId _optionEvil
		{
			get
			{
				return (WorldEvilId)(WorldGen.WorldGenParam_Evil + 1);
			}
			set
			{
				WorldGen.WorldGenParam_Evil = (int)(value - 1);
			}
		}

		public bool HasEnteredSpecialSeed => _secretSeedTextsEntered.Count > 0;

		public bool HasDisabledSecretSeed => _disabledSecretSeedTextsEntered.Count > 0;

		public UIWorldCreation()
		{
			_goBackTarget = this;
			BuildPage();
			SeedDust.Clear();
			SeedParticleSystem.Clear();
			ResetSpecialSeedRing();
		}

		public void SetGoBackTarget(UIState state)
		{
			_goBackTarget = state;
		}

		private void BuildPage()
		{
			int num = 18;
			RemoveAllChildren();
			UIElement uIElement = new UIElement
			{
				Width = StyleDimension.FromPixels(500f),
				Height = StyleDimension.FromPixels(434f + (float)num),
				Top = StyleDimension.FromPixels(170f - (float)num),
				HAlign = 0.5f,
				VAlign = 0f
			};
			uIElement.SetPadding(0f);
			Append(uIElement);
			UIPanel uIPanel = new UIPanel
			{
				Width = StyleDimension.FromPercent(1f),
				Height = StyleDimension.FromPixels(280 + num),
				Top = StyleDimension.FromPixels(50f),
				BackgroundColor = new Color(33, 43, 79) * 0.8f
			};
			uIPanel.SetPadding(0f);
			uIElement.Append(uIPanel);
			MakeBackAndCreatebuttons(uIElement);
			UIElement uIElement2 = new UIElement
			{
				Top = StyleDimension.FromPixelsAndPercent(0f, 0f),
				Width = StyleDimension.FromPixelsAndPercent(0f, 1f),
				Height = StyleDimension.FromPixelsAndPercent(0f, 1f),
				HAlign = 1f
			};
			uIElement2.SetPadding(0f);
			uIElement2.PaddingTop = 8f;
			uIElement2.PaddingBottom = 12f;
			uIPanel.Append(uIElement2);
			MakeInfoMenu(uIElement2);
		}

		private void PreparePreviouslyUnlockedSecretSeeds()
		{
			SecretSeedsTracker.PrepareInterface();
		}

		private void MakeInfoMenu(UIElement parentContainer)
		{
			UIElement uIElement = new UIElement
			{
				Width = StyleDimension.FromPixelsAndPercent(0f, 1f),
				Height = StyleDimension.FromPixelsAndPercent(0f, 1f),
				HAlign = 0.5f,
				VAlign = 0f
			};
			uIElement.SetPadding(10f);
			uIElement.PaddingBottom = 0f;
			uIElement.PaddingTop = 0f;
			parentContainer.Append(uIElement);
			float num = 0f;
			float num2 = 44f;
			float num3 = 88f + num2;
			float pixels = num2;
			GroupOptionButton<bool> groupOptionButton = new GroupOptionButton<bool>(option: true, null, Language.GetText("UI.WorldCreationRandomizeNameDescription"), Color.White, "Images/UI/WorldCreation/IconRandomName")
			{
				Width = StyleDimension.FromPixelsAndPercent(40f, 0f),
				Height = new StyleDimension(40f, 0f),
				HAlign = 0f,
				Top = StyleDimension.FromPixelsAndPercent(num, 0f),
				ShowHighlightWhenSelected = false
			};
			groupOptionButton.OnLeftMouseDown += ClickRandomizeName;
			groupOptionButton.OnMouseOver += ShowOptionDescription;
			groupOptionButton.OnMouseOut += ClearOptionDescription;
			groupOptionButton.SetSnapPoint("RandomizeName", 0);
			uIElement.Append(groupOptionButton);
			UICharacterNameButton uICharacterNameButton = new UICharacterNameButton(Language.GetText("UI.WorldCreationName"), Language.GetText("UI.WorldCreationNameEmpty"), Language.GetText("UI.WorldDescriptionName"))
			{
				Width = StyleDimension.FromPixelsAndPercent(0f - num3, 1f),
				HAlign = 0f,
				Left = new StyleDimension(pixels, 0f),
				Top = StyleDimension.FromPixelsAndPercent(num, 0f)
			};
			uICharacterNameButton.OnLeftMouseDown += Click_SetName;
			uICharacterNameButton.OnMouseOver += ShowOptionDescription;
			uICharacterNameButton.OnMouseOut += ClearOptionDescription;
			uICharacterNameButton.SetSnapPoint("Name", 0);
			uIElement.Append(uICharacterNameButton);
			_namePlate = uICharacterNameButton;
			num += uICharacterNameButton.GetDimensions().Height + 4f;
			_advancedSeedButton = new GroupOptionButton<bool>(option: true, null, Language.GetText("UI.WorldCreationSeedMenuDescription"), Color.White, "Images/UI/WorldCreation/IconRandomSeed")
			{
				Width = StyleDimension.FromPixelsAndPercent(40f, 0f),
				Height = new StyleDimension(40f, 0f),
				HAlign = 0f,
				Top = StyleDimension.FromPixelsAndPercent(num, 0f),
				ShowHighlightWhenSelected = false
			};
			_advancedSeedButton.OnLeftMouseDown += ClickAdvancedSeedMenu;
			_advancedSeedButton.OnMouseOver += ShowOptionDescription;
			_advancedSeedButton.OnMouseOut += ClearOptionDescription;
			_advancedSeedButton.SetSnapPoint("RandomizeSeed", 0);
			_advancedSeedButton.OnDraw += DrawSpecialSeedRingCallback;
			uIElement.Append(_advancedSeedButton);
			UICharacterNameButton uICharacterNameButton2 = new UICharacterNameButton(Language.GetText("UI.WorldCreationSeed"), Language.GetText("UI.WorldCreationSeedEmpty"), Language.GetText("UI.WorldDescriptionSeed"))
			{
				Width = StyleDimension.FromPixelsAndPercent(0f - num3, 1f),
				HAlign = 0f,
				Left = new StyleDimension(pixels, 0f),
				Top = StyleDimension.FromPixelsAndPercent(num, 0f),
				DistanceFromTitleToOption = 29f
			};
			uICharacterNameButton2.OnLeftMouseDown += Click_SetSeed;
			uICharacterNameButton2.OnMouseOver += ShowOptionDescription;
			uICharacterNameButton2.OnMouseOut += ClearOptionDescription;
			uICharacterNameButton2.SetSnapPoint("Seed", 0);
			uIElement.Append(uICharacterNameButton2);
			_seedPlate = uICharacterNameButton2;
			UIWorldCreationPreview uIWorldCreationPreview = new UIWorldCreationPreview
			{
				Width = StyleDimension.FromPixels(84f),
				Height = StyleDimension.FromPixels(84f),
				HAlign = 1f,
				VAlign = 0f
			};
			uIElement.Append(uIWorldCreationPreview);
			_previewPlate = uIWorldCreationPreview;
			num += uICharacterNameButton2.GetDimensions().Height + 10f;
			AddHorizontalSeparator(uIElement, num + 2f);
			float usableWidthPercent = 1f;
			AddWorldSizeOptions(uIElement, num, ClickSizeOption, "size", usableWidthPercent);
			num += 48f;
			AddHorizontalSeparator(uIElement, num);
			AddWorldDifficultyOptions(uIElement, num, ClickDifficultyOption, "difficulty", usableWidthPercent);
			num += 48f;
			AddHorizontalSeparator(uIElement, num);
			AddWorldEvilOptions(uIElement, num, ClickEvilOption, "evil", usableWidthPercent);
			num += 48f;
			AddHorizontalSeparator(uIElement, num);
			AddDescriptionPanel(uIElement, num, "desc");
			SetDefaultOptions();
		}

		private static void AddHorizontalSeparator(UIElement Container, float accumualtedHeight)
		{
			UIHorizontalSeparator element = new UIHorizontalSeparator
			{
				Width = StyleDimension.FromPercent(1f),
				Top = StyleDimension.FromPixels(accumualtedHeight - 8f),
				Color = Color.Lerp(Color.White, new Color(63, 65, 151, 255), 0.85f) * 0.9f
			};
			Container.Append(element);
		}

		private void SetDefaultOptions()
		{
			Main.ActiveWorldFileData = new WorldFileData();
			AssignRandomWorldName();
			ClearSeed();
			_optionSize = WorldSizeId.Medium;
			if (Main.ActivePlayerFileData.Player.difficulty == 3)
			{
				_optionDifficulty = WorldDifficultyId.Creative;
			}
			_optionEvil = WorldEvilId.Random;
			UpdateSliders();
			UpdatePreviewPlate();
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
				Height = StyleDimension.FromPixelsAndPercent(40f, 0f),
				Top = StyleDimension.FromPixels(2f)
			};
			uISlicedImage.SetSliceDepths(10);
			uISlicedImage.Color = Color.LightGray * 0.7f;
			container.Append(uISlicedImage);
			UIText uIText = new UIText(Language.GetText("UI.WorldDescriptionDefault"), 0.82f)
			{
				HAlign = 0f,
				VAlign = 0f,
				Width = StyleDimension.FromPixelsAndPercent(0f, 1f),
				Height = StyleDimension.FromPixelsAndPercent(0f, 1f),
				Top = StyleDimension.FromPixelsAndPercent(5f, 0f)
			};
			uIText.PaddingLeft = 20f;
			uIText.PaddingRight = 20f;
			uIText.PaddingTop = 6f;
			uISlicedImage.Append(uIText);
			_descriptionText = uIText;
		}

		private void AddWorldSizeOptions(UIElement container, float accumualtedHeight, MouseEvent clickEvent, string tagGroup, float usableWidthPercent)
		{
			WorldSizeId[] array = new WorldSizeId[3]
			{
				WorldSizeId.Small,
				WorldSizeId.Medium,
				WorldSizeId.Large
			};
			LocalizedText[] array2 = new LocalizedText[3]
			{
				Lang.menu[92],
				Lang.menu[93],
				Lang.menu[94]
			};
			LocalizedText[] array3 = new LocalizedText[3]
			{
				Language.GetText("UI.WorldDescriptionSizeSmall"),
				Language.GetText("UI.WorldDescriptionSizeMedium"),
				Language.GetText("UI.WorldDescriptionSizeLarge")
			};
			Color[] array4 = new Color[3]
			{
				Color.Cyan,
				Color.Lerp(Color.Cyan, Color.LimeGreen, 0.5f),
				Color.LimeGreen
			};
			string[] array5 = new string[3] { "Images/UI/WorldCreation/IconSizeSmall", "Images/UI/WorldCreation/IconSizeMedium", "Images/UI/WorldCreation/IconSizeLarge" };
			GroupOptionButton<WorldSizeId>[] array6 = new GroupOptionButton<WorldSizeId>[array.Length];
			for (int i = 0; i < array6.Length; i++)
			{
				GroupOptionButton<WorldSizeId> groupOptionButton = new GroupOptionButton<WorldSizeId>(array[i], array2[i], array3[i], array4[i], array5[i], 1f, 1f, 16f);
				groupOptionButton.Width = StyleDimension.FromPixelsAndPercent(-4 * (array6.Length - 1), 1f / (float)array6.Length * usableWidthPercent);
				groupOptionButton.Left = StyleDimension.FromPercent(1f - usableWidthPercent);
				groupOptionButton.HAlign = (float)i / (float)(array6.Length - 1);
				groupOptionButton.Top.Set(accumualtedHeight, 0f);
				groupOptionButton.OnLeftMouseDown += clickEvent;
				groupOptionButton.OnMouseOver += ShowOptionDescription;
				groupOptionButton.OnMouseOut += ClearOptionDescription;
				groupOptionButton.SetSnapPoint(tagGroup, i);
				container.Append(groupOptionButton);
				array6[i] = groupOptionButton;
			}
			_sizeButtons = array6;
		}

		private void AddWorldDifficultyOptions(UIElement container, float accumualtedHeight, MouseEvent clickEvent, string tagGroup, float usableWidthPercent)
		{
			WorldDifficultyId[] array = new WorldDifficultyId[4]
			{
				WorldDifficultyId.Creative,
				WorldDifficultyId.Normal,
				WorldDifficultyId.Expert,
				WorldDifficultyId.Master
			};
			LocalizedText[] array2 = new LocalizedText[4]
			{
				Language.GetText("UI.Creative"),
				Language.GetText("UI.Normal"),
				Language.GetText("UI.Expert"),
				Language.GetText("UI.Master")
			};
			LocalizedText[] array3 = new LocalizedText[4]
			{
				Language.GetText("UI.WorldDescriptionCreative"),
				Language.GetText("UI.WorldDescriptionNormal"),
				Language.GetText("UI.WorldDescriptionExpert"),
				Language.GetText("UI.WorldDescriptionMaster")
			};
			Color[] array4 = new Color[4]
			{
				Main.creativeModeColor,
				Color.White,
				Main.mcColor,
				Main.hcColor
			};
			string[] array5 = new string[4] { "Images/UI/WorldCreation/IconDifficultyCreative", "Images/UI/WorldCreation/IconDifficultyNormal", "Images/UI/WorldCreation/IconDifficultyExpert", "Images/UI/WorldCreation/IconDifficultyMaster" };
			GroupOptionButton<WorldDifficultyId>[] array6 = new GroupOptionButton<WorldDifficultyId>[array.Length];
			for (int i = 0; i < array6.Length; i++)
			{
				GroupOptionButton<WorldDifficultyId> groupOptionButton = new GroupOptionButton<WorldDifficultyId>(array[i], array2[i], array3[i], array4[i], array5[i], 1f, 1f, 16f);
				groupOptionButton.Width = StyleDimension.FromPixelsAndPercent(-1 * (array6.Length - 1), 1f / (float)array6.Length * usableWidthPercent);
				groupOptionButton.Left = StyleDimension.FromPercent(1f - usableWidthPercent);
				groupOptionButton.HAlign = (float)i / (float)(array6.Length - 1);
				groupOptionButton.Top.Set(accumualtedHeight, 0f);
				groupOptionButton.OnLeftMouseDown += clickEvent;
				groupOptionButton.OnMouseOver += ShowOptionDescription;
				groupOptionButton.OnMouseOut += ClearOptionDescription;
				groupOptionButton.SetSnapPoint(tagGroup, i);
				container.Append(groupOptionButton);
				array6[i] = groupOptionButton;
			}
			_difficultyButtons = array6;
		}

		private void AddWorldEvilOptions(UIElement container, float accumualtedHeight, MouseEvent clickEvent, string tagGroup, float usableWidthPercent)
		{
			WorldEvilId[] array = new WorldEvilId[3]
			{
				WorldEvilId.Random,
				WorldEvilId.Corruption,
				WorldEvilId.Crimson
			};
			LocalizedText[] array2 = new LocalizedText[3]
			{
				Lang.misc[103],
				Lang.misc[101],
				Lang.misc[102]
			};
			LocalizedText[] array3 = new LocalizedText[3]
			{
				Language.GetText("UI.WorldDescriptionEvilRandom"),
				Language.GetText("UI.WorldDescriptionEvilCorrupt"),
				Language.GetText("UI.WorldDescriptionEvilCrimson")
			};
			Color[] array4 = new Color[3]
			{
				Color.White,
				Color.MediumPurple,
				Color.IndianRed
			};
			string[] array5 = new string[3] { "Images/UI/WorldCreation/IconEvilRandom", "Images/UI/WorldCreation/IconEvilCorruption", "Images/UI/WorldCreation/IconEvilCrimson" };
			GroupOptionButton<WorldEvilId>[] array6 = new GroupOptionButton<WorldEvilId>[array.Length];
			for (int i = 0; i < array6.Length; i++)
			{
				GroupOptionButton<WorldEvilId> groupOptionButton = new GroupOptionButton<WorldEvilId>(array[i], array2[i], array3[i], array4[i], array5[i], 1f, 1f, 16f);
				groupOptionButton.Width = StyleDimension.FromPixelsAndPercent(-4 * (array6.Length - 1), 1f / (float)array6.Length * usableWidthPercent);
				groupOptionButton.Left = StyleDimension.FromPercent(1f - usableWidthPercent);
				groupOptionButton.HAlign = (float)i / (float)(array6.Length - 1);
				groupOptionButton.Top.Set(accumualtedHeight, 0f);
				groupOptionButton.OnLeftMouseDown += clickEvent;
				groupOptionButton.OnMouseOver += ShowOptionDescription;
				groupOptionButton.OnMouseOut += ClearOptionDescription;
				groupOptionButton.SetSnapPoint(tagGroup, i);
				container.Append(groupOptionButton);
				array6[i] = groupOptionButton;
			}
			_evilButtons = array6;
		}

		private void ClickRandomizeName(UIMouseEvent evt, UIElement listeningElement)
		{
			AssignRandomWorldName();
			UpdateInputFields();
			UpdateSliders();
			UpdatePreviewPlate();
		}

		private void ClickAdvancedSeedMenu(UIMouseEvent evt, UIElement listeningElement)
		{
			ResetSpecialSeedRing();
			UIWorldCreationAdvanced uIWorldCreationAdvanced = new UIWorldCreationAdvanced(this);
			SetGoBackTarget(uIWorldCreationAdvanced);
			Main.MenuUI.SetState(uIWorldCreationAdvanced);
		}

		public void ClearSeedText()
		{
			_optionSeed = "";
			_isSpecialSeedText = false;
			UpdateInputFields();
		}

		public void ClearSeed()
		{
			_optionSeed = string.Empty;
			_isSpecialSeedText = false;
			_secretSeedTextsEntered.Clear();
			_disabledSecretSeedTextsEntered.Clear();
			WorldGenerationOptions.Reset();
			WorldGen.SecretSeed.ClearAllSeeds();
			PreparePreviouslyUnlockedSecretSeeds();
			UpdateInputFields();
		}

		public void RandomizeSeed()
		{
			_optionSeed = Main.rand.Next().ToString();
			_isSpecialSeedText = false;
			UpdateInputFields();
			UpdateSliders();
			UpdatePreviewPlate();
		}

		private void ClickSizeOption(UIMouseEvent evt, UIElement listeningElement)
		{
			GroupOptionButton<WorldSizeId> groupOptionButton = (GroupOptionButton<WorldSizeId>)listeningElement;
			_optionSize = groupOptionButton.OptionValue;
			GroupOptionButton<WorldSizeId>[] sizeButtons = _sizeButtons;
			for (int i = 0; i < sizeButtons.Length; i++)
			{
				sizeButtons[i].SetCurrentOption(groupOptionButton.OptionValue);
			}
			UpdatePreviewPlate();
		}

		private void ClickDifficultyOption(UIMouseEvent evt, UIElement listeningElement)
		{
			GroupOptionButton<WorldDifficultyId> groupOptionButton = (GroupOptionButton<WorldDifficultyId>)listeningElement;
			_optionDifficulty = groupOptionButton.OptionValue;
			GroupOptionButton<WorldDifficultyId>[] difficultyButtons = _difficultyButtons;
			for (int i = 0; i < difficultyButtons.Length; i++)
			{
				difficultyButtons[i].SetCurrentOption(groupOptionButton.OptionValue);
			}
			UpdatePreviewPlate();
		}

		private void ClickEvilOption(UIMouseEvent evt, UIElement listeningElement)
		{
			GroupOptionButton<WorldEvilId> groupOptionButton = (GroupOptionButton<WorldEvilId>)listeningElement;
			_optionEvil = groupOptionButton.OptionValue;
			GroupOptionButton<WorldEvilId>[] evilButtons = _evilButtons;
			for (int i = 0; i < evilButtons.Length; i++)
			{
				evilButtons[i].SetCurrentOption(groupOptionButton.OptionValue);
			}
			UpdatePreviewPlate();
		}

		private void UpdatePreviewPlate()
		{
			_previewPlate.UpdateOption((byte)_optionDifficulty, (byte)_optionEvil, (byte)_optionSize);
		}

		private void UpdateSliders()
		{
			GroupOptionButton<WorldSizeId>[] sizeButtons = _sizeButtons;
			for (int i = 0; i < sizeButtons.Length; i++)
			{
				sizeButtons[i].SetCurrentOption(_optionSize);
			}
			GroupOptionButton<WorldDifficultyId>[] difficultyButtons = _difficultyButtons;
			for (int i = 0; i < difficultyButtons.Length; i++)
			{
				difficultyButtons[i].SetCurrentOption(_optionDifficulty);
			}
			GroupOptionButton<WorldEvilId>[] evilButtons = _evilButtons;
			for (int i = 0; i < evilButtons.Length; i++)
			{
				evilButtons[i].SetCurrentOption(_optionEvil);
			}
		}

		public void ShowOptionDescription(UIMouseEvent evt, UIElement listeningElement)
		{
			LocalizedText localizedText = null;
			if (listeningElement is GroupOptionButton<WorldSizeId> groupOptionButton)
			{
				localizedText = groupOptionButton.Description;
			}
			if (listeningElement is GroupOptionButton<WorldDifficultyId> groupOptionButton2)
			{
				localizedText = groupOptionButton2.Description;
			}
			if (listeningElement is GroupOptionButton<WorldEvilId> groupOptionButton3)
			{
				localizedText = groupOptionButton3.Description;
			}
			if (listeningElement is UICharacterNameButton uICharacterNameButton)
			{
				localizedText = uICharacterNameButton.Description;
			}
			if (listeningElement is GroupOptionButton<bool> groupOptionButton4)
			{
				localizedText = groupOptionButton4.Description;
			}
			if (localizedText != null)
			{
				_descriptionText.SetText(localizedText);
			}
		}

		public void ClearOptionDescription(UIMouseEvent evt, UIElement listeningElement)
		{
			_descriptionText.SetText(Language.GetText("UI.WorldDescriptionDefault"));
		}

		private void MakeBackAndCreatebuttons(UIElement outerContainer)
		{
			UITextPanel<LocalizedText> uITextPanel = new UITextPanel<LocalizedText>(Language.GetText("UI.Back"), 0.7f, large: true)
			{
				Width = StyleDimension.FromPixelsAndPercent(-10f, 0.5f),
				Height = StyleDimension.FromPixels(50f),
				VAlign = 1f,
				HAlign = 0f,
				Top = StyleDimension.FromPixels(-45f)
			};
			uITextPanel.OnMouseOver += FadedMouseOver;
			uITextPanel.OnMouseOut += FadedMouseOut;
			uITextPanel.OnLeftMouseDown += Click_GoBack;
			uITextPanel.SetSnapPoint("Back", 0);
			outerContainer.Append(uITextPanel);
			UITextPanel<LocalizedText> uITextPanel2 = new UITextPanel<LocalizedText>(Language.GetText("UI.Create"), 0.7f, large: true)
			{
				Width = StyleDimension.FromPixelsAndPercent(-10f, 0.5f),
				Height = StyleDimension.FromPixels(50f),
				VAlign = 1f,
				HAlign = 1f,
				Top = StyleDimension.FromPixels(-45f)
			};
			uITextPanel2.OnMouseOver += FadedMouseOver;
			uITextPanel2.OnMouseOut += FadedMouseOut;
			uITextPanel2.OnLeftMouseDown += Click_NamingAndCreating;
			uITextPanel2.SetSnapPoint("Create", 0);
			outerContainer.Append(uITextPanel2);
		}

		private void Click_GoBack(UIMouseEvent evt, UIElement listeningElement)
		{
			GoBack();
		}

		private static void GoBack()
		{
			SoundEngine.PlaySound(11);
			Main.OpenWorldSelectUI();
		}

		private void FadedMouseOver(UIMouseEvent evt, UIElement listeningElement)
		{
			SoundEngine.PlaySound(12);
			((UIPanel)evt.Target).BackgroundColor = new Color(73, 94, 171);
			((UIPanel)evt.Target).BorderColor = Colors.FancyUIFatButtonMouseOver;
		}

		private void FadedMouseOut(UIMouseEvent evt, UIElement listeningElement)
		{
			((UIPanel)evt.Target).BackgroundColor = new Color(63, 82, 151) * 0.8f;
			((UIPanel)evt.Target).BorderColor = Color.Black;
		}

		private void Click_SetName(UIMouseEvent evt, UIElement listeningElement)
		{
			SoundEngine.PlaySound(10);
			Main.clrInput();
			UIVirtualKeyboard state = new UIVirtualKeyboard(Lang.menu[48].Value, "", OnFinishedSettingName, GoBackHere, 0, allowEmpty: true, 27);
			Main.MenuUI.SetState(state);
		}

		private void Click_SetSeed(UIMouseEvent evt, UIElement listeningElement)
		{
			OpenSeedInputMenu();
		}

		public void OpenSeedInputMenu()
		{
			SoundEngine.PlaySound(10);
			Main.clrInput();
			UIVirtualKeyboard state = new UIVirtualKeyboard(Language.GetTextValue("UI.EnterSeed"), "", OnFinishedSettingSeed, GoBackHere, 0, allowEmpty: true, int.MaxValue);
			Main.MenuUI.SetState(state);
		}

		private void Click_NamingAndCreating(UIMouseEvent evt, UIElement listeningElement)
		{
			SoundEngine.PlaySound(10);
			if (string.IsNullOrEmpty(_optionwWorldName))
			{
				_optionwWorldName = "";
				Main.clrInput();
				UIVirtualKeyboard state = new UIVirtualKeyboard(Lang.menu[48].Value, "", OnFinishedNamingAndCreating, GoBackHere, 0, allowEmpty: false, 27);
				Main.MenuUI.SetState(state);
			}
			else
			{
				FinishCreatingWorld();
			}
		}

		private void OnFinishedSettingName(string name)
		{
			_optionwWorldName = name.Trim();
			UpdateInputFields();
			GoBackHere();
		}

		private void UpdateInputFields()
		{
			_namePlate.SetContents(_optionwWorldName);
			_namePlate.Recalculate();
			_namePlate.TrimDisplayIfOverElementDimensions(27);
			_namePlate.Recalculate();
			FillSeedContent(_seedPlate);
		}

		public void FillSeedContent(UICharacterNameButton button)
		{
			button.SetContents(_optionSeed);
			button.Recalculate();
			button.TrimDisplayIfOverElementDimensions(WorldFileData.MAX_USER_SEED_TEXT_LENGTH);
			button.Recalculate();
		}

		public void ToggleSeedOption(AWorldGenerationOption seedOption)
		{
			if (_isSpecialSeedText)
			{
				_optionSeed = string.Empty;
				_isSpecialSeedText = false;
				UpdateInputFields();
				UpdateSliders();
				UpdatePreviewPlate();
			}
			seedOption.Enabled = !seedOption.Enabled;
		}

		public void EnableSecretSeedOptions(bool enabled)
		{
			if (enabled)
			{
				for (int i = 0; i < _disabledSecretSeedTextsEntered.Count; i++)
				{
					if (WorldGen.SecretSeed.CheckInputForSecretSeed(_disabledSecretSeedTextsEntered[i], out var secretSeed) && !secretSeed.Enabled)
					{
						_secretSeedTextsEntered.Add(_disabledSecretSeedTextsEntered[i]);
						WorldGen.SecretSeed.Enable(secretSeed, playSound: false);
					}
				}
				_disabledSecretSeedTextsEntered.Clear();
			}
			else
			{
				_disabledSecretSeedTextsEntered.Clear();
				_disabledSecretSeedTextsEntered.AddRange(_secretSeedTextsEntered);
				WorldGen.SecretSeed.ClearAllSeeds();
				_secretSeedTextsEntered.Clear();
			}
		}

		public string GetJoinedSecretSeedString(DynamicSpriteFont font, float maxWidth, float maxHeight)
		{
			float num = 0f;
			string text = string.Empty;
			List<string> list = (HasEnteredSpecialSeed ? _secretSeedTextsEntered : _disabledSecretSeedTextsEntered);
			if (list.Count == 0)
			{
				return "-";
			}
			string text2 = list[0];
			for (int i = 1; i < list.Count; i++)
			{
				string text3 = $"{text2}|{list[i]}";
				if (font.MeasureString(text3).X >= maxWidth)
				{
					if (num <= maxHeight)
					{
						text = text + text2 + "\n";
					}
					num += (float)font.LineSpacing;
					text3 = list[i];
				}
				text2 = text3;
			}
			if (num <= maxHeight)
			{
				text += text2;
			}
			return text;
		}

		private void OnFinishedSettingSeed(string seed)
		{
			_optionSeed = seed.Trim();
			if (WorldFileData.TryApplyingCopiedSeed(_optionSeed, playSound: true, out var processedSeed, out var _, out var secretSeedTexts))
			{
				_optionSeed = processedSeed;
				_secretSeedTextsEntered = secretSeedTexts;
				_disabledSecretSeedTextsEntered.Clear();
			}
			else
			{
				_optionSeed = Utils.TrimUserString(_optionSeed, WorldFileData.MAX_USER_SEED_TEXT_LENGTH);
				AWorldGenerationOption optionFromSeedText = WorldGenerationOptions.GetOptionFromSeedText(_optionSeed);
				_isSpecialSeedText = optionFromSeedText != null;
				if (_isSpecialSeedText)
				{
					WorldGenerationOptions.SelectOption(optionFromSeedText);
					SoundEngine.PlaySound(24);
				}
				if (WorldGen.SecretSeed.CheckInputForSecretSeed(_optionSeed, out var secretSeed))
				{
					if (!secretSeed.Enabled)
					{
						_secretSeedTextsEntered.Add(_optionSeed);
						WorldGen.SecretSeed.Enable(secretSeed);
						EnableSecretSeedOptions(enabled: true);
						CalculatedStyle dimensions = _advancedSeedButton.GetDimensions();
						if (_goBackTarget != this && _goBackTarget is UIWorldCreationAdvanced uIWorldCreationAdvanced)
						{
							uIWorldCreationAdvanced.RefreshSecretSeedButton();
							dimensions = uIWorldCreationAdvanced.GetSecretSeedButton().GetDimensions();
							uIWorldCreationAdvanced.GetSecretSeedButton().SetCurrentOption(HasEnteredSpecialSeed);
						}
						Vector2 vector = dimensions.Center();
						Vector2 vector2 = Main.rand.NextVector2Circular(5f, 5f);
						Spawn_RainbowRodHit(new ParticleOrchestraSettings
						{
							PositionInWorld = vector,
							MovementVector = new Vector2(16f, 0f) + vector2
						});
						if (_goBackTarget != this)
						{
							Spawn_RainbowRodHit(new ParticleOrchestraSettings
							{
								PositionInWorld = vector,
								MovementVector = new Vector2(16f, 0f) - vector2
							});
						}
						Vector2 vector3 = Main.rand.NextVector2Circular(5f, 5f);
						Spawn_RainbowRodHit(new ParticleOrchestraSettings
						{
							PositionInWorld = vector,
							MovementVector = new Vector2(0f, 16f) + vector3
						});
						if (_goBackTarget != this)
						{
							Spawn_RainbowRodHit(new ParticleOrchestraSettings
							{
								PositionInWorld = vector,
								MovementVector = new Vector2(0f, 16f) - vector3
							});
						}
						for (int i = 0; i < 3; i++)
						{
							Spawn_BestReforge(new ParticleOrchestraSettings
							{
								PositionInWorld = vector + new Vector2(dimensions.Width * 0.25f * (float)(i - 1), 0f)
							});
						}
					}
					ClearSeedText();
				}
			}
			UpdateInputFields();
			UpdateSliders();
			UpdatePreviewPlate();
			if (SubmitSeed != null)
			{
				SubmitSeed();
			}
			GoBackHere();
		}

		private void Spawn_BestReforge(ParticleOrchestraSettings settings)
		{
			Vector2 accelerationPerFrame = new Vector2(0f, 0.16350001f);
			Asset<Texture2D> textureAsset = Main.Assets.Request<Texture2D>("Images/UI/Creative/Research_Spark");
			for (int i = 0; i < 8; i++)
			{
				Vector2 vector = Main.rand.NextVector2Circular(3f, 4f);
				SeedParticleSystem.Add(new CreativeSacrificeParticle(textureAsset, null, settings.MovementVector + vector, settings.PositionInWorld)
				{
					AccelerationPerFrame = accelerationPerFrame,
					ScaleOffsetPerFrame = -1f / 60f
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
				if (dust.dustIndex != 200)
				{
					Dust dust2 = SeedDust.CloneDust(dust);
					dust2.scale /= 2f;
					dust2.fadeIn *= 0.75f;
					dust2.color = new Color(255, 255, 255, 255);
				}
			}
		}

		private void GoBackHere()
		{
			Main.MenuUI.SetState(_goBackTarget);
		}

		private void OnFinishedNamingAndCreating(string name)
		{
			OnFinishedSettingName(name);
			FinishCreatingWorld();
		}

		private void FinishCreatingWorld()
		{
			string name = (Main.worldName = _optionwWorldName.Trim());
			WorldDifficultyId optionDifficulty = _optionDifficulty;
			Main.ActiveWorldFileData = WorldFile.CreateMetadata(name, SocialAPI.Cloud != null && SocialAPI.Cloud.EnabledByDefault, Main.GameMode);
			_optionDifficulty = optionDifficulty;
			if (_optionSeed.Length == 0 || _isSpecialSeedText)
			{
				Main.ActiveWorldFileData.SetSeedToRandomWithCurrentEvents();
			}
			else
			{
				Main.ActiveWorldFileData.SetSeed(_optionSeed);
			}
			if (_secretSeedTextsEntered.Count > 0)
			{
				string seed = string.Join("|", _secretSeedTextsEntered) + "|" + Main.ActiveWorldFileData.SeedText;
				Main.ActiveWorldFileData.SetSeed(seed);
			}
			WorldGenerator.Controller controller = new WorldGenerator.Controller
			{
				Paused = (DebugOptions.enableDebugCommands && Main.keyState.PressingControl())
			};
			WorldGen.CreateNewWorld(null, controller);
		}

		private void AssignRandomWorldName()
		{
			do
			{
				LocalizedText localizedText = Language.SelectRandom(Lang.CreateDialogFilter("RandomWorldName_Composition.", checkConditions: false));
				LocalizedText localizedText2 = Language.SelectRandom(Lang.CreateDialogFilter("RandomWorldName_Adjective."));
				LocalizedText localizedText3 = Language.SelectRandom(Lang.CreateDialogFilter("RandomWorldName_Location."));
				LocalizedText localizedText4 = Language.SelectRandom(Lang.CreateDialogFilter("RandomWorldName_Noun."));
				var obj = new
				{
					Adjective = localizedText2.Value,
					Location = localizedText3.Value,
					Noun = localizedText4.Value
				};
				_optionwWorldName = localizedText.FormatWith(obj);
				if (Main.rand.Next(10000) == 0)
				{
					_optionwWorldName = Language.GetTextValue("SpecialWorldName.TheConstant");
				}
			}
			while (_optionwWorldName.Length > 27);
		}

		public override void Draw(SpriteBatch spriteBatch)
		{
			if (_goBackTarget != this)
			{
				_goBackTarget = this;
			}
			base.Draw(spriteBatch);
			SetupGamepadPoints(spriteBatch);
			DrawSeedSystems(spriteBatch);
		}

		public void ResetSpecialSeedRing()
		{
			ringPoint = 0f;
			Array.Clear(oldPos, 0, oldPos.Length);
			Array.Clear(oldTangent, 0, oldTangent.Length);
		}

		public void DrawSpecialSeedRingCallback(UIElement element, SpriteBatch spriteBatch)
		{
			if (!HasEnteredSpecialSeed)
			{
				return;
			}
			spriteBatch.End();
			spriteBatch.Begin(SpriteSortMode.Deferred, BlendState.AlphaBlend, SamplerState.AnisotropicClamp, DepthStencilState.None, RasterizerState.CullCounterClockwise, null, Main.UIScaleMatrix);
			if (oldPos[0] == Vector2.Zero)
			{
				for (int i = 0; i < 61; i++)
				{
					UpdateSpecialSeedRing(element);
				}
			}
			else
			{
				specialSeedIndex = (specialSeedIndex + 1) % 4;
				if (specialSeedIndex % 4 == 0)
				{
					UpdateSpecialSeedRing(element);
				}
			}
			DrawSpecialSeedRing();
			spriteBatch.End();
			spriteBatch.Begin(SpriteSortMode.Deferred, BlendState.AlphaBlend, SamplerState.AnisotropicClamp, DepthStencilState.None, RasterizerState.CullCounterClockwise, null, Main.UIScaleMatrix);
		}

		public void DrawSpecialSeedRingCallbackWithoutCondition(UIElement element, SpriteBatch spriteBatch)
		{
			spriteBatch.End();
			spriteBatch.Begin(SpriteSortMode.Deferred, BlendState.AlphaBlend, SamplerState.AnisotropicClamp, DepthStencilState.None, RasterizerState.CullCounterClockwise, null, Main.UIScaleMatrix);
			if (oldPos[0] == Vector2.Zero)
			{
				for (int i = 0; i < 61; i++)
				{
					UpdateSpecialSeedRing(element);
				}
			}
			else
			{
				UpdateSpecialSeedRing(element);
			}
			DrawSpecialSeedRing();
			spriteBatch.End();
			spriteBatch.Begin(SpriteSortMode.Deferred, BlendState.AlphaBlend, SamplerState.AnisotropicClamp, DepthStencilState.None, RasterizerState.CullCounterClockwise, null, Main.UIScaleMatrix);
		}

		private void UpdateSpecialSeedRing(UIElement element)
		{
			CalculatedStyle dimensions = _advancedSeedButton.GetDimensions();
			if (_goBackTarget != this && _goBackTarget is UIWorldCreationAdvanced uIWorldCreationAdvanced)
			{
				uIWorldCreationAdvanced.RefreshSecretSeedButton();
				dimensions = uIWorldCreationAdvanced.GetSecretSeedButton().GetDimensions();
			}
			if (element is GroupOptionButton<WorldGen.SecretSeed>)
			{
				dimensions = element.GetDimensions();
			}
			Rectangle rectangle = dimensions.ToRectangle();
			rectangle.Inflate(-1, -1);
			int num = rectangle.Width * 2 + rectangle.Height * 2;
			float num2 = (float)num / 60f;
			ringPoint += num2;
			if (ringPoint >= (float)num)
			{
				ringPoint -= num;
			}
			float num3 = (float)Math.Sqrt(rectangle.Width / 2 * rectangle.Width / 2 + rectangle.Height / 2 * rectangle.Height / 2);
			float num4 = (float)Math.PI * 2f * ringPoint / (float)num;
			Vector2 vector = new Vector2((float)Math.Cos(num4), (float)Math.Sin(num4));
			Vector2 vector2 = vector * num3;
			float num5 = Math.Abs(vector2.X) / ((float)rectangle.Width / 2f);
			float num6 = Math.Abs(vector2.Y) / ((float)rectangle.Height / 2f);
			if (num5 > num6)
			{
				vector2 /= num5;
				vector /= num5;
			}
			else
			{
				vector2 /= num6;
				vector /= num6;
			}
			vector2 += rectangle.Center.ToVector2();
			for (int num7 = oldPos.Length - 1; num7 > 0; num7--)
			{
				oldPos[num7] = oldPos[num7 - 1];
				oldTangent[num7] = oldTangent[num7 - 1];
			}
			oldPos[0] = vector2;
			oldTangent[0] = vector;
		}

		private void DrawSpecialSeedRing()
		{
			MiscShaderData miscShaderData = GameShaders.Misc["RainbowRod"];
			miscShaderData.UseSaturation(trial);
			miscShaderData.UseOpacity((_goBackTarget != this) ? opacity2 : opacity);
			miscShaderData.UseSpriteTransformMatrix(Main.UIScaleMatrix);
			miscShaderData.Apply();
			float num = 4f;
			if (_goBackTarget != this)
			{
				num = 5f;
			}
			int num2 = oldPos.Length;
			_vertexStrip.Reset(num2 * 2);
			int num3 = num2;
			for (int i = 0; i < num2 && !(oldPos[i] == Vector2.Zero); i++)
			{
				Vector2 vector = oldPos[i];
				float num4 = (float)i / (float)(num3 - 1);
				num4 *= 0.6f;
				Color vertexColor = StripColors(num4);
				float num5 = StripWidth(num4);
				Vector2 vector2 = oldTangent[i] * num;
				Vector3 uvA = new Vector3(num4, num5 / 2f, num5);
				Vector3 uvB = new Vector3(num4, num5 / 2f, num5);
				_vertexStrip.AddVertexPair(vector + vector2, vector, uvA, uvB, vertexColor);
			}
			_vertexStrip.PrepareIndices(includeBacksides: true);
			_vertexStrip.DrawTrail();
			Main.pixelShader.CurrentTechnique.Passes[0].Apply();
			miscShaderData.UseSpriteTransformMatrix(null);
		}

		private Color StripColors(float progressOnStrip)
		{
			Color result = Main.hslToRgb((progressOnStrip - Main.GlobalTimeWrappedHourly * animationSpeed) % 1f, saturation, 0.5f);
			result.A = 0;
			return result;
		}

		private float StripWidth(float progressOnStrip)
		{
			_ = 1f;
			float lerpValue = Utils.GetLerpValue(0f, 0.2f, progressOnStrip, clamped: true);
			return 24f * lerpValue;
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
			List<SnapPoint> snapPoints = GetSnapPoints();
			SnapPoint snapPoint = null;
			SnapPoint snapPoint2 = null;
			SnapPoint snapPoint3 = null;
			SnapPoint snapPoint4 = null;
			SnapPoint snapPoint5 = null;
			SnapPoint snapPoint6 = null;
			for (int i = 0; i < snapPoints.Count; i++)
			{
				SnapPoint snapPoint7 = snapPoints[i];
				switch (snapPoint7.Name)
				{
				case "Back":
					snapPoint = snapPoint7;
					break;
				case "Create":
					snapPoint2 = snapPoint7;
					break;
				case "Name":
					snapPoint3 = snapPoint7;
					break;
				case "Seed":
					snapPoint4 = snapPoint7;
					break;
				case "RandomizeName":
					snapPoint5 = snapPoint7;
					break;
				case "RandomizeSeed":
					snapPoint6 = snapPoint7;
					break;
				}
			}
			List<SnapPoint> snapGroup = GetSnapGroup(snapPoints, "size");
			List<SnapPoint> snapGroup2 = GetSnapGroup(snapPoints, "difficulty");
			List<SnapPoint> snapGroup3 = GetSnapGroup(snapPoints, "evil");
			UILinkPointNavigator.SetPosition(num, snapPoint.Position);
			UILinkPoint uILinkPoint = UILinkPointNavigator.Points[num];
			uILinkPoint.Unlink();
			UILinkPoint uILinkPoint2 = uILinkPoint;
			num++;
			UILinkPointNavigator.SetPosition(num, snapPoint2.Position);
			uILinkPoint = UILinkPointNavigator.Points[num];
			uILinkPoint.Unlink();
			UILinkPoint uILinkPoint3 = uILinkPoint;
			num++;
			UILinkPointNavigator.SetPosition(num, snapPoint5.Position);
			uILinkPoint = UILinkPointNavigator.Points[num];
			uILinkPoint.Unlink();
			UILinkPoint uILinkPoint4 = uILinkPoint;
			num++;
			UILinkPointNavigator.SetPosition(num, snapPoint3.Position);
			uILinkPoint = UILinkPointNavigator.Points[num];
			uILinkPoint.Unlink();
			UILinkPoint uILinkPoint5 = uILinkPoint;
			num++;
			UILinkPointNavigator.SetPosition(num, snapPoint6.Position);
			uILinkPoint = UILinkPointNavigator.Points[num];
			uILinkPoint.Unlink();
			UILinkPoint uILinkPoint6 = uILinkPoint;
			num++;
			UILinkPointNavigator.SetPosition(num, snapPoint4.Position);
			uILinkPoint = UILinkPointNavigator.Points[num];
			uILinkPoint.Unlink();
			UILinkPoint uILinkPoint7 = uILinkPoint;
			num++;
			UILinkPoint[] array = new UILinkPoint[snapGroup.Count];
			for (int j = 0; j < snapGroup.Count; j++)
			{
				UILinkPointNavigator.SetPosition(num, snapGroup[j].Position);
				uILinkPoint = UILinkPointNavigator.Points[num];
				uILinkPoint.Unlink();
				array[j] = uILinkPoint;
				num++;
			}
			UILinkPoint[] array2 = new UILinkPoint[snapGroup2.Count];
			for (int k = 0; k < snapGroup2.Count; k++)
			{
				UILinkPointNavigator.SetPosition(num, snapGroup2[k].Position);
				uILinkPoint = UILinkPointNavigator.Points[num];
				uILinkPoint.Unlink();
				array2[k] = uILinkPoint;
				num++;
			}
			UILinkPoint[] array3 = new UILinkPoint[snapGroup3.Count];
			for (int l = 0; l < snapGroup3.Count; l++)
			{
				UILinkPointNavigator.SetPosition(num, snapGroup3[l].Position);
				uILinkPoint = UILinkPointNavigator.Points[num];
				uILinkPoint.Unlink();
				array3[l] = uILinkPoint;
				num++;
			}
			LoopHorizontalLineLinks(array);
			LoopHorizontalLineLinks(array2);
			EstablishUpDownRelationship(array, array2);
			for (int m = 0; m < array.Length; m++)
			{
				array[m].Up = uILinkPoint7.ID;
			}
			if (true)
			{
				LoopHorizontalLineLinks(array3);
				EstablishUpDownRelationship(array2, array3);
				for (int n = 0; n < array3.Length; n++)
				{
					array3[n].Down = uILinkPoint2.ID;
				}
				array3[array3.Length - 1].Down = uILinkPoint3.ID;
				uILinkPoint3.Up = array3[array3.Length - 1].ID;
				uILinkPoint2.Up = array3[0].ID;
			}
			else
			{
				for (int num2 = 0; num2 < array2.Length; num2++)
				{
					array2[num2].Down = uILinkPoint2.ID;
				}
				array2[array2.Length - 1].Down = uILinkPoint3.ID;
				uILinkPoint3.Up = array2[array2.Length - 1].ID;
				uILinkPoint2.Up = array2[0].ID;
			}
			uILinkPoint3.Left = uILinkPoint2.ID;
			uILinkPoint2.Right = uILinkPoint3.ID;
			uILinkPoint5.Down = uILinkPoint7.ID;
			uILinkPoint5.Left = uILinkPoint4.ID;
			uILinkPoint4.Right = uILinkPoint5.ID;
			uILinkPoint7.Up = uILinkPoint5.ID;
			uILinkPoint7.Down = array[0].ID;
			uILinkPoint7.Left = uILinkPoint6.ID;
			uILinkPoint6.Right = uILinkPoint7.ID;
			uILinkPoint6.Up = uILinkPoint4.ID;
			uILinkPoint6.Down = array[0].ID;
			uILinkPoint4.Down = uILinkPoint6.ID;
		}

		private void EstablishUpDownRelationship(UILinkPoint[] topSide, UILinkPoint[] bottomSide)
		{
			int num = Math.Max(topSide.Length, bottomSide.Length);
			for (int i = 0; i < num; i++)
			{
				int num2 = Math.Min(i, topSide.Length - 1);
				int num3 = Math.Min(i, bottomSide.Length - 1);
				topSide[num2].Down = bottomSide[num3].ID;
				bottomSide[num3].Up = topSide[num2].ID;
			}
		}

		private void LoopHorizontalLineLinks(UILinkPoint[] pointsLine)
		{
			for (int i = 1; i < pointsLine.Length - 1; i++)
			{
				pointsLine[i - 1].Right = pointsLine[i].ID;
				pointsLine[i].Left = pointsLine[i - 1].ID;
				pointsLine[i].Right = pointsLine[i + 1].ID;
				pointsLine[i + 1].Left = pointsLine[i].ID;
			}
		}

		private List<SnapPoint> GetSnapGroup(List<SnapPoint> ptsOnPage, string groupName)
		{
			List<SnapPoint> list = ptsOnPage.Where((SnapPoint a) => a.Name == groupName).ToList();
			list.Sort(SortPoints);
			return list;
		}

		private int SortPoints(SnapPoint a, SnapPoint b)
		{
			return a.Id.CompareTo(b.Id);
		}

		public void AddSeedFromSeedmenu(string seed)
		{
			_secretSeedTextsEntered.Add(seed);
		}

		public void RemoveSeedFromSeedMenu(string seed)
		{
			_secretSeedTextsEntered.Remove(seed);
		}

		public void HandleBackButtonUsage()
		{
			GoBack();
		}
	}
}
