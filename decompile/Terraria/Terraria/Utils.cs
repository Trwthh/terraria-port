using System;
using System.Collections;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text.RegularExpressions;
using System.Threading;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Microsoft.Xna.Framework.Graphics.PackedVector;
using Microsoft.Xna.Framework.Input;
using ReLogic.Content;
using ReLogic.Graphics;
using ReLogic.OS;
using ReLogic.Utilities;
using Terraria.DataStructures;
using Terraria.GameContent;
using Terraria.Localization;
using Terraria.UI;
using Terraria.UI.Chat;
using Terraria.Utilities;
using Terraria.Utilities.Terraria.Utilities;

namespace Terraria
{
	public static class Utils
	{
		public delegate bool TileActionAttempt(int x, int y);

		public delegate void LaserLineFraming(int stage, Vector2 currentPosition, float distanceLeft, Rectangle lastFrame, out float distanceCovered, out Rectangle frame, out Vector2 origin, out Color color);

		public delegate Color ColorLerpMethod(float percent);

		public class RandomTeleportationAttemptSettings
		{
			public Vector2 teleporteeSize;

			public Vector2 teleporteeVelocity;

			public float teleporteeGravityDirection;

			public bool mostlySolidFloor;

			public bool avoidLava;

			public bool avoidAnyLiquid;

			public bool avoidHurtTiles;

			public bool avoidWalls;

			public int attemptsBeforeGivingUp;

			public int maximumFallDistanceFromOrignalPoint;

			public bool strictRange;

			public int[] tilesToAvoid;

			public int tilesToAvoidRange;

			public bool allowSolidTopFloor;

			public Func<int, int, Tile, Tile, Tile, bool> specializedConditions;
		}

		public struct ChaseResults
		{
			public bool InterceptionHappens;

			public Vector2 InterceptionPosition;

			public float InterceptionTime;

			public Vector2 ChaserVelocity;
		}

		public static readonly int MaxFloatInt = 16777216;

		public const long MaxCoins = 9999999999L;

		public static Dictionary<DynamicSpriteFont, float[]> charLengths = new Dictionary<DynamicSpriteFont, float[]>();

		private static Regex _substitutionRegex = new Regex("{(\\?(?:!)?)?([a-zA-Z][\\w\\.]*)}", RegexOptions.Compiled);

		private const ulong RANDOM_MULTIPLIER = 25214903917uL;

		private const ulong RANDOM_ADD = 11uL;

		private const ulong RANDOM_MASK = 281474976710655uL;

		private static readonly List<Point> _floodFillQueue1 = new List<Point>(2500);

		private static readonly List<Point> _floodFillQueue2 = new List<Point>(2500);

		private static readonly BitSet2D _floodFillBitset = new BitSet2D();

		public static Color ColorLerp_BlackToWhite(float percent)
		{
			return Color.Lerp(Color.Black, Color.White, percent);
		}

		public static double Lerp(double value1, double value2, double amount)
		{
			return value1 + (value2 - value1) * amount;
		}

		public static Vector2 Round(Vector2 input)
		{
			return new Vector2((float)Math.Round(input.X), (float)Math.Round(input.Y));
		}

		public static bool IsPowerOfTwo(int x)
		{
			if (x != 0)
			{
				return (x & (x - 1)) == 0;
			}
			return false;
		}

		public static float SmoothStep(float min, float max, float x)
		{
			return MathHelper.Clamp((x - min) / (max - min), 0f, 1f);
		}

		public static double SmoothStep(double min, double max, double x)
		{
			return Clamp((x - min) / (max - min), 0.0, 1.0);
		}

		public static float UnclampedSmoothStep(float min, float max, float x)
		{
			return (x - min) / (max - min);
		}

		public static double UnclampedSmoothStep(double min, double max, double x)
		{
			return (x - min) / (max - min);
		}

		public static void CycleControlControl_ClickHold(ref ButtonControlMode mode)
		{
			switch (mode)
			{
			case ButtonControlMode.Hold:
				mode = ButtonControlMode.Click;
				break;
			default:
				mode = ButtonControlMode.Hold;
				break;
			}
		}

		public static void CycleControlControl_OffOnClickHold(ref ButtonControlMode mode)
		{
			switch (mode)
			{
			default:
				mode = ButtonControlMode.Hold;
				break;
			case ButtonControlMode.Hold:
				mode = ButtonControlMode.Click;
				break;
			case ButtonControlMode.Click:
				mode = ButtonControlMode.OnAlways;
				break;
			case ButtonControlMode.OnAlways:
				mode = ButtonControlMode.OffAlways;
				break;
			}
		}

		public static Dictionary<string, string> ParseArguements(string[] args)
		{
			string text = null;
			string text2 = "";
			Dictionary<string, string> dictionary = new Dictionary<string, string>();
			for (int i = 0; i < args.Length; i++)
			{
				if (args[i].Length == 0)
				{
					continue;
				}
				if (args[i][0] == '-' || args[i][0] == '+')
				{
					if (text != null)
					{
						dictionary.Add(text.ToLower(), text2);
						text2 = "";
					}
					text = args[i];
					text2 = "";
				}
				else
				{
					if (text2 != "")
					{
						text2 += " ";
					}
					text2 += args[i];
				}
			}
			if (text != null)
			{
				dictionary.Add(text.ToLower(), text2);
				text2 = "";
			}
			return dictionary;
		}

		public static void Swap<T>(ref T t1, ref T t2)
		{
			T val = t1;
			t1 = t2;
			t2 = val;
		}

		public static T Clamp<T>(T value, T min, T max) where T : IComparable<T>
		{
			if (value.CompareTo(max) > 0)
			{
				return max;
			}
			if (value.CompareTo(min) < 0)
			{
				return min;
			}
			return value;
		}

		public static Rectangle Clamp(Rectangle r, Rectangle bounds)
		{
			return new Rectangle(Clamp(r.X, bounds.Left, bounds.Right - r.Width), Clamp(r.Y, bounds.Top, bounds.Bottom - r.Height), r.Width, r.Height);
		}

		public static float Turn01ToCyclic010(float value)
		{
			return 1f - ((float)Math.Cos(value * ((float)Math.PI * 2f)) * 0.5f + 0.5f);
		}

		public static float PingPongFrom01To010(float value)
		{
			value %= 1f;
			if (value < 0f)
			{
				value += 1f;
			}
			if (value >= 0.5f)
			{
				return 2f - value * 2f;
			}
			return value * 2f;
		}

		public static void Shift<T>(T[] array, int n)
		{
			if (n == 0 || n >= array.Length || n <= -array.Length)
			{
				return;
			}
			if (n > 0)
			{
				if (n < array.Length)
				{
					Array.Copy(array, 0, array, n, array.Length - n);
				}
			}
			else if (n > -array.Length)
			{
				Array.Copy(array, -n, array, 0, array.Length + n);
			}
		}

		public static float MultiLerp(float percent, params float[] floats)
		{
			float num = 1f / ((float)floats.Length - 1f);
			float num2 = num;
			int num3 = 0;
			while (percent / num2 > 1f && num3 < floats.Length - 2)
			{
				num2 += num;
				num3++;
			}
			return MathHelper.Lerp(floats[num3], floats[num3 + 1], (percent - num * (float)num3) / num);
		}

		public static Color MultiLerp(float percent, params Color[] colors)
		{
			float num = 1f / ((float)colors.Length - 1f);
			float num2 = num;
			int num3 = 0;
			while (percent / num2 > 1f && num3 < colors.Length - 2)
			{
				num2 += num;
				num3++;
			}
			return Color.Lerp(colors[num3], colors[num3 + 1], (percent - num * (float)num3) / num);
		}

		public static float WrappedLerp(float value1, float value2, float percent)
		{
			float num = percent * 2f;
			if (num > 1f)
			{
				num = 2f - num;
			}
			return MathHelper.Lerp(value1, value2, num);
		}

		public static float GetLerpValue(float from, float to, float t, bool clamped = false)
		{
			if (clamped)
			{
				if (from < to)
				{
					if (t < from)
					{
						return 0f;
					}
					if (t > to)
					{
						return 1f;
					}
				}
				else
				{
					if (t < to)
					{
						return 1f;
					}
					if (t > from)
					{
						return 0f;
					}
				}
			}
			return (t - from) / (to - from);
		}

		public static float Remap(float fromValue, float fromMin, float fromMax, float toMin, float toMax, bool clamped = true)
		{
			return MathHelper.Lerp(toMin, toMax, GetLerpValue(fromMin, fromMax, fromValue, clamped));
		}

		public static double Remap(double fromValue, double fromMin, double fromMax, double toMin, double toMax, bool clamped = true)
		{
			return Lerp(toMin, toMax, GetLerpValue(fromMin, fromMax, fromValue, clamped));
		}

		public static double EaseOutBounce(double x)
		{
			return BounceEaseOut(x, 4, 2.0);
		}

		private static double BounceEaseOut(double t, int bounces, double elasticity)
		{
			double num = (double)bounces * Math.PI;
			double num2 = Math.Pow(1.0 - t, elasticity);
			double num3 = Math.Abs(Math.Sin(t * num));
			return 1.0 - num2 * num3;
		}

		public static double EaseInCirc(double x)
		{
			return 1.0 - Math.Sqrt(1.0 - x * x);
		}

		public static double EaseOutCirc(double x)
		{
			return Math.Sqrt(1.0 - Math.Pow(x - 1.0, 2.0));
		}

		public static void GetPortraitMovement(double t, out double offsetX, out double scaleX)
		{
			t %= 1.0;
			double num = 1.0 / 6.0;
			int num2 = (int)(t / num);
			double num3 = t % num / num;
			offsetX = 0.0;
			scaleX = 1.0;
			switch (num2)
			{
			case 0:
				offsetX = 0.0;
				scaleX = 1.0 - 2.0 * num3;
				break;
			case 1:
				offsetX = 0.0 - 0.5 * EaseOutCirc(num3);
				scaleX = -1.0;
				break;
			case 2:
				offsetX = -0.5 - 0.5 * EaseOutCirc(num3);
				scaleX = -1.0;
				break;
			case 3:
				offsetX = -1.0;
				scaleX = -1.0 + 2.0 * num3;
				break;
			case 4:
				offsetX = -1.0 + 0.5 * EaseOutCirc(num3);
				scaleX = 1.0;
				break;
			case 5:
				offsetX = -0.5 + 0.5 * EaseOutCirc(num3);
				scaleX = 1.0;
				break;
			}
		}

		public static Color ShiftHue(Color color, float hueShift, float luminanceShift, float saturationBoost)
		{
			Vector3 vector = Main.rgbToHsl(color);
			float num = (vector.X + hueShift) % 1f;
			if (num < 0f)
			{
				num += 1f;
			}
			return Main.hslToRgb(num, vector.Y + saturationBoost, Clamp(vector.Z + luminanceShift, 0f, 1f), color.A);
		}

		public static Color ShiftBlueToCyanTheme(Color color)
		{
			return ShiftHue(color, -0.04f, 0.04f, 0.2f);
		}

		public static void ClampWithinWorld(ref int minX, ref int minY, ref int maxX, ref int maxY, bool lastValuesInclusiveToIteration = false, int fluffX = 0, int fluffY = 0)
		{
			int num = (lastValuesInclusiveToIteration ? 1 : 0);
			minX = Clamp(minX, fluffX, Main.maxTilesX - num - fluffX);
			maxX = Clamp(maxX, fluffX, Main.maxTilesX - num - fluffX);
			minY = Clamp(minY, fluffY, Main.maxTilesY - num - fluffY);
			maxY = Clamp(maxY, fluffY, Main.maxTilesY - num - fluffY);
		}

		public static void DrawNotificationIcon(SpriteBatch spritebatch, Rectangle hitbox, float rotationMultiplier = 1f, bool worldSpace = false)
		{
			DrawNotificationIcon(spritebatch, hitbox.BottomRight() + new Vector2(-7f, -6f), rotationMultiplier, worldSpace);
		}

