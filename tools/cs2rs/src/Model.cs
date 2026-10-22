// Model: configuration, the name rule, the C# -> Rust type map and the table of translated members.
using System.Collections.Concurrent;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;

namespace Cs2Rs;

public sealed class TypeCfg
{
    [JsonPropertyName("type")] public string Type { get; set; }
    /// Member names to translate (all overloads of a name). null = the whole type.
    [JsonPropertyName("members")] public List<string> Members { get; set; }
    /// Emit reflect_fields / reflect_prop (what tools\refdump reads by reflection).
    [JsonPropertyName("reflect")] public bool Reflect { get; set; }
}

public sealed class NativeTypeCfg
{
    [JsonPropertyName("rust")] public string Rust { get; set; }
    /// "struct" (value, used as is), "class" (Gc<rust>), "value" (a Copy handle type used as is, e.g. Seq<T>)
    [JsonPropertyName("kind")] public string Kind { get; set; }
    /// Optional, for "class": a Rust expression over `{0}` (an `Obj`, never null) that answers `x is ThisClass`.
    /// Needed when several C# classes share one Rust type (a `Stream` hierarchy behind one enum); without it the
    /// test is "the object's Rust type is exactly `rust`". NATIVE_SCHEMA.md.
    [JsonPropertyName("is")] public string Is { get; set; }
}

public sealed class Config
{
    [JsonPropertyName("sources")] public List<string> Sources { get; set; } = new();
    [JsonPropertyName("references")] public List<string> References { get; set; } = new();
    [JsonPropertyName("types")] public List<TypeCfg> Types { get; set; } = new();
    [JsonPropertyName("nativeTypes")] public Dictionary<string, NativeTypeCfg> NativeTypes { get; set; } = new();
    /// doc id -> template string, or { "get": ..., "set": ... } for properties / fields.
    [JsonPropertyName("nativeMembers")] public Dictionary<string, JsonElement> NativeMembers { get; set; } = new();
    [JsonPropertyName("nativeSlices")] public Dictionary<string, string> NativeSlices { get; set; } = new();
    [JsonPropertyName("stubs")] public Dictionary<string, string> Stubs { get; set; } = new();
    [JsonPropertyName("genModule")] public string GenModule { get; set; } = "gen";
    [JsonPropertyName("include")] public List<string> Include { get; set; } = new();
    /// Rust path of the generated host functions (Host.cs) and of the hand-written registry they call.
    [JsonPropertyName("hostModule")] public string HostModule { get; set; } = "crate::host::api";
    [JsonPropertyName("hostRoot")] public string HostRoot { get; set; } = "crate::host";
    /// Every type declared in these namespaces (exact match, or prefix when the entry ends with ".*") is translated whole.
    [JsonPropertyName("namespaces")] public List<string> Namespaces { get; set; } = new();
    /// Translate every struct / interface declared in the sources that is not listed (plain data the listed classes need).
    [JsonPropertyName("autoStructs")] public bool AutoStructs { get; set; }
    /// Types never translated (stay opaque) even when a namespace entry covers them.
    [JsonPropertyName("exclude")] public List<string> Exclude { get; set; } = new();
    /// Float evaluation rule: "f32" (as written) or "x87" (DESIGN.md section 5).
    [JsonPropertyName("floatMode")] public string FloatMode { get; set; } = "f32";
    /// "jit32" only: float locals the original's JIT keeps IN MEMORY (rounded at every store) where the allocation
    /// rule says x87 stack. Key: `Type.Method:local` (the decompiler's local name), value: the evidence (which
    /// comparison with the real game shows it). The rule works on the decompiler's locals; the JIT allocates the IL
    /// locals, which the C# compiler reuses for several source variables: such a local can have more references and
    /// calls across its lifetime than the decompiled variable shows (FLOAT_RULE.md section 3).
    [JsonPropertyName("floatInMemory")] public Dictionary<string, string> FloatInMemory { get; set; } = new();
    /// "jit32" only: float locals on the x87 stack that the original rounds at an array creation (`new T[n] {...}`)
    /// they are live across: the JIT's allocation helper is a call instruction, and x87 values live across a call
    /// are spilled to 4-byte slots. Key `Type.Method:local`, value: the evidence (the machine code that shows it).
    /// Only these locals, only in these methods (FloatAlloc.cs); elsewhere an array creation is not a call.
    [JsonPropertyName("floatSpillAtAlloc")] public Dictionary<string, string> FloatSpillAtAlloc { get; set; } = new();
    /// Float locals (`Type.Method:local`) that the real JIT keeps on the x87 stack although the allocation rule puts
    /// them in memory, and spills to an 8-byte slot (`fstp qword`) around the calls they are live across: never
    /// rounded until stored elsewhere. Each entry names the machine code that shows it.
    [JsonPropertyName("floatStackAcrossCalls")] public Dictionary<string, string> FloatStackAcrossCalls { get; set; } = new();
    /// "jit32" only: the x87 evaluation of a library member's own body (documentation id -> Rust template with the
    /// placeholders of `nativeMembers`), read from the machine code the original's JIT made for it (Vector2.Length:
    /// `x*x + y*y` kept at double precision, `fsqrt`, then rounded to float). Used instead of the member's
    /// `nativeMembers` template at the call sites `jitBodySites` lists as call instructions; nowhere else.
    [JsonPropertyName("jitBodies")] public Dictionary<string, string> JitBodies { get; set; } = new();
    /// The call sites for `jitBodies` (written by gen_jit_bodies.py from a jitdump of the run whose results are
    /// compared): caller, callee (`ClrMethodKey`), IL call sites, call instructions. Path relative to the config.
    [JsonPropertyName("jitBodySites")] public string JitBodySites { get; set; }
    /// "jit32" only: per method with inlined XNA Vector2 operators, where the JIT kept their results (written by
    /// gen_jit_vec_temps.py from the reference runs' machine code): `method 	 stack|memory`. `stack`: the components
    /// of an operator chain stay on the x87 stack until stored; `memory`: each operator's components are rounded.
    /// Path relative to the config (Emit.cs VecInlined).
    [JsonPropertyName("jitVecTemps")] public string JitVecTemps { get; set; }
    /// "jit32" only: what the Vector2 operators of a game method became where a caller inlined it (written by
    /// gen_jit_vec_ctx.py): `caller 	 callee 	 k 	 op=c|i;...[|stack|memory]`. Such a call site calls a twin of the
    /// callee translated with those answers (`name__vcN`, Gen.cs; Emit.cs VecInlined). Path relative to the config.
    [JsonPropertyName("jitVecCtx")] public string JitVecCtx { get; set; }
    /// "jit32" only: the game methods with a float parameter that the real JIT inlined into a caller at every IL site
    /// (written by gen_jit_inlined.py from the reference runs' machine code): `caller 	 callee 	 sites`. The JIT
    /// substitutes the caller's local for a parameter the inlinee never writes, so a float local the caller keeps on
    /// the x87 stack enters the inlined body unrounded: such a site calls the callee's twin `name__u` (float parameters
    /// f64, Gen.cs; Emit.cs `InlinedArgsTwin`). FLOAT_RULE.md section 10, item 26. Path relative to the config.
    [JsonPropertyName("jitInlined")] public string JitInlined { get; set; }

    /// Directories whose `*.json` files are merged in as native tables (same schema), in ordinal file-name order.
    /// Unlike `include`, a member or type that two such files define, or that such a file and the including file
    /// (or its own includes) define, is an error naming both files. NATIVE_SCHEMA.md.
    [JsonPropertyName("includeDirs")] public List<string> IncludeDirs { get; set; } = new();
    /// Keys of native tables (exact, or a prefix ending in `*`) this config does not use: the member / type falls
    /// back to the host boundary. For entries of another owner's table that do not fit yet; each needs its reason.
    [JsonPropertyName("ignoreNative")] public List<string> IgnoreNative { get; set; } = new();

    /// Facts the reachability report (`--reach`) may assume: documentation id of a static bool field / property ->
    /// value (`"F:Terraria.Main.dedServ": true`). Branches they prove dead are still translated; only the report's
    /// call graph leaves them out. Never used for code generation.
    [JsonPropertyName("reachAssume")] public Dictionary<string, bool> ReachAssume { get; set; } = new();

    /// Types (full name, or `Namespace.*`) whose reflection data (`gen::meta`) includes instance fields and
    /// properties, together with every translated type their fields and properties mention. For the proofs, which
    /// read game state the way `tools\refdump` did, and for game code that reflects over instances.
    [JsonPropertyName("reflect")] public List<string> Reflect { get; set; } = new();
    /// Types whose reflection data is generated (static fields) without following the types of their fields.
    [JsonPropertyName("reflectShallow")] public List<string> ReflectShallow { get; set; } = new();
    /// Game types that reach Newtonsoft only as `object` (no generic call names them): member tables are generated
    /// for them and for the translated classes derived from them. Names or `Namespace.*`.
    [JsonPropertyName("jsonTypes")] public List<string> JsonTypes { get; set; } = new();
    /// Types (full name) whose reflection data lists their methods (`Type.GetMethods`, the attributes of a method,
    /// `Delegate.CreateDelegate`), together with every translated class derived from them.
    [JsonPropertyName("reflectMethods")] public List<string> ReflectMethods { get; set; } = new();
    /// Library members that do nothing the translated program can observe: documentation id -> reason. A call is
    /// counted (`rt::inert`) but is not a gap. Everything else that is unimplemented fails the proof tests.
    [JsonPropertyName("inert")] public Dictionary<string, string> Inert { get; set; } = new();
    /// Body of a generated host-boundary function: documentation id of the member (an accessor: `M:...get_X`) ->
    /// Rust expression over the function's parameter names (`this`, the C# parameter names, type parameters).
    /// For members of library INTERFACES that both translated types and runtime objects implement
    /// (`IEnumerator.MoveNext`, `IDisposable.Dispose`): the dispatcher in `gen::ifaces` tries the translated
    /// implementers by type id and falls through to this function, which a `nativeMembers` template cannot be.
    [JsonPropertyName("hostBodies")] public Dictionary<string, string> HostBodies { get; set; } = new();
    /// Body of a translated GAME method, replaced by hand-written Rust: documentation id of the method -> Rust
    /// expression over the generated function's parameter names (`this`, the C# parameter names); `{T0}`.. are the
    /// Rust types of the method's type arguments (one expansion per instantiation). Unlike a `nativeMembers` entry,
    /// which rewrites call sites, this keeps the function, so virtual / interface dispatch and method-group delegates
    /// reach the replacement too. Every entry needs its reason next to it in the config.
    [JsonPropertyName("bodies")] public Dictionary<string, string> Bodies { get; set; } = new();
    /// Methods that ALSO get a resumable form (DESIGN.md section 8f): `X__resumable`, an `async fn` whose state
    /// lives in one heap object instead of on a stack. The normal function is emitted unchanged.
    [JsonPropertyName("resumable")] public ResumableCfg Resumable { get; set; }
    /// Type tests against LIBRARY classes that have no Rust type of their own (a base class of several native
    /// classes, e.g. `GraphicsResource`): documentation id of the type -> Rust expression over `{0}` (the object as
    /// `Obj`, non-null) giving `bool`. Used by `is`, `as`, casts and type patterns; without an entry such a test of a
    /// non-null object stops (`host::is_instance`).
    [JsonPropertyName("typeTests")] public Dictionary<string, string> TypeTests { get; set; } = new();

