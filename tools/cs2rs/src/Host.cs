// Host boundary: every library member (.NET, XNA, ReLogic, Steamworks ...) that translated code uses and that
// native.json does not map gets one generated Rust function in `<crate>::host::api::<namespace>`.
// The set of those functions is the exact specification of the shim the host has to provide (HOST_API.md).
// DESIGN.md section 10.
using System.Collections.Concurrent;
using System.Text;
using Microsoft.CodeAnalysis;

namespace Cs2Rs;

public sealed class HostFn
{
    public string Key;            // doc id + kind
    public string NativeKey;      // the key that maps this member in nativeMembers, as HOST_API.md shows it
    public string Module, Name;   // host::api::<Module>::<Name>
    public string Display;        // Namespace.Type.Member(params) as C# writes it
    public string Namespace;
    public List<string> Generics = new();
    public List<(string name, string type)> Params = new();
    public string Ret;            // "()" for void
    public string Value;          // configured value (stubs), or null
    public string Inert;          // reason from the config's "inert" (a void member that does nothing observable), or null
    public string Body;           // Rust body from the config's "hostBodies" (runtime objects behind a library interface), or null
    public string Kind;           // call / get / set / new / field-get / field-set
    public int Sites;
    /// Referenced by a generated dispatcher: emitted even without a direct call site.
    public bool Forced;
    public SortedSet<string> Users = new(StringComparer.Ordinal);
    public string Path(string hostModule) => $"{hostModule}::{Module}::{Name}";
}

public sealed class HostType
{
    public INamedTypeSymbol Sym; public string Module, Name; public int Uses;
}

public sealed class HostApi
{
    readonly Model M;
    public readonly ConcurrentDictionary<string, HostFn> Fns = new(StringComparer.Ordinal);
    public readonly ConcurrentDictionary<string, HostType> Types = new(StringComparer.Ordinal);
    /// Type tests / casts against library classes: full type name -> (sites, users).
    public readonly ConcurrentDictionary<string, HostFn> TypeTests = new(StringComparer.Ordinal);
    public HostApi(Model m) { M = m; }

    public static string NsModule(INamespaceSymbol ns)
    {
        if (ns == null || ns.IsGlobalNamespace) return "global";
        return Names.Ident(Names.Snake(ns.ToDisplayString().Replace('.', '_')));
    }

    public static string TypeIdent(INamedTypeSymbol t)
    {
        var parts = new List<string>();
        for (var c = t; c != null; c = c.ContainingType) parts.Insert(0, c.Name + (c.Arity > 0 ? c.Arity.ToString() : ""));
        return string.Join("_", parts);
    }

    /// Rust path of an opaque library value type (declared in the host module of its namespace).
    public string StructType(INamedTypeSymbol n, string hostModule)
    {
        var key = M.DocId(n);
        var ht = Types.GetOrAdd(key, _ => new HostType { Sym = n, Module = NsModule(n.ContainingNamespace), Name = Names.Ident(TypeIdent(n)) });
        return $"{hostModule}::{ht.Module}::{ht.Name}";
    }

    static readonly SymbolDisplayFormat TypeFmt = new(
        typeQualificationStyle: SymbolDisplayTypeQualificationStyle.NameAndContainingTypesAndNamespaces,
        genericsOptions: SymbolDisplayGenericsOptions.IncludeTypeParameters);
    static readonly SymbolDisplayFormat MemberFmt = new(
        typeQualificationStyle: SymbolDisplayTypeQualificationStyle.NameAndContainingTypes,
        genericsOptions: SymbolDisplayGenericsOptions.IncludeTypeParameters,
        memberOptions: SymbolDisplayMemberOptions.IncludeParameters,
        parameterOptions: SymbolDisplayParameterOptions.IncludeType | SymbolDisplayParameterOptions.IncludeParamsRefOut,
        miscellaneousOptions: SymbolDisplayMiscellaneousOptions.UseSpecialTypes);

