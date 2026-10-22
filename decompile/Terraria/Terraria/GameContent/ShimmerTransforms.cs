using Terraria.Enums;
using Terraria.ID;

namespace Terraria.GameContent
{
	public static class ShimmerTransforms
	{
		public static class RecipeSets
		{
			public static bool[] PostSkeletron;

			public static bool[] PostGolem;
		}

		public static int GetDecraftingRecipeIndex(int type)
		{
			int num = ItemID.Sets.IsCrafted[type];
			if (num < 0)
			{
				return -1;
			}
			if (WorldGen.crimson && ItemID.Sets.IsCraftedCrimson[type] >= 0)
			{
				return ItemID.Sets.IsCraftedCrimson[type];
			}
			if (!WorldGen.crimson && ItemID.Sets.IsCraftedCorruption[type] >= 0)
			{
				return ItemID.Sets.IsCraftedCorruption[type];
			}
			return num;
		}

		public static bool IsItemTransformLocked(int type)
		{
			if (!NPC.downedMoonlord && ItemID.Sets.ShimmerPostMoonlord[type])
			{
				return true;
			}
			return false;
		}

		public static bool IsItemDecraftLocked(int type)
		{
			return IsRecipeIndexDecraftLocked(GetDecraftingRecipeIndex(type));
		}

		public static bool IsRecipeIndexDecraftLocked(int recipeIndex)
		{
			if (recipeIndex < 0)
			{
				return false;
			}
			if (!NPC.downedBoss3 && RecipeSets.PostSkeletron[recipeIndex])
			{
				return true;
			}
			if (!NPC.downedGolemBoss && RecipeSets.PostGolem[recipeIndex])
			{
				return true;
			}
			return false;
		}

		public static bool IsItemDecraftableAndIsDecraftUnlocked(Item item)
		{
			if (item == null)
			{
				return false;
			}
			int decraftingRecipeIndex = GetDecraftingRecipeIndex(item.GetShimmerEquivalentType(forDecrafting: true));
			if (IsRecipeIndexDecraftLocked(decraftingRecipeIndex))
			{
				return false;
			}
			if (decraftingRecipeIndex < 0)
			{
				return false;
			}
			return item.stack / Main.recipe[decraftingRecipeIndex].createItem.stack > 0;
		}

		public static void UpdateRecipeSets()
		{
			RecipeSets.PostSkeletron = Utils.MapArray(Main.recipe, (Recipe r) => r.ContainsIngredient(154));
			RecipeSets.PostGolem = Utils.MapArray(Main.recipe, (Recipe r) => r.ContainsIngredient(1101));
		}

		public static int GetTransformToItem(int type)
		{
			int num = ItemID.Sets.ShimmerTransformToItem[type];
			if (num > 0)
			{
				return num;
			}
			if (ContentSamples.ItemsByType[type].createTile == 139)
			{
				switch (ContentSamples.ItemsByType[type].placeStyle)
				{
				case 90:
					return 5538;
				case 89:
					return 5579;
				case 97:
					return 5638;
				case 96:
					return 5639;
				default:
					return 576;
				}
			}
			if (type == 3461)
			{
				return GetLunarBrickTransformFromMoonPhase(Main.GetMoonPhase());
			}
			return 0;
		}

		private static int GetLunarBrickTransformFromMoonPhase(MoonPhase moonPhase)
		{
			switch (moonPhase)
			{
			default:
				return 5406;
			case MoonPhase.QuarterAtRight:
				return 5407;
			case MoonPhase.HalfAtRight:
				return 5405;
			case MoonPhase.ThreeQuartersAtRight:
				return 5404;
			case MoonPhase.Full:
				return 5408;
			case MoonPhase.ThreeQuartersAtLeft:
				return 5401;
			case MoonPhase.HalfAtLeft:
				return 5403;
			case MoonPhase.QuarterAtLeft:
				return 5402;
			}
		}
	}
}
