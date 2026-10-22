using System;
using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;
using Terraria.GameInput;
using Terraria.UI;

namespace Terraria.GameContent.UI.Elements
{
	public abstract class UIDynamicItemCollection : UIElement
	{
		public const string SnapPointName_ItemSlot = "DynamicItemCollectionSlot";
	}
	public abstract class UIDynamicItemCollection<TEntry> : UIDynamicItemCollection
	{
		private List<TEntry> _contents = new List<TEntry>();

		private int _itemsPerLine;

		private const int sizePerEntryX = 44;

		private const int sizePerEntryY = 44;

		private List<SnapPoint> _dummySnapPoints = new List<SnapPoint>();

		public int Count => _contents.Count;

		public UIDynamicItemCollection()
		{
			Width = new StyleDimension(0f, 1f);
			HAlign = 0.5f;
			UpdateSize();
		}

		protected abstract Item GetItem(TEntry entry);

		protected override void DrawSelf(SpriteBatch spriteBatch)
		{
			Main.inventoryScale = 0.84615386f;
			GetGridParameters(out var startX, out var startY, out var startItemIndex, out var endItemIndex);
			int num = _itemsPerLine;
			Point value = UserInterface.ActiveInstance.MousePosition.ToPoint();
			for (int i = startItemIndex; i < endItemIndex; i++)
			{
				TEntry entry = _contents[i];
				Rectangle itemSlotHitbox = GetItemSlotHitbox(startX, startY, startItemIndex, i);
				if (TextureAssets.Item[GetItem(entry).type].State == AssetState.NotLoaded)
				{
					num--;
				}
				bool hovering = base.IsMouseHovering && itemSlotHitbox.Contains(value) && !PlayerInput.IgnoreMouseInterface;
				DrawSlot(spriteBatch, entry, itemSlotHitbox.TopLeft(), hovering);
				if (num <= 0)
				{
					break;
				}
			}
			for (int j = 0; j < _contents.Count; j++)
			{
				if (num <= 0)
				{
					break;
				}
				Item item = GetItem(_contents[(j + endItemIndex) % _contents.Count]);
				if (TextureAssets.Item[item.type].State == AssetState.NotLoaded)
				{
					Main.instance.LoadItem(item.type);
					num -= 4;
				}
			}
		}

		protected abstract void DrawSlot(SpriteBatch spriteBatch, TEntry entry, Vector2 pos, bool hovering);

		private Rectangle GetItemSlotHitbox(int startX, int startY, int startItemIndex, int i)
		{
			int num = i - startItemIndex;
			int num2 = num % _itemsPerLine;
			int num3 = num / _itemsPerLine;
			return new Rectangle(startX + num2 * 44, startY + num3 * 44, 44, 44);
		}

		private void GetGridParameters(out int startX, out int startY, out int startItemIndex, out int endItemIndex)
		{
			Rectangle rectangle = GetDimensions().ToRectangle();
			Rectangle viewCullingArea = base.Parent.GetViewCullingArea();
			int x = rectangle.Center.X;
			startX = x - (int)((float)(44 * _itemsPerLine) * 0.5f);
			startY = rectangle.Top;
			startItemIndex = 0;
			endItemIndex = _contents.Count;
			int num = (Math.Min(viewCullingArea.Top, rectangle.Top) - viewCullingArea.Top) / 44;
			startY += -num * 44;
			startItemIndex += -num * _itemsPerLine;
			int num2 = (int)Math.Ceiling((float)viewCullingArea.Height / 44f) * _itemsPerLine;
			if (endItemIndex > num2 + startItemIndex + _itemsPerLine)
			{
				endItemIndex = num2 + startItemIndex + _itemsPerLine;
			}
		}

		public override void Recalculate()
		{
			base.Recalculate();
			UpdateSize();
		}

		public override void Update(GameTime gameTime)
		{
			base.Update(gameTime);
			if (base.IsMouseHovering)
			{
				Main.LocalPlayer.mouseInterface = true;
			}
		}

		public void SetContentsToShow(List<TEntry> itemsToShow)
		{
			_contents.Clear();
			_contents.AddRange(itemsToShow);
			UpdateSize();
		}

		public int GetItemsPerLine()
		{
			return _itemsPerLine;
		}

		public override List<SnapPoint> GetSnapPoints()
		{
			List<SnapPoint> list = new List<SnapPoint>();
			GetGridParameters(out var startX, out var startY, out var startItemIndex, out var endItemIndex);
			Rectangle viewCullingArea = base.Parent.GetViewCullingArea();
			int num = endItemIndex - startItemIndex;
			while (_dummySnapPoints.Count < num)
			{
				_dummySnapPoints.Add(new SnapPoint("DynamicItemCollectionSlot", 0, Vector2.Zero, Vector2.Zero));
			}
			int num2 = 0;
			Vector2 vector = GetDimensions().Position();
			Vector2 vector2 = TextureAssets.InventoryBack.Size() * Main.inventoryScale;
			viewCullingArea.Y += (int)(vector2.Y * 0.25f);
			viewCullingArea.Height -= (int)(vector2.Y * 0.25f);
			for (int i = startItemIndex; i < endItemIndex; i++)
			{
				Vector2 vector3 = GetItemSlotHitbox(startX, startY, startItemIndex, i).TopLeft() + vector2 * 0.75f;
				if (viewCullingArea.Contains(vector3.ToPoint()))
				{
					SnapPoint snapPoint = _dummySnapPoints[num2];
					snapPoint.ThisIsAHackThatChangesTheSnapPointsInfo(Vector2.Zero, vector3 - vector, i);
					snapPoint.Calculate(this);
					num2++;
					list.Add(snapPoint);
				}
			}
			foreach (UIElement element in Elements)
			{
				list.AddRange(element.GetSnapPoints());
			}
			return list;
		}

		public void UpdateSize()
		{
			int num = (_itemsPerLine = GetDimensions().ToRectangle().Width / 44);
			int num2 = (int)Math.Ceiling((float)_contents.Count / (float)num);
			MinHeight.Set(44 * num2, 0f);
		}
	}
}
