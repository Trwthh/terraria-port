// FloatAlloc: which `float` locals of a method the 32-bit JIT keeps on the x87 stack (float mode "jit32").
// The rule and its evidence: port\crates\terraria-x87\FLOAT_RULE.md, sections 3 and 5. DESIGN.md section 5.
//
// A float local is either in memory (every store rounds to 32 bits) or on the x87 stack (stores do not round; the
// value is rounded at every non-inlined call it is live across). This file decides which, per method, from:
// reference counts weighted by loop depth, liveness (live across a call, live into a handler, interference), address
// taken, and the six x87 registers. What is NOT modelled is listed in DESIGN.md ("jit32: limits").
using System.Collections.Immutable;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.FlowAnalysis;
using Microsoft.CodeAnalysis.Operations;

namespace Cs2Rs;

public sealed class FloatAlloc
{
    /// Locals kept on the x87 stack (an unrounded f64 variable in the generated code).
    public readonly HashSet<ISymbol> Stack = new(SymbolEqualityComparer.Default);
    /// Call syntax -> stack locals that are live across it (rounded there).
    public readonly Dictionary<SyntaxNode, List<ISymbol>> Spill = new();
    public bool MinOpts;
    /// The allocation comes from the real JIT (config `jitFloatAlloc`), not from the rule.
    public bool FromJit;
    /// Evaluation-stack values the decompiler made into locals: on the x87 stack, never rounded at a call.
    public readonly HashSet<ISymbol> Kept = new(SymbolEqualityComparer.Default);
    /// More float candidates interfere than there are registers and the tie is not determined (FLOAT_RULE.md 4.1).
    public bool Undetermined;

    public const int MinOptsIlBytes = 40000;   // about 20,000 IL instructions (B02: 32,026 bytes optimised; B03: 41,626 not)
    public const int InlineIlBytes = 32;       // the JIT's "always inline" size class

    sealed class Ev { public int Kind; public ISymbol Sym; public double Weight; public SyntaxNode Call; public bool Rule, Real, Alloc, Proven; }   // 0 read, 1 write, 2 call
    /// Calls whose rounding the reference run's own machine code proves (config `jitRefCalls`): the emitter rounds
    /// at the call itself where the call is evaluated only on some paths of its statement (`a && F()`).
    public readonly HashSet<SyntaxNode> ProvenCalls = new();

    readonly Model M;
    /// The analysed body (`Analyse`): the sites of a callee in a statement, for config `jitRefCallSites`.
    IOperation _root;
    /// The analysed method as jit_calls.tsv names it (`Model.ClrMethodKey`); null: no table lookups.
    string _caller;
    /// Locals of this method that config `floatSpillAtAlloc` names (`Type.Method:local`): rounded at an array
    /// creation they are live across (the JIT's allocation helper is a call instruction; measured per method).
    HashSet<string> _allocSpill;
    readonly Dictionary<ISymbol, int> _index = new(SymbolEqualityComparer.Default);
    readonly List<ISymbol> _vars = new();
    /// The float variables analysed (parameters and locals; empty for MinOpts).
    public IReadOnlyList<ISymbol> Vars => _vars;
    FloatAlloc(Model m) { M = m; }

    static double WeightOf(SyntaxNode n)
    {
        double w = 1;
        for (var p = n?.Parent; p != null && p is not (BaseMethodDeclarationSyntax or AccessorDeclarationSyntax or AnonymousFunctionExpressionSyntax or LocalFunctionStatementSyntax); p = p.Parent)
            if (p is ForStatementSyntax or WhileStatementSyntax or DoStatementSyntax or ForEachStatementSyntax or ForEachVariableStatementSyntax) w *= 8;
        return w;
    }

