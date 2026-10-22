using Microsoft.Xna.Framework;

namespace ReLogic.Peripherals.RGB.Corsair
{
	internal class CorsairMouse : CorsairGenericDevice
	{
		private CorsairMouse(Fragment fragment, CorsairLedPosition[] leds, DeviceColorProfile colorProfile)
			: base(RgbDeviceType.Mouse, fragment, leds, colorProfile)
		{
			base.PreferredLevelOfDetail = EffectDetailLevel.Low;
		}

		public static CorsairMouse Create(int deviceIndex, CorsairDeviceInfo deviceInfo, DeviceColorProfile colorProfile)
		{
			CorsairLedPosition[] array;
			switch (deviceInfo.PhysicalLayout)
			{
			case CorsairPhysicalLayout.CPL_Zones1:
				array = new CorsairLedPosition[1]
				{
					new CorsairLedPosition
					{
						LedId = CorsairLedId.CLM_1
					}
				};
				break;
			case CorsairPhysicalLayout.CPL_Zones2:
				array = new CorsairLedPosition[2]
				{
					new CorsairLedPosition
					{
						LedId = CorsairLedId.CLM_1
					},
					new CorsairLedPosition
					{
						LedId = CorsairLedId.CLM_2
					}
				};
				break;
			case CorsairPhysicalLayout.CPL_Zones3:
				array = new CorsairLedPosition[3]
				{
					new CorsairLedPosition
					{
						LedId = CorsairLedId.CLM_1
					},
					new CorsairLedPosition
					{
						LedId = CorsairLedId.CLM_2
					},
					new CorsairLedPosition
					{
						LedId = CorsairLedId.CLM_3
					}
				};
				break;
			case CorsairPhysicalLayout.CPL_Zones4:
				array = new CorsairLedPosition[4]
				{
					new CorsairLedPosition
					{
						LedId = CorsairLedId.CLM_1
					},
					new CorsairLedPosition
					{
						LedId = CorsairLedId.CLM_2
					},
					new CorsairLedPosition
					{
						LedId = CorsairLedId.CLM_3
					},
					new CorsairLedPosition
					{
						LedId = CorsairLedId.CLM_4
					}
				};
				break;
			default:
				array = new CorsairLedPosition[0];
				break;
			}
			return new CorsairMouse(Fragment.FromGrid(new Rectangle(27, 0, 1, array.Length)), array, colorProfile);
		}
	}
}