    public static string DisplayOf(ISymbol s, string kind)
    {
        var owner = s.ContainingType.ToDisplayString(TypeFmt);
        string member = s switch
        {
            IMethodSymbol { MethodKind: MethodKind.Constructor } c => ".ctor(" + string.Join(", ", c.Parameters.Select(p => (p.RefKind == RefKind.Ref ? "ref " : p.RefKind == RefKind.Out ? "out " : "") + p.Type.ToDisplayString(SymbolDisplayFormat.MinimallyQualifiedFormat))) + ")",
            IMethodSymbol m => m.ToDisplayString(MemberFmt),
            _ => s.Name,
        };
        return owner + "." + member + (kind is "field-get" or "field-set" ? (kind == "field-set" ? " (field, write)" : " (field)") : "");
    }

    /// The host function for library member `sym` (a method / accessor / constructor / field), or null with a
    /// reason when its signature cannot be written in Rust.
    public HostFn Get(ISymbol sym, string kind, out string reason)
    {
        reason = null;
        sym = sym.OriginalDefinition;
        var key = M.DocId(sym) + (kind is "field-set" ? "#set" : "");
        if (Fns.TryGetValue(key, out var have)) return have;
        var f = new HostFn { Key = key, Kind = kind };
        f.NativeKey = sym is IMethodSymbol { AssociatedSymbol: IPropertySymbol aps } am
            ? "`" + M.DocId(aps.OriginalDefinition) + "` (`" + (am.MethodKind == MethodKind.PropertyGet ? "get" : "set") + "`)"
            : "`" + M.DocId(sym) + "`" + (kind == "field-set" ? " (`set`)" : kind == "field-get" ? " (`get`)" : "");
        var ct = sym.ContainingType;
        f.Module = NsModule(ct.ContainingNamespace);
        f.Namespace = ct.ContainingNamespace == null || ct.ContainingNamespace.IsGlobalNamespace ? "(global)" : ct.ContainingNamespace.ToDisplayString();
        f.Display = DisplayOf(sym, kind);
        if (sym is IMethodSymbol { IsExtern: true } xm && xm.GetDllImportData() is { } dll) f.Display += " [P/Invoke " + dll.ModuleName + "]";
        var tname = Names.Snake(TypeIdent(ct));
        // generics: type parameters that stay Rust type parameters, outer types first
        var seen = new HashSet<string>();
        void AddGenerics(IEnumerable<ITypeParameterSymbol> tps)
        {
            foreach (var tp in tps) { var n = Names.Ident(tp.Name); if (M.RT(tp) == n && seen.Add(n)) f.Generics.Add(n); }
        }
        var chain = new List<INamedTypeSymbol>();
        for (var c = ct; c != null; c = c.ContainingType) chain.Insert(0, c);
        foreach (var c in chain) AddGenerics(c.TypeParameters);
        bool isStatic = sym.IsStatic || sym is IMethodSymbol { MethodKind: MethodKind.Constructor };
        if (!isStatic)
        {
            var rt = M.RT(ct);
            if (rt == null) { reason = "receiver type with no Rust representation: " + Model.Short(ct); return null; }
            f.Params.Add(("this", ct.IsValueType ? "*mut " + rt : rt));
        }
        string T(ITypeSymbol t, out bool ok) { var r = M.RT(t); ok = r != null; return r; }
        if (sym is IMethodSymbol m)
        {
            AddGenerics(m.TypeParameters);
            var used = new HashSet<string> { "this" };
            foreach (var p in m.Parameters)
            {
                var pt = T(p.Type, out bool ok);
                if (!ok) { reason = "host signature uses a type with no Rust representation: " + Model.Short(p.Type); return null; }
                var pn = Names.Ident(p.Name); if (pn == "rt") pn = "rt_";
                while (!used.Add(pn)) pn += "_";
                f.Params.Add((pn, p.RefKind is RefKind.Ref or RefKind.Out ? "*mut " + pt : pt));
            }
            if (m.MethodKind == MethodKind.Constructor)
            {
                f.Ret = M.RT(ct);
                f.Name = tname + "__new";
            }
            else
            {
                f.Ret = m.ReturnsVoid ? "()" : T(m.ReturnType, out _);
                string mn = m.MethodKind switch
                {
                    MethodKind.PropertyGet => "get_" + Names.Snake(m.AssociatedSymbol.Name.Split('.').Last()),
                    MethodKind.PropertySet => "set_" + Names.Snake(m.AssociatedSymbol.Name.Split('.').Last()),
                    MethodKind.EventAdd => "add_" + Names.Snake(m.AssociatedSymbol.Name.Split('.').Last()),
                    MethodKind.EventRemove => "remove_" + Names.Snake(m.AssociatedSymbol.Name.Split('.').Last()),
                    _ => Names.Snake(m.Name.Split('.').Last()),
                };
                f.Name = tname + "__" + mn;
            }
            if (f.Ret == null) { reason = "host call returning a type with no Rust representation: " + Model.Short(m.MethodKind == MethodKind.Constructor ? ct : m.ReturnType); return null; }
            var ps = m.Parameters.Select(p => (p.RefKind == RefKind.Ref ? "ref_" : p.RefKind == RefKind.Out ? "out_" : "") + Model.Spell(p.Type)).ToList();
            if (m.MethodKind == MethodKind.Conversion) ps.Add("to_" + Model.Spell(m.ReturnType));
            if (ps.Count > 0 && m.MethodKind is not (MethodKind.PropertySet or MethodKind.EventAdd or MethodKind.EventRemove) || m.MethodKind == MethodKind.PropertyGet && ps.Count > 0)
                f.Name += "__" + string.Join("_", ps);
            if (m.TypeParameters.Length > 0) f.Name += "__g" + m.TypeParameters.Length;
        }
        else if (sym is IFieldSymbol fs)
        {
            var ft = M.RT(fs.Type);
            if (ft == null) { reason = "host field of a type with no Rust representation: " + Model.Short(fs.Type); return null; }
            if (kind == "field-set") { f.Params.Add(("value", ft)); f.Ret = "()"; f.Name = tname + "__set_field_" + Names.Snake(fs.Name); }
            else { f.Ret = ft; f.Name = tname + "__field_" + Names.Snake(fs.Name); }
        }
        else { reason = "host member kind " + sym.Kind; return null; }
        f.Name = Names.Ident(f.Name.Replace('`', '_'));
        f.Value = M.Stub(sym);
        f.Inert = M.Cfg.Inert.TryGetValue(M.DocId(sym), out var why) ? why : null;
        f.Body = M.Cfg.HostBodies.TryGetValue(M.DocId(sym), out var hb) ? hb : null;
        return Fns.GetOrAdd(key, f);
    }

