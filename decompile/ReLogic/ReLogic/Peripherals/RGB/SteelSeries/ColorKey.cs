using Microsoft.Xna.Framework;
using Newtonsoft.Json.Linq;

namespace ReLogic.Peripherals.RGB.SteelSeries
{
	internal class ColorKey
	{
		private const int TimesToDenyIdenticalColors = 30;

		public string EventName;

		public string TriggerName;

		private Color _colorToShow;

		private bool _needsToSendMessage;

		public bool IsVisible;

		private int _timesDeniedColorRepeats;

		public void UpdateColor(Color color, bool isVisible)
		{
			IsVisible = isVisible;
			if (_colorToShow == color && _timesDeniedColorRepeats < 30)
			{
				_timesDeniedColorRepeats++;
				return;
			}
			_timesDeniedColorRepeats = 0;
			_colorToShow = color;
			_needsToSendMessage = true;
		}

		public JObject TryGettingRequest()
		{
			if (!_needsToSendMessage)
			{
				return null;
			}
			_needsToSendMessage = false;
			JObject jObject = new JObject();
			jObject.Add("red", _colorToShow.R);
			jObject.Add("green", _colorToShow.G);
			jObject.Add("blue", _colorToShow.B);
			JObject jObject2 = new JObject();
			jObject2.Add(TriggerName, jObject);
			JObject jObject3 = new JObject();
			jObject3.Add("frame", jObject2);
			return new JObject
			{
				{ "event", EventName },
				{ "data", jObject3 }
			};
		}
	}
}