    /// Is a call of `m` compiled as a call instruction (FLOAT_RULE.md section 6)? Unknown callees count as calls.
    /// A call site's two answers: `rule`: the size rule below, which stands in for the JIT's register pressure when
    /// it decides which float locals get an x87 register; `real`: whether the real JIT compiled a call instruction
    /// here (jit_calls.tsv; the rule where the table has no answer), which is where a value on the x87 stack is
    /// rounded. Measured 2026-10-13: the table alone as the register-allocation input breaks HitLineWall (the JIT
    /// kept those locals in memory) while fixing CanHitLine (its Math.Sign is inlined: no rounding there).
    (bool rule, bool real, bool proven) CallKind(IMethodSymbol m, IOperation at)
    {
        bool rule = IsCall(m, at);
        m = m.ReducedFrom ?? m;
        // the reference run's own code (jit_refcalls.tsv): a call instruction at every site rounds what is live across
        // it, an inlined callee rounds nothing; whatever the size rule says (ApplyHellLight: Tile.slope() is a call
        // there, the rule thought it inlined; Math.Sin is fsin, the rule thought it a call)
        if (_caller != null && Model.ClrMethodKey(m.OriginalDefinition) is { } rk && M.JitRefCalls(_caller, rk) is { } rv && (rv.calls == 0 || rv.calls >= rv.sites))
        {
            Interlocked.Increment(ref M.JitRefCallsUsed);
            return (rule, rv.calls > 0, true);
        }
        // a call instruction at some sites only: this site's own answer (jit_refcall_sites.tsv; Dust.GetAlpha: the JIT
        // inlined Color.get_R in `(float)newColor.R * num` and kept num unrounded, but called it in `new Color(newColor.R, ..)`)
        if (_caller != null && Model.ClrMethodKey(m.OriginalDefinition) is { } sk && M.JitRefCalls(_caller, sk) is { } sv && SiteIsCall(m, sk, at) is bool siteCall)
        {
            Interlocked.Increment(ref M.JitRefCallSitesUsed);
            return (rule, siteCall, true);
        }
        var (r1, r2) = CallKindTable(m, rule);
        return (r1, r2, false);
    }

    /// The answer of jit_refcall_sites.tsv for this site of `callee`: the innermost decompiled statement of the pair's
    /// rows that contains it, and its ordinal among the sites of the same callee in that statement (source order, the
    /// IL order of one statement); null when the table has no answer or the statement's sites do not match its rows.
    bool? SiteIsCall(IMethodSymbol m, string calleeKey, IOperation at)
    {
        var rows = M.JitRefCallSites(_caller, calleeKey);
        if (rows == null || at?.Syntax?.SyntaxTree is not { } tree || _root == null) return null;
        string path = tree.FilePath.Replace('\\', '/');
        var text = tree.GetText();
        TextSpanOf span = null;
        foreach (var r in rows)
        {
            if (!path.EndsWith("/" + r.file, StringComparison.OrdinalIgnoreCase) || r.sl < 1 || r.el > text.Lines.Count) continue;
            int s = text.Lines[r.sl - 1].Start + r.sc - 1, e = text.Lines[r.el - 1].Start + r.ec - 1;
            if (s <= at.Syntax.SpanStart && at.Syntax.Span.End <= e && (span == null || e - s < span.End - span.Start))
                span = new TextSpanOf { Start = s, End = e, Sl = r.sl, Sc = r.sc };
        }
        if (span == null) return null;
        var inSpan = rows.Where(r => r.sl == span.Sl && r.sc == span.Sc && path.EndsWith("/" + r.file, StringComparison.OrdinalIgnoreCase)).OrderBy(r => r.ord).ToList();
        var sites = _root.DescendantsAndSelf().Where(o => o.Syntax.SpanStart >= span.Start && o.Syntax.Span.End <= span.End && SiteCallee(o) is { } c
                && Model.ClrMethodKey(c.OriginalDefinition) == calleeKey).Select(o => o.Syntax).Distinct().OrderBy(s => s.SpanStart).ThenBy(s => s.Span.Length).ToList();
        if (sites.Count != inSpan.Count) return null;
        int k = sites.IndexOf(at.Syntax);
        return k < 0 || inSpan[k].ord != k ? null : inSpan[k].call;
    }
    sealed class TextSpanOf { public int Start, End, Sl, Sc; }
    static IMethodSymbol SiteCallee(IOperation o) => o switch
    {
        IInvocationOperation i => (i.TargetMethod.ReducedFrom ?? i.TargetMethod),
        IObjectCreationOperation oc => oc.Constructor,
        IPropertyReferenceOperation p => p.Parent is ISimpleAssignmentOperation a && a.Target == p ? p.Property.SetMethod : p.Property.GetMethod,
        _ => null,
    };