    /// IL sizes of the original methods (written by gen_il_sizes.py from the reference database), for float mode
    /// "jit32": path relative to the config.
    [JsonPropertyName("ilSizes")] public string IlSizes { get; set; }
    /// Which calls the real 32-bit JIT compiled as call instructions and which it inlined, per caller (written by
    /// gen_jit_calls.py from a whole-game `jitdump`), for float mode "jit32": path relative to the config.
    [JsonPropertyName("jitCalls")] public string JitCalls { get; set; }
    /// Which calls the real 32-bit JIT compiled as call instructions and which it inlined, per caller, in the
    /// reference run whose results a gate compares (written by gen_jit_refcalls.py from that run's own dump, game
    /// callees included; only callers that ran there), for float mode "jit32": path relative to the config. For the
    /// pairs it lists it decides where x87 values are rounded, before `jitCalls` and the size rule.
    [JsonPropertyName("jitRefCalls")] public string JitRefCalls { get; set; }
    /// The sites of the `jitRefCalls` pairs with a call instruction at some sites only (gen_jit_refcalls.py:
    /// `caller 	 callee 	 file 	 start line 	 start column 	 end line 	 end column 	 ordinal 	 call`, the
    /// decompiled statement of each IL site through ILSpy's PDB): decides those sites one by one (FloatAlloc.CallKind).
    [JsonPropertyName("jitRefCallSites")] public string JitRefCallSites { get; set; }
    /// Where the real JIT kept each float local of the methods that ran in the reference runs (gen_jit_float_alloc.py
    /// from the CLR's own variable locations): `method 	 local 	 memory|stack|none|conflict|-`. Replaces the
    /// allocation rule for those methods (FloatAlloc.cs); a float local of such a method with no row is an IL
    /// evaluation-stack value (never rounded).
    [JsonPropertyName("jitFloatAlloc")] public string JitFloatAlloc { get; set; }
    /// The types of the game's executable in metadata order (written by gen_il_sizes.py), for
    /// `gen::types::STATIC_INITS` (`Assembly.GetTypes()` order): path relative to the config.
    [JsonPropertyName("typeOrder")] public string TypeOrder { get; set; }
    /// Types left out of `gen::types::STATIC_INITS` (CLR name -> reason): their static initialisers are not run by
    /// the force-load at start-up. They still run on first use, as every static initialiser does, so leaving one out
    /// changes WHEN it runs (and makes a stop inside it surface at the first real use), never whether.
    [JsonPropertyName("forceInitSkip")] public Dictionary<string, string> ForceInitSkip { get; set; } = new();

    public string BaseDir;
    public static string SkipNativePrefix;
    /// Native types removed by `ignoreNative`: a member template whose signature mentions one is not used either
    /// (it was written for the native Rust type).
    public readonly HashSet<string> IgnoredTypes = new(StringComparer.Ordinal);
    /// File that supplied each native type / member / slice / stub (for the duplicate report).
    public readonly Dictionary<string, string> Origin = new(StringComparer.Ordinal);

    static readonly JsonSerializerOptions Opts = new() { ReadCommentHandling = JsonCommentHandling.Skip, AllowTrailingCommas = true };

    public static Config Load(string path)
    {
        path = Path.GetFullPath(path);
        Config cfg;
        try { cfg = JsonSerializer.Deserialize<Config>(File.ReadAllText(path), Opts); }
        catch (JsonException e) { throw new ConfigException($"{path}: {e.Message}"); }
        cfg.BaseDir = Path.GetDirectoryName(path);
        foreach (var k in cfg.NativeTypes.Keys) cfg.Origin["T|" + k] = path;
        foreach (var k in cfg.NativeMembers.Keys) cfg.Origin["M|" + k] = path;
        foreach (var k in cfg.NativeSlices.Keys) cfg.Origin["S|" + k] = path;
        foreach (var k in cfg.Stubs.Keys) cfg.Origin["U|" + k] = path;
        // `include`: the including file wins (a config may override the shared table on purpose)
        foreach (var inc in cfg.Include)
        {
            var sub = Load(Path.Combine(cfg.BaseDir, inc));
            foreach (var kv in sub.NativeTypes) if (cfg.NativeTypes.TryAdd(kv.Key, kv.Value)) cfg.Origin["T|" + kv.Key] = sub.Origin["T|" + kv.Key];
            foreach (var kv in sub.NativeMembers) if (cfg.NativeMembers.TryAdd(kv.Key, kv.Value)) cfg.Origin["M|" + kv.Key] = sub.Origin["M|" + kv.Key];
            foreach (var kv in sub.NativeSlices) if (cfg.NativeSlices.TryAdd(kv.Key, kv.Value)) cfg.Origin["S|" + kv.Key] = sub.Origin["S|" + kv.Key];
            foreach (var kv in sub.Stubs) if (cfg.Stubs.TryAdd(kv.Key, kv.Value)) cfg.Origin["U|" + kv.Key] = sub.Origin["U|" + kv.Key];
            foreach (var t in sub.Types) if (!cfg.Types.Any(x => x.Type == t.Type)) cfg.Types.Add(t);
            foreach (var n in sub.Namespaces) if (!cfg.Namespaces.Contains(n)) cfg.Namespaces.Add(n);
            foreach (var n in sub.Exclude) if (!cfg.Exclude.Contains(n)) cfg.Exclude.Add(n);
            foreach (var kv in sub.Inert) cfg.Inert.TryAdd(kv.Key, kv.Value);
            foreach (var kv in sub.HostBodies) cfg.HostBodies.TryAdd(kv.Key, kv.Value);
            foreach (var kv in sub.Bodies) cfg.Bodies.TryAdd(kv.Key, kv.Value);
            foreach (var kv in sub.TypeTests) cfg.TypeTests.TryAdd(kv.Key, kv.Value);
            if (sub.AutoStructs) cfg.AutoStructs = true;
            if (cfg.Sources.Count == 0) cfg.Sources = sub.Sources.Select(s => Path.GetFullPath(Path.Combine(sub.BaseDir, s))).ToList();
            if (cfg.References.Count == 0) cfg.References = sub.References.Select(s => Path.GetFullPath(Path.Combine(sub.BaseDir, s))).ToList();
        }
        // `includeDirs`: native tables of other owners (native.d\*.json). Nothing may be defined twice.
        foreach (var dir in cfg.IncludeDirs)
        {
            var full = Path.GetFullPath(Path.Combine(cfg.BaseDir, dir));
            if (!Directory.Exists(full)) continue;
            foreach (var file in Directory.GetFiles(full, "*.json").OrderBy(f => Path.GetFileName(f), StringComparer.Ordinal))
            {
                // `--skip-native host_`: the snippets and the core set build without the host crate's tables
                if (SkipNativePrefix != null && Path.GetFileName(file).StartsWith(SkipNativePrefix, StringComparison.Ordinal)) continue;
                var sub = Load(file);
                if (sub.Types.Count + sub.Namespaces.Count + sub.Exclude.Count + sub.Sources.Count + sub.References.Count + sub.Include.Count + sub.IncludeDirs.Count > 0 || sub.AutoStructs)
                    throw new ConfigException($"{file}: a native table may only contain nativeTypes, nativeMembers, nativeSlices and stubs");
                void Merge<V>(Dictionary<string, V> into, Dictionary<string, V> from, string tag, string what)
                {
                    foreach (var kv in from.OrderBy(k => k.Key, StringComparer.Ordinal))
                    {
                        if (!into.TryAdd(kv.Key, kv.Value))
                            throw new ConfigException($"duplicate {what} {kv.Key}: defined in {cfg.Origin[tag + kv.Key]} and in {file}");
                        cfg.Origin[tag + kv.Key] = file;
                    }
                }
                Merge(cfg.NativeTypes, sub.NativeTypes, "T|", "native type");
                Merge(cfg.NativeMembers, sub.NativeMembers, "M|", "native member");
                Merge(cfg.NativeSlices, sub.NativeSlices, "S|", "native slice");
                Merge(cfg.Stubs, sub.Stubs, "U|", "stub");
            }
        }
        if (cfg.IgnoreNative.Count > 0)
        {
            bool Ignored(string k) => cfg.IgnoreNative.Any(x => x == k || (x.EndsWith("*") && k.StartsWith(x.Substring(0, x.Length - 1), StringComparison.Ordinal)));
            foreach (var k in cfg.NativeTypes.Keys.Where(Ignored).ToList()) { cfg.NativeTypes.Remove(k); cfg.IgnoredTypes.Add(k); }
            foreach (var k in cfg.NativeMembers.Keys.Where(Ignored).ToList()) cfg.NativeMembers.Remove(k);
        }
        return cfg;
    }
}

/// Config "resumable" (DESIGN.md section 8f).
public sealed class ResumableCfg
{
    /// Documentation ids of static methods with no `ref` / `out` parameter. Inside their resumable forms a call of
    /// another listed method awaits that method's resumable form.
    [JsonPropertyName("methods")] public List<string> Methods { get; set; } = new();
    /// Rust path of `fn(i32) -> impl Future<Output = ()>`: `Thread.Sleep(ms)` inside a resumable form.
    [JsonPropertyName("sleep")] public string Sleep { get; set; }
    /// Rust path of `fn<R, F: Future<Output = rt::Flow<R>>>(F) -> impl Future<Output = Result<rt::Flow<R>, Gc<Exception>>>`:
    /// the `try` of a resumable form (`rt::try_` around every poll of the body).
    [JsonPropertyName("try")] public string Try { get; set; }
    /// Library members that WAIT (documentation id -> native template, `nativeMembers` syntax, ending in `.await`):
    /// inside a resumable form a call of such a member uses this template instead of its `nativeMembers` one, so
    /// the machine parks while it waits (`TcpListener.AcceptTcpClient` in `TcpSocket.ListenLoop`).
    [JsonPropertyName("await")] public Dictionary<string, string> Await { get; set; } = new();
}

public sealed class ConfigException : Exception { public ConfigException(string m) : base(m) { } }

public static class Names
{
    static readonly HashSet<string> Keywords = new(StringComparer.Ordinal) {
        "as","break","const","continue","else","enum","extern","false","fn","for","if","impl","in","let","loop","match","mod","move",
        "mut","pub","ref","return","static","struct","trait","true","type","unsafe","use","where","while","async",
        "await","dyn","abstract","become","box","do","final","macro","override","priv","typeof","unsized","virtual","yield","try","gen",
        "self","Self","super","crate" };
    static readonly Regex R1 = new("(?<=[a-z0-9])(?=[A-Z])", RegexOptions.Compiled);
    static readonly Regex R2 = new("(?<=[A-Z])(?=[A-Z][a-z])", RegexOptions.Compiled);
    static readonly Regex Bad = new("[^A-Za-z0-9_]", RegexOptions.Compiled);

