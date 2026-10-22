using Microsoft.Xna.Framework;

namespace Terraria.ID
{
	public class PlayerVoiceID
	{
		public static class Sets
		{
			public static SetFactory Factory = new SetFactory(4);

			public static Color[] Colors = Factory.CreateCustomSet(Color.White, 1, Color.CornflowerBlue, 2, Color.HotPink, 3, Color.LimeGreen);
		}

		public static int[] VariantOrder = new int[3] { 1, 2, 3 };

		public const int None = 0;

		public const int Male = 1;

		public const int Female = 2;

		public const int Other = 3;

		public const int Count = 4;
	}
}
