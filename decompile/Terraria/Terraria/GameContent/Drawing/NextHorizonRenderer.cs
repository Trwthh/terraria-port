using System;
using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Microsoft.Xna.Framework.Input;
using ReLogic.Content;
using Terraria.DataStructures;
using Terraria.GameContent.Skies;
using Terraria.Graphics.Shaders;

namespace Terraria.GameContent.Drawing
{
	public class NextHorizonRenderer : IHorizonRenderer
	{
		private static Asset<Texture2D>[] _sunriseTextures;

		private static Asset<Texture2D>[] _sunsetTextures;

		private static Asset<Texture2D> _sunflareGradientTexture;

		private static Asset<Texture2D> _sunflareGradientDitherTexture;

		private static Asset<Texture2D> _sunflarePointBlurryTexture;

		private static Asset<Texture2D> _sunflarePointSharpTexture;

		private static Asset<Texture2D> _bokehTexture;

		private static Asset<Texture2D> _spectraTexture;

		private static Asset<Texture2D> _sunflare1Texture;

		private static Asset<Texture2D> _sunflare2Texture;

		private List<DrawData> _drawData = new List<DrawData>(200);

		private void LoadTextures()
		{
			if (_sunriseTextures == null)
			{
				_sunriseTextures = new Asset<Texture2D>[4]
				{
					Main.Assets.Request<Texture2D>("Images/Misc/Sunrise/Sunrise_Blue"),
					Main.Assets.Request<Texture2D>("Images/Misc/Sunrise/Sunrise_Violet"),
					Main.Assets.Request<Texture2D>("Images/Misc/Sunrise/Sunrise_Yellow"),
					Main.Assets.Request<Texture2D>("Images/Misc/Sunrise/Sunrise_Aluminum")
				};
				_sunsetTextures = new Asset<Texture2D>[4]
				{
					Main.Assets.Request<Texture2D>("Images/Misc/Sunset/Sunset_Blue"),
					Main.Assets.Request<Texture2D>("Images/Misc/Sunset/Sunset_Dark"),
					Main.Assets.Request<Texture2D>("Images/Misc/Sunset/Sunset_Pink"),
					Main.Assets.Request<Texture2D>("Images/Misc/Sunset/Sunset_Red")
				};
				_sunflareGradientTexture = Main.Assets.Request<Texture2D>("Images/Misc/Sunflare/colorgradient");
				_sunflareGradientDitherTexture = Main.Assets.Request<Texture2D>("Images/Misc/Sunflare/colorgradientdither");
				_sunflarePointBlurryTexture = Main.Assets.Request<Texture2D>("Images/Misc/Sunflare/Lens/PointBlurry");
				_sunflarePointSharpTexture = Main.Assets.Request<Texture2D>("Images/Misc/Sunflare/Lens/PointSharp");
				_sunflare1Texture = Main.Assets.Request<Texture2D>("Images/Misc/Sunflare/flare1");
				_sunflare2Texture = Main.Assets.Request<Texture2D>("Images/Misc/Sunflare/flare2");
				_bokehTexture = Main.Assets.Request<Texture2D>("Images/Misc/Sunflare/Lens/Flare1");
				_spectraTexture = Main.Assets.Request<Texture2D>("Images/Misc/Sunflare/Lens/Flare2");
			}
		}

		private static Rectangle GetGradientRect()
		{
			int num = 400;
			int val = (int)((1.0 - Utils.GetLerpValue(40.0, Main.worldSurface, Main.screenPosition.Y / 16f)) * (double)num);
			int y = Math.Max(0, val) - num;
			return new Rectangle(0, y, Main.screenWidth, Main.screenHeight + num);
		}

