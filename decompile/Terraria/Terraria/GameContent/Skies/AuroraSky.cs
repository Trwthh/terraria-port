using System;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.Enums;
using Terraria.GameInput;
using Terraria.Graphics;
using Terraria.Graphics.Effects;
using Terraria.Graphics.Shaders;
using Terraria.Utilities;

namespace Terraria.GameContent.Skies
{
	public class AuroraSky : CustomSky
	{
		private delegate void ScriptMethodSignature(VertexStrip vertexStrip, float skyOpacity, ref Color lastSkyColor);

		private UnifiedRandom _random = new UnifiedRandom();

		private bool _isActive;

		private bool _isLeaving;

		private float _opacity;

		private VertexStrip vertexStrip = new VertexStrip();

		private Color _lastSkyColor;

		public override void OnLoad()
		{
		}

		public override void Update(GameTime gameTime)
		{
			if (FocusHelper.PauseSkies)
			{
				return;
			}
			if (_isLeaving)
			{
				_opacity -= (float)gameTime.ElapsedGameTime.TotalSeconds * 0.5f;
				if (_opacity < 0f)
				{
					_isActive = false;
					_opacity = 0f;
				}
			}
			else
			{
				_opacity += (float)gameTime.ElapsedGameTime.TotalSeconds * 0.3f;
				if (_opacity > 1f)
				{
					_opacity = 1f;
				}
			}
		}

		public override void Draw(SpriteBatch spriteBatch, float minDepth, float maxDepth)
		{
			if (maxDepth == float.MaxValue)
			{
				DrawAuroraSky(vertexStrip, _opacity, ref _lastSkyColor);
			}
		}

