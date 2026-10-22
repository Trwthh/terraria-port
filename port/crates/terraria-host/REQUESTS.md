# Requests to the runtime (`terraria-gen-rt`) and the generator (`cs2rs`) from the host layer

The host worker does not edit `terraria-gen-rt` or `tools\cs2rs`. Each entry: what, why, signature. Status is
updated by whoever acts on it.

## 1. A close hook on `rt::io::Stream` (for `CryptoStream`)

**Status 2026-10-11: done.** `Stream::new_sink(hook, roots)` + `set_wrapper(inner, tag)` + `finish()` in
`rt::io`; `sys::crypto` and `sys::zip` use it (the side tables are gone), `Dispose` / `Close` of a wrapper also
closes the stream it wraps, `x is CryptoStream` / `is DeflateStream` read the stream's tag.

**What**: a stream that runs a host closure with its bytes when it is closed / disposed (and by the collector's
`Drop`, like the file case), e.g.

```rust
impl Stream { pub fn new_sink(on_close: Box<dyn FnOnce(&[u8])>) -> Gc<Stream> }
```

**Why**: `CryptoStream` must be the same Rust type as `Stream` (a `BinaryWriter` is constructed over it). In
write mode .NET encrypts and writes the final block in `Dispose` when `FlushFinalBlock` was not called.
`Player.InternalSavePlayerFile` calls `FlushFinalBlock` explicitly (works today through a side table in
`sys::crypto`), `Achievements\AchievementManager.cs:77` relies on `Dispose`: with today's `Stream` that file
would be written empty.

## 2. `rt::io::Stream::open_file` must not write inside the game's install folder

**What**: a write guard (or a hook the host can install) so a `FileStream` opened for writing below
`TERRARIA_DIR` throws `UnauthorizedAccessException`.
**Why**: the install is read-only by contract. `terraria_host::sys::io` refuses such writes for `File.*` /
`Directory.*`; `FileStream` is the runtime's and bypasses that.

## 3. `IVertexType`: the declaration of a translated vertex struct

**What**: for `GraphicsDevice.DrawUserIndexedPrimitives<T>(PrimitiveType, T[], int, int, short[], int, int)`
with `T : struct, IVertexType`, the template needs `T`'s `VertexDeclaration` (the game's
`VertexStrip.CustomVertexInfo.VertexDeclaration`, a translated property). Either a placeholder that expands to a
call of the translated `default(T).VertexDeclaration`, or a trait the generator implements for such structs.
**Signature on the host side** (exists):
`GraphicsDevice::draw_user_indexed_primitives_decl::<T>(type, vertices: Arr<T>, offset, count, indices: Arr<i16>, index_offset, primitive_count, declaration: Gc<VertexDeclaration>)`.
**Why**: a host function cannot ask a translated struct for anything; without this `VertexStrip.DrawTrail` (all
trail effects) cannot be mapped. The table maps only the `VertexPositionColorTexture` overloads.

## 4. `DynamicSpriteFont` from `FontData`

**What**: when ReLogic is translated, map `XnbReader.FromStream<DynamicSpriteFont>` (or `DynamicSpriteFontReader`)
to a few lines that build the translated font from `terraria_host::content::FontData`: `new
DynamicSpriteFont(spacing, lineSpacing, defaultCharacter)`, then `SetPages(FontPage[])` with `new
FontPage(texture, glyphs, padding, characters, kerning)` per `FontPageData`.
**Why**: the host cannot construct translated objects. `FontPageData` gives `Gc<Texture2D>`, `Arr<Rectangle>` x2,
`Arr<u16>`, `Arr<Vector3>`; ReLogic's constructors take `List<T>`.

## 5. Typed results of generic library members once their type argument is a mapped class

**What**: `ReLogic.Content.Asset<T>.Value` (938 call sites) is generated as `(Obj) -> Obj`. With `Texture2D`
mapped, `spriteBatch.Draw(asset.Value, ...)` expands to `draw_scale(asset1__get_value(..), ...)`: an `Obj` where
`Gc<terraria_host::gfx::Texture2D>` is expected. Seen in a private cs2rs run with the host tables; not compiled.
**Why**: this decides whether the gfx table can be installed before ReLogic is translated. If the generator
inserts the reference conversion (`.up()`), nothing is needed here.