		public void DrawHorizon()
		{
			if (!Main.ShouldDrawSurfaceBackground())
			{
				return;
			}
			LoadTextures();
			int sunriseSunsetTextureIndex = GetSunriseSunsetTextureIndex();
			Asset<Texture2D> asset = _sunriseTextures[sunriseSunsetTextureIndex % _sunriseTextures.Length];
			Asset<Texture2D> asset2 = _sunsetTextures[sunriseSunsetTextureIndex % _sunsetTextures.Length];
			GetVisibilities(out var sunsetVisibility, out var sunriseVisibility, out var _);
			SpriteBatch spriteBatch = Main.spriteBatch;
			Rectangle gradientRect = GetGradientRect();
			foreach (BackgroundGradientDrawer backgroundDrawer in SunGradients.BackgroundDrawers)
			{
				backgroundDrawer.Draw();
			}
			if (sunriseVisibility != 0f)
			{
				spriteBatch.Draw(asset.Value, gradientRect, Color.White * sunriseVisibility);
			}
			if (sunsetVisibility != 0f)
			{
				spriteBatch.Draw(asset2.Value, gradientRect, Color.White * sunsetVisibility);
			}
		}

		public float GetMoonStrength()
		{
			return Utils.Remap(Math.Abs(4 - Main.moonPhase), 0f, 4f, 0f, 1f);
		}

		public void DrawSurfaceLayer(int layerIndex)
		{
			if (Main.ShouldDrawSurfaceBackground())
			{
				LoadTextures();
				SpriteBatch spriteBatch = Main.spriteBatch;
				Rectangle gradientRect = GetGradientRect();
				GetVisibilities(out var sunsetVisibility, out var sunriseVisibility, out var _);
				int sunriseSunsetTextureIndex = GetSunriseSunsetTextureIndex();
				List<Color[]> sunrises = SunGradients.Sunrises;
				Color[] array = sunrises[sunriseSunsetTextureIndex % sunrises.Count];
				List<Color[]> sunsets = SunGradients.Sunsets;
				Color[] array2 = sunsets[sunriseSunsetTextureIndex % sunsets.Count];
				Color color = Color.Transparent;
				BlendColor(ref color, array2[0], sunsetVisibility);
				BlendColor(ref color, array[0], sunriseVisibility);
				float num = 1f;
				switch (layerIndex)
				{
				case 0:
					num = 1f;
					break;
				case 1:
					num = 0.75f;
					break;
				case 2:
					num = 0.5f;
					break;
				case 3:
					num = 0.5f;
					break;
				}
				_ = _sunriseTextures[sunriseSunsetTextureIndex % _sunriseTextures.Length];
				_ = _sunsetTextures[sunriseSunsetTextureIndex % _sunsetTextures.Length];
				_ = Main.tileBatch;
				if (layerIndex == 3)
				{
					float num2 = 0.6f;
					num = 1f;
					spriteBatch.Draw(_sunflareGradientTexture.Value, gradientRect, null, array[0] * num * sunriseVisibility * num2, 0f, Vector2.Zero, SpriteEffects.FlipHorizontally, 0f);
					spriteBatch.Draw(_sunflareGradientTexture.Value, gradientRect, null, array2[0] * num * sunsetVisibility * num2, 0f, Vector2.Zero, SpriteEffects.None, 0f);
				}
			}
		}

		private int GetSunriseSunsetTextureIndex()
		{
			return Main.HorizonPhase;
		}

		public void ModifyHorizonLight(ref Color color)
		{
			if (Main.ShouldDrawSurfaceBackground())
			{
				GetVisibilities(out var sunsetVisibility, out var sunriseVisibility, out var _);
				int sunriseSunsetTextureIndex = GetSunriseSunsetTextureIndex();
				List<Color[]> sunrises = SunGradients.Sunrises;
				Color[] gradient = sunrises[sunriseSunsetTextureIndex % sunrises.Count];
				List<Color[]> sunsets = SunGradients.Sunsets;
				Color[] gradient2 = sunsets[sunriseSunsetTextureIndex % sunsets.Count];
				BlendColor(ref color, gradient2, sunsetVisibility);
				BlendColor(ref color, gradient, sunriseVisibility);
			}
		}

