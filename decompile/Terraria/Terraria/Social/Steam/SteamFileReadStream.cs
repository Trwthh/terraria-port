using System;
using System.IO;
using System.Threading;
using Steamworks;

namespace Terraria.Social.Steam
{
	public class SteamFileReadStream : Stream
	{
		private readonly string _path;

		private int? _length;

		private uint _position;

		private CallResult<RemoteStorageFileReadAsyncComplete_t> _callback = new CallResult<RemoteStorageFileReadAsyncComplete_t>();

		public override bool CanRead => true;

		public override bool CanSeek => true;

		public override bool CanWrite => false;

		public override long Length
		{
			get
			{
				if (!_length.HasValue)
				{
					_length = SteamRemoteStorage.GetFileSize(_path);
				}
				return _length.Value;
			}
		}

		public override long Position
		{
			get
			{
				return _position;
			}
			set
			{
				Seek(value, SeekOrigin.Begin);
			}
		}

		public SteamFileReadStream(string path)
		{
			_path = path;
		}

		public override int Read(byte[] buffer, int offset, int count)
		{
			if (buffer == null)
			{
				throw new ArgumentNullException("buffer");
			}
			if (offset != 0)
			{
				throw new NotSupportedException("offset != 0");
			}
			if (count < 0 || count > buffer.Length)
			{
				throw new ArgumentOutOfRangeException("count");
			}
			if (count == 0)
			{
				return 0;
			}
			if (_position >= Length)
			{
				return 0;
			}
			SteamAPICall_t steamAPICall_t = SteamRemoteStorage.FileReadAsync(_path, _position, (uint)count);
			if (steamAPICall_t == SteamAPICall_t.Invalid)
			{
				throw new IOException("FileReadAsync call invalid");
			}
			Tuple<int, Exception> ret = null;
			_callback.Set(steamAPICall_t, delegate(RemoteStorageFileReadAsyncComplete_t result, bool bIOFailure)
			{
				try
				{
					if (bIOFailure)
					{
						throw new IOException("Steam IO Failure");
					}
					if (result.m_eResult != EResult.k_EResultOK)
					{
						throw new IOException("Steam IO Failure: " + result.m_eResult);
					}
					if (!SteamRemoteStorage.FileReadAsyncComplete(result.m_hFileReadAsync, buffer, result.m_cubRead))
					{
						throw new IOException("FileReadAsyncComplete failed");
					}
					ret = new Tuple<int, Exception>((int)result.m_cubRead, null);
				}
				catch (Exception item)
				{
					ret = new Tuple<int, Exception>(0, item);
				}
			});
			while (ret == null)
			{
				CoreSocialModule.Pulse();
				Thread.Sleep(0);
			}
			if (ret.Item2 != null)
			{
				throw new IOException("Exception in Read", ret.Item2);
			}
			_position += (uint)ret.Item1;
			return ret.Item1;
		}

		public override long Seek(long offset, SeekOrigin origin)
		{
			long num;
			switch (origin)
			{
			case SeekOrigin.Begin:
				num = offset;
				break;
			case SeekOrigin.Current:
				num = _position + offset;
				break;
			default:
				num = Length - offset;
				break;
			}
			if (num < 0 || num > uint.MaxValue)
			{
				throw new ArgumentOutOfRangeException("offset");
			}
			_position = (uint)num;
			return _position;
		}

		public override void SetLength(long value)
		{
			throw new NotSupportedException();
		}

		public override void Write(byte[] buffer, int offset, int count)
		{
			throw new NotSupportedException();
		}

		public override void Flush()
		{
			throw new NotSupportedException();
		}

		protected override void Dispose(bool disposing)
		{
			if (disposing)
			{
				_callback.Dispose();
			}
		}
	}
}
