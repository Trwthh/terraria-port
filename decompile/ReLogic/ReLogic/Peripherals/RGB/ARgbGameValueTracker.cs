using System;
using Newtonsoft.Json.Linq;

namespace ReLogic.Peripherals.RGB
{
	public abstract class ARgbGameValueTracker
	{
		public string EventName;

		protected bool _needsToSendMessage;

		public bool IsVisible;

		public JObject TryGettingRequest()
		{
			if (!_needsToSendMessage)
			{
				return null;
			}
			_needsToSendMessage = false;
			JObject jObject = new JObject();
			WriteValueToData(jObject);
			return new JObject
			{
				{ "event", EventName },
				{ "data", jObject }
			};
		}

		protected abstract void WriteValueToData(JObject data);
	}
	public abstract class ARgbGameValueTracker<TValueType> : ARgbGameValueTracker where TValueType : IComparable
	{
		private const int TimesToDenyIdenticalValues = 30;

		protected TValueType _currentValue;

		private int _timesDeniedRepeat;

		public void Update(TValueType value, bool isVisible)
		{
			IsVisible = isVisible;
			ref TValueType currentValue = ref _currentValue;
			object obj = value;
			if (currentValue.Equals(obj) && _timesDeniedRepeat < 30)
			{
				_timesDeniedRepeat++;
				return;
			}
			_timesDeniedRepeat = 0;
			_currentValue = value;
			_needsToSendMessage = true;
		}
	}
}
