using System.IO;
using Microsoft.Xna.Framework;
using Terraria.Net;

namespace Terraria.GameContent
{
	public static class UnbreakableWallScan
	{
		public class NetModule : Terraria.Net.NetModule
		{
			public override bool Deserialize(BinaryReader reader, int userId)
			{
				if (Main.netMode != 1)
				{
					return false;
				}
				Main.player[reader.ReadByte()].insideUnbreakableWalls = reader.ReadBoolean();
				return true;
			}

			internal static void BroadcastChange(Player player)
			{
				NetPacket packet = Terraria.Net.NetModule.CreatePacket<NetModule>();
				packet.Writer.Write((byte)player.whoAmI);
				packet.Writer.Write(player.insideUnbreakableWalls);
				NetManager.Instance.Broadcast(packet);
			}
		}

		public static readonly int ScanDistance = 250;

		public static readonly Point[] Directions = new Point[8]
		{
			new Point(1, 0),
			new Point(1, 1),
			new Point(0, 1),
			new Point(-1, 1),
			new Point(-1, 0),
			new Point(-1, -1),
			new Point(0, -1),
			new Point(1, -1)
		};

		public static void Update(Player player)
		{
			_ = Main.netMode;
			_ = 1;
		}

		public static bool InsideUnbreakableWalls(Point pt)
		{
			int num = 0;
			for (int i = 0; i < Directions.Length; i++)
			{
				if (LineScan(pt, Directions[i]))
				{
					num |= 1 << i;
				}
			}
			for (int j = 0; j < Directions.Length; j++)
			{
				if ((num & 0x1F) == 0)
				{
					return false;
				}
				num = ((num << 1) & 0xFF) | (num >> 7);
			}
			return true;
		}

		public static bool LineScan(Point pt, Point dir)
		{
			int num = 0;
			while (num < ScanDistance)
			{
				if (!WorldGen.InWorld(pt))
				{
					return false;
				}
				Tile tile = Main.tile[pt.X, pt.Y];
				if (tile == null)
				{
					return false;
				}
				if (tile.wall == 350)
				{
					return tile.wallColor() >= 16;
				}
				num++;
				pt.X += dir.X;
				pt.Y += dir.Y;
			}
			return false;
		}
	}
}
