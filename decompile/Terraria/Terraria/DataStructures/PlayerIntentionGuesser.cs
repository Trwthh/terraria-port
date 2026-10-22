using Microsoft.Xna.Framework;
using Terraria.GameContent;

namespace Terraria.DataStructures
{
	public class PlayerIntentionGuesser
	{
		public int LastX;

		public int LastY;

		public Vector2 LastPosition;

		public Vector2 LastCenter;

		public Vector2 LastMouse;

		public int LastDirection;

		public int LastWidth;

		public GuessedPlayerIntention Intention;

		public SmartCursorHelper.SmartCursorUsageInfo UsageProxy = new SmartCursorHelper.SmartCursorUsageInfo();

		public int TimeWithIntention;

		public int PlayerActiveActionTimeLeft;

		public void Track(Player player, int x, int y, GuessedPlayerIntention intention)
		{
			if (PlayerActiveActionTimeLeft != 0)
			{
				LastX = x;
				LastY = y;
				Intention = intention;
				LastPosition = player.position;
				LastCenter = player.Center;
				LastDirection = player.direction;
				LastWidth = player.width;
				LastMouse = Main.MouseWorld;
			}
		}

		public void AllowTracking(int time = 60)
		{
			PlayerActiveActionTimeLeft = time;
		}

		public void Update(Player player)
		{
			if (player.whoAmI != Main.myPlayer)
			{
				return;
			}
			TimeWithIntention++;
			if (PlayerActiveActionTimeLeft > 0)
			{
				PlayerActiveActionTimeLeft--;
			}
			if (Intention != GuessedPlayerIntention.None)
			{
				float num = player.Center.Distance(LastCenter);
				bool flag = false;
				if (num > 80f)
				{
					flag = true;
				}
				if (player.controlJump)
				{
					flag = true;
				}
				bool usingOrReusingItem = player.UsingOrReusingItem;
				if (usingOrReusingItem && Intention == GuessedPlayerIntention.HarvestTreasure && player.HeldItem.pick <= 0)
				{
					flag = true;
				}
				if (usingOrReusingItem && Intention == GuessedPlayerIntention.HarvestTrees && player.HeldItem.axe <= 0)
				{
					flag = true;
				}
				if (TimeWithIntention >= 480)
				{
					flag = true;
				}
				if (player.dead)
				{
					flag = true;
				}
				if (flag)
				{
					Intention = GuessedPlayerIntention.None;
					TimeWithIntention = 0;
				}
			}
		}

		public void PrepareUsageProxy(Player player, int itemType, int areaInflateWidth, int areaInflateHeight)
		{
			UsageProxy.player = player;
			if (UsageProxy.item == null)
			{
				UsageProxy.item = new Item();
			}
			UsageProxy.item.SetDefaults(itemType);
			UsageProxy.position = LastPosition;
			UsageProxy.Center = LastCenter;
			UsageProxy.mouse = LastMouse;
			UsageProxy.screenTargetX = LastX;
			UsageProxy.screenTargetY = LastY;
			UsageProxy.screenTargetX = Utils.Clamp(UsageProxy.screenTargetX, 10, Main.maxTilesX - 10);
			UsageProxy.screenTargetY = Utils.Clamp(UsageProxy.screenTargetY, 10, Main.maxTilesY - 10);
			Rectangle value = new Rectangle(LastX, LastY, 1, 1);
			value.Inflate(areaInflateWidth, areaInflateHeight);
			Rectangle value2 = new Rectangle(0, 0, Main.maxTilesX, Main.maxTilesY);
			value2.Inflate(-10, -10);
			Rectangle rectangle = default(Rectangle);
			rectangle = Rectangle.Intersect(value, value2);
			UsageProxy.reachableStartX = rectangle.Left;
			UsageProxy.reachableStartY = rectangle.Top;
			UsageProxy.reachableEndX = rectangle.Right;
			UsageProxy.reachableEndY = rectangle.Bottom;
		}
	}
}
