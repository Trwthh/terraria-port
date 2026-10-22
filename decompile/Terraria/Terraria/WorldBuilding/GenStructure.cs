using Microsoft.Xna.Framework;

namespace Terraria.WorldBuilding
{
	public abstract class GenStructure : GenBase
	{
		public virtual bool Place(Point origin, StructureMap structures)
		{
			return Place(origin, structures, null);
		}

		public abstract bool Place(Point origin, StructureMap structures, GenerationProgress progress);
	}
}