    /// The port-wide rule: `snake()` of tools\gen_tables.py.
    public static string Snake(string name)
    {
        var s = R1.Replace(name, "_");
        s = R2.Replace(s, "_");
        return s.ToLowerInvariant();
    }
    public static string Ident(string name)
    {
        name = Bad.Replace(name, "_");
        if (name.Length == 0 || char.IsDigit(name[0])) name = "_" + name;
        return Keywords.Contains(name) ? name + "_" : name;
    }
    public static string SnakeIdent(string name) => Ident(Snake(name));
    public static string Upper(string name) => Ident(Snake(name).ToUpperInvariant());
}

public enum MStatus { Clean, Unported, Failed, NotEmitted }

public sealed class MethodInfo2
{
    public IMethodSymbol Sym;
    /// For one instantiation of a generic method: the uninstantiated member (`Sym` is constructed from it) and the
    /// type arguments. Null / default for ordinary methods.
    public IMethodSymbol Open;
    public System.Collections.Immutable.ImmutableArray<ITypeSymbol> TypeArgs;
    public string Name;          // rust fn name
    public bool Emittable = true;
    public string NotEmittedReason;
    public bool NeedsDispatcher; // virtual root inside translated types
}

public sealed class FieldInfo2
{
    /// The field; null for the delegate field behind a field-like event (`Event` is set instead: Roslyn does not
    /// list that compiler-generated field among the members).
    public IFieldSymbol Sym;
    public IEventSymbol Event;
    public string DeclName => Sym?.Name ?? Event.Name;
    public ITypeSymbol DeclType => Sym?.Type ?? Event.Type;
    public string Name;
    public string RustType;      // null: not representable
    /// `[FieldOffset(n)]` of a field of a `[StructLayout(LayoutKind.Explicit)]` struct, else -1.
    public int Offset = -1;
    /// Bytes the field occupies in an explicit-layout struct (0 otherwise).
    public int Size;
    /// An explicit-layout field that shares its bytes with fields declared before it (a C# union member): it is
    /// not a field of the Rust struct; its place is a pointer cast at `Offset` (DESIGN.md section 1).
    public bool Overlay;
}

/// One translated C# type.
public sealed class TType
{
    public INamedTypeSymbol Sym;
    public TypeCfg Cfg;
    public List<string> Module;   // path below the gen module: terraria, item
    public string ModPath;        // terraria::item
    public string StructName;     // Item
    public string StructPath;     // terraria::item::Item
    public int TypeId;            // 0 for structs / static classes
    public TType Base;
    /// The class derives directly from a natively implemented class (`ClusterGroup : List<Cluster>`): the native
    /// value is embedded as the first field (`rt::NativeSlot`). NATIVE_SCHEMA.md section 3a.
    public INamedTypeSymbol NativeBase;
    /// The class derives (directly, or through untranslated library classes) from System.Exception: its first
    /// field is the runtime's `Exception` and `ExcKind` is the nearest library exception type.
    public string ExcKind;
    public bool IsStruct, IsStatic, IsInterface;
    public bool IsClass => !IsStruct && !IsStatic && !IsInterface;
    public bool HasExplicitCctor;
    /// The module has a `Statics` struct and `s()` (set when the module is emitted).
    public bool HasStatics;
    public List<FieldInfo2> InstFields = new();
    /// Explicit layout (`[StructLayout(LayoutKind.Explicit)]` with primitive fields): size and alignment in bytes;
    /// 0 for every other type. `InstFields` then holds the fields that own their bytes, in offset order.
    public int ExplicitSize, ExplicitAlign;
    public List<FieldInfo2> StaticFields = new();
    public List<FieldInfo2> Consts = new();
    public List<MethodInfo2> Methods = new();
    public Dictionary<ISymbol, FieldInfo2> FieldMap = new(SymbolEqualityComparer.Default);
    public Dictionary<ISymbol, MethodInfo2> MethodMap = new(SymbolEqualityComparer.Default);
    /// Instantiations of the type's generic methods, by method definition (Mono.cs).
    public Dictionary<ISymbol, List<MethodInfo2>> GenericMap = new(SymbolEqualityComparer.Default);
    /// Generic methods no call site instantiates (no Rust function).
    public List<IMethodSymbol> Uninstantiated = new();
    /// Substitution of the type's own type parameters (an instantiation of a generic class), or null.
    public Dictionary<ITypeParameterSymbol, ITypeSymbol> Map;
    public string WholeTypeReason; // set when the type cannot be emitted at all
    public volatile bool Populated;
    public bool Included(ISymbol member)
    {
        if (Cfg?.Members == null) return true;
        var n = member is IMethodSymbol ms && ms.AssociatedSymbol != null ? ms.AssociatedSymbol.Name : member.Name;
        if (member is IFieldSymbol fs && fs.AssociatedSymbol != null) n = fs.AssociatedSymbol.Name;
        return Cfg.Members.Contains(n);
    }
}

public sealed class Model
{
    public readonly Config Cfg;
    public readonly CSharpCompilation Comp;
    /// Dry-run: treat every type declared in source as translated (measures the emitter, not the config).
    public readonly bool AssumeAll;
    /// Translated types: non-generic definitions, and closed instantiations of generic definitions (constructed symbols).
    public readonly Dictionary<INamedTypeSymbol, TType> Types = new(SymbolEqualityComparer.Default);
    /// Generic type definitions selected by the config: emitted once per closed instantiation (Mono.cs).
    public readonly HashSet<INamedTypeSymbol> GenericDefs = new(SymbolEqualityComparer.Default);
    /// Instantiations of generic methods by containing (closed) type, in key order.
    public Dictionary<INamedTypeSymbol, List<MethodInst>> MethodInsts = new(SymbolEqualityComparer.Default);
    readonly HashSet<string> _modPaths = new(StringComparer.Ordinal);
    public readonly List<TType> Ordered = new();
    readonly ConcurrentDictionary<ISymbol, string> _docIds = new(SymbolEqualityComparer.Default);
    public const int FirstTypeId = 64;
    public readonly INamedTypeSymbol ExceptionType;
    public readonly HostApi Host;

    public Model(Config cfg, CSharpCompilation comp, bool assumeAll)
    {
        Cfg = cfg; Comp = comp; AssumeAll = assumeAll; Mono.Comp = comp;
        ExceptionType = comp.GetTypeByMetadataName("System.Exception");
        Host = new HostApi(this);
    }

    public string DocId(ISymbol s) => _docIds.GetOrAdd(s, x => x.GetDocumentationCommentId() ?? "");

    /// Stub value from the config: exact documentation id, or `M:Type.Name(*)` for every overload.
    public string Stub(ISymbol s)
    {
        var id = DocId(s);
        if (Cfg.Stubs.TryGetValue(id, out var v)) return v;
        int p = id.IndexOf('(');
        return Cfg.Stubs.TryGetValue((p < 0 ? id : id.Substring(0, p)) + "(*)", out v) ? v : null;
    }

    // ------------------------------------------------------------------ translated types
    public TType Get(ITypeSymbol t)
    {
        // a closed instantiation of a generic class is a type of its own; inside generic code the ambient
        // substitution (Mono.Enter) closes the type first
        if (Mono.Sub(t) is not INamedTypeSymbol n) return null;
        return Types.TryGetValue(n, out var tt) ? tt : null;
    }

    /// Member tables are built on first use (dry-run creates types lazily).
    public TType Pop(TType t)
    {
        if (t.Populated) return t;
        lock (_popLock) { if (!t.Populated) { Populate(t); t.Populated = true; } }
        return t;
    }
    readonly object _popLock = new();
    public FieldInfo2 Field(TType t, ISymbol f) => Pop(t).FieldMap.TryGetValue(f.OriginalDefinition, out var r) ? r : null;
    /// The Rust function of method `m` of `t`. For a generic method `m` must carry its type arguments (they may be
    /// written with the type parameters of the code being emitted: the ambient substitution closes them).
    /// The Rust function of `open` (a member of the closed type `t`, not constructed) for the type arguments `args`
    /// (default / empty: an ordinary method).
    public MethodInfo2 MethodInst(TType t, IMethodSymbol open, System.Collections.Immutable.ImmutableArray<ITypeSymbol> args)
    {
        Pop(t);
        if (args.IsDefaultOrEmpty || !open.IsGenericMethod) return t.MethodMap.TryGetValue(open.OriginalDefinition, out var r) ? r : null;
        if (!t.GenericMap.TryGetValue(open.OriginalDefinition, out var list)) return null;
        foreach (var x in list)
            if (x.TypeArgs.SequenceEqual(args, (IEqualityComparer<ITypeSymbol>)SymbolEqualityComparer.Default)) return x;
        return null;
    }
    public MethodInfo2 Method(TType t, ISymbol m)
    {
        Pop(t);
        if (m is IMethodSymbol { IsGenericMethod: true } gm)
        {
            gm = gm.ReducedFrom ?? gm;
            if (!t.GenericMap.TryGetValue(gm.OriginalDefinition, out var list)) return null;
            var args = gm.TypeArguments.Select(Mono.Sub).ToList();
            foreach (var x in list)
                if (x.TypeArgs.SequenceEqual(args, (IEqualityComparer<ITypeSymbol>)SymbolEqualityComparer.Default)) return x;
            return null;
        }
        return t.MethodMap.TryGetValue(m.OriginalDefinition, out var r) ? r : null;
    }

    public TType MakeType(INamedTypeSymbol sym, TypeCfg cfg)
    {
        var t = new TType { Sym = sym, Cfg = cfg };
        var mod = new List<string>();
        var chain = new List<INamedTypeSymbol>();
        for (var c = sym; c != null; c = c.ContainingType) chain.Insert(0, c);
        // one module per type; an instantiation of a generic class is named by its type arguments (`ref__int`)
        static string Seg(INamedTypeSymbol c) => c.TypeArguments.Length > 0 && Mono.IsClosed(c)
            ? Names.Ident(Names.Snake(c.Name) + "__" + string.Join("_", c.TypeArguments.Select(Spell)))
            : Names.SnakeIdent(c.Name + (c.Arity > 0 ? "_" + c.Arity : ""));
        if (sym.ContainingType != null && Types.TryGetValue(sym.ContainingType, out var outerT))
        {
            mod.AddRange(outerT.Module); mod.Add(Seg(sym));
        }
        else
        {
            for (var ns = sym.ContainingNamespace; ns != null && !ns.IsGlobalNamespace; ns = ns.ContainingNamespace) mod.Insert(0, Names.SnakeIdent(ns.Name));
            foreach (var c in chain) mod.Add(Seg(c));
        }
        // two instantiations whose arguments spell alike (same simple name in two namespaces): number the later one
        { var b = mod[^1]; int k = 2; while (!_modPaths.Add(string.Join("::", mod))) mod[^1] = b + "_" + k++; }
        t.Module = mod;
        t.ModPath = string.Join("::", mod);
        t.StructName = Names.Ident(sym.Name);
        t.StructPath = t.ModPath + "::" + t.StructName;
        t.IsStruct = sym.TypeKind == TypeKind.Struct;
        t.IsInterface = sym.TypeKind == TypeKind.Interface;
        t.IsStatic = sym.IsStatic;
        if (sym.IsGenericType)
        {
            if (!Mono.IsClosed(sym)) t.WholeTypeReason = "generic type definition (emitted per closed instantiation)";
            else t.Map = Mono.MapOf(sym);
        }
        return t;
    }