		private static void DrawAuroraSky(VertexStrip vertexStrip, float skyOpacity, ref Color lastSkyColor)
		{
			MiscShaderData miscShaderData = GameShaders.Misc["Aurora"];
			float num = (Main.dayTime ? 54000f : 32400f);
			float fromValue = (float)Main.time;
			skyOpacity *= Utils.Remap(fromValue, 0f, 180f, 0f, 1f) * Utils.Remap(fromValue, num - 180f, num, 1f, 0f);
			if (skyOpacity <= 0.01f || Main.dayTime)
			{
				return;
			}
			bool flag = false;
			bool flag2 = false;
			bool flag3 = false;
			bool flag4 = false;
			int num2 = 1;
			float num3 = 1f;
			float num4 = 1f;
			bool flag5 = false;
			float saturation = 1f;
			switch (Main.GetMoonPhase())
			{
			case MoonPhase.Full:
				flag = true;
				num2 = 3;
				break;
			case MoonPhase.ThreeQuartersAtLeft:
				num2 = 2;
				flag5 = true;
				break;
			case MoonPhase.HalfAtLeft:
				flag2 = true;
				flag3 = true;
				num2 = 3;
				flag4 = true;
				num4 *= 0.5f;
				break;
			case MoonPhase.QuarterAtLeft:
				return;
			case MoonPhase.Empty:
				flag2 = true;
				num2 = 3;
				break;
			case MoonPhase.QuarterAtRight:
				num2 = 2;
				flag5 = true;
				saturation = 0.5f;
				break;
			case MoonPhase.HalfAtRight:
				return;
			case MoonPhase.ThreeQuartersAtRight:
				flag2 = true;
				flag3 = true;
				num2 = 3;
				flag4 = true;
				num4 *= 0.5f;
				saturation = 0.5f;
				break;
			}
			PlayerInput.SetZoom_Background();
			Main.spriteBatch.End();
			Vector2 vector = new Vector2(1920f, 1080f);
			float scale = (float)Main.ScreenSize.X / vector.X;
			miscShaderData.UseSpriteTransformMatrix(Main.LatestSurfaceBackgroundBeginner.transformMatrix * Matrix.CreateScale(scale));
			Vector2 lastCelestialBodyPosition = Main.LastCelestialBodyPosition;
			lastCelestialBodyPosition.Y *= vector.X / vector.Y / ((float)Main.ScreenSize.X / (float)Main.ScreenSize.Y);
			float num5 = Main.GlobalTimeWrappedHourly / 60f;
			for (int i = 0; i < num2; i++)
			{
				vertexStrip.Reset();
				int num6 = 140;
				float num7 = 2.5f;
				float num8 = 0f;
				float luminosity = 1f;
				Vector4 specificData = new Vector4(0f, 0f, 0f, 0f);
				if (i == 0)
				{
					specificData.Y = 0f;
				}
				if (i == 1)
				{
					specificData.Y = 0.7f;
				}
				if (i == 2)
				{
					specificData.Y = 0.8f;
				}
				if (flag4)
				{
					luminosity = 1f;
					specificData.X = 0.3f;
				}
				if (flag2)
				{
					num7 = 1f;
					num8 += 0.33f;
					if (i != 0)
					{
						specificData.Y = 0.4f + (float)i * 0.2f;
					}
					if (!flag3)
					{
						specificData.Z = 0.2f;
					}
				}
				if (flag && i != 0)
				{
					specificData.Y = 0.4f;
				}
				if (flag5 && i == 0)
				{
					specificData.Y = 0.3f;
				}
				if (flag5 && i == 1)
				{
					specificData.Y = 0.5f;
				}
				if (flag && i == 0)
				{
					specificData.Y = 0.5f;
				}
				if (flag2 && i == 0)
				{
					specificData.Y = 0.7f;
				}
				miscShaderData.UseShaderSpecificData(specificData);
				for (int num9 = num6; num9 >= 0; num9--)
				{
					float num10 = (float)num9 / (float)num6;
					float num11 = num10;
					if (flag5 && i == 1)
					{
						num10 = Utils.Remap(num10, 0f, 1f, 50f / (float)num6, 90f / (float)num6);
					}
					float amount = num10;
					if (!flag)
					{
						amount = 1f - num10;
					}
					float num12 = MathHelper.Lerp(0.4f, 0.1f, num10);
					float num13 = 0.4f + num5;
					float num14 = 3f;
					float num15 = 0.5f + (float)Math.Cos((double)num10 * Math.PI * (double)num14 + (double)num13) * 0.4f * MathHelper.Lerp(1f, 0.3f, amount);
					float num16 = Utils.Remap(Math.Abs((float)Math.Sin((double)num10 * Math.PI * (double)num14 + (double)num13)), 0f, 0.98f, 0f, 1f);
					float num17 = MathHelper.Lerp(0.2f, 0.05f, amount) * num3;
					float num18 = 0.5f - 0.5f * (float)Math.Cos(num10 * ((float)Math.PI * 2f));
					float num19 = num5;
					if (flag5)
					{
						float num20 = num5 * 0.16f;
						if (i == 1)
						{
							Utils.Remap(num10, 0f, 1f, 50f / (float)num6, 90f / (float)num6);
						}
						num12 += (1f - num10) * 0.05f;
						num17 += 0.05f;
						if (i == 1)
						{
							num12 = 0.5f + (float)Math.Cos(num20 * ((float)Math.PI * 2f) * 0.15f + num10 * 60f) * 0.03f;
							float num21 = num10 + num20;
							num15 = 0.5f + (float)Math.Cos((double)num21 * Math.PI * 2.0) * 1.4f * MathHelper.Lerp(1f, 0.3f, num10);
							num15 += (float)Math.Sin(num20 * ((float)Math.PI * 2f)) * MathHelper.Lerp(0.4f, 0.13f, num10);
							num12 -= (float)Math.Cos(num20 * ((float)Math.PI * 2f) * 3f + num10 * 5f) * 0.06f;
							num17 += 0.15f;
							num15 = num11 * 1.1f;
							num16 = 1f - (float)Math.Sin(num11 * ((float)Math.PI * 2f) * 2f + (float)Math.PI / 2f) * 0.35f - 0.35f;
							num12 = (float)Math.Sin(num11 * ((float)Math.PI * 2f) * 2f + (float)Math.PI / 2f) * 0.0125f + 0.55f;
							num17 = 0.16f * num3 + 0.05f + (float)Math.Sin(num11 * ((float)Math.PI * 2f) * 2f) * 0.025f;
							num17 += 0.2f;
						}
						if (i == 0)
						{
							float num22 = Utils.Remap(num10, 0f, 0.3f, 0f, 1f);
							num18 *= num22 * num22 * num22;
							num17 -= 0.1f;
							num17 += 0.8f * num10 * num10;
						}
					}
					if (flag && i == 0)
					{
						float num23 = num5 * 0.16f;
						num12 = 0.5f + (float)Math.Cos(num23 * ((float)Math.PI * 2f) * 0.15f + num10 * 60f) * 0.03f;
						float num24 = num10 + num23;
						num15 = 0.5f + (float)Math.Cos((double)num24 * Math.PI * 2.0) * 1.4f * MathHelper.Lerp(1f, 0.3f, num10);
						num15 += (float)Math.Sin(num23 * ((float)Math.PI * 2f)) * MathHelper.Lerp(0.4f, 0.13f, num10);
						num17 += (float)(Math.Sin(num23 * ((float)Math.PI * 2f)) + 1.0) * MathHelper.Lerp(0.24f, 0.15f, num10) * num3;
						num12 -= (float)Math.Cos(num23 * ((float)Math.PI * 2f) * 3f + num10 * 5f) * 0.06f;
						num15 = num11 * 1.1f;
						num12 = (float)Math.Sin(num11 * ((float)Math.PI * 2f) * 2f + (float)Math.PI / 2f + num5 * 2f + (float)Math.PI) * 0.025f + 0.55f;
						num17 = 0.16f * num3 + 0.05f + (float)Math.Sin(num11 * ((float)Math.PI * 2f) * 2f + num5 * 2f) * 0.02f;
						num16 = 1f - (float)Math.Sin(num11 * ((float)Math.PI * 2f) * 2f + (float)Math.PI / 2f) * 0.35f - 0.35f;
					}
					if (flag2)
					{
						float num25 = num5 * 0.16f;
						if (i == 0)
						{
							num12 = 0.5f + (float)Math.Cos(num25 * ((float)Math.PI * 2f) * 0.15f + num10 * 60f) * 0.03f;
							float num26 = num10 + num25;
							num15 = 0.5f + (float)Math.Cos((double)num26 * Math.PI * 2.0) * 1.4f * MathHelper.Lerp(1f, 0.3f, num10);
							num15 += (float)Math.Sin(num25 * ((float)Math.PI * 2f)) * MathHelper.Lerp(0.4f, 0.13f, num10);
							num12 -= (float)Math.Cos(num25 * ((float)Math.PI * 2f) * 3f + num10 * 5f) * 0.06f;
							num17 += 0.15f;
							num15 = num11 * 1.1f;
							num16 = 1f - (float)Math.Sin(num11 * ((float)Math.PI * 2f) * 2f + (float)Math.PI / 2f) * 0.35f - 0.35f;
							num12 = (float)Math.Sin(num11 * ((float)Math.PI * 2f) * 2f + (float)Math.PI / 2f) * 0.025f + 0.55f;
							num17 = 0.16f * num3 + 0.05f + (float)Math.Sin(num11 * ((float)Math.PI * 2f) * 2f) * 0.05f;
						}
						else
						{
							_ = 1;
							_ = 1;
							_ = 2;
							if (i == 1 || i == 2)
							{
								num12 = MathHelper.Lerp(0.3f, 0.3f, num10);
								Math.Sin(num5 * ((float)Math.PI * 2f));
								float value = (float)Math.Cos(num5 * ((float)Math.PI * 2f));
								if (i == 1)
								{
									num17 += 0.5f * num10;
								}
								num12 -= (float)Math.Cos(num10 * ((float)Math.PI * 2f) + num5 * 2f) * 0.07f;
								num16 = Utils.Remap(Math.Abs(value), 0f, 0.98f, 0f, 1f);
								num16 = 1f;
								num15 = num10;
								num19 += 0.35f;
								if (!flag3)
								{
									num19 -= 0.35f;
								}
								num18 *= 0.55f;
								if (i == 2)
								{
									Math.Sin(num5 * ((float)Math.PI * 2f));
									Math.Cos(num5 * ((float)Math.PI * 2f));
									num12 -= (float)Math.Cos(num5 * ((float)Math.PI * 2f) * 0.35f + num10 * 13.73f) * 0.04f * (1f - num10) + 0.04f;
									num12 -= 0.03f;
								}
							}
							else
							{
								switch (i)
								{
								case 1:
								{
									num12 = MathHelper.Lerp(0.4f, 0.1f, num10);
									Math.Sin(num5 * ((float)Math.PI * 2f));
									float value3 = (float)Math.Cos(num5 * ((float)Math.PI * 2f));
									num12 -= (float)Math.Cos(num10 * ((float)Math.PI * 2f) + num5 * 2f) * 0.07f;
									num16 = Utils.Remap(Math.Abs(value3), 0f, 0.98f, 0f, 1f);
									num16 = 1f;
									num15 = num10;
									num19 += 0.35f;
									num18 *= 0.55f;
									break;
								}
								case 2:
								{
									num12 = MathHelper.Lerp(0.1f, 0.4f, num10);
									Math.Sin(num5 * ((float)Math.PI * 2f));
									float value2 = (float)Math.Cos(num5 * ((float)Math.PI * 2f));
									num12 -= (float)Math.Cos(num5 * ((float)Math.PI * 2f) * 0.35f) * 0.15f * (1f - num10);
									num19 += 0.35f;
									num16 = Utils.Remap(Math.Abs(value2), 0f, 0.98f, 0f, 1f);
									num16 = 1f;
									num15 = num10;
									break;
								}
								}
							}
						}
					}
					if (flag3)
					{
						num19 = num5 + (float)i * 0.05f;
						num7 = 0.5f;
						num8 = 0.02f;
					}
					if (flag2 && !flag3)
					{
						luminosity = 1f;
						num8 = 0.45f;
					}
					if (flag && i != 0)
					{
						num18 = Math.Max(num18 * 2f, num10);
						if (num18 > 1f)
						{
							num18 = 1f;
						}
						num15 = MathHelper.Lerp(num15, lastCelestialBodyPosition.X, num10);
						num12 += 0.05f;
						num12 = MathHelper.Lerp(num12, lastCelestialBodyPosition.Y + 0.025f, num10);
						num18 *= 0.5f;
					}
					Vector2 v = vector * new Vector2(num15, num12);
					Vector2 v2 = vector * new Vector2(num15, num12 - num17);
					if (!flag)
					{
						float num27 = Main.GlobalTimeWrappedHourly * 0.1f;
						v += ((num27 + 0.3f) * ((float)Math.PI * 2f)).ToRotationVector2() * 2f;
						v2 += ((num27 * 0.8f + 0.67f) * ((float)Math.PI * 2f)).ToRotationVector2() * 2f;
						v2.Y += (float)Math.Sin((num27 + num10) * ((float)Math.PI * 2f) * 3f) * 15f - 15f;
						v.Y += (float)Math.Sin((num27 + num10) * ((float)Math.PI * 2f) * 0.5f) * 1f;
						v2.Y += (float)Math.Sin((num27 + num10) * ((float)Math.PI * 2f) * 0.5f) * 1f;
						v.X += (float)Math.Sin((num27 + num10) * ((float)Math.PI * 2f) * 1f) * 3f;
						v2.X += (float)Math.Sin((num27 + num10) * ((float)Math.PI * 2f) * 0.75f) * 3f;
					}
					Color color = Main.hslToRgb((float)((double)num19 + Math.Cos(num10 * ((float)Math.PI * 2f) * num7) * 0.1) % 1f, saturation, 0.5f);
					Color color2 = Main.hslToRgb((float)((double)num19 + Math.Cos(num10 * ((float)Math.PI * 2f) * num7) * 0.1 + (double)num8) % 1f, saturation, luminosity);
					if (i == 0 && num9 == 19)
					{
						lastSkyColor = color;
					}
					float num28 = num16 * skyOpacity * num18 * num4;
					if (flag)
					{
						float fromValue2 = (vector * new Vector2(num15, num12 - num17 * 0.25f)).Distance(vector * lastCelestialBodyPosition);
						num28 *= Utils.Remap(fromValue2, 29f, 60f, 0f, 1f);
						float num29 = 505f;
						float num30 = 1f - num10;
						num30 *= num30 * num30;
						if (i == 1)
						{
							v.X -= num29 * num30;
							v2.X -= num29 * num30;
							num28 -= num10 * num10 * 0.36f;
						}
						if (i == 2)
						{
							v.X += num29 * num30;
							v2.X += num29 * num30;
							num28 -= num10 * num10 * 0.36f;
						}
					}
					vertexStrip.AddVertexPair(v, v2, num10, color * num28, color2 * num28);
				}
				miscShaderData.Apply();
				vertexStrip.PrepareIndices(includeBacksides: true);
				vertexStrip.DrawTrail();
			}
			Main.LatestSurfaceBackgroundBeginner.Begin(Main.spriteBatch);
		}

		public static void ModifyTileColor(ref Color tileColor, float intensity)
		{
			if (!(SkyManager.Instance["Aurora"] is AuroraSky auroraSky))
			{
				return;
			}
			float opacity = auroraSky._opacity;
			if (!(opacity <= 0f))
			{
				MoonPhase moonPhase = Main.GetMoonPhase();
				if (moonPhase != MoonPhase.QuarterAtLeft)
				{
					Color lastSkyColor = auroraSky._lastSkyColor;
					lastSkyColor.A = byte.MaxValue;
					tileColor = Color.Lerp(tileColor, lastSkyColor, opacity * intensity);
				}
			}
		}

		public override void Activate(Vector2 position, params object[] args)
		{
			_isActive = true;
			_isLeaving = false;
		}

		public override void Deactivate(params object[] args)
		{
			_isLeaving = true;
		}

		public override void Reset()
		{
			_opacity = 0f;
			_isActive = false;
		}

		public override bool IsActive()
		{
			return _isActive;
		}
	}
}
