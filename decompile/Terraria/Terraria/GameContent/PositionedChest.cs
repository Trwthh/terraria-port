using Microsoft.Xna.Framework;

namespace Terraria.GameContent
{
	public struct PositionedChest
	{
		public Chest chest;

		public Vector2 position;

		public PositionedChest(Chest chest, Vector2 position)
		{
			this.chest = chest;
			this.position = position;
		}
	}
}