    /// The instantiations Discovery found become translated types (after the configured ones, in key order).
    public void AddInstances(Discovery d)
    {
        _enums.AddRange(d.Enums.Keys);
        foreach (var n in d.TypeInsts.Keys.OrderBy(Mono.Key, StringComparer.Ordinal))
            AddConfigured(n, new TypeCfg { Type = Mono.Key(n) });
        foreach (var mi in d.MethodInsts.Values.OrderBy(x => x.Key, StringComparer.Ordinal))
        {
            if (!MethodInsts.TryGetValue(mi.Type, out var l)) MethodInsts[mi.Type] = l = new List<MethodInst>();
            l.Add(mi);
        }
        // selected generic definitions nothing instantiates: reported, not emitted
        foreach (var g in GenericDefs.OrderBy(Mono.Key, StringComparer.Ordinal))
            if (!d.TypeInsts.Keys.Any(k => SymbolEqualityComparer.Default.Equals(k.OriginalDefinition, g)))
            {
                var t = new TType { Sym = g, Cfg = null, Module = new List<string>(), ModPath = "", StructName = Names.Ident(g.Name), StructPath = "" };
                t.IsStruct = g.TypeKind == TypeKind.Struct; t.IsInterface = g.TypeKind == TypeKind.Interface; t.IsStatic = g.IsStatic;
                t.WholeTypeReason = "generic type with no closed instantiation in the program";
                Unused.Add(t);
            }
    }
    public readonly List<TType> Unused = new();

    public void AddConfigured(INamedTypeSymbol sym, TypeCfg cfg)
    {
        var t = MakeType(sym, cfg);
        Types[sym] = t;
        Ordered.Add(t);
    }

    public void PopulateAll()
    {
        // ids below 64 belong to the runtime (rt::tid: boxed primitives, strings, XNA structs)
        int id = FirstTypeId;
        foreach (var t in Ordered)
            if (!t.IsStatic && !t.IsInterface && t.WholeTypeReason == null) t.TypeId = id++;
        // enums box with an id of their own, after the classes and structs (sorted: independent of discovery order)
        foreach (var e in _enums.OrderBy(Mono.Key, StringComparer.Ordinal)) EnumIds[e] = id++;
        foreach (var t in Ordered) Pop(t);
    }
    readonly List<INamedTypeSymbol> _enums = new();
    public readonly Dictionary<INamedTypeSymbol, int> EnumIds = new(SymbolEqualityComparer.Default);

    static bool IsAutoProp(IPropertySymbol p)
    {
        foreach (var r in p.DeclaringSyntaxReferences)
            if (r.GetSyntax() is PropertyDeclarationSyntax pd)
                return pd.ExpressionBody == null && pd.AccessorList != null && pd.AccessorList.Accessors.All(a => a.Body == null && a.ExpressionBody == null)
                    && !p.IsAbstract && p.ContainingType.TypeKind != TypeKind.Interface && !p.IsExtern;
        return false;
    }

    void Populate(TType t)
    {
        var sym = t.Sym;
        if (sym.BaseType != null && sym.BaseType.SpecialType != SpecialType.System_Object && sym.TypeKind == TypeKind.Class)
        {
            t.Base = Get(sym.BaseType);
            if (t.Base != null && t.Base.WholeTypeReason != null) t.Base = null;
            if (t.Base == null && IsBclException(sym.BaseType)) t.ExcKind = FullName(sym.BaseType);
            if (t.Base == null && t.ExcKind == null && Native(sym.BaseType) is { Kind: "class" } && RT(sym.BaseType) != null) t.NativeBase = sym.BaseType;
        }
        var usedF = new HashSet<string>(); var usedS = new HashSet<string>(); var usedC = new HashSet<string>();
        string Uniq(HashSet<string> used, string n) { var b = n; int k = 2; while (!used.Add(n)) n = b + "_" + k++; return n; }
        foreach (var m in sym.GetMembers())
        {
            if (m is IFieldSymbol f)
            {
                if (!t.Included(f)) continue;
                string baseName = f.AssociatedSymbol is IPropertySymbol ap ? Names.Snake(ap.Name) + "__backing"
                    : f.AssociatedSymbol is IEventSymbol ev ? Names.Snake(ev.Name) + "__event" : Names.SnakeIdent(f.Name);
                var fi = new FieldInfo2 { Sym = f, RustType = RT(f.Type) };
                if (f.IsConst) { fi.Name = Uniq(usedC, Names.Upper(f.Name)); t.Consts.Add(fi); }
                else if (f.IsStatic) { fi.Name = Uniq(usedS, baseName); t.StaticFields.Add(fi); }
                else { fi.Name = Uniq(usedF, baseName == "base" ? "base_" : baseName); t.InstFields.Add(fi); }
                t.FieldMap[f.OriginalDefinition] = fi;
            }
        }
        foreach (var ev in sym.GetMembers().OfType<IEventSymbol>())
        {
            // field-like event (`public event Action X;`): one delegate field, `+=` / `-=` combine into it
            if (!t.Included(ev) || ev.IsAbstract || sym.TypeKind == TypeKind.Interface) continue;
            if (ev.AddMethod == null || !ev.AddMethod.IsImplicitlyDeclared) continue;
            var fi = new FieldInfo2 { Event = ev, RustType = RT(ev.Type) };
            if (fi.RustType != null && !fi.RustType.StartsWith("Delegate<")) fi.RustType = null;
            if (ev.IsStatic) { fi.Name = Uniq(usedS, Names.Snake(ev.Name) + "__event"); t.StaticFields.Add(fi); }
            else { fi.Name = Uniq(usedF, Names.Snake(ev.Name) + "__event"); t.InstFields.Add(fi); }
            t.FieldMap[ev.OriginalDefinition] = fi;
        }
        ApplyExplicitLayout(t, sym);
        // methods: name groups
        var list = new List<MethodInfo2>();
        foreach (var m in sym.GetMembers())
        {
            if (m is not IMethodSymbol ms) continue;
            if (ms.MethodKind == MethodKind.StaticConstructor) { t.HasExplicitCctor = !ms.IsImplicitlyDeclared; continue; }
            if (!t.Included(ms) && ms.MethodKind != MethodKind.Constructor) continue;
            var mi = new MethodInfo2 { Sym = ms };
            switch (ms.MethodKind)
            {
                case MethodKind.Ordinary: case MethodKind.Constructor: case MethodKind.PropertyGet: case MethodKind.PropertySet:
                case MethodKind.UserDefinedOperator: case MethodKind.Conversion: case MethodKind.ExplicitInterfaceImplementation: break;
                case MethodKind.EventAdd: case MethodKind.EventRemove: break;
                case MethodKind.Destructor: mi.Emittable = false; mi.NotEmittedReason = "finalizer"; break;
                default: continue;
            }
            if (ms.MethodKind == MethodKind.Constructor && ms.IsImplicitlyDeclared && (t.IsStruct || t.IsStatic)) continue;
            if (ms.IsGenericMethod)
            {
                // one Rust function per instantiation the program uses (Mono.cs); none: no function at all
                var insts = MethodInsts.TryGetValue(sym, out var all) ? all.Where(x => SymbolEqualityComparer.Default.Equals(x.Def, ms.OriginalDefinition)).ToList() : new List<MethodInst>();
                if (insts.Count == 0) t.Uninstantiated.Add(ms);
                foreach (var inst in insts)
                    list.Add(new MethodInfo2 { Sym = ms.Construct(inst.Args.ToArray()), Open = ms, TypeArgs = inst.Args, Emittable = mi.Emittable, NotEmittedReason = mi.NotEmittedReason });
                continue;
            }
            list.Add(mi);
        }
        var reserved = new HashSet<string> { "s", "cctor", "new", "init_fields", "reflect_fields", "reflect_prop" };
        string BaseName(IMethodSymbol ms)
        {
            string n = ms.MethodKind switch
            {
                MethodKind.Constructor => "ctor",
                MethodKind.PropertyGet => ms.AssociatedSymbol is IPropertySymbol { IsIndexer: true } ? "get_item" : Names.Snake(ms.AssociatedSymbol.Name.Split('.').Last()),
                MethodKind.PropertySet => ms.AssociatedSymbol is IPropertySymbol { IsIndexer: true } ? "set_item" : "set_" + Names.Snake(ms.AssociatedSymbol.Name.Split('.').Last()),
                _ => Names.Snake(ms.Name.Split('.').Last()),
            };
            n = Names.Ident(n);
            if (reserved.Contains(n)) n += "_";
            return n;
        }
        var usedM = new HashSet<string>();
        foreach (var g in list.GroupBy(m => BaseName(m.Sym)).ToList())
        {
            bool many = g.Select(x => (ISymbol)(x.Open ?? x.Sym)).Distinct(SymbolEqualityComparer.Default).Count() > 1;
            foreach (var mi in g)
            {
                var n = g.Key;
                if (many)
                {
                    var ps = (mi.Open ?? mi.Sym).Parameters.Select(p => (p.RefKind == RefKind.Ref ? "ref_" : p.RefKind == RefKind.Out ? "out_" : "") + Spell(p.Type)).ToList();
                    if (mi.Sym.MethodKind == MethodKind.Conversion) ps.Add("to_" + Spell(mi.Sym.ReturnType));
                    n += "__" + (ps.Count == 0 ? "void" : string.Join("_", ps));
                }
                if (mi.Open != null) n += "__g_" + string.Join("_", mi.TypeArgs.Select(Spell));
                mi.Name = Uniq(usedM, Names.Ident(n));
            }
        }
        foreach (var mi in list)
        {
            var ms = mi.Sym;
            if (mi.Emittable)
            {
                if (t.WholeTypeReason != null) { mi.Emittable = false; mi.NotEmittedReason = t.WholeTypeReason; }
                else if (ms.IsExtern) { mi.Emittable = false; mi.NotEmittedReason = ms.GetDllImportData() != null ? "extern method (P/Invoke: calls go to the host boundary)" : "extern method"; }
                else if (ms.ReturnsByRef || ms.ReturnsByRefReadonly) { mi.Emittable = false; mi.NotEmittedReason = "ref return"; }
                else if (!ms.ReturnsVoid && RT(ms.ReturnType) == null) { mi.Emittable = false; mi.NotEmittedReason = "signature uses a type with no Rust representation: " + Short(ms.ReturnType); }
                else foreach (var p in ms.Parameters)
                    if (RT(p.Type) == null) { mi.Emittable = false; mi.NotEmittedReason = "signature uses a type with no Rust representation: " + Short(p.Type); break; }
            }
            t.Methods.Add(mi);
            if (mi.Open != null)
            {
                if (!t.GenericMap.TryGetValue(ms.OriginalDefinition, out var gl)) t.GenericMap[ms.OriginalDefinition] = gl = new List<MethodInfo2>();
                gl.Add(mi);
            }
            else t.MethodMap[ms.OriginalDefinition] = mi;
        }
    }