		public void DrawSun(Vector2 sunPosition)
		{
			GetVisibilities(out var sunsetVisibility, out var sunriseVisibility, out var celestialVisibility);
			sunsetVisibility *= celestialVisibility;
			sunriseVisibility *= celestialVisibility;
			LoadTextures();
			Color color = new Color(255, 255, 255, 0);
			SpriteBatch spriteBatch = Main.spriteBatch;
			spriteBatch.Draw(_sunflare1Texture.Value, sunPosition, null, color * sunsetVisibility * 0.75f, 0f, _sunflare1Texture.Size() / 2f, 3f, SpriteEffects.None, 0f);
			spriteBatch.Draw(_sunflare1Texture.Value, sunPosition, null, color * sunsetVisibility * 0.35f, 0f, _sunflare1Texture.Size() / 2f, 2f, SpriteEffects.None, 0f);
			spriteBatch.Draw(_sunflare2Texture.Value, sunPosition, null, color * sunriseVisibility * 0.7f * 0.5f, 0f, _sunflare2Texture.Size() / 2f, 2f, SpriteEffects.None, 0f);
			spriteBatch.Draw(_sunflare2Texture.Value, sunPosition, null, color * sunriseVisibility * 0.3f * 0.5f, 0f, _sunflare2Texture.Size() / 2f, 1.5f, SpriteEffects.None, 0f);
			spriteBatch.Draw(_sunflare2Texture.Value, sunPosition, null, color * sunriseVisibility * 0.2f * 0.5f, 0f, _sunflare2Texture.Size() / 2f, 1f, SpriteEffects.None, 0f);
		}

		private void BlendColor(ref Color color, Color[] gradient, float opacity)
		{
			BlendColor(ref color, gradient[gradient.Length / 2], opacity);
		}

		private void BlendColor(ref Color color, Color colorToChoose, float opacity)
		{
			if (!(opacity <= 0f))
			{
				color = Color.Lerp(value2: new Color(Math.Max(color.R, colorToChoose.R), Math.Max(color.G, colorToChoose.G), Math.Max(color.B, colorToChoose.B), Math.Max(color.A, colorToChoose.A)), value1: color, amount: opacity);
			}
		}

		private static void GetVisibilities(out float sunsetVisibility, out float sunriseVisibility, out float celestialVisibility)
		{
			sunsetVisibility = 1f;
			sunriseVisibility = 1f;
			celestialVisibility = GetCelestialEffectPower();
			float num = 1f;
			num *= Main.atmo;
			float num2 = 1f - Main.cloudAlpha;
			num *= num2 * num2;
			num *= 1f - Main.SmoothedMushroomLightInfluence;
			sunriseVisibility *= num;
			sunsetVisibility *= num;
			double time = Main.time;
			double num3 = 54000.0;
			if (Main.dayTime)
			{
				float fromMin = 3600f;
				int num4 = 2700;
				float fromMax = 10800f;
				float num5 = -10800f;
				float num6 = -3600f;
				sunriseVisibility *= Utils.Remap((float)time, 0f, num4, 0f, 1f) * Utils.Remap((float)time, fromMin, fromMax, 1f, 0f);
				float num7 = Utils.Remap((float)time, (float)num3 + num5, (float)num3 + num6, 0f, 1f);
				float num8 = Utils.Remap((float)time, (float)num3 + num6, (float)num3, 1f, 0f);
				sunsetVisibility *= num7 * num8 * num8;
				if (Main.eclipse)
				{
					sunsetVisibility = 0f;
					sunriseVisibility = 0f;
				}
			}
			else
			{
				sunriseVisibility = 0f;
				sunsetVisibility = 0f;
			}
			if (Main.gameMenu && WorldGen.drunkWorldGen)
			{
				sunsetVisibility = (sunriseVisibility = 0f);
			}
		}

		public void CloudsStart()
		{
			_drawData.Clear();
		}

