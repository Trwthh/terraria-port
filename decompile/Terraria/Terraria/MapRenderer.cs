using System;
using System.Collections.Generic;
using System.Diagnostics;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.DataStructures;
using Terraria.GameContent;
using Terraria.Map;
using Terraria.WorldBuilding;

namespace Terraria
{
	public class MapRenderer
	{
		public static readonly int textureMaxWidth;

		public static readonly int textureMaxHeight;

		private static readonly int numTargetsX;

		private static readonly int numTargetsY;

		private static readonly Color[] _mapColorCacheArray;

		private static RenderTarget2D mapSectionTexture;

		private static readonly RenderTarget2D[,] mapTarget;

		private static readonly bool[,] initMap;

		private static readonly bool[,] mapWasContentLost;

		private static readonly List<Point16>[,] changeQueues;

		private static readonly int ChangeRefreshThreshold;

		public static bool ChangesQueued
		{
			get
			{
				List<Point16>[,] array = changeQueues;
				int upperBound = array.GetUpperBound(0);
				int upperBound2 = array.GetUpperBound(1);
				for (int i = array.GetLowerBound(0); i <= upperBound; i++)
				{
					for (int j = array.GetLowerBound(1); j <= upperBound2; j++)
					{
						if (array[i, j].Count > 0)
						{
							return true;
						}
					}
				}
				return false;
			}
		}

		private static GraphicsDevice GraphicsDevice => Main.instance.GraphicsDevice;

		static MapRenderer()
		{
			textureMaxWidth = 2000;
			textureMaxHeight = 1800;
			numTargetsX = 5;
			numTargetsY = 2;
			_mapColorCacheArray = new Color[30000];
			mapTarget = new RenderTarget2D[numTargetsX, numTargetsY];
			initMap = new bool[numTargetsX, numTargetsY];
			mapWasContentLost = new bool[numTargetsX, numTargetsY];
			changeQueues = new List<Point16>[numTargetsX, numTargetsY];
			ChangeRefreshThreshold = 2048;
			for (int i = 0; i < numTargetsX; i++)
			{
				for (int j = 0; j < numTargetsY; j++)
				{
					changeQueues[i, j] = new List<Point16>(ChangeRefreshThreshold);
				}
			}
		}

		private static bool checkMap(int i, int j)
		{
			if (mapTarget[i, j] == null || mapTarget[i, j].IsDisposed)
			{
				initMap[i, j] = false;
			}
			if (initMap[i, j])
			{
				return true;
			}
			try
			{
				int width = textureMaxWidth;
				int height = textureMaxHeight;
				if (i == numTargetsX - 1)
				{
					width = 400;
				}
				if (j == numTargetsY - 1)
				{
					height = 600;
				}
				mapTarget[i, j] = new RenderTarget2D(GraphicsDevice, width, height, mipMap: false, GraphicsDevice.PresentationParameters.BackBufferFormat, DepthFormat.None, 0, RenderTargetUsage.PreserveContents);
				initMap[i, j] = true;
				return true;
			}
			catch
			{
				Main.mapEnabled = false;
				for (int k = 0; k < numTargetsX; k++)
				{
					for (int l = 0; l < numTargetsY; l++)
					{
						try
						{
							initMap[k, l] = false;
							mapTarget[k, l].Dispose();
						}
						catch
						{
						}
					}
				}
				return false;
			}
		}

		public static void DrawToMap(Rectangle area)
		{
			if (!Main.mapEnabled)
			{
				return;
			}
			area = WorldUtils.ClampToWorld(area);
			int num = Main.maxTilesX / textureMaxWidth;
			int num2 = Main.maxTilesY / textureMaxHeight;
			for (int i = 0; i <= num; i++)
			{
				for (int j = 0; j <= num2; j++)
				{
					if (!checkMap(i, j))
					{
						return;
					}
				}
			}
			TimeLogger.StartTimestamp fromTimestamp = TimeLogger.Start();
			if (Main.clearMap || Main.refreshMap)
			{
				List<Point16>[,] array = changeQueues;
				int upperBound = array.GetUpperBound(0);
				int upperBound2 = array.GetUpperBound(1);
				for (int k = array.GetLowerBound(0); k <= upperBound; k++)
				{
					for (int l = array.GetLowerBound(1); l <= upperBound2; l++)
					{
						array[k, l].Clear();
					}
				}
			}
			if (Main.clearMap)
			{
				for (int m = 0; m <= num; m++)
				{
					for (int n = 0; n <= num2; n++)
					{
						GraphicsDevice.SetRenderTarget(mapTarget[m, n]);
						GraphicsDevice.Clear(Color.Transparent);
						GraphicsDevice.SetRenderTarget(null);
					}
				}
				Main.clearMap = false;
			}
			RenderTarget2D activeTarget = null;
			int updateCount = 0;
			for (int num3 = 0; num3 < numTargetsX; num3++)
			{
				if (updateCount >= Main.maxMapUpdates)
				{
					break;
				}
				for (int num4 = 0; num4 < numTargetsY; num4++)
				{
					if (updateCount >= Main.maxMapUpdates)
					{
						break;
					}
					Rectangle value = new Rectangle(num3 * textureMaxWidth, num4 * textureMaxHeight, textureMaxWidth, textureMaxHeight);
					DrawAreaToMap(Rectangle.Intersect(area, value), num3, num4, ref activeTarget, ref updateCount);
				}
			}
			if (activeTarget != null)
			{
				Main.spriteBatch.End();
				GraphicsDevice.SetRenderTarget(null);
			}
			Main.mapReady = true;
			Main.loadMapLastX = 0;
			Main.loadMap = false;
			Main.loadMapLock = false;
			TimeLogger.MapUpdate.AddTime(fromTimestamp);
		}

