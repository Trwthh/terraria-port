// Mono: closed-world monomorphisation of the game's generic classes and generic methods. DESIGN.md section 8a.
//
// A generic game class is never emitted as such. Every CLOSED instantiation the program can construct
// (`Ref<int>`, `WeightedRandom<string>`, `Asset<Texture2D>`) is found before anything is emitted and becomes a type
// of its own: its own module, struct, type id and `Statics` (C# gives every instantiation its own static fields).
// A generic METHOD of a game class is treated the same way: one Rust function per (method, type arguments).
//
// Roslyn gives signatures of constructed symbols already substituted (`Ref<int>.Value` is an `int` field). Bodies
// are bound once, in terms of the definition's type parameters; while a body is emitted for one instantiation the
// substitution is ambient (`Mono.Enter`), and `Model.RT` / `Model.Get` and the emitter's type tests apply it.
using System.Collections.Concurrent;
using System.Collections.Immutable;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Operations;

namespace Cs2Rs;

public static class Mono
{
    [ThreadStatic] static Dictionary<ITypeParameterSymbol, ITypeSymbol> _map;
    public static CSharpCompilation Comp;

    public readonly struct Scope : IDisposable
    {
        readonly Dictionary<ITypeParameterSymbol, ITypeSymbol> _saved;
        readonly bool _entered;
        public Scope(Dictionary<ITypeParameterSymbol, ITypeSymbol> saved) { _saved = saved; _entered = true; }
        /// `default(Scope)` is "no scope": disposing it changes nothing.
        public void Dispose() { if (_entered) _map = _saved; }
    }

    /// Makes `map` the substitution of this thread until the scope is disposed (null = no substitution).
    public static Scope Enter(Dictionary<ITypeParameterSymbol, ITypeSymbol> map)
    {
        var s = new Scope(_map);
        _map = map == null || map.Count == 0 ? null : map;
        return s;
    }

    public static bool Active => _map != null;

    /// Type parameters of the definitions of `type` (and its containing types) and of `method` -> their arguments.
    public static Dictionary<ITypeParameterSymbol, ITypeSymbol> MapOf(INamedTypeSymbol type, IMethodSymbol method = null)
    {
        var map = new Dictionary<ITypeParameterSymbol, ITypeSymbol>(SymbolEqualityComparer.Default);
        for (var c = type; c != null; c = c.ContainingType)
        {
            var ps = c.OriginalDefinition.TypeParameters; var args = c.TypeArguments;
            for (int i = 0; i < ps.Length && i < args.Length; i++)
                if (!SymbolEqualityComparer.Default.Equals(ps[i], args[i])) map[ps[i]] = args[i];
        }
        if (method != null)
        {
            var ps = method.OriginalDefinition.TypeParameters; var args = method.TypeArguments;
            for (int i = 0; i < ps.Length && i < args.Length; i++)
                if (!SymbolEqualityComparer.Default.Equals(ps[i], args[i])) map[ps[i]] = args[i];
        }
        return map;
    }

    /// `t` with the ambient substitution applied.
    public static ITypeSymbol Sub(ITypeSymbol t)
    {
        if (_map == null || t == null) return t;
        return Sub0(t);
    }

    static ITypeSymbol Sub0(ITypeSymbol t)
    {
        switch (t)
        {
            case ITypeParameterSymbol tp:
                return _map.TryGetValue(tp, out var r) ? r : (tp.OriginalDefinition is ITypeParameterSymbol od && _map.TryGetValue(od, out r) ? r : t);
            case IArrayTypeSymbol a:
                {
                    var e = Sub0(a.ElementType);
                    return SymbolEqualityComparer.Default.Equals(e, a.ElementType) ? t : Comp.CreateArrayTypeSymbol(e, a.Rank);
                }
            case IPointerTypeSymbol p:
                {
                    var e = Sub0(p.PointedAtType);
                    return SymbolEqualityComparer.Default.Equals(e, p.PointedAtType) ? t : Comp.CreatePointerTypeSymbol(e);
                }
            case INamedTypeSymbol n:
                {
                    if (!n.IsGenericType) return n;
                    var ct = n.ContainingType;
                    var nct = ct == null ? null : (INamedTypeSymbol)Sub0(ct);
                    bool changed = ct != null && !SymbolEqualityComparer.Default.Equals(ct, nct);
                    var args = n.TypeArguments;
                    var nargs = new ITypeSymbol[args.Length];
                    for (int i = 0; i < args.Length; i++)
                    {
                        nargs[i] = Sub0(args[i]);
                        if (!SymbolEqualityComparer.Default.Equals(nargs[i], args[i])) changed = true;
                    }
                    if (!changed) return n;
                    if (n.IsUnboundGenericType) return n;
                    INamedTypeSymbol def = n.OriginalDefinition;
                    if (nct != null)
                    {
                        def = null;
                        foreach (var x in nct.GetTypeMembers(n.Name, n.Arity))
                            if (SymbolEqualityComparer.Default.Equals(x.OriginalDefinition, n.OriginalDefinition)) { def = x; break; }
                        if (def == null) return n;
                    }
                    return n.Arity > 0 ? def.Construct(nargs) : def;
                }
        }
        return t;
    }

