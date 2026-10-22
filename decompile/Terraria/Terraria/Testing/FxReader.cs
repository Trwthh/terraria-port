using System;
using System.IO;
using Microsoft.Xna.Framework.Content.Pipeline;
using Microsoft.Xna.Framework.Content.Pipeline.Graphics;
using Microsoft.Xna.Framework.Content.Pipeline.Processors;
using Microsoft.Xna.Framework.Graphics;
using ReLogic.Content;
using ReLogic.Content.Readers;

namespace Terraria.Testing
{
	public class FxReader : IAssetReader
	{
		private class DummyPipelineContext : ContentProcessorContext
		{
			private readonly ContentBuildLogger _logger = new PipelineLogger();

			public override TargetPlatform TargetPlatform => TargetPlatform.Windows;

			public override GraphicsProfile TargetProfile => GraphicsProfile.Reach;

			public override ContentBuildLogger Logger => _logger;

			public override OpaqueDataDictionary Parameters
			{
				get
				{
					throw new NotImplementedException();
				}
			}

			public override string BuildConfiguration => "Release";

			public override string OutputFilename
			{
				get
				{
					throw new NotImplementedException();
				}
			}

			public override string OutputDirectory
			{
				get
				{
					throw new NotImplementedException();
				}
			}

			public override string IntermediateDirectory
			{
				get
				{
					throw new NotImplementedException();
				}
			}

			public override void AddDependency(string filename)
			{
				throw new NotImplementedException();
			}

			public override void AddOutputFile(string filename)
			{
				throw new NotImplementedException();
			}

			public override TOutput BuildAndLoadAsset<TInput, TOutput>(ExternalReference<TInput> sourceAsset, string processorName, OpaqueDataDictionary processorParameters, string importerName)
			{
				throw new NotImplementedException();
			}

			public override ExternalReference<TOutput> BuildAsset<TInput, TOutput>(ExternalReference<TInput> sourceAsset, string processorName, OpaqueDataDictionary processorParameters, string importerName, string assetName)
			{
				throw new NotImplementedException();
			}

			public override TOutput Convert<TInput, TOutput>(TInput input, string processorName, OpaqueDataDictionary processorParameters)
			{
				throw new NotImplementedException();
			}
		}

		private class PipelineLogger : ContentBuildLogger
		{
			public override void LogImportantMessage(string message, params object[] messageArgs)
			{
			}

			public override void LogMessage(string message, params object[] messageArgs)
			{
			}

			public override void LogWarning(string helpLink, ContentIdentity contentIdentity, string message, params object[] messageArgs)
			{
			}
		}

		private readonly GraphicsDevice _graphicsDevice;

		public FxReader(GraphicsDevice graphicsDevice)
		{
			_graphicsDevice = graphicsDevice;
		}

		public T FromStream<T>(Stream stream) where T : class
		{
			if (typeof(T) != typeof(Effect))
			{
				throw AssetLoadException.FromInvalidReader<FxReader, T>();
			}
			string effectCode;
			using (StreamReader streamReader = new StreamReader(stream))
			{
				effectCode = streamReader.ReadToEnd();
			}
			CompiledEffectContent compiledEffectContent = new EffectProcessor().Process(new EffectContent
			{
				EffectCode = effectCode
			}, new DummyPipelineContext());
			return new Effect(_graphicsDevice, compiledEffectContent.GetEffectCode()) as T;
		}
	}
}