		public static void DrawNotificationIcon(SpriteBatch spritebatch, Vector2 position, float rotationMultiplier = 1f, bool worldSpace = false, float scaleMultiplier = 1f)
		{
			Texture2D value = Main.Assets.Request<Texture2D>("Images/UI/UI_quickicon1").Value;
			float amount = (float)Math.Sin((float)Math.PI * 2f * (Main.GlobalTimeWrappedHourly % 1f / 1f)) * 0.5f + 0.5f;
			Color color = Color.White;
			float num = (float)Math.Sin(Main.GlobalTimeWrappedHourly % 2f / 2f * ((float)Math.PI * 2f)) * ((float)Math.PI * 2f) * 0.035f * rotationMultiplier;
			if (worldSpace)
			{
				color = Lighting.GetColor(position.ToTileCoordinates());
				position -= Main.screenPosition;
				if (Main.LocalPlayer.gravDir == -1f)
				{
					num += (float)Math.PI;
					position = Main.ReverseGravitySupport(position);
				}
			}
			Color value2 = color;
			value2.A /= 2;
			Color color2 = Color.Lerp(color, value2, amount);
			spritebatch.Draw(value, position, null, color2, num, new Vector2(value.Width / 2, value.Height - 4), scaleMultiplier, SpriteEffects.None, 0f);
		}

		public static Vector2 ConstrainedToPointInRectangle(Rectangle bounds, Vector2 centerTestPosition)
		{
			if (bounds.Contains(centerTestPosition.ToPoint()))
			{
				return centerTestPosition;
			}
			Vector2 vector = new Vector2(bounds.Center.X, bounds.Center.Y);
			Vector2 vector2 = vector - centerTestPosition;
			float val = ((vector2.X == 0f) ? float.MaxValue : Math.Abs((vector.X - (float)(bounds.Width / 2) - centerTestPosition.X) / vector2.X));
			float val2 = ((vector2.Y == 0f) ? float.MaxValue : Math.Abs((vector.Y - (float)(bounds.Height / 2) - centerTestPosition.Y) / vector2.Y));
			float num = Math.Min(val, val2);
			Vector2 result = centerTestPosition + vector2 * num;
			result.X = MathHelper.Clamp(result.X, bounds.Left, bounds.Right);
			result.Y = MathHelper.Clamp(result.Y, bounds.Top, bounds.Bottom);
			return result;
		}

		private static bool CheckForGoodTeleportationSpot_CheckNoInvalidTiles(int tpx, int tpy, RandomTeleportationAttemptSettings settings)
		{
			if (settings.tilesToAvoidRange > 0 && settings.tilesToAvoid != null)
			{
				int tilesToAvoidRange = settings.tilesToAvoidRange;
				for (int i = -tilesToAvoidRange; i <= tilesToAvoidRange; i++)
				{
					for (int j = -tilesToAvoidRange; j <= tilesToAvoidRange; j++)
					{
						int num = tpx + i;
						int num2 = tpy + j;
						if (!WorldGen.InWorld(num, num2, 2))
						{
							continue;
						}
						Tile tile = Main.tile[num, num2];
						if (tile == null || !tile.active())
						{
							continue;
						}
						ushort type = tile.type;
						for (int k = 0; k < settings.tilesToAvoid.Length; k++)
						{
							if (type == settings.tilesToAvoid[k])
							{
								return false;
							}
						}
					}
				}
			}
			return true;
		}

		public static Vector2 CheckForGoodTeleportationSpot(ref bool canSpawn, int teleportStartX, int teleportRangeX, int teleportStartY, int teleportRangeY, RandomTeleportationAttemptSettings settings)
		{
			int num = (int)settings.teleporteeSize.X;
			int num2 = (int)settings.teleporteeSize.Y;
			Vector2 teleporteeVelocity = settings.teleporteeVelocity;
			float teleporteeGravityDirection = settings.teleporteeGravityDirection;
			Rectangle rectangle = new Rectangle(teleportStartX, teleportStartY, teleportRangeX + 1, teleportRangeY + 1);
			int num3 = 0;
			int num4 = 0;
			int num5 = 0;
			int num6 = num;
			Vector2 vector = new Vector2(num4, num5) * 16f + new Vector2(-num6 / 2 + 8, -num2);
			while (!canSpawn && num3 < settings.attemptsBeforeGivingUp)
			{
				num3++;
				num4 = Main.rand.Next(rectangle.Left, rectangle.Right + 1);
				num5 = Main.rand.Next(rectangle.Top, rectangle.Bottom + 1);
				int num7 = 45;
				num4 = (int)MathHelper.Clamp(num4, num7, Main.maxTilesX - num7);
				num5 = (int)MathHelper.Clamp(num5, num7, Main.maxTilesY - num7);
				if (settings.strictRange && !rectangle.Contains(new Point(num4, num5)))
				{
					continue;
				}
				vector = new Vector2(num4, num5) * 16f + new Vector2(-num6 / 2 + 8, -num2);
				if (Collision.SolidCollision(vector, num6, num2))
				{
					continue;
				}
				if (Main.tile[num4, num5] == null)
				{
					Main.tile[num4, num5] = new Tile();
				}
				Tile tile = Main.tile[num4, num5];
				if ((settings.avoidWalls && tile.wall > 0) || (tile.wall == 87 && !NPC.downedPlantBoss) || (Main.wallDungeon[tile.wall] && (double)num5 > Main.worldSurface && !NPC.downedBoss3) || !CheckForGoodTeleportationSpot_CheckNoInvalidTiles(num4, num5, settings))
				{
					continue;
				}
				bool flag = false;
				int num8 = 0;
				while (num8 < settings.maximumFallDistanceFromOrignalPoint)
				{
					if (settings.strictRange && !rectangle.Contains(new Point(num4, num5 + num8)))
					{
						flag = true;
						break;
					}
					if (Main.tile[num4, num5 + num8] == null)
					{
						Main.tile[num4, num5 + num8] = new Tile();
					}
					Tile tile2 = Main.tile[num4, num5 + num8];
					vector = new Vector2(num4, num5 + num8) * 16f + new Vector2(-num6 / 2 + 8, -num2);
					Collision.SlopeCollision(vector, teleporteeVelocity, num6, num2, teleporteeGravityDirection);
					if (!Collision.SolidCollision(vector, num6, num2 + 1, settings.allowSolidTopFloor))
					{
						num8++;
						continue;
					}
					if (tile2.active() && !tile2.inActive() && Main.tileSolid[tile2.type])
					{
						break;
					}
					num8++;
				}
				if (flag)
				{
					continue;
				}
				int num9 = (int)vector.X / 16;
				int num10 = (int)vector.Y / 16;
				if (!CheckForGoodTeleportationSpot_CheckNoInvalidTiles(num9, num10, settings))
				{
					continue;
				}
				int num11 = (int)(vector.X + (float)num6 * 0.5f) / 16;
				int num12 = (int)(vector.Y + (float)num2) / 16;
				Tile tileSafely = Framing.GetTileSafely(num9, num10);
				Tile tileSafely2 = Framing.GetTileSafely(num11, num12);
				Tile tileSafely3 = Framing.GetTileSafely(num11 - 1, num12);
				Tile tileSafely4 = Framing.GetTileSafely(num11 + 1, num12);
				if ((settings.specializedConditions != null && !settings.specializedConditions(num11, num12, tileSafely2, tileSafely3, tileSafely4)) || (settings.avoidAnyLiquid && tileSafely2.liquid > 0))
				{
					continue;
				}
				if (settings.mostlySolidFloor)
				{
					bool flag2 = false;
					bool flag3 = false;
					if (settings.allowSolidTopFloor)
					{
						flag2 = !tileSafely3.inActive() && WorldGen.SolidTileAllowBottomSlope(num11 - 1, num12);
						flag3 = !tileSafely4.inActive() && WorldGen.SolidTileAllowBottomSlope(num11 + 1, num12);
					}
					else
					{
						flag2 = tileSafely3.active() && !tileSafely3.inActive() && Main.tileSolid[tileSafely3.type] && !Main.tileSolidTop[tileSafely3.type];
						flag3 = tileSafely4.active() && !tileSafely4.inActive() && Main.tileSolid[tileSafely4.type] && !Main.tileSolidTop[tileSafely4.type];
					}
					if (!flag2 && !flag3)
					{
						continue;
					}
				}
				if ((settings.avoidWalls && tileSafely.wall > 0) || (settings.avoidAnyLiquid && Collision.WetCollision(vector, num6, num2)) || (settings.avoidLava && Collision.LavaCollision(vector, num6, num2)) || (settings.avoidHurtTiles && Collision.AnyHurtingTiles(vector, num6, num2)) || Collision.SolidCollision(vector, num6, num2, settings.allowSolidTopFloor) || num8 >= settings.maximumFallDistanceFromOrignalPoint - 1)
				{
					continue;
				}
				Vector2 vector2 = Vector2.UnitX * 16f;
				if (Collision.TileCollision(vector - vector2, vector2, num, num2, fallThrough: false, fall2: false, (int)teleporteeGravityDirection) != vector2)
				{
					continue;
				}
				vector2 = -Vector2.UnitX * 16f;
				if (Collision.TileCollision(vector - vector2, vector2, num, num2, fallThrough: false, fall2: false, (int)teleporteeGravityDirection) != vector2)
				{
					continue;
				}
				vector2 = Vector2.UnitY * 16f;
				if (!(Collision.TileCollision(vector - vector2, vector2, num, num2, fallThrough: false, fall2: false, (int)teleporteeGravityDirection) != vector2))
				{
					vector2 = -Vector2.UnitY * 16f;
					if (!(Collision.TileCollision(vector - vector2, vector2, num, num2, fallThrough: false, fall2: false, (int)teleporteeGravityDirection) != vector2) && (!Main.dualDungeonsSeed || !UnbreakableWallScan.InsideUnbreakableWalls(new Point(num9, num10))))
					{
						canSpawn = true;
						num5 += num8;
						break;
					}
				}
			}
			return vector;
		}

		public static ChaseResults GetChaseResults(Vector2 chaserPosition, float chaserSpeed, Vector2 runnerPosition, Vector2 runnerVelocity)
		{
			ChaseResults result = default(ChaseResults);
			if (chaserPosition == runnerPosition)
			{
				return new ChaseResults
				{
					InterceptionHappens = true,
					InterceptionPosition = chaserPosition,
					InterceptionTime = 0f,
					ChaserVelocity = Vector2.Zero
				};
			}
			if (chaserSpeed <= 0f)
			{
				return default(ChaseResults);
			}
			Vector2 value = chaserPosition - runnerPosition;
			float num = value.Length();
			float num2 = runnerVelocity.Length();
			if (num2 == 0f)
			{
				result.InterceptionTime = num / chaserSpeed;
				result.InterceptionPosition = runnerPosition;
			}
			else
			{
				float a = chaserSpeed * chaserSpeed - num2 * num2;
				float b = 2f * Vector2.Dot(value, runnerVelocity);
				float c = (0f - num) * num;
				if (!SolveQuadratic(a, b, c, out var result2, out var result3))
				{
					return default(ChaseResults);
				}
				if (result2 < 0f && result3 < 0f)
				{
					return default(ChaseResults);
				}
				if (result2 > 0f && result3 > 0f)
				{
					result.InterceptionTime = Math.Min(result2, result3);
				}
				else
				{
					result.InterceptionTime = Math.Max(result2, result3);
				}
				result.InterceptionPosition = runnerPosition + runnerVelocity * result.InterceptionTime;
			}
			result.ChaserVelocity = (result.InterceptionPosition - chaserPosition) / result.InterceptionTime;
			result.InterceptionHappens = true;
			return result;
		}

		public static float GetJumpForce(float jumpHeight, float atGravity)
		{
			return (float)Math.Sqrt(jumpHeight / atGravity * 2f) * atGravity;
		}

		public static float GetJumpTimeToApex(float jumpHeight, float atGravity)
		{
			return (float)Math.Sqrt(jumpHeight / atGravity * 2f);
		}

		public static Vector2 FactorAcceleration(Vector2 currentVelocity, float timeToInterception, Vector2 descendOfProjectile, int framesOfLenience)
		{
			float num = Math.Max(0f, timeToInterception - (float)framesOfLenience);
			Vector2 vector = descendOfProjectile * (num * num) / 2f / timeToInterception;
			return currentVelocity - vector;
		}