		public void DrawCloud(float globalCloudAlpha, Cloud theCloud, int cloudPass, float cY)
		{
			Asset<Texture2D> asset = TextureAssets.Cloud[theCloud.type];
			Vector2 position = new Vector2(theCloud.position.X, cY) + asset.Size() / 2f;
			Color cloudColor = theCloud.cloudColor(Main.ColorOfTheSkies);
			OriginalColorsForCloud(theCloud, cloudPass, ref cloudColor);
			if (Main.atmo < 1f)
			{
				cloudColor *= Main.atmo;
			}
			_drawData.Add(new DrawData(asset.Value, position, null, cloudColor * globalCloudAlpha, theCloud.rotation, asset.Size() / 2f, theCloud.scale, theCloud.spriteDir));
		}

		private void OriginalColorsForCloud(Cloud theCloud, int cloudPass, ref Color cloudColor)
		{
			if (cloudPass == 1)
			{
				float num = theCloud.scale * 0.8f;
				float num2 = (theCloud.scale + 1f) / 2f * 0.9f;
				cloudColor.R = (byte)((float)(int)cloudColor.R * num);
				cloudColor.G = (byte)((float)(int)cloudColor.G * num2);
			}
		}

		private void BetterColorsForClouds(Cloud theCloud, int cloudPass, ref Vector2 cloudDrawPosition, ref Color cloudColor)
		{
			float num = 0f;
			switch (cloudPass)
			{
			case 1:
				num = 0.7f;
				break;
			case 2:
				num = 0.35f;
				break;
			}
			if (Main.keyState.IsKeyDown(Keys.LeftShift))
			{
				num = 0f;
			}
			if (num > 0f)
			{
				GetVisibilities(out var sunsetVisibility, out var sunriseVisibility, out var _);
				int sunriseSunsetTextureIndex = GetSunriseSunsetTextureIndex();
				List<Color[]> sunrises = SunGradients.Sunrises;
				Color[] gradient = sunrises[sunriseSunsetTextureIndex % sunrises.Count];
				List<Color[]> sunsets = SunGradients.Sunsets;
				Color[] gradient2 = sunsets[sunriseSunsetTextureIndex % sunsets.Count];
				float normalizedScreenHeight = cloudDrawPosition.Y / (float)Main.screenHeight;
				float alpha = theCloud.Alpha;
				BlendColorAlongGradientBasedOnHeight(ref cloudColor, sunsetVisibility, normalizedScreenHeight, gradient2, alpha);
				BlendColorAlongGradientBasedOnHeight(ref cloudColor, sunriseVisibility, normalizedScreenHeight, gradient, alpha);
			}
		}

		private void BlendColorAlongGradientBasedOnHeight(ref Color color, float visibility, float normalizedScreenHeight, Color[] gradient, float opacity)
		{
			float num = MathHelper.Clamp(normalizedScreenHeight * (float)gradient.Length, 0f, gradient.Length - 1);
			float num2 = num % 1f;
			int num3 = (int)Math.Floor(num);
			if (num2 == 0f || num3 == gradient.Length - 1)
			{
				BlendColor(ref color, gradient[num3] * opacity, visibility);
				return;
			}
			Color colorToChoose = Color.Lerp(gradient[num3], gradient[num3 + 1], num2) * opacity;
			BlendColor(ref color, colorToChoose, visibility);
		}

		private static float GetCelestialEffectPower()
		{
			float num = 1800f;
			float num2 = 1800f;
			float toMax = 0f;
			if (Main.dayTime)
			{
				return Utils.Remap((float)Main.time, 0f, num * 2f, 0f, 1f) * Utils.Remap((float)Main.time, 54000f - num, 54000f, 1f, toMax);
			}
			return Utils.Remap((float)Main.time, 0f, num2 * 2f, 0f, 1f) * Utils.Remap((float)Main.time, 32400f - num2, 32400f, 1f, 0f);
		}