    public static string Short(ITypeSymbol t) => t.ToDisplayString(SymbolDisplayFormat.MinimallyQualifiedFormat);

    /// Spelling of a parameter type inside an overload suffix.
    public static string Spell(ITypeSymbol t)
    {
        switch (t.SpecialType)
        {
            case SpecialType.System_Boolean: return "bool"; case SpecialType.System_Byte: return "byte"; case SpecialType.System_SByte: return "sbyte";
            case SpecialType.System_Int16: return "short"; case SpecialType.System_UInt16: return "ushort"; case SpecialType.System_Int32: return "int";
            case SpecialType.System_UInt32: return "uint"; case SpecialType.System_Int64: return "long"; case SpecialType.System_UInt64: return "ulong";
            case SpecialType.System_Single: return "float"; case SpecialType.System_Double: return "double"; case SpecialType.System_Char: return "char";
            case SpecialType.System_String: return "string"; case SpecialType.System_Object: return "object"; case SpecialType.System_Decimal: return "decimal";
        }
        if (t is IArrayTypeSymbol a) return Spell(a.ElementType) + string.Concat(Enumerable.Repeat("_arr", a.Rank));
        if (t is INamedTypeSymbol n)
        {
            var s = Names.Snake(n.Name);
            if (n.TypeArguments.Length > 0) s += "_" + string.Join("_", n.TypeArguments.Select(Spell));
            return s;
        }
        if (t is IPointerTypeSymbol p) return Spell(p.PointedAtType) + "_ptr";
        return Names.Snake(t.Name);
    }

    // ------------------------------------------------------------------ type map
    readonly ConcurrentDictionary<ITypeSymbol, string> _rt = new(SymbolEqualityComparer.Default);
    public string RT(ITypeSymbol t)
    {
        t = Mono.Sub(t);
        if (t == null) return null;
        if (_rt.TryGetValue(t, out var r)) return r == "" ? null : r;
        r = RT0(t);
        _rt[t] = r ?? "";
        return r;
    }

    public static string Prim(SpecialType st) => st switch
    {
        SpecialType.System_Boolean => "bool", SpecialType.System_Byte => "u8", SpecialType.System_SByte => "i8",
        SpecialType.System_Int16 => "i16", SpecialType.System_UInt16 => "u16", SpecialType.System_Int32 => "i32",
        SpecialType.System_UInt32 => "u32", SpecialType.System_Int64 => "i64", SpecialType.System_UInt64 => "u64",
        SpecialType.System_Single => "f32", SpecialType.System_Double => "f64", SpecialType.System_Char => "u16",
        _ => null,
    };

    public NativeTypeCfg Native(ITypeSymbol t)
    {
        t = Mono.Sub(t);
        if (t is INamedTypeSymbol n && Cfg.NativeTypes.TryGetValue(DocId(n.OriginalDefinition), out var nt)) return nt;
        return null;
    }

    string RT0(ITypeSymbol t)
    {
        var p = Prim(t.SpecialType);
        if (p != null) return p;
        switch (t.SpecialType)
        {
            case SpecialType.System_String: return "Str";
            case SpecialType.System_Object: return "Obj";
            case SpecialType.System_Void: return "()";
            // System.Decimal: the runtime's 96-bit decimal (rt::decimal::Decimal; the game uses it in one place, the
            // progress numbers of the achievement list)
            case SpecialType.System_Decimal: return "rt::decimal::Decimal";
        }
        if (t is IArrayTypeSymbol a)
        {
            var e = RT(a.ElementType);
            if (e == null) return null;
            return a.Rank == 1 ? $"Arr<{e}>" : a.Rank == 2 ? $"Arr2<{e}>" : a.Rank == 3 ? $"Arr3<{e}>" : null;
        }
        if (t is ITypeParameterSymbol tp)
        {
            // A type parameter that reaches this point belongs to a LIBRARY member (the game's own generic code is
            // monomorphised, Mono.cs): it stays a Rust type parameter of the host function, whatever its constraint,
            // so `ConditionalWeakTable<K, V>.GetValue` returns the caller's V and not an erased `object`.
            return Names.Ident(tp.Name);
        }
        if (t is IPointerTypeSymbol ptr)
        {
            // `T*`: a raw pointer (Emit.cs "pointers"); `void*` is a byte address
            if (ptr.PointedAtType.SpecialType == SpecialType.System_Void) return "*mut u8";
            var e = RT(ptr.PointedAtType);
            return e == null ? null : $"*mut {e}";
        }
        if (t is not INamedTypeSymbol n) return null;
        if (n.TypeKind == TypeKind.Enum) return Prim(n.EnumUnderlyingType.SpecialType);
        if (n.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T)
        {
            var e = RT(n.TypeArguments[0]);
            return e == null ? null : $"Nullable<{e}>";
        }
        // an anonymous type: an object of the runtime (`rt::AnonObj`: property names + boxed values)
        if (n.IsAnonymousType) return "Obj";
        var nt = Native(n);
        if (nt != null)
        {
            var s = nt.Rust;
            // {0}, {1} ...: type arguments of the containing types, then of the type (Dictionary<K, V>.KeyCollection: K, V)
            var targs = new List<ITypeSymbol>();
            var chain = new List<INamedTypeSymbol>();
            for (var c = n; c != null; c = c.ContainingType) chain.Insert(0, c);
            foreach (var c in chain) targs.AddRange(c.TypeArguments);
            for (int i = 0; i < targs.Count; i++)
            {
                if (!s.Contains("{" + i + "}")) continue;
                var e = RT(targs[i]);
                if (e == null) return null;
                s = s.Replace("{" + i + "}", e);
            }
            return nt.Kind == "class" ? $"Gc<{s}>" : s;
        }
        if (n.TypeKind == TypeKind.Delegate)
        {
            var inv = n.DelegateInvokeMethod;
            if (inv == null) return "Obj";
            var ps = new List<string>();
            foreach (var q in inv.Parameters)
            {
                var e = RT(q.Type);
                if (e == null) return "Obj";
                ps.Add(q.RefKind is RefKind.Ref or RefKind.Out ? $"*mut {e}" : e);
            }
            var ret = inv.ReturnsVoid ? "" : RT(inv.ReturnType);
            if (ret == null) return "Obj";
            return $"Delegate<dyn Fn({string.Join(", ", ps)}){(ret == "" ? "" : " -> " + ret)}>";
        }
        var tt = Get(n);
        if (tt != null && tt.WholeTypeReason == null)
        {
            if (tt.IsStruct) return tt.StructPath;
            if (tt.IsInterface || tt.IsStatic) return "Obj";
            return $"Gc<{tt.StructPath}>";
        }
        if (IsBclException(n)) return "Gc<Exception>";
        if (n.TypeKind is TypeKind.Class or TypeKind.Interface) return "Obj";
        // library value type with no mapping: opaque struct declared by the host boundary
        if (n.TypeKind == TypeKind.Struct && !n.IsGenericType && IsLibrary(n) && n.ContainingType?.IsGenericType != true)
            return Host.StructType(n, Cfg.HostModule);
        return null; // value type that is neither translated nor mapped
    }

    public static bool IsLibrary(ISymbol s) => s.Locations.All(l => !l.IsInSource);

    /// The native class a translated class derives from (directly or through translated bases), or null.
    public INamedTypeSymbol NativeBaseOf(TType t)
    {
        for (var c = t; c != null; c = Pop(c).Base) if (Pop(c).NativeBase != null) return c.NativeBase;
        return null;
    }

    // ---- reflection data with instance members (config "reflect")
    HashSet<INamedTypeSymbol> _reflect; List<TType> _reflectForce;
    static bool NameMatches(List<string> pats, string full) => pats.Any(x => x == full || (x.EndsWith(".*") && full.StartsWith(x.Substring(0, x.Length - 1), StringComparison.Ordinal)));
    void ComputeReflect()
    {
        lock (_popLock)
        {
            if (_reflect != null) return;
            var set = new HashSet<INamedTypeSymbol>(SymbolEqualityComparer.Default); var force = new List<TType>();
            var q = new Queue<TType>();
            void Add(ITypeSymbol s)
            {
                while (s is IArrayTypeSymbol a) s = a.ElementType;
                if (s is not INamedTypeSymbol n) return;
                if (n.TypeKind == TypeKind.Delegate) return;
                foreach (var ta in n.TypeArguments) Add(ta);
                if (Types.TryGetValue(n, out var tt) && tt.WholeTypeReason == null && !tt.IsInterface && set.Add(tt.Sym)) { q.Enqueue(tt); force.Add(tt); }
            }
            var roots = new HashSet<TType>();
            foreach (var t in Ordered)
            {
                if (t.WholeTypeReason != null || t.IsInterface) continue;
                var full = FullName(t.Sym);
                if (NameMatches(Cfg.Reflect, full)) { if (set.Add(t.Sym)) { q.Enqueue(t); force.Add(t); roots.Add(t); } }
                else if (NameMatches(Cfg.ReflectShallow, full) || ReflectMethods(t)) force.Add(t);
            }
            while (q.Count > 0)
            {
                var t = q.Dequeue();
                if (t.Sym.BaseType != null) Add(t.Sym.BaseType);
                bool anyPublic = false;
                foreach (var f in t.Sym.GetMembers().OfType<IFieldSymbol>())
                {
                    if (f.IsConst || f.IsImplicitlyDeclared) continue;
                    if (f.IsStatic) { if (f.DeclaredAccessibility == Accessibility.Public || roots.Contains(t)) Add(f.Type); continue; }
                    if (f.DeclaredAccessibility == Accessibility.Public) { anyPublic = true; Add(f.Type); }
                }
                foreach (var p in t.Sym.GetMembers().OfType<IPropertySymbol>())
                    if (!p.IsStatic && !p.IsIndexer && p.GetMethod != null && p.GetMethod.DeclaredAccessibility == Accessibility.Public) { anyPublic = true; Add(p.Type); }
                // a type without public data is read through its private fields (tools\refdump does the same)
                if (!anyPublic) foreach (var f in t.Sym.GetMembers().OfType<IFieldSymbol>()) if (!f.IsStatic && !f.IsImplicitlyDeclared) Add(f.Type);
            }
            _reflectForce = force; _reflect = set;
        }
    }
    /// Does the reflection data of `t` list its methods (the config's `reflectMethods`: the type or a base of it)?
    public bool ReflectMethods(TType t)
    {
        if (Cfg.ReflectMethods.Count == 0) return false;
        for (var c = t.Sym; c != null; c = c.BaseType)
            if (NameMatches(Cfg.ReflectMethods, FullName(c.OriginalDefinition))) return true;
        return false;
    }
    /// Size (= alignment) of a primitive Rust type, 0 for anything else.
    static int PrimSize(string rt) => rt switch
    {
        "bool" or "u8" or "i8" => 1,
        "u16" or "i16" => 2,
        "u32" or "i32" or "f32" => 4,
        "u64" or "i64" or "f64" => 8,
        _ => 0,
    };

