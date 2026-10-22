using Microsoft.Xna.Framework;

namespace Terraria.WorldBuilding
{
	public struct LandmassData
	{
		public LandmassDataType DataType;

		public Vector2 Position;

		public int RadiusOrHalfSize;

		public int Style;

		public int ExtraData;

		public Vector2 Top
		{
			get
			{
				return Position - new Vector2(0f, RadiusOrHalfSize);
			}
			set
			{
				Position = value + new Vector2(0f, RadiusOrHalfSize);
			}
		}
	}
}
