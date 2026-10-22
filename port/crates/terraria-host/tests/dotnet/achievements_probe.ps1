# What Newtonsoft.Json (the game's dll, meta\refs) writes for the shapes of Terraria.Achievements: a
# Dictionary<string, Achievement> through BsonWriter, as AchievementManager.Save does before the AES layer.
# Classes below copy the ATTRIBUTES and member layout of Achievement / AchievementCondition / CustomIntCondition /
# CustomFloatCondition (decompile\Terraria\Terraria\Achievements, GameContent\Achievements); no game code is loaded.
# Usage (headless): powershell -NoProfile -ExecutionPolicy Bypass -File achievements_probe.ps1 <path to Newtonsoft.Json.dll>
# Output: the JSON text of the same object (member order), the BSON bytes in hex, and the member order of a
# two-class chain under the default contract.
param([string]$Dll)
$ErrorActionPreference = 'Stop'
$Dll = (Resolve-Path $Dll).Path
Add-Type -Path $Dll
$src = @'
using System.Collections.Generic;
using System.IO;
using Newtonsoft.Json;
using Newtonsoft.Json.Bson;
#pragma warning disable 618
namespace Probe {
    [JsonObject(MemberSerialization.OptIn)]
    public abstract class Condition {
        public readonly string Name;
        protected object _tracker;
        [JsonProperty("Completed")] private bool _isCompleted;
        public bool IsCompleted { get { return _isCompleted; } }
        protected Condition(string name) { Name = name; }
        public void Complete() { _isCompleted = true; }
    }
    public class FlagCondition : Condition { public FlagCondition(string n) : base(n) { } }
    public class IntCondition : Condition {
        [JsonProperty("Value")] private int _value;
        private int _maxValue;
        public int Value { get { return _value; } set { _value = value; } }
        public IntCondition(string n, int max) : base(n) { _maxValue = max; }
    }
    public class FloatCondition : Condition {
        [JsonProperty("Value")] private float _value;
        private float _maxValue;
        public float Value { get { return _value; } set { _value = value; } }
        public FloatCondition(string n, float max) : base(n) { _maxValue = max; }
    }
    [JsonObject(MemberSerialization.OptIn)]
    public class Achievement {
        public readonly string Name;
        public readonly int Id;
        [JsonProperty("Conditions")] private Dictionary<string, Condition> _conditions = new Dictionary<string, Condition>();
        private int _completedCount;
        public bool IsCompleted { get { return _completedCount == _conditions.Count; } }
        public Achievement(string name) { Name = name; }
        public void Add(Condition c) { _conditions[c.Name] = c; }
    }
    // member order across a class chain under the default (opt-out) contract
    public class PlainBase { public int BaseField = 1; public int BaseProp { get; set; } [JsonProperty] private int basePrivate = 2; }
    public class PlainDerived : PlainBase { public int DerivedField = 3; public int DerivedProp { get; set; } [JsonProperty] private int derivedPrivate = 4; }
    public static class Run {
        public static string Go() {
            var all = new Dictionary<string, Achievement>();
            var a = new Achievement("TIMBER"); var f = new FlagCondition("ItemPickupCondition"); f.Complete(); a.Add(f); all[a.Name] = a;
            var b = new Achievement("LUCKY_BREAK"); var i = new IntCondition("Use", 1000); i.Value = 37; b.Add(i); b.Add(new FlagCondition("Other")); all[b.Name] = b;
            var c = new Achievement("MARATHON_MEDALIST"); var x = new FloatCondition("Move", 1106688f); x.Value = 1234.5f; x.Complete(); c.Add(x); all[c.Name] = c;
            var settings = new JsonSerializerSettings();
            string json = JsonConvert.SerializeObject(all, settings);
            var ms = new MemoryStream();
            var w = new BsonWriter(ms);
            JsonSerializer.Create(settings).Serialize(w, all);
            w.Flush();
            var hex = new System.Text.StringBuilder();
            foreach (var by in ms.ToArray()) hex.Append(by.ToString("x2"));
            return json + "\n" + hex.ToString() + "\n" + JsonConvert.SerializeObject(new PlainDerived());
        }
    }
}
'@
Add-Type -TypeDefinition $src -ReferencedAssemblies $Dll -IgnoreWarnings
[Probe.Run]::Go()