		public void CloudsEnd()
		{
			if (_drawData.Count == 0)
			{
				return;
			}
			Main.spriteBatch.End();
			SpriteDrawBuffer spriteBuffer = Main.spriteBuffer;
			foreach (DrawData drawDatum in _drawData)
			{
				drawDatum.Draw(spriteBuffer);
			}
			MiscShaderData miscShaderData = GameShaders.Misc["HorizonClouds"];
			miscShaderData.UseSpriteTransformMatrix(Main.LatestSurfaceBackgroundBeginner.transformMatrix);
			HorizonHelper.GetCelestialBodyColors(out var sunColor, out var moonColor);
			Color tileColor = (Main.dayTime ? sunColor : moonColor);
			AuroraSky.ModifyTileColor(ref tileColor, 1f);
			miscShaderData.UseColor(tileColor);
			Vector2 celestialBodyPosition = GetCelestialBodyPosition();
			GetVisibilities(out var sunsetVisibility, out var sunriseVisibility, out var celestialVisibility);
			float num = Math.Max(sunsetVisibility, sunriseVisibility) * celestialVisibility;
			if (!Main.dayTime)
			{
				num = Math.Max(num, celestialVisibility * 0.15f);
			}
			num *= Utils.Clamp(1f - Main.cloudBGAlpha, 0f, 1f);
			miscShaderData.UseShaderSpecificData(new Vector4(celestialBodyPosition.X, celestialBodyPosition.Y, num, 0f));
			for (int i = 0; i < _drawData.Count; i++)
			{
				miscShaderData.Apply(_drawData[i]);
				spriteBuffer.DrawSingle(i);
			}
			spriteBuffer.Unbind();
			Main.LatestSurfaceBackgroundBeginner.Begin(Main.spriteBatch);
		}

		private static Vector2 GetCelestialBodyPosition()
		{
			return Main.LastCelestialBodyPosition * Main.ScreenSize.ToVector2();
		}

		public void DrawLensFlare()
		{
			if (Main.ShouldDrawSurfaceBackground() && Main.HorizonHelper.SunVisibilityEnabled)
			{
				SpriteBatch spriteBatch = Main.spriteBatch;
				Vector2 celestialBodyPosition = GetCelestialBodyPosition();
				Vector2 screenCenter = Main.ScreenSize.ToVector2() / 2f;
				GetVisibilities(out var sunsetVisibility, out var sunriseVisibility, out var celestialVisibility);
				float num = AdjustIntensity(sunriseVisibility, celestialVisibility);
				float num2 = AdjustIntensity(sunsetVisibility, celestialVisibility);
				if (!((double)num <= 0.01) || !((double)num2 <= 0.01))
				{
					Main.LatestSurfaceBackgroundBeginner.Begin(spriteBatch, SpriteSortMode.Immediate);
					EffectPass effectPass = Main.pixelShader.CurrentTechnique.Passes[0];
					MiscShaderData miscShaderData = GameShaders.Misc["LensFlare"];
					miscShaderData.UseImage1(Main.HorizonHelper.SunVisibilityPixelTexture);
					miscShaderData.Apply();
					DrawSunriseFlare(spriteBatch, celestialBodyPosition, screenCenter, num);
					DrawSunsetFlare(spriteBatch, celestialBodyPosition, screenCenter, num2);
					spriteBatch.End();
					effectPass.Apply();
				}
			}
		}

		private float AdjustIntensity(float temporalIntensity, float celestialVisibility)
		{
			float num = temporalIntensity;
			num *= celestialVisibility;
			num *= num * num;
			int sunScorchCounter = Main.SceneMetrics.PerspectivePlayer.sunScorchCounter;
			if (sunScorchCounter > 0)
			{
				float lerpValue = Utils.GetLerpValue(0f, 300f, sunScorchCounter, clamped: true);
				lerpValue = 1f - lerpValue;
				num = 1f - lerpValue * lerpValue;
				num *= celestialVisibility;
				num *= 5f;
			}
			return num;
		}