		private static void DrawAreaToMap(Rectangle area, int rX, int rY, ref RenderTarget2D activeTarget, ref int updateCount)
		{
			RenderTarget2D renderTarget2D = mapTarget[rX, rY];
			if (renderTarget2D == null || renderTarget2D.IsContentLost)
			{
				return;
			}
			for (int i = area.Left; i < area.Right; i++)
			{
				for (int j = area.Top; j < area.Bottom; j++)
				{
					MapTile mapTile = Main.Map[i, j];
					if (mapTile.IsChanged)
					{
						if (updateCount++ >= Main.maxMapUpdates)
						{
							return;
						}
						if (Main.loadMap)
						{
							Main.loadMapLastX = i;
						}
						DrawChangesToMap(rX, rY, ref activeTarget, renderTarget2D, i, ref j, mapTile);
					}
				}
			}
			List<Point16> list = changeQueues[rX, rY];
			for (int k = 0; k < list.Count; k++)
			{
				Point16 point = list[k];
				MapTile mapTile2 = Main.Map[point.X, point.Y];
				if (mapTile2.IsChanged)
				{
					if (updateCount++ >= Main.maxMapUpdates)
					{
						list.RemoveRange(0, k);
						return;
					}
					int j2 = point.Y;
					DrawChangesToMap(rX, rY, ref activeTarget, renderTarget2D, point.X, ref j2, mapTile2);
				}
			}
			list.Clear();
		}

		private static void DrawChangesToMap(int rX, int rY, ref RenderTarget2D activeTarget, RenderTarget2D target, int i, ref int j, MapTile mapTile)
		{
			Main.Map.ConsumeUpdate(i, j);
			if (target != activeTarget)
			{
				if (activeTarget != null)
				{
					Main.spriteBatch.End();
				}
				activeTarget = target;
				GraphicsDevice.SetRenderTarget(target);
				Main.spriteBatch.Begin();
			}
			int num = i - rX * textureMaxWidth;
			int num2 = j - rY * textureMaxHeight;
			Color mapTileXnaColor = MapHelper.GetMapTileXnaColor(mapTile, i, j);
			int num3 = 1;
			int num4 = 1;
			int num5 = j + 1;
			while (true)
			{
				MapTile other;
				MapTile mapTile2 = (other = Main.Map[i, num5]);
				if (!mapTile2.IsChanged || !mapTile.Equals(other) || num5 / textureMaxHeight != rY)
				{
					break;
				}
				Main.Map.ConsumeUpdate(i, num5);
				num3++;
				num5++;
				j++;
			}
			if (num3 == 1)
			{
				num5 = i + 1;
				while (true)
				{
					MapTile other;
					MapTile mapTile2 = (other = Main.Map[num5, j]);
					if (!mapTile2.IsChanged || !mapTile.Equals(other) || num5 / textureMaxWidth != rX)
					{
						break;
					}
					Main.Map.ConsumeUpdate(num5, j);
					num4++;
					num5++;
				}
			}
			Main.spriteBatch.Draw(TextureAssets.MagicPixel.Value, new Vector2(num, num2), new Rectangle(0, 0, num4, num3), mapTileXnaColor, 0f, default(Vector2), 1f, SpriteEffects.None, 0f);
		}

		public static void QueueChange(int x, int y)
		{
			if (!Main.refreshMap)
			{
				int num = x / textureMaxWidth;
				int num2 = y / textureMaxHeight;
				List<Point16> list = changeQueues[num, num2];
				if (list.Count >= ChangeRefreshThreshold)
				{
					Main.refreshMap = true;
				}
				else
				{
					list.Add(new Point16(x, y));
				}
			}
		}