		public static bool SolveQuadratic(float a, float b, float c, out float result1, out float result2)
		{
			float num = b * b - 4f * a * c;
			result1 = 0f;
			result2 = 0f;
			if (num > 0f)
			{
				result1 = (0f - b + (float)Math.Sqrt(num)) / (2f * a);
				result2 = (0f - b - (float)Math.Sqrt(num)) / (2f * a);
				return true;
			}
			if (num < 0f)
			{
				return false;
			}
			result1 = (result2 = (0f - b + (float)Math.Sqrt(num)) / (2f * a));
			return true;
		}

		public static double GetLerpValue(double from, double to, double t, bool clamped = false)
		{
			if (clamped)
			{
				if (from < to)
				{
					if (t < from)
					{
						return 0.0;
					}
					if (t > to)
					{
						return 1.0;
					}
				}
				else
				{
					if (t < to)
					{
						return 1.0;
					}
					if (t > from)
					{
						return 0.0;
					}
				}
			}
			return (t - from) / (to - from);
		}

		public static float GetDayTimeAs24FloatStartingFromMidnight()
		{
			if (Main.dayTime)
			{
				return 4.5f + (float)(Main.time / 54000.0) * 15f;
			}
			return 19.5f + (float)(Main.time / 32400.0) * 9f;
		}

		public static Vector2 GetDayTimeAsDirectionIn24HClock()
		{
			return GetDayTimeAsDirectionIn24HClock(GetDayTimeAs24FloatStartingFromMidnight());
		}

		public static Vector2 GetDayTimeAsDirectionIn24HClock(float timeFrom0To24)
		{
			return new Vector2(0f, -1f).RotatedBy(timeFrom0To24 / 24f * ((float)Math.PI * 2f));
		}

		public static string[] ConvertMonoArgsToDotNet(string[] brokenArgs)
		{
			ArrayList arrayList = new ArrayList();
			string text = "";
			for (int i = 0; i < brokenArgs.Length; i++)
			{
				if (brokenArgs[i].StartsWith("-"))
				{
					if (text != "")
					{
						arrayList.Add(text);
						text = "";
					}
					else
					{
						arrayList.Add("");
					}
					arrayList.Add(brokenArgs[i]);
				}
				else
				{
					if (text != "")
					{
						text += " ";
					}
					text += brokenArgs[i];
				}
			}
			arrayList.Add(text);
			string[] array = new string[arrayList.Count];
			arrayList.CopyTo(array);
			return array;
		}

		public static T Max<T>(params T[] args) where T : IComparable
		{
			T result = args[0];
			for (int i = 1; i < args.Length; i++)
			{
				object obj = args[i];
				if (result.CompareTo(obj) < 0)
				{
					result = args[i];
				}
			}
			return result;
		}

		public static float LineRectangleDistance(Rectangle rect, Vector2 lineStart, Vector2 lineEnd)
		{
			Vector2 vector = rect.TopLeft();
			Vector2 vector2 = rect.TopRight();
			Vector2 vector3 = rect.BottomLeft();
			Vector2 vector4 = rect.BottomRight();
			if (lineStart.Between(vector, vector4) || lineEnd.Between(vector, vector4))
			{
				return 0f;
			}
			float value = vector.Distance(vector.ClosestPointOnLine(lineStart, lineEnd));
			float value2 = vector2.Distance(vector2.ClosestPointOnLine(lineStart, lineEnd));
			float value3 = vector3.Distance(vector3.ClosestPointOnLine(lineStart, lineEnd));
			float value4 = vector4.Distance(vector4.ClosestPointOnLine(lineStart, lineEnd));
			return MathHelper.Min(value, MathHelper.Min(value2, MathHelper.Min(value3, value4)));
		}

		public static List<List<TextSnippet>> WordwrapStringSmart(string text, Color c, DynamicSpriteFont font, float maxWidth = -1f, int maxLines = -1)
		{
			List<List<TextSnippet>> list = new List<List<TextSnippet>>();
			List<TextSnippet> list2 = new List<TextSnippet>();
			list.Add(list2);
			foreach (PositionedSnippet item in ChatManager.LayoutSnippets(font, ChatManager.ParseMessage(text, c), Vector2.One, maxWidth))
			{
				while (item.Line >= list.Count)
				{
					if (list.Count == maxLines)
					{
						return list;
					}
					list.Add(list2 = new List<TextSnippet>());
				}
				list2.Add(item.Snippet);
			}
			return list;
		}

		public static string[] WordwrapString(string text, DynamicSpriteFont font, int maxWidth, int maxLines, out int lineAmount)
		{
			string[] array = font.CreateWrappedText(text, maxWidth, Language.ActiveCulture.CultureInfo).Split('\n');
			lineAmount = Math.Min(array.Length, maxLines);
			string[] array2 = new string[maxLines];
			Array.Copy(array, array2, lineAmount);
			return array2;
		}

		public static string[] WordwrapStringLegacy(string text, DynamicSpriteFont font, int maxWidth, int maxLines, out int lineAmount)
		{
			string[] array = new string[maxLines];
			int num = 0;
			List<string> list = new List<string>(text.Split('\n'));
			List<string> list2 = new List<string>(list[0].Split(' '));
			for (int i = 1; i < list.Count && i < maxLines; i++)
			{
				list2.Add("\n");
				list2.AddRange(list[i].Split(' '));
			}
			bool flag = true;
			while (list2.Count > 0)
			{
				string text2 = list2[0];
				string text3 = " ";
				if (list2.Count == 1)
				{
					text3 = "";
				}
				if (text2 == "\n")
				{
					array[num++] += text2;
					flag = true;
					if (num >= maxLines)
					{
						break;
					}
					list2.RemoveAt(0);
				}
				else if (flag)
				{
					if (font.MeasureString(text2).X > (float)maxWidth)
					{
						string text4 = text2[0].ToString() ?? "";
						int num2 = 1;
						while (font.MeasureString(text4 + text2[num2] + "-").X <= (float)maxWidth)
						{
							text4 += text2[num2++];
						}
						text4 += "-";
						array[num++] = text4 + " ";
						if (num >= maxLines)
						{
							break;
						}
						list2.RemoveAt(0);
						list2.Insert(0, text2.Substring(num2));
					}
					else
					{
						ref string reference = ref array[num];
						reference = reference + text2 + text3;
						flag = false;
						list2.RemoveAt(0);
					}
				}
				else if (font.MeasureString(array[num] + text2).X > (float)maxWidth)
				{
					num++;
					if (num >= maxLines)
					{
						break;
					}
					flag = true;
				}
				else
				{
					ref string reference2 = ref array[num];
					reference2 = reference2 + text2 + text3;
					flag = false;
					list2.RemoveAt(0);
				}
			}
			lineAmount = Math.Min(num + 1, maxLines);
			return array;
		}

		public static Rectangle CenteredRectangle(Vector2 center, Vector2 size)
		{
			return new Rectangle((int)(center.X - size.X / 2f), (int)(center.Y - size.Y / 2f), (int)size.X, (int)size.Y);
		}

		public static Rectangle CenteredRectangle(Point center, Point size)
		{
			return new Rectangle(center.X - size.X / 2, center.Y - size.Y / 2, size.X, size.Y);
		}

		public static Rectangle Including(this Rectangle rect, Point point)
		{
			int num = Math.Min(rect.Left, point.X);
			int num2 = Math.Max(rect.Right, point.X);
			int num3 = Math.Min(rect.Top, point.Y);
			int num4 = Math.Max(rect.Bottom, point.Y);
			return new Rectangle(num, num3, num2 - num, num4 - num3);
		}

		public static Vector2 Vector2FromElipse(Vector2 angleVector, Vector2 elipseSizes)
		{
			if (elipseSizes == Vector2.Zero)
			{
				return Vector2.Zero;
			}
			if (angleVector == Vector2.Zero)
			{
				return Vector2.Zero;
			}
			angleVector.Normalize();
			Vector2 vector = Vector2.Normalize(elipseSizes);
			vector = Vector2.One / vector;
			angleVector *= vector;
			angleVector.Normalize();
			return angleVector * elipseSizes / 2f;
		}

		public static bool FloatIntersect(float r1StartX, float r1StartY, float r1Width, float r1Height, float r2StartX, float r2StartY, float r2Width, float r2Height)
		{
			if (r1StartX > r2StartX + r2Width || r1StartY > r2StartY + r2Height || r1StartX + r1Width < r2StartX || r1StartY + r1Height < r2StartY)
			{
				return false;
			}
			return true;
		}

		public static bool DoubleIntersect(double r1StartX, double r1StartY, double r1Width, double r1Height, double r2StartX, double r2StartY, double r2Width, double r2Height)
		{
			if (r1StartX > r2StartX + r2Width || r1StartY > r2StartY + r2Height || r1StartX + r1Width < r2StartX || r1StartY + r1Height < r2StartY)
			{
				return false;
			}
			return true;
		}

		public static bool LineSegmentsIntersect(Vector2D start1, Vector2D end1, Vector2D start2, Vector2D end2)
		{
			Vector2D vector2D = end1 - start1;
			Vector2D value = end2 - start2;
			double num = Vector2D.Cross(vector2D, value);
			if (num == 0.0)
			{
				return false;
			}
			Vector2D value2 = start2 - start1;
			_ = Vector2D.Cross(value2, vector2D) / num;
			double num2 = Vector2D.Cross(value2, vector2D) / num;
			double num3 = Vector2D.Cross(value2, vector2D) / num;
			if (0.0 <= num2 && num2 <= 1.0 && 0.0 <= num3)
			{
				return num3 <= 1.0;
			}
			return false;
		}

		public static long CoinsCount(out bool overFlowing, Item[] inv, params int[] ignoreSlots)
		{
			List<int> list = new List<int>(ignoreSlots);
			long num = 0L;
			for (int i = 0; i < inv.Length; i++)
			{
				if (!list.Contains(i))
				{
					switch (inv[i].type)
					{
					case 71:
						num += inv[i].stack;
						break;
					case 72:
						num += (long)inv[i].stack * 100L;
						break;
					case 73:
						num += (long)inv[i].stack * 10000L;
						break;
					case 74:
						num += (long)inv[i].stack * 1000000L;
						break;
					}
				}
			}
			overFlowing = false;
			return num;
		}

		public static int[] CoinsSplit(long count)
		{
			int[] array = new int[4];
			long num = 0L;
			long num2 = 1000000L;
			for (int num3 = 3; num3 >= 0; num3--)
			{
				array[num3] = (int)((count - num) / num2);
				num += array[num3] * num2;
				num2 /= 100;
			}
			return array;
		}

		public static long CoinsCombineStacks(out bool overFlowing, params long[] coinCounts)
		{
			long num = 0L;
			foreach (long num2 in coinCounts)
			{
				num += num2;
				if (num >= 9999999999L)
				{
					overFlowing = true;
					return 9999999999L;
				}
			}
			overFlowing = false;
			return num;
		}

		public static void PoofOfSmoke(Vector2 position)
		{
			int num = Main.rand.Next(3, 7);
			for (int i = 0; i < num; i++)
			{
				int num2 = Gore.NewGore(position, (Main.rand.NextFloat() * ((float)Math.PI * 2f)).ToRotationVector2() * new Vector2(2f, 0.7f) * 0.7f, Main.rand.Next(11, 14));
				Main.gore[num2].scale = 0.7f;
				Main.gore[num2].velocity *= 0.5f;
			}
			for (int j = 0; j < 10; j++)
			{
				Dust obj = Main.dust[Dust.NewDust(position, 14, 14, 16, 0f, 0f, 100, default(Color), 1.5f)];
				obj.position += new Vector2(5f);
				obj.velocity = (Main.rand.NextFloat() * ((float)Math.PI * 2f)).ToRotationVector2() * new Vector2(2f, 0.7f) * 0.7f * (0.5f + 0.5f * Main.rand.NextFloat());
			}
		}

		public static Vector2 ToScreenPosition(this Vector2 worldPosition)
		{
			return Vector2.Transform(worldPosition - Main.screenPosition, Main.GameViewMatrix.TransformationMatrix) / Main.UIScale;
		}