		private void DrawSunsetFlare(SpriteBatch spriteBatch, Vector2 sunPosition, Vector2 screenCenter, float intensity)
		{
			if (!(intensity <= 0.01f))
			{
				LoadTextures();
				LensFlareElement lensFlareElement = default(LensFlareElement);
				lensFlareElement.Texture = _sunflarePointBlurryTexture;
				lensFlareElement.RepeatTimes = 3;
				lensFlareElement.DistanceStart = 0.33f;
				lensFlareElement.DistanceAlongIndex = 0.05f;
				lensFlareElement.ScaleStart = 0.3f;
				lensFlareElement.ScaleOverIndex = -0.04f;
				lensFlareElement.Color = new Color(43, 32, 0, 0) * 0.47058824f;
				lensFlareElement.IntensityOverIndex = -0.125f;
				lensFlareElement.Draw(spriteBatch, sunPosition, screenCenter, intensity);
				lensFlareElement = default(LensFlareElement);
				lensFlareElement.Texture = _sunflarePointSharpTexture;
				lensFlareElement.RepeatTimes = 3;
				lensFlareElement.DistanceStart = 0.03f;
				lensFlareElement.DistanceAlongIndex = 0.05f;
				lensFlareElement.ScaleStart = 0.3f;
				lensFlareElement.ScaleOverIndex = 0.04f;
				lensFlareElement.Color = new Color(43, 32, 0, 0) * 0.47058824f;
				lensFlareElement.IntensityOverIndex = -0.125f;
				lensFlareElement.Draw(spriteBatch, sunPosition, screenCenter, intensity);
				lensFlareElement = default(LensFlareElement);
				lensFlareElement.Texture = _sunflarePointBlurryTexture;
				lensFlareElement.RepeatTimes = 1;
				lensFlareElement.DistanceStart = 0.41f;
				lensFlareElement.ScaleStart = 0.3f;
				lensFlareElement.Color = new Color(255, 0, 65, 0) * 0.11764706f;
				lensFlareElement.Draw(spriteBatch, sunPosition, screenCenter, intensity);
				lensFlareElement = default(LensFlareElement);
				lensFlareElement.Texture = _bokehTexture;
				lensFlareElement.RepeatTimes = 1;
				lensFlareElement.DistanceStart = 0.475f;
				lensFlareElement.ScaleStart = 0.3f;
				lensFlareElement.Color = new Color(255, 255, 255, 0) * (8f / 51f);
				lensFlareElement.Draw(spriteBatch, sunPosition, screenCenter, intensity);
				lensFlareElement = default(LensFlareElement);
				lensFlareElement.Texture = _bokehTexture;
				lensFlareElement.RepeatTimes = 6;
				lensFlareElement.DistanceStart = 0.225f;
				lensFlareElement.DistanceAlongIndex = 0.04f;
				lensFlareElement.ScaleStart = 0.24f;
				lensFlareElement.ScaleOverIndex = -0.04f;
				lensFlareElement.Color = new Color(255, 255, 255, 0) * (4f / 51f);
				lensFlareElement.Draw(spriteBatch, sunPosition, screenCenter, intensity);
				lensFlareElement = default(LensFlareElement);
				lensFlareElement.Texture = _sunflarePointBlurryTexture;
				lensFlareElement.RepeatTimes = 1;
				lensFlareElement.DistanceStart = 0.6f;
				lensFlareElement.ScaleStart = 1f;
				lensFlareElement.Color = new Color(255, 157, 0, 0) * (8f / 51f);
				lensFlareElement.Draw(spriteBatch, sunPosition, screenCenter, intensity);
				lensFlareElement = default(LensFlareElement);
				lensFlareElement.Texture = _spectraTexture;
				lensFlareElement.RepeatTimes = 1;
				lensFlareElement.DistanceStart = 0.65f;
				lensFlareElement.ScaleStart = 0.4f;
				lensFlareElement.Rotation = (float)Math.PI;
				lensFlareElement.Color = new Color(255, 255, 255, 0) * (2f / 51f);
				lensFlareElement.Draw(spriteBatch, sunPosition, screenCenter, intensity);
			}
		}

