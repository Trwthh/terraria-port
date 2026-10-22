using Terraria.DataStructures;

namespace Terraria.Modules
{
	public class TilePlacementHooksModule
	{
		public PlacementHook check;

		public PlacementHook postPlaceEveryone;

		public PlacementHook postPlaceMyPlayer;

		public PlacementHook placeOverride;

		public GetStyleMethod getStyleMethod;

		public TilePlacementHooksModule(TilePlacementHooksModule copyFrom = null)
		{
			if (copyFrom == null)
			{
				check = default(PlacementHook);
				postPlaceEveryone = default(PlacementHook);
				postPlaceMyPlayer = default(PlacementHook);
				placeOverride = default(PlacementHook);
				getStyleMethod = null;
			}
			else
			{
				check = copyFrom.check;
				postPlaceEveryone = copyFrom.postPlaceEveryone;
				postPlaceMyPlayer = copyFrom.postPlaceMyPlayer;
				placeOverride = copyFrom.placeOverride;
				getStyleMethod = copyFrom.getStyleMethod;
			}
		}
	}
}
