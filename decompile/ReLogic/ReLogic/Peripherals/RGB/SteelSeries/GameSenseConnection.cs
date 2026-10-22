using System;
using System.Diagnostics;
using System.IO;
using System.Net;
using System.Text;
using System.Threading;
using FullSerializer;
using Newtonsoft.Json.Linq;
using ReLogic.OS;
using SteelSeries.GameSense;

namespace ReLogic.Peripherals.RGB.SteelSeries
{
	public class GameSenseConnection
	{
		public delegate void ClientStateEvent();

		public string GameName;

		public string GameDisplayName;

		public IconColor IconColor;

		private Bind_Event[] Events;

		private const string _SceneObjName = "GameSenseManager_Auto";

		private const string _GameSenseObjName = "GameSenseManager";

		private const uint _MsgQueueSize = 100u;

		private const int _ServerProbeInterval = 5000;

		private const int _MsgCheckInterval = 10;

		private const long _MaxIdleTimeBeforeHeartbeat = 1000L;

		private Thread _gameSenseThread;

		private bool _mGameSenseWrkShouldRun;

		private Uri _uriBase;

		private LocklessQueue<QueueMsg> _mMsgQueue;

		private ClientState _mClientState;

		private string _mServerPort;

		private fsSerializer _mSerializer;

		public ClientStateEvent OnConnectionBecameActive;

		public ClientStateEvent OnConnectionBecameInactive;

		private HttpWebRequest _currentRequest;

		public void SetEvents(params Bind_Event[] bindEvents)
		{
			Events = bindEvents;
		}

		public void BeginConnection()
		{
			_mSerializer = new fsSerializer();
			_mMsgQueue = new LocklessQueue<QueueMsg>(100u);
			_gameSenseThread = new Thread(_gamesenseWrk);
			_mGameSenseWrkShouldRun = true;
			_setClientState(ClientState.Probing);
			try
			{
				_gameSenseThread.Start();
				_addGUIDefinedEvents();
			}
			catch (Exception e)
			{
				_logException("Could not start the client thread", e);
				_setClientState(ClientState.Inactive);
			}
		}

		private void _addGUIDefinedEvents()
		{
			if (GameName == null || GameDisplayName == null || Events == null)
			{
				_logWarning("Incomplete game registration form");
				_setClientState(ClientState.Inactive);
			}
			else
			{
				RegisterGame(GameName, GameDisplayName, IconColor);
				RegisterEvents(Events);
			}
		}

		public void TryRegisteringEvents(Bind_Event[] theEvents)
		{
			RegisterEvents(theEvents);
		}

		private void RegisterEvents(Bind_Event[] theEvents)
		{
			foreach (Bind_Event bind_Event in theEvents)
			{
				QueueMsg queueMsg;
				if (bind_Event.handlers == null || bind_Event.handlers.Length == 0)
				{
					queueMsg = new QueueMsgRegisterEvent();
					queueMsg.data = new Register_Event(GameName, bind_Event.eventName, bind_Event.minValue, bind_Event.maxValue, bind_Event.iconId);
				}
				else
				{
					bind_Event.game = GameName;
					queueMsg = new QueueMsgBindEvent();
					queueMsg.data = bind_Event;
				}
				_mMsgQueue.PEnqueue(queueMsg);
			}
		}

		public void RegisterGame(string name, string displayName, IconColor iconColor)
		{
			GameName = name.ToUpper();
			GameDisplayName = displayName;
			IconColor = iconColor;
			Register_Game register_Game = new Register_Game();
			register_Game.game = GameName;
			register_Game.game_display_name = GameDisplayName;
			register_Game.icon_color_id = iconColor;
			QueueMsgRegisterGame queueMsgRegisterGame = new QueueMsgRegisterGame();
			queueMsgRegisterGame.data = register_Game;
			_mMsgQueue.PEnqueue(queueMsgRegisterGame);
		}

		public void RemoveGame(string name)
		{
			GameName = name.ToUpper();
			QueueMsgRemoveGame queueMsgRemoveGame = new QueueMsgRemoveGame();
			queueMsgRemoveGame.data = new Game(GameName);
			_mMsgQueue.PEnqueue(queueMsgRemoveGame);
		}

		public void EndConnection()
		{
			_logDbgMsg("Ending Connection");
			_mGameSenseWrkShouldRun = false;
			if (_currentRequest != null)
			{
				_currentRequest.Abort();
			}
		}