		public static Vector2 ScreenToWorldPosition(this Vector2 screenPosition)
		{
			return Vector2.Transform(screenPosition * Main.UIScale, Matrix.Invert(Main.GameViewMatrix.TransformationMatrix)) + Main.screenPosition;
		}

		public static string PrettifyPercentDisplay(float percent, string originalFormat)
		{
			return percent.ToString(originalFormat, CultureInfo.InvariantCulture).TrimEnd('0', '%', ' ').TrimEnd('.', ' ')
				.TrimStart('0', ' ') + "%";
		}

		public static void TrimTextIfNeeded(ref string text, DynamicSpriteFont font, float scale, float maxWidth)
		{
			bool flag = false;
			Vector2 vector = font.MeasureString(text) * scale;
			while (vector.X > maxWidth)
			{
				text = TrimLastCharacter(text);
				flag = true;
				vector = font.MeasureString(text) * scale;
			}
			if (flag)
			{
				text = TrimLastCharacter(text) + "…";
			}
		}

		public static string FormatWith(string original, object obj)
		{
			PropertyDescriptorCollection properties = TypeDescriptor.GetProperties(obj);
			return _substitutionRegex.Replace(original, delegate(Match match)
			{
				if (match.Groups[1].Length != 0)
				{
					return "";
				}
				string name = match.Groups[2].ToString();
				PropertyDescriptor propertyDescriptor = properties.Find(name, ignoreCase: false);
				return (propertyDescriptor == null) ? "" : (propertyDescriptor.GetValue(obj) ?? "").ToString();
			});
		}

		public static bool TryCreatingDirectory(string folderPath)
		{
			if (Directory.Exists(folderPath))
			{
				return true;
			}
			try
			{
				Directory.CreateDirectory(folderPath);
				return true;
			}
			catch (Exception exception)
			{
				FancyErrorPrinter.ShowDirectoryCreationFailError(exception, folderPath);
				return false;
			}
		}

		public static void OpenFolder(string folderPath)
		{
			if (TryCreatingDirectory(folderPath))
			{
				if (Platform.IsLinux)
				{
					Process.Start(new ProcessStartInfo(folderPath)
					{
						FileName = "open-folder",
						Arguments = folderPath,
						UseShellExecute = true,
						CreateNoWindow = true
					});
				}
				else
				{
					Process.Start(folderPath);
				}
			}
		}

		public static TimeSpan SWTicksToTimeSpan(long swTicks)
		{
			return new TimeSpan((long)((double)swTicks * 10000000.0 / (double)Stopwatch.Frequency));
		}

		public static long TimeSpanToSWTicks(TimeSpan timeSpan)
		{
			return timeSpan.Ticks * Stopwatch.Frequency / 10000000;
		}

		public static byte[] ToByteArray(this string str)
		{
			byte[] array = new byte[str.Length * 2];
			Buffer.BlockCopy(str.ToCharArray(), 0, array, 0, array.Length);
			return array;
		}

		public static float NextFloat(this UnifiedRandom r)
		{
			return (float)r.NextDouble();
		}

		public static float NextFloatDirection(this UnifiedRandom r)
		{
			return (float)r.NextDouble() * 2f - 1f;
		}

		public static float NextFloat(this UnifiedRandom random, FloatRange range)
		{
			return random.NextFloat() * (range.Maximum - range.Minimum) + range.Minimum;
		}

		public static T NextFromList<T>(this UnifiedRandom random, params T[] objs)
		{
			return objs[random.Next(objs.Length)];
		}

		public static bool JustBecameTrue(bool state, ref bool releasedStateHolder)
		{
			bool result = false;
			if (state)
			{
				if (releasedStateHolder)
				{
					result = true;
				}
				releasedStateHolder = false;
			}
			else
			{
				releasedStateHolder = true;
			}
			return result;
		}

		public static T NextFromCollection<T>(this UnifiedRandom random, List<T> objs)
		{
			return objs[random.Next(objs.Count)];
		}

		public static int Next(this UnifiedRandom random, IntRange range)
		{
			return random.Next(range.Minimum, range.Maximum + 1);
		}

		public static Point NextFromRectangle(this UnifiedRandom r, Rectangle rect)
		{
			return new Point(r.Next(rect.Left, rect.Right), r.Next(rect.Top, rect.Bottom));
		}

		public static Vector2 NextVector2Square(this UnifiedRandom r, float min, float max)
		{
			return new Vector2((max - min) * (float)r.NextDouble() + min, (max - min) * (float)r.NextDouble() + min);
		}

		public static Vector2 NextVector2FromRectangle(this UnifiedRandom r, Rectangle rect)
		{
			return new Vector2((float)rect.X + r.NextFloat() * (float)rect.Width, (float)rect.Y + r.NextFloat() * (float)rect.Height);
		}

		public static Vector2 NextVector2Unit(this UnifiedRandom r, float startRotation = 0f, float rotationRange = (float)Math.PI * 2f)
		{
			return (startRotation + rotationRange * r.NextFloat()).ToRotationVector2();
		}

		public static Vector2 NextVector2Circular(this UnifiedRandom r, float circleHalfWidth, float circleHalfHeight)
		{
			return r.NextVector2Unit() * new Vector2(circleHalfWidth, circleHalfHeight) * r.NextFloat();
		}

		public static Vector2 NextVector2CircularEdge(this UnifiedRandom r, float circleHalfWidth, float circleHalfHeight)
		{
			return r.NextVector2Unit() * new Vector2(circleHalfWidth, circleHalfHeight);
		}

		public static Vector2D NextVector2DSquare(this UnifiedRandom r, double min, double max)
		{
			return new Vector2D((max - min) * r.NextDouble() + min, (max - min) * r.NextDouble() + min);
		}

		public static Vector2D NextVector2DFromRectangle(this UnifiedRandom r, Rectangle rect)
		{
			return new Vector2D((double)rect.X + r.NextDouble() * (double)rect.Width, (double)rect.Y + r.NextDouble() * (double)rect.Height);
		}

		public static Vector2D NextVector2DUnit(this UnifiedRandom r, double startRotation = 0.0, double rotationRange = 6.2831854820251465)
		{
			return (startRotation + rotationRange * r.NextDouble()).ToRotationVector2D();
		}

		public static Vector2D NextVector2DCircular(this UnifiedRandom r, double circleHalfWidth, double circleHalfHeight)
		{
			return r.NextVector2DUnit() * new Vector2D(circleHalfWidth, circleHalfHeight) * r.NextDouble();
		}

		public static Vector2D NextVector2DCircularEdge(this UnifiedRandom r, double circleHalfWidth, double circleHalfHeight)
		{
			return r.NextVector2DUnit() * new Vector2D(circleHalfWidth, circleHalfHeight);
		}

		public static int Width(this Asset<Texture2D> asset)
		{
			if (!asset.IsLoaded)
			{
				return 0;
			}
			return asset.Value.Width;
		}

		public static int Height(this Asset<Texture2D> asset)
		{
			if (!asset.IsLoaded)
			{
				return 0;
			}
			return asset.Value.Height;
		}

		public static Rectangle Frame(this Asset<Texture2D> tex, int horizontalFrames = 1, int verticalFrames = 1, int frameX = 0, int frameY = 0, int sizeOffsetX = 0, int sizeOffsetY = 0)
		{
			if (!tex.IsLoaded)
			{
				return Rectangle.Empty;
			}
			return tex.Value.Frame(horizontalFrames, verticalFrames, frameX, frameY, sizeOffsetX, sizeOffsetY);
		}

		public static Rectangle OffsetSize(this Rectangle rect, int xSize, int ySize)
		{
			rect.Width += xSize;
			rect.Height += ySize;
			return rect;
		}

		public static Vector2 Size(this Asset<Texture2D> tex)
		{
			if (!tex.IsLoaded)
			{
				return Vector2.Zero;
			}
			return tex.Value.Size();
		}

		public static Rectangle Frame(this Texture2D tex, int horizontalFrames = 1, int verticalFrames = 1, int frameX = 0, int frameY = 0, int sizeOffsetX = 0, int sizeOffsetY = 0)
		{
			int num = tex.Width / horizontalFrames;
			int num2 = tex.Height / verticalFrames;
			return new Rectangle(num * frameX, num2 * frameY, num + sizeOffsetX, num2 + sizeOffsetY);
		}

		public static Vector2 OriginFlip(this Rectangle rect, Vector2 origin, SpriteEffects effects)
		{
			if ((effects & SpriteEffects.FlipHorizontally) != SpriteEffects.None)
			{
				origin.X = (float)rect.Width - origin.X;
			}
			if ((effects & SpriteEffects.FlipVertically) != SpriteEffects.None)
			{
				origin.Y = (float)rect.Height - origin.Y;
			}
			return origin;
		}

		public static Vector2 Size(this Texture2D tex)
		{
			return new Vector2(tex.Width, tex.Height);
		}

		public static void WriteRGB(this BinaryWriter bb, Color c)
		{
			bb.Write(c.R);
			bb.Write(c.G);
			bb.Write(c.B);
		}

		public static void WriteVector2(this BinaryWriter bb, Vector2 v)
		{
			bb.Write(v.X);
			bb.Write(v.Y);
		}

		public static void WritePackedVector2(this BinaryWriter bb, Vector2 v)
		{
			bb.Write(new HalfVector2(v.X, v.Y).PackedValue);
		}

		public static Color ReadRGB(this BinaryReader bb)
		{
			return new Color(bb.ReadByte(), bb.ReadByte(), bb.ReadByte());
		}

		public static Vector2 ReadVector2(this BinaryReader bb)
		{
			return new Vector2(bb.ReadSingle(), bb.ReadSingle());
		}

		public static Vector2 ReadPackedVector2(this BinaryReader bb)
		{
			HalfVector2 halfVector = new HalfVector2
			{
				PackedValue = bb.ReadUInt32()
			};
			return halfVector.ToVector2();
		}

		public static void Write7BitEncodedInt(this BinaryWriter writer, int value)
		{
			uint num;
			for (num = (uint)value; num > 127; num >>= 7)
			{
				writer.Write((byte)(num | 0xFFFFFF80u));
			}
			writer.Write((byte)num);
		}

		public static int Read7BitEncodedInt(this BinaryReader reader)
		{
			uint num = 0u;
			byte b;
			for (int i = 0; i < 28; i += 7)
			{
				b = reader.ReadByte();
				num |= (uint)((b & 0x7F) << i);
				if ((uint)b <= 127u)
				{
					return (int)num;
				}
			}
			b = reader.ReadByte();
			if (b > 15)
			{
				throw new FormatException("Bad 7bit encoded int");
			}
			return (int)num | (b << 28);
		}

		public static Vector2 Left(this Rectangle r)
		{
			return new Vector2(r.X, r.Y + r.Height / 2);
		}

		public static Vector2 Right(this Rectangle r)
		{
			return new Vector2(r.X + r.Width, r.Y + r.Height / 2);
		}

		public static Vector2 Top(this Rectangle r)
		{
			return new Vector2(r.X + r.Width / 2, r.Y);
		}

		public static Vector2 Bottom(this Rectangle r)
		{
			return new Vector2(r.X + r.Width / 2, r.Y + r.Height);
		}

		public static Vector2 TopLeft(this Rectangle r)
		{
			return new Vector2(r.X, r.Y);
		}

		public static Vector2 TopRight(this Rectangle r)
		{
			return new Vector2(r.X + r.Width, r.Y);
		}

		public static Vector2 BottomLeft(this Rectangle r)
		{
			return new Vector2(r.X, r.Y + r.Height);
		}

		public static Vector2 BottomRight(this Rectangle r)
		{
			return new Vector2(r.X + r.Width, r.Y + r.Height);
		}

		public static Vector2D TopLeftDouble(this Rectangle r)
		{
			return new Vector2D(r.X, r.Y);
		}

		public static Vector2D TopRightDouble(this Rectangle r)
		{
			return new Vector2D(r.X + r.Width, r.Y);
		}

		public static Vector2D BottomLeftDouble(this Rectangle r)
		{
			return new Vector2D(r.X, r.Y + r.Height);
		}

		public static Vector2D BottomRightDouble(this Rectangle r)
		{
			return new Vector2D(r.X + r.Width, r.Y + r.Height);
		}