    (bool rule, bool real) CallKindTable(IMethodSymbol m, bool rule)
    {
        if (_caller != null && Model.ClrMethodKey(m.OriginalDefinition) is { } ck && M.JitCalls(_caller, ck) is { } v)
        {
            // inlined: no call instruction, so nothing is rounded there. Only for a callee without float values: an
            // inlined float callee (Utils.RotatedBy in CheckAABBvLineCollision) also keeps ITS intermediates on the
            // x87 stack, which the separately translated callee does not reproduce; there the rule's answer measured
            // closer (2026-10-13: 503 against 1,203 mismatching calls)
            if (v.calls == 0 && !HasFloat(m)) { Interlocked.Increment(ref M.JitTableInlined); return (rule, false); }
            if (v.calls == 0) { Interlocked.Increment(ref M.JitTableMixed); return (rule, rule); }
            // a call the size rule thought inlined: not rounded here either (measured 2026-10-13: rounding there made
            // CheckAABBvLineCollision6 and the light map worse; what the JIT does to a value live across such a call
            // is not modelled yet)
            if (v.calls >= v.sites) { Interlocked.Increment(ref M.JitTableCalls); return (rule, rule); }
            Interlocked.Increment(ref M.JitTableMixed);
        }
        else Interlocked.Increment(ref M.JitTableMissing);
        return (rule, rule);
    }

    /// Does the method take or return a float, double or a struct of them (Vector2 ...)?
    static bool HasFloat(IMethodSymbol m)
    {
        static bool F(ITypeSymbol t) => t.SpecialType is SpecialType.System_Single or SpecialType.System_Double
            || (t.TypeKind == TypeKind.Struct && t.SpecialType == SpecialType.None && t.GetMembers().OfType<IFieldSymbol>().Any(f => !f.IsStatic && f.Type.SpecialType is SpecialType.System_Single or SpecialType.System_Double));
        return F(m.ReturnType) || m.Parameters.Any(p => F(p.Type)) || (!m.IsStatic && m.ContainingType != null && F(m.ContainingType));
    }

    bool IsCall(IMethodSymbol m, IOperation at)
    {
        m = m.ReducedFrom ?? m;
        var def = m.OriginalDefinition;
        var ct = def.ContainingType;
        if (ct == null) return true;
        string tn = Model.FullName(ct);
        if (tn is "Microsoft.Xna.Framework.Vector2" or "Microsoft.Xna.Framework.Vector3" or "Microsoft.Xna.Framework.Vector4" or "Microsoft.Xna.Framework.Point" or "Microsoft.Xna.Framework.Color" or "Microsoft.Xna.Framework.Rectangle")
            return !(def.MethodKind is MethodKind.Constructor or MethodKind.UserDefinedOperator or MethodKind.BuiltinOperator or MethodKind.PropertyGet && def.AssociatedSymbol is IPropertySymbol { Name: "X" or "Y" or "Zero" or "One" });
        if (tn == "System.Math" && def.Name is "Abs" or "Sqrt") return false;   // fabs / fsqrt
        // `array.Length` of a one-dimensional array is `ldlen`, not a call (Utils.MultiLerp: `while (percent / num2 > 1f
        // && num3 < floats.Length - 2)` keeps num and num2 on the x87 stack; counted as a call in the loop header it
        // demoted both to memory, and a shock icon's scale was 1 ulp off: gallery run, frame 851)
        if (tn == "System.Array" && def.Name == "get_Length" && at is IPropertyReferenceOperation { Instance.Type: IArrayTypeSymbol { IsSZArray: true } }) return false;
        if (def.IsVirtual || def.IsAbstract || def.IsOverride || ct.TypeKind == TypeKind.Interface || def.MethodKind == MethodKind.DelegateInvoke) return true;
        if (M.Get(ct) == null) return true;
        // auto-property accessors and getters that return a field are inlined
        if (def.MethodKind is MethodKind.PropertyGet or MethodKind.PropertySet && def.DeclaringSyntaxReferences.FirstOrDefault()?.GetSyntax() is AccessorDeclarationSyntax { Body: null, ExpressionBody: null }) return false;
        // a float result that is an arithmetic expression is never inlined; other small methods are
        int il = M.IlSize(def);
        if (il < 0 || il > InlineIlBytes) return true;
        if (def.ReturnType.SpecialType is SpecialType.System_Single or SpecialType.System_Double)
        {
            var syn = def.DeclaringSyntaxReferences.FirstOrDefault()?.GetSyntax();
            bool plain = syn != null && syn.DescendantNodes().OfType<ReturnStatementSyntax>().All(r => r.Expression is IdentifierNameSyntax or MemberAccessExpressionSyntax or LiteralExpressionSyntax)
                && !(syn is ArrowExpressionClauseSyntax a && a.Expression is not (IdentifierNameSyntax or MemberAccessExpressionSyntax or LiteralExpressionSyntax))
                && !syn.DescendantNodes().OfType<ArrowExpressionClauseSyntax>().Any(a2 => a2.Expression is not (IdentifierNameSyntax or MemberAccessExpressionSyntax or LiteralExpressionSyntax));
            if (!plain) return true;
        }
        return false;
    }

