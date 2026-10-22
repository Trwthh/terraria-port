using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.UI;

namespace Terraria.Graphics.Renderers
{
	public class ItemTransferParticle : IPooledParticle, IParticle
	{
		private Vector2 StartPosition;

		private Vector2 EndPosition;

		private Vector2 StartOffset;

		private Vector2 EndOffset;

		private Vector2 BezierHelper1;

		private Vector2 BezierHelper2;

		private bool TransitionIn;

		private bool Fullbright;

		private bool InInventory;

		private Item _itemInstance;

		private int _lifeTimeCounted;

		private int _lifeTimeTotal;

		public bool ShouldBeRemovedFromRenderer { get; private set; }

		public bool IsRestingInPool { get; private set; }

		public ItemTransferParticle()
		{
			_itemInstance = new Item();
		}

		public void Update(ref ParticleRendererSettings settings)
		{
			if (++_lifeTimeCounted >= _lifeTimeTotal)
			{
				ShouldBeRemovedFromRenderer = true;
			}
		}

		public void Prepare(int itemType, int lifeTimeTotal, Vector2 startPosition, Vector2 endPosition, Vector2 offsetStart, Vector2 offsetEnd, bool transitionIn, bool fullbright, bool inInventory, int stack = 1)
		{
			_itemInstance.SetDefaults(itemType);
			_itemInstance.stack = stack;
			_lifeTimeTotal = lifeTimeTotal;
			StartPosition = startPosition;
			StartOffset = offsetStart;
			EndPosition = endPosition;
			EndOffset = offsetEnd;
			TransitionIn = transitionIn;
			Fullbright = fullbright;
			InInventory = inInventory;
			Vector2 vector = (EndPosition - StartPosition).SafeNormalize(Vector2.UnitY).RotatedBy(1.5707963705062866);
			bool num = vector.Y < 0f;
			bool flag = vector.Y == 0f;
			if (!num || (flag && Main.rand.Next(2) == 0))
			{
				vector *= -1f;
			}
			vector = new Vector2(0f, -1f);
			float num2 = Vector2.Distance(EndPosition, StartPosition);
			BezierHelper1 = vector * num2 + Main.rand.NextVector2Circular(32f, 32f);
			BezierHelper2 = -vector * num2 + Main.rand.NextVector2Circular(32f, 32f);
		}

		public void Draw(ref ParticleRendererSettings settings, SpriteBatch spritebatch)
		{
			float num = (float)_lifeTimeCounted / (float)_lifeTimeTotal;
			float toMin = Utils.Remap(num, 0.1f, 0.5f, 0f, 0.85f);
			toMin = Utils.Remap(num, 0.5f, 0.9f, toMin, 1f);
			Vector2.Hermite(ref StartPosition, ref BezierHelper1, ref EndPosition, ref BezierHelper2, toMin, out var result);
			Vector2 zero = Vector2.Zero;
			zero = ((num <= 0.15f) ? Vector2.Lerp(Vector2.Zero, StartOffset, num / 0.15f) : ((num <= 0.5f) ? Vector2.Lerp(StartOffset, EndOffset, (num - 0.15f) / 0.35f) : ((!(num <= 0.85f)) ? Vector2.Lerp(EndOffset, Vector2.Zero, Utils.Remap(num, 0.85f, 0.95f, 0f, 1f)) : EndOffset)));
			result += zero;
			float num2 = Utils.Remap(num, 0f, 0.15f, (!TransitionIn) ? 1 : 0, 1f) * Utils.Remap(num, 0.85f, 0.95f, 1f, 0f);
			Color color = (Fullbright ? Color.White : Lighting.GetColor(result.ToTileCoordinates()));
			int context = 31;
			int num3 = 32;
			if (InInventory)
			{
				num3 = 32;
				num2 = 1f;
				float num4 = num;
				num4 *= num4;
				result = Vector2.Lerp(StartPosition - new Vector2(26f, 26f) * Main.inventoryScale, EndPosition - new Vector2(26f, 26f) * Main.inventoryScale, num4);
				context = 14;
			}
			if (InInventory)
			{
				ItemSlot.Draw(spritebatch, ref _itemInstance, context, settings.AnchorPosition + result, color);
			}
			else
			{
				ItemSlot.DrawItemIcon(_itemInstance, context, Main.spriteBatch, settings.AnchorPosition + result, _itemInstance.scale * num2, num3, color);
			}
		}

		public void RestInPool()
		{
			IsRestingInPool = true;
		}

		public virtual void FetchFromPool()
		{
			_lifeTimeCounted = 0;
			_lifeTimeTotal = 0;
			IsRestingInPool = false;
			ShouldBeRemovedFromRenderer = false;
			StartPosition = (EndPosition = (BezierHelper1 = (BezierHelper2 = Vector2.Zero)));
		}
	}
}