    public void Hit(HostFn f, string user)
    {
        lock (f) { f.Sites++; f.Users.Add(user); }
    }

    public string TypeTest(INamedTypeSymbol t, string user)
    {
        var name = t.ToDisplayString(TypeFmt);
        var f = TypeTests.GetOrAdd(name, _ => new HostFn { Key = name, Display = name, Namespace = t.ContainingNamespace?.ToDisplayString() ?? "(global)" });
        Hit(f, user);
        return name;
    }

    // ------------------------------------------------------------------ output
    static string Q(string s) => "\"" + s.Replace("\\", "\\\\").Replace("\"", "\\\"") + "\"";

    /// Writes `<dir>/mod.rs` and one file per namespace. Returns the files written (full paths) and the change count.
    public (List<string> files, int changed) WriteRust(string dir, string genModule, string hostRoot)
    {
        var files = new List<string>(); int changed = 0;
        var mods = Fns.Values.Where(f => (f.Sites > 0 || f.Forced)).Select(f => f.Module).Concat(Types.Values.Select(t => t.Module)).Distinct().OrderBy(x => x, StringComparer.Ordinal).ToList();
        var root = new StringBuilder("// @generated by tools/cs2rs. Do not edit.\n");
        root.Append("//! The host boundary: one function per library member (.NET / XNA / ReLogic / Steamworks) that the translated\n");
        root.Append("//! code calls and that `tools/cs2rs/native.json` does not map to hand-written Rust. Bodies go through the single\n");
        root.Append("//! registry in `host` (`unimplemented` / `unimplemented_value` / `configured`). The list is `tools/cs2rs/HOST_API.md`.\n");
        root.Append("#![allow(non_snake_case, non_camel_case_types, unused_variables, unused_imports, dead_code, clippy::all)]\n\n");
        foreach (var m in mods) root.Append($"pub mod {m};\n");
        void W(string path, string text) { if (Program.WriteIfChanged(path, text)) changed++; files.Add(Path.GetFullPath(path)); }
        W(Path.Combine(dir, "mod.rs"), root.ToString());
        foreach (var mod in mods)
        {
            var sb = new StringBuilder("// @generated by tools/cs2rs. Do not edit.\n");
            sb.Append($"use crate::rt;\nuse crate::rt::*;\nuse crate::{genModule}::*;\nuse {hostRoot} as host;\n\n");
            foreach (var t in Types.Values.Where(t => t.Module == mod).OrderBy(t => t.Name, StringComparer.Ordinal))
            {
                sb.Append($"/// `{t.Sym.ToDisplayString(TypeFmt)}`: opaque library value type. The host shim defines the real layout.\n");
                sb.Append($"#[repr(C)]\n#[derive(Clone, Copy)]\npub struct {t.Name} {{\n    pub opaque: [u64; 4],\n}}\nunsafe impl rt::Trace for {t.Name} {{\n    const NEEDS_TRACE: bool = false;\n    const BOX: rt::BoxKind = rt::BoxKind::Value(rt::tid::OPAQUE);\n    #[inline(always)]\n    unsafe fn trace(&self, _m: &mut rt::Marker) {{}}\n}}\nimpl rt::CsKey for {t.Name} {{\n    fn cs_eq(&self, o: &Self) -> bool {{\n        self.opaque == o.opaque\n    }}\n    fn cs_hash<H: core::hash::Hasher>(&self, h: &mut H) {{\n        core::hash::Hash::hash(&self.opaque, h)\n    }}\n}}\n\n// ordering an opaque value has no meaning: the type has to be mapped natively first (loud, never a made-up order)\nimpl rt::sys::CsOrd for {t.Name} {{\n    fn cs_cmp(&self, _o: &Self) -> i32 {{\n        host::unimplemented_value::<i32>({Q(t.Sym.ToDisplayString(TypeFmt) + " (ordering of an opaque library value)")})\n    }}\n}}\n\n");
            }
            foreach (var f in Fns.Values.Where(f => f.Module == mod && (f.Sites > 0 || f.Forced)).OrderBy(f => f.Name, StringComparer.Ordinal).ThenBy(f => f.Key, StringComparer.Ordinal))
            {
                var g = f.Generics.Count > 0 ? "<" + string.Join(", ", f.Generics.Select(x => x + ": rt::Val")) + ">" : "";
                sb.Append($"/// `{f.Display}`\n");
                sb.Append($"pub unsafe fn {f.Name}{g}({string.Join(", ", f.Params.Select(p => p.name + ": " + p.type))}){(f.Ret == "()" ? "" : " -> " + f.Ret)} {{\n");
                if (f.Body != null) sb.Append($"    // hostBodies (translator config)\n    {f.Body}\n");
                else if (f.Value != null) sb.Append($"    host::configured({Q(f.Display)}, {f.Value})\n");
                else if (f.Kind == "new" && f.Ret == "Obj") sb.Append($"    host::new_opaque({Q(f.Display)})\n");
                else if (f.Ret == "()" && f.Inert != null) sb.Append($"    // inert by the translator config: {f.Inert.Replace("\n", " ")}\n    host::inert({Q(f.Display)})\n");
                else if (f.Ret == "()") sb.Append($"    host::unimplemented({Q(f.Display)})\n");
                else sb.Append($"    host::unimplemented_value::<{f.Ret}>({Q(f.Display)})\n");
                sb.Append("}\n\n");
            }
            W(Path.Combine(dir, mod + ".rs"), sb.ToString());
        }
        return (files, changed);
    }

