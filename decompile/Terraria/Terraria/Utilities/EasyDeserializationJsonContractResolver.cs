using System;
using System.ComponentModel;
using System.Reflection;
using Newtonsoft.Json;
using Newtonsoft.Json.Serialization;

namespace Terraria.Utilities
{
	public class EasyDeserializationJsonContractResolver : DefaultContractResolver
	{
		protected override JsonContract CreateContract(Type objectType)
		{
			JsonContract jsonContract = base.CreateContract(objectType);
			if (jsonContract is JsonStringContract && objectType != typeof(string))
			{
				TypeConverter converter = TypeDescriptor.GetConverter(objectType);
				if (converter != null && converter.CanConvertTo(typeof(string)) && !converter.CanConvertFrom(typeof(string)))
				{
					jsonContract = base.CreateObjectContract(objectType);
				}
			}
			if (objectType.IsArray || objectType.IsValueType)
			{
				jsonContract.IsReference = false;
			}
			return jsonContract;
		}

		protected override JsonProperty CreateProperty(MemberInfo member, MemberSerialization memberSerialization)
		{
			JsonProperty jsonProperty = base.CreateProperty(member, memberSerialization);
			if (!jsonProperty.Writable)
			{
				jsonProperty.Ignored = true;
			}
			return jsonProperty;
		}
	}
}