    /// Size and alignment of a value of C# type `ft` (Rust type `rt`) inside a `#[repr(C)]` struct: a primitive, a
    /// pointer-sized value (IntPtr, a pointer, a handle), or a struct of such fields in declaration order (computed
    /// from the symbols, so it does not depend on the order the types are built in). (0, 0): not known.
    (int size, int align) LayoutOf(string rt, ITypeSymbol ft, int depth = 0)
    {
        if (rt == null) return (0, 0);
        int ps = PrimSize(rt);
        if (ps > 0) return (ps, ps);
        if (rt is "isize" or "usize" || rt.StartsWith("*mut ")) return (8, 8);
        if (depth < 8 && Mono.Sub(ft) is INamedTypeSymbol { TypeKind: TypeKind.Struct } n && n.SpecialType == SpecialType.None && !IsLibrary(n) && Native(n) == null
            && !n.GetAttributes().Any(a => a.AttributeClass?.Name == "StructLayoutAttribute" && a.ConstructorArguments.Length == 1 && (a.ConstructorArguments[0].Value is int k && k == 2 || a.ConstructorArguments[0].Value is short k2 && k2 == 2)))
        {
            int size = 0, align = 1, count = 0;
            foreach (var g in n.GetMembers().OfType<IFieldSymbol>())
            {
                if (g.IsStatic || g.IsConst) continue;
                var (s, a) = LayoutOf(RT(g.Type), g.Type, depth + 1);
                if (s == 0) return (0, 0);
                size = (size + a - 1) / a * a + s;
                align = Math.Max(align, a);
                count++;
            }
            return count == 0 ? (0, 0) : ((size + align - 1) / align * align, align);
        }
        return (0, 0);
    }

    /// `[StructLayout(LayoutKind.Explicit)]`: the fields get their `[FieldOffset]`; a field whose bytes overlap a
    /// field declared before it becomes an overlay. Structs whose fields are primitives, pointer-sized values or
    /// sequential structs of those, at naturally aligned offsets, are laid out this way (a union of structs:
    /// the Win32 INPUT union of ReLogic's IME code); any other explicit struct keeps the sequential layout and is
    /// named on stderr.
    void ApplyExplicitLayout(TType t, INamedTypeSymbol sym)
    {
        var sl = sym.GetAttributes().FirstOrDefault(a => a.AttributeClass?.Name == "StructLayoutAttribute");
        if (sl == null || sl.ConstructorArguments.Length != 1 || !(sl.ConstructorArguments[0].Value is int kind && kind == 2 || sl.ConstructorArguments[0].Value is short k2 && k2 == 2)) return;
        if (!sym.IsValueType || t.InstFields.Count == 0) return;
        var owners = new List<FieldInfo2>();
        int size = 0, align = 1;
        foreach (var f in t.InstFields)
        {
            var fo = f.Sym?.GetAttributes().FirstOrDefault(a => a.AttributeClass?.Name == "FieldOffsetAttribute");
            var (ps, pa) = f.Sym == null ? (0, 0) : LayoutOf(f.RustType, f.Sym.Type);
            if (fo == null || fo.ConstructorArguments.Length != 1 || fo.ConstructorArguments[0].Value is not int off || ps == 0 || off % pa != 0)
            {
                Console.Error.WriteLine($"explicit layout NOT honoured (a field that is not a naturally aligned primitive or struct of primitives): {FullName(sym)}.{f.DeclName}");
                foreach (var g in t.InstFields) { g.Offset = -1; g.Overlay = false; g.Size = 0; }
                return;
            }
            f.Offset = off;
            f.Size = ps;
            f.Overlay = owners.Any(o => off < o.Offset + o.Size && o.Offset < off + ps);
            if (!f.Overlay) owners.Add(f);
            size = Math.Max(size, off + ps);
            align = Math.Max(align, pa);
        }
        t.ExplicitAlign = align;
        t.ExplicitSize = (size + align - 1) / align * align;
        t.InstFields = owners.OrderBy(f => f.Offset).ToList();
    }

    /// Does the reflection data of `t` include instance fields and properties?
    public bool ReflectMembers(TType t) { ComputeReflect(); return _reflect.Contains(t.Sym); }
    public bool ReflectsEnum(INamedTypeSymbol e) => NameMatches(Cfg.Reflect, FullName(e));
    /// Types whose reflection data is generated even when no `typeof` in the program mentions them.
    public List<TType> ReflectForced() { ComputeReflect(); return _reflectForce; }
    /// Method-group delegates: method id (the value `Delegate::from_method` gets) -> `Declaring.Type+Nested.Method`.
    public readonly ConcurrentDictionary<ulong, string> MethodNames = new();

    /// Translated types that are type arguments of the host crate's JSON members (`ToObject<T>`, `DeserializeObject<T>`).
    public readonly ConcurrentDictionary<INamedTypeSymbol, bool> JsonTypes = new(SymbolEqualityComparer.Default);
    /// The config's `jsonTypes`, and every translated class derived from a type that already has a member table
    /// (Newtonsoft writes an object by its run-time type). Returns whether a type was added.
    public bool NoteDerivedJsonTypes()
    {
        bool added = false;
        foreach (var tt in Types.Values)
        {
            if (tt.WholeTypeReason != null || tt.IsInterface || tt.IsStatic || tt.Sym is not INamedTypeSymbol n || n.IsGenericType || JsonTypes.ContainsKey(n)) continue;
            bool want = NameMatches(Cfg.JsonTypes, FullName(n));
            for (var b = n.BaseType; !want && b != null; b = b.BaseType) want = JsonTypes.ContainsKey(b);
            if (want && JsonTypes.TryAdd(n, true)) added = true;
        }
        return added;
    }
    public void NoteJsonType(ITypeSymbol t)
    {
        while (t is IArrayTypeSymbol a) t = a.ElementType;
        if (t is not INamedTypeSymbol n) return;
        foreach (var ta in n.TypeArguments) NoteJsonType(ta);
        if (Types.TryGetValue(n, out var tt) && tt.WholeTypeReason == null && !tt.IsInterface && !tt.IsStatic) JsonTypes.TryAdd(n, true);
    }

    // ---- float mode "jit32": IL sizes of the original methods and what the allocation did
    Dictionary<string, int> _il;
    /// IL byte size of the original method (largest overload with that parameter count), -1 when unknown.
    public int IlSize(IMethodSymbol m)
    {
        if (_il == null)
            lock (_popLock)
            {
                if (_il == null)
                {
                    var d = new Dictionary<string, int>(StringComparer.Ordinal);
                    if (Cfg.IlSizes != null)
                        foreach (var line in File.ReadLines(Path.Combine(Cfg.BaseDir, Cfg.IlSizes)))
                        {
                            var p = line.Split('\t');
                            if (p.Length == 4) d[p[0] + "\t" + p[1] + "\t" + p[2]] = int.Parse(p[3]);
                        }
                    _il = d;
                }
            }
        if (_il.Count == 0 || m.ContainingType == null) return -1;
        var parts = new List<string>();
        for (var c = m.ContainingType.OriginalDefinition; c != null; c = c.ContainingType) parts.Insert(0, c.MetadataName);
        var ns = m.ContainingType.ContainingNamespace == null || m.ContainingType.ContainingNamespace.IsGlobalNamespace ? "" : m.ContainingType.ContainingNamespace.ToDisplayString() + ".";
        return _il.TryGetValue(ns + string.Join("+", parts) + "\t" + m.MetadataName + "\t" + m.Parameters.Length, out int v) ? v : -1;
    }
    // ---- float mode "jit32": what the real JIT did at the call sites of a caller (jit_calls.tsv)
    Dictionary<string, (int sites, int calls)> _jitCalls;
    /// Call sites of `callee` in `caller` and how many of them the JIT compiled as call instructions; null when the
    /// table does not have the pair. Keys: `ClrMethodKey`.
    public (int sites, int calls)? JitCalls(string caller, string callee)
    {
        if (_jitCalls == null)
            lock (_popLock)
            {
                if (_jitCalls == null)
                {
                    var d = new Dictionary<string, (int, int)>(StringComparer.Ordinal);
                    if (Cfg.JitCalls != null)
                        foreach (var line in File.ReadLines(Path.Combine(Cfg.BaseDir, Cfg.JitCalls)))
                        {
                            var p = line.Split('\t');
                            if (p.Length == 4) d[p[0] + "\t" + p[1]] = (int.Parse(p[2]), int.Parse(p[3]));
                        }
                    _jitCalls = d;
                }
            }
        return _jitCalls.TryGetValue(caller + "\t" + callee, out var v) ? v : null;
    }
    public int JitTableInlined, JitTableCalls, JitTableMixed, JitTableMissing;

    // ---- float mode "jit32": what the reference run's JIT did at the call sites of a caller (jit_refcalls.tsv)
    Dictionary<string, (int sites, int calls)> _jitRefCalls;
    /// As `JitCalls`, from the dump of a reference run (config `jitRefCalls`); null when the table does not have the pair.
    public (int sites, int calls)? JitRefCalls(string caller, string callee)
    {
        if (_jitRefCalls == null)
            lock (_popLock)
            {
                if (_jitRefCalls == null)
                {
                    var d = new Dictionary<string, (int, int)>(StringComparer.Ordinal);
                    if (Cfg.JitRefCalls != null)
                        foreach (var line in File.ReadLines(Path.Combine(Cfg.BaseDir, Cfg.JitRefCalls)))
                        {
                            var p = line.Split('	');
                            if (p.Length == 4) d[p[0] + "	" + p[1]] = (int.Parse(p[2]), int.Parse(p[3]));
                        }
                    _jitRefCalls = d;
                }
            }
        return _jitRefCalls.TryGetValue(caller + "	" + callee, out var v) ? v : null;
    }
    public int JitRefCallsUsed;