		public static Vector2 Center(this Rectangle r)
		{
			return new Vector2(r.X + r.Width / 2, r.Y + r.Height / 2);
		}

		public static Vector2 Size(this Rectangle r)
		{
			return new Vector2(r.Width, r.Height);
		}

		public static float Distance(this Rectangle r, Vector2 point)
		{
			if (FloatIntersect(r.Left, r.Top, r.Width, r.Height, point.X, point.Y, 0f, 0f))
			{
				return 0f;
			}
			if (point.X >= (float)r.Left && point.X <= (float)r.Right)
			{
				if (point.Y < (float)r.Top)
				{
					return (float)r.Top - point.Y;
				}
				return point.Y - (float)r.Bottom;
			}
			if (point.Y >= (float)r.Top && point.Y <= (float)r.Bottom)
			{
				if (point.X < (float)r.Left)
				{
					return (float)r.Left - point.X;
				}
				return point.X - (float)r.Right;
			}
			if (point.X < (float)r.Left)
			{
				if (point.Y < (float)r.Top)
				{
					return Vector2.Distance(point, r.TopLeft());
				}
				return Vector2.Distance(point, r.BottomLeft());
			}
			if (point.Y < (float)r.Top)
			{
				return Vector2.Distance(point, r.TopRight());
			}
			return Vector2.Distance(point, r.BottomRight());
		}

		public static double Distance(this Rectangle r, Vector2D point)
		{
			if (DoubleIntersect(r.Left, r.Top, r.Width, r.Height, point.X, point.Y, 0.0, 0.0))
			{
				return 0.0;
			}
			if (point.X >= (double)r.Left && point.X <= (double)r.Right)
			{
				if (point.Y < (double)r.Top)
				{
					return (double)r.Top - point.Y;
				}
				return point.Y - (double)r.Bottom;
			}
			if (point.Y >= (double)r.Top && point.Y <= (double)r.Bottom)
			{
				if (point.X < (double)r.Left)
				{
					return (double)r.Left - point.X;
				}
				return point.X - (double)r.Right;
			}
			if (point.X < (double)r.Left)
			{
				if (point.Y < (double)r.Top)
				{
					return Vector2D.Distance(point, r.TopLeftDouble());
				}
				return Vector2D.Distance(point, r.BottomLeftDouble());
			}
			if (point.Y < (double)r.Top)
			{
				return Vector2D.Distance(point, r.TopRightDouble());
			}
			return Vector2D.Distance(point, r.BottomRightDouble());
		}

		public static Vector2 ClosestPointInRect(this Rectangle r, Vector2 point)
		{
			Vector2 result = point;
			if (result.X < (float)r.Left)
			{
				result.X = r.Left;
			}
			if (result.X > (float)r.Right)
			{
				result.X = r.Right;
			}
			if (result.Y < (float)r.Top)
			{
				result.Y = r.Top;
			}
			if (result.Y > (float)r.Bottom)
			{
				result.Y = r.Bottom;
			}
			return result;
		}

		public static Rectangle Modified(this Rectangle r, int x, int y, int w, int h)
		{
			return new Rectangle(r.X + x, r.Y + y, r.Width + w, r.Height + h);
		}

		public static bool IntersectsConeFastInaccurate(this Rectangle targetRect, Vector2 coneCenter, float coneLength, float coneRotation, float maximumAngle)
		{
			Vector2 point = coneCenter + coneRotation.ToRotationVector2() * coneLength;
			Vector2 spinningpoint = targetRect.ClosestPointInRect(point) - coneCenter;
			float num = spinningpoint.RotatedBy(0f - coneRotation).ToRotation();
			if (num < 0f - maximumAngle || num > maximumAngle)
			{
				return false;
			}
			return spinningpoint.Length() < coneLength;
		}

		public static bool IntersectsConeSlowMoreAccurate(this Rectangle targetRect, Vector2 coneCenter, float coneLength, float coneRotation, float maximumAngle)
		{
			Vector2 point = coneCenter + coneRotation.ToRotationVector2() * coneLength;
			if (DoesFitInCone(targetRect.ClosestPointInRect(point), coneCenter, coneLength, coneRotation, maximumAngle))
			{
				return true;
			}
			if (DoesFitInCone(targetRect.TopLeft(), coneCenter, coneLength, coneRotation, maximumAngle))
			{
				return true;
			}
			if (DoesFitInCone(targetRect.TopRight(), coneCenter, coneLength, coneRotation, maximumAngle))
			{
				return true;
			}
			if (DoesFitInCone(targetRect.BottomLeft(), coneCenter, coneLength, coneRotation, maximumAngle))
			{
				return true;
			}
			if (DoesFitInCone(targetRect.BottomRight(), coneCenter, coneLength, coneRotation, maximumAngle))
			{
				return true;
			}
			return false;
		}

		public static bool DoesFitInCone(Vector2 point, Vector2 coneCenter, float coneLength, float coneRotation, float maximumAngle)
		{
			Vector2 spinningpoint = point - coneCenter;
			float num = spinningpoint.RotatedBy(0f - coneRotation).ToRotation();
			if (num < 0f - maximumAngle || num > maximumAngle)
			{
				return false;
			}
			return spinningpoint.Length() < coneLength;
		}

		public static float ToRotation(this Vector2 v)
		{
			return (float)Math.Atan2(v.Y, v.X);
		}

		public static double ToRotation(this Vector2D v)
		{
			return Math.Atan2(v.Y, v.X);
		}

		public static Vector2 ToRotationVector2(this float f)
		{
			return new Vector2((float)Math.Cos(f), (float)Math.Sin(f));
		}

		public static Vector2D ToRotationVector2D(this double f)
		{
			return new Vector2D(Math.Cos(f), Math.Sin(f));
		}

		public static Vector2 RotatedBy(this Vector2 spinningpoint, double radians, Vector2 center = default(Vector2))
		{
			float num = (float)Math.Cos(radians);
			float num2 = (float)Math.Sin(radians);
			Vector2 vector = spinningpoint - center;
			Vector2 result = center;
			result.X += vector.X * num - vector.Y * num2;
			result.Y += vector.X * num2 + vector.Y * num;
			return result;
		}

		public static Vector2D RotatedBy(this Vector2D spinningpoint, double radians, Vector2D center = default(Vector2D))
		{
			double num = Math.Cos(radians);
			double num2 = Math.Sin(radians);
			Vector2D vector2D = spinningpoint - center;
			Vector2D result = center;
			result.X += vector2D.X * num - vector2D.Y * num2;
			result.Y += vector2D.X * num2 + vector2D.Y * num;
			return result;
		}

		public static Vector2 RotatedByRandom(this Vector2 spinninpoint, double maxRadians)
		{
			return spinninpoint.RotatedBy(Main.rand.NextDouble() * maxRadians - Main.rand.NextDouble() * maxRadians);
		}

		public static Vector2 Floor(this Vector2 vec)
		{
			vec.X = (int)vec.X;
			vec.Y = (int)vec.Y;
			return vec;
		}

		public static bool HasNaNs(this Vector2 vec)
		{
			if (!float.IsNaN(vec.X))
			{
				return float.IsNaN(vec.Y);
			}
			return true;
		}

		public static bool Between(this Vector2 vec, Vector2 minimum, Vector2 maximum)
		{
			if (vec.X >= minimum.X && vec.X <= maximum.X && vec.Y >= minimum.Y)
			{
				return vec.Y <= maximum.Y;
			}
			return false;
		}

		public static Vector2 ScaledBy(this Vector2 vec, Vector2 other)
		{
			return Vector2.Multiply(vec, other);
		}

		public static Vector2 ScaledBy(this Vector2 vec, float scaleX, float scaleY)
		{
			return Vector2.Multiply(vec, new Vector2(scaleX, scaleY));
		}

		public static Vector2 ToVector2(this Point p)
		{
			return new Vector2(p.X, p.Y);
		}

		public static Vector2 ToVector2(this Point16 p)
		{
			return new Vector2(p.X, p.Y);
		}

		public static Vector3 ToVector3(this Vector2 v)
		{
			return new Vector3(v.X, v.Y, 0f);
		}

		public static Vector2D ToVector2D(this Point p)
		{
			return new Vector2D(p.X, p.Y);
		}

		public static Vector2D ToVector2D(this Point16 p)
		{
			return new Vector2D(p.X, p.Y);
		}

		public static Vector2 ToWorldCoordinates(this Point p, float autoAddX = 8f, float autoAddY = 8f)
		{
			return p.ToVector2() * 16f + new Vector2(autoAddX, autoAddY);
		}

		public static Vector2 ToWorldCoordinates(this Point16 p, float autoAddX = 8f, float autoAddY = 8f)
		{
			return p.ToVector2() * 16f + new Vector2(autoAddX, autoAddY);
		}

		public static Vector2 MoveTowards(this Vector2 currentPosition, Vector2 targetPosition, float maxAmountAllowedToMove)
		{
			Vector2 v = targetPosition - currentPosition;
			if (v.Length() < maxAmountAllowedToMove)
			{
				return targetPosition;
			}
			return currentPosition + v.SafeNormalize(Vector2.Zero) * maxAmountAllowedToMove;
		}

		public static float MoveTowards(float original, float target, float amount)
		{
			if (original == target)
			{
				return target;
			}
			int num = Math.Sign(target - original);
			float num2 = original + amount * (float)num;
			if (Math.Sign(target - num2) != num)
			{
				return target;
			}
			return num2;
		}

		public static Point16 ToTileCoordinates16(this Vector2 vec)
		{
			return new Point16((int)vec.X >> 4, (int)vec.Y >> 4);
		}

		public static Point16 ToTileCoordinates16(this Vector2D vec)
		{
			return new Point16((int)vec.X >> 4, (int)vec.Y >> 4);
		}

		public static Point ToTileCoordinates(this Vector2 vec)
		{
			return new Point((int)vec.X >> 4, (int)vec.Y >> 4);
		}

		public static Point ToTileCoordinates(this Vector2D vec)
		{
			return new Point((int)vec.X >> 4, (int)vec.Y >> 4);
		}

		public static Point ToPoint(this Vector2 v)
		{
			return new Point((int)v.X, (int)v.Y);
		}

		public static Point ToPoint(this Vector2D v)
		{
			return new Point((int)v.X, (int)v.Y);
		}

		public static Vector2 ToVector2(this Vector2D v)
		{
			return new Vector2((float)v.X, (float)v.Y);
		}

		public static Vector2D ToVector2D(this Vector2 v)
		{
			return new Vector2D(v.X, v.Y);
		}

		public static Vector2 SafeNormalize(this Vector2 v, Vector2 defaultValue)
		{
			if (v == Vector2.Zero || v.HasNaNs())
			{
				return defaultValue;
			}
			return Vector2.Normalize(v);
		}

		public static Vector2D SafeNormalize(this Vector2D v, Vector2D defaultValue)
		{
			if (v == Vector2D.Zero)
			{
				return defaultValue;
			}
			return Vector2D.Normalize(v);
		}

		public static Point ClampedInWorld(this Point p, int fluff = 0)
		{
			return new Point(Clamp(p.X, fluff, Main.maxTilesX - fluff - 1), Clamp(p.Y, fluff, Main.maxTilesX - fluff - 1));
		}

		public static Vector2 ClosestPointOnLine(this Vector2 P, Vector2 A, Vector2 B)
		{
			Vector2 value = P - A;
			Vector2 vector = B - A;
			float num = vector.LengthSquared();
			float num2 = Vector2.Dot(value, vector) / num;
			if (num2 < 0f)
			{
				return A;
			}
			if (num2 > 1f)
			{
				return B;
			}
			return A + vector * num2;
		}

		public static Vector2D ClosestPointOnLine(this Vector2D P, Vector2D A, Vector2D B)
		{
			Vector2D value = P - A;
			Vector2D vector2D = B - A;
			double num = vector2D.LengthSquared();
			double num2 = Vector2D.Dot(value, vector2D) / num;
			if (num2 < 0.0)
			{
				return A;
			}
			if (num2 > 1.0)
			{
				return B;
			}
			return A + vector2D * num2;
		}

