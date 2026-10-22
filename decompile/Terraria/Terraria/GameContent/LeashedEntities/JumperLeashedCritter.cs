using System;
using Microsoft.Xna.Framework;
using Terraria.DataStructures;

namespace Terraria.GameContent.LeashedEntities
{
	public class JumperLeashedCritter : LeashedCritter
	{
		public static JumperLeashedCritter Prototype = new JumperLeashedCritter();

		private const int State_Normal = 0;

		private const int State_Recalling = 1;

		protected int minWaitTime;

		protected int maxWaitTime;

		protected float maxJumpWidth;

		protected float minJumpWidth;

		protected float maxJumpHeight;

		protected float maxJumpDuration;

		protected int jumpCooldown;

		protected bool canStandOnWater;

		protected static readonly float buoyancy = 0.4f;

		public JumperLeashedCritter()
		{
			strayingRangeInBlocksX = (strayingRangeInBlocksY = 12);
			minWaitTime = 180;
			maxWaitTime = 300;
			maxJumpWidth = 112f;
			minJumpWidth = 48f;
			maxJumpHeight = 64f;
			maxJumpDuration = 30f;
			jumpCooldown = 60;
			canStandOnWater = false;
		}

		public override void Spawn(bool newlyAdded)
		{
			base.Spawn(newlyAdded);
			PickNewTarget();
			SetJumpCooldown();
		}

		public override void Update()
		{
			base.Update();
			WaitTime--;
			if (WaitTime <= 0)
			{
				switch (State)
				{
				case 0:
					if (!TryStartJump())
					{
						PickNewTarget();
						SetJumpCooldown();
					}
					break;
				case 1:
					Recall();
					PickNewTarget();
					SetJumpCooldown();
					State = 0;
					break;
				}
			}
			Move(out var hitSomething);
			if (hitSomething && State != 1)
			{
				PickNewTarget();
				SetJumpCooldown();
			}
			if ((TargetPosition.ToWorldCoordinates() - base.Center).Length() < 8f && CanStandOnTile(TargetPosition))
			{
				base.Center = TargetPosition.ToWorldCoordinates();
				velocity = Vector2.Zero;
				PickNewTarget();
				SetJumpCooldown();
			}
			spriteDirection = direction;
			if (Main.netMode != 2)
			{
				VisualEffects();
			}
			CopyToDummy();
			LeashedCritter._dummy.FindFrame();
			CopyFromDummy();
		}

		private void SetJumpCooldown()
		{
			WaitTime = (short)rand.Next(minWaitTime, maxWaitTime + 1);
		}

		private bool TryStartJump()
		{
			Vector2 vector = TargetPosition.ToWorldCoordinates() - base.Center;
			if (vector.Y * -1f > maxJumpHeight)
			{
				return false;
			}
			float num = Math.Min(Math.Abs(vector.X), maxJumpWidth);
			if (num <= minJumpWidth)
			{
				return false;
			}
			float num2 = num / maxJumpWidth;
			float num3 = maxJumpDuration * num2;
			float num4 = vector.Y * num2 / num3 - 0.5f * LeashedCritter.gravity * num3;
			if (!(num4 < LeashedCritter.gravity * -1f))
			{
				return false;
			}
			direction = Math.Sign(vector.X);
			velocity.X = num / num3 * (float)direction;
			velocity.Y = num4;
			WaitTime = (short)(num3 + (float)jumpCooldown);
			return true;
		}

		private void Move(out bool hitSomething)
		{
			hitSomething = false;
			Point point = base.Center.ToTileCoordinates();
			int num = Math.Sign((int)velocity.X);
			if (num != 0)
			{
				direction = num;
			}
			int num2 = Math.Sign((int)velocity.Y);
			Vector2 vector = new Vector2(num, num2) * base.Size * 0.5f;
			Vector2 vec = base.Center + vector + velocity;
			Point point2 = vec.ToTileCoordinates();
			if (WorldGen.AnyLiquidAt(point, 0))
			{
				velocity.Y -= buoyancy;
				if (State != 1)
				{
					WaitTime = (short)minWaitTime;
				}
			}
			if (!WorldGen.SolidTileAllowPlatformTopFrame(point2.X, point2.Y))
			{
				Move_NoObstruction(point, vec.Y);
				return;
			}
			hitSomething = true;
			bool flag = false;
			if (num2 != 0)
			{
				Point point3 = point;
				point3.Y += num2;
				flag = WorldGen.SolidTileAllowPlatformTopFrame(point3.X, point3.Y);
			}
			bool flag2 = false;
			if (num != 0)
			{
				Point point4 = point;
				point4.X += num;
				flag2 = WorldGen.SolidTileNoPlatforms(point4.X, point4.Y);
			}
			if (flag)
			{
				velocity.Y = 0f;
			}
			if (flag2)
			{
				velocity.X = 0f;
			}
			if (!flag && !flag2)
			{
				velocity = Vector2.Zero;
			}
		}

