using System;

namespace Microsoft.Xna.Framework.Audio;

public class SoundEffectInstance : IDisposable
{
	private SoundEffect effect;

	private bool disposed;

	private bool looped;

	private float currentVolume = 1f;

	private float currentPitch;

	private float currentPan;

	private bool is3d;

	private bool isFireAndForget;

	internal bool isPacketSubmitted;

	private UnsafeNativeStructures.XACT_LISTENER_DATA[] listenerData;

	private uint voiceHandle = uint.MaxValue;

	private object voiceHandleLock = new object();

	internal uint VoiceHandle
	{
		get
		{
			return voiceHandle;
		}
		set
		{
			voiceHandle = value;
		}
	}

	internal object VoiceHandleLock => voiceHandleLock;

	internal bool IsFireAndForget => isFireAndForget;

	public bool IsDisposed => disposed;

	internal SoundEffect SoundEffect
	{
		get
		{
			return effect;
		}
		set
		{
			effect = value;
		}
	}

	public float Volume
	{
		get
		{
			return currentVolume;
		}
		set
		{
			lock (voiceHandleLock)
			{
				if (IsDisposed)
				{
					throw new ObjectDisposedException(GetType().Name, FrameworkResources.ObjectDisposedException);
				}
				if (value >= 0f && value <= 1f)
				{
					Helpers.ThrowExceptionFromErrorCode(SoundEffectUnsafeNativeMethods.SetVolume(voiceHandle, value));
					currentVolume = value;
					return;
				}
				throw new ArgumentOutOfRangeException("value");
			}
		}
	}

	public float Pitch
	{
		get
		{
			return currentPitch;
		}
		set
		{
			lock (voiceHandleLock)
			{
				if (IsDisposed)
				{
					throw new ObjectDisposedException(GetType().Name, FrameworkResources.ObjectDisposedException);
				}
				if (value >= -1f && value <= 1f)
				{
					Helpers.ThrowExceptionFromErrorCode(SoundEffectUnsafeNativeMethods.SetPitch(voiceHandle, value));
					currentPitch = value;
					return;
				}
				throw new ArgumentOutOfRangeException("value");
			}
		}
	}

	public float Pan
	{
		get
		{
			return currentPan;
		}
		set
		{
			lock (voiceHandleLock)
			{
				if (IsDisposed)
				{
					throw new ObjectDisposedException(GetType().Name, FrameworkResources.ObjectDisposedException);
				}
				if (!isPacketSubmitted)
				{
					is3d = false;
				}
				if (is3d)
				{
					throw new InvalidOperationException(FrameworkResources.InvalidCall);
				}
				if (value >= -1f && value <= 1f)
				{
					Helpers.ThrowExceptionFromErrorCode(SoundEffectUnsafeNativeMethods.SetPan(voiceHandle, value));
					currentPan = value;
					return;
				}
				throw new ArgumentOutOfRangeException("value");
			}
		}
	}

	public virtual bool IsLooped
	{
		get
		{
			return looped;
		}
		set
		{
			if (IsDisposed)
			{
				throw new ObjectDisposedException(GetType().Name, FrameworkResources.ObjectDisposedException);
			}
			if (isPacketSubmitted)
			{
				throw new InvalidOperationException(FrameworkResources.InvalidCall);
			}
			looped = value;
		}
	}

	public SoundState State
	{
		get
		{
			lock (voiceHandleLock)
			{
				if (IsDisposed)
				{
					throw new ObjectDisposedException(GetType().Name, FrameworkResources.ObjectDisposedException);
				}
				VoiceState state = VoiceState.Stopped;
				Helpers.ThrowExceptionFromErrorCode(SoundEffectUnsafeNativeMethods.GetState(voiceHandle, ref state));
				SoundState result = SoundState.Stopped;
				if ((state & VoiceState.Paused) != 0)
				{
					result = SoundState.Paused;
				}
				if (state == VoiceState.Playing || state == VoiceState.Stopping)
				{
					result = SoundState.Playing;
				}
				return result;
			}
		}
	}

	internal SoundEffectInstance(SoundEffect parentEffect, bool fireAndForget)
	{
		if (parentEffect.IsDisposed)
		{
			throw new ObjectDisposedException(typeof(SoundEffect).Name, FrameworkResources.ObjectDisposedException);
		}
		effect = parentEffect;
		AllocateVoice();
		Volume = 1f;
		Pitch = 0f;
		Pan = 0f;
		looped = false;
		is3d = false;
		isFireAndForget = fireAndForget;
	}

	internal SoundEffectInstance()
	{
	}

	~SoundEffectInstance()
	{
		Dispose(disposing: false);
	}

	public void Dispose()
	{
		Dispose(disposing: true);
		GC.SuppressFinalize(this);
	}