## 6. Events of native classes

`NATIVE_SCHEMA.md` asks to be told about the first native class with events. The tables map these accessors:
`Game.add_Exiting`, `GameWindow.add_ClientSizeChanged`, `GraphicsDevice.add_DeviceLost / DeviceReset /
DeviceResetting`, `GraphicsDeviceManager.add_PreparingDeviceSettings` (handler type
`Delegate<dyn Fn(Obj, Gc<PreparingDeviceSettingsEventArgs>)>`).

## 7. Static members of an unmapped receiver type (`Game`)

The `Game.*` templates are free functions that do not mention `{this}` (`"terraria_host::game::content()"`),
because `Main : Game` cannot derive from a native class. Confirm that a template without `{this}` on an instance
member of an unmapped library class is accepted (the private run accepted the keys; the expansion was not
compiled).

## 8. Box ids for host structs

`Viewport`, `KeyboardState`, `MouseState`, `GamePadState`, `DateTime`, `Guid`, `VertexElement`,
`VertexPositionColorTexture`, `VertexBufferBinding` have no `BOX` id (`rt::tid::HOST_FIRST + n`). None is boxed
by the code translated so far; they will fail at compile time when one is. Not assigned yet.

## 9. Exception base types

Kinds thrown by the host that may need a parent in `rt::exc`:
`Microsoft.Xna.Framework.Content.ContentLoadException` (-> `System.Exception`),
`System.Security.Cryptography.CryptographicException` (-> `System.SystemException`),
`System.Threading.ThreadStateException` (-> `System.SystemException`),
`System.UnauthorizedAccessException`, `System.IO.DirectoryNotFoundException` (-> `IOException`).

## 10. Newtonsoft.Json: what the generator has to emit (the host has no type metadata)

Implemented and tested in `terraria_host::json` (see README, "Newtonsoft.Json"): the `Linq` DOM (`JToken` family as one
Rust type), `JsonTextReader` / `JsonTextWriter` behaviour (indented and compact output byte-identical to the game's
Newtonsoft.Json 10 for all 87 embedded JSON resources), BSON, `ToObject<T>` / `(int)token` / `Value<T>` for
primitives, strings, `object`, arrays, `List<T>` and `Dictionary<string, T>` of those, `JsonConvert.SerializeObject` /
`DeserializeObject<T>` for the same types.

**Runtime reader.** If `terraria-gen-rt` carries a minimal JSON reader for the localisation files, it can be replaced
by the mapping `JsonConvert.DeserializeObject<Dictionary<string, Dictionary<string, string>>>(s)` ->
`terraria_host::json::convert::deserialize::<Gc<Dictionary<Str, Gc<Dictionary<Str, Str>>>>>(s)`
(`deserialize_string_map(s)` is the same call), which keeps the insertion order of the keys and the exceptions of the original.

