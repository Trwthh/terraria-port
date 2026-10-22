using System;
using Microsoft.Xna.Framework;
using Terraria.DataStructures;

namespace Terraria.GameContent.LeashedEntities
{
	public class FlyerLeashedCritter : LeashedCritter
	{
		public static FlyerLeashedCritter Prototype = new FlyerLeashedCritter();

		protected int minWaitTime;

		protected int maxWaitTime;

		protected float maxFlySpeed;

		protected float acceleration;

		protected int brakeDuration;

		protected float rotationScalar;

		protected float hoverAmplitude;

		protected float hoverPeriod;

		protected bool hasGroundBias;

		private const float HoverYVelocity = 0.0001f;

		public FlyerLeashedCritter()
		{
			anchorStyle = 4;
			strayingRangeInBlocksX = (strayingRangeInBlocksY = 7);
			minWaitTime = 60;
			maxWaitTime = 300;
			maxFlySpeed = 1f;
			acceleration = 0.2f;
			brakeDuration = 10;
		}

		public override void Spawn(bool newlyAdded)
		{
			base.Spawn(newlyAdded);
			if (!WorldGen.SolidTileAllowPlatformTopFrame(base.AnchorPosition.X, base.AnchorPosition.Y + 1))
			{
				velocity.Y = 0.0001f;
			}
			PickNewTarget();
		}

		protected void PickNewTarget()
		{
			bool num = hasGroundBias && base.AnchorPosition.Y == TargetPosition.Y && rand.Next(4) != 0;
			TargetPosition = new Point16(base.AnchorPosition.X + rand.Next(-strayingRangeInBlocksX, strayingRangeInBlocksX + 1), base.AnchorPosition.Y + rand.Next(-strayingRangeInBlocksY, 1));
			if (num)
			{
				TargetPosition.Y = base.AnchorPosition.Y;
			}
		}

		protected override void CopyToDummy()
		{
			base.CopyToDummy();
			if (velocity.Y != 0f)
			{
				LeashedCritter._dummy.rotation = velocity.X * rotationScalar;
			}
		}

		public override void Update()
		{
			base.Update();
			WaitTime--;
			if (WaitTime <= 0)
			{
				WaitTime = (short)rand.Next(minWaitTime, maxWaitTime + 1);
				PickNewTarget();
			}
			Point origin = base.Center.ToTileCoordinates();
			Vector2 vector = TargetPosition.ToWorldCoordinates();
			Vector2 vector2 = vector - base.Center;
			float num = vector2.Length();
			Vector2 vector3 = vector2 / num;
			if (vector3.HasNaNs())
			{
				vector3 = Vector2.Zero;
			}
			velocity += vector3 * acceleration;
			float num2 = velocity.Length();
			float val = Math.Min(1f, num / ((float)brakeDuration * maxFlySpeed));
			float num3 = maxFlySpeed * Math.Max(val, 0.25f);
			if (num2 > num3)
			{
				velocity *= num3 / num2;
				num2 = num3;
			}
			bool flag = num < maxFlySpeed;
			bool flag2 = flag;
			if (!flag2)
			{
				Point destination = (base.Center + base.Size * 0.5f * vector3 + velocity).ToTileCoordinates();
				flag2 = WorldGen.SolidTileAllowPlatformTopFrame(destination.X, destination.Y) || IsBlockedByMedium(origin, destination);
			}
			if (flag2)
			{
				bool flag3 = false;
				if (flag)
				{
					base.Center = vector;
					flag3 = true;
				}
				velocity.X = 0f;
				velocity.Y = ((flag3 && WorldGen.SolidTileAllowPlatformTopFrame(origin.X, origin.Y + 1)) ? 0f : 0.0001f);
			}
			else
			{
				base.Center += velocity;
				if (velocity.Y == 0f && !WorldGen.SolidTileAllowPlatformTopFrame(origin.X, origin.Y + 1))
				{
					velocity.Y = 0.0001f;
				}
			}
			int num4 = Math.Sign(velocity.X);
			if (num4 != 0 && num4 != direction)
			{
				direction = num4;
				spriteDirection = -direction;
			}
			if (Main.netMode != 2)
			{
				VisualEffects();
			}
			CopyToDummy();
			LeashedCritter._dummy.FindFrame();
			CopyFromDummy();
		}

		private bool IsBlockedByMedium(Point origin, Point destination)
		{
			if (origin == destination)
			{
				return false;
			}
			int num = (isAquatic ? 255 : 0);
			int liquidAmount;
			int liquidType;
			bool num2 = IsLiquidBoundary(origin, out liquidAmount, out liquidType);
			bool flag = liquidType > 0;
			bool flag2 = liquidAmount == num;
			if (!(!num2 && !flag && flag2))
			{
				return false;
			}
			int liquidAmount2;
			int liquidType2;
			bool num3 = IsLiquidBoundary(destination, out liquidAmount2, out liquidType2);
			bool flag3 = liquidType2 > 0;
			bool flag4 = liquidAmount2 == num;
			return !(!num3 && !flag3 && flag4);
		}

		private bool IsLiquidBoundary(Point coordinates, out int liquidAmount, out int liquidType)
		{
			Tile tileSafely = Framing.GetTileSafely(coordinates);
			liquidAmount = tileSafely.liquid;
			liquidType = ((tileSafely.liquid > 0) ? tileSafely.liquidType() : (-1));
			if (tileSafely.nactive() && Main.tileSolid[tileSafely.type] && !Main.tileSolidTop[tileSafely.type])
			{
				return false;
			}
			if (liquidAmount > 0 && liquidAmount < 255)
			{
				return true;
			}
			if (liquidAmount == 0)
			{
				coordinates.Y++;
				Tile tileSafely2 = Framing.GetTileSafely(coordinates);
				if (tileSafely2.liquid == byte.MaxValue)
				{
					if (tileSafely2.nactive() && Main.tileSolid[tileSafely2.type])
					{
						return Main.tileSolidTop[tileSafely2.type];
					}
					return true;
				}
				return false;
			}
			coordinates.Y--;
			Tile tileSafely3 = Framing.GetTileSafely(coordinates);
			if (tileSafely3.liquid == 0)
			{
				if (tileSafely3.nactive() && Main.tileSolid[tileSafely3.type])
				{
					return Main.tileSolidTop[tileSafely3.type];
				}
				return true;
			}
			return false;
		}

		public override Vector2 GetDrawOffset()
		{
			if (velocity.Y == 0f)
			{
				Point16 point = base.Center.ToTileCoordinates16();
				if (Framing.GetTileSafely(point.X, point.Y + 1).halfBrick())
				{
					return new Vector2(0f, 8f);
				}
				return Vector2.Zero;
			}
			if (hoverPeriod == 0f || hoverAmplitude == 0f)
			{
				return Vector2.Zero;
			}
			return GetBobbingOffset();
		}

		protected Vector2 GetBobbingOffset()
		{
			double num = Main.timeForVisualEffects + (double)(whoAmI * npcType);
			num *= (double)(hoverPeriod * ((float)Math.PI * 2f));
			return new Vector2(0f, (float)Math.Sin(num) * hoverAmplitude);
		}
	}
}
