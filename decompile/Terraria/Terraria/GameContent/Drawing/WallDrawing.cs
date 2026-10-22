using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.GameContent.Liquid;
using Terraria.Graphics;
using Terraria.ID;
using Terraria.Testing;

namespace Terraria.GameContent.Drawing
{
	public class WallDrawing : TileDrawingBase
	{
		public static bool QuickPaintLookup = true;

		private static VertexColors _glowPaintColors = new VertexColors(Color.White);

		private Tile[,] _tileArray;

		private TilePaintSystemV2 _paintSystem;

		private bool _shouldShowInvisibleWalls;

		private DrawBlackHelper drawBlackHelper;

		private TilePaintSystemV2.WallVariationKey _lastPaintLookupKey;

		private Texture2D _lastPaintLookupTexture;

		public void LerpVertexColorsWithColor(ref VertexColors colors, Color lerpColor, float percent)
		{
			colors.TopLeftColor = Color.Lerp(colors.TopLeftColor, lerpColor, percent);
			colors.TopRightColor = Color.Lerp(colors.TopRightColor, lerpColor, percent);
			colors.BottomLeftColor = Color.Lerp(colors.BottomLeftColor, lerpColor, percent);
			colors.BottomRightColor = Color.Lerp(colors.BottomRightColor, lerpColor, percent);
		}

		public WallDrawing(TilePaintSystemV2 paintSystem)
		{
			_paintSystem = paintSystem;
		}

		public void Update()
		{
			if (!Main.dedServ)
			{
				_shouldShowInvisibleWalls = Main.ShouldShowInvisibleBlocksAndWalls();
			}
		}

		public static void DrawOutline(Texture2D texture, Vector2 position, Rectangle sourceRectangle, Color color)
		{
			Main.spriteBatch.Draw(texture, position, sourceRectangle, color, 0f, Vector2.Zero, 1f, SpriteEffects.None, 0f);
		}

