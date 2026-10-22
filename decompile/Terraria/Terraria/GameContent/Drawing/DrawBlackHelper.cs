using Microsoft.Xna.Framework;

namespace Terraria.GameContent.Drawing
{
	public struct DrawBlackHelper
	{
		private readonly uint layer;

		private readonly Vector2 drawOffset;

		private int y;

		private int startX;

		private int endX;

		public DrawBlackHelper(uint layer, Vector2 drawOffset)
		{
			this.layer = layer;
			this.drawOffset = drawOffset;
			y = 0;
			startX = 0;
			endX = 0;
		}

		public void DrawBlack(int x, int y)
		{
			if (y == this.y && x == endX)
			{
				endX++;
				return;
			}
			EndStrip();
			this.y = y;
			startX = x;
			endX = x + 1;
		}

		public void EndStrip()
		{
			if (startX != endX)
			{
				Vector2 vector = new Vector2(startX << 4, y << 4) - Main.screenPosition + drawOffset;
				Main.tileBatch.SetLayer(layer, 0);
				Main.tileBatch.Draw(TextureAssets.BlackTile.Value, new Vector4(vector.X, vector.Y, endX - startX << 4, 16f), Color.Black);
			}
		}
	}
}
