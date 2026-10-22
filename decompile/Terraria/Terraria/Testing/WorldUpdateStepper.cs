using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Input;
using Terraria.GameInput;

namespace Terraria.Testing
{
	public static class WorldUpdateStepper
	{
		public static bool DrawnThisUpdate;

		private static uint StartUpdateCount;

		private static int StepHeldCount;

		public static bool Paused { get; private set; }

		public static bool ShouldUpdateWorld()
		{
			GetStepKeyState(out var down, out var wasDown);
			if (!down)
			{
				StepHeldCount = 0;
			}
			if (!Paused)
			{
				return true;
			}
			if (down && ++StepHeldCount >= 12 && StepHeldCount % 2 == 0)
			{
				return true;
			}
			if (wasDown && !down)
			{
				return true;
			}
			return false;
		}

		private static void GetStepKeyState(out bool down, out bool wasDown)
		{
			down = Main.keyState.IsKeyDown(Keys.End) || Main.keyState.IsKeyDown(Keys.Add) || PlayerInput.MouseInfo.XButton1 == ButtonState.Pressed;
			wasDown = Main.oldKeyState.IsKeyDown(Keys.End) || Main.oldKeyState.IsKeyDown(Keys.Add) || PlayerInput.MouseInfoOld.XButton1 == ButtonState.Pressed;
		}

		public static void TogglePaused()
		{
			Paused = !Paused;
			StartUpdateCount = Main.GameUpdateCount;
		}

		internal static void DrawHUD()
		{
			if (Paused && RecordReplay.Mode != RecordReplay.ReplayMode.Replaying)
			{
				Vector2 pos = Main.ScreenSize.ToVector2() - new Vector2(5f, 22f);
				Utils.DrawBorderString(Main.spriteBatch, "Updates Since Paused: " + (int)(Main.GameUpdateCount - StartUpdateCount), pos, Color.White, 1f, 1f);
			}
		}
	}
}