		public void DrawWalls()
		{
			FlushLogData = TimeLogger.FlushWallTiles;
			DrawCallLogData = TimeLogger.WallDrawCalls;
			if (DebugOptions.hideWalls)
			{
				return;
			}
			float gfxQuality = Main.gfxQuality;
			Vector2 screenPosition = Main.screenPosition;
			int[] wallBlend = Main.wallBlend;
			_tileArray = Main.tile;
			int num = (int)(120f * (1f - gfxQuality) + 40f * gfxQuality);
			if (DebugOptions.devLightTilesCheat)
			{
				num = 1000;
			}
			int num2 = (int)((float)num * 0.4f);
			int num3 = (int)((float)num * 0.35f);
			int num4 = (int)((float)num * 0.3f);
			TileDrawing.GetScreenDrawArea(!Main.drawToScreen, out var drawOffSet, out var firstTileX, out var lastTileX, out var firstTileY, out var lastTileY);
			VertexColors vertices = default(VertexColors);
			Rectangle sourceRectangle = new Rectangle(0, 0, 32, 32);
			int underworldLayer = Main.UnderworldLayer;
			drawBlackHelper = new DrawBlackHelper(0u, drawOffSet);
			_lastPaintLookupKey = default(TilePaintSystemV2.WallVariationKey);
			for (int i = firstTileY; i < lastTileY; i++)
			{
				for (int j = firstTileX; j < lastTileX; j++)
				{
					Tile tile = _tileArray[j, i];
					if (tile == null)
					{
						tile = new Tile();
						_tileArray[j, i] = tile;
					}
					ushort wall = tile.wall;
					if (wall <= 0 || FullTile(j, i) || (wall == 318 && !_shouldShowInvisibleWalls) || (tile.invisibleWall() && !_shouldShowInvisibleWalls))
					{
						continue;
					}
					Color color = Lighting.GetColor(j, i);
					if (tile.fullbrightWall())
					{
						color = Color.White;
					}
					if (wall == 318)
					{
						color = Color.White;
					}
					if (TileDrawingBase.DrawOwnBlacks)
					{
						if (color.R == 0 && color.G == 0 && color.B == 0)
						{
							drawBlackHelper.DrawBlack(j, i);
							continue;
						}
					}
					else if (color.R == 0 && color.G == 0 && color.B == 0 && i < underworldLayer)
					{
						continue;
					}
					Main.instance.LoadWall(wall);
					Texture2D wallDrawTexture = GetWallDrawTexture(tile);
					Main.tileBatch.SetLayer((uint)(((tile.active() ? 1 : 0) << 21) | (tile.wallColor() << 16) | wall), 0);
					sourceRectangle.X = tile.wallFrameX();
					sourceRectangle.Y = tile.wallFrameY() + Main.wallFrame[wall] * 180;
					ushort wall2 = tile.wall;
					if ((uint)(wall2 - 242) <= 1u)
					{
						int num5 = 20;
						int num6 = (Main.wallFrameCounter[wall] + j * 11 + i * 27) % (num5 * 8);
						sourceRectangle.Y = tile.wallFrameY() + 180 * (num6 / num5);
					}
					if (Lighting.NotRetro && !Main.wallLight[wall] && tile.wall != 241 && (tile.wall < 88 || tile.wall > 93) && !WorldGen.SolidTile(tile))
					{
						if (tile.wall == 346)
						{
							vertices.TopRightColor = (vertices.TopLeftColor = (vertices.BottomRightColor = (vertices.BottomLeftColor = new Color((byte)Main.DiscoR, (byte)Main.DiscoG, (byte)Main.DiscoB))));
						}
						else if (tile.wall == 44)
						{
							vertices.TopRightColor = (vertices.TopLeftColor = (vertices.BottomRightColor = (vertices.BottomLeftColor = new Color((byte)Main.DiscoR, (byte)Main.DiscoG, (byte)Main.DiscoB))));
						}
						else
						{
							Lighting.GetCornerColors(j, i, out vertices);
							wall2 = tile.wall;
							if ((uint)(wall2 - 341) <= 4u)
							{
								LerpVertexColorsWithColor(ref vertices, Color.White, 0.5f);
							}
							if (tile.fullbrightWall())
							{
								vertices = _glowPaintColors;
							}
						}
						Main.tileBatch.Draw(wallDrawTexture, new Vector2(j * 16 - (int)screenPosition.X - 8, i * 16 - (int)screenPosition.Y - 8) + drawOffSet, sourceRectangle, vertices, Vector2.Zero);
						if (tile.wall == 347)
						{
							Texture2D value = TextureAssets.GlowMask[361].Value;
							LiquidRenderer.SetShimmerVertexColors_Sparkle(ref vertices, 0.7f, j, i, top: true);
							Main.tileBatch.Draw(value, new Vector2(j * 16 - (int)screenPosition.X - 8, i * 16 - (int)screenPosition.Y - 8) + drawOffSet, sourceRectangle, vertices, Vector2.Zero);
						}
					}
					else
					{
						Color color2 = color;
						if (wall == 44 || wall == 346)
						{
							color2 = new Color(Main.DiscoR, Main.DiscoG, Main.DiscoB);
						}
						if ((uint)(wall - 341) <= 4u)
						{
							color2 = Color.Lerp(color2, Color.White, 0.5f);
						}
						Main.tileBatch.Draw(wallDrawTexture, new Vector2(j * 16 - (int)screenPosition.X - 8, i * 16 - (int)screenPosition.Y - 8) + drawOffSet, sourceRectangle, color2, Vector2.Zero);
						if (tile.wall == 347)
						{
							Texture2D value2 = TextureAssets.GlowMask[361].Value;
							Color color3 = LiquidRenderer.GetShimmerGlitterColor(top: true, j, i) * 0.7f;
							Main.tileBatch.Draw(value2, new Vector2(j * 16 - (int)screenPosition.X - 8, i * 16 - (int)screenPosition.Y - 8) + drawOffSet, sourceRectangle, color3, Vector2.Zero);
						}
					}
					if (color.R > num2 || color.G > num3 || color.B > num4)
					{
						bool num7 = _tileArray[j - 1, i].wall > 0 && wallBlend[_tileArray[j - 1, i].wall] != wallBlend[tile.wall];
						bool flag = _tileArray[j + 1, i].wall > 0 && wallBlend[_tileArray[j + 1, i].wall] != wallBlend[tile.wall];
						bool flag2 = _tileArray[j, i - 1].wall > 0 && wallBlend[_tileArray[j, i - 1].wall] != wallBlend[tile.wall];
						bool flag3 = _tileArray[j, i + 1].wall > 0 && wallBlend[_tileArray[j, i + 1].wall] != wallBlend[tile.wall];
						if (num7)
						{
							DrawOutline(TextureAssets.WallOutline.Value, new Vector2(j * 16 - (int)screenPosition.X, i * 16 - (int)screenPosition.Y) + drawOffSet, new Rectangle(0, 0, 2, 16), color);
						}
						if (flag)
						{
							DrawOutline(TextureAssets.WallOutline.Value, new Vector2(j * 16 - (int)screenPosition.X + 14, i * 16 - (int)screenPosition.Y) + drawOffSet, new Rectangle(14, 0, 2, 16), color);
						}
						if (flag2)
						{
							DrawOutline(TextureAssets.WallOutline.Value, new Vector2(j * 16 - (int)screenPosition.X, i * 16 - (int)screenPosition.Y) + drawOffSet, new Rectangle(0, 0, 16, 2), color);
						}
						if (flag3)
						{
							DrawOutline(TextureAssets.WallOutline.Value, new Vector2(j * 16 - (int)screenPosition.X, i * 16 - (int)screenPosition.Y + 14) + drawOffSet, new Rectangle(0, 14, 16, 2), color);
						}
					}
				}
			}
			drawBlackHelper.EndStrip();
			RestartLayeredBatch();
			Main.instance.DrawTileCracks(2, Main.LocalPlayer.hitReplace);
			Main.instance.DrawTileCracks(2, Main.LocalPlayer.hitTile);
		}

		public Texture2D GetWallDrawTexture(Tile tile)
		{
			return GetWallDrawTexture(tile.wall, tile.wallColor());
		}

