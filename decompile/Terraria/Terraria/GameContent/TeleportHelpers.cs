using System;
using Microsoft.Xna.Framework;

namespace Terraria.GameContent
{
	public class TeleportHelpers
	{
		public static bool FindClosestTeleportSpotNoSpace(Player player, out Vector2 resultPosition)
		{
			bool result = false;
			resultPosition = player.position;
			player.velocity = Vector2.Zero;
			Vector2 vector = new Vector2((float)player.width * 0.5f, player.height);
			Vector2 bottom = player.Bottom;
			Point point = bottom.ToTileCoordinates();
			int value = point.X - 25;
			int value2 = point.X + 25;
			int value3 = point.Y - 25;
			int value4 = point.Y + 25;
			value = Utils.Clamp(value, 40, Main.maxTilesX - 40);
			value2 = Utils.Clamp(value2, 40, Main.maxTilesX - 40);
			value3 = Utils.Clamp(value3, 40, Main.maxTilesY - 40);
			value4 = Utils.Clamp(value4, 40, Main.maxTilesY - 40);
			float num = float.MaxValue;
			for (int i = value; i < value2; i++)
			{
				for (int j = value3; j < value4; j++)
				{
					Vector2 vector2 = new Vector2(i * 16 + 8, j * 16 + 15) - vector;
					Tile tile = Main.tile[i, j];
					Tile tile2 = Main.tile[i, j + 1];
					bool flag = WorldGen.SolidOrSlopedTile(tile) || tile.liquid > 0;
					bool flag2 = WorldGen.SolidOrSlopedTile(tile2) && tile2.liquid == 0;
					if (!TileIsDangerous(i, j, player) && !flag && flag2 && !Collision.LavaCollision(vector2, player.width, player.height) && !Collision.AnyHurtingTiles(vector2, player.width, player.height) && !Collision.SolidCollision(vector2, player.width, player.height))
					{
						float num2 = (vector2 - bottom).Length();
						if (num2 < num)
						{
							resultPosition = vector2;
							num = num2;
							result = true;
						}
					}
				}
			}
			return result;
		}