		private void _gamesenseWrk()
		{
			QueueMsg queueMsg = null;
			Stopwatch stopwatch = new Stopwatch();
			stopwatch.Start();
			while (_mGameSenseWrkShouldRun)
			{
				switch (_mClientState)
				{
				case ClientState.Active:
				{
					QueueMsg queueMsg2;
					while ((queueMsg2 = _mMsgQueue.CDequeue()) == null)
					{
						Thread.Sleep(10);
						if (stopwatch.ElapsedMilliseconds > 1000)
						{
							queueMsg2 = new QueueMsgSendHeartbeat();
							queueMsg2.data = new Game(GameName);
							break;
						}
					}
					try
					{
						_sendMsg(queueMsg2);
						stopwatch.Reset();
						stopwatch.Start();
					}
					catch (ServerDownException e2)
					{
						_logException("Failed connecting to GameSense server", e2);
						queueMsg = queueMsg2;
						_setClientState(ClientState.Probing);
					}
					catch (CriticalMessageIllFormedException e3)
					{
						_logException("Message ill-formed", e3);
						_setClientState(ClientState.Inactive);
					}
					catch (Exception e4)
					{
						_logException("Failed processing msg", e4);
					}
					break;
				}
				case ClientState.Probing:
					_mServerPort = _getServerPort();
					if (_mServerPort == null)
					{
						_logWarning("Failed to obtain GameSense server port. GameSense will not function");
						_setClientState(ClientState.Inactive);
						break;
					}
					_initializeUris();
					if (queueMsg != null)
					{
						try
						{
							_sendMsg(queueMsg);
							queueMsg = null;
						}
						catch (ServerDownException e)
						{
							_logException("Failed connecting to GameSense server", e);
							_logDbgMsg("Retrying in 5 seconds...");
							Thread.Sleep(5000);
							break;
						}
					}
					_setClientState(ClientState.Active);
					break;
				case ClientState.Inactive:
					_logDbgMsg("Entering inactive state");
					_mGameSenseWrkShouldRun = false;
					break;
				default:
					_logErrorMsg("Unknown GameSense client state");
					_setClientState(ClientState.Inactive);
					break;
				}
			}
			_logDbgMsg("Worker exiting");
		}

		private void _setClientState(ClientState state)
		{
			if (_mClientState != state)
			{
				_mClientState = state;
				if (state == ClientState.Active && OnConnectionBecameActive != null)
				{
					OnConnectionBecameActive();
				}
				if (state == ClientState.Inactive && OnConnectionBecameInactive != null)
				{
					OnConnectionBecameInactive();
				}
			}
		}

		private static void _logException(string msg, Exception e)
		{
		}

		private static void _logWarning(string msg)
		{
		}

		private static void _logDbgMsg(string msg)
		{
		}

		private static void _logErrorMsg(string msg)
		{
		}

		private void _sendMsg(QueueMsg msg)
		{
			string text = _toJSON(msg.data);
			if (msg is JsonMsg jsonMsg)
			{
				text = jsonMsg.JsonText;
			}
			_logDbgMsg(text);
			try
			{
				_sendServer(msg.uri, text);
			}
			catch (WebException ex)
			{
				switch (ex.Status)
				{
				case WebExceptionStatus.ProtocolError:
					if (msg.IsCritical())
					{
						Stream responseStream = ex.Response.GetResponseStream();
						string message = new StreamReader(responseStream, Encoding.UTF8).ReadToEnd();
						responseStream.Close();
						throw new CriticalMessageIllFormedException(message);
					}
					break;
				case WebExceptionStatus.ConnectFailure:
					throw new ServerDownException(ex.Message);
				default:
					_logException("Unexpected status", ex);
					throw;
				}
			}
		}

		private void SendJson(Uri uri, string data, bool isCritical)
		{
			_logDbgMsg(data);
			try
			{
				_sendServer(uri, data);
			}
			catch (WebException ex)
			{
				switch (ex.Status)
				{
				case WebExceptionStatus.ProtocolError:
					if (isCritical)
					{
						Stream responseStream = ex.Response.GetResponseStream();
						string message = new StreamReader(responseStream, Encoding.UTF8).ReadToEnd();
						responseStream.Close();
						throw new CriticalMessageIllFormedException(message);
					}
					break;
				case WebExceptionStatus.ConnectFailure:
					throw new ServerDownException(ex.Message);
				default:
					_logException("Unexpected status", ex);
					throw;
				}
			}
		}

