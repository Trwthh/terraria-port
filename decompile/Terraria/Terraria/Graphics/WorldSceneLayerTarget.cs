using System;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;

namespace Terraria.Graphics
{
	public class WorldSceneLayerTarget
	{
		private readonly RenderTarget2D _target;

		private Vector2 _position;

		public Texture2D Texture => _target;

		public Vector2 Position => _position;

		public bool IsPartiallyOffscreen
		{
			get
			{
				if (_position == Vector2.Zero)
				{
					return true;
				}
				Vector2 vector = new Vector2(_target.Width, _target.Height);
				Vector2 vector2 = Position + vector / 2f - Main.Camera.Center;
				Vector2 vector3 = (vector - Main.Camera.ScaledSize) / 2f;
				if (!(Math.Abs(vector2.X) > vector3.X))
				{
					return Math.Abs(vector2.Y) > vector3.Y;
				}
				return true;
			}
		}

		public bool IsContentLost => _target.IsContentLost;

		public WorldSceneLayerTarget(GraphicsDevice graphicsDevice, int width, int height)
		{
			_target = new RenderTarget2D(graphicsDevice, width, height, mipMap: false, graphicsDevice.PresentationParameters.BackBufferFormat, DepthFormat.None);
		}

		public void UpdateContent(Action render)
		{
			Vector2 screenPosition = Main.screenPosition;
			Point screenSize = Main.ScreenSize;
			Vector2 zoom = Main.GameViewMatrix.Zoom;
			Vector2 center = Main.Camera.Center;
			Main.screenWidth = _target.Width - Main.offScreenRange * 2;
			Main.screenHeight = _target.Height - Main.offScreenRange * 2;
			Main.screenPosition = Utils.Round(center - Main.ScreenSize.ToVector2() / 2f);
			Main.GameViewMatrix.Zoom = Vector2.One;
			GraphicsDevice graphicsDevice = Main.instance.GraphicsDevice;
			RenderTargetBinding[] renderTargets = graphicsDevice.GetRenderTargets();
			graphicsDevice.SetRenderTarget(_target);
			graphicsDevice.Clear(Color.Transparent);
			_position = Main.screenPosition - new Vector2(Main.offScreenRange, Main.offScreenRange);
			render();
			graphicsDevice.SetRenderTargets(renderTargets);
			Main.screenPosition = screenPosition;
			Main.screenWidth = screenSize.X;
			Main.screenHeight = screenSize.Y;
			Main.GameViewMatrix.Zoom = zoom;
		}

		public void Dispose()
		{
			_target.Dispose();
		}
	}
}