		private void DrawSunriseFlare(SpriteBatch spriteBatch, Vector2 sunPosition, Vector2 screenCenter, float intensity)
		{
			if (!(intensity <= 0.01f))
			{
				LoadTextures();
				LensFlareElement lensFlareElement = default(LensFlareElement);
				lensFlareElement.Texture = _sunflarePointSharpTexture;
				lensFlareElement.RepeatTimes = 3;
				lensFlareElement.DistanceStart = 0.33f;
				lensFlareElement.DistanceAlongIndex = 0.05f;
				lensFlareElement.ScaleStart = 0.3f;
				lensFlareElement.ScaleOverIndex = -0.04f;
				lensFlareElement.Color = new Color(0, 32, 43, 0) * 0.47058824f;
				lensFlareElement.IntensityOverIndex = -0.125f;
				lensFlareElement.Draw(spriteBatch, sunPosition, screenCenter, intensity);
				lensFlareElement = default(LensFlareElement);
				lensFlareElement.Texture = _sunflarePointSharpTexture;
				lensFlareElement.RepeatTimes = 3;
				lensFlareElement.DistanceStart = 0.03f;
				lensFlareElement.DistanceAlongIndex = 0.05f;
				lensFlareElement.ScaleStart = 0.3f;
				lensFlareElement.ScaleOverIndex = 0.04f;
				lensFlareElement.Color = new Color(0, 32, 43, 0) * 0.47058824f;
				lensFlareElement.IntensityOverIndex = -0.125f;
				lensFlareElement.Draw(spriteBatch, sunPosition, screenCenter, intensity);
				lensFlareElement = default(LensFlareElement);
				lensFlareElement.Texture = _sunflarePointBlurryTexture;
				lensFlareElement.RepeatTimes = 1;
				lensFlareElement.DistanceStart = 0.41f;
				lensFlareElement.ScaleStart = 0.3f;
				lensFlareElement.Color = new Color(65, 0, 255, 0) * 0.11764706f;
				lensFlareElement.Draw(spriteBatch, sunPosition, screenCenter, intensity);
				lensFlareElement = default(LensFlareElement);
				lensFlareElement.Texture = _bokehTexture;
				lensFlareElement.RepeatTimes = 1;
				lensFlareElement.DistanceStart = 0.525f;
				lensFlareElement.Rotation = 0.01f;
				lensFlareElement.ScaleStart = 0.3f;
				lensFlareElement.Color = new Color(255, 255, 255, 0) * (8f / 51f);
				lensFlareElement.Draw(spriteBatch, sunPosition, screenCenter, intensity);
				lensFlareElement = default(LensFlareElement);
				lensFlareElement.Texture = _bokehTexture;
				lensFlareElement.RepeatTimes = 6;
				lensFlareElement.DistanceStart = 0.225f;
				lensFlareElement.DistanceAlongIndex = 0.04f;
				lensFlareElement.ScaleStart = 0.24f;
				lensFlareElement.ScaleOverIndex = -0.04f;
				lensFlareElement.Color = new Color(255, 255, 255, 0) * (4f / 51f);
				lensFlareElement.Draw(spriteBatch, sunPosition, screenCenter, intensity);
				lensFlareElement = default(LensFlareElement);
				lensFlareElement.Texture = _sunflarePointBlurryTexture;
				lensFlareElement.RepeatTimes = 1;
				lensFlareElement.DistanceStart = 0.6f;
				lensFlareElement.ScaleStart = 1f;
				lensFlareElement.Color = new Color(0, 157, 255, 0) * (8f / 51f);
				lensFlareElement.Draw(spriteBatch, sunPosition, screenCenter, intensity);
				lensFlareElement = default(LensFlareElement);
				lensFlareElement.Texture = _spectraTexture;
				lensFlareElement.RepeatTimes = 1;
				lensFlareElement.DistanceStart = 0.65f;
				lensFlareElement.ScaleStart = 0.38f;
				lensFlareElement.Rotation = (float)Math.PI;
				lensFlareElement.Color = new Color(255, 255, 255, 0) * (2f / 51f);
				lensFlareElement.Draw(spriteBatch, sunPosition, screenCenter, intensity);
			}
		}
	}
}