    // ---- float mode "jit32": the sites of the mixed pairs of jit_refcalls.tsv (jit_refcall_sites.tsv)
    Dictionary<string, List<(string file, int sl, int sc, int el, int ec, int ord, bool call)>> _jitRefCallSites;
    /// The sites of a (caller, callee) pair of `JitRefCalls` that is a call instruction at some of its sites only
    /// (config `jitRefCallSites`); null when the table does not place the pair.
    public List<(string file, int sl, int sc, int el, int ec, int ord, bool call)> JitRefCallSites(string caller, string callee)
    {
        if (_jitRefCallSites == null)
            lock (_popLock)
            {
                if (_jitRefCallSites == null)
                {
                    var d = new Dictionary<string, List<(string, int, int, int, int, int, bool)>>(StringComparer.Ordinal);
                    if (Cfg.JitRefCallSites != null && File.Exists(Path.Combine(Cfg.BaseDir, Cfg.JitRefCallSites)))
                        foreach (var line in File.ReadLines(Path.Combine(Cfg.BaseDir, Cfg.JitRefCallSites)))
                        {
                            var p = line.Split('\t');
                            if (p.Length != 9) continue;
                            string k = p[0] + "\t" + p[1];
                            if (!d.TryGetValue(k, out var l)) d[k] = l = new();
                            l.Add((p[2], int.Parse(p[3]), int.Parse(p[4]), int.Parse(p[5]), int.Parse(p[6]), int.Parse(p[7]), p[8] == "1"));
                        }
                    _jitRefCallSites = d;
                }
            }
        return _jitRefCallSites.TryGetValue(caller + "\t" + callee, out var v) ? v : null;
    }
    public int JitRefCallSitesUsed;

    Dictionary<string, Dictionary<string, string>> _jitFloatAlloc;
    /// The real JIT's allocation of the float locals of `caller` (config `jitFloatAlloc`): local name -> kind; null
    /// when the method is not in the table.
    public Dictionary<string, string> JitFloatAlloc(string caller)
    {
        if (_jitFloatAlloc == null)
            lock (_popLock)
            {
                if (_jitFloatAlloc == null)
                {
                    var d = new Dictionary<string, Dictionary<string, string>>(StringComparer.Ordinal);
                    if (Cfg.JitFloatAlloc != null)
                        foreach (var line in File.ReadLines(Path.Combine(Cfg.BaseDir, Cfg.JitFloatAlloc)))
                        {
                            var p = line.Split('	');
                            if (p.Length != 3) continue;
                            if (!d.TryGetValue(p[0], out var m)) d[p[0]] = m = new Dictionary<string, string>(StringComparer.Ordinal);
                            m[p[1]] = p[2];
                        }
                    _jitFloatAlloc = d;
                }
            }
        return caller != null && _jitFloatAlloc.TryGetValue(caller, out var v) ? v : null;
    }
    public int JitFloatAllocMethods, JitFloatAllocMemory, JitFloatAllocStack, JitFloatAllocTemps, JitFloatAllocMismatch;

    // ---- float mode "jit32": where a library member's x87 body (config `jitBodies`) runs (jit_bodies.tsv)
    Dictionary<string, (int sites, int calls)> _jitBodySites;
    /// Does the caller call `callee` with a call instruction at every one of its IL call sites, in the dumped run?
    public bool JitBodyCalled(string caller, string callee)
    {
        if (caller == null || callee == null) return false;
        if (_jitBodySites == null)
            lock (_popLock)
            {
                if (_jitBodySites == null)
                {
                    var d = new Dictionary<string, (int, int)>(StringComparer.Ordinal);
                    if (Cfg.JitBodySites != null)
                        foreach (var line in File.ReadLines(Path.Combine(Cfg.BaseDir, Cfg.JitBodySites)))
                        {
                            var p = line.Split('\t');
                            if (p.Length == 4) d[p[0] + "\t" + p[1]] = (int.Parse(p[2]), int.Parse(p[3]));
                        }
                    _jitBodySites = d;
                }
            }
        return _jitBodySites.TryGetValue(caller + "\t" + callee, out var v) && v.sites > 0 && v.calls >= v.sites;
    }
    /// The caller ran in a reference run and has IL call sites of `callee` (whether the JIT called or inlined them).
    public bool JitBodySeen(string caller, string callee)
    {
        if (caller == null || callee == null) return false;
        JitBodyCalled(caller, callee);
        return _jitBodySites.TryGetValue(caller + "\t" + callee, out var v) && v.sites > 0;
    }
    public int JitBodiesUsed;
    /// Call instructions to `callee` in the caller's code, -1 when the pair is not in jit_bodies.tsv.
    public int JitBodyCalls(string caller, string callee)
    {
        if (caller == null || callee == null) return -1;
        JitBodyCalled(caller, callee);
        return _jitBodySites.TryGetValue(caller + "	" + callee, out var v) ? v.calls : -1;
    }
    Dictionary<string, string> _jitVecTemps;
    /// "stack" / "memory" (jit_vec_temps.tsv) or null.
    public string JitVecTemps(string caller)
    {
        if (caller == null) return null;
        if (_jitVecTemps == null)
            lock (_popLock)
            {
                if (_jitVecTemps == null)
                {
                    var d = new Dictionary<string, string>(StringComparer.Ordinal);
                    if (Cfg.JitVecTemps != null)
                        foreach (var line in File.ReadLines(Path.Combine(Cfg.BaseDir, Cfg.JitVecTemps)))
                        {
                            var p = line.Split('	');
                            if (p.Length == 2) d[p[0]] = p[1];
                        }
                    _jitVecTemps = d;
                }
            }
        return _jitVecTemps.TryGetValue(caller, out var v) ? v : null;
    }
    public int VecInlinedSites, VecInlinedStackSites, VecCtxSites;
    Dictionary<string, string> _vecCtx;
    Dictionary<string, List<string>> _vecCtxSigs;
    void LoadVecCtx()
    {
        if (_vecCtx != null) return;
        lock (_popLock)
        {
            if (_vecCtx != null) return;
            var d = new Dictionary<string, string>(StringComparer.Ordinal);
            var sigs = new Dictionary<string, List<string>>(StringComparer.Ordinal);
            if (Cfg.JitVecCtx != null)
                foreach (var line in File.ReadLines(Path.Combine(Cfg.BaseDir, Cfg.JitVecCtx)))
                {
                    var p = line.Split('\t');
                    if (p.Length != 4) continue;
                    d[p[0] + "\t" + p[1] + "\t" + p[2]] = p[3];
                    if (!sigs.TryGetValue(p[1], out var l)) sigs[p[1]] = l = new List<string>();
                    if (!l.Contains(p[3])) l.Add(p[3]);
                }
            foreach (var l in sigs.Values) l.Sort(StringComparer.Ordinal);
            _vecCtxSigs = sigs;
            _vecCtx = d;
        }
    }
    HashSet<string> _jitInlined, _jitInlinedCallees;
    void LoadJitInlined()
    {
        if (_jitInlined != null) return;
        lock (_popLock)
        {
            if (_jitInlined != null) return;
            var d = new HashSet<string>(StringComparer.Ordinal);
            var c = new HashSet<string>(StringComparer.Ordinal);
            if (Cfg.JitInlined != null)
                foreach (var line in File.ReadLines(Path.Combine(Cfg.BaseDir, Cfg.JitInlined)))
                {
                    var p = line.Split('\t');
                    if (p.Length != 3) continue;
                    d.Add(p[0] + "\t" + p[1]);
                    c.Add(p[1]);
                }
            _jitInlinedCallees = c;
            _jitInlined = d;
        }
    }
    /// Did the reference runs' JIT inline `callee` into `caller` at every site (config `jitInlined`)?
    public bool JitInlinedAt(string caller, string callee)
    {
        if (caller == null || callee == null || Cfg.FloatMode != "jit32") return false;
        LoadJitInlined();
        return _jitInlined.Contains(caller + "\t" + callee);
    }
    public int InlinedArgSites;
    /// Float mode "jit32": the ordinals of the float parameters of `def` that its twin `name__u` takes unrounded (f64):
    /// `def` is inlined somewhere (config `jitInlined`), is called directly (the conditions of VecTwin), and never
    /// writes those parameters, takes their address or captures them. Empty: no twin.
    public IReadOnlyList<int> ArgTwinParams(IMethodSymbol def)
    {
        if (Cfg.FloatMode != "jit32" || def == null) return Array.Empty<int>();
        def = def.OriginalDefinition;
        return _argTwin.GetOrAdd(def, d =>
        {
            var none = Array.Empty<int>();
            if (ClrMethodKey(d) is not { } key) return none;
            LoadJitInlined();
            if (!_jitInlinedCallees.Contains(key)) return none;
            if (d.MethodKind is not (MethodKind.Ordinary or MethodKind.PropertyGet)) return none;
            if (d.IsVirtual || d.IsAbstract || d.IsOverride || d.IsExtern || d.IsGenericMethod || d.ContainingType == null) return none;
            if (d.ContainingType.TypeKind == TypeKind.Interface || d.ContainingType.IsGenericType) return none;
            if (Cfg.Bodies.ContainsKey(DocId(d)) || IsResumable(d)) return none;
            var tt = Get(d.ContainingType);
            if (tt == null) return none;
            var mi = Method(tt, d);
            if (mi == null || !mi.Emittable || mi.Open != null) return none;
            var syn = d.DeclaringSyntaxReferences.FirstOrDefault()?.GetSyntax();
            if (syn is not BaseMethodDeclarationSyntax { Body: not null } and not BaseMethodDeclarationSyntax { ExpressionBody: not null }) return none;
            if (syn.DescendantNodes().OfType<YieldStatementSyntax>().Any()) return none;
            var outp = new List<int>();
            foreach (var p in d.Parameters)
            {
                if (p.Type.SpecialType != SpecialType.System_Single || p.RefKind != RefKind.None) continue;
                bool written = false;
                foreach (var id in syn.DescendantNodes().OfType<IdentifierNameSyntax>())
                {
                    if (id.Identifier.ValueText != p.Name) continue;
                    var par = id.Parent;
                    while (par is ParenthesizedExpressionSyntax) par = par.Parent;
                    if (par is AssignmentExpressionSyntax asg && asg.Left.DescendantNodesAndSelf().Contains(id)) written = true;
                    else if (par is PrefixUnaryExpressionSyntax pu && (pu.IsKind(SyntaxKind.PreIncrementExpression) || pu.IsKind(SyntaxKind.PreDecrementExpression) || pu.IsKind(SyntaxKind.AddressOfExpression))) written = true;
                    else if (par is PostfixUnaryExpressionSyntax) written = true;
                    else if (par is ArgumentSyntax arg && !arg.RefKindKeyword.IsKind(SyntaxKind.None)) written = true;
                    if (id.Ancestors().Any(x => x is AnonymousFunctionExpressionSyntax or LocalFunctionStatementSyntax)) written = true;
                    if (written) break;
                }
                if (!written) outp.Add(p.Ordinal);
            }
            return outp.ToArray();
        });
    }
    readonly ConcurrentDictionary<IMethodSymbol, int[]> _argTwin = new(SymbolEqualityComparer.Default);
    /// The twin index (`__vcN`) for the k-th call site of `callee` in `caller`, or -1.
    public int VecCtxTwin(string caller, string callee, int k)
    {
        if (caller == null || callee == null) return -1;
        LoadVecCtx();
        return _vecCtx.TryGetValue(caller + "\t" + callee + "\t" + k, out var sig) ? _vecCtxSigs[callee].IndexOf(sig) : -1;
    }
    /// The contexts of `callee` that need a twin (index = N of `__vcN`).
    public IReadOnlyList<string> VecCtxSigs(string callee)
    {
        if (callee == null) return Array.Empty<string>();
        LoadVecCtx();
        return _vecCtxSigs.TryGetValue(callee, out var l) ? l : Array.Empty<string>();
    }

