using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Newtonsoft.Json.Linq;
using SteelSeries.GameSense;
using SteelSeries.GameSense.DeviceZone;

namespace ReLogic.Peripherals.RGB.SteelSeries
{
	internal class SteelSeriesSecondaryDeviceByZone : RgbDevice, IGameSenseDevice, IGameSenseUpdater
	{
		private string _zoneTarget;

		private int _xPosition;

		private int _yPosition;

		private ColorKey _colorKey;

		private List<JObject> _requestList = new List<JObject>();

		public SteelSeriesSecondaryDeviceByZone(DeviceColorProfile colorProfile, RgbDeviceType type, string zoneNameToCheck, int xPosition, int yPosition)
			: base(RgbDeviceVendor.SteelSeries, type, Fragment.FromGrid(new Rectangle(xPosition, yPosition, 1, 1)), colorProfile)
		{
			_zoneTarget = zoneNameToCheck;
			base.PreferredLevelOfDetail = EffectDetailLevel.High;
			_xPosition = xPosition;
			_yPosition = yPosition;
		}

		public override void Present()
		{
			Vector4 processedLedColor = GetProcessedLedColor(0);
			Color color = new Color(processedLedColor);
			_colorKey.UpdateColor(color, isVisible: true);
		}

		public List<JObject> TryGetEventUpdateRequest()
		{
			_requestList.Clear();
			JObject jObject = _colorKey.TryGettingRequest();
			if (jObject != null)
			{
				_requestList.Add(jObject);
			}
			return _requestList;
		}

		public void CollectEventsToTrack(Bind_Event[] bindEvents, ARgbGameValueTracker[] miscEvents)
		{
			foreach (Bind_Event bind_Event in bindEvents)
			{
				if (bind_Event.handlers[0] is ContextColorEventHandlerType contextColorEventHandlerType && contextColorEventHandlerType.DeviceZone is RGBZonedDevice rGBZonedDevice && !(rGBZonedDevice.zone != _zoneTarget))
				{
					ColorKey colorKey = new ColorKey
					{
						EventName = bind_Event.eventName,
						TriggerName = contextColorEventHandlerType.ContextFrameKey
					};
					_colorKey = colorKey;
					break;
				}
			}
		}
	}
}
