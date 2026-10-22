using Newtonsoft.Json.Linq;

namespace ReLogic.Peripherals.RGB
{
	public class IntRgbGameValueTracker : ARgbGameValueTracker<int>
	{
		protected override void WriteValueToData(JObject data)
		{
			data.Add("value", _currentValue);
		}
	}
}