    /// Does evaluating `op` execute a call instruction?
    public bool ContainsCall(IOperation op, bool provenOnly = false)
    {
        var ev = new List<Ev>();
        Walk(op, ev);
        return ev.Any(e => e.Kind == 2 && e.Real && (e.Proven || !provenOnly));
    }

    int Var(ISymbol s)
    {
        if (s is ILocalSymbol { Type.SpecialType: SpecialType.System_Single, RefKind: RefKind.None } || s is IParameterSymbol { Type.SpecialType: SpecialType.System_Single, RefKind: RefKind.None })
        {
            if (!_index.TryGetValue(s, out int i)) { _index[s] = i = _vars.Count; _vars.Add(s); }
            return i;
        }
        return -1;
    }

    void Walk(IOperation op, List<Ev> ev)
    {
        if (op == null) return;
        switch (op)
        {
            case IFlowAnonymousFunctionOperation or IAnonymousFunctionOperation or ILocalFunctionOperation: return;
            case ISimpleAssignmentOperation a when a.Target is ILocalReferenceOperation or IParameterReferenceOperation:
                {
                    Walk(a.Value, ev);
                    var s = a.Target is ILocalReferenceOperation l ? (ISymbol)l.Local : ((IParameterReferenceOperation)a.Target).Parameter;
                    if (Var(s) >= 0) ev.Add(new Ev { Kind = 1, Sym = s, Weight = WeightOf(op.Syntax) });
                    return;
                }
            case ICompoundAssignmentOperation ca when ca.Target is ILocalReferenceOperation or IParameterReferenceOperation:
                {
                    var s = ca.Target is ILocalReferenceOperation l ? (ISymbol)l.Local : ((IParameterReferenceOperation)ca.Target).Parameter;
                    if (Var(s) >= 0) ev.Add(new Ev { Kind = 0, Sym = s, Weight = WeightOf(op.Syntax) });
                    Walk(ca.Value, ev);
                    if (ca.OperatorMethod != null) AddCall(ca.OperatorMethod, op, ev);
                    if (Var(s) >= 0) ev.Add(new Ev { Kind = 1, Sym = s, Weight = WeightOf(op.Syntax) });
                    return;
                }
            case IIncrementOrDecrementOperation inc when inc.Target is ILocalReferenceOperation or IParameterReferenceOperation:
                {
                    var s = inc.Target is ILocalReferenceOperation l ? (ISymbol)l.Local : ((IParameterReferenceOperation)inc.Target).Parameter;
                    if (Var(s) >= 0) { ev.Add(new Ev { Kind = 0, Sym = s, Weight = WeightOf(op.Syntax) }); ev.Add(new Ev { Kind = 1, Sym = s, Weight = WeightOf(op.Syntax) }); }
                    return;
                }
            // `float y = 0f;`: a write (otherwise y would count as live from the method's entry to its first read, and
            // be spilled at calls before it exists: Main.rgbToHsl)
            case IVariableDeclaratorOperation vd when vd.Symbol.Type.SpecialType == SpecialType.System_Single:
                {
                    var init = vd.GetVariableInitializer()?.Value;
                    if (init != null) Walk(init, ev);
                    if (Var(vd.Symbol) >= 0) ev.Add(new Ev { Kind = 1, Sym = vd.Symbol, Weight = WeightOf(op.Syntax) });
                    return;
                }
            case ILocalReferenceOperation lr:
                if (Var(lr.Local) >= 0) ev.Add(new Ev { Kind = 0, Sym = lr.Local, Weight = WeightOf(op.Syntax) });
                return;
            case IParameterReferenceOperation pr:
                if (Var(pr.Parameter) >= 0) ev.Add(new Ev { Kind = 0, Sym = pr.Parameter, Weight = WeightOf(op.Syntax) });
                return;
        }
        // `new T[n] { ... }`: the allocation helper runs after the sizes and before the initialiser (a call
        // instruction: x87 values live across it are spilled). Only where config `floatSpillAtAlloc` names a local of
        // this method (CheckLinevLine: the machine code of the reference run, work\jit\collref)
        if (_allocSpill != null && op is IArrayCreationOperation ac)
        {
            foreach (var d in ac.DimensionSizes) Walk(d, ev);
            ev.Add(new Ev { Kind = 2, Call = op.Syntax, Weight = WeightOf(op.Syntax), Rule = false, Real = true, Alloc = true });
            Walk(ac.Initializer, ev);
            return;
        }
        foreach (var c in op.ChildOperations) Walk(c, ev);
        IMethodSymbol callee = op switch
        {
            IInvocationOperation i => i.TargetMethod,
            IObjectCreationOperation oc => oc.Constructor,
            IPropertyReferenceOperation p => p.Parent is ISimpleAssignmentOperation sa && sa.Target == op ? p.Property.SetMethod : p.Property.GetMethod,
            IBinaryOperation { OperatorMethod: { } bm } => bm,
            IUnaryOperation { OperatorMethod: { } um } => um,
            IConversionOperation { OperatorMethod: { } cm } => cm,
            _ => null,
        };
        if (callee != null) AddCall(callee, op, ev);
    }

