using System.Collections.Generic;
using System.Linq;
using Microsoft.Xna.Framework;
using Newtonsoft.Json.Linq;
using SteelSeries.GameSense;

namespace ReLogic.Peripherals.RGB.SteelSeries
{
	internal class SteelSeriesKeyboard : RgbKeyboard, IGameSenseDevice, IGameSenseUpdater
	{
		public const string EVENT_ID_BITMAP = "DO_RAINBOWS";

		private const int HowManyKeysGameSenseKeyboardUses = 132;

		private const int Rows = 6;

		private const int Columns = 22;

		private readonly int[][] _keyboardKeyColors = new int[132][];

		private readonly Dictionary<string, ColorKey> _keyboardTriggersForLookup = new Dictionary<string, ColorKey>();

		private readonly List<ColorKey> _keyboardTriggersForIteration = new List<ColorKey>();

		private readonly List<ARgbGameValueTracker> eventTrackers = new List<ARgbGameValueTracker>();

		private int _timesSent;

		private List<JObject> _updateRequestsList = new List<JObject>();

		public SteelSeriesKeyboard(DeviceColorProfile colorProfile)
			: base(RgbDeviceVendor.SteelSeries, Fragment.FromGrid(new Rectangle(0, 0, 22, 6)), colorProfile)
		{
			for (int i = 0; i < _keyboardKeyColors.Length; i++)
			{
				_keyboardKeyColors[i] = new int[3];
			}
		}

		public override void Render(IEnumerable<RgbKey> keys)
		{
			foreach (RgbKey key in keys)
			{
				if (_keyboardTriggersForLookup.TryGetValue(key.KeyTriggerName, out var value))
				{
					Color color = ProcessLedColor(key.CurrentColor);
					value.UpdateColor(color, key.IsVisible);
				}
			}
		}

		public override void Present()
		{
			for (int i = 0; i < base.LedCount; i++)
			{
				Vector4 processedLedColor = GetProcessedLedColor(i);
				int[] obj = _keyboardKeyColors[i];
				obj[0] = (int)(processedLedColor.X * 255f);
				obj[1] = (int)(processedLedColor.Y * 255f);
				obj[2] = (int)(processedLedColor.Z * 255f);
			}
		}

		public List<JObject> TryGetEventUpdateRequest()
		{
			_updateRequestsList.Clear();
			_updateRequestsList.Add(GetBitmapRequestForFullKeyboard());
			for (int i = 0; i < _keyboardTriggersForIteration.Count; i++)
			{
				JObject jObject = _keyboardTriggersForIteration[i].TryGettingRequest();
				if (jObject != null)
				{
					_updateRequestsList.Add(jObject);
				}
			}
			return _updateRequestsList;
		}

		private JObject GetBitmapRequestForFullKeyboard()
		{
			JObject jObject = new JObject();
			jObject.Add("bitmap", JToken.FromObject(_keyboardKeyColors));
			List<string> list = (from x in _keyboardTriggersForIteration
				where x.IsVisible
				select x.EventName).ToList();
			IEnumerable<string> collection = from x in eventTrackers
				where x.IsVisible
				select x.EventName;
			list.AddRange(collection);
			jObject.Add("excluded-events", JToken.FromObject(list));
			JObject jObject2 = new JObject();
			jObject2.Add("frame", jObject);
			jObject2.Add("value", _timesSent);
			_timesSent++;
			if (_timesSent >= 100)
			{
				_timesSent = 1;
			}
			return new JObject
			{
				{ "event", "DO_RAINBOWS" },
				{ "data", jObject2 }
			};
		}

		public void CollectEventsToTrack(Bind_Event[] bindEvents, ARgbGameValueTracker[] miscEvents)
		{
			foreach (Bind_Event bind_Event in bindEvents)
			{
				if (bind_Event.handlers.Length >= 1 && bind_Event.handlers[0] is ContextColorEventHandlerType contextColorEventHandlerType)
				{
					ColorKey colorKey = new ColorKey
					{
						EventName = bind_Event.eventName,
						TriggerName = contextColorEventHandlerType.ContextFrameKey
					};
					_keyboardTriggersForLookup.Add(contextColorEventHandlerType.ContextFrameKey, colorKey);
					_keyboardTriggersForIteration.Add(colorKey);
				}
			}
			eventTrackers.AddRange(miscEvents);
		}
	}
}