		public static bool RectangleLineCollision(Vector2 rectTopLeft, Vector2 rectBottomRight, Vector2 lineStart, Vector2 lineEnd)
		{
			if (lineStart.Between(rectTopLeft, rectBottomRight) || lineEnd.Between(rectTopLeft, rectBottomRight))
			{
				return true;
			}
			Vector2 p = new Vector2(rectBottomRight.X, rectTopLeft.Y);
			Vector2 vector = new Vector2(rectTopLeft.X, rectBottomRight.Y);
			Vector2[] array = new Vector2[4]
			{
				rectTopLeft.ClosestPointOnLine(lineStart, lineEnd),
				p.ClosestPointOnLine(lineStart, lineEnd),
				vector.ClosestPointOnLine(lineStart, lineEnd),
				rectBottomRight.ClosestPointOnLine(lineStart, lineEnd)
			};
			for (int i = 0; i < array.Length; i++)
			{
				if (array[0].Between(rectTopLeft, vector))
				{
					return true;
				}
			}
			return false;
		}

		public static Vector2 RotateRandom(this Vector2 spinninpoint, double maxRadians)
		{
			return spinninpoint.RotatedBy(Main.rand.NextDouble() * maxRadians - Main.rand.NextDouble() * maxRadians);
		}

		public static float AngleTo(this Vector2 Origin, Vector2 Target)
		{
			return (float)Math.Atan2(Target.Y - Origin.Y, Target.X - Origin.X);
		}

		public static float AngleFrom(this Vector2 Origin, Vector2 Target)
		{
			return (float)Math.Atan2(Origin.Y - Target.Y, Origin.X - Target.X);
		}

		public static Vector2 rotateTowards(Vector2 currentPosition, Vector2 currentVelocity, Vector2 targetPosition, float maxChange)
		{
			float num = currentVelocity.Length();
			float targetAngle = currentPosition.AngleTo(targetPosition);
			return currentVelocity.ToRotation().AngleTowards(targetAngle, maxChange).ToRotationVector2() * num;
		}

		public static float Distance(this Vector2 Origin, Vector2 Target)
		{
			return Vector2.Distance(Origin, Target);
		}

		public static double Distance(this Vector2D Origin, Vector2D Target)
		{
			return Vector2D.Distance(Origin, Target);
		}

		public static float DistanceSQ(this Vector2 Origin, Vector2 Target)
		{
			return Vector2.DistanceSquared(Origin, Target);
		}

		public static Vector2 DirectionTo(this Vector2 Origin, Vector2 Target)
		{
			return Vector2.Normalize(Target - Origin);
		}

		public static Vector2 DirectionFrom(this Vector2 Origin, Vector2 Target)
		{
			return Vector2.Normalize(Origin - Target);
		}

		public static bool WithinRange(this Vector2 Origin, Vector2 Target, float MaxRange)
		{
			return Vector2.DistanceSquared(Origin, Target) <= MaxRange * MaxRange;
		}

		public static Vector2 XY(this Vector4 vec)
		{
			return new Vector2(vec.X, vec.Y);
		}

		public static Vector2 ZW(this Vector4 vec)
		{
			return new Vector2(vec.Z, vec.W);
		}

		public static Vector3 XZW(this Vector4 vec)
		{
			return new Vector3(vec.X, vec.Z, vec.W);
		}

		public static Vector3 YZW(this Vector4 vec)
		{
			return new Vector3(vec.Y, vec.Z, vec.W);
		}

		public static Color MultiplyRGB(this Color firstColor, Color secondColor)
		{
			return new Color((byte)((float)(firstColor.R * secondColor.R) / 255f), (byte)((float)(firstColor.G * secondColor.G) / 255f), (byte)((float)(firstColor.B * secondColor.B) / 255f));
		}

		public static Color MultiplyRGBA(this Color firstColor, Color secondColor)
		{
			return new Color((byte)((float)(firstColor.R * secondColor.R) / 255f), (byte)((float)(firstColor.G * secondColor.G) / 255f), (byte)((float)(firstColor.B * secondColor.B) / 255f), (byte)((float)(firstColor.A * secondColor.A) / 255f));
		}

		public static string Hex3(this Color color)
		{
			return (color.R.ToString("X2") + color.G.ToString("X2") + color.B.ToString("X2")).ToLower();
		}

		public static string Hex4(this Color color)
		{
			return (color.R.ToString("X2") + color.G.ToString("X2") + color.B.ToString("X2") + color.A.ToString("X2")).ToLower();
		}

		public static int ToDirectionInt(this bool value)
		{
			if (!value)
			{
				return -1;
			}
			return 1;
		}

		public static int ToInt(this bool value)
		{
			if (!value)
			{
				return 0;
			}
			return 1;
		}

		public static int ModulusPositive(this int myInteger, int modulusNumber)
		{
			return (myInteger % modulusNumber + modulusNumber) % modulusNumber;
		}

		public static float AngleLerp(this float curAngle, float targetAngle, float amount)
		{
			float angle;
			if (targetAngle < curAngle)
			{
				float num = targetAngle + (float)Math.PI * 2f;
				angle = ((num - curAngle > curAngle - targetAngle) ? MathHelper.Lerp(curAngle, targetAngle, amount) : MathHelper.Lerp(curAngle, num, amount));
			}
			else
			{
				if (!(targetAngle > curAngle))
				{
					return curAngle;
				}
				float num = targetAngle - (float)Math.PI * 2f;
				angle = ((targetAngle - curAngle > curAngle - num) ? MathHelper.Lerp(curAngle, num, amount) : MathHelper.Lerp(curAngle, targetAngle, amount));
			}
			return MathHelper.WrapAngle(angle);
		}

		public static float AngleTowards(this float curAngle, float targetAngle, float maxChange)
		{
			curAngle = MathHelper.WrapAngle(curAngle);
			targetAngle = MathHelper.WrapAngle(targetAngle);
			if (curAngle < targetAngle)
			{
				if (targetAngle - curAngle > (float)Math.PI)
				{
					curAngle += (float)Math.PI * 2f;
				}
			}
			else if (curAngle - targetAngle > (float)Math.PI)
			{
				curAngle -= (float)Math.PI * 2f;
			}
			curAngle += MathHelper.Clamp(targetAngle - curAngle, 0f - maxChange, maxChange);
			return MathHelper.WrapAngle(curAngle);
		}

		public static float RotateUntil(this float curAngle, float targetAngle, float changePerTick)
		{
			curAngle = MathHelper.WrapAngle(curAngle);
			targetAngle = MathHelper.WrapAngle(targetAngle);
			if (curAngle < targetAngle)
			{
				if (targetAngle - curAngle > (float)Math.PI)
				{
					curAngle += (float)Math.PI * 2f;
				}
			}
			else if (curAngle - targetAngle > (float)Math.PI)
			{
				curAngle -= (float)Math.PI * 2f;
			}
			curAngle += changePerTick;
			curAngle = MathHelper.WrapAngle(curAngle);
			if (curAngle > targetAngle)
			{
				curAngle = targetAngle;
			}
			return curAngle;
		}

		public static bool deepCompare(this int[] firstArray, int[] secondArray)
		{
			if (firstArray == null && secondArray == null)
			{
				return true;
			}
			if (firstArray != null && secondArray != null)
			{
				if (firstArray.Length != secondArray.Length)
				{
					return false;
				}
				for (int i = 0; i < firstArray.Length; i++)
				{
					if (firstArray[i] != secondArray[i])
					{
						return false;
					}
				}
				return true;
			}
			return false;
		}

		public static bool deepCompare(this Rectangle[,] firstArray, Rectangle[,] secondArray)
		{
			if (firstArray == null && secondArray == null)
			{
				return true;
			}
			if (firstArray != null && secondArray != null)
			{
				if (firstArray.Length != secondArray.Length)
				{
					return false;
				}
				if (firstArray.GetLength(0) != secondArray.GetLength(0))
				{
					return false;
				}
				if (firstArray.GetLength(1) != secondArray.GetLength(1))
				{
					return false;
				}
				for (int i = 0; i < firstArray.GetLength(0); i++)
				{
					for (int j = 0; j < firstArray.GetLength(1); j++)
					{
						if (firstArray[i, j] != secondArray[i, j])
						{
							return false;
						}
					}
				}
				return true;
			}
			return false;
		}

		public static List<int> GetTrueIndexes(this bool[] array)
		{
			List<int> list = new List<int>();
			for (int i = 0; i < array.Length; i++)
			{
				if (array[i])
				{
					list.Add(i);
				}
			}
			return list;
		}

		public static List<int> GetTrueIndexes(params bool[][] arrays)
		{
			List<int> list = new List<int>();
			foreach (bool[] array in arrays)
			{
				list.AddRange(array.GetTrueIndexes());
			}
			return list.Distinct().ToList();
		}

		public static int Count<T>(this T[] arr, T value)
		{
			int num = 0;
			foreach (T x in arr)
			{
				if (EqualityComparer<T>.Default.Equals(x, value))
				{
					num++;
				}
			}
			return num;
		}

		public static bool PressingShift(this KeyboardState kb)
		{
			if (!kb.IsKeyDown(Keys.LeftShift))
			{
				return kb.IsKeyDown(Keys.RightShift);
			}
			return true;
		}

		public static bool PressingControl(this KeyboardState kb)
		{
			if (!kb.IsKeyDown(Keys.LeftControl))
			{
				return kb.IsKeyDown(Keys.RightControl);
			}
			return true;
		}

		public static bool PressingAlt(this KeyboardState kb)
		{
			if (!kb.IsKeyDown(Keys.LeftAlt))
			{
				return kb.IsKeyDown(Keys.RightAlt);
			}
			return true;
		}

		public static R[] MapArray<T, R>(T[] array, Func<T, R> mapper)
		{
			R[] array2 = new R[array.Length];
			for (int i = 0; i < array.Length; i++)
			{
				array2[i] = mapper(array[i]);
			}
			return array2;
		}

		public static bool PlotLine(Point16 p0, Point16 p1, TileActionAttempt plot, bool jump = true)
		{
			return PlotLine(p0.X, p0.Y, p1.X, p1.Y, plot, jump);
		}

		public static bool PlotLine(Point p0, Point p1, TileActionAttempt plot, bool jump = true)
		{
			return PlotLine(p0.X, p0.Y, p1.X, p1.Y, plot, jump);
		}

		private static bool PlotLine(int x0, int y0, int x1, int y1, TileActionAttempt plot, bool jump = true)
		{
			if (x0 == x1 && y0 == y1)
			{
				return plot(x0, y0);
			}
			bool flag = Math.Abs(y1 - y0) > Math.Abs(x1 - x0);
			if (flag)
			{
				Swap(ref x0, ref y0);
				Swap(ref x1, ref y1);
			}
			int num = Math.Abs(x1 - x0);
			int num2 = Math.Abs(y1 - y0);
			int num3 = num / 2;
			int num4 = y0;
			int num5 = ((x0 < x1) ? 1 : (-1));
			int num6 = ((y0 < y1) ? 1 : (-1));
			for (int i = x0; i != x1; i += num5)
			{
				if (flag)
				{
					if (!plot(num4, i))
					{
						return false;
					}
				}
				else if (!plot(i, num4))
				{
					return false;
				}
				num3 -= num2;
				if (num3 >= 0)
				{
					continue;
				}
				num4 += num6;
				if (!jump)
				{
					if (flag)
					{
						if (!plot(num4, i))
						{
							return false;
						}
					}
					else if (!plot(i, num4))
					{
						return false;
					}
				}
				num3 += num;
			}
			return true;
		}

		public static int RandomNext(ref ulong seed, int bits)
		{
			seed = RandomNextSeed(seed);
			return (int)(seed >> 48 - bits);
		}

		public static ulong RandomNextSeed(ulong seed)
		{
			return (seed * 25214903917L + 11) & 0xFFFFFFFFFFFFL;
		}

		public static float RandomFloat(ref ulong seed)
		{
			return (float)RandomNext(ref seed, 24) / 16777216f;
		}

		public static int RandomInt(ref ulong seed, int max)
		{
			if ((max & -max) == max)
			{
				return (int)((long)max * (long)RandomNext(ref seed, 31) >> 31);
			}
			int num;
			int num2;
			do
			{
				num = RandomNext(ref seed, 31);
				num2 = num % max;
			}
			while (num - num2 + (max - 1) < 0);
			return num2;
		}

