using System;
using Microsoft.Xna.Framework;
using Terraria.DataStructures;

namespace Terraria.GameContent.LeashedEntities
{
	public class WalkerLeashedCritter : LeashedCritter
	{
		public static WalkerLeashedCritter Prototype = new WalkerLeashedCritter();

		private const int State_Standing = 0;

		private const int State_PickDirection = 1;

		private const int State_Walking = 2;

		private const int State_Falling = 3;

		private const int State_Recalling = 4;

		protected float walkingPace;

		public WalkerLeashedCritter()
		{
			walkingPace = 0.8f;
			strayingRangeInBlocksX = (strayingRangeInBlocksY = 3);
		}

		protected bool AdvanceTargetPosition()
		{
			if (Math.Abs(TargetPosition.X - base.AnchorPosition.X) >= strayingRangeInBlocksX)
			{
				direction = Math.Sign(base.AnchorPosition.X - TargetPosition.X);
			}
			if (!WorldGen.InWorld(TargetPosition.X + direction, TargetPosition.Y))
			{
				direction *= -1;
			}
			spriteDirection = direction;
			int num = TargetPosition.X + direction;
			short y = TargetPosition.Y;
			bool num2 = !WorldGen.SolidTileAllowTopSlope(num, y - 1);
			bool flag = !WorldGen.SolidTileNoPlatforms(num, y);
			bool flag2 = !WorldGen.SolidTileAllowBottomSlope(num, y + 1);
			bool flag3 = WorldGen.AnyLiquidAt(num, y + 1);
			bool flag4 = !WorldGen.SolidTileAllowBottomSlope(num, y + 2);
			bool flag5 = num2 && !flag;
			bool flag6 = flag && flag2 && !flag3 && !flag4;
			bool flag7 = flag && !flag2;
			if (flag5)
			{
				TargetPosition = new Point16(num, y - 1);
			}
			else if (flag6)
			{
				TargetPosition = new Point16(num, y + 1);
			}
			else
			{
				if (!flag7)
				{
					return false;
				}
				TargetPosition = new Point16(num, y);
			}
			return true;
		}

		public override void Update()
		{
			base.Update();
			Point16 tilePosition = base.Center.ToTileCoordinates16();
			HandleFalling(tilePosition);
			WaitTime--;
			if (WaitTime <= 0)
			{
				if (State == 4)
				{
					Recall();
				}
				WaitTime = (short)rand.Next(60, 61);
				State = (byte)rand.Next(2);
			}
			HandleWalking();
			int value = TargetPosition.X - tilePosition.X;
			int num = TargetPosition.Y - tilePosition.Y;
			if (Math.Abs(value) == 1 && Math.Abs(num) == 1)
			{
				velocity.Y = num * 2;
			}
			float maxAmountAllowedToMove = velocity.Length();
			Vector2 vector = TargetPosition.ToWorldCoordinates();
			base.Center = base.Center.MoveTowards(vector, maxAmountAllowedToMove);
			if (base.Center == vector && State == 0)
			{
				velocity = Vector2.Zero;
			}
			if (Main.netMode != 2)
			{
				VisualEffects();
			}
			CopyToDummy();
			LeashedCritter._dummy.FindFrame();
			CopyFromDummy();
		}

		private void HandleFalling(Point16 tilePosition)
		{
			if (WorldGen.SolidTileAllowPlatformTopFrame(tilePosition.X, tilePosition.Y + 1))
			{
				velocity.Y = 0f;
				if (State == 3 || State == 4)
				{
					base.Center = TargetPosition.ToWorldCoordinates();
				}
				if (State == 3)
				{
					State = 0;
					WaitTime = 0;
				}
				return;
			}
			velocity.Y += LeashedCritter.gravity;
			if (velocity.Y > LeashedCritter.maxFallSpeed)
			{
				velocity.Y = LeashedCritter.maxFallSpeed;
			}
			TargetPosition.X = tilePosition.X;
			TargetPosition.Y = (short)Math.Min(tilePosition.Y + 1, Main.maxTilesY - 1);
			if (State != 4)
			{
				if (TargetPosition.Y - base.AnchorPosition.Y > strayingRangeInBlocksY)
				{
					State = 4;
					WaitTime = 20;
				}
				else
				{
					State = 3;
				}
			}
		}

		private void HandleWalking()
		{
			if (State == 3 || State == 4)
			{
				return;
			}
			velocity.X = walkingPace * (float)direction;
			if (State != 0 && !(base.Center.Distance(TargetPosition.ToWorldCoordinates()) >= 1f))
			{
				if (State == 1)
				{
					direction = rand.Next(2) * 2 - 1;
					State = 2;
				}
				if (!AdvanceTargetPosition())
				{
					WaitTime = 30;
					State = 0;
				}
			}
		}

		protected override void CopyToDummy()
		{
			base.CopyToDummy();
			if (State == 4)
			{
				LeashedCritter._dummy.Opacity = (float)WaitTime / 20f;
			}
		}

		public override Vector2 GetDrawOffset()
		{
			Point16 point = base.Center.ToTileCoordinates16();
			if (Framing.GetTileSafely(point.X, point.Y + 1).halfBrick())
			{
				return new Vector2(0f, 8f);
			}
			return base.GetDrawOffset();
		}
	}
}