    /// `Full.Type::Name(ParamType,...)` with CLR names as the JIT harness writes them (nested types with '+'); null
    /// for a method of a generic type or a generic method, or with a parameter type that has no plain CLR name here.
    public static string ClrMethodKey(IMethodSymbol m)
    {
        m = m.ReducedFrom ?? m;
        if (m.IsGenericMethod || m.ContainingType == null) return null;
        string t = ClrTypeName(m.ContainingType);
        if (t == null) return null;
        var ps = new List<string>();
        foreach (var p in m.Parameters)
        {
            string pt = ClrTypeName(p.Type);
            if (pt == null) return null;
            ps.Add(pt + (p.RefKind != RefKind.None ? "&" : ""));
        }
        return t + "::" + m.MetadataName + "(" + string.Join(",", ps) + ")";
    }

    static string ClrTypeName(ITypeSymbol t)
    {
        if (t is IArrayTypeSymbol a)
        {
            string e = ClrTypeName(a.ElementType);
            return e == null ? null : e + "[" + new string(',', a.Rank - 1) + "]";
        }
        if (t is not INamedTypeSymbol n) return null;
        var parts = new List<string>();
        var args = new List<ITypeSymbol>();
        for (var c = n; c != null; c = c.ContainingType)
        {
            if (c.IsGenericType && c.IsUnboundGenericType) return null;
            parts.Insert(0, c.MetadataName);
            args.InsertRange(0, c.TypeArguments);
        }
        var top = n;
        while (top.ContainingType != null) top = top.ContainingType;
        var ns = top.ContainingNamespace == null || top.ContainingNamespace.IsGlobalNamespace ? "" : top.ContainingNamespace.ToDisplayString() + ".";
        string name = ns + string.Join("+", parts);
        if (args.Count == 0) return name;
        // a constructed generic type, as the CLR names it in a method signature (the dumps' keys, JitDump.cs):
        // `List`1[[Terraria.DataStructures.DrawData, Terraria, Version=1.4.5.8, Culture=neutral, PublicKeyToken=null]]`,
        // every argument with its assembly's display name (the game's own types: the Terraria assembly; jit_calls.py
        // load_json names the server's TerrariaServer.exe the same)
        var rendered = new List<string>();
        foreach (var ta in args)
        {
            if (ta is ITypeParameterSymbol) return null;
            string an = ClrTypeName(ta), asm = ClrAssemblyName(ta);
            if (an == null || asm == null) return null;
            rendered.Add("[" + an + ", " + asm + "]");
        }
        return name + "[" + string.Join(",", rendered) + "]";
    }

    /// The display name of the assembly a type comes from, as the CLR prints it in a generic argument.
    static string ClrAssemblyName(ITypeSymbol t)
    {
        while (t is IArrayTypeSymbol at) t = at.ElementType;
        var asm = t.ContainingAssembly;
        if (asm == null) return null;
        if (asm is ISourceAssemblySymbol || asm.Name == "Terraria")
        {
            var top = t as INamedTypeSymbol;
            while (top?.ContainingType != null) top = top.ContainingType;
            string ns = top?.ContainingNamespace?.ToDisplayString() ?? "";
            return ns == "ReLogic" || ns.StartsWith("ReLogic.", StringComparison.Ordinal)
                ? "ReLogic, Version=1.0.0.0, Culture=neutral, PublicKeyToken=null"
                : "Terraria, Version=1.4.5.8, Culture=neutral, PublicKeyToken=null";
        }
        return asm.Identity.GetDisplayName();
    }

    /// Resumable forms that can start a thread (void, no parameters; static, or an instance method called on the
    /// start delegate's receiver `recv`): (method id of `Delegate::from_method`, Rust call of the resumable form).
    /// Written to `gen::resumable` (DESIGN.md section 8f).
    public readonly ConcurrentDictionary<ulong, string> ResumableThreads = new();
    public bool IsResumable(IMethodSymbol m) => Cfg.Resumable != null && Cfg.Resumable.Methods.Contains(DocId(m.OriginalDefinition));

    /// Float mode "jit32": a translated method that returns `float` also gets a twin `name__x87` returning the
    /// unrounded value (f64) of its return expressions: the 32-bit JIT returns a float in ST0 without a store, and
    /// the caller continues with that value until it stores or casts it (FLOAT_RULE.md section 2, C09 / V02 / V05 /
    /// V08; refdata/entities: `scale = Opacity / 2f * localAI[0]` with `Opacity => 1f - (float)alpha / 255f`).
    /// A direct call of such a method in an unrounded float expression calls the twin (Emit.F64). Only methods that
    /// are called directly: not virtual / abstract / override, not of an interface, not generic, not an auto-property
    /// accessor, not replaced by config "bodies", not resumable.
    public bool TwinF64(IMethodSymbol def)
    {
        if (Cfg.FloatMode != "jit32" || def == null) return false;
        def = def.OriginalDefinition;
        return _twin.GetOrAdd(def, d =>
        {
            if (d.ReturnType.SpecialType != SpecialType.System_Single || d.MethodKind is not (MethodKind.Ordinary or MethodKind.PropertyGet)) return false;
            if (d.IsVirtual || d.IsAbstract || d.IsOverride || d.IsExtern || d.IsGenericMethod || d.ContainingType == null) return false;
            if (d.ContainingType.TypeKind == TypeKind.Interface || d.ContainingType.IsGenericType) return false;
            if (Cfg.Bodies.ContainsKey(DocId(d)) || IsResumable(d)) return false;
            var tt = Get(d.ContainingType);
            if (tt == null) return false;
            var mi = Method(tt, d);
            if (mi == null || !mi.Emittable || mi.Open != null) return false;
            var syn = d.DeclaringSyntaxReferences.FirstOrDefault()?.GetSyntax();
            if (syn == null) return false;
            if (syn is AccessorDeclarationSyntax { Body: null, ExpressionBody: null }) return false;
            if (syn is BaseMethodDeclarationSyntax { Body: null, ExpressionBody: null }) return false;
            if (syn.DescendantNodes().OfType<YieldStatementSyntax>().Any()) return false;
            return true;
        });
    }
    readonly ConcurrentDictionary<IMethodSymbol, bool> _twin = new(SymbolEqualityComparer.Default);
    /// Float mode "jit32": a translated method that a caller inlined with other answers for its Vector2 operators
    /// (jit_vec_ctx.tsv) gets twins `name__vcN`; the conditions of TwinF64, any return type.
    public bool VecTwin(IMethodSymbol def)
    {
        if (Cfg.FloatMode != "jit32" || def == null) return false;
        def = def.OriginalDefinition;
        if (VecCtxSigs(ClrMethodKey(def)).Count == 0) return false;
        if (def.MethodKind is not (MethodKind.Ordinary or MethodKind.PropertyGet)) return false;
        if (def.IsVirtual || def.IsAbstract || def.IsOverride || def.IsExtern || def.IsGenericMethod || def.ContainingType == null) return false;
        if (def.ContainingType.TypeKind == TypeKind.Interface || def.ContainingType.IsGenericType) return false;
        if (Cfg.Bodies.ContainsKey(DocId(def)) || IsResumable(def)) return false;
        var tt = Get(def.ContainingType);
        if (tt == null) return false;
        var mi = Method(tt, def);
        if (mi == null || !mi.Emittable || mi.Open != null) return false;
        var syn = def.DeclaringSyntaxReferences.FirstOrDefault()?.GetSyntax();
        return syn is BaseMethodDeclarationSyntax { Body: not null } or BaseMethodDeclarationSyntax { ExpressionBody: not null };
    }
    /// The id a method-group delegate of `m` carries (`Delegate::from_method`): FNV-1a of its documentation id,
    /// declaring type and type arguments, never 0.
    public ulong MethodId(IMethodSymbol m)
    {
        ulong h = 14695981039346656037UL;
        foreach (var ch in DocId(m.OriginalDefinition) + "|" + Mono.Key(Mono.Sub(m.ContainingType)) + "|" + string.Join(",", m.TypeArguments.Select(x => Mono.Key(Mono.Sub(x))))) { h ^= ch; h *= 1099511628211UL; }
        return h | 1;
    }

    public int FloatMethods, FloatMinOpts, FloatStackLocals, FloatDemoted;
    public readonly ConcurrentBag<string> FloatUndetermined = new();
    /// One line per analysed body when CS2RS_FLOAT_REPORT names a file (tools/cs2rs/gen_float_candidates.py): CLR key,
    /// MinOpts, float locals, the ones on the x87 stack, the stack ones rounded at a call.
    public readonly ConcurrentBag<string> FloatReport = new();

    // ---- generated support modules (filled while bodies are emitted)
    public readonly ConcurrentDictionary<string, string> IfaceFns = new(StringComparer.Ordinal);
    public readonly ConcurrentDictionary<string, string> EnumFns = new(StringComparer.Ordinal);
    public readonly ConcurrentDictionary<string, string> MetaFns = new(StringComparer.Ordinal);
    /// The translated type behind each entry of `MetaFns` (its parameterless constructor for `Activator.CreateInstance`).
    public readonly ConcurrentDictionary<string, TType> MetaTypes = new(StringComparer.Ordinal);
    public string MetaText() => string.Concat(MetaFns.OrderBy(k => k.Key, StringComparer.Ordinal).Select(k => k.Value));
    public string IfaceText() => string.Concat(IfaceFns.OrderBy(k => k.Key, StringComparer.Ordinal).Select(k => k.Value));
    public string EnumText() => string.Concat(EnumFns.OrderBy(k => k.Key, StringComparer.Ordinal).Select(k => k.Value));

    public bool IsBclException(ITypeSymbol t)
    {
        if (Get(t) != null) return false;
        for (var c = t as INamedTypeSymbol; c != null; c = c.BaseType)
            if (SymbolEqualityComparer.Default.Equals(c, ExceptionType)) return true;
        return false;
    }

    static readonly SymbolDisplayFormat FullFmt = new(typeQualificationStyle: SymbolDisplayTypeQualificationStyle.NameAndContainingTypesAndNamespaces);
    /// Namespace-qualified name; an instantiation of a generic type carries its type arguments (`Terraria.Ref<int>`).
    public static string FullName(ISymbol s) => s is INamedTypeSymbol { IsGenericType: true } g && Mono.IsClosed(g) ? Mono.Key(g) : s.ToDisplayString(FullFmt);
    public static string ShortMember(ISymbol s)
    {
        var owner = s.ContainingType != null ? s.ContainingType.ToDisplayString(new SymbolDisplayFormat(typeQualificationStyle: SymbolDisplayTypeQualificationStyle.NameAndContainingTypes)) + "." : "";
        return owner + s.Name;
    }
}