    /// No type parameter anywhere inside `t`.
    public static bool IsClosed(ITypeSymbol t)
    {
        switch (t)
        {
            case null: return true;
            case ITypeParameterSymbol: return false;
            case IArrayTypeSymbol a: return IsClosed(a.ElementType);
            case IPointerTypeSymbol p: return IsClosed(p.PointedAtType);
            case INamedTypeSymbol n:
                if (n.IsUnboundGenericType) return false;
                foreach (var a in n.TypeArguments) if (!IsClosed(a)) return false;
                return n.ContainingType == null || IsClosed(n.ContainingType);
        }
        return true;
    }

    static readonly SymbolDisplayFormat KeyFmt = new(
        globalNamespaceStyle: SymbolDisplayGlobalNamespaceStyle.Omitted,
        typeQualificationStyle: SymbolDisplayTypeQualificationStyle.NameAndContainingTypesAndNamespaces,
        genericsOptions: SymbolDisplayGenericsOptions.IncludeTypeParameters,
        miscellaneousOptions: SymbolDisplayMiscellaneousOptions.UseSpecialTypes | SymbolDisplayMiscellaneousOptions.ExpandNullable);

    /// A stable, unique text for a closed type (sort key of instantiations, display in reports).
    public static string Key(ITypeSymbol t) => t.ToDisplayString(KeyFmt);

    /// Nesting depth of type arguments (`List<Ref<int>>` = 2): the guard against unbounded instantiation.
    public static int Depth(ITypeSymbol t)
    {
        switch (t)
        {
            case IArrayTypeSymbol a: return Depth(a.ElementType);
            case INamedTypeSymbol n:
                int d = 0;
                foreach (var x in n.TypeArguments) d = Math.Max(d, Depth(x));
                if (n.ContainingType != null) d = Math.Max(d, Depth(n.ContainingType) - 1);
                return n.TypeArguments.Length > 0 || n.ContainingType?.IsGenericType == true ? d + 1 : d;
        }
        return 0;
    }
}

/// One instantiation of a generic method: the containing type (closed), the definition, the type arguments.
public sealed class MethodInst
{
    public INamedTypeSymbol Type; public IMethodSymbol Def; public ImmutableArray<ITypeSymbol> Args; public string Key;
}

/// Finds every closed instantiation of the selected generic types and of the generic methods of translated types.
public sealed class Discovery
{
    readonly Model M; readonly Gen G;
    public readonly ConcurrentDictionary<INamedTypeSymbol, byte> TypeInsts = new(SymbolEqualityComparer.Default);
    public readonly ConcurrentDictionary<string, MethodInst> MethodInsts = new(StringComparer.Ordinal);
    readonly ConcurrentQueue<object> _work = new();
    /// Every enum type the translated code mentions: each gets a type id of its own (boxing; DESIGN.md section 2).
    public readonly ConcurrentDictionary<INamedTypeSymbol, byte> Enums = new(SymbolEqualityComparer.Default);
    public const int MaxDepth = 6;
    public readonly ConcurrentBag<string> TooDeep = new();

    public Discovery(Model m, Gen g) { M = m; G = g; }

    bool Translated(INamedTypeSymbol closed) => M.Types.ContainsKey(closed) || TypeInsts.ContainsKey(closed);

    public void RegisterType(ITypeSymbol t)
    {
        t = Mono.Sub(t);
        switch (t)
        {
            case null: return;
            case IArrayTypeSymbol a: RegisterType(a.ElementType); return;
            case IPointerTypeSymbol p: RegisterType(p.PointedAtType); return;
            case INamedTypeSymbol n:
                if (n.TypeKind == TypeKind.Enum) { if (!Enums.ContainsKey(n)) Enums.TryAdd(n, 0); return; }
                if (!n.IsGenericType) return;
                foreach (var x in n.TypeArguments) RegisterType(x);
                if (n.ContainingType != null) RegisterType(n.ContainingType);
                if (M.GenericDefs.Contains(n.OriginalDefinition) && Mono.IsClosed(n))
                {
                    if (TypeInsts.ContainsKey(n)) return;
                    if (Mono.Depth(n) > MaxDepth) { TooDeep.Add(Mono.Key(n)); return; }
                    if (TypeInsts.TryAdd(n, 0)) _work.Enqueue(n);
                }
                return;
        }
    }