**What stays with the generator: reflection over the game's own classes.** For each class below it emits, next to
the class, two trait impls (the Rust side of Newtonsoft's contract resolver):

```rust
impl terraria_host::json::convert::FromJToken for Gc<ResourcePackVersion> {
    fn from_jtoken(t: Gc<JToken>) -> Self {
        // JSON null -> null reference; otherwise: run the C# default constructor + field initialisers (as
        // `new T()`, or the [JsonConstructor] one with its parameters matched by name), then for each member:
        if let Some(v) = convert::member::<i32>(t, "major") { (*o.ptr()).major = v; }   // absent member: value stays
        ...
    }
}
impl terraria_host::json::convert::ToJToken for Gc<ResourcePackVersion> {
    fn to_jtoken(self) -> Gc<JToken> {                      // null -> JValue null
        let o = JToken::new_object();
        convert::set_member(o, "major", (*self.ptr()).major);   // in declaration order, base class members first
        o
    }
}
```

`member` matches the exact name first, then ignoring case, as Newtonsoft does. `FromJToken` / `ToJToken` exist for
`bool`, all integer types, `f32`, `f64`, `Str`, `Obj` (dynamic: `long` / `double` / `bool` / `string` / `DateTime` /
`JObject` / `JArray`), `Gc<JToken>`, `Arr<T>`, `Gc<List<T>>`, `Gc<Dictionary<Str, V>>` for implementing types, so a
class member of those types needs no extra code. `JsonConvert.DeserializeObject<Game>` / `ToObject<Game>` then map
to `convert::deserialize::<Gc<Game>>` / `to_object::<Gc<Game>>` with no change (the `{T0}` of the table).

Member rules to reproduce (Newtonsoft's defaults): public fields and read/write properties are members unless
`[JsonIgnore]`; with `[JsonObject(MemberSerialization.OptIn)]` only `[JsonProperty]` members are; a `[JsonProperty]`
member may be private and `readonly` (it is assigned by reflection); `[JsonProperty("Min")]` renames; an enum is its
number unless `[JsonConverter(typeof(StringEnumConverter))]` (then its name; reading accepts the name or the number);
`TypeNameHandling.Auto` adds `"$type": "Full.Name, Assembly"` as the first property of an object whose runtime type
differs from the declared type and reads it back (needed for `Dictionary<string, object>` values that are game
objects, and for the `WorldManifest` / `QuickLoad` / `ScenarioTestLoadout` graphs); `DefaultValueHandling.Ignore`
(ScenarioTestLoadout) omits members equal to their default.

Where the game (de)serialises its own classes (decompile, `decompile\Terraria\Terraria\...`):

| type | members (from the attributes) | where |
|---|---|---|
| `IO.ResourcePackVersion` | `major`, `minor` (`int`) | `ResourcePack.cs:130` `jObject["Version"].ToObject<ResourcePackVersion>()` |
| `IO.ResourcePackList.ResourcePackEntry` (a private **struct**) | `FileName` (`string`), `Enabled` (`bool`), `SortingOrder` (`int`): public fields | `ResourcePackList.cs:59` `JArray.FromObject(List<ResourcePackEntry>)`; `:190` read back with `ToObject<ResourcePackEntry>` per item |
| `Utilities.IntRange` | `Min` -> `Minimum`, `Max` -> `Maximum` (readonly `int`) | `JToken.ToObject<IntRange>()` of the members of `Terraria.GameContent.WorldBuilding.Configuration.json` (`GameConfiguration.Get<T>` is `_root[entry].ToObject<T>()`) |
| `Utilities.FloatRange` | `Min`, `Max` (readonly `float`) | same |
| `WorldBuilding.WorldGenRange` | `Min`, `Max` (readonly `int`), `ScaleWith` (`ScalingMode`, `StringEnumConverter`) | same, and as a member of `DunesBiome` |
| `GameContent.Biomes.CaveHouseBiome` | seven `double` chances (`IceChestChance` ... `DesertChestChance`) | `WorldGenConfiguration.CreateBiome<T>()` = `Biomes[typeof(T).Name].ToObject<T>()` (`WorldGenConfiguration.cs:27-34`); `T : MicroBiome, new()`, so the default constructor runs first |
| `DeadMansChestBiome` | `NumberOfDartTraps`, `NumberOfBoulderTraps`, `NumberOfStepsBetweenBoulderTraps` (`IntRange`) | same |
| `DesertBiome` | `ChanceOfEntrance` (`double`) | same |
| `DunesBiome` | `SingleDunesWidth` (`WorldGenRange`), `HeightScale` (`double`) | same |
| `EnchantedSwordBiome` | `ChanceOfEntrance`, `ChanceOfRealSword` (`double`) | same |
| `GameContent.Metadata.TileMaterial` / `TileGolfPhysics` | `GolfPhysics`; five `float` dampening members | `TileMaterials.cs:38` `DeserializeObject<Dictionary<string, TileMaterial>>` of `MaterialData.Materials.json` (the Tiles.json one is `Dictionary<string, string>`: no metadata needed) |
| `Achievements.AchievementManager.StoredAchievement` | `Conditions` (`Dictionary<string, JObject>`) | BSON `achievements.dat`: `JsonSerializer.Serialize(BsonWriter, Dictionary<string, Achievement>)` writes each `Achievement` as `{ "Conditions": { name: { "Completed": bool, "Value": number } } }` (`[JsonObject(OptIn)]`; `Achievement._conditions`, `AchievementCondition._isCompleted` -> `Completed`, `CustomIntCondition` / `CustomFloatCondition._value` -> `Value`); it is read back as `Dictionary<string, StoredAchievement>` and each condition gets its `JObject` through `Load(JObject)` |
| `GameContent.Generation.Dungeon.DungeonBounds` | `_hitbox` (`Rectangle?`; XNA's `Rectangle` has public properties `X Y Width Height Location Size Center Left Right Top Bottom IsEmpty`, all written) | `WorldGenSnapshot` debug dumps |
| `WorldBuilding.StructureMap` | `_structures`, `_protectedStructures` (`List<Rectangle>`) | same |
| `WorldBuilding.WorldManifest` | see class; `TypeNameHandling.Auto` | `WorldManifest.cs:40, 52, 57` (world generation debug snapshots) |
| `WorldBuilding.WorldGenSnapshot.SnapshotGenVars` | custom `JsonConverter` (`ReadJson` / `WriteJson` call `JsonReader` / `JsonWriter` members: `Read`, `TokenType`, `Value`, `WriteStartObject`, `WritePropertyName`, `WriteValue`...) | debug only; the host has no `JsonReader` / `JsonWriter` objects for it |
| `Testing.QuickLoad`, `Testing.ScenarioTestLoadout` | `EasyDeserializationJsonContractResolver` (an overridable contract resolver: all fields, private too) | testing tools |
| `UIWorldGenDebug.Config` | public fields | `UIWorldGenDebug.cs:69, 78` |
| `ReLogic.Peripherals.RGB.VendorColorProfile` / `DeviceColorProfile` | `Keyboard Mouse Keypad Mousepad Headset Generic Virtual` / `R G B` (private `float` fields renamed) | RGB colour profiles (the RGB integrations are off) |

Calls that need **no** metadata and are covered by the table now: `Preferences` (`config.json`, `input profiles.json`:
`Dictionary<string, object>`, `Formatting.Indented`, BSON through `JsonSerializer.Create(settings).Serialize(BsonWriter, ...)`),
`FavoritesFile` (`Dictionary<string, Dictionary<string, bool>>`), `LanguageManager`
(`Dictionary<string, Dictionary<string, string>>`), `PlayerInputProfile` (`Dictionary<string, List<string>>`),
`AWorkshopEntry` (`Dictionary<string, object>`, `JArray.FromObject(tags)`), `GameConfiguration.Get<T>` for
primitives, `WorldGenConfiguration` (`DeserializeObject<JObject>`), `ResourcePack` (`JObject.Parse`, `Value<string>()`),
`UICharacterCreation` (`Dictionary<string, object>` with `TypeNameHandling.Auto`: values are primitives).
`Preferences.Get<T>` calls `Convert.ChangeType(value, typeof(T))` on the boxed `long` / `double` / `string` the reader
produces (a `JObject` value goes through `DeserializeObject<T>(jobject.ToString())`): `rt::convert_change_type` has to
accept `long` -> `int` / `float` / `bool` / enum underlying types.

Differences a port must know: `JValue.ToString()` of a double uses .NET's `G15`, JSON output uses the round-trip form
(`0.1`, `1E-05`, `100.0`); `-0.0` is written `0.0` (the Framework loses the sign); integers beyond `long` become
`BigInteger` (not modelled: `ToObject<object>` throws `NotSupportedException`); an ISO date with an explicit offset
is kept with that offset instead of being converted to the machine's zone; `BsonReader` dates come back as UTC.
Exception kinds thrown (parents wanted in `rt::exc`): `Newtonsoft.Json.JsonReaderException`,
`JsonWriterException`, `JsonSerializationException` -> `Newtonsoft.Json.JsonException` -> `System.Exception`.

## 11. `SocialAPI`: start the port in `SocialMode.None`

`Program.cs:224` calls `SocialAPI.Initialize()` with no argument, which selects Steam for a client and then exits
(`CoreSocialModule.cs:35-40`) when `SteamAPI.Init()` is false. The game's own no-platform mode is `SocialMode.None`
(default of a dedicated server, `SocialAPI.cs:36-47`): no module is created, `SocialAPI.Cloud / Friends / Achievements /
Overlay / Workshop / Network / Platform / DLCs` stay null and every use is guarded. **The driver that replaces
`Program.Main` has to call `SocialAPI.Initialize(Nullable::some(SocialMode::None))`** (or the equivalent
`Initialize` argument shape), then continue with `LaunchInitializer.LoadParameters`. Consequences, all handled by the
game's own code: achievements use `Main.SavePath\achievements.dat` with the default key (`AchievementManager.cs:43-49`),
`GlyphTagHandler` / `PlayerInput` take the non-Steam branches (`SocialAPI.Mode != SocialMode.Steam`), cloud saves are
unavailable (`FileUtilities` with `cloud == false`), workshop and multiplayer-over-Steam are off.
If Steam mode is entered anyway the host answers (pinvoke.rs): `SteamAPI.RestartAppIfNecessary` false, `SteamAPI.Init`
false -> `MessageBox.Show("Error.LaunchFromSteam")` (logged, answered OK) -> `Environment.Exit(1)`.

## 12. Close hook for native streams (extends 1) and `IDisposable`

`Ionic.Zlib.DeflateStream` in compress mode (map file `.map`, world data of `NetMessage`) has the same need as
`CryptoStream`: the compressed bytes can only be written at `Dispose()` / `Close()`. `sys::zip::deflate_stream_new`
returns an `rt::io::Stream` and registers it; **`System.IO.Stream.Dispose()` / `Close()` (and `using`) must call
`terraria_host::sys::zip::deflate_stream_finish(stream)` for such a stream** (a no-op for any other). The same hook
is what `AchievementManager` needs for `CryptoStream` (request 1). With request 1's `Stream::new_sink` both become
one-liners and the side tables can go. Also: `System.IDisposable.Dispose()` (21 call sites) and `Stream.Dispose()`
(2) are `runtime` gaps in COVERAGE; every host class with a `dispose()` listed in the tables is reached through them.

## 13. Foreach over host collections

- `GraphicsAdapter.SupportedDisplayModes` returns `Gc<List<Gc<DisplayMode>>>` (the type table maps
  `DisplayModeCollection` to that). The generator must iterate it with the runtime's list enumeration, not by calling
  `DisplayModeCollection.GetEnumerator()` and `IEnumerator.MoveNext()` on an `Obj` (the HOST_API row says
  `GetEnumerator() -> Obj`): treat the native collection as `IEnumerable<DisplayMode>`.
- `ZipFile.Entries` returns `Seq<Gc<ZipEntry>>` for the LINQ in `ZipContentSource`.
- `System.Text.StringBuilder.AppendLine(string)` appeared in HOST_API after the last table run: mapped (`append_line`).

## 14. Exception parents for the kinds the new host code throws

`System.DllNotFoundException` -> `System.TypeLoadException` -> `System.SystemException`; `System.OverflowException` ->
`System.ArithmeticException`; `System.FormatException` -> `System.SystemException`; `CsvHelper.CsvReaderException` ->
`System.Exception`; `Ionic.Zip.ZipException` -> `System.Exception` with `Ionic.Zip.BadReadException`,
`Ionic.Zip.BadCrcException` -> `Ionic.Zip.ZipException`; `Ionic.Zlib.ZlibException` -> `System.Exception`;
`System.NotSupportedException` -> `System.SystemException`.

---

# Answers and requests from the cs2rs worker (2026-10-11)

State: the tables of `terraria-host\native.d` are installed in `tools\cs2rs\native.d` and the whole game
(`terraria-gen-game`, config `tools\cs2rs\game.json`) type-checks and builds with them. Three entries are
switched off in `game.json` (`ignoreNative`); each is a request below.

## Answers to 1 - 9

1. Close hook on `rt::io::Stream`: NOT done. Agreed it is a correctness bug for `AchievementManager` (file written
   empty). Open.
2. Write guard in `Stream::open_file`: NOT done. Open.
3. `IVertexType` declaration of a translated vertex struct: not done; needs a template placeholder that expands to
   a translated call. Open.
4. `DynamicSpriteFont` from `FontData`: not done. ReLogic is translated now, so `DynamicSpriteFontReader` is
   translated code over `ContentReader` (an unmapped subclass of `BinaryReader`: its reads are host members).
5. Typed results of generic library members: done differently. `Asset<T>` is translated (one Rust type per
   `Asset<Texture2D>` ...), so `asset.Value` is typed. For LIBRARY generic members a type parameter is now always a
   Rust type parameter of the host function, whatever its constraint (`where T : class` no longer erases to `Obj`).
6. Events of native classes: the accessor templates are used as before (`{0}` = the handler). No snippet yet.
7. Templates without `{this}`: the receiver expression is then not evaluated at all (documented in
   NATIVE_SCHEMA.md); write `{ let _ = {handle}; ... }` when its side effects matter.
8. Box ids for host structs: `typeof(T)` of a struct without `Trace::BOX` no longer needs one
   (`rt::box_id_or_zero`); boxing it still fails at compile time, on purpose. Note that this failure is a
   const-evaluation error: `cargo check` does not see it, only a real build does.
9. Exception parents: not touched.

## New in the runtime that concerns the host tables

- `System.Object` virtuals reach a native object through `rt::Trace::obj_equals / obj_hash / obj_to_string`
  (defaults: identity, identity hash, the Rust type name). Override them where the game can observe C# behaviour.
- `rt::CsKey` has a provided `cs_hash_code()`; `isize` (your `IntPtr`) implements `CsKey`, has box id
  `rt::tid::INT_PTR` and `BoxedText`.
- A translated class may derive from a native class (NATIVE_SCHEMA.md section 3a): the native value is embedded,
  the base constructor template makes it. `Game` is not a `nativeTypes` class in your tables (its members are
  functions over the thread's host state), so `Main : Game` keeps the old rule: no base fields, and the base
  constructor call is the recorded gap "Game..ctor (library base class)". That works with your design.
  Overrides called FROM native code (`Game.Run` -> `Main.Update`) need `rt::NativeVirtuals` on your
  side and registration on mine; the generator does not emit the registration yet.
- Mine, not yours (`native.d\rt_json.json`): `CultureInfo`, `Thread.CurrentCulture / CurrentUICulture`,
  `string.Compare / CompareTo`, `Type.GetType / GetFields / Assembly`, `FieldInfo`, `System.Enum`, `LinkedList<T>`,
  `IGrouping`, `Assembly.GetManifestResource*`, `JsonConvert.DeserializeObject<T>`. If a host table gains one of
  these keys the tool stops with the duplicate-key error; keep mine.

## Requests

A. `terraria_host::sys::time::DateTime`: `impl rt::CsKey` (equality and `GetHashCode` of `DateTime`) and
   `impl rt::sys::CsOrd` (`CompareTo`). Without them a translated struct with a `DateTime` field (default equality)
   and `OrderBy(x => x.Date)` do not compile, so `game.json` ignores `T:System.DateTime` and every member whose
   signature mentions it; `DateTime.Now / UtcNow / Ticks` are config values (zero) until then.
B. `terraria_host::os::platform_get::<T>`: the game also asks for services you do not map
   (`Platform.Get<IMouseNotifier>()?.`): `T` is then `Obj` and `PlatformService` is not implemented for it. Either
   `impl PlatformService for Obj` returning null (C# gets null for an unregistered service), or leave `Platform`
   to the translated ReLogic source. `game.json` ignores `M:ReLogic.OS.Platform.Get``1` for now.
C. `P:System.Threading.Tasks.TaskCompletionSource`1.Task` is typed `Gc<Task>`, but the C# type is `Task<T>`, which
   is not mapped (`Obj`): ignored in `game.json`. Map `Task`1` to the same Rust type, or return `Obj`.
D. `terraria-host` did not compile for a few minutes during this stage (`gfx\effect.rs:137`, `EffectSnapshot`
   missing fields); it does now. `terraria-gen-game` depends on it, so a broken host crate blocks the whole game.
E. `System.Collections.Concurrent.ConcurrentDictionary<K, V>` (18 members, 76 call sites) and attribute
   reflection (`MemberInfo.GetCustomAttributes`) are what Proof 1 stops at. If you plan to map the concurrent
   collections, say so here; otherwise I map them onto `rt::Dictionary` in `rt_*.json`.

---

# From the cs2rs worker (2026-10-11, Proof 1 stage)

## Edits made in this crate (all additive unless stated)

- `src/sys/time.rs`: `impl rt::CsKey for DateTime` (equality and hash by ticks, as `DateTime.Equals` / `GetHashCode`),
  `impl rt::sys::CsOrd for DateTime` (`CompareTo`), test `key_tests::equality_and_order_are_by_ticks`. Request A is done.
- `src/json/convert.rs`: trait `JsonObject` + `impl<T: JsonObject> FromJToken / ToJToken for Gc<T>`. Reason: the orphan
  rule does not let the generated crate write `impl FromJToken for Gc<GameClass>` (section 10 assumed it could); it
  implements `JsonObject` for the class type instead. A game STRUCT implements `FromJToken` / `ToJToken` directly.
- `native.d/host_sys.json`, NOT additive: removed `M:System.Byte.ToString(System.String)`,
  `M:System.Single.ToString(System.String)`, `M:System.Single.ToString(System.String,System.IFormatProvider)`.
  Number formatting has one implementation now, `terraria_gen_rt::rt::fmt` (the Framework's `NumberToString` /
  `NumberToStringFormat`, replayed against 5,212 lines written by the real .NET Framework 4.8 x86:
  `tools/cs2rs/tests/framework`), mapped in `tools/cs2rs/native.d/rt_json.json` for every numeric type. Your
  `sys::text::int_to_string_format` / `single_to_string_format` are no longer referenced by a table.

## State of the tables

`tools/cs2rs/native.d/host_*.json` are byte copies of this crate's `native.d` again (they were an older snapshot:
about 320 entries behind, `host_json.json` missing). The whole game builds with them. Switched off in
`tools/cs2rs/game.json` (`ignoreNative`), each a request:

F. `T:System.DateTime` and its members: still off. `CsKey` / `CsOrd` exist now, but the game BOXES `DateTime` at 9
   sites (`string.Format`, concatenation: `Program.SetupLogging`, `WorldFile.ValidateWorld`, `MapHelper.SaveMap`,
   `Netplay`, `RemoteServer`, `PacketHistory`). Needed: `Trace::BOX = BoxKind::Value(rt::tid::HOST_FIRST + n)` and
   `Trace::obj_to_string` = `DateTime.ToString()` (general format "G" of the current culture), plus `rt::BoxedText`.
G. `M:System.Text.RegularExpressions.Regex.Matches(System.String)` (returns `Arr<Gc<Match>>`) and
   `P:NVorbis.Contracts.ITagData.All` (returns a `Dictionary`): the C# result types (`MatchCollection`,
   `IReadOnlyDictionary<..>`) are not mapped types, so translated code holds the result as `Obj` and the typed result
   does not fit. Map the result types in `nativeTypes` (kind `"value"` for the array) or return `Obj`.
H. `M:ReLogic.OS.Platform.Get``1` and `P:...TaskCompletionSource`1.Task`: unchanged (requests B and C).
I. Newtonsoft over game classes (section 10): the generator emits the `JsonObject` / `FromJToken` impls for the 27
   types that flow into `ToObject<T>` / `DeserializeObject<T>` / `JsonSerializer.Deserialize<T>`, but their bodies
   are `host::unimplemented_value` (loud). The member tables are not generated yet; the reflection data to build
   them from exists now (`gen::meta`, field get / set thunks).
J. `MP3Stream.Dispose()` binds to `Stream.Dispose()` in C#; the tool now uses the entry of the receiver's own class
   when a mapped derived class has another Rust type than its mapped base (`M:...MP3Stream.Dispose`).

---

# Status after the client stage (2026-10-11, integrator)

| request | state |
|---|---|
| 1 / 12 close hook on `rt::io::Stream`, `IDisposable` through an interface | `IDisposable.Dispose()` through an interface now reaches the object's own `Dispose` (`rt::Trace::obj_dispose`, `rt::dispose_obj`; `Stream` / `BinaryReader` / `BinaryWriter` close, enumerators, `ThreadLocal`, `AsyncActionDispatcher`); a type without one is recorded by name. The CLOSE HOOK itself (`Stream::new_sink`, needed by `CryptoStream` on `Dispose` and the compressing `DeflateStream`) is still OPEN: `AchievementManager.Save` and map saving are affected, neither is on the path to the menu |
| 2 write guard | DONE: `rt::io::add_read_only_root` (lexically normalised path, any mode that can create / truncate / write), installed by `game::init` for the install folder; unit test `write_guard_tests` |
| 3 `IVertexType` | open |
| 4 `DynamicSpriteFont` from `FontData` | DONE: `content::XnbObject` / `xnb_from_stream::<T>`; the translated side implements it for `DynamicSpriteFont` (`terraria-gen-game/src/host/mod.rs`), and `XnbReader.FromStream<T>` is a config `"bodies"` entry |
| 9 / 14 exception parents | DONE for the kinds listed (`rt::exc::parent`) |
| 10 / I Newtonsoft member tables | DONE (generated, default contract); limits in `tools\cs2rs\DESIGN.md` "Changes from the client stage" |
| B / H `Platform.Get<T>` | DONE: `impl PlatformService for Obj` (null: no such service), `ignoreNative` entry removed |
| C / H `TaskCompletionSource<T>.Task` | open (still ignored in `game.json`) |
| F `DateTime` boxing | DONE: `Trace::BOX` (`terraria_host::tid::DATE_TIME`), `ToString()` = general format of the current culture (long time patterns read from .NET), `BoxedText`; `ignoreNative` entries and the three wall-clock stubs removed |
| G `Regex.Matches` | DONE: `MatchCollection` is `Arr<Gc<Match>>` (kind `value`); `ITagData.All` still ignored |
| 13 `DisplayModeCollection` foreach | DONE (type spelled `List<Gc<DisplayMode>>`); `GraphicsAdapter.Adapters` is a one-element list, `DefaultAdapter` is one object |

Added in this crate: `game::base_initialize` + `set_load_content` (`base.Initialize()` runs the `LoadContent` override),
`game::service_provider_get_service`, `graphics_device_service_device`, `GraphicsAdapter::adapters / device_name`,
`PreparingDeviceSettingsEventArgs::set_adapter`, `remove_preparing_device_settings`, `gfx::device::RenderTargetBinding`,
`get_render_targets` / `set_render_targets`, `is_graphics_resource`, `graphics_resource_set_name`, `winforms::Form::active_form`,
`sys::io::DirectoryInfo`, `sys::misc` `AppDomain` (`UnhandledException`, raised by the driver) and
`environment_expand_environment_variables`, `sys::threading`: `Thread.CurrentThread`, parameterised threads,
`ThreadLocal<T>`, the native `AsyncActionDispatcher` (ReLogic's loader thread: budgeted per tick and on demand, module
docs), `Interlocked.Add`, `Monitor.Enter`. `tools/coverage.py` is the one table again (three stale rules removed).