		public static bool RequestMagicConchTeleportPosition(Player player, int crawlOffsetX, bool rightOcean, out Point landingPoint)
		{
			landingPoint = default(Point);
			int num = 40;
			int num2 = 0;
			int x = (rightOcean ? (Main.maxTilesX - num) : num);
			int y = 50;
			int num3 = (int)Main.worldSurface - num2;
			Point point = new Point(x, y);
			int num4 = 1;
			int num5 = -1;
			int num6 = 1;
			int num7 = 0;
			int num8 = 5000;
			Vector2 vector = new Vector2((float)player.width * 0.5f, player.height);
			int num9 = 40;
			bool flag = WorldGen.SolidOrSlopedTile(Main.tile[point.X, point.Y], includePlatforms: true);
			int num10 = 0;
			int num11 = 400;
			if (WorldGen.Skyblock.lowTiles)
			{
				num8 = num11 * ((int)Main.worldSurface - 10);
			}
			while (num7 < num8 && num10 < num11)
			{
				num7++;
				Tile tile = Main.tile[point.X, point.Y];
				Tile tile2 = Main.tile[point.X, point.Y + num6];
				bool flag2 = WorldGen.SolidOrSlopedTile(tile, includePlatforms: true) || tile.liquid > 0;
				bool flag3 = WorldGen.SolidOrSlopedTile(tile2, includePlatforms: true) || tile2.liquid > 0;
				if (!flag2 && !flag3)
				{
					Tile tile3 = Main.tile[point.X - 1, point.Y];
					Tile tile4 = Main.tile[point.X + 1, point.Y];
					Tile tile5 = Main.tile[point.X - 1, point.Y + num6];
					Tile tile6 = Main.tile[point.X + 1, point.Y + num6];
					bool num12 = WorldGen.SolidOrSlopedTile(tile3, includePlatforms: true) || tile3.liquid > 0;
					bool flag4 = WorldGen.SolidOrSlopedTile(tile4, includePlatforms: true) || tile4.liquid > 0;
					bool flag5 = WorldGen.SolidOrSlopedTile(tile5, includePlatforms: true) || tile5.liquid > 0;
					bool flag6 = WorldGen.SolidOrSlopedTile(tile6, includePlatforms: true) || tile6.liquid > 0;
					if (!num12 && !flag4 && !flag5 && !flag6)
					{
						point.Y += num4;
						if (WorldGen.Skyblock.lowTiles && point.Y >= num3)
						{
							point.Y = y;
							point.X += crawlOffsetX;
							num10++;
						}
						continue;
					}
				}
				if (IsInSolidTilesExtended(new Vector2(point.X * 16 + 8, point.Y * 16 + 15) - vector, player.velocity, player.width, player.height, (int)player.gravDir))
				{
					if (flag)
					{
						point.Y += num4;
					}
					else
					{
						point.Y += num5;
					}
					continue;
				}
				if (flag2)
				{
					if (flag)
					{
						point.Y += num4;
					}
					else
					{
						point.Y += num5;
					}
					continue;
				}
				flag = false;
				if (!IsInSolidTilesExtended(new Vector2(point.X * 16 + 8, point.Y * 16 + 15 + 16) - vector, player.velocity, player.width, player.height, (int)player.gravDir) && !flag3 && (double)point.Y < Main.worldSurface)
				{
					point.Y += num4;
					if (WorldGen.Skyblock.lowTiles && point.Y >= num3)
					{
						point.Y = y;
						point.X += crawlOffsetX;
						num10++;
					}
					continue;
				}
				if (tile2.liquid > 0 && !WorldGen.SolidOrSlopedTile(point.X, point.Y + num6, includePlatforms: true))
				{
					point.X += crawlOffsetX;
					num10++;
					continue;
				}
				if (TileIsDangerous(point.X - crawlOffsetX, point.Y, player) || TileIsDangerous(point.X, point.Y, player))
				{
					point.X += crawlOffsetX;
					num10++;
					continue;
				}
				if (TileIsDangerous(point.X - crawlOffsetX, point.Y + num6, player) || TileIsDangerous(point.X, point.Y + num6, player))
				{
					point.X += crawlOffsetX;
					num10++;
					continue;
				}
				if (point.Y < num9)
				{
					point.Y += num4;
					continue;
				}
				if (!WorldGen.Skyblock.lowTiles && !WorldGen.SolidOrSlopedTile(point.X, point.Y + num6, includePlatforms: true))
				{
					point.X += crawlOffsetX;
					num10++;
				}
				break;
			}
			if (num7 == num8 || num10 >= num11)
			{
				return false;
			}
			if (!WorldGen.InWorld(point.X, point.Y, 40))
			{
				return false;
			}
			int num13 = 20;
			if (WorldGen.Skyblock.lowTiles)
			{
				num13 = 10;
			}
			bool flag7 = false;
			Tile tile7 = Main.tile[point.X, point.Y];
			if (WorldGen.SolidOrSlopedTile(tile7, includePlatforms: true) || tile7.liquid > 0)
			{
				flag7 = true;
			}
			if (!flag7)
			{
				for (int i = 0; i < num13; i++)
				{
					int num14 = point.Y + i;
					Tile tile8 = Main.tile[point.X, num14];
					if (WorldGen.SolidOrSlopedTile(tile8, includePlatforms: true) || tile8.liquid > 0)
					{
						flag7 = true;
						point.Y += Math.Max(0, i - 1);
						break;
					}
				}
			}
			if (WorldGen.Skyblock.lowTiles)
			{
				if (!flag7)
				{
					for (int j = 0; j < num13; j++)
					{
						int num15 = point.Y + j;
						Tile tile9 = Main.tile[point.X - 1, num15];
						if (WorldGen.SolidOrSlopedTile(tile9, includePlatforms: true) || tile9.liquid > 0)
						{
							flag7 = true;
							point.X--;
							point.Y += Math.Max(0, j - 1);
							break;
						}
					}
				}
				if (!flag7)
				{
					for (int k = 0; k < num13; k++)
					{
						int num16 = point.Y + k;
						Tile tile10 = Main.tile[point.X + 1, num16];
						if (WorldGen.SolidOrSlopedTile(tile10, includePlatforms: true) || tile10.liquid > 0)
						{
							flag7 = true;
							point.X++;
							point.Y += Math.Max(0, k - 1);
							break;
						}
					}
				}
			}
			if (!flag7)
			{
				return false;
			}
			landingPoint = point;
			return true;
		}

		private static bool TileIsDangerous(int x, int y, Player player)
		{
			Tile tile = Main.tile[x, y];
			if (tile.liquid > 0 && tile.lava())
			{
				return true;
			}
			if (tile.wall == 87 && (double)y > Main.worldSurface && !NPC.downedPlantBoss)
			{
				return true;
			}
			if (Main.wallDungeon[tile.wall] && (double)y > Main.worldSurface && !NPC.downedBoss3)
			{
				return true;
			}
			if (tile.active() && Collision.CanTileHurt(tile.type, x, y, player))
			{
				return true;
			}
			return false;
		}

		private static bool IsInSolidTilesExtended(Vector2 testPosition, Vector2 playerVelocity, int width, int height, int gravDir)
		{
			if (Collision.LavaCollision(testPosition, width, height))
			{
				return true;
			}
			if (Collision.AnyHurtingTiles(testPosition, width, height))
			{
				return true;
			}
			if (Collision.SolidCollision(testPosition, width, height))
			{
				return true;
			}
			Vector2 vector = Vector2.UnitX * 16f;
			if (Collision.TileCollision(testPosition - vector, vector, width, height, fallThrough: true, fall2: true, gravDir) != vector)
			{
				return true;
			}
			vector = -Vector2.UnitX * 16f;
			if (Collision.TileCollision(testPosition - vector, vector, width, height, fallThrough: true, fall2: true, gravDir) != vector)
			{
				return true;
			}
			vector = Vector2.UnitY * 16f;
			if (Collision.TileCollision(testPosition - vector, vector, width, height, fallThrough: true, fall2: true, gravDir) != vector)
			{
				return true;
			}
			vector = -Vector2.UnitY * 16f;
			if (Collision.TileCollision(testPosition - vector, vector, width, height, fallThrough: true, fall2: true, gravDir) != vector)
			{
				return true;
			}
			return false;
		}
	}
}