	protected virtual void Dispose(bool disposing)
	{
		lock (voiceHandleLock)
		{
			if (!IsDisposed)
			{
				disposed = true;
				if (effect != null)
				{
					effect.ChildDestroyed(this);
				}
				DeallocateVoice();
			}
		}
	}

	internal unsafe virtual void AllocateVoice()
	{
		lock (voiceHandleLock)
		{
			if (voiceHandle != uint.MaxValue)
			{
				throw new InvalidOperationException();
			}
			uint num = default(uint);
			Helpers.ThrowExceptionFromErrorCode(SoundEffectUnsafeNativeMethods.CreateSoundEffectInstance(effect.Handle, &num));
			voiceHandle = num;
			isPacketSubmitted = false;
		}
	}

	internal void DeallocateVoice()
	{
		lock (voiceHandleLock)
		{
			if (voiceHandle != uint.MaxValue)
			{
				SoundEffectUnsafeNativeMethods.DestroySoundEffectInstance(voiceHandle);
			}
			voiceHandle = uint.MaxValue;
			isPacketSubmitted = false;
		}
	}

	public virtual void Play()
	{
		lock (voiceHandleLock)
		{
			if (IsDisposed)
			{
				throw new ObjectDisposedException(GetType().Name, FrameworkResources.ObjectDisposedException);
			}
			if (!isPacketSubmitted)
			{
				Helpers.ThrowExceptionFromErrorCode(SoundEffectUnsafeNativeMethods.SubmitPacket(voiceHandle, effect.Loop.Start, effect.Loop.Length, looped ? (-1) : 0));
				isPacketSubmitted = true;
			}
			Helpers.ThrowExceptionFromErrorCode(SoundEffectUnsafeNativeMethods.Play(voiceHandle));
		}
	}

	public void Stop()
	{
		Stop(immediate: true);
	}

	public void Stop(bool immediate)
	{
		lock (voiceHandleLock)
		{
			if (IsDisposed)
			{
				throw new ObjectDisposedException(GetType().Name, FrameworkResources.ObjectDisposedException);
			}
			Helpers.ThrowExceptionFromErrorCode(SoundEffectUnsafeNativeMethods.Stop(voiceHandle, immediate));
		}
	}

	public void Pause()
	{
		lock (voiceHandleLock)
		{
			if (IsDisposed)
			{
				throw new ObjectDisposedException(GetType().Name, FrameworkResources.ObjectDisposedException);
			}
			Helpers.ThrowExceptionFromErrorCode(SoundEffectUnsafeNativeMethods.Pause(voiceHandle));
		}
	}

	public void Resume()
	{
		lock (voiceHandleLock)
		{
			if (IsDisposed)
			{
				throw new ObjectDisposedException(GetType().Name, FrameworkResources.ObjectDisposedException);
			}
			if (!isPacketSubmitted)
			{
				Play();
			}
			else if (State == SoundState.Paused || State == SoundState.Stopped)
			{
				Helpers.ThrowExceptionFromErrorCode(SoundEffectUnsafeNativeMethods.Play(voiceHandle));
			}
		}
	}

	public void Apply3D(AudioListener listener, AudioEmitter emitter)
	{
		SafeApply3D(new AudioListener[1] { listener }, emitter);
	}

	public void Apply3D(AudioListener[] listeners, AudioEmitter emitter)
	{
		SafeApply3D(listeners, emitter);
	}

	private void SafeApply3D(AudioListener[] listeners, AudioEmitter emitter)
	{
		UnsafeApply3D(listeners, emitter);
	}

	private unsafe void UnsafeApply3D(AudioListener[] listeners, AudioEmitter emitter)
	{
		lock (voiceHandleLock)
		{
			if (IsDisposed)
			{
				throw new ObjectDisposedException(GetType().Name, FrameworkResources.ObjectDisposedException);
			}
			if (!isPacketSubmitted)
			{
				is3d = true;
			}
			if (!is3d)
			{
				throw new InvalidOperationException(FrameworkResources.InvalidApply3DCall);
			}
			if (listenerData == null || listenerData.Length < listeners.Length)
			{
				listenerData = new UnsafeNativeStructures.XACT_LISTENER_DATA[listeners.Length];
			}
			Array.Clear(listenerData, 0, listenerData.Length);
			for (int i = 0; i < listeners.Length; i++)
			{
				ref UnsafeNativeStructures.XACT_LISTENER_DATA reference = ref listenerData[i];
				reference = listeners[i].listenerData;
			}
			fixed (UnsafeNativeStructures.XACT_LISTENER_DATA* pListeners = listenerData)
			{
				fixed (UnsafeNativeStructures.XACT_EMITTER_DATA* emitterData = &emitter.emitterData)
				{
					Helpers.ThrowExceptionFromErrorCode(SoundEffectUnsafeNativeMethods.Apply3D(voiceHandle, pListeners, listeners.Length, emitterData));
				}
			}
		}
	}
}
