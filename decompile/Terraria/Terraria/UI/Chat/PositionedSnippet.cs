using Microsoft.Xna.Framework;

namespace Terraria.UI.Chat
{
	public struct PositionedSnippet
	{
		public readonly TextSnippet Snippet;

		public readonly int OrigIndex;

		public readonly int Line;

		public Vector2 Position;

		public Vector2 Size;

		public PositionedSnippet(TextSnippet snippet, int origIndex, int line, Vector2 position, Vector2 size)
		{
			Snippet = snippet;
			OrigIndex = origIndex;
			Line = line;
			Position = position;
			Size = size;
		}

		public void Scale(float scale)
		{
			Position *= scale;
			Size *= scale;
		}
	}
}
