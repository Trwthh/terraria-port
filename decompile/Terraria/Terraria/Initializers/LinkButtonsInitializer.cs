using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;
using Terraria.DataStructures;

namespace Terraria.Initializers
{
	public class LinkButtonsInitializer
	{
		public static void Load()
		{
			List<TitleLinkButton> titleLinks = Main.TitleLinks;
			titleLinks.Add(MakeSimpleButton("TitleLinks.Discord", "https://discord.gg/terraria", 0));
			titleLinks.Add(MakeSimpleButton("TitleLinks.Instagram", "https://www.instagram.com/terraria_logic/", 1));
			titleLinks.Add(MakeSimpleButton("TitleLinks.Reddit", "https://www.reddit.com/r/Terraria/", 2));
			titleLinks.Add(MakeSimpleButton("TitleLinks.Twitter", "https://twitter.com/Terraria_Logic", 3));
			titleLinks.Add(MakeSimpleButton("TitleLinks.Bluesky", "https://bsky.app/profile/terraria.bsky.social", 4));
			titleLinks.Add(MakeSimpleButton("TitleLinks.Forums", "https://forums.terraria.org/index.php", 5));
			titleLinks.Add(MakeSimpleButton("TitleLinks.Merch", "https://terraria.org/store", 6));
			titleLinks.Add(MakeSimpleButton("TitleLinks.Wiki", "https://terraria.wiki.gg/", 7));
		}

		private static TitleLinkButton MakeSimpleButton(string textKey, string linkUrl, int horizontalFrameIndex)
		{
			Asset<Texture2D> asset = Main.Assets.Request<Texture2D>("Images/UI/TitleLinkButtons");
			Rectangle value = asset.Frame(8, 2, horizontalFrameIndex);
			Rectangle value2 = asset.Frame(8, 2, horizontalFrameIndex, 1);
			value.Width--;
			value.Height--;
			value2.Width--;
			value2.Height--;
			return new TitleLinkButton
			{
				TooltipTextKey = textKey,
				LinkUrl = linkUrl,
				FrameWehnSelected = value2,
				FrameWhenNotSelected = value,
				Image = asset
			};
		}
	}
}