		public static int RandomInt(ref ulong seed, int min, int max)
		{
			return RandomInt(ref seed, max - min) + min;
		}

		public static bool PlotTileLine(Vector2 start, Vector2 end, float width, TileActionAttempt plot)
		{
			return PlotTileLine(start.ToVector2D(), end.ToVector2D(), width, plot);
		}

		public static bool PlotTileLine(Vector2D start, Vector2D end, double width, TileActionAttempt plot)
		{
			double num = width / 2.0;
			Vector2D vector2D = end - start;
			Vector2D vector2D2 = vector2D / vector2D.Length();
			Vector2D vector2D3 = new Vector2D(0.0 - vector2D2.Y, vector2D2.X) * num;
			Point point = (start - vector2D3).ToTileCoordinates();
			Point point2 = (start + vector2D3).ToTileCoordinates();
			Point point3 = start.ToTileCoordinates();
			Point point4 = end.ToTileCoordinates();
			Point lineMinOffset = new Point(point.X - point3.X, point.Y - point3.Y);
			Point lineMaxOffset = new Point(point2.X - point3.X, point2.Y - point3.Y);
			return PlotLine(point3.X, point3.Y, point4.X, point4.Y, (int x, int y) => PlotLine(x + lineMinOffset.X, y + lineMinOffset.Y, x + lineMaxOffset.X, y + lineMaxOffset.Y, plot, jump: false));
		}

		public static bool PlotTileTale(Vector2D start, Vector2D end, double width, TileActionAttempt plot)
		{
			double halfWidth = width / 2.0;
			Vector2D vector2D = end - start;
			Vector2D vector2D2 = vector2D / vector2D.Length();
			Vector2D perpOffset = new Vector2D(0.0 - vector2D2.Y, vector2D2.X);
			Point pointStart = start.ToTileCoordinates();
			Point point = end.ToTileCoordinates();
			int length = 0;
			PlotLine(pointStart.X, pointStart.Y, point.X, point.Y, delegate
			{
				length++;
				return true;
			});
			length--;
			int curLength = 0;
			return PlotLine(pointStart.X, pointStart.Y, point.X, point.Y, delegate(int x, int y)
			{
				double num = 1.0 - (double)curLength / (double)length;
				curLength++;
				Point point2 = (start - perpOffset * halfWidth * num).ToTileCoordinates();
				Point point3 = (start + perpOffset * halfWidth * num).ToTileCoordinates();
				Point point4 = new Point(point2.X - pointStart.X, point2.Y - pointStart.Y);
				Point point5 = new Point(point3.X - pointStart.X, point3.Y - pointStart.Y);
				return PlotLine(x + point4.X, y + point4.Y, x + point5.X, y + point5.Y, plot, jump: false);
			});
		}

		public static void FloodFillTile(Point point, float maxDist, TileActionAttempt plot)
		{
			if (!WorldGen.InWorld(point))
			{
				return;
			}
			List<Point> t = _floodFillQueue1;
			List<Point> t2 = _floodFillQueue2;
			BitSet2D floodFillBitset = _floodFillBitset;
			floodFillBitset.Reset(point, (int)Math.Ceiling(maxDist) + 1);
			t2.Clear();
			t2.Add(point);
			floodFillBitset.Add(point);
			while (t2.Count > 0)
			{
				Swap(ref t, ref t2);
				t2.Clear();
				foreach (Point item in t)
				{
					if (plot(item.X, item.Y))
					{
						Point point2 = new Point(item.X - 1, item.Y);
						if (WorldGen.InWorld(point2) && floodFillBitset.Add(point2))
						{
							t2.Add(point2);
						}
						point2 = new Point(item.X + 1, item.Y);
						if (WorldGen.InWorld(point2) && floodFillBitset.Add(point2))
						{
							t2.Add(point2);
						}
						point2 = new Point(item.X, item.Y - 1);
						if (WorldGen.InWorld(point2) && floodFillBitset.Add(point2))
						{
							t2.Add(point2);
						}
						point2 = new Point(item.X, item.Y + 1);
						if (WorldGen.InWorld(point2) && floodFillBitset.Add(point2))
						{
							t2.Add(point2);
						}
					}
				}
			}
		}

		public static int RandomConsecutive(double random, int odds)
		{
			return (int)Math.Log(1.0 - random, 1.0 / (double)odds);
		}

		public static Vector2 RandomVector2(UnifiedRandom random, float min, float max)
		{
			return new Vector2((max - min) * (float)random.NextDouble() + min, (max - min) * (float)random.NextDouble() + min);
		}

		public static Vector2D RandomVector2D(UnifiedRandom random, double min, double max)
		{
			return new Vector2D((max - min) * random.NextDouble() + min, (max - min) * random.NextDouble() + min);
		}

		public static bool IndexInRange<T>(this T[] t, int index)
		{
			if (index >= 0)
			{
				return index < t.Length;
			}
			return false;
		}

		public static bool IndexInRange<T>(this List<T> t, int index)
		{
			if (index >= 0)
			{
				return index < t.Count;
			}
			return false;
		}

		public static T SelectRandom<T>(UnifiedRandom random, params T[] choices)
		{
			return choices[random.Next(choices.Length)];
		}

		public static void DrawBorderStringFourWay(SpriteBatch sb, DynamicSpriteFont font, string text, float x, float y, Color textColor, Color borderColor, Vector2 origin, float scale = 1f)
		{
			Color color = borderColor;
			Vector2 zero = Vector2.Zero;
			for (int i = 0; i < 5; i++)
			{
				switch (i)
				{
				case 0:
					zero.X = x - 2f;
					zero.Y = y;
					break;
				case 1:
					zero.X = x + 2f;
					zero.Y = y;
					break;
				case 2:
					zero.X = x;
					zero.Y = y - 2f;
					break;
				case 3:
					zero.X = x;
					zero.Y = y + 2f;
					break;
				default:
					zero.X = x;
					zero.Y = y;
					color = textColor;
					break;
				}
				sb.DrawString(font, text, zero, color, 0f, origin, scale, SpriteEffects.None, 0f);
			}
		}

		public static Vector2 DrawBorderStringMeasured(SpriteBatch sb, string text, Vector2 pos, Color color, float scale = 1f, float anchorx = 0f, float anchory = 0f, int maxCharactersDisplayed = -1)
		{
			if (maxCharactersDisplayed != -1)
			{
				text = TrimUserString(text, maxCharactersDisplayed);
			}
			DynamicSpriteFont value = FontAssets.MouseText.Value;
			Vector2 vector = value.MeasureString(text);
			ChatManager.DrawColorCodedStringWithShadow(sb, value, text, pos, color, 0f, new Vector2(anchorx, anchory) * vector, new Vector2(scale), -1f, 1.5f);
			return vector * scale;
		}

		public static void DrawBorderString(SpriteBatch sb, string text, Vector2 pos, Color color, float scale = 1f, float anchorx = 0f, float anchory = 0f, int maxCharactersDisplayed = -1)
		{
			if (maxCharactersDisplayed != -1)
			{
				text = TrimUserString(text, maxCharactersDisplayed);
			}
			DynamicSpriteFont value = FontAssets.MouseText.Value;
			Vector2 origin = ((anchorx == 0f && anchory == 0f) ? Vector2.Zero : (new Vector2(anchorx, anchory) * value.MeasureString(text)));
			ChatManager.DrawColorCodedStringWithShadow(sb, value, text, pos, color, 0f, origin, new Vector2(scale), -1f, 1.5f);
		}

		public static Vector2 DrawBorderStringBig(SpriteBatch spriteBatch, string text, Vector2 pos, Color color, float scale = 1f, float anchorx = 0f, float anchory = 0f, int maxCharactersDisplayed = -1)
		{
			if (maxCharactersDisplayed != -1 && text.Length > maxCharactersDisplayed)
			{
				text.Substring(0, maxCharactersDisplayed);
			}
			DynamicSpriteFont value = FontAssets.DeathText.Value;
			for (int i = -1; i < 2; i++)
			{
				for (int j = -1; j < 2; j++)
				{
					spriteBatch.DrawString(value, text, pos + new Vector2(i, j), Color.Black, 0f, new Vector2(anchorx, anchory) * value.MeasureString(text), scale, SpriteEffects.None, 0f);
				}
			}
			spriteBatch.DrawString(value, text, pos, color, 0f, new Vector2(anchorx, anchory) * value.MeasureString(text), scale, SpriteEffects.None, 0f);
			return value.MeasureString(text) * scale;
		}

		public static void DrawInvBG(SpriteBatch sb, Rectangle R, Color c = default(Color))
		{
			DrawInvBG(sb, R.X, R.Y, R.Width, R.Height, c);
		}

		public static void DrawInvBG(SpriteBatch sb, float x, float y, float w, float h, Color c = default(Color))
		{
			DrawInvBG(sb, (int)x, (int)y, (int)w, (int)h, c);
		}

		public static void DrawInvBG(SpriteBatch sb, int x, int y, int w, int h, Color c = default(Color))
		{
			if (c == default(Color))
			{
				c = new Color(63, 65, 151, 255) * 0.785f;
			}
			Texture2D value = TextureAssets.InventoryBack13.Value;
			if (w < 20)
			{
				w = 20;
			}
			if (h < 20)
			{
				h = 20;
			}
			sb.Draw(value, new Rectangle(x, y, 10, 10), new Rectangle(0, 0, 10, 10), c);
			sb.Draw(value, new Rectangle(x + 10, y, w - 20, 10), new Rectangle(10, 0, 10, 10), c);
			sb.Draw(value, new Rectangle(x + w - 10, y, 10, 10), new Rectangle(value.Width - 10, 0, 10, 10), c);
			sb.Draw(value, new Rectangle(x, y + 10, 10, h - 20), new Rectangle(0, 10, 10, 10), c);
			sb.Draw(value, new Rectangle(x + 10, y + 10, w - 20, h - 20), new Rectangle(10, 10, 10, 10), c);
			sb.Draw(value, new Rectangle(x + w - 10, y + 10, 10, h - 20), new Rectangle(value.Width - 10, 10, 10, 10), c);
			sb.Draw(value, new Rectangle(x, y + h - 10, 10, 10), new Rectangle(0, value.Height - 10, 10, 10), c);
			sb.Draw(value, new Rectangle(x + 10, y + h - 10, w - 20, 10), new Rectangle(10, value.Height - 10, 10, 10), c);
			sb.Draw(value, new Rectangle(x + w - 10, y + h - 10, 10, 10), new Rectangle(value.Width - 10, value.Height - 10, 10, 10), c);
		}

		public static string ReadEmbeddedResource(string path)
		{
			using (Stream stream = Assembly.GetExecutingAssembly().GetManifestResourceStream(path))
			{
				using (StreamReader streamReader = new StreamReader(stream))
				{
					return streamReader.ReadToEnd();
				}
			}
		}

		public static void DrawSplicedPanel(SpriteBatch sb, Texture2D texture, int x, int y, int w, int h, int leftEnd, int rightEnd, int topEnd, int bottomEnd, Color c)
		{
			if (w < leftEnd + rightEnd)
			{
				w = leftEnd + rightEnd;
			}
			if (h < topEnd + bottomEnd)
			{
				h = topEnd + bottomEnd;
			}
			sb.Draw(texture, new Rectangle(x, y, leftEnd, topEnd), new Rectangle(0, 0, leftEnd, topEnd), c);
			sb.Draw(texture, new Rectangle(x + leftEnd, y, w - leftEnd - rightEnd, topEnd), new Rectangle(leftEnd, 0, texture.Width - leftEnd - rightEnd, topEnd), c);
			sb.Draw(texture, new Rectangle(x + w - rightEnd, y, topEnd, rightEnd), new Rectangle(texture.Width - rightEnd, 0, rightEnd, topEnd), c);
			sb.Draw(texture, new Rectangle(x, y + topEnd, leftEnd, h - topEnd - bottomEnd), new Rectangle(0, topEnd, leftEnd, texture.Height - topEnd - bottomEnd), c);
			sb.Draw(texture, new Rectangle(x + leftEnd, y + topEnd, w - leftEnd - rightEnd, h - topEnd - bottomEnd), new Rectangle(leftEnd, topEnd, texture.Width - leftEnd - rightEnd, texture.Height - topEnd - bottomEnd), c);
			sb.Draw(texture, new Rectangle(x + w - rightEnd, y + topEnd, rightEnd, h - topEnd - bottomEnd), new Rectangle(texture.Width - rightEnd, topEnd, rightEnd, texture.Height - topEnd - bottomEnd), c);
			sb.Draw(texture, new Rectangle(x, y + h - bottomEnd, leftEnd, bottomEnd), new Rectangle(0, texture.Height - bottomEnd, leftEnd, bottomEnd), c);
			sb.Draw(texture, new Rectangle(x + leftEnd, y + h - bottomEnd, w - leftEnd - rightEnd, bottomEnd), new Rectangle(leftEnd, texture.Height - bottomEnd, texture.Width - leftEnd - rightEnd, bottomEnd), c);
			sb.Draw(texture, new Rectangle(x + w - rightEnd, y + h - bottomEnd, rightEnd, bottomEnd), new Rectangle(texture.Width - rightEnd, texture.Height - bottomEnd, rightEnd, bottomEnd), c);
		}