		private static string _getPropsPath()
		{
			if (Platform.IsWindows)
			{
				return Environment.ExpandEnvironmentVariables("%PROGRAMDATA%/SteelSeries/SteelSeries Engine 3/coreProps.json");
			}
			return "/Library/Application Support/SteelSeries Engine 3/coreProps.json";
		}

		private static string _readProps()
		{
			string path = _getPropsPath();
			string result = null;
			try
			{
				if (File.Exists(path))
				{
					result = File.ReadAllText(path);
				}
				else
				{
					_logErrorMsg("Could not read server props file, because it can't be found");
				}
			}
			catch (Exception e)
			{
				_logException("Could not read server props file", e);
			}
			return result;
		}

		private static string _getServerPort()
		{
			string result = null;
			string text = _readProps();
			if (text != null)
			{
				try
				{
					JObject jObject = JObject.Parse(text);
					coreProps coreProps = new coreProps
					{
						address = (string)jObject.GetValue("address")
					};
					string[] array = coreProps.address.Split(':');
					Convert.ToUInt16(array[1]);
					result = array[1];
				}
				catch (Exception e)
				{
					_logException("Cannot parse port information", e);
				}
			}
			return result;
		}

		private void _initializeUris()
		{
			_uriBase = new Uri("http://127.0.0.1:" + _mServerPort);
			QueueMsgRegisterGame._uri = new Uri(_uriBase, "game_metadata");
			QueueMsgBindEvent._uri = new Uri(_uriBase, "bind_game_event");
			QueueMsgRegisterEvent._uri = new Uri(_uriBase, "register_game_event");
			QueueMsgSendEvent._uri = new Uri(_uriBase, "game_event");
			QueueMsgSendHeartbeat._uri = new Uri(_uriBase, "game_heartbeat");
			QueueMsgRemoveGame._uri = new Uri(_uriBase, "remove_game");
			JsonMsg._bitmapEventUri = new Uri(_uriBase, "bitmap_event");
		}

		[DebuggerNonUserCode]
		private void _sendServer(Uri uri, string data)
		{
			byte[] bytes = Encoding.ASCII.GetBytes(data);
			HttpWebRequest httpWebRequest = (HttpWebRequest)WebRequest.Create(uri);
			httpWebRequest.ContentType = "application/json";
			httpWebRequest.Method = "POST";
			Stream requestStream = httpWebRequest.GetRequestStream();
			requestStream.Write(bytes, 0, bytes.Length);
			requestStream.Close();
			_currentRequest = httpWebRequest;
			try
			{
				((HttpWebResponse)httpWebRequest.GetResponse()).Close();
			}
			catch
			{
			}
		}

		private string _toJSON<T>(T obj)
		{
			string text = null;
			if (_mSerializer.TrySerialize(obj, out var data).Succeeded)
			{
				return fsJsonPrinter.CompressedJson(data);
			}
			throw new Exception("Failed serializing object: " + obj.ToString());
		}

		private bool _isClientActive()
		{
			return _mClientState == ClientState.Active;
		}

		public void SendEvent(string fullEventJson)
		{
			if (_isClientActive())
			{
				_mMsgQueue.PEnqueue(new JsonMsg
				{
					JsonText = fullEventJson
				});
			}
		}

		public void SendEvent(string upperCaseEventName, int value)
		{
			if (_isClientActive())
			{
				Send_Event send_Event = new Send_Event();
				send_Event.game = GameName;
				send_Event.event_name = upperCaseEventName;
				send_Event.data.value = value;
				QueueMsgSendEvent queueMsgSendEvent = new QueueMsgSendEvent();
				queueMsgSendEvent.data = send_Event;
				_mMsgQueue.PEnqueue(queueMsgSendEvent);
			}
		}

		public void RegisterEvent(string upperCaseEventName, int minValue = 0, int maxValue = 100, EventIconId iconId = EventIconId.Default)
		{
			if (_isClientActive())
			{
				QueueMsgRegisterEvent queueMsgRegisterEvent = new QueueMsgRegisterEvent();
				queueMsgRegisterEvent.data = new Register_Event(GameName, upperCaseEventName, minValue, maxValue, iconId);
				_mMsgQueue.PEnqueue(queueMsgRegisterEvent);
			}
		}
	}
}
