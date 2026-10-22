using System;
using Microsoft.Xna.Framework;
using Terraria.Testing;

namespace Terraria.DataStructures
{
	public static class ActiveSections
	{
		public static readonly uint SectionInactiveTime = 60u;

		private static uint[,] LastActiveTime = new uint[Main.maxTilesX / 200 + 1, Main.maxTilesY / 150 + 1];

		public static event Action<Point> SectionActivated;

		public static void CheckSection(Vector2 position, int fluff = 1)
		{
			int sectionX = Netplay.GetSectionX((int)(position.X / 16f));
			int sectionY = Netplay.GetSectionY((int)(position.Y / 16f));
			for (int i = sectionX - fluff; i < sectionX + fluff + 1; i++)
			{
				for (int j = sectionY - fluff; j < sectionY + fluff + 1; j++)
				{
					if (i >= 0 && i < Main.maxSectionsX && j >= 0 && j < Main.maxSectionsY)
					{
						bool num = IsSectionActive(new Point(i, j));
						LastActiveTime[i, j] = Main.GameUpdateCount;
						if (!num)
						{
							ActiveSections.SectionActivated(new Point(i, j));
						}
					}
				}
			}
		}

		public static bool IsSectionActive(Point sectionCoords)
		{
			sectionCoords = sectionCoords.ClampSectionCoords();
			return LastActiveTime[sectionCoords.X, sectionCoords.Y] + SectionInactiveTime >= Main.GameUpdateCount;
		}

		public static int TimeTillInactive(Point sectionCoords)
		{
			sectionCoords = sectionCoords.ClampSectionCoords();
			return (int)Math.Max(0L, (long)(LastActiveTime[sectionCoords.X, sectionCoords.Y] + SectionInactiveTime) - (long)Main.GameUpdateCount);
		}

		public static void Reset()
		{
			Array.Clear(LastActiveTime, 0, LastActiveTime.Length);
		}

		public static Point ClampSectionCoords(this Point point)
		{
			return new Point(Utils.Clamp(point.X, 0, Main.maxSectionsX), Utils.Clamp(point.Y, 0, Main.maxSectionsY));
		}

		internal static void AddGameplaySnapshotComponents()
		{
			StateSnapshot.Gameplay.AddCollection("ActiveSections.LastActiveTime", LastActiveTime);
		}
	}
}