		public Texture2D GetWallDrawTexture(int wallType, int paintColor)
		{
			TilePaintSystemV2.WallVariationKey wallVariationKey = new TilePaintSystemV2.WallVariationKey
			{
				WallType = wallType,
				PaintColor = paintColor
			};
			if (_lastPaintLookupKey == wallVariationKey)
			{
				return _lastPaintLookupTexture;
			}
			_lastPaintLookupKey = wallVariationKey;
			_lastPaintLookupTexture = LookupWallDrawTexture(wallVariationKey);
			return _lastPaintLookupTexture;
		}

		private Texture2D LookupWallDrawTexture(TilePaintSystemV2.WallVariationKey key)
		{
			if (key.PaintColor != 0)
			{
				Texture2D texture2D = _paintSystem.TryGetWallAndRequestIfNotReady(key.WallType, key.PaintColor);
				if (texture2D != null)
				{
					return texture2D;
				}
			}
			return TextureAssets.Wall[key.WallType].Value;
		}

		protected bool FullTile(int x, int y)
		{
			if (_tileArray[x - 1, y] == null || _tileArray[x - 1, y].blockType() != 0 || _tileArray[x + 1, y] == null || _tileArray[x + 1, y].blockType() != 0)
			{
				return false;
			}
			Tile tile = _tileArray[x, y];
			if (tile == null)
			{
				return false;
			}
			if (tile.active())
			{
				if (Main.tileFrameImportant[tile.type] || TileID.Sets.DrawsWalls[tile.type])
				{
					return false;
				}
				if (tile.invisibleBlock() && !_shouldShowInvisibleWalls)
				{
					return false;
				}
				if (DebugOptions.ShowUnbreakableWall && tile.wall == 350)
				{
					return false;
				}
				if (tile.type == 740)
				{
					short frameX = tile.frameX;
					short frameY = tile.frameY;
					if ((frameX == 180 || frameX == 198) && frameY >= 0 && frameY <= 36)
					{
						return false;
					}
					if (frameX >= 108 && frameX <= 144 && (frameY == 18 || frameY == 36))
					{
						return false;
					}
				}
				if (Main.tileSolid[tile.type] && !Main.tileSolidTop[tile.type])
				{
					int frameX2 = tile.frameX;
					int frameY2 = tile.frameY;
					if (Main.tileLargeFrames[tile.type] > 0)
					{
						if (frameY2 == 18 || frameY2 == 108)
						{
							if (frameX2 >= 18 && frameX2 <= 54)
							{
								return true;
							}
							if (frameX2 >= 108 && frameX2 <= 144)
							{
								return true;
							}
						}
					}
					else
					{
						switch (frameY2)
						{
						case 0:
							if (frameX2 >= 180 && frameX2 <= 198)
							{
								return true;
							}
							break;
						case 18:
							if (frameX2 >= 18 && frameX2 <= 54)
							{
								return true;
							}
							if (frameX2 >= 108 && frameX2 <= 144)
							{
								return true;
							}
							if (frameX2 >= 180 && frameX2 <= 198)
							{
								return true;
							}
							break;
						case 36:
							if (frameX2 >= 108 && frameX2 <= 144)
							{
								return true;
							}
							if (frameX2 >= 180 && frameX2 <= 198)
							{
								return true;
							}
							break;
						case 90:
						case 91:
						case 92:
						case 93:
						case 94:
						case 95:
						case 96:
						case 97:
						case 98:
						case 99:
						case 100:
						case 101:
						case 102:
						case 103:
						case 104:
						case 105:
						case 106:
						case 107:
						case 108:
						case 109:
						case 110:
						case 111:
						case 112:
						case 113:
						case 114:
						case 115:
						case 116:
						case 117:
						case 118:
						case 119:
						case 120:
						case 121:
						case 122:
						case 123:
						case 124:
						case 125:
						case 126:
						case 127:
						case 128:
						case 129:
						case 130:
						case 131:
						case 132:
						case 133:
						case 134:
						case 135:
						case 136:
						case 137:
						case 138:
						case 139:
						case 140:
						case 141:
						case 142:
						case 143:
						case 144:
						case 145:
						case 146:
						case 147:
						case 148:
						case 149:
						case 150:
						case 151:
						case 152:
						case 153:
						case 154:
						case 155:
						case 156:
						case 157:
						case 158:
						case 159:
						case 160:
						case 161:
						case 162:
						case 163:
						case 164:
						case 165:
						case 166:
						case 167:
						case 168:
						case 169:
						case 170:
						case 171:
						case 172:
						case 173:
						case 174:
						case 175:
						case 176:
						case 177:
						case 178:
						case 179:
						case 180:
							if (frameX2 <= 54)
							{
								return true;
							}
							if (frameX2 >= 144 && frameX2 <= 216)
							{
								return true;
							}
							break;
						default:
							if (frameY2 == 198 && frameX2 >= 108 && frameX2 <= 144)
							{
								return true;
							}
							break;
						}
					}
				}
			}
			return false;
		}
	}
}