		public static void DrawSettingsPanel(SpriteBatch spriteBatch, Vector2 position, float width, Color color)
		{
			DrawPanel(TextureAssets.SettingsPanel.Value, 2, 0, spriteBatch, position, width, color);
		}

		public static void DrawSettings2Panel(SpriteBatch spriteBatch, Vector2 position, float width, Color color)
		{
			DrawPanel(TextureAssets.SettingsPanel.Value, 2, 0, spriteBatch, position, width, color);
		}

		public static void DrawPanel(Texture2D texture, int edgeWidth, int edgeShove, SpriteBatch spriteBatch, Vector2 position, float width, Color color)
		{
			spriteBatch.Draw(texture, position, new Rectangle(0, 0, edgeWidth, texture.Height), color);
			spriteBatch.Draw(texture, new Vector2(position.X + (float)edgeWidth, position.Y), new Rectangle(edgeWidth + edgeShove, 0, texture.Width - (edgeWidth + edgeShove) * 2, texture.Height), color, 0f, Vector2.Zero, new Vector2((width - (float)(edgeWidth * 2)) / (float)(texture.Width - (edgeWidth + edgeShove) * 2), 1f), SpriteEffects.None, 0f);
			spriteBatch.Draw(texture, new Vector2(position.X + width - (float)edgeWidth, position.Y), new Rectangle(texture.Width - edgeWidth, 0, edgeWidth, texture.Height), color);
		}

		public static void DrawRectangle(SpriteBatch sb, Vector2 start, Vector2 end, Color colorStart, Color colorEnd, float width)
		{
			DrawLine(sb, start, new Vector2(start.X, end.Y), colorStart, colorEnd, width);
			DrawLine(sb, start, new Vector2(end.X, start.Y), colorStart, colorEnd, width);
			DrawLine(sb, end, new Vector2(start.X, end.Y), colorStart, colorEnd, width);
			DrawLine(sb, end, new Vector2(end.X, start.Y), colorStart, colorEnd, width);
		}

		public static void DrawLaser(SpriteBatch sb, Texture2D tex, Vector2 start, Vector2 end, Vector2 scale, LaserLineFraming framing)
		{
			Vector2 vector = start;
			Vector2 vector2 = Vector2.Normalize(end - start);
			float num = (end - start).Length();
			float rotation = vector2.ToRotation() - (float)Math.PI / 2f;
			if (vector2.HasNaNs())
			{
				return;
			}
			framing(0, vector, num, default(Rectangle), out var distanceCovered, out var frame, out var origin, out var color);
			sb.Draw(tex, vector, frame, color, rotation, frame.Size() / 2f, scale, SpriteEffects.None, 0f);
			num -= distanceCovered * scale.Y;
			vector += vector2 * ((float)frame.Height - origin.Y) * scale.Y;
			if (num > 0f)
			{
				float num2 = 0f;
				while (num2 + 1f < num)
				{
					framing(1, vector, num - num2, frame, out distanceCovered, out frame, out origin, out color);
					if (distanceCovered < 1f)
					{
						distanceCovered = 1f;
					}
					if (scale.Y < 0.05f)
					{
						scale.Y = 0.05f;
					}
					if (num - num2 < (float)frame.Height)
					{
						distanceCovered *= (num - num2) / (float)frame.Height;
						frame.Height = (int)(num - num2);
					}
					sb.Draw(tex, vector, frame, color, rotation, origin, scale, SpriteEffects.None, 0f);
					num2 += distanceCovered * scale.Y;
					vector += vector2 * distanceCovered * scale.Y;
				}
			}
			framing(2, vector, num, default(Rectangle), out distanceCovered, out frame, out origin, out color);
			sb.Draw(tex, vector, frame, color, rotation, origin, scale, SpriteEffects.None, 0f);
		}

		public static void DrawLine(SpriteBatch spriteBatch, Point start, Point end, Color color)
		{
			DrawLine(spriteBatch, new Vector2(start.X << 4, start.Y << 4), new Vector2(end.X << 4, end.Y << 4), color);
		}

		public static void DrawLine(SpriteBatch spriteBatch, Vector2 start, Vector2 end, Color color)
		{
			float num = Vector2.Distance(start, end);
			Vector2 vector = (end - start) / num;
			Vector2 vector2 = start;
			Vector2 screenPosition = Main.screenPosition;
			float rotation = vector.ToRotation();
			for (float num2 = 0f; num2 <= num; num2 += 4f)
			{
				float num3 = num2 / num;
				spriteBatch.Draw(TextureAssets.BlackTile.Value, vector2 - screenPosition, null, new Color(new Vector4(num3, num3, num3, 1f) * color.ToVector4()), rotation, Vector2.Zero, 0.25f, SpriteEffects.None, 0f);
				vector2 = start + num2 * vector;
			}
		}

		public static void DrawLine(SpriteBatch spriteBatch, Vector2 start, Vector2 end, Color colorStart, Color colorEnd, float width)
		{
			float num = Vector2.Distance(start, end);
			float rotation = (end - start).ToRotation();
			int num2 = Math.Min(5, (int)num);
			if (colorStart == colorEnd)
			{
				num2 = 1;
			}
			for (int i = 0; i < num2; i++)
			{
				spriteBatch.Draw(TextureAssets.BlackTile.Value, Vector2.Lerp(start, end, (float)i / (float)num2) - Main.screenPosition, null, Color.Lerp(colorStart, colorEnd, ((float)i + 0.5f) / (float)num2), rotation, new Vector2(0f, 8f), new Vector2(num / (float)num2 / 16f, width / 16f), SpriteEffects.None, 0f);
			}
		}

		public static void DrawRectForTilesInWorld(SpriteBatch spriteBatch, Rectangle rect, Color color)
		{
			DrawRectForTilesInWorld(spriteBatch, new Point(rect.X, rect.Y), new Point(rect.X + rect.Width, rect.Y + rect.Height), color);
		}

		public static void DrawRectForTilesInWorld(SpriteBatch spriteBatch, Point start, Point end, Color color)
		{
			DrawRect(spriteBatch, new Vector2(start.X << 4, start.Y << 4), new Vector2((end.X << 4) - 4, (end.Y << 4) - 4), color);
		}

		public static void DrawRect(SpriteBatch spriteBatch, Rectangle rect, Color color)
		{
			DrawRect(spriteBatch, new Vector2(rect.X, rect.Y), new Vector2(rect.X + rect.Width, rect.Y + rect.Height), color);
		}

		public static void DrawRect(SpriteBatch spriteBatch, Vector2 start, Vector2 end, Color color)
		{
			DrawLine(spriteBatch, start, new Vector2(start.X, end.Y), color);
			DrawLine(spriteBatch, start, new Vector2(end.X, start.Y), color);
			DrawLine(spriteBatch, end, new Vector2(start.X, end.Y), color);
			DrawLine(spriteBatch, end, new Vector2(end.X, start.Y), color);
		}

		public static void DrawRect(SpriteBatch spriteBatch, Vector2 topLeft, Vector2 topRight, Vector2 bottomRight, Vector2 bottomLeft, Color color)
		{
			DrawLine(spriteBatch, topLeft, topRight, color);
			DrawLine(spriteBatch, topRight, bottomRight, color);
			DrawLine(spriteBatch, bottomRight, bottomLeft, color);
			DrawLine(spriteBatch, bottomLeft, topLeft, color);
		}

		public static void DrawSelectedCraftingBarIndicator(SpriteBatch spriteBatch, int craftX, int craftY)
		{
			int num = 16;
			Color ourFavoriteColor = Main.OurFavoriteColor;
			float num2 = 16f;
			for (float num3 = num2; num3 > 0f; num3 -= 1f)
			{
				float num4 = 1f - num3 / num2;
				spriteBatch.Draw(TextureAssets.BlackTile.Value, new Rectangle(craftX - 16, craftY + num + (int)num3 * -1, 32, 2), ourFavoriteColor * (num4 * 0.6f));
			}
			spriteBatch.Draw(TextureAssets.BlackTile.Value, new Rectangle(craftX - 16, craftY + num, 32, 4), ourFavoriteColor);
		}

		public static void DrawCursorSingle(SpriteBatch sb, Color color, float rot = float.NaN, float scale = 1f, Vector2 manualPosition = default(Vector2), int cursorSlot = 0, int specialMode = 0)
		{
			bool flag = false;
			bool flag2 = true;
			bool flag3 = true;
			Vector2 origin = Vector2.Zero;
			Vector2 vector = new Vector2(Main.mouseX, Main.mouseY);
			if (manualPosition != Vector2.Zero)
			{
				vector = manualPosition;
			}
			if (float.IsNaN(rot))
			{
				rot = 0f;
			}
			else
			{
				flag = true;
				rot -= (float)Math.PI * 3f / 4f;
			}
			if (cursorSlot == 4 || cursorSlot == 5)
			{
				flag2 = false;
				origin = new Vector2(8f);
				if (flag && specialMode == 0)
				{
					float num = rot;
					if (num < 0f)
					{
						num += (float)Math.PI * 2f;
					}
					for (float num2 = 0f; num2 < 4f; num2 += 1f)
					{
						if (Math.Abs(num - (float)Math.PI / 2f * num2) <= (float)Math.PI / 4f)
						{
							rot = (float)Math.PI / 2f * num2;
							break;
						}
					}
				}
			}
			Vector2 vector2 = Vector2.One;
			if ((Main.ThickMouse && cursorSlot == 0) || cursorSlot == 1)
			{
				vector2 = Main.DrawThickCursor(cursorSlot == 1);
			}
			if (flag2)
			{
				sb.Draw(TextureAssets.Cursors[cursorSlot].Value, vector + vector2 + Vector2.One, null, color.MultiplyRGB(new Color(0.2f, 0.2f, 0.2f, 0.5f)), rot, origin, scale * 1.1f, SpriteEffects.None, 0f);
			}
			if (flag3)
			{
				sb.Draw(TextureAssets.Cursors[cursorSlot].Value, vector + vector2, null, color, rot, origin, scale, SpriteEffects.None, 0f);
			}
		}

		public static bool TryOperateInLock(object _lock, Action action)
		{
			if (!Monitor.TryEnter(_lock))
			{
				return false;
			}
			try
			{
				action();
				return true;
			}
			finally
			{
				Monitor.Exit(_lock);
			}
		}

		public static bool ParseCommandPrefix(string text, string prefix, out string remainder)
		{
			remainder = "";
			if (!text.StartsWith(prefix, ignoreCase: true, CultureInfo.InvariantCulture))
			{
				return false;
			}
			if (text.Length == prefix.Length)
			{
				return true;
			}
			if (text[prefix.Length] != ' ')
			{
				return false;
			}
			remainder = text.Substring(prefix.Length + 1);
			return true;
		}

		public static string TrimUserString(string s, int length)
		{
			if (s.Length <= length)
			{
				return s;
			}
			if (length > 0 && char.IsHighSurrogate(s[length - 1]))
			{
				length--;
			}
			return s.Substring(0, length);
		}

		public static string TrimLastCharacter(string s)
		{
			return TrimUserString(s, s.Length - 1);
		}
	}
}
