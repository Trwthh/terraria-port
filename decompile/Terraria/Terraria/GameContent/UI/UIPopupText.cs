using Microsoft.Xna.Framework;

namespace Terraria.GameContent.UI
{
	public class UIPopupText
	{
		public Vector2 position;

		public Vector2 velocity;

		public float alpha;

		public int alphaDir = 1;

		public string name;

		public string displayText;

		public float scale = 1f;

		public float rotation;

		public Color color;

		public bool active;

		public int lifeTime;

		public int framesSinceSpawn;

		public static int activeTime = 60;

		public UIPopupTextContext context;

		public float TargetScale => 1f;

		public void PrepareDisplayText()
		{
			displayText = name;
		}

		public void Update(int whoAmI, UIPopupTextManager manager)
		{
			if (!active)
			{
				return;
			}
			framesSinceSpawn++;
			float targetScale = TargetScale;
			alpha += (float)alphaDir * 0.01f;
			if ((double)alpha <= 0.7)
			{
				alpha = 0.7f;
				alphaDir = 1;
			}
			if (alpha >= 1f)
			{
				alpha = 1f;
				alphaDir = -1;
			}
			bool flag = false;
			Vector2 textHitbox = GetTextHitbox();
			Rectangle rectangle = new Rectangle((int)(position.X - textHitbox.X / 2f), (int)(position.Y - textHitbox.Y / 2f), (int)textHitbox.X, (int)textHitbox.Y);
			for (int i = 0; i < 20; i++)
			{
				UIPopupText uIPopupText = manager.popupText[i];
				if (!uIPopupText.active || i == whoAmI)
				{
					continue;
				}
				Vector2 textHitbox2 = uIPopupText.GetTextHitbox();
				Rectangle value = new Rectangle((int)(uIPopupText.position.X - textHitbox2.X / 2f), (int)(uIPopupText.position.Y - textHitbox2.Y / 2f), (int)textHitbox2.X, (int)textHitbox2.Y);
				if (rectangle.Intersects(value) && (position.Y < uIPopupText.position.Y || (position.Y == uIPopupText.position.Y && whoAmI < i)))
				{
					flag = true;
					int num = manager.numActive;
					if (num > 3)
					{
						num = 3;
					}
					uIPopupText.lifeTime = activeTime + 15 * num;
					lifeTime = activeTime + 15 * num;
				}
			}
			if (!flag)
			{
				if (context != UIPopupTextContext.SpecialSeed || (scale != targetScale && lifeTime > 0))
				{
					velocity.Y *= 0.86f;
					if (scale == targetScale)
					{
						velocity.Y *= 0.4f;
					}
				}
			}
			else if (velocity.Y > -6f)
			{
				velocity.Y -= 0.2f;
			}
			else
			{
				velocity.Y *= 0.86f;
			}
			velocity.X *= 0.93f;
			position += velocity;
			lifeTime--;
			if (lifeTime <= 0)
			{
				scale -= 0.03f * targetScale;
				if ((double)scale < 0.1 * (double)targetScale)
				{
					active = false;
				}
				lifeTime = 0;
				return;
			}
			if (scale < targetScale)
			{
				scale += 0.1f * targetScale;
			}
			if (scale > targetScale)
			{
				scale = targetScale;
			}
		}

		public Vector2 GetTextHitbox()
		{
			string text = displayText;
			Vector2 result = FontAssets.MouseText.Value.MeasureString(text);
			result *= scale;
			result.Y *= 0.8f;
			return result;
		}
	}
}
