using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using rail;

namespace Terraria.Social.WeGame
{
	public class WeGameP2PWriter
	{
		public class WriteInformation
		{
			public byte[] Data;

			public int Size;

			public WriteInformation(byte[] data)
			{
				Data = data;
				Size = 0;
			}
		}

		private static readonly int BUFFER_SIZE = 1024;

		private RailID _local_id;

		private Dictionary<RailID, List<WriteInformation>> _pendingSendData = new Dictionary<RailID, List<WriteInformation>>();

		private Dictionary<RailID, List<WriteInformation>> _pendingSendDataSwap = new Dictionary<RailID, List<WriteInformation>>();

		private ConcurrentBag<byte[]> _bufferPool = new ConcurrentBag<byte[]>();

		private object _lock = new object();

		public void QueueSend(RailID user, byte[] data, int length)
		{
			lock (_lock)
			{
				if (!_pendingSendData.TryGetValue(user, out var value))
				{
					value = (_pendingSendData[user] = new List<WriteInformation>());
				}
				int num = length;
				int num2 = 0;
				while (num > 0)
				{
					WriteInformation writeInformation = value.LastOrDefault();
					if (writeInformation == null || writeInformation.Size == writeInformation.Data.Length)
					{
						if (!_bufferPool.TryTake(out var result))
						{
							result = new byte[BUFFER_SIZE];
						}
						writeInformation = new WriteInformation(result);
						value.Add(writeInformation);
					}
					int num3 = Math.Min(num, writeInformation.Data.Length - writeInformation.Size);
					Array.Copy(data, num2, writeInformation.Data, writeInformation.Size, num3);
					writeInformation.Size += num3;
					num -= num3;
					num2 += num3;
				}
			}
		}

		public void SetLocalPeer(RailID rail_id)
		{
			if (_local_id == null)
			{
				_local_id = new RailID();
			}
			_local_id.id_ = rail_id.id_;
		}

		private RailID GetLocalPeer()
		{
			return _local_id;
		}

		private bool IsValid()
		{
			if (_local_id != null)
			{
				return _local_id.IsValid();
			}
			return false;
		}

		public void SendAll()
		{
			if (!IsValid())
			{
				return;
			}
			lock (_lock)
			{
				Utils.Swap(ref _pendingSendData, ref _pendingSendDataSwap);
			}
			foreach (KeyValuePair<RailID, List<WriteInformation>> item in _pendingSendDataSwap)
			{
				foreach (WriteInformation item2 in item.Value)
				{
					_ = rail_api.RailFactory().RailNetworkHelper().SendData(GetLocalPeer(), item.Key, item2.Data, (uint)item2.Size) == RailResult.kSuccess;
					_bufferPool.Add(item2.Data);
				}
				item.Value.Clear();
			}
		}
	}
}
