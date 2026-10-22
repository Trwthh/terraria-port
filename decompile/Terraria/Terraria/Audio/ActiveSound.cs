using Microsoft.Xna.Framework;
using Microsoft.Xna.Framework.Audio;
using Terraria.Testing;

namespace Terraria.Audio
{
	public class ActiveSound
	{
		public delegate bool LoopedPlayCondition();

		public readonly bool IsGlobal;

		public Vector2 Position;

		public float Volume;

		public float Pitch;

		public LoopedPlayCondition Condition;

		public SoundEffectInstance Sound { get; private set; }

		public SoundStyle Style { get; private set; }

		public bool IsPlaying
		{
			get
			{
				if (Sound != null)
				{
					return Sound.State == SoundState.Playing;
				}
				return false;
			}
		}

		private void UseOverrides(SoundPlayOverrides overrides)
		{
			if (overrides.Volume.HasValue)
			{
				Volume = overrides.Volume.Value;
			}
		}

		public ActiveSound(SoundStyle style, Vector2 position, SoundPlayOverrides overrides)
		{
			Invariant.Assert(style.IsTrackable, "Only trackable sounds may be used here.");
			Position = position;
			Volume = 1f;
			Pitch = style.PitchVariance;
			IsGlobal = false;
			Style = style;
			UseOverrides(overrides);
			Play();
		}

		public ActiveSound(SoundStyle style)
		{
			Invariant.Assert(style.IsTrackable, "Only trackable sounds may be used here.");
			Position = Vector2.Zero;
			Volume = 1f;
			Pitch = style.PitchVariance;
			IsGlobal = true;
			Style = style;
			Play();
		}

		public ActiveSound(SoundStyle style, Vector2 position, LoopedPlayCondition condition, SoundPlayOverrides overrides)
		{
			Invariant.Assert(style.IsTrackable, "Only trackable sounds may be used here.");
			Position = position;
			Volume = 1f;
			Pitch = style.PitchVariance;
			IsGlobal = false;
			Style = style;
			UseOverrides(overrides);
			PlayLooped(condition);
		}

		private void Play()
		{
			SoundEffectInstance soundEffectInstance = (Sound = Style.GetRandomSound().CreateInstance());
			soundEffectInstance.Pitch += Style.GetRandomPitch();
			Pitch = soundEffectInstance.Pitch;
			soundEffectInstance.Volume = DetermineIntendedVolume();
			soundEffectInstance.Play();
			SoundInstanceGarbageCollector.Track(soundEffectInstance);
			Update();
		}

		private void PlayLooped(LoopedPlayCondition condition)
		{
			SoundEffectInstance soundEffectInstance = (Sound = Style.GetRandomSound().CreateInstance());
			soundEffectInstance.Pitch += Style.GetRandomPitch();
			Pitch = soundEffectInstance.Pitch;
			soundEffectInstance.IsLooped = true;
			Condition = condition;
			soundEffectInstance.Play();
			SoundInstanceGarbageCollector.Track(soundEffectInstance);
			Update();
		}

		public void Stop()
		{
			if (Sound != null)
			{
				Sound.Stop();
			}
		}

		public void Pause()
		{
			if (Sound != null && Sound.State == SoundState.Playing)
			{
				Sound.Pause();
			}
		}

		public void Resume()
		{
			if (Sound != null && Sound.State == SoundState.Paused)
			{
				Sound.Resume();
			}
		}

		public void Update()
		{
			if (Sound != null)
			{
				if (Condition != null && !Condition())
				{
					Sound.Stop(immediate: true);
					return;
				}
				float volume = DetermineIntendedVolume();
				Sound.Volume = volume;
				Sound.Pitch = Pitch;
			}
		}

		private float DetermineIntendedVolume()
		{
			float num = 1f;
			if (!IsGlobal)
			{
				Vector2 vector = Position - Main.Camera.Center;
				Sound.Pan = MathHelper.Clamp(vector.X / ((float)Main.MaxWorldViewSize.X * 0.5f), -1f, 1f);
				num = MathHelper.Clamp(1f - vector.Length() / LegacySoundPlayer.SoundAttenuationDistance, 0f, 1f);
			}
			num *= Style.Volume * Volume;
			switch (Style.Type)
			{
			case SoundType.Sound:
				num *= Main.soundVolume;
				break;
			case SoundType.Ambient:
				num *= Main.ambientVolume;
				break;
			case SoundType.Music:
				num *= Main.musicVolume;
				break;
			}
			return MathHelper.Clamp(num, 0f, 1f);
		}
	}
}