    /// The operation's own method (an operator, an invocation, a getter) is a real call here; its operands not counted.
    public bool OwnCall(IOperation op)
    {
        IMethodSymbol callee = op switch
        {
            IInvocationOperation i => i.TargetMethod,
            IObjectCreationOperation oc => oc.Constructor,
            IPropertyReferenceOperation p => p.Property.GetMethod,
            IBinaryOperation { OperatorMethod: { } bm } => bm,
            IUnaryOperation { OperatorMethod: { } um } => um,
            IConversionOperation { OperatorMethod: { } cm } => cm,
            _ => null,
        };
        return callee != null && CallKind(callee, op).Item2;
    }

    void AddCall(IMethodSymbol callee, IOperation op, List<Ev> ev)
    {
        var (rule, real, proven) = CallKind(callee, op);
        if (rule || real) ev.Add(new Ev { Kind = 2, Call = op.Syntax, Weight = WeightOf(op.Syntax), Rule = rule, Real = real, Proven = proven });
    }

    /// Analyses one method body. `root` is the operation Roslyn gives for the declaration; `addressTaken` are the
    /// locals the emitter already knows cannot be wide (ref / out, captured by a lambda).
    /// `extraStack`: float parameters of a `__u` twin (Emit.WideParams): the caller's x87 stack value the inlined body
    /// reads unrounded; they hold a register and are rounded at every call they are live across, like a "stack" local.
    public static FloatAlloc Analyse(Model m, IOperation root, IMethodSymbol method, HashSet<ISymbol> addressTaken, HashSet<ISymbol> extraStack = null)
    {
        var fa = new FloatAlloc(m);
        if (method != null) fa._caller = Model.ClrMethodKey(method.OriginalDefinition);
        fa._root = root;
        if (method != null && m.Cfg.FloatSpillAtAlloc.Count > 0)
        {
            var owner = method.ContainingType.ToDisplayString() + "." + method.Name + ":";
            var names = m.Cfg.FloatSpillAtAlloc.Keys.Where(k => k.StartsWith(owner, StringComparison.Ordinal)).Select(k => k.Substring(owner.Length)).ToList();
            if (names.Count > 0) fa._allocSpill = new HashSet<string>(names);
        }
        if (root == null) return fa;
        if (method != null && m.IlSize(method.OriginalDefinition) >= MinOptsIlBytes) { fa.MinOpts = true; return fa; }
        ControlFlowGraph cfg;
        try
        {
            cfg = root switch
            {
                IMethodBodyOperation mb => ControlFlowGraph.Create(mb),
                IConstructorBodyOperation cb => ControlFlowGraph.Create(cb),
                IBlockOperation { Parent: null } b => ControlFlowGraph.Create(b),
                _ => null,
            };
        }
        catch (Exception) { cfg = null; }
        if (cfg == null) return fa;
        int nb = cfg.Blocks.Length;
        var events = new List<Ev>[nb];
        var inHandler = new bool[nb];
        for (int b = 0; b < nb; b++)
        {
            var blk = cfg.Blocks[b];
            var ev = events[b] = new List<Ev>();
            foreach (var o in blk.Operations) fa.Walk(o, ev);
            fa.Walk(blk.BranchValue, ev);
            for (var r = blk.EnclosingRegion; r != null; r = r.EnclosingRegion)
                if (r.Kind is ControlFlowRegionKind.Catch or ControlFlowRegionKind.Finally or ControlFlowRegionKind.Filter or ControlFlowRegionKind.FilterAndHandler) inHandler[b] = true;
        }
        int nv = fa._vars.Count;
        if (nv == 0 || nv > 512) return fa;
        // backward liveness to a fixed point
        var liveOut = new System.Collections.BitArray[nb]; var liveIn = new System.Collections.BitArray[nb];
        for (int b = 0; b < nb; b++) { liveOut[b] = new System.Collections.BitArray(nv); liveIn[b] = new System.Collections.BitArray(nv); }
        IEnumerable<int> Succ(BasicBlock blk)
        {
            if (blk.FallThroughSuccessor?.Destination != null) yield return blk.FallThroughSuccessor.Destination.Ordinal;
            if (blk.ConditionalSuccessor?.Destination != null) yield return blk.ConditionalSuccessor.Destination.Ordinal;
        }
        bool changed = true;
        while (changed)
        {
            changed = false;
            for (int b = nb - 1; b >= 0; b--)
            {
                var o = new System.Collections.BitArray(nv);
                foreach (int s in Succ(cfg.Blocks[b])) o.Or(liveIn[s]);
                var i = new System.Collections.BitArray(o);
                for (int k = events[b].Count - 1; k >= 0; k--)
                {
                    var e = events[b][k];
                    if (e.Kind == 1) i[fa._index[e.Sym]] = false;
                    else if (e.Kind == 0) i[fa._index[e.Sym]] = true;
                }
                bool same = true;
                for (int v = 0; v < nv && same; v++) same = i[v] == liveIn[b][v] && o[v] == liveOut[b][v];
                if (!same) { liveIn[b] = i; liveOut[b] = o; changed = true; }
            }
        }
        var refs = new double[nv]; var payload = new double[nv]; var memory = new bool[nv];
        var interfere = new bool[nv, nv];
        var across = new List<(SyntaxNode call, List<int> vars, bool proven)>();
        for (int b = 0; b < nb; b++)
        {
            var live = new System.Collections.BitArray(liveOut[b]);
            for (int k = events[b].Count - 1; k >= 0; k--)
            {
                var e = events[b][k];
                if (e.Kind == 2)
                {
                    var l = new List<int>();
                    for (int v = 0; v < nv; v++) if (live[v] && (!e.Alloc || fa._allocSpill.Contains(fa._vars[v].Name))) { if (e.Rule) payload[v] += 2 * e.Weight; l.Add(v); }
                    if (l.Count > 0 && e.Real) across.Add((e.Call, l, e.Proven));
                    continue;
                }
                int x = fa._index[e.Sym];
                refs[x] += e.Weight;
                if (e.Kind == 1)
                {
                    for (int v = 0; v < nv; v++) if (v != x && live[v]) { interfere[v, x] = true; interfere[x, v] = true; }
                    live[x] = false;
                }
                else
                {
                    live[x] = true;
                    // a value read inside a handler was live on entry to it (it is never both written and read there
                    // in the cases that matter): memory
                    if (inHandler[b]) memory[x] = true;
                }
            }
        }
        // parameters are written at entry: they interfere with every variable live there
        for (int v = 0; v < nv; v++)
            if (fa._vars[v] is IParameterSymbol)
                for (int u = 0; u < nv; u++) if (u != v && liveIn[cfg.Blocks[0].Ordinal][u]) { interfere[u, v] = true; interfere[v, u] = true; }
        for (int v = 0; v < nv; v++)
        {
            var s = fa._vars[v];
            if (addressTaken.Contains(s)) memory[v] = true;
            if (s is IParameterSymbol && refs[v] <= 1) memory[v] = true;
            if (refs[v] - payload[v] < 1) memory[v] = true;
        }
        // the real JIT's own allocation where a reference run ran the method (config `jitFloatAlloc`,
        // gen_jit_float_alloc.py: the CLR's variable locations of that run): it replaces the rule for the locals it
        // names; a float local of the method that is not an IL local (ILSpy made it from an evaluation-stack value)
        // is unrounded and kept across calls (spilled as a qword: AI_065_Butterflies num22)
        var forced = new bool[nv];
        var facts = m.JitFloatAlloc(fa._caller);
        if (facts != null)
        {
            var declared = new HashSet<string>(StringComparer.Ordinal);
            var inits = new Dictionary<ISymbol, IOperation>(SymbolEqualityComparer.Default);
            foreach (var d in root.DescendantsAndSelf())
            {
                if (d is IVariableDeclaratorOperation vd)
                {
                    declared.Add(vd.Symbol.Name);
                    if (vd.GetVariableInitializer()?.Value is { } iv) inits[vd.Symbol] = iv;
                }
                // `out float dmg` (a declaration expression), catch / foreach variables
                else if (d is ILocalReferenceOperation lr) declared.Add(lr.Local.Name);
            }
            if (!facts.Keys.Where(k => k != "*").All(declared.Contains)) Interlocked.Increment(ref m.JitFloatAllocMismatch);
            else
            {
                Interlocked.Increment(ref m.JitFloatAllocMethods);
                fa.FromJit = true;
                for (int v = 0; v < nv; v++)
                {
                    if (fa._vars[v] is not ILocalSymbol ls || addressTaken.Contains(ls)) continue;
                    if (ls.DeclaringSyntaxReferences.FirstOrDefault()?.GetSyntax().Ancestors().Any(a => a is AnonymousFunctionExpressionSyntax or LocalFunctionStatementSyntax) == true) continue;
                    if (facts.TryGetValue(ls.Name, out var kind))
                    {
                        if (kind == "memory") { memory[v] = true; Interlocked.Increment(ref m.JitFloatAllocMemory); }
                        // no location in the debug info and tracked: a local the JIT keeps on the x87 stack without
                        // reporting it, as a "stack" local (rounded where it is spilled across a call, `fstp dword`:
                        // Projectile.AI_186_PrincessWeapon num7 around Utils.Remap; never stored otherwise:
                        // NextHorizonRenderer.OriginalColorsForCloud). gen_jit_float_alloc.py 'temp'; FLOAT_RULE.md
                        // section 10, item 13
                        else if (kind == "temp") { memory[v] = false; forced[v] = true; Interlocked.Increment(ref m.JitFloatAllocStack); }
                        else if (kind == "stack") { memory[v] = false; forced[v] = true; Interlocked.Increment(ref m.JitFloatAllocStack); }
                    }
                    // an evaluation-stack value spilled across a call: a float-typed temporary (`fstp dword`) when the
                    // value is a float call result, optionally negated (LightningGenerator.GenerateBolt `num7 = 0f -
                    // Vector2.Dot(..)` across Math.Min / Math.Max: `fchs; fstp dword [ebp-0x8c]`), a double one
                    // otherwise (arithmetic: AI_065_Butterflies num22 `(float)Next(..) * 0.01f`, `fstp qword`):
                    // FLOAT_RULE.md section 10, item 17
                    else if (inits.TryGetValue(ls, out var iv) && FloatCallValue(iv)) { memory[v] = false; forced[v] = true; Interlocked.Increment(ref m.JitFloatAllocTemps); }
                    else { memory[v] = false; forced[v] = true; fa.Kept.Add(ls); Interlocked.Increment(ref m.JitFloatAllocTemps); }
                }
            }
        }
        if (extraStack != null)
            for (int v = 0; v < nv; v++)
                if (extraStack.Contains(fa._vars[v])) { memory[v] = false; forced[v] = true; }
        // (the "64 hottest locals" limit needs the reference counts of the non-float locals too: approximated by
        // the float variables alone, i.e. only methods with more than 64 float variables are affected)
        var order = Enumerable.Range(0, nv).Where(v => !memory[v] && !forced[v]).OrderByDescending(v => refs[v]).ThenBy(v => fa._vars[v] is IParameterSymbol p ? p.Ordinal - 100000 : fa._vars[v].Locations.FirstOrDefault()?.SourceSpan.Start ?? 0).ToList();
        if (order.Count > 64) foreach (var v in order.Skip(64)) memory[v] = true;
        var regs = new List<int>[6];
        for (int r = 0; r < 6; r++) regs[r] = new List<int>();
        var onStack = new bool[nv];
        // the JIT's stack variables hold their registers first
        for (int v = 0; v < nv; v++)
        {
            if (!forced[v]) continue;
            onStack[v] = true;
            for (int r = 0; r < 6; r++) if (regs[r].All(h => !interfere[h, v])) { regs[r].Add(v); break; }
        }
        foreach (int v in order.Take(64))
        {
            bool placed = false;
            for (int r = 0; r < 6 && !placed; r++)
                if (regs[r].All(h => !interfere[h, v])) { regs[r].Add(v); placed = true; }
            if (placed) onStack[v] = true;
            else
            {
                memory[v] = true;
                // a tie with a variable that did get a register: the JIT's choice is not determined (FLOAT_RULE.md 4.1)
                if (order.Any(u => onStack[u] && refs[u] == refs[v] && interfere[u, v])) fa.Undetermined = true;
            }
        }
        // locals only become unrounded variables in the generated code; a parameter on the stack holds a register
        // (it competes above) but is never written without rounding in the cases probed, except in-place updates
        for (int v = 0; v < nv; v++) if (onStack[v] && (fa._vars[v] is ILocalSymbol || extraStack?.Contains(fa._vars[v]) == true)) fa.Stack.Add(fa._vars[v]);
        foreach (var (call, vars, proven) in across)
        {
            var l = vars.Where(v => onStack[v] && (fa._vars[v] is ILocalSymbol || extraStack?.Contains(fa._vars[v]) == true) && !fa.Kept.Contains(fa._vars[v])).Select(v => fa._vars[v]).ToList();
            if (l.Count == 0) continue;
            if (proven) fa.ProvenCalls.Add(call);
            if (!fa.Spill.TryGetValue(call, out var have)) fa.Spill[call] = have = new List<ISymbol>();
            foreach (var s in l) if (!have.Contains(s, SymbolEqualityComparer.Default)) have.Add(s);
        }
        return fa;
    }

    /// A float value the JIT types as float on its evaluation stack: the result of a call that returns a float (not an
    /// x87 intrinsic: Math.Abs(float) is `fabs`, an arithmetic node; Math.Min / Max (float) are calls), possibly negated (`0f - x`, `-x`).
    static bool FloatCallValue(IOperation op)
    {
        while (op is IConversionOperation { IsImplicit: true } c) op = c.Operand;
        if (op is IParenthesizedOperation p) return FloatCallValue(p.Operand);
        if (op is IUnaryOperation { OperatorKind: UnaryOperatorKind.Minus } u) return FloatCallValue(u.Operand);
        if (op is IBinaryOperation { OperatorKind: BinaryOperatorKind.Subtract } b && b.LeftOperand.ConstantValue is { HasValue: true, Value: float f } && f == 0f && BitConverter.SingleToInt32Bits(f) == 0)
            return FloatCallValue(b.RightOperand);
        if (op is IInvocationOperation inv)
            return inv.TargetMethod.ReturnType.SpecialType == SpecialType.System_Single && !(inv.TargetMethod.ContainingType.ToDisplayString() == "System.Math" && inv.TargetMethod.Name == "Abs");
        return false;
    }
}
