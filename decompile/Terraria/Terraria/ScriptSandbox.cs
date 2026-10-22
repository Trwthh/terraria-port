using System.Collections.Generic;
using Microsoft.Xna.Framework;

namespace Terraria
{
	public static class ScriptSandbox
	{
		public delegate void ScriptType(ref Color color);

		public delegate void ScriptTypeDictionaryIntVector2(ref Dictionary<int, Vector2> dictionary);

		public static void Test()
		{
		}

		public static void MethodThatTakesRef(ref Color color)
		{
		}

		public static void MethodForNPCPortraits_CloseUp(ref Dictionary<int, Vector2> dictionary)
		{
			dictionary = new Dictionary<int, Vector2>
			{
				{
					17,
					new Vector2(6f, 0f)
				},
				{
					18,
					new Vector2(-6f, 0f)
				},
				{
					19,
					new Vector2(0f, 0f)
				},
				{
					20,
					new Vector2(0f, 0f)
				},
				{
					22,
					new Vector2(0f, 0f)
				},
				{
					38,
					new Vector2(0f, -6f)
				},
				{
					54,
					new Vector2(6f, 0f)
				},
				{
					37,
					new Vector2(0f, 0f)
				},
				{
					107,
					new Vector2(0f, 0f)
				},
				{
					108,
					new Vector2(6f, 4f)
				},
				{
					124,
					new Vector2(-6f, 4f)
				},
				{
					142,
					new Vector2(3f, 6f)
				},
				{
					160,
					new Vector2(-6f, 18f)
				},
				{
					178,
					new Vector2(-6f, 12f)
				},
				{
					207,
					new Vector2(0f, 0f)
				},
				{
					208,
					new Vector2(-12f, 0f)
				},
				{
					209,
					new Vector2(0f, 6f)
				},
				{
					227,
					new Vector2(-3f, 0f)
				},
				{
					228,
					new Vector2(9f, 6f)
				},
				{
					229,
					new Vector2(0f, 0f)
				},
				{
					353,
					new Vector2(-6f, 6f)
				},
				{
					368,
					new Vector2(0f, 6f)
				},
				{
					369,
					new Vector2(-6f, -6f)
				},
				{
					441,
					new Vector2(-9f, 0f)
				},
				{
					453,
					new Vector2(-12f, 0f)
				},
				{
					550,
					new Vector2(15f, 12f)
				},
				{
					588,
					new Vector2(3f, 0f)
				},
				{
					633,
					new Vector2(-3f, 0f)
				},
				{
					663,
					new Vector2(0f, -6f)
				},
				{
					637,
					new Vector2(-15f, 8f)
				},
				{
					638,
					new Vector2(-24f, 12f)
				},
				{
					656,
					new Vector2(0f, 0f)
				},
				{
					684,
					new Vector2(-3f, 2f)
				},
				{
					670,
					new Vector2(0f, 2f)
				},
				{
					678,
					new Vector2(-3f, 2f)
				},
				{
					679,
					new Vector2(0f, 2f)
				},
				{
					680,
					new Vector2(-6f, 2f)
				},
				{
					681,
					new Vector2(0f, 2f)
				},
				{
					683,
					new Vector2(-6f, 2f)
				},
				{
					682,
					new Vector2(-3f, 2f)
				}
			};
		}

		public static void MethodForNPCPortraits_FullBody(ref Dictionary<int, Vector2> dictionary)
		{
			dictionary = new Dictionary<int, Vector2>
			{
				{
					18,
					new Vector2(-3f, 0f)
				},
				{
					20,
					new Vector2(-4f, 0f)
				},
				{
					124,
					new Vector2(-6f, 2f)
				},
				{
					178,
					new Vector2(-4f, 0f)
				},
				{
					208,
					new Vector2(-4f, 0f)
				},
				{
					227,
					new Vector2(6f, 0f)
				},
				{
					353,
					new Vector2(-4f, 0f)
				},
				{
					369,
					new Vector2(-4f, 0f)
				},
				{
					441,
					new Vector2(-10f, 0f)
				},
				{
					588,
					new Vector2(-2f, 0f)
				},
				{
					633,
					new Vector2(-6f, 0f)
				},
				{
					637,
					new Vector2(-8f, 0f)
				},
				{
					638,
					new Vector2(-8f, 0f)
				},
				{
					684,
					new Vector2(-12f, 0f)
				},
				{
					670,
					new Vector2(-6f, 0f)
				},
				{
					678,
					new Vector2(-8f, 0f)
				},
				{
					679,
					new Vector2(-6f, 0f)
				},
				{
					680,
					new Vector2(-10f, 0f)
				},
				{
					681,
					new Vector2(-6f, 0f)
				},
				{
					683,
					new Vector2(-6f, 0f)
				},
				{
					682,
					new Vector2(-6f, 0f)
				}
			};
		}
	}
}