		public static void DrawToMap_Section(int secX, int secY)
		{
			if (mapSectionTexture == null)
			{
				mapSectionTexture = new RenderTarget2D(GraphicsDevice, 200, 150);
			}
			Stopwatch stopwatch = Stopwatch.StartNew();
			Color[] mapColorCacheArray = _mapColorCacheArray;
			int num = secX * 200;
			int num2 = num + 200;
			int num3 = secY * 150;
			int num4 = num3 + 150;
			int num5 = num / textureMaxWidth;
			int num6 = num3 / textureMaxHeight;
			int num7 = num % textureMaxWidth;
			int num8 = num3 % textureMaxHeight;
			if (!checkMap(num5, num6))
			{
				return;
			}
			int num9 = 0;
			_ = Color.Transparent;
			for (int i = num3; i < num4; i++)
			{
				for (int j = num; j < num2; j++)
				{
					MapTile tile = Main.Map[j, i];
					mapColorCacheArray[num9] = MapHelper.GetMapTileXnaColor(tile, j, i);
					num9++;
				}
			}
			try
			{
				GraphicsDevice.SetRenderTarget(mapTarget[num5, num6]);
			}
			catch (ObjectDisposedException)
			{
				initMap[num5, num6] = false;
				return;
			}
			Main.spriteBatch.Begin(SpriteSortMode.Immediate, BlendState.AlphaBlend);
			double totalMilliseconds = stopwatch.Elapsed.TotalMilliseconds;
			mapSectionTexture.SetData(mapColorCacheArray, 0, mapColorCacheArray.Length);
			_ = stopwatch.Elapsed.TotalMilliseconds;
			totalMilliseconds = stopwatch.Elapsed.TotalMilliseconds;
			Main.spriteBatch.Draw(mapSectionTexture, new Vector2(num7, num8), Color.White);
			Main.spriteBatch.End();
			GraphicsDevice.SetRenderTarget(null);
			_ = stopwatch.Elapsed.TotalMilliseconds;
			stopwatch.Stop();
		}

		public static void CheckMapTargets()
		{
			for (int i = 0; i < numTargetsX; i++)
			{
				for (int j = 0; j < numTargetsY; j++)
				{
					if (mapTarget[i, j] != null)
					{
						if (mapTarget[i, j].IsContentLost && !mapWasContentLost[i, j])
						{
							mapWasContentLost[i, j] = true;
							Main.refreshMap = true;
							Main.clearMap = true;
						}
						else if (!mapTarget[i, j].IsContentLost && mapWasContentLost[i, j])
						{
							mapWasContentLost[i, j] = false;
						}
					}
				}
			}
		}

		public static void DrawMap(float X, float Y, float minSizeX, float maxSizeX, float minSizeY, float maxSizeY, float xOff, float yOff, float scale, byte alpha)
		{
			_ = Main.maxTilesX / textureMaxWidth;
			int num = Main.maxTilesY / textureMaxHeight;
			float num2 = (float)textureMaxWidth * scale;
			float num3 = (float)textureMaxHeight * scale;
			float num4 = X;
			float num5 = 0f;
			for (int i = 0; i <= 4; i++)
			{
				if ((float)((i + 1) * textureMaxWidth) <= minSizeX || (float)(i * textureMaxWidth) >= minSizeX + maxSizeX)
				{
					continue;
				}
				for (int j = 0; j <= num; j++)
				{
					if ((float)((j + 1) * textureMaxHeight) > minSizeY && (float)(j * textureMaxHeight) < minSizeY + maxSizeY)
					{
						float num6 = X + (float)(int)((float)i * num2);
						float num7 = Y + (float)(int)((float)j * num3);
						float num8 = i * textureMaxWidth;
						float num9 = j * textureMaxHeight;
						float num10 = 0f;
						float num11 = 0f;
						if (num8 < minSizeX)
						{
							num10 = minSizeX - num8;
							num6 = X;
						}
						else
						{
							num6 -= minSizeX * scale;
						}
						if (num9 < minSizeY)
						{
							num11 = minSizeY - num9;
							num7 = Y;
						}
						else
						{
							num7 -= minSizeY * scale;
						}
						num6 = num4;
						float num12 = textureMaxWidth;
						float num13 = textureMaxHeight;
						float num14 = (i + 1) * textureMaxWidth;
						float num15 = (j + 1) * textureMaxHeight;
						if (num14 >= maxSizeX)
						{
							num12 -= num14 - maxSizeX;
						}
						if (num15 >= maxSizeY)
						{
							num13 -= num15 - maxSizeY;
						}
						if (num12 > num10)
						{
							Rectangle value = new Rectangle((int)num10, (int)num11, (int)num12 - (int)num10, (int)num13 - (int)num11);
							Main.spriteBatch.Draw(mapTarget[i, j], new Vector2(num6 + xOff, num7 + yOff), value, new Color(alpha, alpha, alpha, alpha), 0f, default(Vector2), scale, SpriteEffects.None, 0f);
						}
						num5 = (float)((int)num12 - (int)num10) * scale;
					}
					if (j == num)
					{
						num4 += num5;
					}
				}
			}
		}
	}
}