    public string Markdown()
    {
        var sb = new StringBuilder("# Host API: library members the translated code calls\n\nGenerated by `cs2rs`. Do not edit.\n\n");
        var all = Fns.Values.Where(f => (f.Sites > 0 || f.Forced)).ToList();
        sb.Append("Every row is one generated function in `port\\crates\\terraria-gen\\src\\host\\api\\<namespace>.rs` whose body calls the registry ");
        sb.Append("(`host::unimplemented`: logs once and continues, for `void`; `host::unimplemented_value`: panics; `host::configured`: returns the value written in the translator config; ");
        sb.Append("`host::new_opaque`: the constructor of a library class returns a fresh opaque object, which translated code can only hand back to other host members). ");
        sb.Append("A member disappears from this list when `tools\\cs2rs\\native.json` or a file of `tools\\cs2rs\\native.d\\` maps it to hand-written Rust (`NATIVE_SCHEMA.md`): the second column of the namespace tables is the key to use there, the third the Rust types the template receives and has to produce.\n\n");
        sb.Append($"Distinct members: **{all.Count}** ({all.Count(f => f.Ret == "()")} void, {all.Count(f => f.Ret != "()" && f.Value == null && !(f.Kind == "new" && f.Ret == "Obj"))} value-returning without a configured value, {all.Count(f => f.Kind == "new" && f.Ret == "Obj" && f.Value == null)} constructors of opaque library classes, {all.Count(f => f.Value != null)} with a configured value). Call sites: {all.Sum(f => f.Sites)}. ");
        sb.Append($"Opaque library value types: {Types.Count}. Type tests / casts against library classes: {TypeTests.Count} types, {TypeTests.Values.Sum(t => t.Sites)} sites.\n\n");
        sb.Append("## By namespace\n\n| namespace | members | call sites |\n|---|---:|---:|\n");
        foreach (var g in all.GroupBy(f => f.Namespace).OrderByDescending(g => g.Sum(f => f.Sites)).ThenBy(g => g.Key, StringComparer.Ordinal))
            sb.Append($"| `{g.Key}` | {g.Count()} | {g.Sum(f => f.Sites)} |\n");
        sb.Append("\n## Top 30 by call sites\n\n| member | call sites | classes |\n|---|---:|---:|\n");
        foreach (var f in all.OrderByDescending(f => f.Sites).ThenBy(f => f.Display, StringComparer.Ordinal).Take(30))
            sb.Append($"| `{f.Display}` | {f.Sites} | {f.Users.Count} |\n");
        foreach (var g in all.GroupBy(f => f.Namespace).OrderBy(g => g.Key, StringComparer.Ordinal))
        {
            sb.Append($"\n## `{g.Key}`\n\n| member | `nativeMembers` key | Rust types (receiver, parameters) -> result | returns | call sites | used by |\n|---|---|---|---|---:|---|\n");
            foreach (var f in g.OrderBy(f => f.Display, StringComparer.Ordinal).ThenBy(f => f.Key, StringComparer.Ordinal))
                sb.Append($"| `{f.Display}` | {f.NativeKey} | `({string.Join(", ", f.Params.Select(p => p.type))}){(f.Ret == "()" ? "" : " -> " + f.Ret)}` | {(f.Ret == "()" ? (f.Inert != null ? "void, inert (config)" : "void") : f.Value != null ? "configured" : f.Kind == "new" && f.Ret == "Obj" ? "new opaque object" : "value")} | {f.Sites} | {string.Join(", ", f.Users.Take(8))}{(f.Users.Count > 8 ? $", ... ({f.Users.Count})" : "")} |\n");
        }
        if (Types.Count > 0)
        {
            sb.Append("\n## Opaque library value types\n\n");
            foreach (var t in Types.Values.OrderBy(t => t.Module + t.Name, StringComparer.Ordinal)) sb.Append($"- `{t.Sym.ToDisplayString(TypeFmt)}`\n");
        }
        if (TypeTests.Count > 0)
        {
            sb.Append("\n## Type tests and casts against library classes (`host::is_instance`)\n\n| type | sites | used by |\n|---|---:|---|\n");
            foreach (var t in TypeTests.Values.OrderBy(t => t.Display, StringComparer.Ordinal)) sb.Append($"| `{t.Display}` | {t.Sites} | {string.Join(", ", t.Users.Take(8))} |\n");
        }
        return sb.ToString();
    }
}
