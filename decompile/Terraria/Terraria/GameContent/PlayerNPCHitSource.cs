using System;
using Terraria.ID;

namespace Terraria.GameContent
{
	public struct PlayerNPCHitSource : IEquatable<PlayerNPCHitSource>
	{
		public readonly int ItemType;

		public readonly int ProjType;

		public readonly string Other;

		public string Name
		{
			get
			{
				if (ItemType != 0)
				{
					if (ContentSamples.ItemsByType.TryGetValue(ItemType, out var value))
					{
						return value.Name;
					}
					return "Item " + ItemType;
				}
				if (ProjType != 0)
				{
					if (ContentSamples.ProjectilesByType.TryGetValue(ProjType, out var value2))
					{
						return value2.Name;
					}
					return "Projectile " + ItemType;
				}
				return Other;
			}
		}

		public PlayerNPCHitSource(int itemType, int projType, string other)
		{
			ItemType = itemType;
			ProjType = projType;
			Other = other;
		}

		public static PlayerNPCHitSource FromItem(int itemType)
		{
			return new PlayerNPCHitSource(itemType, 0, null);
		}

		public static PlayerNPCHitSource FromProjectile(int projType)
		{
			return new PlayerNPCHitSource(0, projType, null);
		}

		public static PlayerNPCHitSource FromOther(string s)
		{
			return new PlayerNPCHitSource(0, 0, null);
		}

		public bool Equals(PlayerNPCHitSource other)
		{
			if (ItemType == other.ItemType && ProjType == other.ProjType)
			{
				return Other == other.Other;
			}
			return false;
		}

		public override int GetHashCode()
		{
			return ((17 * 31 + ItemType) * 31 + ProjType) * 31 + Other.GetHashCode();
		}

		public override bool Equals(object other)
		{
			if (other is PlayerNPCHitSource)
			{
				return Equals((PlayerNPCHitSource)other);
			}
			return false;
		}

		public static bool operator ==(PlayerNPCHitSource left, PlayerNPCHitSource right)
		{
			return left.Equals(right);
		}

		public static bool operator !=(PlayerNPCHitSource left, PlayerNPCHitSource right)
		{
			return !left.Equals(right);
		}
	}
}