    public void RegisterMethod(IMethodSymbol m)
    {
        if (m == null) return;
        m = m.ReducedFrom ?? m;
        if (m.MethodKind is MethodKind.LocalFunction or MethodKind.AnonymousFunction) return;
        RegisterType(m.ContainingType);
        if (!m.IsGenericMethod) return;
        foreach (var x in m.TypeArguments) RegisterType(x);
        if (Mono.Sub(m.ContainingType) is not INamedTypeSymbol owner) return;
        AddMethod(owner, m.OriginalDefinition, m.TypeArguments.Select(Mono.Sub).ToImmutableArray());
    }

    bool AddMethod(INamedTypeSymbol owner, IMethodSymbol def, ImmutableArray<ITypeSymbol> args)
    {
        if (!Mono.IsClosed(owner) || args.Any(a => !Mono.IsClosed(a))) return false;
        if (!def.Locations.Any(l => l.IsInSource)) return false;
        // the owner is a translated type, or an instantiation that RegisterType just added (or will not: unselected)
        if (!Translated(owner)) return false;
        var key = Mono.Key(owner) + "|" + M.DocId(def) + "|" + string.Join(",", args.Select(Mono.Key));
        var mi = new MethodInst { Type = owner, Def = def, Args = args, Key = key };
        if (!MethodInsts.TryAdd(key, mi)) return false;
        _work.Enqueue(mi);
        return true;
    }

    /// The member of the closed type `owner` whose definition is `def`.
    public static IMethodSymbol MemberOf(INamedTypeSymbol owner, IMethodSymbol def)
    {
        foreach (var x in owner.GetMembers(def.Name))
            if (x is IMethodSymbol ms && SymbolEqualityComparer.Default.Equals(ms.OriginalDefinition, def)) return ms;
        return null;
    }

    void Signature(INamedTypeSymbol n)
    {
        RegisterType(n.BaseType);
        foreach (var i in n.AllInterfaces) RegisterType(i);
        foreach (var m in n.GetMembers())
        {
            switch (m)
            {
                case IFieldSymbol f: RegisterType(f.Type); break;
                case IPropertySymbol p: RegisterType(p.Type); foreach (var q in p.Parameters) RegisterType(q.Type); break;
                case IEventSymbol e: RegisterType(e.Type); break;
                case IMethodSymbol ms when !ms.IsGenericMethod:
                    RegisterType(ms.ReturnType);
                    foreach (var q in ms.Parameters) RegisterType(q.Type);
                    break;
            }
        }
    }

    /// Everything a type (a translated non-generic type, or one instantiation) mentions: signatures and bodies.
    void ProcessType(INamedTypeSymbol n)
    {
        using var scope = Mono.Enter(Mono.MapOf(n));
        Signature(n);
        foreach (var (_, init) in G.Initializers(n, true)) Walk(init);
        foreach (var (_, init) in G.Initializers(n, false)) Walk(init);
        foreach (var m in n.GetMembers())
            if (m is IMethodSymbol { IsGenericMethod: false, IsAbstract: false } ms) Walk(G.BodyOp(ms, out _));
    }

    void ProcessMethod(MethodInst mi)
    {
        var member = MemberOf(mi.Type, mi.Def);
        if (member == null) return;
        var cm = member.Construct(mi.Args.ToArray());
        using var scope = Mono.Enter(Mono.MapOf(mi.Type, cm));
        RegisterType(cm.ReturnType);
        foreach (var q in cm.Parameters) RegisterType(q.Type);
        if (!member.IsAbstract) Walk(G.BodyOp(member, out _));
    }