		private void Move_NoObstruction(Point currentTile, float nextY)
		{
			if (velocity.Y >= 0f && nextY % 16f >= 8f)
			{
				Point tile = currentTile;
				tile.Y++;
				if (CanStandOnTile(tile))
				{
					base.Center = currentTile.ToWorldCoordinates();
					velocity = Vector2.Zero;
					return;
				}
			}
			base.Center += velocity;
			velocity.Y = MathHelper.Clamp(velocity.Y + LeashedCritter.gravity, 0f - LeashedCritter.maxFallSpeed, LeashedCritter.maxFallSpeed);
			if (State != 1 && Math.Abs(currentTile.Y - base.AnchorPosition.Y) > strayingRangeInBlocksY)
			{
				State = 1;
				WaitTime = 20;
			}
		}

		private void PickNewTarget()
		{
			int num = (int)(maxJumpWidth / 16f);
			int num2 = (int)(minJumpWidth / 16f);
			int num3 = TargetPosition.X - (base.AnchorPosition.X - strayingRangeInBlocksX);
			int num4 = base.AnchorPosition.X + strayingRangeInBlocksX - TargetPosition.X;
			bool flag = num3 >= num2;
			bool flag2 = num4 >= num2;
			if (flag || flag2)
			{
				int num5 = ((!(flag && flag2)) ? ((!flag) ? 1 : (-1)) : (rand.Next(2) * 2 - 1));
				int num6 = ((num5 < 1) ? num3 : num4);
				int num7 = rand.Next(1, num6 / num + 1);
				int num8 = num6 % num;
				if (num8 < num2)
				{
					num8 = 0;
				}
				int startX = TargetPosition.X + (num7 * num + num8) * num5;
				if (TryGetReachableTile(startX, out var tile))
				{
					TargetPosition = tile;
				}
			}
		}

		private bool TryGetReachableTile(int startX, out Point16 tile)
		{
			tile = Point16.Zero;
			int num = Math.Sign(base.AnchorPosition.X - startX);
			if (num == 0)
			{
				return false;
			}
			short num2 = (short)Math.Min(base.AnchorPosition.Y, (int)(base.Center.Y / 16f));
			for (int i = startX; i != base.AnchorPosition.X; i += num)
			{
				tile = new Point16(i, num2);
				if (WorldGen.SolidTileAllowPlatformTopFrame(tile.X, tile.Y) || WorldGen.AnyLiquidAt(tile, 0))
				{
					float num3 = maxJumpHeight / 16f;
					for (int j = 0; (float)j < num3; j++)
					{
						tile.Y--;
						if (!WorldGen.SolidTileAllowPlatformTopFrame(tile.X, tile.Y) && !WorldGen.AnyLiquidAt(tile, 0))
						{
							return true;
						}
					}
					continue;
				}
				int num4 = base.AnchorPosition.Y + strayingRangeInBlocksY;
				tile.Y = (short)(num2 + 1);
				while (tile.Y < num4)
				{
					if (CanStandOnTile(tile))
					{
						tile.Y--;
						return true;
					}
					tile.Y++;
				}
			}
			return false;
		}

		private bool CanStandOnTile(Point tile)
		{
			if (WorldGen.SolidTileAllowPlatformTopFrame(tile.X, tile.Y))
			{
				return true;
			}
			if (canStandOnWater && WorldGen.AnyLiquidAt(tile, 0))
			{
				return !WorldGen.AnyLiquidAt(tile.X, tile.Y - 1, 0);
			}
			return false;
		}

		protected override void CopyToDummy()
		{
			base.CopyToDummy();
			if (State == 1)
			{
				LeashedCritter._dummy.Opacity = (float)WaitTime / 20f;
			}
		}

		public override Vector2 GetDrawOffset()
		{
			Point16 point = base.Center.ToTileCoordinates16();
			if (Framing.GetTileSafely(point.X, point.Y + 1).halfBrick())
			{
				return new Vector2(0f, base.Center.Y % 16f);
			}
			return base.GetDrawOffset();
		}
	}
}
