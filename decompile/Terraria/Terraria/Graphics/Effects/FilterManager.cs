using System;
using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;
using Terraria.IO;

namespace Terraria.Graphics.Effects
{
	public class FilterManager : EffectManager<Filter>
	{
		private const float OPACITY_RATE = 1f;

		private LinkedList<Filter> _activeFilters = new LinkedList<Filter>();

		private int _filterLimit = 16;

		private EffectPriority _priorityThreshold;

		private int _activeFilterCount;

		private bool _captureThisFrame;

		public void BindTo(Preferences preferences)
		{
			preferences.OnSave += Configuration_OnSave;
			preferences.OnLoad += Configuration_OnLoad;
		}

		private void Configuration_OnSave(Preferences preferences)
		{
			preferences.Put("FilterLimit", _filterLimit);
			preferences.Put("FilterPriorityThreshold", Enum.GetName(typeof(EffectPriority), _priorityThreshold));
		}

		private void Configuration_OnLoad(Preferences preferences)
		{
			_filterLimit = preferences.Get("FilterLimit", 16);
			if (Enum.TryParse<EffectPriority>(preferences.Get("FilterPriorityThreshold", "VeryLow"), out var result))
			{
				_priorityThreshold = result;
			}
		}

		public override void OnActivate(Filter effect, Vector2 position)
		{
			if (_activeFilters.Contains(effect))
			{
				if (effect.Active)
				{
					return;
				}
				if (effect.Priority >= _priorityThreshold)
				{
					_activeFilterCount--;
				}
				_activeFilters.Remove(effect);
			}
			else
			{
				effect.Opacity = 0f;
			}
			if (effect.Priority >= _priorityThreshold)
			{
				_activeFilterCount++;
			}
			if (_activeFilters.Count == 0)
			{
				_activeFilters.AddLast(effect);
				return;
			}
			for (LinkedListNode<Filter> linkedListNode = _activeFilters.First; linkedListNode != null; linkedListNode = linkedListNode.Next)
			{
				Filter value = linkedListNode.Value;
				if (effect.Priority <= value.Priority)
				{
					_activeFilters.AddAfter(linkedListNode, effect);
					return;
				}
			}
			_activeFilters.AddLast(effect);
		}

		public void BeginCapture(RenderTarget2D screenTarget1)
		{
			_captureThisFrame = true;
			Main.instance.GraphicsDevice.SetRenderTarget(screenTarget1);
			Main.instance.GraphicsDevice.Clear(Color.Transparent);
		}

		public void Update(GameTime gameTime)
		{
			LinkedListNode<Filter> linkedListNode = _activeFilters.First;
			_ = _activeFilters.Count;
			int num = 0;
			while (linkedListNode != null)
			{
				Filter value = linkedListNode.Value;
				LinkedListNode<Filter> next = linkedListNode.Next;
				bool flag = false;
				if (value.Priority >= _priorityThreshold)
				{
					num++;
					if (num > _activeFilterCount - _filterLimit)
					{
						value.Update(gameTime);
						flag = true;
					}
				}
				if (value.Active && flag)
				{
					value.Opacity = Math.Min(value.Opacity + (float)gameTime.ElapsedGameTime.TotalSeconds * 1f, 1f);
				}
				else
				{
					value.Opacity = Math.Max(value.Opacity - (float)gameTime.ElapsedGameTime.TotalSeconds * 1f, 0f);
				}
				if (!value.Active && value.Opacity == 0f)
				{
					if (value.Priority >= _priorityThreshold)
					{
						_activeFilterCount--;
					}
					_activeFilters.Remove(linkedListNode);
				}
				linkedListNode = next;
			}
		}

		public void EndCapture(RenderTarget2D finalTexture, RenderTarget2D screenTarget1, RenderTarget2D screenTarget2)
		{
			EndCapture(finalTexture, screenTarget1, screenTarget2, screenTarget1.Size(), screenTarget1.Size(), Vector2.Zero);
		}

		public void EndCapture(RenderTarget2D finalTexture, RenderTarget2D screenTarget1, RenderTarget2D screenTarget2, Vector2 screenSize, Vector2 sceneSize, Vector2 sceneOffset)
		{
			if (!_captureThisFrame)
			{
				return;
			}
			_captureThisFrame = false;
			TimeLogger.StartTimestamp fromTimestamp = TimeLogger.Start();
			Rectangle value = new Rectangle(0, 0, (int)screenSize.X, (int)screenSize.Y);
			RenderTarget2D t = screenTarget1;
			RenderTarget2D t2 = screenTarget2;
			GraphicsDevice graphicsDevice = Main.instance.GraphicsDevice;
			graphicsDevice.SetRenderTarget(t2);
			graphicsDevice.Clear(Color.Transparent);
			Main.spriteBatch.Begin(SpriteSortMode.Immediate, BlendState.AlphaBlend);
			SpriteEffects effects = Main.GameViewMatrix.Effects;
			Main.spriteBatch.Draw(Main.skyTarget, Vector2.Zero, value, Color.White, 0f, Vector2.Zero, 1f, effects, 0f);
			Main.spriteBatch.Draw(t, Vector2.Zero, value, Color.White, 0f, Vector2.Zero, 1f, effects, 0f);
			Main.spriteBatch.End();
			Utils.Swap(ref t2, ref t);
			int num = 0;
			LinkedListNode<Filter> linkedListNode = _activeFilters.First;
			Filter filter = null;
			while (linkedListNode != null)
			{
				Filter value2 = linkedListNode.Value;
				LinkedListNode<Filter> next = linkedListNode.Next;
				if (value2.Priority >= _priorityThreshold)
				{
					num++;
					if (num > _activeFilterCount - _filterLimit && value2.IsVisible())
					{
						if (filter != null)
						{
							graphicsDevice.SetRenderTarget(t2);
							graphicsDevice.Clear(Color.Transparent);
							Main.spriteBatch.Begin(SpriteSortMode.Immediate, BlendState.AlphaBlend);
							filter.Apply(t.Size(), sceneSize, sceneOffset);
							Main.spriteBatch.Draw(t, Vector2.Zero, value, Main.ColorOfTheSkies);
							Main.spriteBatch.End();
							Utils.Swap(ref t2, ref t);
						}
						filter = value2;
					}
				}
				linkedListNode = next;
			}
			graphicsDevice.SetRenderTarget(finalTexture);
			graphicsDevice.Clear(Color.Transparent);
			if (Main.player[Main.myPlayer].gravDir == -1f)
			{
				Main.spriteBatch.Begin(SpriteSortMode.Immediate, BlendState.AlphaBlend, SamplerState.LinearClamp, DepthStencilState.Default, RasterizerState.CullNone, null, Main.GameViewMatrix.EffectMatrix);
			}
			else
			{
				Main.spriteBatch.Begin(SpriteSortMode.Immediate, BlendState.AlphaBlend);
			}
			if (filter != null)
			{
				filter.Apply(t.Size(), sceneSize, sceneOffset);
				Main.spriteBatch.Draw(t, Vector2.Zero, value, Main.ColorOfTheSkies);
			}
			else
			{
				Main.spriteBatch.Draw(t, Vector2.Zero, value, Color.White);
			}
			Main.spriteBatch.End();
			for (int i = 0; i < 8; i++)
			{
				graphicsDevice.Textures[i] = null;
			}
			TimeLogger.Filters.AddTime(fromTimestamp);
		}

		public bool HasActiveFilter()
		{
			return _activeFilters.Count != 0;
		}

		public bool CanCapture()
		{
			return HasActiveFilter();
		}
	}
}
