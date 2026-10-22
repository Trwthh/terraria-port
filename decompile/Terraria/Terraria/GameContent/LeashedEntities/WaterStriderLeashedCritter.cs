using Microsoft.Xna.Framework;

namespace Terraria.GameContent.LeashedEntities
{
	internal class WaterStriderLeashedCritter : JumperLeashedCritter
	{
		public new static WaterStriderLeashedCritter Prototype = new WaterStriderLeashedCritter();

		public WaterStriderLeashedCritter()
		{
			minWaitTime = 240;
			maxWaitTime = 540;
			strayingRangeInBlocksX = 5;
			strayingRangeInBlocksY = 12;
			maxJumpWidth = 32f;
			minJumpWidth = 8f;
			maxJumpHeight = 0f;
			maxJumpDuration = 7f;
			jumpCooldown = 120;
			canStandOnWater = true;
			drawBubble = false;
		}

		public override Vector2 GetDrawOffset()
		{
			Vector2 drawOffset = base.GetDrawOffset();
			Point point = base.Center.ToTileCoordinates();
			if (WorldGen.AnyLiquidAt(point, 0))
			{
				return drawOffset;
			}
			for (int i = 0; i < 2; i++)
			{
				point.Y++;
				byte liquid = Framing.GetTileSafely(point).liquid;
				if (liquid != 0)
				{
					drawOffset.Y = (255 - liquid) / 16;
					break;
				}
			}
			return drawOffset;
		}
	}
}
