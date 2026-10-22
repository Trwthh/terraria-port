using System;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;

namespace Terraria.GameContent.Drawing
{
	public class HorizonHelper
	{
		public static bool DebugSunVisibility = false;

		private readonly int SampleAreaSize = 128;

		private readonly int SmallTextureSize = 64;

		private RenderTarget2D _tinyTarget;

		private RenderTarget2D _pixelTarget;

		private bool _targetUpToDate;

		private BlendState _horizonBlendState = new BlendState
		{
			AlphaSourceBlend = Blend.Zero,
			AlphaDestinationBlend = Blend.InverseSourceAlpha,
			ColorSourceBlend = Blend.Zero,
			ColorDestinationBlend = Blend.InverseSourceAlpha
		};

		private static Color[] MoonColors = new Color[9]
		{
			new Color(230, 235, 255),
			new Color(250, 235, 160),
			new Color(230, 255, 230),
			new Color(160, 240, 255),
			new Color(180, 255, 255),
			new Color(230, 255, 230),
			new Color(255, 180, 255),
			new Color(255, 200, 180),
			new Color(225, 180, 255)
		};

		public bool SunVisibilityEnabled => _targetUpToDate;

		public Texture2D SunVisibilityPixelTexture => _pixelTarget;

		public void UpdateSunVisibility(RenderTarget2D bigTarget)
		{
			_targetUpToDate = false;
			if (Main.ForegroundSunlightEffects && bigTarget != null)
			{
				TimeLogger.StartTimestamp fromTimestamp = TimeLogger.Start();
				GraphicsDevice graphicsDevice = Main.instance.GraphicsDevice;
				if (_tinyTarget == null || _tinyTarget.IsContentLost)
				{
					_tinyTarget = new RenderTarget2D(graphicsDevice, SmallTextureSize, SmallTextureSize, mipMap: true, SurfaceFormat.Alpha8, DepthFormat.None);
				}
				if (_pixelTarget == null || _pixelTarget.IsContentLost)
				{
					_pixelTarget = new RenderTarget2D(graphicsDevice, 1, 1, mipMap: false, SurfaceFormat.Alpha8, DepthFormat.None);
				}
				Rectangle rectangle = Utils.CenteredRectangle(Main.ReverseGravitySupport(Main.LastCelestialBodyPosition * Main.ScreenSize.ToVector2()), new Vector2(SampleAreaSize) * Main.BackgroundViewMatrix.RenderZoom);
				if (DebugSunVisibility)
				{
					Test_DrawSmallTarget(bigTarget, rectangle);
				}
				graphicsDevice.SetRenderTarget(_tinyTarget);
				graphicsDevice.Clear(Color.Transparent);
				Main.spriteBatch.Begin(SpriteSortMode.Immediate, BlendState.AlphaBlend, SamplerState.PointClamp, DepthStencilState.Default, RasterizerState.CullCounterClockwise);
				Main.spriteBatch.Draw(bigTarget, _tinyTarget.Bounds, rectangle, Color.White);
				Main.spriteBatch.End();
				graphicsDevice.SetRenderTarget(_pixelTarget);
				graphicsDevice.Clear(Color.White);
				Main.spriteBatch.Begin(SpriteSortMode.Immediate, _horizonBlendState, SamplerState.LinearClamp, DepthStencilState.Default, RasterizerState.CullCounterClockwise);
				Main.spriteBatch.Draw(_tinyTarget, _pixelTarget.Bounds, Color.White);
				Main.spriteBatch.End();
				graphicsDevice.SetRenderTarget(null);
				_targetUpToDate = true;
				TimeLogger.SunVisibility.AddTime(fromTimestamp);
			}
		}

		private void Test_DrawSmallTarget(RenderTarget2D bigTarget, Rectangle sunSampleRect)
		{
			GraphicsDevice graphicsDevice = Main.instance.GraphicsDevice;
			graphicsDevice.SetRenderTarget(bigTarget);
			Main.spriteBatch.Begin(SpriteSortMode.Immediate, new BlendState
			{
				ColorDestinationBlend = Blend.Zero,
				ColorSourceBlend = Blend.SourceAlpha,
				AlphaDestinationBlend = Blend.Zero,
				AlphaSourceBlend = Blend.SourceAlpha
			}, SamplerState.PointClamp, DepthStencilState.Default, RasterizerState.CullCounterClockwise);
			Main.spriteBatch.Draw(_tinyTarget, new Rectangle(0, 0, sunSampleRect.Width, sunSampleRect.Height), Color.White);
			Main.spriteBatch.End();
			Main.spriteBatch.Begin();
			Main.spriteBatch.Draw(TextureAssets.MagicPixel.Value, new Rectangle(sunSampleRect.Left, sunSampleRect.Top, 1, sunSampleRect.Height), Color.Red);
			Main.spriteBatch.Draw(TextureAssets.MagicPixel.Value, new Rectangle(sunSampleRect.Right, sunSampleRect.Top, 1, sunSampleRect.Height), Color.Red);
			Main.spriteBatch.Draw(TextureAssets.MagicPixel.Value, new Rectangle(sunSampleRect.Left, sunSampleRect.Top, sunSampleRect.Width, 1), Color.Red);
			Main.spriteBatch.Draw(TextureAssets.MagicPixel.Value, new Rectangle(sunSampleRect.Left, sunSampleRect.Bottom, sunSampleRect.Width, 1), Color.Red);
			Main.spriteBatch.Draw(TextureAssets.MagicPixel.Value, new Rectangle(sunSampleRect.Width, 0, 1, sunSampleRect.Height), Color.Red);
			Main.spriteBatch.Draw(TextureAssets.MagicPixel.Value, new Rectangle(0, sunSampleRect.Height, sunSampleRect.Width, 1), Color.Red);
			byte[] array = new byte[1];
			_pixelTarget.GetData(array);
			Utils.DrawBorderString(text: $"{(float)(int)array[0] / 255f:F3}", sb: Main.spriteBatch, pos: new Vector2(10f, sunSampleRect.Height + 20), color: Color.White);
			Main.spriteBatch.End();
			graphicsDevice.SetRenderTarget(null);
		}

		public static void GetCelestialBodyColors(out Color sunColor, out Color moonColor)
		{
			sunColor = new Color(255, 246, 204);
			moonColor = GetMoonColor() * GetMoonStrength();
		}

		private static Color GetMoonColor()
		{
			Color color = new Color(230, 235, 255);
			int num = Main.moonType;
			if (!TextureAssets.Moon.IndexInRange(num))
			{
				num = Utils.Clamp(num, 0, 8);
			}
			color = MoonColors[num];
			if (Main.pumpkinMoon)
			{
				color = new Color(255, 225, 180);
			}
			if (Main.snowMoon)
			{
				color = new Color(220, 220, 255);
			}
			if (WorldGen.drunkWorldGen)
			{
				color = new Color(255, 255, 255);
			}
			return color;
		}

		public static float GetMoonStrength()
		{
			return Utils.Remap(Math.Abs(4 - Main.moonPhase), 0f, 4f, 0f, 1f);
		}
	}
}
