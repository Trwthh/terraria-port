using System;
using System.Collections.Generic;
using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Graphics;

namespace Terraria.Testing
{
	public class DebugVisualizer
	{
		public enum UpdatePhase
		{
			Update,
			UpdateInWorld,
			Draw
		}

		private class DrawItem
		{
			public readonly UpdatePhase Phase = CurrentPhase;

			public readonly Action<SpriteBatch> Draw;

			public int TimeLeft;

			public DrawItem(Action<SpriteBatch> draw, int lifetime)
			{
				Draw = draw;
				TimeLeft = lifetime;
			}
		}

		private class PhaseOverrideHandle : IDisposable
		{
			private readonly UpdatePhase _prev;

			public PhaseOverrideHandle(UpdatePhase phase)
			{
				UpdatePhase currentPhase = CurrentPhase;
				CurrentPhase = phase;
				_prev = currentPhase;
			}

			public void Dispose()
			{
				CurrentPhase = _prev;
			}
		}

		public static readonly DebugVisualizer UI = new DebugVisualizer(ui: true);

		public static readonly DebugVisualizer World = new DebugVisualizer(ui: false);

		private static UpdatePhase CurrentPhase;

		private readonly List<DrawItem> items = new List<DrawItem>();

		private readonly bool _ui;

		private DebugVisualizer(bool ui)
		{
			_ui = ui;
		}

		public void Add(Action<SpriteBatch> draw = null, int lifetime = 1)
		{
			items.Add(new DrawItem(draw, lifetime));
		}

		public void AddLine(Vector2 start, Vector2 end, Color colorStart, Color colorEnd = default(Color), int lifetime = 1, float width = 1f)
		{
			if (colorEnd == default(Color))
			{
				colorEnd = colorStart;
			}
			Add(delegate(SpriteBatch sb)
			{
				Utils.DrawLine(sb, start, end, colorStart, colorEnd, width);
			}, lifetime);
		}

		public void AddLine(Point start, Point end, Color colorStart, Color colorEnd = default(Color), int lifetime = 1, float width = 1f)
		{
			AddLine(start.ToVector2(), end.ToVector2(), colorStart, colorEnd, lifetime, width);
		}

		public void AddRectangle(Rectangle rect, Color color, int lifetime = 1, float width = 1f)
		{
			AddRectangle(rect.TopLeft(), rect.BottomRight(), color, color, lifetime, width);
		}

		public void AddRectangle(Vector2 start, Vector2 end, Color colorStart, Color colorEnd = default(Color), int lifetime = 1, float width = 1f)
		{
			AddLine(start, new Vector2(start.X, end.Y), colorStart, colorEnd, lifetime, width);
			AddLine(start, new Vector2(end.X, start.Y), colorStart, colorEnd, lifetime, width);
			AddLine(end, new Vector2(start.X, end.Y), colorStart, colorEnd, lifetime, width);
			AddLine(end, new Vector2(end.X, start.Y), colorStart, colorEnd, lifetime, width);
		}

		public void AddFilledRectangle(Rectangle rect, Color color, int lifetime = 1)
		{
			Vector2 vector = rect.TopLeft();
			Vector2 vector2 = rect.BottomRight();
			AddLine(new Vector2(vector.X, (vector.Y + vector2.Y) / 2f), new Vector2(vector2.X, (vector.Y + vector2.Y) / 2f), color, color, lifetime, rect.Height);
		}

		public static void PreUpdate()
		{
			SetPhase(UpdatePhase.Update);
		}

		public static void PreWorldUpdate()
		{
			SetPhase(UpdatePhase.UpdateInWorld);
		}

		public static void PreDraw()
		{
			SetPhase(UpdatePhase.Draw);
		}

		private static void SetPhase(UpdatePhase phase)
		{
			CurrentPhase = phase;
			UI.Tick();
			World.Tick();
		}

		public void Tick()
		{
			int num = 0;
			for (int i = 0; i < items.Count; i++)
			{
				DrawItem drawItem = items[i];
				if (drawItem.Phase == CurrentPhase)
				{
					drawItem.TimeLeft--;
				}
				if (drawItem.TimeLeft > 0)
				{
					items[num++] = drawItem;
				}
			}
			items.RemoveRange(num, items.Count - num);
		}

		public void Draw(SpriteBatch spriteBatch)
		{
			if (items.Count == 0)
			{
				return;
			}
			spriteBatch.Begin(SpriteSortMode.Deferred, BlendState.AlphaBlend, Main.DefaultSamplerState, null, null, null, _ui ? (Matrix.CreateTranslation(Main.screenPosition.X, Main.screenPosition.Y, 0f) * Main.UIScaleMatrix) : Main.GameViewMatrix.TransformationMatrix);
			foreach (DrawItem item in items)
			{
				item.Draw(spriteBatch);
			}
			spriteBatch.End();
		}

		public static IDisposable InPhase(UpdatePhase phase)
		{
			return new PhaseOverrideHandle(phase);
		}
	}
}