    void Walk(IOperation root)
    {
        if (root == null) return;
        foreach (var d in root.DescendantsAndSelf())
        {
            if (d.Type != null) RegisterType(d.Type);
            switch (d)
            {
                case IInvocationOperation inv: RegisterMethod(inv.TargetMethod); break;
                case IObjectCreationOperation oc: if (oc.Constructor != null) RegisterType(oc.Constructor.ContainingType); break;
                case IMethodReferenceOperation mr: RegisterMethod(mr.Method); break;
                case IMemberReferenceOperation mem: RegisterType(mem.Member.ContainingType); break;
                case IBinaryOperation { OperatorMethod: { } om }: RegisterMethod(om); break;
                case IUnaryOperation { OperatorMethod: { } om }: RegisterMethod(om); break;
                case IConversionOperation { OperatorMethod: { } om }: RegisterMethod(om); break;
                case ICompoundAssignmentOperation { OperatorMethod: { } om }: RegisterMethod(om); break;
                case IIncrementOrDecrementOperation { OperatorMethod: { } om }: RegisterMethod(om); break;
                case ITypeOfOperation to: RegisterType(to.TypeOperand); break;
                case IIsTypeOperation it: RegisterType(it.TypeOperand); break;
                case IDeclarationPatternOperation dp: RegisterType(dp.MatchedType); break;
                case ITypePatternOperation tp: RegisterType(tp.MatchedType); break;
                case IVariableDeclaratorOperation vd: RegisterType(vd.Symbol.Type); break;
                case ICatchClauseOperation cc: RegisterType(cc.ExceptionType); break;
                case IAnonymousFunctionOperation lam: foreach (var q in lam.Symbol.Parameters) RegisterType(q.Type); RegisterType(lam.Symbol.ReturnType); break;
                case ILocalFunctionOperation lf: foreach (var q in lf.Symbol.Parameters) RegisterType(q.Type); RegisterType(lf.Symbol.ReturnType); break;
                case IForEachLoopOperation fe when fe.Syntax is CommonForEachStatementSyntax fs && fe.SemanticModel != null:
                    {
                        var info = fe.SemanticModel.GetForEachStatementInfo(fs);
                        RegisterMethod(info.GetEnumeratorMethod); RegisterMethod(info.MoveNextMethod); RegisterMethod(info.CurrentProperty?.GetMethod); RegisterMethod(info.DisposeMethod);
                        RegisterType(info.ElementType);
                        break;
                    }
            }
        }
    }

    void Drain()
    {
        while (_work.TryDequeue(out var w))
        {
            if (w is INamedTypeSymbol n) ProcessType(n);
            else if (w is MethodInst mi) ProcessMethod(mi);
        }
    }

    static bool OverridesDef(IMethodSymbol m, IMethodSymbol rootDef)
    {
        for (var c = m; c != null; c = c.OverriddenMethod)
            if (SymbolEqualityComparer.Default.Equals(c.OriginalDefinition, rootDef)) return true;
        return false;
    }

    /// An instantiation of a virtual / abstract / interface generic method needs the same instantiation of every
    /// override and implementation (the dispatcher calls them), and of the methods it overrides (call sites name the root).
    bool CloseVirtuals()
    {
        bool added = false;
        var all = M.Types.Keys.Concat(TypeInsts.Keys).Where(t => t.TypeKind is TypeKind.Class or TypeKind.Struct).OrderBy(Mono.Key, StringComparer.Ordinal).ToList();
        foreach (var mi in MethodInsts.Values.OrderBy(x => x.Key, StringComparer.Ordinal).ToList())
        {
            var def = mi.Def;
            if (def.ContainingType.TypeKind == TypeKind.Interface)
            {
                var im = MemberOf(mi.Type, def);
                if (im == null) continue;
                foreach (var x in all)
                {
                    if (!x.AllInterfaces.Contains(mi.Type, SymbolEqualityComparer.Default)) continue;
                    if (x.FindImplementationForInterfaceMember(im) is IMethodSymbol impl && impl.ContainingType is { } ict)
                        added |= AddMethod(ict, impl.OriginalDefinition, mi.Args);
                }
                continue;
            }
            if (!(def.IsVirtual || def.IsAbstract || def.IsOverride)) continue;
            var member = MemberOf(mi.Type, def);
            for (var o = member?.OverriddenMethod; o != null; o = o.OverriddenMethod)
                added |= AddMethod(o.ContainingType, o.OriginalDefinition, mi.Args);
            foreach (var x in all)
            {
                if (x.TypeKind != TypeKind.Class) continue;
                bool derives = false;
                for (var b = x.BaseType; b != null; b = b.BaseType) if (SymbolEqualityComparer.Default.Equals(b, mi.Type)) { derives = true; break; }
                if (!derives) continue;
                for (var c = x; c != null && !SymbolEqualityComparer.Default.Equals(c, mi.Type); c = c.BaseType)
                    foreach (var k in c.GetMembers(def.Name).OfType<IMethodSymbol>())
                        if (k.IsOverride && k.IsGenericMethod && OverridesDef(k, def)) added |= AddMethod(c, k.OriginalDefinition, mi.Args);
            }
        }
        return added;
    }

    /// Roots: every configured (non-generic) type, walked on the worker threads; then the instantiations they
    /// reach, to a fixed point.
    public void Run()
    {
        var roots = M.Ordered.Select(t => t.Sym).ToList();
        Program.ParallelBig(Enumerable.Range(0, roots.Count).OrderByDescending(i => roots[i].Locations.Where(l => l.IsInSource).Sum(l => l.SourceSpan.Length)).ToList(), Program.Dop, i => ProcessType(roots[i]));
        do { Drain(); } while (CloseVirtuals());
    }
}
