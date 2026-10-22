// Emitter: one method body (Roslyn IOperation tree) -> Rust statements. Rules: DESIGN.md.
using System.Collections.Immutable;
using System.Globalization;
using System.Text;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Operations;

namespace Cs2Rs;

public sealed class Emitter
{
    readonly Model M;
    readonly TType Owner;
    readonly bool OwnerIsStruct;
    readonly HashSet<string> _ownerConsts;
    StringBuilder sb = new();
    int ind = 1;
    int _tmp, _lbl;
    public readonly List<(string kind, string where)> Fails = new();
    /// Emitting the resumable form of a method (DESIGN.md section 8f): the body runs inside an `async fn`; `try` is
    /// the config's async try, `Thread.Sleep` and calls of other resumable methods are awaited.
    public bool Resumable;
    /// The `__x87` twin of a float method (Model.TwinF64): its own `return expr` gives the unrounded f64 value.
    public bool TwinF64;
    /// A `__vcN` twin (Model.VecCtxTwin): the answers of jit_vec_ctx.tsv for the Vector2 operators of this body, as
    /// in the caller that inlined it: operator (`op_Name(params)`) -> 'c' (call) / 'i' (inlined); VecCtxMode "stack" /
    /// "memory" for the inlined ones.
    public Dictionary<string, char> VecCtx;
    public string VecCtxMode;
    /// A `__u` twin (Model.ArgTwinParams): these float parameters hold the caller's unrounded x87 value (the JIT
    /// inlined the method and substituted the caller's local for them); they are f64 variables like a "stack" local,
    /// rounded at every call they are live across. FLOAT_RULE.md section 10, item 26.
    public HashSet<ISymbol> WideParams;
    public SortedSet<string> Unported = new(StringComparer.Ordinal);
    public readonly SortedSet<string> Stubs = new(StringComparer.Ordinal);
    public SortedSet<string> HostCalls = new(StringComparer.Ordinal);
    /// `void` callees that are not translated (`rt::gap`: logged, execution continues). Also in `Unported`.
    public SortedSet<string> Gaps = new(StringComparer.Ordinal);
    /// Translated functions this body calls (Rust paths), static initialisers it can trigger (`mod::cctor`),
    /// field initialisers (`mod::init_fields`): the edges of the reachability report (`--reach`).
    public SortedSet<string> Calls = new(StringComparer.Ordinal);
    /// The same four sets for code in branches that the config's `reachAssume` proves dead (`if (!Main.dedServ)`
    /// with `Main.dedServ` assumed true). Used ONLY by the reachability report; the Rust text is the same.
    public readonly SortedSet<string> DeadUnported = new(StringComparer.Ordinal), DeadHostCalls = new(StringComparer.Ordinal),
        DeadGaps = new(StringComparer.Ordinal), DeadCalls = new(StringComparer.Ordinal);
    int _dead;

    /// Emits `body`; what it reaches is recorded as dead (see `DeadCalls`).
    void Dead(bool dead, Action body)
    {
        if (!dead || _dead > 0) { body(); return; }
        var (u, h, g, c) = (Unported, HostCalls, Gaps, Calls);
        (Unported, HostCalls, Gaps, Calls) = (DeadUnported, DeadHostCalls, DeadGaps, DeadCalls);
        _dead++;
        try { body(); }
        finally { _dead--; (Unported, HostCalls, Gaps, Calls) = (u, h, g, c); }
    }

    /// Three-valued value of a condition under the config's `reachAssume` (null: unknown).
    bool? Assumed(IOperation c)
    {
        if (M.Cfg.ReachAssume.Count == 0 || c == null) return null;
        while (c is IParenthesizedOperation p) c = p.Operand;
        if (c.ConstantValue.HasValue && c.ConstantValue.Value is bool cb) return cb;
        switch (c)
        {
            case IFieldReferenceOperation f: return M.Cfg.ReachAssume.TryGetValue(M.DocId(f.Field.OriginalDefinition), out var v) ? v : null;
            case IPropertyReferenceOperation pr: return M.Cfg.ReachAssume.TryGetValue(M.DocId(pr.Property.OriginalDefinition), out var pv) ? pv : null;
            case IUnaryOperation { OperatorKind: UnaryOperatorKind.Not } u: return !Assumed(u.Operand);
            case IBinaryOperation { OperatorKind: BinaryOperatorKind.ConditionalAnd } b:
                { var l = Assumed(b.LeftOperand); var r = Assumed(b.RightOperand); return l == false || r == false ? false : l == true && r == true ? true : null; }
            case IBinaryOperation { OperatorKind: BinaryOperatorKind.ConditionalOr } b:
                { var l = Assumed(b.LeftOperand); var r = Assumed(b.RightOperand); return l == true || r == true ? true : l == false && r == false ? false : null; }
        }
        return null;
    }
    static bool NoFallThrough(IOperation s)
    {
        while (s is IBlockOperation b) { if (b.Operations.Length == 0) return false; s = b.Operations[^1]; }
        return s is IReturnOperation { Kind: OperationKind.Return } or IThrowOperation || s is IExpressionStatementOperation { Operation: IThrowOperation };
    }
    /// After this statement the rest of its statement list cannot run under `reachAssume`.
    bool EndsListUnderAssumption(IOperation s) => s is IConditionalOperation c && (Assumed(c.Condition) == true && NoFallThrough(c.WhenTrue) || Assumed(c.Condition) == false && c.WhenFalse != null && NoFallThrough(c.WhenFalse));
    /// One entry per host call site; committed to the host API only when the body is emitted (Gen.Finish).
    public readonly List<HostFn> HostHits = new();

    sealed class Jump { public string Code; public int Depth; }
    sealed class TryFrame { public List<Jump> Exits = new(); public bool Returns; }
    Dictionary<ILabelSymbol, Jump> _jumps = new(SymbolEqualityComparer.Default);
    List<TryFrame> _tries = new();
    HashSet<ILabelSymbol> _gotoTargets = new(SymbolEqualityComparer.Default);
    HashSet<ISymbol> _mutated;
    IOperation _root;
    string _retType = "()";           // Rust return type of the function / lambda being emitted
    List<string> _pendingDecls = new();
    readonly Stack<(string name, bool isStruct)> _implicit = new();
    readonly Stack<string> _condAccess = new();
    readonly Stack<string> _catchVars = new();

    public Emitter(Model m, TType owner)
    {
        M = m; Owner = owner; OwnerIsStruct = owner.IsStruct;
        _ownerConsts = new HashSet<string>(owner.Consts.Select(c => c.Name));
    }

    /// An operand that is already Rust text (a temporary holding a receiver, the resource of a `using`).
    /// Lets the call machinery, which works on IOperation, be used for calls the emitter synthesises.
    sealed class Rendered : IOperation
    {
        public string Text; public bool Place;
        public ITypeSymbol Type { get; set; }
        public IOperation Parent => null;
        public OperationKind Kind => OperationKind.None;
        public SyntaxNode Syntax { get; set; }
        public Optional<object> ConstantValue => default;
        public IEnumerable<IOperation> Children => Array.Empty<IOperation>();
        public IOperation.OperationList ChildOperations => default;
        public string Language => LanguageNames.CSharp;
        public bool IsImplicit => true;
        public SemanticModel SemanticModel => null;
        public void Accept(OperationVisitor visitor) { }
        public TResult Accept<TArgument, TResult>(OperationVisitor<TArgument, TResult> visitor, TArgument argument) => default;
    }
    Rendered R(string text, ITypeSymbol type, IOperation at, bool place = false) => new Rendered { Text = text, Type = type, Syntax = at?.Syntax, Place = place };

    // ------------------------------------------------------------------ plumbing
    /// A loop header (`'l3: loop`, `'l3: while ..`) was the last line: the `{` that follows gets the yield point
    /// (DESIGN.md "Loading screens": where work on a worker fiber can give the frame back).
    bool _loopHead;
    void Line(string s)
    {
        sb.Append(' ', ind * 4).Append(s).Append('\n');
        if (_loopHead && s == "{") sb.Append(' ', (ind + 1) * 4).Append("rt::yield_point();\n");
        _loopHead = s.Length > 3 && s[0] == '\'' && (s.EndsWith(": loop") || s.Contains(": while "));
    }
    string Tmp(string p = "t") => "__" + p + (++_tmp);
    static string Where(IOperation op)
    {
        var loc = op?.Syntax?.GetLocation();
        if (loc == null || !loc.IsInSource) return "?";
        var ls = loc.GetLineSpan();
        return Path.GetFileName(ls.Path) + ":" + (ls.StartLinePosition.Line + 1);
    }
    string Fail(IOperation op, string kind)
    {
        Fails.Add((kind, Where(op)));
        return "rt::bad()";
    }
    string Unp(string name, string rustType)
    {
        Unported.Add(name);
        return $"rt::unported::<{rustType ?? "()"}>({Lit(name)})";
    }
    static string Lit(string s)
    {
        var b = new StringBuilder("\"");
        foreach (var c in s)
        {
            switch (c)
            {
                case '"': b.Append("\\\""); break;
                case '\\': b.Append("\\\\"); break;
                case '\n': b.Append("\\n"); break;
                case '\r': b.Append("\\r"); break;
                case '\t': b.Append("\\t"); break;
                case '\0': b.Append("\\0"); break;
                default:
                    if (c < 0x20 || c == 0x7f || (c >= 0xD800 && c <= 0xDFFF) || c == '\u2028' || c == '\u2029') b.Append("\\u{").Append(((int)c).ToString("x")).Append('}');
                    else b.Append(c);
                    break;
            }
        }
        return b.Append('"').ToString();
    }
    static bool HasLoneSurrogate(string s)
    {
        for (int i = 0; i < s.Length; i++)
        {
            if (char.IsHighSurrogate(s[i]) && i + 1 < s.Length && char.IsLowSurrogate(s[i + 1])) { i++; continue; }
            if (char.IsSurrogate(s[i])) return true;
        }
        return false;
    }
    static string StrLit(string s)
    {
        // surrogate pairs must be emitted as one scalar
        var b = new StringBuilder("s!(\"");
        for (int i = 0; i < s.Length; i++)
        {
            char c = s[i];
            if (char.IsHighSurrogate(c) && i + 1 < s.Length) { b.Append("\\u{").Append(char.ConvertToUtf32(c, s[i + 1]).ToString("x")).Append('}'); i++; continue; }
            switch (c)
            {
                case '"': b.Append("\\\""); break;
                case '\\': b.Append("\\\\"); break;
                case '\n': b.Append("\\n"); break;
                case '\r': b.Append("\\r"); break;
                case '\t': b.Append("\\t"); break;
                case '\0': b.Append("\\0"); break;
                default:
                    if (c < 0x20 || c == 0x7f || c == '\u2028' || c == '\u2029' || c == '\ufeff') b.Append("\\u{").Append(((int)c).ToString("x")).Append('}');
                    else b.Append(c);
                    break;
            }
        }
        return b.Append("\")").ToString();
    }

    string RTf(IOperation at, ITypeSymbol t)
    {
        var r = M.RT(t);
        if (r == null) { Fail(at, "type with no Rust representation: " + (t == null ? "?" : KindOfType(t))); return "()"; }
        return r;
    }
    static string KindOfType(ITypeSymbol t) => t.SpecialType == SpecialType.System_Decimal ? "decimal"
        : t is IPointerTypeSymbol ? "pointer" : t is IArrayTypeSymbol ? "array rank>2 / of unmapped element"
        : t.TypeKind == TypeKind.Struct ? "struct not translated or mapped (" + Model.Short(t) + ")" : t.TypeKind.ToString();

    static string Norm(string rt) => rt == "Obj" ? "Gc<Object>" : rt;
    static bool IsGc(string rt) => rt == "Obj" || rt.StartsWith("Gc<");
    static string Inner(string rt) { rt = Norm(rt); return rt.Substring(3, rt.Length - 4); }

    string Zero(ITypeSymbol t)
    {
        t = Mono.Sub(t);
        switch (t.SpecialType)
        {
            case SpecialType.System_Boolean: return "false";
            case SpecialType.System_Single: return "0.0f32";
            case SpecialType.System_Double: return "0.0f64";
        }
        if (Model.Prim(t.SpecialType) != null) return "0";
        if (t.TypeKind == TypeKind.Enum) return "0";
        var zr = M.RT(t);
        return zr == null ? "rt::zero()" : $"rt::zero::<{zr}>()";
    }

    string LocalName(ISymbol l)
    {
        var n = Names.Ident(l.Name);
        // C# discards as lambda parameters (`(_, _) => ...`): Rust's `_` is a pattern, not a name
        if (n.Trim('_').Length == 0 && l is IParameterSymbol dp) return "_d" + dp.Ordinal;
        if (n == "this" || n == "rt" || _ownerConsts.Contains(n)) n += "_";
        return n;
    }

    // ------------------------------------------------------------------ entry points
    public string ConstText(object v, ITypeSymbol t) => Const(v, t, true);
    public string BoxOf(ITypeSymbol t, string e) => Box(t, e, null);
    /// `(T)o` for reflection setters: an unboxing, a checked downcast to a translated class, or the object itself.
    /// Null (or a recorded failure) when the type has no rule.
    public string UnboxOf(ITypeSymbol t, string e)
    {
        t = Mono.Sub(t);
        var r = M.RT(t);
        if (r == null) return null;
        if (r == "Obj") return e;
        var tt = M.Get(t);
        if (tt != null && tt.IsClass && tt.TypeId != 0) return $"rt::cast_class::<Object, {tt.StructPath}>({e}, {tt.ModPath}::TYPE_ID, &types::PARENT)";
        if (IsGc(r) && !IsRtCast(r, t)) return null;
        return Unbox(t, e, null, false);
    }
    /// A constant of an attribute application as a Rust expression.
    public string TypedConst(TypedConstant c)
    {
        switch (c.Kind)
        {
            case TypedConstantKind.Primitive: case TypedConstantKind.Enum: return Const(c.Value, c.Type, true);
            case TypedConstantKind.Type: return c.Value is ITypeSymbol ts ? TypeOfExpr(ts) : "Gc::NULL";
            case TypedConstantKind.Array:
                {
                    if (c.IsNull) return "rt::zero()";
                    var et = M.RT(((IArrayTypeSymbol)c.Type).ElementType);
                    if (et == null) { Fails.Add(("attribute argument of an unrepresentable array type", "?")); return "rt::bad()"; }
                    return $"Arr::<{et}>::from_slice(&[{string.Join(", ", c.Values.Select(TypedConst))}])";
                }
        }
        Fails.Add(("attribute argument kind " + c.Kind, "?"));
        return "rt::bad()";
    }
    /// Identifier of the reflection data of a type in `gen::meta`.
    public static string MetaIdent(string name) => Names.Ident(Names.Snake(name.Replace('.', '_').Replace('+', '_').Replace('<', '_').Replace('>', '_').Replace(',', '_').Replace(" ", "").Replace('[', '_').Replace(']', '_'))).ToUpperInvariant();
    /// Inside reflection data: `typeof` of a field's type is the plain Type object (the same object by name), so
    /// that data of one type does not pull in the data of every type its fields mention.
    public bool NoMeta;
    public string StaticsPath = null;

    /// `: base(args)` of a native base class: the constructor template makes the native value, which moves into
    /// the embedded slot (`rt::NativeSlot`).
    public string NativeBaseInit(IMethodSymbol ctor, IList<string> args, IOperation at)
    {
        var nb = M.NativeBaseOf(Owner);
        if (ctor == null || nb == null || NativeTemplate(ctor.OriginalDefinition) is not { Get: { } tmpl })
        {
            Fails.Add(("constructor of the native base class has no template (" + (ctor == null ? "?" : M.DocId(ctor.OriginalDefinition)) + ")", Where(at)));
            return "rt::bad()";
        }
        var nbr = M.RT(nb);
        return $"rt::NativeSlot::init(this as *mut rt::NativeSlot<{Inner(nbr)}>, {Subst(tmpl, at, null, args, null, ctor, nb)})";
    }

    void Prepare(IOperation root, string retType)
    {
        _root = root; _retType = retType; _mutated = null; _cells = null; _wide = null;
        _vecComp.Clear(); _vecAddr = null; _vecStack = null; _vecClearAfter = false;
        if (root == null) return;
        foreach (var d in root.DescendantsAndSelf())
            if (d is IBranchOperation { BranchKind: BranchKind.GoTo } b) _gotoTargets.Add(b.Target);
    }

    /// For the reflection data (`rt::MethodMeta::bind`): a delegate of the method's own signature bound to the
    /// object `o` (an `Obj` in scope), as `object`. Null when the signature has no delegate form.
    public string MethodDelegateObj(IMethodSymbol m, TType t)
    {
        if (m.Parameters.Any(p => p.RefKind != RefKind.None || M.RT(p.Type) == null)) return null;
        var ret = m.ReturnsVoid ? "()" : M.RT(m.ReturnType);
        if (ret == null) return null;
        var names = m.Parameters.Select((p, k) => $"__a{k}").ToList();
        var sig = $"dyn Fn({string.Join(", ", m.Parameters.Select(p => M.RT(p.Type)))}){(ret == "()" ? "" : " -> " + ret)}";
        string recvAddr = "0", tracer = "None", cap = "";
        IOperation recv = null;
        if (!m.IsStatic)
        {
            if (t.IsStruct) return null;
            cap = $"let __t: Gc<{t.StructPath}> = Gc(o.0 as *mut {t.StructPath}); ";
            recv = R("__t", t.Sym, null);
            recvAddr = "__t.addr()";
            tracer = "Some(Box::new(move |__m: &mut rt::Marker| __m.mark(__t)))";
        }
        var call = CallMethod(m, recv, names, null, false, null);
        ulong h = 14695981039346656037UL;
        foreach (var ch in M.DocId(m.OriginalDefinition) + "|" + Mono.Key(Mono.Sub(m.ContainingType)) + "|") { h ^= ch; h *= 1099511628211UL; }
        M.MethodNames.TryAdd(h | 1, Gen.ClrName(Mono.Sub(m.ContainingType)) + "." + m.Name);
        return $"{{ {cap}Delegate::<{sig}>::from_method(Box::new(move |{string.Join(", ", names)}| unsafe {{ {call} }}), 0x{(h | 1):x}u64, {recvAddr}, {tracer}).up::<Object>() }}";
    }

    /// Element type (Rust) of the iterator method being emitted, or null. DESIGN.md section 8d.
    string _iterElem;

    /// `yield` directly in this body (not in a lambda or local function)?
    static bool IsIteratorBody(IOperation root)
    {
        bool Walk(IOperation o)
        {
            foreach (var c in o.ChildOperations)
            {
                if (c is IAnonymousFunctionOperation or ILocalFunctionOperation) continue;
                if (c is IReturnOperation { Kind: OperationKind.YieldReturn or OperationKind.YieldBreak }) return true;
                if (Walk(c)) return true;
            }
            return false;
        }
        return root is IMethodBodyOperation && Walk(root);
    }

    /// An iterator method: the body becomes the state machine of `rt::iter` (an `async` block; `yield return v` is
    /// `__y.put(v).await`). `IEnumerable<T>` gives a sequence that starts the body again for every enumeration,
    /// `IEnumerator` / `IEnumerator<T>` one enumerator object.
    void IteratorBody(IOperation root, string retType)
    {
        var rt = Mono.Sub(Method.ReturnType) as INamedTypeSymbol;
        string kind = null; ITypeSymbol elem = null;
        if (rt != null)
        {
            var od = rt.OriginalDefinition.ToDisplayString();
            if (od == "System.Collections.Generic.IEnumerable<T>") { kind = "enumerable"; elem = rt.TypeArguments[0]; }
            else if (od == "System.Collections.Generic.IEnumerator<T>") { kind = "enumerator"; elem = rt.TypeArguments[0]; }
            else if (od == "System.Collections.IEnumerator") kind = "enumerator";
        }
        if (kind == null) { Fail(root, "iterator method returning " + (rt == null ? "?" : Model.Short(rt))); return; }
        var er = elem == null ? "Obj" : M.RT(elem);
        if (er == null) { Fail(root, "iterator element type with no Rust representation"); return; }
        if ((kind == "enumerable") != retType.StartsWith("Seq<")) { Fail(root, "iterator method whose result type is " + retType); return; }
        var ps = Method.Parameters.Where(p => M.RT(p.Type) != null).ToList();
        if (Method.Parameters.Any(p => p.RefKind != RefKind.None)) { Fail(root, "iterator method with a ref / out parameter"); return; }
        Line($"return rt::iter::{kind}::<{er}, _>(move |__y: rt::iter::Yielder<{er}>| async move {{ unsafe {{");
        ind++;
        // the state machine owns its copies of the arguments (they are locals of the body from here on)
        foreach (var p in ps) Line($"let mut {LocalName(p)} = {LocalName(p)};");
        var saved = (_iterElem, _retType);
        _iterElem = er; _retType = "()";
        foreach (var c in Cells().OfType<IParameterSymbol>().Where(p => !IsLambdaParam(p)).OrderBy(p => p.Ordinal))
            Line($"let mut {LocalName(c)}: Gc<{M.RT(c.Type)}> = rt::new_cell::<{M.RT(c.Type)}>({LocalName(c)});");
        var mb = (IMethodBodyOperation)root;
        Top(mb.BlockBody ?? mb.ExpressionBody);
        Flush();
        (_iterElem, _retType) = saved;
        ind--;
        if (kind == "enumerator") Line("} });");
        else
        {
            // what the factory holds between enumerations: the arguments and the receiver
            var marks = ps.Select(p => $"rt::Trace::trace(&{LocalName(p)}, __m);").ToList();
            if (!Method.IsStatic) marks.Insert(0, OwnerIsStruct ? "" : "__m.mark(Gc(this));");
            Line($"}} }}, move |__m: &mut rt::Marker| {{ unsafe {{ {string.Join(" ", marks.Where(x => x != ""))} }} }});");
        }
    }

    /// Method / accessor / constructor body (may be called after AssignInit lines).
    public void AppendBody(IOperation root, string retType)
    {
        Prepare(root, retType);
        if (Method != null && IsIteratorBody(root))
        {
            if (Resumable) { Fail(root, "resumable form of an iterator method"); return; }
            IteratorBody(root, retType); return;
        }
        foreach (var c in Cells().OfType<IParameterSymbol>().Where(p => !IsLambdaParam(p)).OrderBy(p => p.Ordinal))
            Line($"let mut {LocalName(c)}: Gc<{M.RT(c.Type)}> = rt::new_cell::<{M.RT(c.Type)}>({LocalName(c)});");
        if (root is IMethodBodyOperation mb) Top(mb.BlockBody ?? mb.ExpressionBody);
        else if (root is IConstructorBodyOperation cb)
        {
            if (cb.Initializer != null) S(cb.Initializer);
            Top(cb.BlockBody ?? cb.ExpressionBody);
        }
        else if (root is IBlockOperation) Top(root);
        else if (root != null) { var e = TwinF64 && retType == "f64" && IsF32(root) ? F64(root) : E(root); Flush(); Line(retType == "()" ? (root.Type == null || root.Type.SpecialType == SpecialType.System_Void ? e + ";" : "let _ = " + e + ";") : "return " + e + ";"); }
    }
    static bool IsLambdaParam(IParameterSymbol p) => p.ContainingSymbol is IMethodSymbol { MethodKind: MethodKind.AnonymousFunction or MethodKind.LocalFunction };

    void Top(IOperation block)
    {
        if (block is IBlockOperation b) StmtList(b.Operations);
        else if (block != null) S(block);
    }
    /// `place = <initialiser>;`
    public void AssignInit(string place, IOperation value)
    {
        Prepare(value, "()");
        var e = E(value); Flush(); Line($"{place} = {e};");
    }
    public string Finish(string retType)
    {
        Flush();
        if (retType != "()") Line("rt::unreachable()");
        return sb.ToString();
    }
    public void Raw(string line) => Line(line);

    void Flush()
    {
        foreach (var d in _pendingDecls) Line(d);
        _pendingDecls.Clear();
    }

    // ------------------------------------------------------------------ statements
    void S(IOperation op, StringBuilder declSink = null)
    {
        var outer = sb;
        sb = new StringBuilder();
        var savedDecls = _pendingDecls; _pendingDecls = new List<string>();
        if (M.Cfg.FloatMode == "jit32" && op?.Syntax != null && op is IExpressionStatementOperation or IVariableDeclarationGroupOperation or IReturnOperation or IConditionalOperation)
        {
            Wide();
            if (_spillAt != null && _spillAt.TryGetValue(op.Syntax, out var spill))
                // only a local declared before the statement (a liveness that reaches back before the declaration,
                // `float y; ... num4 = (y = 0f)` in Main.rgbToHsl, has nothing to round there)
                foreach (var v in spill) if (_wide.Contains(v) && DeclaredBefore(v, op.Syntax)) Line($"{LocalName(v)} = {LocalName(v)} as f32 as f64; // x87 stack variable live across a call: spilled as a float");
        }
        bool vecCtl = _vecComp.Count > 0 || VecStackMethod();
        if (vecCtl) VecBeforeStatement(op);
        try { S0(op); }
        catch (Exception ex) when (ex is not OutOfMemoryException) { Fail(op, "internal error: " + ex.GetType().Name + " " + ex.Message.Split('\n')[0]); }
        if (vecCtl && op is not (IExpressionStatementOperation or IVariableDeclarationGroupOperation or IReturnOperation)) _vecComp.Clear();
        // a call statement whose arguments read tracked components (VecArgsBeforeCall): the call ends the tracking
        if (_vecClearAfter) { _vecComp.Clear(); _vecClearAfter = false; }
        var text = sb; sb = outer;
        var decls = _pendingDecls; _pendingDecls = savedDecls;
        if (declSink != null) { foreach (var d in decls) declSink.Append(' ', ind * 4).Append(d).Append('\n'); }
        else foreach (var d in decls) Line(d);
        sb.Append(text);
    }

    void StmtList(ImmutableArray<IOperation> ops)
    {
        if (ops.Any(o => o is ILabeledOperation)) { Segmented(ops.Select(o => (o, (ISwitchCaseOperation)null)).ToList(), null); return; }
        bool dead = false;
        foreach (var o in ops)
        {
            if (dead) Dead(true, () => S(o)); else S(o);
            if (!dead && _dead == 0 && EndsListUnderAssumption(o)) dead = true;
        }
    }

    void Block(IOperation body)
    {
        // body of if / loop: always braces
        Line("{"); ind++;
        if (body is IBlockOperation b) StmtList(b.Operations); else if (body != null) S(body);
        ind--; Line("}");
    }

    void S0(IOperation op)
    {
        switch (op)
        {
            case null: return;
            case IBlockOperation b: Block(b); return;
            case IEmptyOperation: return;
            case IExpressionStatementOperation es: ExprStmt(es.Operation); return;
            case IVariableDeclarationGroupOperation g: Decls(g, null); return;
            case IConditionalOperation c when ElseIfChain(c) >= 5:
                {
                    // A long `if / else if / else if ...` chain (NPC.AI, Projectile.AI: hundreds of links) is nested
                    // hundreds of levels deep when written as `else { if .. }`, and rustc's front end is quadratic in
                    // that depth. Same control flow, flat: each link leaves the labelled block when it ran.
                    int n = ++_lbl;
                    Line($"'if{n}:"); Line("{"); ind++;
                    IOperation cur = c;
                    while (cur is IConditionalOperation link)
                    {
                        var lc = E(link.Condition); Flush();
                        Line($"if {lc}");
                        Line("{"); ind++;
                        Dead(Assumed(link.Condition) == false, () => { if (link.WhenTrue is IBlockOperation tb) StmtList(tb.Operations); else if (link.WhenTrue != null) S(link.WhenTrue); });
                        Line($"break 'if{n};");
                        ind--; Line("}");
                        cur = link.WhenFalse;
                    }
                    if (cur is IBlockOperation eb) StmtList(eb.Operations); else if (cur != null) S(cur);
                    ind--; Line("}");
                    return;
                }
            case IConditionalOperation c:
                {
                    var cond = E(c.Condition); Flush();
                    Line($"if {cond}");
                    var known = Assumed(c.Condition);
                    Dead(known == false, () => Block(c.WhenTrue));
                    if (c.WhenFalse != null)
                    {
                        Line("else");
                        Dead(known == true, () => Block(c.WhenFalse));
                    }
                    return;
                }
            case IWhileLoopOperation w: While(w); return;
            case IForLoopOperation f: For(f); return;
            case IForEachLoopOperation fe: ForEach(fe); return;
            case IReturnOperation r:
                if (r.Kind != OperationKind.Return)
                {
                    // DESIGN.md section 8d: the body runs inside an `async` block (rt::iter)
                    if (_iterElem == null) { Fail(op, "yield outside an iterator method body"); return; }
                    if (_tries.Count > 0) { Fail(op, "yield inside try (the try body is a closure, which cannot suspend)"); return; }
                    if (r.Kind == OperationKind.YieldBreak) { Flush(); Line("return;"); return; }
                    var yv = r.ReturnedValue == null ? "rt::zero()" : E(r.ReturnedValue);
                    Flush(); Line($"__y.put({yv}).await;");
                    return;
                }
                // the __x87 twin of a float method: the value its caller sees in ST0, unrounded
                if (TwinF64 && _retType == "f64" && r.ReturnedValue != null && IsF32(r.ReturnedValue)) { var fv = F64(r.ReturnedValue); Return(fv, op); return; }
                Return(r.ReturnedValue == null ? null : E(r.ReturnedValue), op);
                return;
            case IBranchOperation br:
                if (!_jumps.TryGetValue(br.Target, out var j)) { Fail(op, "branch to unknown label (" + br.BranchKind + ")"); return; }
                Flush(); Line(JumpCode(j));
                return;
            case ILabeledOperation lab:
                // a label that is not a direct child of a statement list (e.g. the single statement of an if)
                Segmented(new List<(IOperation, ISwitchCaseOperation)> { (lab, null) }, null);
                return;
            case ISwitchOperation sw: Switch(sw); return;
            case IThrowOperation th:
                {
                    var e = Throw(th); Flush(); Line(e + ";"); return;
                }
            case ITryOperation t: Try(t); return;
            case ILockOperation lk:
                {
                    var e = E(lk.LockedValue); Flush();
                    Line($"let _ = {e};");
                    Block(lk.Body);
                    return;
                }
            case IUsingOperation us: Using(us); return;
            case IUsingDeclarationOperation: Fail(op, "using declaration (needs IDisposable dispatch)"); return;
            case ILocalFunctionOperation: Fail(op, "local function"); return;
            case IInvalidOperation: Fail(op, "binding error (IInvalidOperation)"); return;
            default:
                if (op.Syntax is FixedStatementSyntax && op.Kind == OperationKind.None) { Fixed(op); return; }
                if (op.Type != null || op is IInvocationOperation) { ExprStmt(op); return; }
                Fail(op, "statement: " + op.Kind);
                return;
        }
    }

    /// Number of conditions in an `if / else if / ...` chain.
    static int ElseIfChain(IConditionalOperation c)
    {
        int n = 1;
        while (c.WhenFalse is IConditionalOperation next) { n++; c = next; }
        return n;
    }

    void Return(string value, IOperation at)
    {
        Flush();
        if (_tries.Count > 0)
        {
            _tries[^1].Returns = true;
            Line(value == null ? "return rt::Flow::Return(());" : $"return rt::Flow::Return({value});");
        }
        else Line(value == null ? "return;" : $"return {value};");
    }

    string JumpCode(Jump j)
    {
        if (j.Depth == _tries.Count) return j.Code;
        var f = _tries[^1];
        int n = f.Exits.IndexOf(j);
        if (n < 0) { f.Exits.Add(j); n = f.Exits.Count - 1; }
        return $"return rt::Flow::Jump({n});";
    }

    // ------------------------------------------------------------------ pointers (`unsafe` code)
    // A C# pointer `T*` is a Rust `*mut T`. Arrays of the runtime never move (rt::arr: one allocation per array,
    // freed by the collector only when the array is unreachable), so `fixed` pins nothing: it only takes the address.
    // Roslyn has no public operation for the pointer forms: `*p`, the implicit dereference of `p->f`, `p[i]` and the
    // `fixed` statement are `OperationKind.None` nodes, recognised here by their children and syntax.

    /// `*p`, the receiver of `p->f` (one child, a pointer), or `p[i]` (two children).
    static bool PtrDeref(IOperation o, out IOperation ptr, out IOperation index)
    {
        ptr = index = null;
        if (o == null || o is Rendered || o.Kind != OperationKind.None || o.Type == null) return false;
        var ch = o.ChildOperations.ToList();
        if (ch.Count == 0 || ch[0].Type is not IPointerTypeSymbol pt) return false;
        if (!SymbolEqualityComparer.Default.Equals(pt.PointedAtType, o.Type)) return false;
        if (ch.Count == 1) { ptr = ch[0]; return true; }
        if (ch.Count == 2 && o.Syntax is ElementAccessExpressionSyntax) { ptr = ch[0]; index = ch[1]; return true; }
        return false;
    }
    static bool IsPtrDeref(IOperation o) => PtrDeref(o, out _, out _);

    /// Address the dereference `o` reads or writes.
    string PtrAddr(IOperation o)
    {
        PtrDeref(o, out var ptr, out var index);
        var p = E(ptr);
        return index == null ? p : $"{p}.wrapping_offset(({E(index, true)}) as isize)";
    }

    /// `fixed (T* p = &place) body` / `fixed (T* p = array) body`.
    void Fixed(IOperation op)
    {
        var ch = op.ChildOperations.ToList();
        if (ch.Count != 2 || ch[0] is not IVariableDeclarationGroupOperation g) { Fail(op, "fixed statement of an unexpected shape"); return; }
        Line("{"); ind++;
        foreach (var d in g.Declarations)
            foreach (var v in d.Declarators)
            {
                var loc = v.Symbol;
                var t = RTf(v, loc.Type);
                var init = v.GetVariableInitializer()?.Value;
                string e;
                var inner = init;
                while (inner != null && inner is not IAddressOfOperation && inner.Type is not IArrayTypeSymbol && (inner.Kind == OperationKind.None || inner is IConversionOperation) && inner.ChildOperations.Count == 1) inner = inner.ChildOperations.First();
                if (inner is IAddressOfOperation ao) e = Addr(ao.Reference);
                else if (inner?.Type is IArrayTypeSymbol { Rank: 1 }) e = $"rt::fixed_arr({E(inner)})"; // null for a null or empty array, as in C#
                else { Fail(v, "fixed initialiser that is neither an address nor an array (" + (inner?.Type == null ? "?" : Model.Short(inner.Type)) + ")"); e = "rt::bad()"; }
                Flush(); Line($"let mut {LocalName(loc)}: {t} = ({e}) as {t};");
            }
        if (ch[1] is IBlockOperation b) StmtList(b.Operations); else S(ch[1]);
        ind--; Line("}");
    }

    void Decls(IVariableDeclarationGroupOperation g, StringBuilder hoist)
    {
        foreach (var d in g.Declarations)
            foreach (var v in d.Declarators)
            {
                var init = v.GetVariableInitializer()?.Value;
                var loc = v.Symbol;
                var name = LocalName(loc);
                if (loc.RefKind != RefKind.None)
                {
                    var t0 = RTf(v, loc.Type);
                    string addr = init == null ? "core::ptr::null_mut()" : Addr(init);
                    Flush(); Line($"let mut {name}: *mut {t0} = {addr};");
                    continue;
                }
                var t = RTf(v, loc.Type);
                if (hoist == null && init != null && !IsCell(loc) && VecTrackable(null, init, loc) is ILocalSymbol)
                {
                    var pre = new StringBuilder();
                    var (vx, vy) = VecX87(init, pre, false);
                    Flush();
                    string cx = Tmp("vx"), cy = Tmp("vy");
                    Line($"{pre}let {cx}: f64 = {vx}; let {cy}: f64 = {vy};");
                    Line($"let mut {name}: {t} = terraria_core::xna::Vector2::new({cx} as f32, {cy} as f32);");
                    _vecComp[loc] = (cx, cy);
                    continue;
                }
                if (IsWide(loc))
                {
                    // "x87-locals": the local is not rounded to 32 bits
                    var we = init == null ? "0.0f64" : F64(init);
                    Flush();
                    if (hoist != null) { hoist.Append(' ', ind * 4).Append($"let mut {name}: f64 = 0.0f64;\n"); if (init != null) Line($"{name} = {we};"); }
                    else Line($"let mut {name}: f64 = {we};");
                    continue;
                }
                if (IsCell(loc))
                {
                    // captured and reassigned: a fresh heap cell each time the declaration executes
                    var e = init == null ? Zero(loc.Type) : E(init);
                    Flush();
                    if (hoist != null) { hoist.Append(' ', ind * 4).Append($"let mut {name}: Gc<{t}> = Gc::NULL;\n"); Line($"{name} = rt::new_cell::<{t}>({e});"); }
                    else Line($"let mut {name}: Gc<{t}> = rt::new_cell::<{t}>({e});");
                    continue;
                }
                if (hoist != null)
                {
                    hoist.Append(' ', ind * 4).Append($"let mut {name}: {t} = {Zero(loc.Type)};\n");
                    if (init != null) { var e = E(init); Flush(); Line($"{name} = {e};"); }
                }
                else if (init != null && init.DescendantsAndSelf().Any(d => d is ILocalReferenceOperation lr && SymbolEqualityComparer.Default.Equals(lr.Local, loc)))
                {
                    // `T x = (x = value);` (decompiler output): the variable exists (zero) before its initialiser runs
                    Flush(); Line($"let mut {name}: {t} = {Zero(loc.Type)};");
                    var e = E(init); Flush(); Line($"{name} = {e};");
                }
                else
                {
                    var e = init == null ? Zero(loc.Type) : E(init);
                    Flush(); Line($"let mut {name}: {t} = {e};");
                }
            }
    }

    void While(IWhileLoopOperation w)
    {
        int n = ++_lbl;
        if (w.ConditionIsTop)
        {
            _jumps[w.ExitLabel] = new Jump { Code = $"break 'l{n};", Depth = _tries.Count };
            _jumps[w.ContinueLabel] = new Jump { Code = $"continue 'l{n};", Depth = _tries.Count };
            bool always = w.Condition == null || (w.Condition.ConstantValue.HasValue && w.Condition.ConstantValue.Value is true && !w.ConditionIsUntil);
            if (always) Line($"'l{n}: loop");
            else
            {
                // the condition is re-evaluated each iteration: keep it inside the loop
                Line($"'l{n}: loop");
                Line("{"); ind++;
                var cond = E(w.Condition); Flush();
                Line(w.ConditionIsUntil ? $"if {cond} {{ break 'l{n}; }}" : $"if !({cond}) {{ break 'l{n}; }}");
                if (w.Body is IBlockOperation b) StmtList(b.Operations); else S(w.Body);
                ind--; Line("}");
                return;
            }
            Block(w.Body);
        }
        else
        {
            _jumps[w.ExitLabel] = new Jump { Code = $"break 'l{n};", Depth = _tries.Count };
            _jumps[w.ContinueLabel] = new Jump { Code = $"break 'c{n};", Depth = _tries.Count };
            Line($"'l{n}: loop");
            Line("{"); ind++;
            Line($"'c{n}:");
            Block(w.Body);
            if (w.Condition != null)
            {
                var cond = E(w.Condition); Flush();
                Line(w.ConditionIsUntil ? $"if {cond} {{ break 'l{n}; }}" : $"if !({cond}) {{ break 'l{n}; }}");
            }
            ind--; Line("}");
        }
    }

    void For(IForLoopOperation f)
    {
        int n = ++_lbl;
        _jumps[f.ExitLabel] = new Jump { Code = $"break 'l{n};", Depth = _tries.Count };
        _jumps[f.ContinueLabel] = new Jump { Code = $"break 'c{n};", Depth = _tries.Count };
        Line("{"); ind++;
        foreach (var b in f.Before) S(b);
        Line($"'l{n}: loop");
        Line("{"); ind++;
        if (f.Condition != null)
        {
            var cond = E(f.Condition); Flush();
            Line($"if !({cond}) {{ break 'l{n}; }}");
        }
        Line($"'c{n}:");
        Block(f.Body);
        foreach (var b in f.AtLoopBottom) S(b);
        ind--; Line("}");
        ind--; Line("}");
    }

    void ForEach(IForEachLoopOperation fe)
    {
        int n = ++_lbl;
        _jumps[fe.ExitLabel] = new Jump { Code = $"break 'l{n};", Depth = _tries.Count };
        _jumps[fe.ContinueLabel] = new Jump { Code = $"continue 'l{n};", Depth = _tries.Count };
        var coll = fe.Collection;
        while (coll is IConversionOperation { IsImplicit: true, OperatorMethod: null } cv && cv.Operand.Type != null && !cv.GetConversion().IsBoxing) coll = cv.Operand;
        if (fe.LoopControlVariable is not IVariableDeclaratorOperation vd) { Fail(fe, "foreach without a declared loop variable (deconstruction / existing variable)"); return; }
        var cr = M.RT(coll.Type);
        string collUp = null;
        if (M.Get(coll.Type) is { IsClass: true } fct && M.NativeBaseOf(fct) is { } fnb && M.RT(fnb) is { } fnr) { cr = fnr; collUp = Inner(fnr); }
        if (fe.IsAsynchronous) { Fail(fe, "await foreach"); return; }
        if (cr == null || !(cr.StartsWith("Arr<") || cr.StartsWith("Arr2<") || cr == "Str" || cr.StartsWith("Gc<List<") || cr.StartsWith("Seq<") || cr.StartsWith("Gc<Dictionary<") || cr.StartsWith("Gc<HashSet<") || cr.StartsWith("Gc<Stack<") || cr.StartsWith("Gc<Queue<") || cr.StartsWith("Gc<rt::sys::LinkedList<") || cr.StartsWith("Gc<rt::sys::OrderedData<") || cr.StartsWith("Gc<rt::sys::Grouping<")))
        { ForEachGeneral(fe, vd, n); return; }
        var c = E(coll); Flush();
        if (collUp != null) c = $"{c}.up::<{collUp}>()";
        if (cr.StartsWith("Gc<rt::sys::LinkedList<")) c = $"rt::sys::LinkedList::as_seq({c})";
        if (cr.StartsWith("Gc<rt::sys::OrderedData<")) c = $"rt::sys::OrderedData::to_seq({c})";
        if (cr.StartsWith("Gc<rt::sys::Grouping<")) c = $"rt::sys::Grouping::to_seq({c})";
        var e = $"__e{n}";
        var t = RTf(fe, vd.Symbol.Type);
        Line("{"); ind++;
        Line($"let mut {e} = rt::get_enumerator({c});");
        Line($"'l{n}: while {e}.move_next()");
        Line("{"); ind++;
        Line(IsCell(vd.Symbol) ? $"let mut {LocalName(vd.Symbol)}: Gc<{t}> = rt::new_cell::<{t}>({e}.current());" : $"let mut {LocalName(vd.Symbol)}: {t} = {e}.current();");
        if (fe.Body is IBlockOperation b) StmtList(b.Operations); else S(fe.Body);
        ind--; Line("}");
        ind--; Line("}");
    }

    /// `foreach` over a type the runtime has no enumerator for: lowered the way the C# compiler does it, with the
    /// collection's own GetEnumerator / MoveNext / Current (translated code, or the host boundary) and Dispose.
    void ForEachGeneral(IForEachLoopOperation fe, IVariableDeclaratorOperation vd, int n)
    {
        var info = fe.Syntax is CommonForEachStatementSyntax fs && fe.SemanticModel != null ? fe.SemanticModel.GetForEachStatementInfo(fs) : default;
        if (info.GetEnumeratorMethod == null || info.MoveNextMethod == null || info.CurrentProperty?.GetMethod == null)
        { Fail(fe, "foreach over " + (fe.Collection.Type == null ? "?" : Model.Short(fe.Collection.Type.OriginalDefinition))); return; }
        var coll = fe.Collection;
        // the conversion Roslyn puts on the collection leads to the type that declares GetEnumerator
        var et = info.GetEnumeratorMethod.ReturnType;
        var er = M.RT(et);
        if (er == null) { Fail(fe, "foreach: enumerator type with no Rust representation (" + Model.Short(et) + ")"); return; }
        var t = RTf(fe, vd.Symbol.Type);
        var en = $"__e{n}";
        string getE = info.GetEnumeratorMethod.IsStatic || info.GetEnumeratorMethod.IsExtensionMethod
            ? null : CallMethod(info.GetEnumeratorMethod, coll, new List<string>(), fe, false, null);
        if (getE == null) { Fail(fe, "foreach through an extension GetEnumerator"); return; }
        Flush();
        Line("{"); ind++;
        Line($"let mut {en}: {er} = {getE};");
        var eop = R(en, et, fe, true);
        void Loop()
        {
            Line($"'l{n}: loop");
            Line("{"); ind++;
            var mv = CallMethod(info.MoveNextMethod, eop, new List<string>(), fe, false, null);
            Flush();
            Line($"if !({mv}) {{ break 'l{n}; }}");
            var cur = CallMethod(info.CurrentProperty.GetMethod, eop, new List<string>(), fe, false, null);
            var ct = info.CurrentProperty.Type;
            if (!SymbolEqualityComparer.Default.Equals(ct, vd.Symbol.Type))
            {
                var cr2 = M.RT(ct);
                if (cr2 == null) cur = Fail(fe, "foreach: element type with no Rust representation");
                else if (cr2 == t) { }
                else if (IsGc(cr2) && IsGc(t)) { var tt = M.Get(vd.Symbol.Type); cur = tt != null && tt.IsClass && tt.TypeId != 0 ? $"rt::cast_class::<_, {tt.StructPath}>({cur}, {tt.ModPath}::TYPE_ID, &types::PARENT)" : $"{cur}.up::<{Inner(t)}>()"; }
                else if (IsGc(cr2)) cur = Unbox(vd.Symbol.Type, cr2 == "Obj" ? cur : $"{cur}.up::<Object>()", fe, false);
                else cur = Fail(fe, "foreach: element conversion " + Model.Short(ct) + " -> " + Model.Short(vd.Symbol.Type));
            }
            Flush();
            Line(IsCell(vd.Symbol) ? $"let mut {LocalName(vd.Symbol)}: Gc<{t}> = rt::new_cell::<{t}>({cur});" : $"let mut {LocalName(vd.Symbol)}: {t} = {cur};");
            if (fe.Body is IBlockOperation b) StmtList(b.Operations); else S(fe.Body);
            ind--; Line("}");
        }
        if (info.DisposeMethod != null)
        {
            // the jumps were registered for the plain loop; inside the try they are exits of the frame
            TryFinally(() =>
            {
                _jumps[fe.ExitLabel] = new Jump { Code = $"break 'l{n};", Depth = _tries.Count };
                _jumps[fe.ContinueLabel] = new Jump { Code = $"continue 'l{n};", Depth = _tries.Count };
                Loop();
            }, () => Dispose(en, et, fe));
        }
        else Loop();
        ind--; Line("}");
    }

    // A statement list with labels (and/or a switch with `goto case`) lowered to a pc loop. DESIGN.md section 6.
    void Segmented(List<(IOperation op, ISwitchCaseOperation caseStart)> items, ISwitchOperation sw)
    {
        int n = ++_lbl;
        string pc = $"__pc{n}", lb = $"'b{n}";
        // segment boundaries
        var segStart = new List<int>();   // index into items where each segment starts
        var caseSeg = new Dictionary<ISwitchCaseOperation, int>();
        for (int i = 0; i < items.Count; i++)
        {
            bool boundary = i == 0 || items[i].op is ILabeledOperation || items[i].caseStart != null;
            if (boundary) segStart.Add(i);
            if (items[i].caseStart != null) caseSeg[items[i].caseStart] = segStart.Count - 1;
            for (var lab = items[i].op as ILabeledOperation; lab != null; lab = lab.Operation as ILabeledOperation)
                _jumps[lab.Label] = new Jump { Code = $"{{ {pc} = {segStart.Count - 1}; continue {lb}; }}", Depth = _tries.Count };
        }
        int end = segStart.Count;
        Line("{"); ind++;
        var hoist = new StringBuilder();
        var body = new StringBuilder();
        var outer = sb; sb = body;
        string dispatch = "0";
        if (sw != null)
        {
            _jumps[sw.ExitLabel] = new Jump { Code = $"break {lb};", Depth = _tries.Count };
            foreach (var c in sw.Cases)
                foreach (var cl in c.Clauses)
                    if (cl.Label != null) _jumps[cl.Label] = new Jump { Code = $"{{ {pc} = {caseSeg[c]}; continue {lb}; }}", Depth = _tries.Count };
            dispatch = SwitchDispatch(sw, c => caseSeg[c].ToString(), end.ToString());
        }
        ind += 2;
        for (int s = 0; s < segStart.Count; s++)
        {
            int from = segStart[s], to = s + 1 < segStart.Count ? segStart[s + 1] : items.Count;
            ind--; Line($"{s} =>"); Line("{"); ind++;
            for (int i = from; i < to; i++)
            {
                var o = items[i].op;
                while (o is ILabeledOperation lab) o = lab.Operation;
                if (o == null) continue;
                if (o is IVariableDeclarationGroupOperation g)
                {
                    var savedDecls = _pendingDecls; _pendingDecls = new List<string>();
                    Decls(g, hoist);
                    _pendingDecls = savedDecls;
                }
                else S(o, hoist);
            }
            bool lastOfCase = sw != null && (s + 1 == segStart.Count || items[segStart[s + 1]].caseStart != null);
            Line(lastOfCase ? $"break {lb};" : $"{pc} = {s + 1};");
            ind--; Line("}"); ind++;
        }
        ind--; Line($"_ => {{ break {lb}; }}");
        ind--;
        sb = outer;
        sb.Append(hoist);
        Line($"let mut {pc}: i32 = {dispatch};");
        Line($"{lb}: loop");
        Line("{"); ind++;
        Line($"match {pc}");
        Line("{");
        sb.Append(body);
        Line("}");
        ind--; Line("}");
        ind--; Line("}");
    }

    string CaseConst(IOperation v, ITypeSymbol switchType)
    {
        if (!v.ConstantValue.HasValue) return null;
        var val = v.ConstantValue.Value;
        if (val == null) return null;
        return val switch
        {
            bool b => b ? "true" : "false",
            char c => ((int)c).ToString(CultureInfo.InvariantCulture),
            string => null,
            IFormattable f => f.ToString(null, CultureInfo.InvariantCulture),
            _ => null,
        };
    }

    /// `match value { consts => target(case), _ => deflt }` as an expression.
    string SwitchDispatch(ISwitchOperation sw, Func<ISwitchCaseOperation, string> target, string noDefault)
    {
        var t = sw.Value.Type;
        bool isString = t?.SpecialType == SpecialType.System_String;
        var v = E(sw.Value);
        var arms = new StringBuilder();
        string deflt = noDefault;
        var strs = new List<string>();
        foreach (var c in sw.Cases)
        {
            var pats = new List<string>();
            foreach (var cl in c.Clauses)
            {
                if (cl is IDefaultCaseClauseOperation) { deflt = target(c); continue; }
                if (cl is ISingleValueCaseClauseOperation sv)
                {
                    if (isString)
                    {
                        if (sv.Value.ConstantValue.HasValue && sv.Value.ConstantValue.Value is string s) { pats.Add(strs.Count.ToString()); strs.Add(StrLit(s)); continue; }
                        Fail(cl, "switch case: null / non-constant string"); continue;
                    }
                    var k = CaseConst(sv.Value, t);
                    if (k == null) { Fail(cl, "switch case value is not an integral constant"); continue; }
                    pats.Add(k);
                    continue;
                }
                Fail(cl, "switch clause: " + cl.CaseKind + (cl is IPatternCaseClauseOperation pc ? (pc.Guard != null ? " with when" : "") + " " + pc.Pattern.Kind : ""));
            }
            if (pats.Count > 0) arms.Append(string.Join(" | ", pats)).Append(" => ").Append(target(c)).Append(", ");
        }
        if (isString) v = $"rt::str_switch({v}, &[{string.Join(", ", strs)}])";
        else if (t == null || !(Model.Prim(t.SpecialType) != null || t.TypeKind == TypeKind.Enum)) Fail(sw, "switch on " + (t == null ? "?" : Model.Short(t)));
        return $"match {v} {{ {arms}_ => {deflt} }}";
    }

    void Switch(ISwitchOperation sw)
    {
        bool seg = false;
        foreach (var c in sw.Cases)
        {
            if (c.Clauses.Any(cl => cl.Label != null && _gotoTargets.Contains(cl.Label))) seg = true;
            if (c.Body.Any(o => o is ILabeledOperation)) seg = true;
        }
        if (seg)
        {
            var items = new List<(IOperation, ISwitchCaseOperation)>();
            foreach (var c in sw.Cases)
            {
                bool first = true;
                foreach (var o in c.Body) { items.Add((o, first ? c : null)); first = false; }
                if (first) items.Add((null, c));
            }
            Segmented(items, sw);
            return;
        }
        int n = ++_lbl;
        _jumps[sw.ExitLabel] = new Jump { Code = $"break 'sw{n};", Depth = _tries.Count };
        var t = sw.Value.Type;
        bool isString = t?.SpecialType == SpecialType.System_String;
        if (!isString && (t == null || !(Model.Prim(t.SpecialType) != null || t.TypeKind == TypeKind.Enum))) { Fail(sw, "switch on " + (t == null ? "?" : Model.Short(t))); }
        var v = E(sw.Value);
        var strs = new List<string>();
        var arms = new List<(string pat, ISwitchCaseOperation c)>();
        ISwitchCaseOperation dflt = null;
        foreach (var c in sw.Cases)
        {
            var pats = new List<string>();
            foreach (var cl in c.Clauses)
            {
                if (cl is IDefaultCaseClauseOperation) { dflt = c; continue; }
                if (cl is ISingleValueCaseClauseOperation sv)
                {
                    if (isString)
                    {
                        if (sv.Value.ConstantValue.HasValue && sv.Value.ConstantValue.Value is string s) { pats.Add(strs.Count.ToString()); strs.Add(StrLit(s)); continue; }
                        Fail(cl, "switch case: null / non-constant string"); continue;
                    }
                    var k = CaseConst(sv.Value, t);
                    if (k == null) { Fail(cl, "switch case value is not an integral constant"); continue; }
                    pats.Add(k);
                    continue;
                }
                Fail(cl, "switch clause: " + cl.CaseKind + (cl is IPatternCaseClauseOperation pc ? (pc.Guard != null ? " with when" : "") + " " + pc.Pattern.Kind : ""));
            }
            if (pats.Count > 0 && c != dflt) arms.Add((string.Join(" | ", pats), c));
            // a section that has both `case k:` and `default:` is reached through `_`
        }
        if (isString) v = $"rt::str_switch({v}, &[{string.Join(", ", strs)}])";
        Flush();
        Line($"'sw{n}:"); Line("{"); ind++;
        Line($"match {v}");
        Line("{"); ind++;
        foreach (var (pat, c) in arms)
        {
            Line($"{pat} =>");
            Line("{"); ind++;
            StmtList(c.Body);
            ind--; Line("}");
        }
        Line("_ =>");
        Line("{"); ind++;
        if (dflt != null) StmtList(dflt.Body);
        ind--; Line("}");
        ind--; Line("}");
        ind--; Line("}");
    }

    string Throw(IThrowOperation th)
    {
        if (th.Exception == null)
        {
            if (_catchVars.Count == 0) return Fail(th, "rethrow outside a translated catch");
            return $"rt::rethrow({_catchVars.Peek()})";
        }
        var ex = th.Exception;
        while (ex is IConversionOperation { IsImplicit: true } c) ex = c.Operand;
        var r = M.RT(ex.Type);
        if (r == "Gc<Exception>") return $"rt::throw({E(ex)})";
        if (IsExcClass(ex.Type)) return $"rt::throw({E(ex)}.up::<Exception>())";
        return Fail(th, "throw of a type that is not an exception class (" + (ex.Type == null ? "?" : Model.Short(ex.Type)) + ")");
    }

    /// A translated class that derives from System.Exception (its layout starts with the runtime exception).
    bool IsExcClass(ITypeSymbol t)
    {
        for (var tt = M.Get(t); tt != null; tt = M.Pop(tt).Base) if (M.Pop(tt).ExcKind != null) return true;
        return false;
    }

    void Try(ITryOperation t)
    {
        int n = ++_lbl;
        string r = $"__r{n}";
        foreach (var c in t.Catches)
        {
            if (c.Filter != null) { Fail(c, "exception filter (catch ... when)"); return; }
            if (c.ExceptionType != null && c.ExceptionType.SpecialType != SpecialType.System_Object && !M.IsBclException(c.ExceptionType) && !IsExcClass(c.ExceptionType)) { Fail(c, "catch of a type that is not an exception class (" + Model.Short(c.ExceptionType) + ")"); return; }
        }
        Line("{"); ind++;
        var frame = new TryFrame();
        // body
        _tries.Add(frame);
        Line($"let mut {r} = {TryOpen()}"); ind++;
        if (t.Body != null) StmtList(t.Body.Operations);
        Line("rt::Flow::Next");
        ind--; Line(TryClose());
        _tries.RemoveAt(_tries.Count - 1);
        if (t.Catches.Length > 0)
        {
            string ex = $"__ex{n}";
            Line($"if let Err({ex}) = {r}");
            Line("{"); ind++;
            bool first = true;
            foreach (var c in t.Catches)
            {
                // `catch { }` (no type) is modelled by Roslyn as catching object: it catches every exception
                string tn = c.ExceptionType == null || c.ExceptionType.SpecialType == SpecialType.System_Object ? "System.Exception" : Model.FullName(c.ExceptionType);
                var ect = c.ExceptionType == null ? null : M.Get(c.ExceptionType);
                bool gameExc = ect != null && IsExcClass(c.ExceptionType);
                Line(gameExc ? $"{(first ? "" : "else ")}if rt::is_instance({ex}, {ect.ModPath}::TYPE_ID, &types::PARENT)" : $"{(first ? "" : "else ")}if rt::exc_is({ex}, {Lit(tn)})");
                first = false;
                Line("{"); ind++;
                _tries.Add(frame);
                _catchVars.Push(ex);
                Line($"{r} = {TryOpen()}"); ind++;
                if (c.ExceptionDeclarationOrExpression is IVariableDeclaratorOperation vd)
                    Line(gameExc ? $"let mut {LocalName(vd.Symbol)}: Gc<{ect.StructPath}> = {ex}.up();" : $"let mut {LocalName(vd.Symbol)}: Gc<Exception> = {ex};");
                else if (c.ExceptionDeclarationOrExpression != null) Fail(c, "catch into an existing variable");
                if (c.Handler != null) StmtList(c.Handler.Operations);
                Line("rt::Flow::Next");
                ind--; Line(TryClose());
                _catchVars.Pop();
                _tries.RemoveAt(_tries.Count - 1);
                ind--; Line("}");
            }
            ind--; Line("}");
        }
        if (t.Finally != null) Block(t.Finally);
        LeaveTry(frame, r);
        ind--; Line("}");
    }

    /// The start of a `try` body: a closure for `rt::try_`, or in a resumable form an `async` block for the
    /// config's async try (awaited by [TryClose]). Both are a scope of their own: `return` leaves the body only, and
    /// the emitter's `rt::Flow` carries returns and jumps out (DESIGN.md sections 7, 8f).
    string TryOpen() => Resumable ? $"{M.Cfg.Resumable.Try}::<{_retType}, _>(async {{" : $"rt::try_(|| -> rt::Flow<{_retType}> {{";
    string TryClose() => Resumable ? "}).await;" : "});";

    /// After the `finally` code: perform the exits (return / break / continue / goto / rethrow) of the frame
    /// in the enclosing context.
    void LeaveTry(TryFrame frame, string r)
    {
        Line($"match {r}");
        Line("{"); ind++;
        Line("Ok(rt::Flow::Next) => {}");
        if (frame.Returns)
        {
            if (_tries.Count > 0) { _tries[^1].Returns = true; Line("Ok(rt::Flow::Return(__v)) => { return rt::Flow::Return(__v); }"); }
            else Line(_retType == "()" ? "Ok(rt::Flow::Return(_)) => { return; }" : "Ok(rt::Flow::Return(__v)) => { return __v; }");
        }
        for (int i = 0; i < frame.Exits.Count; i++) Line($"Ok(rt::Flow::Jump({i})) => {{ {JumpCode(frame.Exits[i])} }}");
        Line("Ok(_) => { rt::unreachable() }");
        Line("Err(__e) => { rt::rethrow(__e) }");
        ind--; Line("}");
    }

    /// `try { body } finally { fin }` for constructs the emitter lowers itself (`using`, `foreach` with Dispose).
    void TryFinally(Action body, Action fin)
    {
        int n = ++_lbl;
        string r = $"__r{n}";
        Line("{"); ind++;
        var frame = new TryFrame();
        _tries.Add(frame);
        Line($"let mut {r} = {TryOpen()}"); ind++;
        body();
        Line("rt::Flow::Next");
        ind--; Line(TryClose());
        _tries.RemoveAt(_tries.Count - 1);
        fin();
        LeaveTry(frame, r);
        ind--; Line("}");
    }

    /// `using (resource) body`: the body in a try, `Dispose()` of each non-null resource in the finally, last first.
    void Using(IUsingOperation us)
    {
        Line("{"); ind++;
        var res = new List<(string name, ITypeSymbol type)>();
        if (us.Resources is IVariableDeclarationGroupOperation g)
        {
            Decls(g, null);
            foreach (var d in g.Declarations) foreach (var v in d.Declarators) res.Add((IsCell(v.Symbol) ? $"(*{LocalName(v.Symbol)}.0)" : LocalName(v.Symbol), v.Symbol.Type));
        }
        else
        {
            var t = RTf(us.Resources, us.Resources.Type); var n = Tmp("u");
            var e = E(us.Resources); Flush(); Line($"let mut {n}: {t} = {e};");
            res.Add((n, us.Resources.Type));
        }
        TryFinally(() => { if (us.Body is IBlockOperation b) StmtList(b.Operations); else S(us.Body); },
                   () => { for (int i = res.Count - 1; i >= 0; i--) Dispose(res[i].name, res[i].type, us); });
        ind--; Line("}");
    }

    void Dispose(string name, ITypeSymbol type, IOperation at)
    {
        var disp = M.Comp.GetSpecialType(SpecialType.System_IDisposable).GetMembers("Dispose").OfType<IMethodSymbol>().First();
        IMethodSymbol target = disp;
        if (type is INamedTypeSymbol nt && nt.TypeKind != TypeKind.Interface && nt.FindImplementationForInterfaceMember(disp) is IMethodSymbol impl) target = impl;
        var call = CallMethod(target, R(name, type, at, true), new List<string>(), at, false, null);
        Flush();
        var r = M.RT(type);
        Line(type.IsValueType || r == null || !(IsGc(r) || IsHandleLike(r)) ? $"{call};" : $"if !{name}.is_null() {{ {call}; }}");
    }

    // ------------------------------------------------------------------ expression statements
    void ExprStmt(IOperation op)
    {
        switch (op)
        {
            case ISimpleAssignmentOperation a when VecTrackable(a.Target, a.Value) is ILocalSymbol vl:
                {
                    var pre = new StringBuilder();
                    var (vx, vy) = VecX87(a.Value, pre, false);
                    Flush();
                    string cx = Tmp("vx"), cy = Tmp("vy");
                    Line($"{pre}let {cx}: f64 = {vx}; let {cy}: f64 = {vy};");
                    Line($"{LocalName(vl)} = terraria_core::xna::Vector2::new({cx} as f32, {cy} as f32);");
                    _vecComp[vl] = (cx, cy);
                    return;
                }
            case ISimpleAssignmentOperation a: { var s = Assign(a, false); Flush(); Line(s); return; }
            case ICompoundAssignmentOperation ca: { var s = Compound(ca, false); Flush(); Line(s); return; }
            case IIncrementOrDecrementOperation inc: { var s = IncDec(inc, false); Flush(); Line(s); return; }
            case IThrowOperation th: { var s = Throw(th); Flush(); Line(s + ";"); return; }
            case IConditionalAccessOperation cao when cao.Type == null || cao.Type.SpecialType == SpecialType.System_Void:
                {
                    var s = CondAccess(cao, true); Flush(); Line(s); return;
                }
            default:
                {
                    var e = E(op); Flush();
                    Line(op.Type == null || op.Type.SpecialType == SpecialType.System_Void ? e + ";" : $"let _ = {e};");
                    return;
                }
        }
    }

    // ------------------------------------------------------------------ purity / places
    static bool Pure(IOperation o) => o switch
    {
        null => true,
        Rendered => true,
        ILocalReferenceOperation or IParameterReferenceOperation or IInstanceReferenceOperation or ILiteralOperation or IDefaultValueOperation => true,
        _ when o.ConstantValue.HasValue => true,
        IFieldReferenceOperation f => Pure(f.Instance),
        IConversionOperation c => c.OperatorMethod == null && Pure(c.Operand),
        IBinaryOperation b => b.OperatorMethod == null && Pure(b.LeftOperand) && Pure(b.RightOperand),
        IUnaryOperation u => u.OperatorMethod == null && Pure(u.Operand),
        IArrayElementReferenceOperation a => Pure(a.ArrayReference) && a.Indices.All(Pure),
        _ => false,
    };

    bool IsPlace(IOperation o) => o switch
    {
        Rendered r => r.Place,
        ILocalReferenceOperation or IParameterReferenceOperation => true,
        IFieldReferenceOperation f => !f.Field.IsConst && !M.Cfg.NativeSlices.ContainsKey(M.DocId(f.Field)) && !IsHostField(f.Field)
            && !(M.Get(f.Field.ContainingType) == null && M.Stub(f.Field.OriginalDefinition) != null && NativeTemplate(f.Field.OriginalDefinition) == null),
        IArrayElementReferenceOperation a => !(a.ArrayReference is IFieldReferenceOperation af && M.Cfg.NativeSlices.ContainsKey(M.DocId(af.Field))),
        IInstanceReferenceOperation i => i.Type != null && i.Type.IsValueType,
        IPropertyReferenceOperation p => NativeTemplate(p.Property) is { IsPlace: true },
        _ => IsPtrDeref(o),
    };

    bool IsHostField(IFieldSymbol f) => Model.IsLibrary(f.ContainingType) && M.Get(f.ContainingType) == null && NativeTemplate(f.OriginalDefinition) == null && M.Stub(f.OriginalDefinition) == null;

    /// The type id a value of `t` is boxed with (`rt::tid` for runtime types, `TYPE_ID` of a translated struct).
    string Tid(ITypeSymbol t)
    {
        t = Mono.Sub(t);
        // an enum boxes with its own id (and makes itself known to the runtime: names for ToString / Enum.GetName)
        if (t is INamedTypeSymbol { TypeKind: TypeKind.Enum } en) return $"rt::enum_tid(&{EnumInfo(en)})";
        switch (Under(t))
        {
            case SpecialType.System_Boolean: return "rt::tid::BOOL"; case SpecialType.System_Byte: return "rt::tid::U8"; case SpecialType.System_SByte: return "rt::tid::I8";
            case SpecialType.System_Int16: return "rt::tid::I16"; case SpecialType.System_UInt16: return "rt::tid::U16"; case SpecialType.System_Int32: return "rt::tid::I32";
            case SpecialType.System_UInt32: return "rt::tid::U32"; case SpecialType.System_Int64: return "rt::tid::I64"; case SpecialType.System_UInt64: return "rt::tid::U64";
            case SpecialType.System_Single: return "rt::tid::F32"; case SpecialType.System_Double: return "rt::tid::F64"; case SpecialType.System_Char: return "rt::tid::CHAR";
        }
        var tt = M.Get(t);
        if (tt != null && tt.IsStruct && tt.WholeTypeReason == null) return $"{tt.ModPath}::TYPE_ID";
        var r = M.RT(t);
        switch (r)
        {
            case "terraria_core::xna::Color": return "rt::tid::COLOR"; case "terraria_core::xna::Vector2": return "rt::tid::VECTOR2";
            case "terraria_core::xna::Vector3": return "rt::tid::VECTOR3"; case "terraria_core::xna::Vector4": return "rt::tid::VECTOR4";
            case "terraria_core::xna::Rectangle": return "rt::tid::RECTANGLE"; case "terraria_core::xna::Point": return "rt::tid::POINT";
            case "terraria_core::xna::Matrix": return "rt::tid::MATRIX";
        }
        if (r != null && r.StartsWith(M.Cfg.HostModule + "::")) return "rt::tid::OPAQUE";
        // a natively mapped struct says how it is boxed itself (`Trace::BOX`; NATIVE_SCHEMA.md)
        if (r != null && M.Native(t) is { Kind: "struct" }) return $"const {{ rt::box_id::<{r}>() }}";
        return null;
    }

    static bool IsHandleLike(string r) => r.StartsWith("Arr<") || r.StartsWith("Arr2<") || r.StartsWith("Arr3<") || r.StartsWith("Delegate<") || r.StartsWith("Seq<");

    /// Value of C# type `t` as `object` / an interface: boxing for value types (a fresh object holding a copy, as in
    /// the CLR), retyping for handles, a string object for `string`. DESIGN.md section 2.
    string Box(ITypeSymbol t, string e, IOperation at)
    {
        if (t == null) return Fail(at, "boxing conversion (?)");
        t = Mono.Sub(t);
        if (t.SpecialType == SpecialType.System_String) return $"rt::str_obj({e})";
        var r = M.RT(t);
        if (r == null) return Fail(at, "boxing of a type with no Rust representation (" + Model.Short(t) + ")");
        if (IsGc(r)) return r == "Obj" ? e : $"{e}.up::<Object>()";
        if (IsHandleLike(r)) return $"{e}.up::<Object>()";
        if (t is ITypeParameterSymbol) return $"rt::Trace::to_obj(&{e})";
        if (t.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T)
        {
            var id0 = Tid(((INamedTypeSymbol)t).TypeArguments[0]);
            return id0 == null ? Fail(at, "boxing conversion (" + Model.Short(t) + ")") : $"rt::box_nullable({e}, {id0})";
        }
        // a `List<T>.Enumerator` seen as `IEnumerator<T>` / `IEnumerator` / `IDisposable` / `object`: an enumerator
        // object of the runtime over a copy of the struct (boxing copies it in C# too)
        if (r.StartsWith("rt::ListEnum<")) return $"rt::iter::enumerator_of_list_enum({e})";
        var id = Tid(t);
        return id == null ? Fail(at, "boxing conversion (" + Model.Short(t) + ")") : $"rt::box_value({e}, {id})";
    }

    /// `(T)o` for an `object` / interface handle `o` (an unboxing or a downcast to a handle-like runtime type).
    string Unbox(ITypeSymbol t, string e, IOperation at, bool tryCast)
    {
        t = Mono.Sub(t);
        if (t.SpecialType == SpecialType.System_String) return tryCast ? $"rt::obj_as_str({e})" : $"rt::obj_str({e})";
        var r = M.RT(t);
        if (r == null) return Fail(at, "unboxing to a type with no Rust representation");
        if (t is ITypeParameterSymbol) return tryCast ? Fail(at, "`as` to a type parameter") : $"<{r} as rt::Trace>::from_obj({e})";
        if (t.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T)
        {
            var id0 = Tid(((INamedTypeSymbol)t).TypeArguments[0]);
            return id0 == null ? Fail(at, "unboxing conversion (" + Model.Short(t) + ")") : tryCast ? $"rt::unbox_or_null::<{RTf(at, ((INamedTypeSymbol)t).TypeArguments[0])}>({e}, {id0})" : $"rt::unbox_nullable::<{RTf(at, ((INamedTypeSymbol)t).TypeArguments[0])}>({e}, {id0})";
        }
        if (IsRtCast(r, t)) return RtCast(r, t, e, tryCast);
        var id = Tid(t);
        if (id == null) return Fail(at, "unboxing conversion (" + Model.Short(t) + ")");
        return $"rt::unbox::<{r}>({e}, {id})";
    }

    /// `o is T` for a handle `o` (static type object / interface / class) and any target type; null when no rule.
    string IsType(string e, ITypeSymbol valueType, ITypeSymbol t)
    {
        valueType = Mono.Sub(valueType); t = Mono.Sub(t);
        var vr = M.RT(valueType);
        if (vr == null || !IsGc(vr)) return null;
        var o = vr == "Obj" ? e : $"{e}.up::<Object>()";
        if (t.SpecialType == SpecialType.System_Object) return $"(!{e}.is_null())";
        if (t.SpecialType == SpecialType.System_String) return $"rt::is_boxed({o}, rt::tid::STRING)";
        var tt = M.Get(t);
        if (tt != null && tt.WholeTypeReason == null)
        {
            if (tt.IsInterface) return $"rt::is_iface({o}, {tt.ModPath}::implements)";
            if (tt.IsStruct) return $"rt::is_boxed({o}, {tt.ModPath}::TYPE_ID)";
            if (tt.TypeId != 0) return $"rt::is_instance({e}, {tt.ModPath}::TYPE_ID, &types::PARENT)";
            return null;
        }
        if (t is ITypeParameterSymbol) return null;
        if (t.IsValueType) { if (t.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T) t = ((INamedTypeSymbol)t).TypeArguments[0]; var id = Tid(t); return id == null ? null : $"rt::is_boxed({o}, {id})"; }
        var r = M.RT(t);
        // a library exception class: the runtime's exception object of that kind (or a translated class deriving from it)
        if (r == "Gc<Exception>") return $"(!rt::as_exception({o}, {Lit(Model.FullName(t))}, types::is_exception).is_null())";
        if (IsRtCast(r, t)) return $"(!{RtCast(r, t, o, true)}.is_null())";
        if (t is INamedTypeSymbol nt && Model.IsLibrary(nt) && (nt.TypeKind is TypeKind.Class or TypeKind.Interface) && r == "Obj")
        {
            if (M.Cfg.TypeTests.TryGetValue(M.DocId(nt.OriginalDefinition), out var ttest))
            { var tv = Tmp("tt"); return $"{{ let {tv}: Obj = {o}; !{tv}.is_null() && {ttest.Replace("{0}", tv)} }}"; }
            return $"{M.Cfg.HostRoot}::is_instance({o}, {Lit(M.Host.TypeTest(nt, OwnerName))})";
        }
        return null;
    }

    sealed class Native { public string Get, Set; public bool IsPlace; public ISymbol Sym; }
    Native NativeTemplate(ISymbol sym)
    {
        if (sym is IMethodSymbol ms) sym = ms.ReducedFrom ?? ms;
        sym = sym.OriginalDefinition;
        // DESIGN.md section 8f: a library member that waits is awaited inside a resumable form
        if (Resumable && M.Cfg.Resumable?.Await is { Count: > 0 } aw && aw.TryGetValue(M.DocId(sym), out var awt))
            return new Native { Get = awt, IsPlace = false, Sym = sym };
        System.Text.Json.JsonElement el;
        // A library override without a mapping of its own uses the mapping of the member it overrides
        // (`MemoryStream.Position` -> `Stream.Position`): one Rust type stands for the hierarchy (NATIVE_SCHEMA.md).
        while (!M.Cfg.NativeMembers.TryGetValue(M.DocId(sym), out el))
        {
            if (sym.ContainingType == null || M.Get(sym.ContainingType) != null || !Model.IsLibrary(sym.ContainingType)) return null;
            ISymbol up = sym switch { IMethodSymbol m => m.OverriddenMethod, IPropertySymbol p => p.OverriddenProperty, _ => null };
            if (up == null) return null;
            sym = up.OriginalDefinition;
        }
        if (M.Cfg.IgnoredTypes.Count > 0)
        {
            bool Ign(ITypeSymbol t) => t is INamedTypeSymbol n && (M.Cfg.IgnoredTypes.Contains(M.DocId(n.OriginalDefinition)) || n.TypeArguments.Any(Ign)) || t is IArrayTypeSymbol a && Ign(a.ElementType);
            bool hit = sym switch
            {
                IMethodSymbol m => Ign(m.ReturnType) || m.Parameters.Any(q => Ign(q.Type)),
                IPropertySymbol q => Ign(q.Type),
                IFieldSymbol f => Ign(f.Type),
                _ => false,
            };
            if (hit) return null;
        }
        if (el.ValueKind == System.Text.Json.JsonValueKind.String) return new Native { Get = el.GetString(), IsPlace = sym is IFieldSymbol or IPropertySymbol, Sym = sym };
        var n = new Native { Sym = sym };
        if (el.TryGetProperty("get", out var g)) n.Get = g.GetString();
        if (el.TryGetProperty("set", out var s)) n.Set = s.GetString();
        return n;
    }

    /// The template of `sym` for a call on receiver `inst`. A member of a natively mapped class reached through a
    /// derived library class that is NOT mapped (`CryptoStream.Dispose()` binds to `Stream.Dispose`) has an opaque
    /// receiver the native Rust type knows nothing about: no template, the call stays a host member.
    Native NativeFor(ISymbol sym, IOperation inst)
    {
        var nat = NativeTemplate(sym);
        if (nat == null || inst?.Type == null || sym.IsStatic) return nat;
        // `MP3Stream.Dispose()` binds to `Stream.Dispose` (not virtual), but MP3Stream is mapped to a Rust type of its
        // own: the base member's template was written for the base's Rust type. Use the derived class's own entry
        // for a member of that name and signature when it has one, else the call stays a host member.
        if (M.Native(nat.Sym.ContainingType) is { Kind: "class" } bn && Mono.Sub(inst.Type) is INamedTypeSymbol rcv && M.Native(rcv) is { Kind: "class" } rn
            && !SymbolEqualityComparer.Default.Equals(rcv.OriginalDefinition, nat.Sym.ContainingType.OriginalDefinition) && rn.Rust != bn.Rust)
        {
            foreach (var own in rcv.GetMembers(sym.Name))
            {
                bool same = own.Kind == sym.Kind && (own is not IMethodSymbol om || sym is not IMethodSymbol sm || om.Parameters.Length == sm.Parameters.Length && om.Parameters.Zip(sm.Parameters, (a, b) => SymbolEqualityComparer.Default.Equals(a.Type, b.Type)).All(x => x));
                if (same && M.Cfg.NativeMembers.ContainsKey(M.DocId(own.OriginalDefinition))) return NativeTemplate(own);
            }
            return null;
        }
        if (M.Native(nat.Sym.ContainingType) is { Kind: "class" } && M.RT(inst.Type) == "Obj" && inst.Type.TypeKind is TypeKind.Class or TypeKind.Error && inst.Type.SpecialType == SpecialType.None && M.Native(inst.Type) == null && Model.IsLibrary(inst.Type)) return null;
        return nat;
    }

    /// Runtime handle types an `object` is cast back to by the Rust type of the heap object (`rt::cast_rt`):
    /// arrays, delegates and every natively implemented class (collections, `nativeTypes` of kind "class").
    bool IsRtCast(string r, ITypeSymbol t) => r != null && (r.StartsWith("Arr<") || r.StartsWith("Arr2<") || r.StartsWith("Delegate<") || (IsGc(r) && r != "Obj" && M.Native(t) is { Kind: "class" }));

    /// `as` / cast of the handle `o` (an `Obj`) to the runtime handle type `r` of C# type `t`.
    string RtCast(string r, ITypeSymbol t, string o, bool tryCast)
    {
        if (M.Native(t) is { Is: { } isT }) return $"rt::{(tryCast ? "as_rt_if" : "cast_rt_if")}::<{r}>({o}, |__h| {isT.Replace("{0}", "__h")})";
        return $"rt::{(tryCast ? "as_rt" : "cast_rt")}::<{r}>({o})";
    }

    /// `{this}` of a native template: the value/place itself for value-like receivers, `(*ptr)` for classes.
    string NativeThis(IOperation inst, ITypeSymbol declType = null)
    {
        if (inst == null) return "";
        var t = Mono.Sub(inst.Type);
        var r = M.RT(t);
        if (r == null) return Fail(inst, "receiver type with no Rust representation: " + KindOfType(t));
        // a member of a native base class on a translated subclass: the native value is the object's first field
        string up = null;
        if (declType != null && IsGc(r) && M.Get(t) is { IsClass: true } && M.RT(declType) is { } dr && IsGc(dr) && dr != r && dr != "Obj") up = Inner(dr);
        if (inst is IInstanceReferenceOperation ir)
        {
            if (ir.ReferenceKind == InstanceReferenceKind.ImplicitReceiver && _implicit.Count > 0)
                return _implicit.Peek().isStruct || !IsGc(r) ? _implicit.Peek().name : up != null ? $"(*{_implicit.Peek().name}.up::<{up}>().ptr())" : $"(*{_implicit.Peek().name}.0)";
            // `base.Member` where the base is a native class: `this` typed as the native value (the first field)
            if (up == null && !OwnerIsStruct && IsGc(r) && r != "Obj" && r != "Gc<Exception>" && M.Get(t) == null && M.NativeBaseOf(Owner) != null) up = Inner(r);
            return up != null ? $"(*rt::Gc(this as *mut {up}).ptr())" : "(*this)";
        }
        if (inst is IConditionalAccessInstanceOperation && _condAccess.Count > 0)
            return IsGc(r) ? (up != null ? $"(*{_condAccess.Peek()}.up::<{up}>().ptr())" : $"(*{_condAccess.Peek()}.ptr())") : _condAccess.Peek();
        // a member of System.Exception on a game exception object: the runtime exception is its first field
        if (IsGc(r) && r != "Gc<Exception>" && IsExcClass(t)) return $"(*{E(inst)}.up::<Exception>().ptr())";
        if (IsGc(r)) return up != null ? $"(*{E(inst)}.up::<{up}>().ptr())" : $"(*{E(inst)}.ptr())";
        return IsPlace(inst) ? P(inst) : $"({E(inst)})";
    }

    string Subst(string tmpl, IOperation at, string thisStr, IList<string> args, string value, IMethodSymbol m, ITypeSymbol recvType)
    {
        var s = tmpl;
        if (s.Contains("{handle}"))
        {
            // the receiver as a handle (`{this}` is the dereferenced place)
            var h = thisStr ?? "";
            if (h == "(*this)") h = "rt::Gc(this)";
            else if (h.StartsWith("(*") && h.EndsWith(".ptr())")) h = h.Substring(2, h.Length - 9);
            else if (h.StartsWith("(*") && h.EndsWith(".0)")) h = h.Substring(2, h.Length - 5);
            else return Fail(at, "native template needs the receiver as a handle");
            // a receiver that is itself an expression with effects (a chained `sb.Append(a).Append(b)`: the inner call's
            // block, or a call returning the builder) is evaluated once into a binding, which both `{this}` and
            // `{handle}` then name (the template would otherwise run the inner call twice)
            if (s.Contains("{this}") && h != "rt::Gc(this)" && (h.Contains("{") || System.Text.RegularExpressions.Regex.IsMatch(h, @"::\w+\(")))
            {
                var b = Tmp("recv");
                s = "{ let " + b + " = " + h + "; " + s.Replace("{handle}", b).Replace("{this}", "(*" + b + ".ptr())") + " }";
            }
            else s = s.Replace("{handle}", h);
        }
        if (s.Contains("{this}")) s = s.Replace("{this}", thisStr ?? "");
        if (value != null) s = s.Replace("{value}", value);
        if (s.Contains("{raw"))
        {
            if (_rawArgs == null) return Fail(at, "native template with {raw} used outside a plain invocation");
            for (int i = _rawArgs.Length - 1; i >= 0; i--)
            {
                if (!s.Contains("{raw" + i + "}")) continue;
                if (_rawArgs[i] == null) { _rawArgs = null; return Fail(at, "member of System.Array on a value whose element type is not known statically (System.Array)"); }
                s = s.Replace("{raw" + i + "}", _rawArgs[i]);
            }
            _rawArgs = null;
        }
        if (args != null) for (int i = args.Count - 1; i >= 0; i--) s = s.Replace("{" + i + "}", args[i]);
        if (s.Contains("{args}")) s = s.Replace("{args}", args == null ? "" : string.Join(", ", args));
        if (m != null)
        {
            // game types that flow into the host crate's Newtonsoft implementation need its traits (Gen.JsonImpls)
            if (tmpl.Contains("terraria_host::json::") || (m.ContainingType?.ContainingNamespace?.ToDisplayString().StartsWith("Newtonsoft.Json") ?? false)) foreach (var ta in m.TypeArguments) M.NoteJsonType(Mono.Sub(ta));
            for (int i = 0; i < m.TypeArguments.Length; i++) if (s.Contains("{T" + i + "}")) s = s.Replace("{T" + i + "}", RTf(at, m.TypeArguments[i]));
            // {E0}: the rt::EnumInfo of an enum type argument (`Enum.TryParse<TEnum>`)
            for (int i = 0; i < m.TypeArguments.Length; i++)
                if (s.Contains("{E" + i + "}"))
                {
                    if (Mono.Sub(m.TypeArguments[i]) is not INamedTypeSymbol { TypeKind: TypeKind.Enum } et) return Fail(at, "native template needs an enum type argument");
                    s = s.Replace("{E" + i + "}", "&" + EnumInfo(et));
                }
            // {VD0}: the `VertexDeclaration` of a type argument that implements IVertexType (XNA's generic draw calls
            // ask a default instance of the vertex struct for it): the struct's translated property getter
            for (int i = 0; i < m.TypeArguments.Length; i++)
                if (s.Contains("{VD" + i + "}"))
                {
                    var vt = Mono.Sub(m.TypeArguments[i]) as INamedTypeSymbol;
                    var ivt = vt?.AllInterfaces.FirstOrDefault(x => x.ToDisplayString() == "Microsoft.Xna.Framework.Graphics.IVertexType");
                    var vdp = ivt?.GetMembers("VertexDeclaration").OfType<IPropertySymbol>().FirstOrDefault();
                    var vimpl = vdp == null ? null : vt.FindImplementationForInterfaceMember(vdp) as IPropertySymbol;
                    var vtt = vt == null ? null : M.Get(vt);
                    if (vimpl?.GetMethod == null || vtt == null || !vtt.IsStruct || M.Method(vtt, vimpl.GetMethod) is not { Emittable: true })
                        return Fail(at, "native template needs a translated vertex struct (IVertexType) as type argument");
                    var vtmp = Tmp("vt");
                    var vget = CallMethod(vimpl.GetMethod, R(vtmp, vt, at, true), new List<string>(), at, false, null);
                    s = s.Replace("{VD" + i + "}", $"{{ let mut {vtmp}: {RTf(at, vt)} = rt::zero(); {vget} }}");
                }
            if (s.Contains("{ret}")) s = s.Replace("{ret}", RTf(at, m.ReturnType));
        }
        if (Mono.Sub(recvType) is INamedTypeSymbol nt)
            for (int i = 0; i < nt.TypeArguments.Length; i++) if (s.Contains("{TT" + i + "}")) s = s.Replace("{TT" + i + "}", RTf(at, nt.TypeArguments[i]));
        return s;
    }

    string ThisPtrAs(TType decl)
    {
        if (decl == null || decl == Owner) return "this";
        return $"(this as *mut {decl.StructPath})";
    }

    /// Pointer to the class instance `inst`, typed as `decl` (the class that declares the member).
    string ClassPtr(IOperation inst, TType decl)
    {
        if (inst is IInstanceReferenceOperation ir)
        {
            if (ir.ReferenceKind == InstanceReferenceKind.ImplicitReceiver && _implicit.Count > 0)
            {
                var want0 = $"Gc<{decl.StructPath}>";
                return M.RT(inst.Type) == want0 ? $"{_implicit.Peek().name}.0" : $"({_implicit.Peek().name}.0 as *mut {decl.StructPath})";
            }
            return decl == Owner ? "this" : $"(this as *mut {decl.StructPath})";
        }
        string e = inst is IConditionalAccessInstanceOperation && _condAccess.Count > 0 ? _condAccess.Peek() : E(inst);
        var want = $"Gc<{decl.StructPath}>";
        return M.RT(inst.Type) == want ? $"{e}.ptr()" : $"({e}.ptr() as *mut {decl.StructPath})";
    }

    /// Place of a struct-typed receiver.
    string StructPlace(IOperation inst)
    {
        if (inst is IInstanceReferenceOperation ir)
        {
            if (ir.ReferenceKind == InstanceReferenceKind.ImplicitReceiver && _implicit.Count > 0) return _implicit.Peek().name;
            return "(*this)";
        }
        if (inst is IConditionalAccessInstanceOperation && _condAccess.Count > 0) return _condAccess.Peek();
        return IsPlace(inst) ? P(inst) : $"({E(inst)})";
    }

    string FieldPlace(IFieldReferenceOperation f)
    {
        var sym = f.Field.OriginalDefinition;
        var ft = M.RT(f.Field.Type);
        var nat = NativeTemplate(sym);
        if (nat != null) return Subst(nat.Get, f, NativeThis(f.Instance, f.Field.ContainingType), null, null, null, f.Instance?.Type);
        if (ft == null) return Fail(f, "field of a type with no Rust representation: " + KindOfType(f.Field.Type));
        var tt = M.Get(f.Field.ContainingType);
        var fi = tt == null ? null : M.Field(tt, sym);
        if (IsHostField(f.Field)) return HostCall(f.Field, "field-get", f.Instance, null, f);
        if (sym.IsStatic)
        {
            if (M.Cfg.NativeSlices.ContainsKey(M.DocId(sym))) return Fail(f, "native table used other than by index / Length");
            if (M.Stub(sym) is { } stub) { Stubs.Add(Model.ShortMember(sym)); return $"rt::stub::<{ft}>({Lit(Model.ShortMember(sym))}, {stub})"; }
            if (fi == null) { return "(*" + Unp(Model.ShortMember(sym), "*mut " + ft) + ")"; }
            if (sym.IsConst) return $"{tt.ModPath}::{fi.Name}";
            Calls.Add($"{tt.ModPath}::cctor");
            return $"(*{tt.ModPath}::s()).{fi.Name}";
        }
        if (fi == null && M.Stub(sym) is { } istub) { Stubs.Add(Model.ShortMember(sym)); return $"rt::stub::<{ft}>({Lit(Model.ShortMember(sym))}, {istub})"; }
        if (fi == null) return "(*" + Unp(Model.ShortMember(sym), "*mut " + ft) + ")";
        if (tt.IsStruct && fi.Overlay)
        {
            // a C# union member ([FieldOffset] over other fields): the bytes at its offset, as its type
            bool place = f.Instance is IInstanceReferenceOperation || f.Instance is IConditionalAccessInstanceOperation || IsPlace(f.Instance);
            if (place) return $"(*((core::ptr::addr_of_mut!({StructPlace(f.Instance)}) as *mut u8).add({fi.Offset}) as *mut {fi.RustType}))";
            var u = Tmp("u");
            return $"{{ let mut {u} = {E(f.Instance)}; *((core::ptr::addr_of_mut!({u}) as *mut u8).add({fi.Offset}) as *mut {fi.RustType}) }}";
        }
        if (tt.IsStruct) return $"{StructPlace(f.Instance)}.{fi.Name}";
        return $"(*{ClassPtr(f.Instance, tt)}).{fi.Name}";
    }

    string P(IOperation op)
    {
        switch (op)
        {
            case Rendered rd: return rd.Text;
            case ILocalReferenceOperation l: return l.Local.RefKind != RefKind.None ? $"(*{LocalName(l.Local)})" : IsCell(l.Local) ? $"(*{LocalName(l.Local)}.0)" : LocalName(l.Local);
            case IParameterReferenceOperation p: return p.Parameter.RefKind is RefKind.Ref or RefKind.Out ? $"(*{LocalName(p.Parameter)})" : IsCell(p.Parameter) ? $"(*{LocalName(p.Parameter)}.0)" : LocalName(p.Parameter);
            case IFieldReferenceOperation f: return FieldPlace(f);
            case IArrayElementReferenceOperation a:
                {
                    if (a.ArrayReference is IFieldReferenceOperation af && M.Cfg.NativeSlices.ContainsKey(M.DocId(af.Field))) return Fail(op, "write to a native table");
                    var arr = E(a.ArrayReference);
                    return $"(*{arr}.at({string.Join(", ", a.Indices.Select(Index))}))";
                }
            case IInstanceReferenceOperation i when i.Type != null && i.Type.IsValueType:
                return i.ReferenceKind == InstanceReferenceKind.ImplicitReceiver && _implicit.Count > 0 ? _implicit.Peek().name : "(*this)";
            case IPropertyReferenceOperation pr when NativeTemplate(pr.Property) is { IsPlace: true } nat:
                return Subst(nat.Get, op, NativeThis(pr.Instance, pr.Property.ContainingType), pr.Arguments.Select(x => E(x.Value)).ToList(), null, null, pr.Property.ContainingType);
            case IDeclarationExpressionOperation de when de.Expression is ILocalReferenceOperation dl:
                {
                    var t = RTf(op, dl.Local.Type);
                    if (IsCell(dl.Local))
                    {
                        _pendingDecls.Add($"let mut {LocalName(dl.Local)}: Gc<{t}> = rt::new_cell::<{t}>({Zero(dl.Local.Type)});");
                        return $"(*{LocalName(dl.Local)}.0)";
                    }
                    _pendingDecls.Add($"let mut {LocalName(dl.Local)}: {t} = {Zero(dl.Local.Type)};");
                    return LocalName(dl.Local);
                }
            case IDiscardOperation d:
                {
                    var t = RTf(op, d.Type); var n = Tmp("discard");
                    _pendingDecls.Add($"let mut {n}: {t} = {Zero(d.Type)};");
                    return n;
                }
        }
        if (IsPtrDeref(op)) return $"(*{PtrAddr(op)})";
        return Fail(op, "not an assignable place: " + op.Kind);
    }

    string Addr(IOperation op)
    {
        if (op is IParameterReferenceOperation { Parameter.RefKind: RefKind.Ref or RefKind.Out } p) return LocalName(p.Parameter);
        if (op is ILocalReferenceOperation { Local.RefKind: not RefKind.None } l) return LocalName(l.Local);
        if (op is IArrayElementReferenceOperation a && !(a.ArrayReference is IFieldReferenceOperation af && M.Cfg.NativeSlices.ContainsKey(M.DocId(af.Field))))
            return $"{E(a.ArrayReference)}.at({string.Join(", ", a.Indices.Select(Index))})";
        if (op is IInstanceReferenceOperation { Type.IsValueType: true, ReferenceKind: InstanceReferenceKind.ContainingTypeInstance }) return "this";
        if (IsPtrDeref(op)) return PtrAddr(op);
        if (!IsPlace(op) && op is not IDeclarationExpressionOperation && op is not IDiscardOperation) return Fail(op, "address of a non-place: " + op.Kind);
        return $"core::ptr::addr_of_mut!({P(op)})";
    }

    string Index(IOperation i)
    {
        var e = E(i);
        if (i.Type?.SpecialType == SpecialType.System_Int32) return e;
        if (i.Type?.SpecialType is SpecialType.System_UInt32 or SpecialType.System_Int64 or SpecialType.System_UInt64) return $"rt::index_from({e})";
        return Fail(i, "array index of type " + (i.Type == null ? "?" : Model.Short(i.Type)));
    }

    // ------------------------------------------------------------------ literals
    string Const(object v, ITypeSymbol t, bool suffix)
    {
        if (v == null)
        {
            if (t == null) return "rt::zero()";
            var r = M.RT(t);
            if (r == null) return "rt::zero()";
            if (r == "Str") return "Str::NULL";
            if (IsGc(r)) return $"Gc::<{Inner(r)}>::NULL";
            return $"rt::zero::<{r}>()";
        }
        var st = t?.TypeKind == TypeKind.Enum ? ((INamedTypeSymbol)t).EnumUnderlyingType.SpecialType : (t?.SpecialType ?? SpecialType.None);
        switch (v)
        {
            case bool b: return b ? "true" : "false";
            case string s: return HasLoneSurrogate(s) ? "rt::bad()" : StrLit(s);
            case char c: return ((int)c).ToString(CultureInfo.InvariantCulture) + "u16";
            case float f: return FloatLit(f, "f32");
            case double d: return FloatLit(d, "f64");
            case int i: return i < 0 ? $"({i}{(suffix ? "i32" : "")})" : i + (suffix ? "i32" : "");
            case decimal dm:
            {
                // the four words of decimal.GetBits: low, mid, high, flags (sign and scale)
                var bits = decimal.GetBits(dm);
                return $"rt::decimal::Decimal::from_bits([{bits[0]}i32, {bits[1]}i32, {bits[2]}i32, {bits[3]}i32])";
            }
        }
        if (v is IFormattable fm)
        {
            var p = Model.Prim(st) ?? Model.Prim(Type.GetTypeCode(v.GetType()) switch
            {
                TypeCode.Byte => SpecialType.System_Byte, TypeCode.SByte => SpecialType.System_SByte, TypeCode.Int16 => SpecialType.System_Int16,
                TypeCode.UInt16 => SpecialType.System_UInt16, TypeCode.UInt32 => SpecialType.System_UInt32, TypeCode.Int64 => SpecialType.System_Int64,
                TypeCode.UInt64 => SpecialType.System_UInt64, _ => SpecialType.System_Int32
            });
            var txt = fm.ToString(null, CultureInfo.InvariantCulture);
            return txt.StartsWith("-") ? $"({txt}{p})" : txt + p;
        }
        return "rt::bad()";
    }

    static string FloatLit(double d, string ty)
    {
        if (double.IsNaN(d)) return ty + "::NAN";
        if (double.IsPositiveInfinity(d)) return ty + "::INFINITY";
        if (double.IsNegativeInfinity(d)) return ty + "::NEG_INFINITY";
        string s = ty == "f32" ? ((float)d).ToString("R", CultureInfo.InvariantCulture) : d.ToString("R", CultureInfo.InvariantCulture);
        if (s.Contains('E'))
        {
            var parts = s.Split('E');
            s = parts[0] + (parts[0].Contains('.') ? "" : ".0") + "e" + int.Parse(parts[1], CultureInfo.InvariantCulture);
        }
        else if (!s.Contains('.')) s += ".0";
        if (d == 0 && double.IsNegative(d) && !s.StartsWith("-")) s = "-" + s;
        return s.StartsWith("-") ? $"({s}{ty})" : s + ty;
    }

    // ------------------------------------------------------------------ expressions
    string E(IOperation op) => E(op, false);

    string E(IOperation op, bool suffix)
    {
        if (op == null) return "rt::bad()";
        if (op is Rendered rd) return rd.Text;
        if (op.ConstantValue.HasValue && op is not IDeclarationExpressionOperation)
        {
            var v = op.ConstantValue.Value;
            if (v is string cs && HasLoneSurrogate(cs)) return Fail(op, "string literal with a lone surrogate");
            // a constant of a translated const field keeps its name
            if (op is IFieldReferenceOperation { Field.IsConst: true } cf)
            {
                var ctt = M.Get(cf.Field.ContainingType);
                var cfi = ctt == null ? null : M.Field(ctt, cf.Field.OriginalDefinition);
                if (cfi != null && cfi.RustType != null) return $"{ctt.ModPath}::{cfi.Name}";
            }
            // (a decimal constant is written by Const: rt::decimal::Decimal::from_bits)
            return Const(v, op.Type, suffix);
        }
        switch (op)
        {
            case ILiteralOperation lit: return Const(null, lit.Type, false);
            case ILocalReferenceOperation wl when IsWide(wl.Local): return $"({LocalName(wl.Local)} as f32)";
            case IParameterReferenceOperation wp when IsWide(wp.Parameter): return $"({LocalName(wp.Parameter)} as f32)";
            case ILocalReferenceOperation or IParameterReferenceOperation: return P(op);
            case IFieldReferenceOperation f: return FieldPlace(f);
            case IInstanceReferenceOperation ir:
                if (ir.ReferenceKind == InstanceReferenceKind.ImplicitReceiver && _implicit.Count > 0) return _implicit.Peek().name;
                if (ir.Type != null && ir.Type.IsValueType) return "(*this)";
                {
                    // `this` seen as a library base class / interface: a struct is boxed, a class handle is retyped
                    if (OwnerIsStruct) return Box(Owner.Sym, "(*this)", op);
                    var here = M.Get(ir.Type);
                    if (here == null || here.IsInterface) return SymbolEqualityComparer.Default.Equals(ir.Type, Owner.Sym) ? "Gc(this)" : "Gc(this as *mut Object)";
                    return here == Owner ? "Gc(this)" : $"Gc(this as *mut {here.StructPath})";
                }
            case IArrayElementReferenceOperation a:
                {
                    if (a.ArrayReference is IFieldReferenceOperation af && M.Cfg.NativeSlices.TryGetValue(M.DocId(af.Field), out var slice))
                        return $"rt::slice_get(&{slice}, {string.Join(", ", a.Indices.Select(Index))})";
                    return P(op);
                }
            case IPropertyReferenceOperation p: return PropGet(p);
            case IInvocationOperation inv: return Invoke(inv);
            case IObjectCreationOperation oc: return New(oc);
            case IArrayCreationOperation ac: return NewArray(ac);
            case ISimpleAssignmentOperation sa: return Assign(sa, true);
            case ICompoundAssignmentOperation ca: return Compound(ca, true);
            case IIncrementOrDecrementOperation inc: return IncDec(inc, true);
            case IBinaryOperation b: return Binary(b);
            case IUnaryOperation u: return Unary(u);
            case IConversionOperation c: return Conv(c);
            case IConditionalOperation co:
                if (X87 && IsF32(co) && IsF32(co.WhenTrue) && IsF32(co.WhenFalse)) return $"({F64(co)} as f32)";
                return $"(if {E(co.Condition)} {{ {E(co.WhenTrue, suffix)} }} else {{ {E(co.WhenFalse, suffix)} }})";
            case ICoalesceOperation cl: return Coalesce(cl);
            case IDefaultValueOperation dv: RTf(op, dv.Type); return Zero(dv.Type);
            case IIsTypeOperation it:
                {
                    var vo = it.ValueOperand;
                    while (vo is IConversionOperation { OperatorMethod: null, IsImplicit: true } vc && vc.Operand.Type != null && vc.Operand.Type.IsReferenceType && vc.Operand.Type.SpecialType != SpecialType.System_String && vc.Type?.SpecialType == SpecialType.System_Object) vo = vc.Operand;
                    var e = vo.Type == null ? null : IsType(E(vo), vo.Type, it.TypeOperand);
                    if (e == null) return Fail(op, "is-type test with no rule (" + (vo.Type == null ? "?" : Model.Short(vo.Type)) + " is " + Model.Short(it.TypeOperand) + ")");
                    return it.IsNegated ? $"(!{e})" : e;
                }
            case IIsPatternOperation ip: return Pattern(ip.Value, ip.Pattern, op);
            case IDelegateCreationOperation dc: return DelegateNew(dc);
            case IAnonymousFunctionOperation: return Fail(op, "lambda outside a delegate creation");
            case IThrowOperation th: return Throw(th);
            case IDeclarationExpressionOperation: return P(op);
            case IInterpolatedStringOperation isx: return Interp(isx);
            case IConditionalAccessOperation cao: return CondAccess(cao, false);
            case IConditionalAccessInstanceOperation:
                return _condAccess.Count > 0 ? _condAccess.Peek() : Fail(op, "conditional access instance");
            case ITranslatedQueryOperation q: return E(q.Operation);
            case ISizeOfOperation so:
                // generated structs and the native XNA value types are `repr(C)` with the fields of the original in order
                return $"(core::mem::size_of::<{RTf(op, so.TypeOperand)}>() as i32)";
            case IAwaitOperation: return Fail(op, "await");
            case ITupleOperation: return Fail(op, "tuple");
            case IAnonymousObjectCreationOperation ao:
                {
                    // `new { A = x, B = y }`: property names in declaration order + the values, boxed (DESIGN.md 8e)
                    var props = ao.Type.GetMembers().OfType<IPropertySymbol>().ToList();
                    var vals = new string[props.Count];
                    foreach (var ini in ao.Initializers)
                    {
                        if (ini is not ISimpleAssignmentOperation { Target: IPropertyReferenceOperation tp } sa0) return Fail(op, "anonymous type initialiser " + ini.Kind);
                        int k = props.FindIndex(x => SymbolEqualityComparer.Default.Equals(x, tp.Property));
                        if (k < 0) return Fail(op, "anonymous type initialiser of an unknown property");
                        // initialisers run in source order
                        var tv = Tmp("an");
                        _pendingDecls.Add($"let {tv}: Obj = {Box(tp.Property.Type, E(sa0.Value, true), op)};");
                        vals[k] = tv;
                    }
                    if (vals.Any(v => v == null)) return Fail(op, "anonymous type with an uninitialised property");
                    return $"rt::anon_new(&[{string.Join(", ", props.Select(x => Lit(x.Name)))}], &[{string.Join(", ", vals)}])";
                }
            case ISwitchExpressionOperation: return Fail(op, "switch expression");
            case IDynamicInvocationOperation or IDynamicMemberReferenceOperation or IDynamicObjectCreationOperation or IDynamicIndexerAccessOperation: return Fail(op, "dynamic");
            case IAddressOfOperation ao: return Addr(ao.Reference);
            case IEventReferenceOperation er:
                {
                    // inside the declaring class a field-like event is its delegate field
                    var ep = EventPlace(er);
                    return ep ?? Fail(op, "event reference (not a field-like event of a translated class)");
                }
            case IEventAssignmentOperation ea: return EventAssign(ea);
            case ITypeOfOperation to when Mono.Sub(to.TypeOperand) is INamedTypeSymbol or IArrayTypeSymbol:
                return TypeOfExpr(Mono.Sub(to.TypeOperand));
            case ITypeOfOperation: return Fail(op, "typeof of a type parameter / pointer");
            case IMethodReferenceOperation: return Fail(op, "method group outside a delegate creation");
            case ICoalesceAssignmentOperation: return Fail(op, "??= assignment");
            case IDeconstructionAssignmentOperation: return Fail(op, "deconstruction");
            case ITypeParameterObjectCreationOperation tpo: return NewT(tpo);
            case IRangeOperation: return Fail(op, "range");
            case IInvalidOperation:
                if (InvalidOutVarCall(op) is { } recovered) return recovered;
                if (Environment.GetEnvironmentVariable("CS2RS_DEBUG_INVALID") != null)
                {
                    void Dump(IOperation o, int d) { Console.Error.WriteLine(new string(' ', d * 2) + o.Kind + " [" + o.GetType().Name + "] type=" + o.Type + " syntax=" + o.Syntax.Kind() + " `" + o.Syntax.ToString().Split((char)10)[0] + "`"); foreach (var c in o.ChildOperations) Dump(c, d + 1); }
                    Console.Error.WriteLine("INVALID at " + Where(op) + " parent=" + op.Parent?.Kind); Dump(op.Parent ?? op, 0);
                }
                return Fail(op, "binding error (IInvalidOperation)");
            case IDiscardOperation: return P(op);
        }
        if (IsPtrDeref(op)) return $"(*{PtrAddr(op)})";
        return Fail(op, "expression: " + op.Kind);
    }

    /// The decompiler's `M(.., out var x, out x)`: the IL passes the address of ONE local for two `out` parameters
    /// (the original source had two discards), and the decompiler prints the second use by name. C# rejects a
    /// reference to an out variable inside the argument list that declares it (CS8196), so Roslyn binds the whole
    /// call as an invalid node whose children are the receiver and the arguments in order. Recovered here when
    /// that is the only thing wrong with the call: the method resolves, every other argument has exactly the
    /// parameter's type. The call then gets the address of the same local twice, as the IL does (the callee's
    /// writes land in it in the callee's order). Null when the node is anything else.
    string InvalidOutVarCall(IOperation op)
    {
        if (op.Syntax is not InvocationExpressionSyntax ies || op.SemanticModel == null) return null;
        if (ies.ArgumentList.Arguments.Any(a => a.NameColon != null)) return null;
        var si = op.SemanticModel.GetSymbolInfo(ies);
        var m = (si.Symbol ?? (si.CandidateSymbols.Length == 1 ? si.CandidateSymbols[0] : null)) as IMethodSymbol;
        if (m == null || m.IsGenericMethod || m.Parameters.Any(q => q.IsParams)) return null;
        var ch = op.ChildOperations.ToList();
        int first = ch.Count == m.Parameters.Length + 1 && !m.IsStatic ? 1 : 0;
        if (ch.Count != first + m.Parameters.Length || (first == 0 && !m.IsStatic)) return null;
        bool self = false;
        for (int i = 0; i < m.Parameters.Length; i++)
        {
            var q = m.Parameters[i]; var c = ch[first + i];
            if (c is ILocalReferenceOperation { Type.TypeKind: TypeKind.Error } lr && q.RefKind == RefKind.Out
                && ch.Skip(first).Take(i).Any(x => x is IDeclarationExpressionOperation { Expression: ILocalReferenceOperation dl } && SymbolEqualityComparer.Default.Equals(dl.Local, lr.Local) && SymbolEqualityComparer.Default.Equals(dl.Local.Type, q.Type)))
            { self = true; continue; }
            if (c is IInvalidOperation || c.Type == null || !SymbolEqualityComparer.Default.Equals(c.Type, q.Type)) return null;
        }
        if (!self) return null;
        var args = new List<string>();
        for (int i = 0; i < m.Parameters.Length; i++)
        {
            var q = m.Parameters[i]; var c = ch[first + i];
            args.Add(q.RefKind is RefKind.Ref or RefKind.Out ? Addr(c) : E(c));
        }
        return CallMethod(m, first == 1 ? ch[0] : null, args, op, false, null);
    }

    // ---- events
    /// Place of the delegate field behind a field-like event of a translated class, or null.
    string EventPlace(IEventReferenceOperation er)
    {
        var ev = er.Event.OriginalDefinition;
        var tt = M.Get(er.Event.ContainingType);
        if (tt == null) return null;
        var fi = M.Field(tt, ev);
        if (fi == null || fi.RustType == null || !fi.RustType.StartsWith("Delegate<")) return null;
        if (ev.IsStatic) return $"(*{tt.ModPath}::s()).{fi.Name}";
        if (er.Instance == null) return null;
        return tt.IsStruct ? $"{StructPlace(er.Instance)}.{fi.Name}" : $"(*{ClassPtr(er.Instance, tt)}).{fi.Name}";
    }

    /// `x.Event += handler` / `-=`: combine / remove on the delegate field, or the accessor of a library event.
    string EventAssign(IEventAssignmentOperation ea)
    {
        if (ea.EventReference is not IEventReferenceOperation er) return Fail(ea, "event subscription on a non-event");
        var ev = er.Event;
        var acc = ea.Adds ? ev.AddMethod : ev.RemoveMethod;
        if (M.Get(ev.ContainingType) == null && Model.IsLibrary(ev.ContainingType))
        {
            if (acc == null) return Fail(ea, "library event without accessor");
            // natively mapped accessor (`M:Type.add_Name(Handler)`: {this}, {0} = the handler delegate)
            if (NativeTemplate(acc) is { Get: { } evt }) return Subst(evt, ea, er.Instance == null ? null : NativeThis(er.Instance), new[] { E(ea.HandlerValue) }, null, acc, er.Instance?.Type ?? ev.ContainingType);
            return HostCall(acc, "call", er.Instance, new[] { E(ea.HandlerValue) }, ea);
        }
        var place = EventPlace(er);
        // an interface / virtual event, or one with custom add / remove bodies: through its accessor (dispatched)
        if (M.Get(ev.ContainingType) is { } evt2 && acc != null && (place == null || evt2.IsInterface || ev.IsVirtual || ev.IsAbstract || ev.IsOverride)
            && M.Method(evt2, acc) is { Emittable: true })
            return CallMethod(acc, er.Instance, new[] { E(ea.HandlerValue) }, ea, er.Instance != null && IsBase(er.Instance), null);
        if (place == null)
        {
            // event with explicit add / remove bodies, or declared by a game type that is not translated
            var name = Model.ShortMember(ev.OriginalDefinition) + (ea.Adds ? " (add)" : " (remove)");
            if (M.Get(ev.ContainingType) != null) return Fail(ea, "event subscription (event with custom accessors or unrepresentable delegate)");
            Unported.Add(name); Gaps.Add(name);
            return $"{{ let _ = {E(ea.HandlerValue)}; rt::gap({Lit(name)}) }}";
        }
        var hr = M.RT(ea.HandlerValue.Type ?? ev.Type);
        if (hr != M.RT(ev.Type)) return Fail(ea, "event subscription with a handler of another delegate type");
        var p = Tmp("p");
        return $"{{ let {p} = core::ptr::addr_of_mut!({place}); let __h = {E(ea.HandlerValue)}; *{p} = rt::wbv(Delegate::{(ea.Adds ? "combine" : "remove")}(*{p}, __h)); }}";
    }

    // ---- floats as the legacy x86 JIT computes them (config "floatMode"; DESIGN.md section 5)
    //
    // The game is 32-bit code: `float` arithmetic runs on the x87 unit with 53-bit precision. An expression tree
    // of `float` operations is therefore evaluated in double precision and rounded to 32 bits only where the
    // value is stored.
    //   "f32"         every `float` operation rounds (the C# text taken literally).
    //   "x87"         `float` + - * / % trees, unary minus, ?: and comparisons are evaluated in f64; the value is
    //                 rounded to f32 when it is assigned (any variable, field, array element), passed as an
    //                 argument, returned, or cast with `(float)`. `(double)tree` and `(int)tree` take the
    //                 unrounded value.
    //   "x87-locals"  as "x87", and a `float` local whose address is never taken (no ref / out, not captured)
    //                 is kept unrounded too (an f64 variable): the JIT keeps such locals on the x87 stack.
    //   "jit32"       as "x87", and the float locals that the 32-bit JIT keeps on the x87 stack are unrounded
    //                 variables, rounded at every non-inlined call they are live across (FloatAlloc.cs; the rule and
    //                 its evidence: port\crates\terraria-x87\FLOAT_RULE.md). Methods too large for the JIT's
    //                 optimiser keep every local in memory, i.e. behave as "x87".
    bool X87 => M.Cfg.FloatMode is "x87" or "x87-locals" or "jit32";
    /// The method whose body is being emitted (for its IL size), when known.
    public IMethodSymbol Method;
    FloatAlloc _fa;
    Dictionary<SyntaxNode, List<ISymbol>> _spillAt;
    /// Invocation syntax -> stack locals rounded at that call itself (FloatAlloc.ProvenCalls evaluated conditionally).
    Dictionary<SyntaxNode, List<ISymbol>> _spillInline;

    /// Is `call` evaluated only on some paths of `st` (the right operand of `&&` / `||` / `??`, a branch of `?:`)?
    static bool Conditional(SyntaxNode call, SyntaxNode st)
    {
        for (SyntaxNode c = call, a = call.Parent; a != null && c != st; c = a, a = a.Parent)
        {
            if (a is BinaryExpressionSyntax be && be.Right == c && be.Kind() is SyntaxKind.LogicalAndExpression or SyntaxKind.LogicalOrExpression or SyntaxKind.CoalesceExpression) return true;
            if (a is ConditionalExpressionSyntax ce && ce.Condition != c) return true;
        }
        return false;
    }
    HashSet<ISymbol> _wide;
    /// `float` locals kept at x87 precision ("x87-locals").
    HashSet<ISymbol> Wide()
    {
        if (_wide != null) return _wide;
        _wide = new HashSet<ISymbol>(SymbolEqualityComparer.Default);
        if (!(M.Cfg.FloatMode is "x87-locals" or "jit32") || _root == null) return _wide;
        if (WideParams != null && M.Cfg.FloatMode == "jit32") _wide.UnionWith(WideParams);
        var no = new HashSet<ISymbol>(SymbolEqualityComparer.Default);
        foreach (var d in _root.DescendantsAndSelf())
        {
            switch (d)
            {
                case IVariableDeclaratorOperation v when v.Symbol.Type.SpecialType == SpecialType.System_Single && v.Symbol.RefKind == RefKind.None
                        && v.Parent is IVariableDeclarationOperation { Parent: IVariableDeclarationGroupOperation }:
                    _wide.Add(v.Symbol); break;
                case IArgumentOperation { Parameter.RefKind: not RefKind.None } arg when arg.Value is ILocalReferenceOperation l: no.Add(l.Local); break;
                case IAddressOfOperation { Reference: ILocalReferenceOperation l2 }: no.Add(l2.Local); break;
                case IAnonymousFunctionOperation lam:
                    foreach (var x in lam.Body.DescendantsAndSelf().OfType<ILocalReferenceOperation>()) no.Add(x.Local);
                    break;
                case IVariableDeclaratorOperation rv when rv.Symbol.RefKind != RefKind.None && rv.GetVariableInitializer()?.Value is ILocalReferenceOperation l3: no.Add(l3.Local); break;
                case ISimpleAssignmentOperation { IsRef: true, Value: ILocalReferenceOperation l4 }: no.Add(l4.Local); break;
            }
        }
        _wide.ExceptWith(no);
        if (M.Cfg.FloatInMemory.Count > 0 && Method != null)
        {
            // measured exceptions to the allocation rule (config `floatInMemory`, each with its evidence)
            var owner = Method.ContainingType.ToDisplayString() + "." + Method.Name + ":";
            _wide.RemoveWhere(v => M.Cfg.FloatInMemory.ContainsKey(owner + v.Name));
        }
        if (M.Cfg.FloatMode == "jit32")
        {
            _fa = FloatAlloc.Analyse(M, _root, Method, no, WideParams);
            _wide.IntersectWith(_fa.Stack);
            foreach (var k in _fa.Kept) if (_wide.Contains(k)) _keptAcrossCalls.Add(k);
            // measured exceptions the other way (config `floatStackAcrossCalls`): on the stack, never rounded at a call
            if (M.Cfg.FloatStackAcrossCalls.Count > 0 && Method != null)
            {
                var owner2 = Method.ContainingType.ToDisplayString() + "." + Method.Name + ":";
                foreach (var d in _root.DescendantsAndSelf().OfType<IVariableDeclaratorOperation>())
                    if (d.Symbol.Type.SpecialType == SpecialType.System_Single && d.Symbol.RefKind == RefKind.None && M.Cfg.FloatStackAcrossCalls.ContainsKey(owner2 + d.Symbol.Name) && !no.Contains(d.Symbol))
                    {
                        _wide.Add(d.Symbol);
                        _keptAcrossCalls.Add(d.Symbol);
                    }
            }
            // the rounding at a call is emitted in front of the statement that contains the call; a call in a place
            // that is evaluated repeatedly without being a statement (a loop condition) keeps its variables in memory
            _spillAt = new Dictionary<SyntaxNode, List<ISymbol>>();
            _spillInline = new Dictionary<SyntaxNode, List<ISymbol>>();
            foreach (var kv0 in _fa.Spill)
            {
                var kv = new KeyValuePair<SyntaxNode, List<ISymbol>>(kv0.Key, kv0.Value.Where(v => !_keptAcrossCalls.Contains(v)).ToList());
                if (kv.Value.Count == 0) continue;
                SyntaxNode st = kv.Key.FirstAncestorOrSelf<StatementSyntax>();
                if (_fa.ProvenCalls.Contains(kv.Key) && kv.Key is InvocationExpressionSyntax && st != null && Conditional(kv.Key, st)
                    && !kv.Key.Ancestors().TakeWhile(a => a != st).Any(a => a is AnonymousFunctionExpressionSyntax))
                {
                    _spillInline[kv.Key] = kv.Value.ToList();
                    continue;
                }
                bool ok = st is ExpressionStatementSyntax or LocalDeclarationStatementSyntax or ReturnStatementSyntax
                    || st is IfStatementSyntax ifs && ifs.Condition.Span.Contains(kv.Key.Span);
                // a lambda between the call and the statement: the call is not part of this body's control flow
                if (kv.Key.Ancestors().TakeWhile(a => a != st).Any(a => a is AnonymousFunctionExpressionSyntax)) continue;
                if (!ok) { foreach (var v in kv.Value) if (_wide.Remove(v) && !Resumable && !TwinF64) System.Threading.Interlocked.Increment(ref M.FloatDemoted); continue; }
                if (!_spillAt.TryGetValue(st, out var l)) _spillAt[st] = l = new List<ISymbol>();
                foreach (var v in kv.Value) if (!l.Contains(v, SymbolEqualityComparer.Default)) l.Add(v);
            }
            // the statistics count each body once (not again for its resumable form or its __x87 twin)
            if (!Resumable && !TwinF64)
            {
                System.Threading.Interlocked.Increment(ref M.FloatMethods);
                if (_fa.MinOpts) System.Threading.Interlocked.Increment(ref M.FloatMinOpts);
                System.Threading.Interlocked.Add(ref M.FloatStackLocals, _wide.Count);
                if (_fa.Undetermined && Method != null) M.FloatUndetermined.Add(Model.ShortMember(Method.OriginalDefinition));
                if (Method != null && Environment.GetEnvironmentVariable("CS2RS_FLOAT_REPORT") != null && Model.ClrMethodKey(Method.OriginalDefinition) is { } fk)
                {
                    var stack = _wide;
                    string J(IEnumerable<ISymbol> s) => string.Join(",", s.Select(v => v.Name).Distinct().OrderBy(x => x, StringComparer.Ordinal));
                    M.FloatReport.Add(fk + "\t" + (_fa.MinOpts ? 1 : 0) + "\t" + J(_fa.Vars.OfType<ILocalSymbol>()) + "\t" + J(stack) + "\t" + J(_fa.Spill.Values.SelectMany(x => x).Where(stack.Contains)));
                }
            }
        }
        return _wide;
    }
    static bool DeclaredBefore(ISymbol v, SyntaxNode at) => v.Locations.FirstOrDefault() is not { } l || l.SourceSpan.Start < at.SpanStart;
    /// Locals of config `floatStackAcrossCalls` in this body.
    readonly HashSet<ISymbol> _keptAcrossCalls = new(SymbolEqualityComparer.Default);
    bool IsWide(ISymbol s) => M.Cfg.FloatMode is "x87-locals" or "jit32" && (s is ILocalSymbol || s is IParameterSymbol && WideParams != null) && Wide().Contains(s);
    /// After the body: is this parameter of a `__u` twin an f64 variable (else the twin rounds it on entry)?
    public bool ParamIsWide(ISymbol p) => _wide != null && _wide.Contains(p);
    /// FLOAT_RULE.md C05: in `a + F(z)` the value of the stack local `a` is already loaded when the call runs and
    /// is spilled to a 4-byte slot: it continues rounded.
    bool PendingAcrossCall(IOperation localRef)
    {
        if (_fa == null) return false;
        for (IOperation c = localRef, p = localRef.Parent; p != null; c = p, p = p.Parent)
        {
            // the innermost binary operation decides: the local is its left operand and the right one calls (C05), or
            // it is an operand of arithmetic evaluated before the call, and that arithmetic's result is what waits:
            // an expression temporary, spilled to an 8-byte slot (C04; SolarSky.Activate `num * (maxTilesX * 16f) +
            // NextFloat() * 40f`: `fmulp`, `fstp qword [ebp-0x18]` before InternalSample, gallery run)
            if (p is IBinaryOperation b) return b.LeftOperand == c && _fa.ContainsCall(b.RightOperand);
            if (p is not (IParenthesizedOperation or IUnaryOperation or IConversionOperation)) break;
        }
        return false;
    }
    /// `0f - x` / `0.0 - x` in the decompiled text is the IL `neg` (x87 `fchs`): ILSpy prints a float negation that
    /// way. Measured over the 4,994 game methods that ran in the reference runs (IL of work/jit/ref/*/jit_code.json
    /// against the decompiled bodies): every such pattern has its `neg`; none is a subtraction from a literal zero.
    /// The difference is the sign of a zero result: `rotation = (0f - velocity.X) * 0.05f` with velocity.X = 0 is
    /// -0.0 in the original (refdata/entities, aiStyle 14 projectiles), +0.0 as a subtraction.
    static bool DecompiledNeg(IBinaryOperation b)
    {
        if (b.OperatorKind != BinaryOperatorKind.Subtract || b.OperatorMethod != null || b.IsLifted || b.ConstantValue.HasValue) return false;
        if (b.LeftOperand.Syntax is not LiteralExpressionSyntax || !b.LeftOperand.ConstantValue.HasValue) return false;
        return b.LeftOperand.ConstantValue.Value switch
        {
            float f => BitConverter.SingleToInt32Bits(f) == 0 && IsF32(b.RightOperand),
            double d => BitConverter.DoubleToInt64Bits(d) == 0 && Mono.Sub(b.RightOperand.Type)?.SpecialType == SpecialType.System_Double,
            _ => false,
        };
    }
    /// Float mode "jit32": is this method's call of `m` a call instruction in the original (not an intrinsic / inlined
    /// body)? A method compiled without optimisation inlines nothing; otherwise the reference runs' code
    /// (jit_refcalls.tsv: a call at every IL site). Unknown: false (the probes' answer for an optimised method).
    bool AbsIsCall(IMethodSymbol m)
    {
        if (M.Cfg.FloatMode != "jit32" || Method == null) return false;
        Wide();
        if (_fa != null && _fa.MinOpts) return true;
        return Model.ClrMethodKey(Method.OriginalDefinition) is { } ck && Model.ClrMethodKey(m.OriginalDefinition) is { } mk
            && M.JitRefCalls(ck, mk) is { } rv && rv.calls > 0 && rv.calls >= rv.sites;
    }
    static bool IsF32(IOperation o) => Mono.Sub(o?.Type)?.SpecialType == SpecialType.System_Single;
    static bool FloatArith(BinaryOperatorKind k) => k is BinaryOperatorKind.Add or BinaryOperatorKind.Subtract or BinaryOperatorKind.Multiply or BinaryOperatorKind.Divide or BinaryOperatorKind.Remainder;

    /// A `float` expression at x87 precision (an f64 Rust expression, not rounded to 32 bits).
    string F64(IOperation op)
    {
        while (op is IParenthesizedOperation par) op = par.Operand;
        if (op.ConstantValue.HasValue && op.ConstantValue.Value is float cf) return FloatLit((double)cf, "f64");
        switch (op)
        {
            case IBinaryOperation { OperatorMethod: null, IsLifted: false } bn when IsF32(bn) && DecompiledNeg(bn):
                return $"(-{F64(bn.RightOperand)})";
            case IBinaryOperation { OperatorMethod: null, IsLifted: false } b when IsF32(b) && FloatArith(b.OperatorKind) && IsF32(b.LeftOperand) && IsF32(b.RightOperand):
                {
                    string sym = b.OperatorKind switch { BinaryOperatorKind.Add => "+", BinaryOperatorKind.Subtract => "-", BinaryOperatorKind.Multiply => "*", BinaryOperatorKind.Divide => "/", _ => "%" };
                    return $"({F64Pending(b.LeftOperand, b.RightOperand)} {sym} {F64(b.RightOperand)})";
                }
            case IUnaryOperation { OperatorMethod: null, IsLifted: false, OperatorKind: UnaryOperatorKind.Minus } u when IsF32(u):
                return $"(-{F64(u.Operand)})";
            case IUnaryOperation { OperatorMethod: null, IsLifted: false, OperatorKind: UnaryOperatorKind.Plus } u2 when IsF32(u2):
                return F64(u2.Operand);
            case IConditionalOperation co when IsF32(co) && co.WhenFalse != null && IsF32(co.WhenTrue) && IsF32(co.WhenFalse):
                return $"(if {E(co.Condition)} {{ {F64(co.WhenTrue)} }} else {{ {F64(co.WhenFalse)} }})";
            case IFieldReferenceOperation { Instance: ILocalReferenceOperation fl, Field.Name: "X" or "Y" } vf when IsVec2(fl.Type) && VecTracked(fl) is { } fc:
                return vf.Field.Name == "X" ? fc.x : fc.y;
            case ILocalReferenceOperation l when IsWide(l.Local):
                return PendingAcrossCall(l) && !_keptAcrossCalls.Contains(l.Local) ? $"({LocalName(l.Local)} as f32 as f64)" : LocalName(l.Local);
            case IParameterReferenceOperation lp when IsWide(lp.Parameter):
                return PendingAcrossCall(lp) ? $"({LocalName(lp.Parameter)} as f32 as f64)" : LocalName(lp.Parameter);
            // `Math.Abs(floatExpr)` is `fabs` on the x87 stack value: neither the argument nor the result is rounded
            // (FLOAT_RULE.md section 2: P19, P20, x87_more/AbsMulF). Seen in the game: Main.UpdateWeather,
            // `num7 += Math.Abs(num8 - windSpeedCurrent) * 0.0015f` (refdata/ticks world scenario, tick 1501 on).
            // Where the JIT compiled a CALL to Math.Abs(float) instead (a method compiled without optimisation inlines
            // nothing: NPC.FindFrame, 113 call instructions at its 113 sites in the entity reference runs; or
            // jit_refcalls.tsv says so), the argument is a float argument of a call: rounded (`fstp dword [esp]`).
            // refdata/entities npc type 243: `frameCounter += Math.Abs(velocity.X * 1.1f)` adds 0.0769999995827675
            // (the product rounded), not 0.07700000199675561.
            case IInvocationOperation { TargetMethod: { Name: "Abs", Parameters.Length: 1 } am } absInv
                when am.ContainingType.ToDisplayString() == "System.Math" && am.Parameters[0].Type.SpecialType == SpecialType.System_Single && absInv.Arguments.Length == 1:
                return AbsIsCall(am) ? $"f64::abs((({F64(absInv.Arguments[0].Value)}) as f32) as f64)" : $"f64::abs({F64(absInv.Arguments[0].Value)})";
            // a library call evaluated by its x87 body with an unrounded result (config `jitBodies`, "f64:"), or a
            // direct call of a float method with an `__x87` twin (Model.TwinF64): the unrounded result
            case IInvocationOperation or IPropertyReferenceOperation when IsF32(op):
                {
                    _f64Alt = null;
                    var e = E(op);
                    var alt = _f64Alt;
                    _f64Alt = null;
                    if (alt is { } x && e == x.plain) return x.alt;
                    return $"({e} as f64)";
                }
        }
        return $"({E(op)} as f64)";
    }
    /// The left operand of a float operation whose right operand runs a call: a float CALL RESULT waiting there is
    /// spilled to a 4-byte slot (rounded); the JIT takes a call's float result for an exact float. Projectile.
    /// AI_165_Whip, `Utils.GetLerpValue(0.1f, 0.7f, t3, true) * Utils.GetLerpValue(0.9f, 0.7f, t3, true) > 0.5f` (whip
    /// 1034): `call GetLerpValue`, `fstp dword [ebp-0xa4c]`, the second call, `fmul dword [ebp-0xa4c]` at all 11 such
    /// pairs (refdata/entities citem 5479 / 5480, a Main.rand draw at tick 12). An arithmetic temporary waiting across a
    /// call keeps 53 bits instead (`fstp qword`, FLOAT_RULE.md C04).
    string F64Pending(IOperation left, IOperation right)
    {
        var l = F64(left);
        var s0 = left;
        while (s0 is IParenthesizedOperation par) s0 = par.Operand;
        if (M.Cfg.FloatMode == "jit32" && s0 is IInvocationOperation or IPropertyReferenceOperation && IsF32(s0) && VecCallIn(s0) && VecCallIn(right))
            return $"(({l}) as f32 as f64)";
        return l;
    }
    /// The last call CallMethod rendered that has an unrounded f64 form: (the rendered rounded call, the f64 form).
    /// F64 takes the f64 form when the expression it rendered is exactly that call.
    (string plain, string alt)? _f64Alt;

    // ---- numbers
    static bool IsFloat(SpecialType s) => s is SpecialType.System_Single or SpecialType.System_Double;
    static SpecialType Under(ITypeSymbol t) { t = Mono.Sub(t); return t == null ? SpecialType.None : t.TypeKind == TypeKind.Enum ? ((INamedTypeSymbol)t).EnumUnderlyingType.SpecialType : t.SpecialType; }
    static bool IsNum(SpecialType s) => Model.Prim(s) != null && s != SpecialType.System_Boolean;

    string NumCast(string e, SpecialType from, SpecialType to)
    {
        if (from == to) return e;
        var tp = Model.Prim(to);
        if (IsFloat(from) && !IsFloat(to))
        {
            var wide = from == SpecialType.System_Single ? $"({e} as f64)" : e;
            return $"rt::conv::to_{(to == SpecialType.System_Char ? "u16" : tp)}({wide})";
        }
        return $"({e} as {tp})";
    }

    string BinExpr(IOperation at, BinaryOperatorKind k, string l, string r, ITypeSymbol opType, ITypeSymbol lType, ITypeSymbol rType)
    {
        var st = Under(opType);
        bool isInt = IsNum(st) && !IsFloat(st);
        bool flt = IsFloat(st);
        switch (k)
        {
            case BinaryOperatorKind.Add: if (isInt) return $"{l}.wrapping_add({r})"; if (flt) return $"({l} + {r})"; break;
            case BinaryOperatorKind.Subtract: if (isInt) return $"{l}.wrapping_sub({r})"; if (flt) return $"({l} - {r})"; break;
            case BinaryOperatorKind.Multiply: if (isInt) return $"{l}.wrapping_mul({r})"; if (flt) return $"({l} * {r})"; break;
            case BinaryOperatorKind.Divide: if (isInt) return $"rt::div({l}, {r})"; if (flt) return $"({l} / {r})"; break;
            case BinaryOperatorKind.Remainder: if (isInt) return $"rt::rem({l}, {r})"; if (flt) return $"({l} % {r})"; break;
            case BinaryOperatorKind.LeftShift: if (isInt) return $"{l}.wrapping_shl({r} as u32)"; break;
            case BinaryOperatorKind.RightShift: if (isInt) return $"{l}.wrapping_shr({r} as u32)"; break;
            case BinaryOperatorKind.And: if (isInt || st == SpecialType.System_Boolean) return $"({l} & {r})"; break;
            case BinaryOperatorKind.Or: if (isInt || st == SpecialType.System_Boolean) return $"({l} | {r})"; break;
            case BinaryOperatorKind.ExclusiveOr: if (isInt || st == SpecialType.System_Boolean) return $"({l} ^ {r})"; break;
            case BinaryOperatorKind.ConditionalAnd: return $"({l} && {r})";
            case BinaryOperatorKind.ConditionalOr: return $"({l} || {r})";
            case BinaryOperatorKind.LessThan: return $"({l} < {r})";
            case BinaryOperatorKind.LessThanOrEqual: return $"({l} <= {r})";
            case BinaryOperatorKind.GreaterThan: return $"({l} > {r})";
            case BinaryOperatorKind.GreaterThanOrEqual: return $"({l} >= {r})";
            case BinaryOperatorKind.Equals: case BinaryOperatorKind.NotEquals:
                {
                    string eq;
                    if (lType?.SpecialType == SpecialType.System_String && rType?.SpecialType == SpecialType.System_String) eq = $"rt::str_eq({l}, {r})";
                    else
                    {
                        var lr = M.RT(lType); var rr = M.RT(rType);
                        if (lr == null || rr == null) return Fail(at, "comparison of a type with no Rust representation");
                        if (Mono.Sub(lType) is ITypeParameterSymbol || Mono.Sub(rType) is ITypeParameterSymbol) return Fail(at, "== on a type parameter");
                        if (lr != rr && IsGc(lr) && IsGc(rr)) eq = $"({l}.0 as *const u8 == {r}.0 as *const u8)";
                        else if (lr != rr) return Fail(at, $"== between different representations");
                        else eq = $"({l} == {r})";
                    }
                    return k == BinaryOperatorKind.Equals ? eq : $"(!{eq})";
                }
        }
        return Fail(at, $"binary operator {k} on {(opType == null ? "?" : Model.Short(opType))}");
    }

    // ---- float mode "jit32": Vector2 locals of a method whose inlined operator results stay on the x87 stack
    // (jit_vec_temps.tsv "stack"). The JIT promotes such a local like the inlinee temporaries: its two fields are float
    // variables. Utils.FactorAcceleration (refdata/entities proj 1047): `Vector2 vector = descendOfProjectile * (num *
    // num) / 2f / timeToInterception; return currentVelocity - vector;` is `fmul`, `fmul`, `fmul`, `fsubrp`, and the
    // only stores are the two of the returned value. A local assigned from an inlined operator chain keeps its
    // unrounded components (`__vxN`, `__vyN`) for the reads that follow in the same block until a statement with a
    // call (a call spills the x87 stack as floats: then the Vector2 value is exact), control flow, or another write.
    // A local whose address the method takes (instance call on it, ref / out argument, lambda capture) is in memory.
    readonly Dictionary<ISymbol, (string x, string y)> _vecComp = new(SymbolEqualityComparer.Default);
    bool _vecClearAfter;
    HashSet<ISymbol> _vecAddr;
    bool? _vecStack;
    bool VecStackMethod() => _vecStack ??= M.Cfg.FloatMode == "jit32" && Method != null
        && (VecCtx != null ? VecCtxMode == "stack" : M.JitVecTemps(Model.ClrMethodKey(Method.OriginalDefinition)) == "stack");
    /// `__vcN` for a call of `def` at `at` when the caller inlined that site (jit_vec_ctx.tsv); the site's index is its
    /// rank among this body's calls of `def` in source order (= IL order).
    string VecTwinSuffix(IMethodSymbol def, IOperation at)
    {
        if (M.Cfg.FloatMode != "jit32" || Method == null || VecCtx != null || at is not IInvocationOperation inv || _root == null) return null;
        string caller = Model.ClrMethodKey(Method.OriginalDefinition), callee = Model.ClrMethodKey(def);
        if (caller == null || callee == null || M.VecCtxSigs(callee).Count == 0) return null;
        var sites = _root.DescendantsAndSelf().OfType<IInvocationOperation>()
            .Where(x => SymbolEqualityComparer.Default.Equals((x.TargetMethod.ReducedFrom ?? x.TargetMethod).OriginalDefinition, def))
            .OrderBy(x => x.Syntax.SpanStart).ToList();
        int k = sites.IndexOf(inv);
        int n = k < 0 ? -1 : M.VecCtxTwin(caller, callee, k);
        if (n < 0) return null;
        if (!Resumable && !TwinF64) System.Threading.Interlocked.Increment(ref M.VecCtxSites);
        return "__vc" + n;
    }
    bool VecCallIn(IOperation op) { Wide(); return _fa != null ? _fa.ContainsCall(op) : VecHasCall(op); }
    /// Float mode "jit32": the arguments of a call of `def` through its `__u` twin (Model.ArgTwinParams), or null for
    /// the plain call. Where the reference runs' JIT inlined `def` into this method at every site (config
    /// `jitInlined`), it substituted the caller's local for each parameter the inlinee never writes: a float local
    /// this method keeps on the x87 stack enters the body unrounded (`HourglassRoom.Perform` passes `percent` to the
    /// inlined `Utils.WrappedLerp`: `fmul dword [2.0]` on the register, the frozen world's dual dungeon). Any other
    /// argument (an expression: stored to a float temporary, I02 / I05; a field, a constant) is the rounded value.
    /// Only when at least one argument is such a local. `a`: the rendered arguments (receiver first), `n` of them args.
    List<string> InlinedArgsTwin(IMethodSymbol def, IOperation at, List<string> a, int n)
    {
        if (M.Cfg.FloatMode != "jit32" || Method == null || at is not IInvocationOperation inv) return null;
        var up = M.ArgTwinParams(def);
        if (up.Count == 0) return null;
        if (!M.JitInlinedAt(Model.ClrMethodKey(Method.OriginalDefinition), Model.ClrMethodKey(def.OriginalDefinition))) return null;
        Wide();
        if (_fa != null && _fa.MinOpts) return null;
        int off = a.Count - n;
        var ua = a.ToList();
        bool any = false;
        foreach (int pi in up)
        {
            if (off + pi >= ua.Count) return null;
            ua[off + pi] = $"(({ua[off + pi]}) as f64)";
        }
        foreach (var xa in inv.Arguments)
        {
            if (xa.Parameter == null || xa.ArgumentKind != ArgumentKind.Explicit || !up.Contains(xa.Parameter.Ordinal)) continue;
            var v = xa.Value;
            while (v is IParenthesizedOperation vp) v = vp.Operand;
            bool wide = v is ILocalReferenceOperation lv && IsWide(lv.Local) && !(_fa?.Kept.Contains(lv.Local) ?? false)
                || v is IParameterReferenceOperation pv && IsWide(pv.Parameter);
            if (!wide) continue;
            ua[off + xa.Parameter.Ordinal] = F64(v);
            any = true;
        }
        if (!any) return null;
        if (!Resumable && !TwinF64) System.Threading.Interlocked.Increment(ref M.InlinedArgSites);
        return ua;
    }
    HashSet<ISymbol> VecAddr()
    {
        if (_vecAddr != null) return _vecAddr;
        _vecAddr = new HashSet<ISymbol>(SymbolEqualityComparer.Default);
        if (_root == null) return _vecAddr;
        foreach (var d in _root.DescendantsAndSelf())
            switch (d)
            {
                case IArgumentOperation { Parameter.RefKind: not RefKind.None, Value: ILocalReferenceOperation l1 }: _vecAddr.Add(l1.Local); break;
                case IAddressOfOperation { Reference: ILocalReferenceOperation l2 }: _vecAddr.Add(l2.Local); break;
                case IInvocationOperation { Instance: ILocalReferenceOperation l3 }: _vecAddr.Add(l3.Local); break;
                case IPropertyReferenceOperation { Instance: ILocalReferenceOperation l4 }: _vecAddr.Add(l4.Local); break;
                case ISimpleAssignmentOperation { IsRef: true, Value: ILocalReferenceOperation l5 }: _vecAddr.Add(l5.Local); break;
                case IVariableDeclaratorOperation rv when rv.Symbol.RefKind != RefKind.None && rv.GetVariableInitializer()?.Value is ILocalReferenceOperation l6: _vecAddr.Add(l6.Local); break;
                case IAnonymousFunctionOperation lam:
                    foreach (var x in lam.Body.DescendantsAndSelf().OfType<ILocalReferenceOperation>()) _vecAddr.Add(x.Local);
                    break;
            }
        return _vecAddr;
    }
    /// The local of `target = value` / a declaration when its components can be tracked: a Vector2 local of a "stack"
    /// method, not address-taken, assigned an inlined operator chain without a call in it.
    ILocalSymbol VecTrackable(IOperation target, IOperation value, ILocalSymbol decl = null)
    {
        var loc = decl ?? (target as ILocalReferenceOperation)?.Local;
        if (loc == null || loc.RefKind != RefKind.None || !IsVec2(loc.Type) || !VecStackMethod() || VecAddr().Contains(loc)) return null;
        while (value is IParenthesizedOperation par) value = par.Operand;
        if (value is not IBinaryOperation { IsLifted: false, OperatorMethod: { } bm } || VecInlined(bm) != false) return null;
        if (InLambda(value) || !VecCallsOnlyFirst(value)) return null;
        return loc;
    }
    /// The calls of an inlined operator chain are all evaluated before its first operator result exists: in the
    /// leftmost operand (VertexStrip.AddVertexPair, `MathHelper.WrapAngle(..).ToRotationVector2() * num`: the two calls,
    /// then `fmul` twice, the products left on the x87 stack for `pos + vector` / `pos - vector`). Nothing on the x87
    /// stack is live across those calls, so the components can be tracked as for a chain without a call.
    bool VecCallsOnlyFirst(IOperation v)
    {
        while (v is IParenthesizedOperation par) v = par.Operand;
        // the operator itself counts where the float analysis takes it for a call (FloatAlloc.OwnCall), as before
        if (v is IBinaryOperation { IsLifted: false, OperatorMethod: { } bm } b && VecInlined(bm) != null)
            return !(_fa != null && _fa.OwnCall(b)) && VecCallsOnlyFirst(b.LeftOperand) && !VecCallIn(b.RightOperand);
        if (v is IUnaryOperation { IsLifted: false, OperatorKind: UnaryOperatorKind.Minus, OperatorMethod: { } um } u && VecInlined(um) != null)
            return !(_fa != null && _fa.OwnCall(u)) && VecCallsOnlyFirst(u.Operand);
        return true;
    }
    /// A statement that is one call whose instance and arguments contain no call, and whose tracked locals are not read
    /// after it: the arguments are evaluated from the x87 components before the call (VertexStrip.AddVertexPair,
    /// `AddVertexPair(pos + vector, pos - vector, ..)`: `fadd st(2)` / `fsubrp st(3)` on the unrounded products, each
    /// argument stored with `fstp dword [esp]`). The call ends the tracking (S, `_vecClearAfter`).
    bool VecArgsBeforeCall(IOperation op)
    {
        if (op is not IExpressionStatementOperation { Operation: IInvocationOperation inv }) return false;
        if (inv.Instance != null && VecCallIn(inv.Instance)) return false;
        if (inv.Arguments.Any(a => VecCallIn(a.Value))) return false;
        int end = op.Syntax.Span.End;
        foreach (var r in op.DescendantsAndSelf().OfType<ILocalReferenceOperation>())
        {
            if (!_vecComp.ContainsKey(r.Local)) continue;
            // read after this statement: not this rule (the JIT may have kept a rounded copy for those reads)
            if (_root != null && _root.DescendantsAndSelf().OfType<ILocalReferenceOperation>().Any(x => SymbolEqualityComparer.Default.Equals(x.Local, r.Local) && x.Syntax.SpanStart >= end)) return false;
        }
        return true;
    }
    (string x, string y)? VecTracked(ILocalReferenceOperation l)
        => _vecComp.TryGetValue(l.Local, out var c) && !InLambda(l) ? c : null;
    static bool InLambda(IOperation o) { for (var p = o.Parent; p != null; p = p.Parent) if (p is IAnonymousFunctionOperation or ILocalFunctionOperation) return true; return false; }
    /// Before a statement: a call in it, or any write to a tracked local, ends the tracking.
    void VecBeforeStatement(IOperation op)
    {
        if (_vecComp.Count == 0) return;
        if (op is not (IExpressionStatementOperation or IVariableDeclarationGroupOperation or IReturnOperation)) { _vecComp.Clear(); return; }
        if (VecCallIn(op))
        {
            if (!VecArgsBeforeCall(op)) { _vecComp.Clear(); return; }
            _vecClearAfter = true;
        }
        // `v = chain` that reads v: its components stay for that read (the pattern then replaces them)
        var self = op is IExpressionStatementOperation { Operation: ISimpleAssignmentOperation sa } ? VecTrackable(sa.Target, sa.Value) : null;
        foreach (var d in op.DescendantsAndSelf())
        {
            ILocalSymbol w = d switch
            {
                IAssignmentOperation { Target: ILocalReferenceOperation t } => t.Local,
                IAssignmentOperation { Target: IFieldReferenceOperation { Instance: ILocalReferenceOperation t2 } } => t2.Local,
                IIncrementOrDecrementOperation { Target: IFieldReferenceOperation { Instance: ILocalReferenceOperation t3 } } => t3.Local,
                _ => null,
            };
            if (w != null && !SymbolEqualityComparer.Default.Equals(w, self)) _vecComp.Remove(w);
        }
    }
    // ---- float mode "jit32": XNA Vector2 operators the JIT inlined (FLOAT_RULE.md section 6, "inlined vector operators")
    static readonly HashSet<string> VecOpNames = new() { "op_Addition", "op_Subtraction", "op_Multiply", "op_Division", "op_UnaryNegation" };
    /// A Vector2 operator that the reference runs' code inlined at every IL site of this method (jit_bodies.tsv: sites,
    /// no call instruction): null when not (the plain template), else whether each operator's result is rounded
    /// (jit_vec_temps.tsv "memory", or no answer) or kept on the x87 stack ("stack").
    bool? VecInlined(IMethodSymbol m)
    {
        if (M.Cfg.FloatMode != "jit32" || Method == null || m == null || !VecOpNames.Contains(m.Name)) return null;
        if (m.ContainingType?.ToDisplayString() != "Microsoft.Xna.Framework.Vector2" || !m.IsStatic) return null;
        string caller = Model.ClrMethodKey(Method.OriginalDefinition), callee = Model.ClrMethodKey(m.OriginalDefinition);
        if (VecCtx != null)
        {
            // a twin for a caller that inlined this method: that caller's code decides
            var opKey = callee == null ? null : callee.Substring(callee.IndexOf("::", StringComparison.Ordinal) + 2);
            return opKey != null && VecCtx.TryGetValue(opKey, out var a) && a == 'i' ? VecCtxMode != "stack" : null;
        }
        if (!M.JitBodySeen(caller, callee) || M.JitBodyCalls(caller, callee) != 0) return null;
        bool round = M.JitVecTemps(caller) != "stack";
        if (!Resumable && !TwinF64)
        {
            System.Threading.Interlocked.Increment(ref M.VecInlinedSites);
            if (!round) System.Threading.Interlocked.Increment(ref M.VecInlinedStackSites);
        }
        return round;
    }
    static bool VecHasCall(IOperation op) => op.DescendantsAndSelf().Any(x => x is IInvocationOperation or IPropertyReferenceOperation
        || x is IObjectCreationOperation { Type.IsValueType: false } || x is IArrayCreationOperation);
    static bool IsVec2(ITypeSymbol t) => Mono.Sub(t)?.ToDisplayString() == "Microsoft.Xna.Framework.Vector2";
    /// The x87 components (f64 expressions) of a Vector2 value. Inlined operators are expanded (their operands bound in
    /// `pre` in evaluation order); anything else is a leaf, the Vector2 value itself. `round`: each operator's result is
    /// a struct in memory, written with `fstp dword` (GolfHelper.ContactListener.OnCollision); else the inlinee's
    /// result struct is promoted and its fields stay on the x87 stack (BallCollision.ClosestPointOnLineSegment,
    /// `lineSegment.Start + vector * num2`: `fmul st(4)`, `faddp st(2)`, one `fstp dword` per component at the store).
    (string x, string y) VecX87(IOperation op, StringBuilder pre, bool round)
    {
        while (op is IParenthesizedOperation par) op = par.Operand;
        if (op is IBinaryOperation { IsLifted: false, OperatorMethod: { } bm } b && VecInlined(bm) != null)
            return VecCombine(bm, null, b.LeftOperand, b.RightOperand, pre, round);
        if (op is IUnaryOperation { IsLifted: false, OperatorKind: UnaryOperatorKind.Minus, OperatorMethod: { } um } u && VecInlined(um) != null)
        {
            // fchs on each component (the sign of a zero flips too; terraria-core `negate`)
            var (nx, ny) = VecX87(u.Operand, pre, round);
            return ($"(-{nx})", $"(-{ny})");
        }
        if (op is ILocalReferenceOperation tl && VecTracked(tl) is { } tc) return tc;
        // `new Vector2(a, b)` as an operand, the constructor inlined too, in a method whose operator results stay on the
        // x87 stack: the arguments ARE the components (no struct in memory), entering as an inlined operator's float
        // argument does (VecScalar: a local unrounded). BatsGroupSkyEntity.UpdateVelocity, `new Vector2(x, 0f) +
        // _velocityVsMagnet`: x stays in ST0, `fld [_velocityVsMagnet.X]`, `faddp st(2)`, then `fldz`, `faddp`
        // (work/jit/client/gallery)
        if (!round && op is IObjectCreationOperation { Constructor: { } vctor } voc && IsVec2(voc.Type) && voc.Arguments.Length == 2 && voc.Initializer == null
            && vctor.Parameters.All(p => p.Type.SpecialType == SpecialType.System_Single) && Method != null
            && M.JitBodySeen(Model.ClrMethodKey(Method.OriginalDefinition), Model.ClrMethodKey(vctor.OriginalDefinition))
            && M.JitBodyCalls(Model.ClrMethodKey(Method.OriginalDefinition), Model.ClrMethodKey(vctor.OriginalDefinition)) == 0)
        {
            var a0 = voc.Arguments.First(a => a.Parameter.Ordinal == 0).Value;
            var a1 = voc.Arguments.First(a => a.Parameter.Ordinal == 1).Value;
            string cx = VecScalar(a0, pre), cy = VecScalar(a1, pre);
            return (cx, cy);
        }
        var t = Tmp("v");
        pre.Append($"let {t}: terraria_core::xna::Vector2 = {E(op)}; ");
        return ($"({t}.x as f64)", $"({t}.y as f64)");
    }
    /// A float local whose declaration initialises it with a constant and which nothing writes afterwards (no
    /// assignment, increment, ref / out / address use, no capture by a lambda or local function): the JIT propagates
    /// it as the constant.
    static bool JitConstLocal(IOperation op)
    {
        while (op is IParenthesizedOperation or IConversionOperation { IsImplicit: true })
            op = op is IParenthesizedOperation pp ? pp.Operand : ((IConversionOperation)op).Operand;
        if (op is not ILocalReferenceOperation { Local: { } loc } lr || loc.Type.SpecialType != SpecialType.System_Single || loc.IsRef) return false;
        if (loc.DeclaringSyntaxReferences.FirstOrDefault()?.GetSyntax() is not VariableDeclaratorSyntax { Initializer.Value: { } init } decl) return false;
        if (lr.SemanticModel == null || lr.SemanticModel.SyntaxTree != decl.SyntaxTree) return false;
        if (!lr.SemanticModel.GetConstantValue(init).HasValue) return false;
        var scope = decl.Ancestors().FirstOrDefault(x => x is BaseMethodDeclarationSyntax or AccessorDeclarationSyntax or LocalFunctionStatementSyntax or AnonymousFunctionExpressionSyntax);
        if (scope == null) return false;
        foreach (var id in scope.DescendantNodes().OfType<IdentifierNameSyntax>())
        {
            if (id.Identifier.ValueText != loc.Name) continue;
            if (!SymbolEqualityComparer.Default.Equals(lr.SemanticModel.GetSymbolInfo(id).Symbol, loc)) continue;
            var par = id.Parent;
            while (par is ParenthesizedExpressionSyntax) par = par.Parent;
            if (par is AssignmentExpressionSyntax asg && asg.Left.DescendantNodesAndSelf().Contains(id)) return false;
            if (par is PrefixUnaryExpressionSyntax pu && (pu.IsKind(SyntaxKind.PreIncrementExpression) || pu.IsKind(SyntaxKind.PreDecrementExpression) || pu.IsKind(SyntaxKind.AddressOfExpression))) return false;
            if (par is PostfixUnaryExpressionSyntax) return false;
            if (par is ArgumentSyntax arg && !arg.RefKindKeyword.IsKind(SyntaxKind.None)) return false;
            if (par is RefExpressionSyntax) return false;
            if (id.Ancestors().TakeWhile(x => x != scope).Any(x => x is AnonymousFunctionExpressionSyntax or LocalFunctionStatementSyntax)) return false;
        }
        return true;
    }
    /// One inlined operator; `left` already rendered (compound assignment) or `leftOp`.
    (string x, string y) VecCombine(IMethodSymbol bm, (string x, string y)? left, IOperation leftOp, IOperation rightOp, StringBuilder pre, bool round)
    {
        bool lv = left != null || IsVec2(leftOp.Type), rv = IsVec2(rightOp.Type);
        (string x, string y) L = default, R = default;
        string sl = null, sr = null;
        if (left != null) L = left.Value; else if (lv) L = VecX87(leftOp, pre, round); else sl = VecScalar(leftOp, pre);
        // a call evaluated while the left result is held (`v * 0.5f + Utils.NextVector2CircularEdge(...)`): the
        // promoted fields live across it are spilled as floats (FLOAT_RULE.md section 1, rule 3; ClosestPointOnLineSegment:
        // `point - lineSegment.Start` stored with `fstp dword` before the call of LengthSquared)
        if (!round && lv && VecHasCall(rightOp)) L = ($"(({L.x}) as f32 as f64)", $"(({L.y}) as f32 as f64)");
        if (rv) R = VecX87(rightOp, pre, round); else sr = VecScalar(rightOp, pre);
        (string x, string y) r;
        switch (bm.Name)
        {
            case "op_Addition": r = ($"({L.x} + {R.x})", $"({L.y} + {R.y})"); break;
            case "op_Subtraction": r = ($"({L.x} - {R.x})", $"({L.y} - {R.y})"); break;
            case "op_Multiply":
                if (lv && rv) r = ($"({L.x} * {R.x})", $"({L.y} * {R.y})");
                else if (lv) r = ($"({L.x} * {sr})", $"({L.y} * {sr})");
                else r = ($"({R.x} * {sl})", $"({R.y} * {sl})");
                break;
            case "op_Division":
                if (rv) r = ($"({L.x} / {R.x})", $"({L.y} / {R.y})");
                else
                {
                    // XNA's own body: `float num = 1f / divider;` then two products (the reciprocal unrounded, as
                    // where it is called: config jitBodies)
                    var n = Tmp("n");
                    // a constant divider: the JIT folds `1f / divider` at float precision and multiplies by a 32-bit
                    // constant (Star.Update, `fallSpeed * f / 100f`: `fmul dword ptr [const]`, the reciprocal rounded;
                    // FLOAT_RULE.md section 10, item 14)
                    // a float local assigned once from a constant and never written again is a constant to the
                    // optimising JIT too (ParticleOrchestrator.Spawn_FlameWaders, `float num = 60f; ... / num`:
                    // `fld dword ptr [const]`, `fmul st(1)`; work/jit/client/probe; FLOAT_RULE.md section 10, item 33)
                    if (rightOp.ConstantValue is { HasValue: true, Value: float or double or int } || (!(_fa?.MinOpts ?? false) && JitConstLocal(rightOp)))
                        pre.Append($"let {n}: f64 = ((1.0f32 / (({sr}) as f32)) as f64); ");
                    else
                        pre.Append($"let {n}: f64 = 1.0f64 / {sr}; ");
                    r = ($"({L.x} * {n})", $"({L.y} * {n})");
                }
                break;
            default: throw new InvalidOperationException("VecCombine " + bm.Name);
        }
        if (round) r = ($"(({r.x}) as f32 as f64)", $"(({r.y}) as f32 as f64)");
        return r;
    }
    /// A float argument of an inlined Vector2 operator, bound once in `pre`. The JIT passes a local, a field and a
    /// call's return value as they are (a call's float result stays unrounded in ST0: OnCollision,
    /// `collision.Normal * Vector2.Dot(velocity, collision.Normal)`, `fmul st(1)` on the result of the call; a local on
    /// the x87 stack: ClosestPointOnLineSegment, `vector * num2`, `fmul st(4)`); any other expression is assigned to the
    /// inlinee's argument with a cast to float (OnCollision `(DirectImpactDampening - SideImpactDampening)`: `fsub`,
    /// `fstp dword [ebp-0x148]`, `fld dword [ebp-0x148]`; FLOAT_RULE.md I02, I05).
    string VecScalar(IOperation op, StringBuilder pre)
    {
        while (op is IParenthesizedOperation par) op = par.Operand;
        string v = op is ILocalReferenceOperation or IInvocationOperation or IPropertyReferenceOperation or IFieldReferenceOperation or IParameterReferenceOperation
            ? F64(op) : $"(({E(op)}) as f64)";
        var t = Tmp("s");
        pre.Append($"let {t}: f64 = {v}; ");
        return t;
    }

    string Binary(IBinaryOperation b)
    {
        if (b.OperatorMethod != null)
        {
            if (b.OperatorKind is BinaryOperatorKind.ConditionalAnd or BinaryOperatorKind.ConditionalOr) return Fail(b, "user-defined && / ||");
            if (b.IsLifted)
            {
                // lifted user-defined operator: null if either operand is null (comparisons: false; == / != compare the nulls)
                bool lcmp = b.OperatorMethod.ReturnType.SpecialType == SpecialType.System_Boolean && b.Type?.SpecialType == SpecialType.System_Boolean;
                var call = CallMethod(b.OperatorMethod, null, new[] { "__a", "__b" }, b, false, null);
                if (b.OperatorKind == BinaryOperatorKind.Equals) return $"{{ let (__l, __r) = ({E(b.LeftOperand)}, {E(b.RightOperand)}); (__l.has_value == __r.has_value) && (!__l.has_value || {{ let (__a, __b) = (__l.value, __r.value); {call} }}) }}";
                if (b.OperatorKind == BinaryOperatorKind.NotEquals) return $"{{ let (__l, __r) = ({E(b.LeftOperand)}, {E(b.RightOperand)}); (__l.has_value != __r.has_value) || (__l.has_value && {{ let (__a, __b) = (__l.value, __r.value); {call} }}) }}";
                return lcmp ? $"{E(b.LeftOperand)}.cmp2({E(b.RightOperand)}, |__a, __b| {call})" : $"{E(b.LeftOperand)}.lift2({E(b.RightOperand)}, |__a, __b| {call})";
            }
            if (VecInlined(b.OperatorMethod) is bool vround)
            {
                var pre = new StringBuilder();
                var (vx, vy) = VecX87(b, pre, vround);
                return $"{{ {pre}terraria_core::xna::Vector2::new(({vx}) as f32, ({vy}) as f32) }}";
            }
            return CallMethod(b.OperatorMethod, null, new[] { E(b.LeftOperand), E(b.RightOperand) }, b, false, new[] { b.LeftOperand.Type, b.RightOperand.Type });
        }
        // System.Decimal: Roslyn shows its operators as built in (no operator method); rt::decimal::Decimal has the
        // ones the game uses
        if (b.LeftOperand.Type?.SpecialType == SpecialType.System_Decimal && b.RightOperand.Type?.SpecialType == SpecialType.System_Decimal && !b.IsLifted)
        {
            string dl = E(b.LeftOperand), dr = E(b.RightOperand);
            switch (b.OperatorKind)
            {
                case BinaryOperatorKind.Equals: return $"({dl} == {dr})";
                case BinaryOperatorKind.NotEquals: return $"({dl} != {dr})";
                case BinaryOperatorKind.LessThan: return $"({dl}.cmp_value(&{dr}) < 0)";
                case BinaryOperatorKind.LessThanOrEqual: return $"({dl}.cmp_value(&{dr}) <= 0)";
                case BinaryOperatorKind.GreaterThan: return $"({dl}.cmp_value(&{dr}) > 0)";
                case BinaryOperatorKind.GreaterThanOrEqual: return $"({dl}.cmp_value(&{dr}) >= 0)";
                case BinaryOperatorKind.Divide: return $"rt::decimal::Decimal::div({dl}, {dr})";
            }
            return Fail(b, "decimal operator " + b.OperatorKind + " (rt::decimal has ==, !=, <, <=, >, >=, /)");
        }
        if (b.LeftOperand.Type is IPointerTypeSymbol || b.RightOperand.Type is IPointerTypeSymbol)
        {
            bool lp = b.LeftOperand.Type is IPointerTypeSymbol, rp = b.RightOperand.Type is IPointerTypeSymbol;
            string pl = E(b.LeftOperand, true), pr = E(b.RightOperand, true);
            switch (b.OperatorKind)
            {
                case BinaryOperatorKind.Add when lp && !rp: return $"{pl}.wrapping_offset(({pr}) as isize)";
                case BinaryOperatorKind.Add when rp && !lp: return $"{pr}.wrapping_offset(({pl}) as isize)";
                case BinaryOperatorKind.Subtract when lp && !rp: return $"{pl}.wrapping_offset((({pr}) as isize).wrapping_neg())";
                case BinaryOperatorKind.Subtract when lp && rp:
                    return $"(((({pl}) as isize).wrapping_sub(({pr}) as isize) / (core::mem::size_of::<{RTf(b, ((IPointerTypeSymbol)b.LeftOperand.Type).PointedAtType)}>() as isize)) as i64)";
                case BinaryOperatorKind.Equals when lp && rp: return $"(({pl}) == ({pr}))";
                case BinaryOperatorKind.NotEquals when lp && rp: return $"(({pl}) != ({pr}))";
                case BinaryOperatorKind.LessThan when lp && rp: return $"(({pl}) < ({pr}))";
                case BinaryOperatorKind.LessThanOrEqual when lp && rp: return $"(({pl}) <= ({pr}))";
                case BinaryOperatorKind.GreaterThan when lp && rp: return $"(({pl}) > ({pr}))";
                case BinaryOperatorKind.GreaterThanOrEqual when lp && rp: return $"(({pl}) >= ({pr}))";
            }
            return Fail(b, "pointer operator " + b.OperatorKind);
        }
        if (b.Type?.SpecialType == SpecialType.System_String && b.OperatorKind == BinaryOperatorKind.Add) return Concat(b);
        if (b.OperatorKind is BinaryOperatorKind.Equals or BinaryOperatorKind.NotEquals)
        {
            // `x == null` and reference equality: C# converts both sides to object first; compare the handles
            static IOperation Strip(IOperation o)
            {
                while (o is IConversionOperation { OperatorMethod: null, IsImplicit: true } c && c.Operand.Type != null && c.Type != null
                       && c.Type.SpecialType == SpecialType.System_Object && c.Operand.Type.IsReferenceType && c.Operand.Type.SpecialType != SpecialType.System_String) o = c.Operand;
                return o;
            }
            static bool IsNull(IOperation o) => o.ConstantValue.HasValue && o.ConstantValue.Value == null;
            bool neq = b.OperatorKind == BinaryOperatorKind.NotEquals;
            var lo = Strip(b.LeftOperand); var ro = Strip(b.RightOperand);
            bool refLike(IOperation o) => Mono.Sub(o.Type) is { } ot && (ot.IsReferenceType || ot.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T) && ot is not ITypeParameterSymbol && M.RT(ot) != null;
            // `t == null` in generic code instantiated with a value type: never null (the CLR boxes and compares)
            bool neverNull(IOperation o) => o.Type is ITypeParameterSymbol && Mono.Sub(o.Type) is { IsValueType: true } vt0 && vt0.OriginalDefinition.SpecialType != SpecialType.System_Nullable_T;
            if (IsNull(ro) && !IsNull(lo) && neverNull(lo)) return $"{{ let _ = {E(lo)}; {(neq ? "true" : "false")} }}";
            if (IsNull(lo) && !IsNull(ro) && neverNull(ro)) return $"{{ let _ = {E(ro)}; {(neq ? "true" : "false")} }}";
            if (IsNull(ro) && !IsNull(lo) && refLike(lo)) return neq ? $"(!{E(lo)}.is_null())" : $"{E(lo)}.is_null()";
            if (IsNull(lo) && !IsNull(ro) && refLike(ro)) return neq ? $"(!{E(ro)}.is_null())" : $"{E(ro)}.is_null()";
            if ((lo != b.LeftOperand || ro != b.RightOperand) && refLike(lo) && refLike(ro))
            {
                var eqr = $"({E(lo)}.addr() == {E(ro)}.addr())";
                return neq ? $"(!{eqr})" : eqr;
            }
        }
        if (b.IsLifted)
        {
            if (b.OperatorKind is BinaryOperatorKind.Equals or BinaryOperatorKind.NotEquals)
            {
                var eq = $"({E(b.LeftOperand)} == {E(b.RightOperand)})";
                return b.OperatorKind == BinaryOperatorKind.Equals ? eq : $"(!{eq})";
            }
            static ITypeSymbol Inner0(ITypeSymbol t) => t != null && t.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T ? ((INamedTypeSymbol)t).TypeArguments[0] : t;
            var li = Inner0(b.LeftOperand.Type); var ri = Inner0(b.RightOperand.Type); var resi = Inner0(b.Type);
            if (li == null || ri == null || !IsLiftedOperand(b.LeftOperand.Type) || !IsLiftedOperand(b.RightOperand.Type)) return Fail(b, "lifted (nullable) operator " + b.OperatorKind + " with a non-nullable operand");
            bool isCmp = b.OperatorKind is BinaryOperatorKind.LessThan or BinaryOperatorKind.LessThanOrEqual or BinaryOperatorKind.GreaterThan or BinaryOperatorKind.GreaterThanOrEqual;
            if (b.OperatorKind is BinaryOperatorKind.ConditionalAnd or BinaryOperatorKind.ConditionalOr || (!isCmp && Under(li) == SpecialType.System_Boolean)) return Fail(b, "lifted (nullable) bool operator " + b.OperatorKind);
            if (Under(li) != Under(ri) && !(b.OperatorKind is BinaryOperatorKind.LeftShift or BinaryOperatorKind.RightShift)) return Fail(b, "lifted (nullable) operator on different operand types");
            var body = BinExpr(b, b.OperatorKind, "__a", "__b", isCmp ? li : resi, li, ri);
            return $"{E(b.LeftOperand)}.{(isCmp ? "cmp2" : "lift2")}({E(b.RightOperand)}, |__a, __b| {body})";
        }
        if (b.Type?.TypeKind == TypeKind.Delegate)
        {
            var dr = M.RT(b.Type);
            if (dr == null || !dr.StartsWith("Delegate<") || M.RT(b.LeftOperand.Type) != dr || M.RT(b.RightOperand.Type) != dr) return Fail(b, "delegate combine / remove on a delegate type with no Rust representation");
            if (b.OperatorKind == BinaryOperatorKind.Add) return $"Delegate::combine({E(b.LeftOperand)}, {E(b.RightOperand)})";
            if (b.OperatorKind == BinaryOperatorKind.Subtract) return $"Delegate::remove({E(b.LeftOperand)}, {E(b.RightOperand)})";
            if (b.OperatorKind == BinaryOperatorKind.Equals) return $"({E(b.LeftOperand)} == {E(b.RightOperand)})";
            if (b.OperatorKind == BinaryOperatorKind.NotEquals) return $"({E(b.LeftOperand)} != {E(b.RightOperand)})";
            return Fail(b, "delegate operator " + b.OperatorKind);
        }
        if (X87 && DecompiledNeg(b) && Mono.Sub(b.Type)?.SpecialType == SpecialType.System_Double) return $"(-{E(b.RightOperand)})";
        if (X87 && IsF32(b.LeftOperand) && IsF32(b.RightOperand))
        {
            if (IsF32(b) && FloatArith(b.OperatorKind)) return $"({F64(b)} as f32)";
            string csym = b.OperatorKind switch
            {
                BinaryOperatorKind.LessThan => "<", BinaryOperatorKind.LessThanOrEqual => "<=", BinaryOperatorKind.GreaterThan => ">", BinaryOperatorKind.GreaterThanOrEqual => ">=",
                BinaryOperatorKind.Equals => "==", BinaryOperatorKind.NotEquals => "!=", _ => null,
            };
            if (csym != null) return $"({F64Pending(b.LeftOperand, b.RightOperand)} {csym} {F64(b.RightOperand)})";
        }
        var lt = b.LeftOperand.Type; var rt = b.RightOperand.Type;
        bool arith = b.OperatorKind is BinaryOperatorKind.Add or BinaryOperatorKind.Subtract or BinaryOperatorKind.Multiply or BinaryOperatorKind.Divide
            or BinaryOperatorKind.Remainder or BinaryOperatorKind.And or BinaryOperatorKind.Or or BinaryOperatorKind.ExclusiveOr;
        bool recv = b.OperatorKind is BinaryOperatorKind.Add or BinaryOperatorKind.Subtract or BinaryOperatorKind.Multiply or BinaryOperatorKind.LeftShift or BinaryOperatorKind.RightShift;
        string l = E(b.LeftOperand, recv), r = E(b.RightOperand, false);
        ITypeSymbol opType = b.Type;
        if (b.OperatorKind is BinaryOperatorKind.LeftShift or BinaryOperatorKind.RightShift) opType = lt;
        if (arith && lt != null && rt != null && Under(lt) != Under(rt) && IsNum(Under(lt)) && IsNum(Under(rt)))
        {
            // enum arithmetic with a differently sized operand: bring both to the result's underlying type
            var u = Under(b.Type);
            l = NumCast(l, Under(lt), u); r = NumCast(r, Under(rt), u);
        }
        if (b.OperatorKind is BinaryOperatorKind.Subtract && lt?.TypeKind == TypeKind.Enum && rt?.TypeKind == TypeKind.Enum)
            opType = ((INamedTypeSymbol)lt).EnumUnderlyingType; // E - E is the underlying type
        bool cmp = b.OperatorKind is BinaryOperatorKind.Equals or BinaryOperatorKind.NotEquals or BinaryOperatorKind.LessThan or BinaryOperatorKind.LessThanOrEqual
            or BinaryOperatorKind.GreaterThan or BinaryOperatorKind.GreaterThanOrEqual;
        if (cmp && lt != null && rt != null && IsNum(Under(lt)) && IsNum(Under(rt)) && Under(lt) != Under(rt)) return Fail(b, "comparison between different numeric types");
        return BinExpr(b, b.OperatorKind, l, r, opType, lt, rt);
    }

    static bool IsLiftedOperand(ITypeSymbol t) => t != null && t.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T;

    string Unary(IUnaryOperation u)
    {
        if (u.OperatorMethod != null && u.IsLifted) return $"{E(u.Operand)}.map(|__a| {CallMethod(u.OperatorMethod, null, new[] { "__a" }, u, false, null)})";
        if (u.OperatorMethod != null) return CallMethod(u.OperatorMethod, null, new[] { E(u.Operand) }, u, false, new[] { u.Operand.Type });
        if (X87 && !u.IsLifted && IsF32(u) && u.OperatorKind == UnaryOperatorKind.Minus) return $"({F64(u)} as f32)";
        if (u.IsLifted)
        {
            var it = ((INamedTypeSymbol)u.Operand.Type).TypeArguments[0]; var ist = Under(it);
            string body = u.OperatorKind switch
            {
                UnaryOperatorKind.Plus => "__a",
                UnaryOperatorKind.Minus => IsFloat(ist) ? "(-__a)" : IsNum(ist) ? "__a.wrapping_neg()" : null,
                UnaryOperatorKind.Not or UnaryOperatorKind.BitwiseNegation => "(!__a)",
                _ => null,
            };
            return body == null ? Fail(u, "lifted (nullable) unary operator " + u.OperatorKind) : $"{E(u.Operand)}.map(|__a| {body})";
        }
        var st = Under(u.Type);
        var e = E(u.Operand, true);
        switch (u.OperatorKind)
        {
            case UnaryOperatorKind.Plus: return e;
            case UnaryOperatorKind.Minus: return IsFloat(st) ? $"(-{e})" : IsNum(st) ? $"{e}.wrapping_neg()" : Fail(u, "unary minus on " + st);
            case UnaryOperatorKind.Not: return $"(!{e})";
            case UnaryOperatorKind.BitwiseNegation: return $"(!{e})";
        }
        return Fail(u, "unary operator " + u.OperatorKind);
    }

    // ---- strings
    void FlattenConcat(IOperation op, List<IOperation> parts)
    {
        if (op is IBinaryOperation { OperatorKind: BinaryOperatorKind.Add, OperatorMethod: null } b && b.Type?.SpecialType == SpecialType.System_String && !b.ConstantValue.HasValue)
        { FlattenConcat(b.LeftOperand, parts); FlattenConcat(b.RightOperand, parts); }
        else parts.Add(op);
    }
    string ToStr(IOperation o)
    {
        // operand of a concatenation / interpolation: C# calls ToString() on the original (unboxed) value
        while (o is IConversionOperation { OperatorMethod: null, IsImplicit: true } c && !o.ConstantValue.HasValue && c.Operand.Type != null && (c.GetConversion().IsBoxing || c.GetConversion().IsReference || c.GetConversion().IsIdentity)) o = c.Operand;
        if (o.ConstantValue.HasValue)
        {
            var v = o.ConstantValue.Value;
            if (v == null) return "Str::NULL";
            if (v is string) return E(o);
            if (v is char ch) return StrLit(ch.ToString());
            if (v is bool bb) return StrLit(bb ? "True" : "False");
        }
        var t = Mono.Sub(o.Type);
        if (t == null) return "Str::NULL";
        return ToStringOf(t, E(o, true), o, true);
    }

    /// `x.ToString()` for a value of static type `t` rendered as `e`: what the C# compiler calls for an operand of a
    /// string concatenation (`concat`: a null reference contributes nothing) or for an explicit call (null throws).
    string ToStringOf(ITypeSymbol t, string e, IOperation at, bool concat)
    {
        t = Mono.Sub(t);
        if (t.SpecialType == SpecialType.System_String) return concat ? e : $"{e}.not_null()";
        if (t.SpecialType == SpecialType.System_Char) return $"rt::char_str({e})";
        if (t.TypeKind == TypeKind.Enum) return EnumToString((INamedTypeSymbol)t, e);
        if (t.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T)
        {
            // `T?`: the value's text, or the empty string for null (Nullable<T>.ToString)
            var it = ((INamedTypeSymbol)t).TypeArguments[0];
            var n = Tmp("n");
            return $"{{ let {n} = {e}; if {n}.has_value {{ {ToStringOf(it, n + ".value", at, concat)} }} else {{ Str::lit(\"\") }} }}";
        }
        if (Model.Prim(t.SpecialType) != null) return $"rt::to_str({e})";
        if (t is ITypeParameterSymbol) return Fail(at, "ToString() of an open type parameter");
        var r = M.RT(t);
        if (r == null) return Fail(at, "ToString() of a type with no Rust representation (" + Model.Short(t) + ")");
        var tt = M.Get(t);
        if (tt != null && tt.IsStruct)
        {
            // the struct's own override, else ValueType.ToString(): the type's name
            for (var c = tt.Sym; c != null; c = null)
                foreach (var m in c.GetMembers("ToString").OfType<IMethodSymbol>())
                    if (m.IsOverride && m.Parameters.Length == 0 && M.Method(tt, m) is { Emittable: true } mi)
                    {
                        Calls.Add($"{tt.ModPath}::{mi.Name}");
                        var sv = Tmp("s");
                        return $"{{ let mut {sv} = {e}; {tt.ModPath}::{mi.Name}(core::ptr::addr_of_mut!({sv})) }}";
                    }
            return $"{{ let _ = {e}; Str::lit({Lit(Gen.ClrName(tt.Sym))}) }}";
        }
        // any reference: the virtual ToString of the object (translated override, boxed value, native object)
        if (IsGc(r)) return $"rt::{(concat ? "obj_to_string_or_empty" : "obj_to_string")}({(r == "Obj" ? e : e + ".up::<Object>()")})";
        if (IsHandleLike(r)) return $"rt::{(concat ? "obj_to_string_or_empty" : "obj_to_string")}({e}.up::<Object>())";
        // a natively implemented value type prints itself (`rt::BoxedText`: XNA structs, TimeSpan ...)
        if (t.IsValueType && !r.StartsWith(M.Cfg.HostModule + "::")) return $"rt::BoxedText::text({e})";
        return Fail(at, "ToString() of " + Model.Short(t) + " (an opaque library value)");
    }

    /// `typeof(T)` for a closed type: one `Type` object per type (`rt::type_of`); an enum carries its names, a
    /// translated type its reflection data (`gen::meta`: static fields).
    public string TypeOfExpr(ITypeSymbol tot)
    {
            if (tot is INamedTypeSymbol { TypeKind: TypeKind.Enum } toe) return $"rt::type_of_enum(&{EnumInfo(toe)})";
            string name = tot is INamedTypeSymbol { IsUnboundGenericType: true } ug ? (ug.ContainingNamespace == null || ug.ContainingNamespace.IsGlobalNamespace ? "" : ug.ContainingNamespace.ToDisplayString() + ".") + ug.MetadataName : Model.FullName(tot);
            var flags = new List<string>();
            bool prim = tot.SpecialType is SpecialType.System_Boolean or SpecialType.System_Byte or SpecialType.System_SByte or SpecialType.System_Int16 or SpecialType.System_UInt16
                or SpecialType.System_Int32 or SpecialType.System_UInt32 or SpecialType.System_Int64 or SpecialType.System_UInt64 or SpecialType.System_Char or SpecialType.System_Single
                or SpecialType.System_Double or SpecialType.System_IntPtr or SpecialType.System_UIntPtr;
            if (prim) flags.Add("rt::type_flags::PRIMITIVE");
            if (tot.IsValueType) flags.Add("rt::type_flags::VALUE_TYPE");
            if (tot is INamedTypeSymbol { IsGenericType: true }) flags.Add("rt::type_flags::GENERIC");
            if (tot.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T && !(tot is INamedTypeSymbol { IsUnboundGenericType: true })) flags.Add("rt::type_flags::NULLABLE");
            var id = tot.IsValueType && tot.OriginalDefinition.SpecialType != SpecialType.System_Nullable_T ? Tid(tot) : null;
            // `typeof` alone does not box: a native struct without a box id still has a Type object
            if (id != null && id.StartsWith("const { rt::box_id::<")) id = "rt::box_id_or_zero::<" + id.Substring("const { rt::box_id::<".Length).Replace(">() }", ">()");
            if (!NoMeta && M.Get(tot) is { WholeTypeReason: null, IsInterface: false } mt)
            {
                var mname = MetaIdent(name);
                var mflags = flags.Count == 0 ? "0" : string.Join(" | ", flags);
                M.MetaFns.GetOrAdd(name, _ => Gen.MetaText(M, mt, mname, name, mflags, id ?? "0"));
                M.MetaTypes.TryAdd(name, mt);
                return $"rt::type_of_meta(&meta::{mname})";
            }
            return $"rt::type_of({Lit(name)}, {(flags.Count == 0 ? "0" : string.Join(" | ", flags))}, {id ?? "0"})";
    }

    /// Path of the generated `rt::EnumInfo` of an enum (`gen::enums`), created on first use.
    /// For generated tables outside method bodies (the Newtonsoft member tables): the enum's `rt::EnumInfo` and its
    /// `ToString()` expression.
    public string EnumInfoRef(INamedTypeSymbol en) => EnumInfo(en);
    public string EnumToStringRef(INamedTypeSymbol en, string e) => EnumToString(en, e);

    string EnumInfo(INamedTypeSymbol en)
    {
        EnumToString(en, "0");
        var full = Model.FullName(en);
        return "enums::" + Names.Ident(Names.Snake(full.Replace('.', '_').Replace('+', '_'))).ToUpperInvariant() + "_INFO";
    }
    /// `e.ToString()` for an enum value: the generated name table of the enum (gen::enums) and .NET's rule.
    string EnumToString(INamedTypeSymbol en, string e)
    {
        var full = Model.FullName(en);
        var fn = Names.Ident(Names.Snake(full.Replace('.', '_').Replace('+', '_')));
        M.EnumFns.GetOrAdd(full, _ =>
        {
            var sbx = new StringBuilder($"/// `{full}.ToString()`\npub fn {fn}(v: i64) -> Str {{\n    static NAMES: &[(u64, &str)] = &[\n");
            // .NET sorts the values as unsigned numbers; names of equal values keep declaration order here
            var members = en.GetMembers().OfType<IFieldSymbol>().Where(f => f.HasConstantValue).Select((f, i) => (f, i, v: f.ConstantValue is ulong ul ? ul : unchecked((ulong)Convert.ToInt64(f.ConstantValue)))).OrderBy(x => x.v).ThenBy(x => x.i);
            foreach (var x in members) sbx.Append($"        ({x.v}, {Lit(x.f.Name)}),\n");
            bool flags = en.GetAttributes().Any(a => a.AttributeClass?.Name == "FlagsAttribute");
            bool signed = en.EnumUnderlyingType.SpecialType is SpecialType.System_SByte or SpecialType.System_Int16 or SpecialType.System_Int32 or SpecialType.System_Int64;
            sbx.Append($"    ];\n    rt::enum_to_str(v, NAMES, {(flags ? "true" : "false")}, {(signed ? "true" : "false")})\n}}\n");
            if (!M.EnumIds.TryGetValue(en, out int eid)) throw new InvalidOperationException("enum without a type id (not seen by Discovery): " + full);
            string under = en.EnumUnderlyingType.SpecialType switch
            {
                SpecialType.System_Byte => "U8", SpecialType.System_SByte => "I8", SpecialType.System_Int16 => "I16", SpecialType.System_UInt16 => "U16",
                SpecialType.System_UInt32 => "U32", SpecialType.System_Int64 => "I64", SpecialType.System_UInt64 => "U64", _ => "I32",
            };
            sbx.Append($"/// `{full}`: type id {eid}\npub static {fn.ToUpperInvariant()}_INFO: rt::EnumInfo = rt::EnumInfo {{ tid: {eid}, name: {Lit(full)}, names: &[\n");
            foreach (var x in en.GetMembers().OfType<IFieldSymbol>().Where(f => f.HasConstantValue).Select((f, i) => (f, i, v: f.ConstantValue is ulong ul ? ul : unchecked((ulong)Convert.ToInt64(f.ConstantValue)))).OrderBy(x => x.v).ThenBy(x => x.i))
                sbx.Append($"    ({x.v}, {Lit(x.f.Name)}),\n");
            sbx.Append($"], flags: {(flags ? "true" : "false")}, signed: {(signed ? "true" : "false")}, underlying: rt::tid::{under} }};\n\n");
            return sbx.ToString();
        });
        return $"enums::{fn}(({e}) as i64)";
    }

    string Concat(IBinaryOperation b)
    {
        var parts = new List<IOperation>();
        FlattenConcat(b, parts);
        return $"rt::concat(&[{string.Join(", ", parts.Select(ToStr))}])";
    }
    string Interp(IInterpolatedStringOperation s)
    {
        // A hole with an alignment or a format: the C# compiler emits `string.Format(composite, args)` for the whole
        // string, and so does this (`rt::fmt::format`: composite formatting of .NET Framework 4).
        if (s.Parts.Any(p => p is IInterpolationOperation { Alignment: not null } or IInterpolationOperation { FormatString: not null }))
        {
            var fmt = new StringBuilder(); var args = new List<string>();
            foreach (var p in s.Parts)
            {
                if (p is IInterpolatedStringTextOperation t)
                {
                    if (t.Text.ConstantValue.Value is not string lit) return Fail(s, "interpolated string text that is not a constant");
                    fmt.Append(lit.Replace("{", "{{").Replace("}", "}}"));
                }
                else if (p is IInterpolationOperation i)
                {
                    fmt.Append('{').Append(args.Count);
                    if (i.Alignment != null)
                    {
                        if (i.Alignment.ConstantValue.Value is not int al) return Fail(s, "interpolation alignment that is not a constant");
                        fmt.Append(',').Append(al.ToString(CultureInfo.InvariantCulture));
                    }
                    if (i.FormatString != null)
                    {
                        if (i.FormatString.ConstantValue.Value is not string fs) return Fail(s, "interpolation format that is not a constant");
                        fmt.Append(':').Append(fs.Replace("{", "{{").Replace("}", "}}"));
                    }
                    fmt.Append('}');
                    var ex = i.Expression;
                    while (ex is IConversionOperation { OperatorMethod: null, IsImplicit: true } c && !ex.ConstantValue.HasValue && c.Operand.Type != null && (c.GetConversion().IsBoxing || c.GetConversion().IsReference || c.GetConversion().IsIdentity)) ex = c.Operand;
                    if (ex.Type == null) args.Add("Gc::NULL");
                    else args.Add(Box(ex.Type, E(ex, true), ex));
                }
                else return Fail(s, "interpolated string part " + p.Kind);
            }
            return $"rt::fmt::format({StrLit(fmt.ToString())}, &[{string.Join(", ", args)}])";
        }
        var parts = new List<string>();
        foreach (var p in s.Parts)
        {
            if (p is IInterpolatedStringTextOperation t) parts.Add(E(t.Text));
            else if (p is IInterpolationOperation i) parts.Add(ToStr(i.Expression));
            else return Fail(s, "interpolated string part " + p.Kind);
        }
        return $"rt::concat(&[{string.Join(", ", parts)}])";
    }

    // ---- conversions
    string Coerce(string e, string from, string to)
    {
        if (from == null || to == null || from == to) return e;
        if (IsGc(from) && IsGc(to)) return $"{e}.up::<{Inner(to)}>()";
        return e;
    }

    string ConvRef(IOperation at, string e, ITypeSymbol s, ITypeSymbol t)
    {
        var rs = M.RT(s); var rt = M.RT(t);
        if (rs == null || rt == null) return Fail(at, "conversion involving a type with no Rust representation");
        if (rs == rt) return e;
        if (IsGc(rs) && IsGc(rt)) return $"{e}.up::<{Inner(rt)}>()";
        if (rt == "Obj" && (rs.StartsWith("Gc<rt::sys::OrderedData<"))) return $"{e}.up::<Object>()";
        if (M.Get(s) is { IsClass: true } nst && M.NativeBaseOf(nst) is { } nbs && M.RT(nbs) is { } nbr) return ConvRef(at, $"{e}.up::<{Inner(nbr)}>()", nbs, t);
        if (rt.StartsWith("Seq<"))
        {
            if (rs == "Str" && rt == "Seq<u16>") return $"rt::sys::str_seq({e})";
            // a library collection interface with no Rust type of its own (ICollection<T>, IList<T>, IReadOnly...: the
            // value is held as `object`): the runtime collection behind it, found by its Rust type at run time
            if (rs == "Obj" && s is INamedTypeSymbol { TypeKind: TypeKind.Interface } sif && Model.IsLibrary(sif) && t is INamedTypeSymbol tif
                && tif.OriginalDefinition.SpecialType == SpecialType.System_Collections_Generic_IEnumerable_T
                && sif.AllInterfaces.Any(i => SymbolEqualityComparer.Default.Equals(i, tif)))
                return $"rt::sys::obj_seq::<{rt.Substring(4, rt.Length - 5)}>({e})";
            // covariance of IEnumerable<T>: a sequence of references seen as a sequence of a base type / interface
            if (rs.StartsWith("Seq<") && IsGc(rs.Substring(4, rs.Length - 5)) && IsGc(rt.Substring(4, rt.Length - 5)))
                return $"rt::sys::seq_up::<{Inner(rs.Substring(4, rs.Length - 5))}, {Inner(rt.Substring(4, rt.Length - 5))}>({e})";
            if (rs.StartsWith("Gc<rt::sys::LinkedList<")) return $"rt::sys::LinkedList::as_seq({e})";
            if (rs.StartsWith("Gc<rt::sys::OrderedData<")) return $"rt::sys::OrderedData::to_seq({e})";
            if (rs.StartsWith("Gc<rt::sys::Grouping<")) return $"rt::sys::Grouping::to_seq({e})";
            if (rs.StartsWith("Gc<List<")) return $"Seq::from_list({e})";
            // array covariance: `Derived[]` where `IEnumerable<Base>` is expected
            if (rs.StartsWith("Arr<Gc<") && rt.StartsWith("Seq<") && rs.Substring(4, rs.Length - 5) != Norm(rt.Substring(4, rt.Length - 5)) && IsGc(rt.Substring(4, rt.Length - 5)))
                return $"rt::sys::arr_up_seq::<_, {Inner(rt.Substring(4, rt.Length - 5))}>({e})";
            if (rs.StartsWith("Arr<")) return $"Seq::from_arr({e})";
            if (rs.StartsWith("Gc<HashSet<")) return $"Seq::from_hash_set({e})";
            if (rs.StartsWith("Gc<Stack<")) return $"Seq::from_stack({e})";
            if (rs.StartsWith("Gc<Queue<")) return $"Seq::from_queue({e})";
            if (rs.StartsWith("Gc<Dictionary<")) return $"Dictionary::as_seq({e})";
            // a translated class that implements IEnumerable<T>: a sequence whose enumerators are the class's own
            // (its GetEnumerator(), then MoveNext / Current of whatever object that returns)
            if (IsGc(rs) && M.Get(s) is { } stt && s is INamedTypeSymbol sn && t is INamedTypeSymbol tn && tn.OriginalDefinition.SpecialType == SpecialType.System_Collections_Generic_IEnumerable_T)
            {
                var ge = tn.GetMembers("GetEnumerator").OfType<IMethodSymbol>().FirstOrDefault();
                var impl = ge == null ? null : sn.FindImplementationForInterfaceMember(ge) as IMethodSymbol;
                var mv = M.Comp.GetTypeByMetadataName("System.Collections.IEnumerator")?.GetMembers("MoveNext").OfType<IMethodSymbol>().FirstOrDefault();
                var cur = (ge?.ReturnType as INamedTypeSymbol)?.GetMembers("Current").OfType<IPropertySymbol>().FirstOrDefault()?.GetMethod;
                if (impl != null && mv != null && cur != null && M.RT(ge.ReturnType) == "Obj")
                {
                    var getE = CallMethod(impl, R("__src", s, at, true), new List<string>(), at, false, null);
                    var mvE = CallMethod(mv, R("__en", mv.ContainingType, at, true), new List<string>(), at, false, null);
                    var curE = CallMethod(cur, R("__en", ge.ReturnType, at, true), new List<string>(), at, false, null);
                    return $"rt::iter::seq_of_object::<{Inner(rs)}, {rt.Substring(4, rt.Length - 5)}>({e}, |__src: {rs}| unsafe {{ {getE} }}, |__en: Obj| unsafe {{ {mvE} }}, |__en: Obj| unsafe {{ {curE} }})";
                }
            }
        }
        if (rs.StartsWith("Arr<") && rt.StartsWith("Arr<"))
        {
            // array covariance: `Derived[]` where `Base[]` / `object[]` is expected
            var es = rs.Substring(4, rs.Length - 5); var et = rt.Substring(4, rt.Length - 5);
            // an array of class handles: the SAME array under the other element type (writes through either are seen
            // by both; the CLR's ArrayTypeMismatchException on a store of a wrong type is not modelled)
            if (IsGc(es) && IsGc(et)) return $"rt::sys::arr_up::<{Inner(es)}, {Inner(et)}>({e})";
            // `string[]` as `object[]`: strings are not objects in this runtime, so the elements are boxed into a NEW
            // array (a snapshot: later writes to either array are not seen by the other). The game's uses pass the
            // array to string.Format and friends, which only read it.
            if (es == "Str" && et == "Obj") return $"rt::sys::arr_str_obj({e})";
        }
        // a value / string / array / delegate / sequence seen as object or as an interface
        if (rt == "Obj" || (IsGc(rt) && !IsGc(rs))) { var bx = Box(s, e, at); return rt == "Obj" ? bx : $"{bx}.up::<{Inner(rt)}>()"; }
        return Fail(at, $"reference conversion {Model.Short(s.OriginalDefinition)} -> {Model.Short(t.OriginalDefinition)}");
    }

    string Conv(IConversionOperation c)
    {
        var conv = c.GetConversion();
        var s = Mono.Sub(c.Operand.Type); var t = Mono.Sub(c.Type);
        if (c.OperatorMethod != null) return CallMethod(c.OperatorMethod, null, new[] { E(c.Operand) }, c, false, new[] { s });
        if (t == null) return Fail(c, "conversion to unknown type");
        if (s == null)
        {
            // null / default / lambda / method group
            if (c.Operand is IAnonymousFunctionOperation or IMethodReferenceOperation) return Fail(c, "lambda / method group converted without delegate creation");
            if (c.Operand.ConstantValue.HasValue && c.Operand.ConstantValue.Value == null) return Const(null, t, false);
            if (c.Operand is IDefaultValueOperation) return Zero(t);
            if (c.Operand is IThrowOperation cth) return Throw(cth);
            return Fail(c, "conversion from an untyped expression: " + c.Operand.Kind);
        }
        if (conv.IsIdentity) return E(c.Operand);
        if (s is IPointerTypeSymbol || t is IPointerTypeSymbol)
        {
            // pointer <-> pointer, pointer <-> IntPtr / integer: the address under another type
            bool sok = s is IPointerTypeSymbol || IsNum(Under(s)) || M.RT(s) == "isize", tok = t is IPointerTypeSymbol || IsNum(Under(t)) || M.RT(t) == "isize";
            if (!sok || !tok) return Fail(c, "pointer conversion " + Model.Short(s) + " -> " + Model.Short(t));
            return $"(({E(c.Operand, true)}) as {RTf(c, t)})";
        }
        if (c.IsChecked) return Fail(c, "checked conversion");
        var su = Under(s); var tu = Under(t);
        // IntPtr / UIntPtr <-> an integer type: the same number (the original is a 32-bit process: no value is lost
        // that it would not lose there)
        if (conv.IsIntPtr && (M.RT(s) is "isize" or "usize" || IsNum(su)) && (M.RT(t) is "isize" or "usize" || IsNum(tu)))
            return $"(({E(c.Operand, true)}) as {RTf(c, t)})";
        // System.Decimal (rt::decimal::Decimal): the conversions .NET has built in
        if (conv.IsNumeric && (su == SpecialType.System_Decimal || tu == SpecialType.System_Decimal) && M.RT(s) != null && M.RT(t) != null)
        {
            if (su == tu) return E(c.Operand);
            if (tu == SpecialType.System_Decimal)
            {
                if (!IsNum(su)) return Fail(c, "conversion to decimal from " + Model.Short(s));
                return $"rt::decimal::Decimal::from_{Model.Prim(su)}({E(c.Operand)})";
            }
            if (!IsNum(tu)) return Fail(c, "conversion from decimal to " + Model.Short(t));
            return $"{E(c.Operand)}.to_{(tu == SpecialType.System_Char ? "u16" : Model.Prim(tu))}()";
        }
        if (conv.IsNumeric || (conv.IsEnumeration && IsNum(su) && IsNum(tu)) || (IsNum(su) && IsNum(tu) && !conv.IsNullable && !conv.IsBoxing && !conv.IsUnboxing))
        {
            if (X87 && su == SpecialType.System_Single && tu != SpecialType.System_Single)
            {
                // the x87 value is converted as it is: no rounding to 32 bits in between
                var wide = F64(c.Operand);
                return tu == SpecialType.System_Double ? wide : $"rt::conv::to_{(tu == SpecialType.System_Char ? "u16" : Model.Prim(tu))}({wide})";
            }
            return NumCast(E(c.Operand, true), su, tu);
        }
        if (conv.IsNullable)
        {
            bool sN = s.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T, tN = t.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T;
            if (!sN && tN)
            {
                var inner = ((INamedTypeSymbol)t).TypeArguments[0];
                var e = E(c.Operand, true);
                if (!SymbolEqualityComparer.Default.Equals(inner, s))
                {
                    if (IsNum(Under(inner)) && IsNum(su)) e = NumCast(e, su, Under(inner)); else return Fail(c, "nullable conversion with a non-numeric inner conversion");
                }
                return $"Nullable::some({e})";
            }
            if (sN && !tN)
            {
                var inner = ((INamedTypeSymbol)s).TypeArguments[0];
                var e = $"{E(c.Operand)}.value()";
                if (!SymbolEqualityComparer.Default.Equals(inner, t))
                {
                    if (IsNum(Under(inner)) && IsNum(tu)) e = NumCast(e, Under(inner), tu); else return Fail(c, "nullable conversion with a non-numeric inner conversion");
                }
                return e;
            }
            if (sN && tN)
            {
                var si = ((INamedTypeSymbol)s).TypeArguments[0]; var ti = ((INamedTypeSymbol)t).TypeArguments[0];
                if (IsNum(Under(si)) && IsNum(Under(ti))) return $"{E(c.Operand)}.map(|__n| {NumCast("__n", Under(si), Under(ti))})";
            }
            return Fail(c, "nullable to nullable conversion");
        }
        // generic code instantiated with reference types: Roslyn's "boxing" / "unboxing" of a type parameter is a
        // reference conversion (an upcast, or a checked downcast)
        bool genericRef = (conv.IsBoxing || conv.IsUnboxing) && s.IsReferenceType && t.IsReferenceType && (c.Operand.Type is ITypeParameterSymbol || c.Type is ITypeParameterSymbol);
        bool genericValue = !genericRef && (c.Operand.Type is ITypeParameterSymbol || c.Type is ITypeParameterSymbol) && !(s is ITypeParameterSymbol) && !(t is ITypeParameterSymbol);
        // `(T)x` / `(U)t` where both sides turn out to be the same value type
        if (genericValue && s.IsValueType && t.IsValueType && SymbolEqualityComparer.Default.Equals(s, t)) return E(c.Operand);
        if (genericValue && conv.IsReference && s.IsValueType && t.IsReferenceType) { var bx0 = Box(s, E(c.Operand, true), c); var brt0 = M.RT(t); return brt0 == null || brt0 == "Obj" || !IsGc(brt0) ? bx0 : $"{bx0}.up::<{Inner(brt0)}>()"; }
        if (genericValue && conv.IsReference && s.IsReferenceType && t.IsValueType)
        {
            var ur0 = M.RT(s);
            if (ur0 == null || !IsGc(ur0)) return Fail(c, "unboxing from a type with no handle representation");
            var ue0 = E(c.Operand);
            return Unbox(t, ur0 == "Obj" ? ue0 : $"{ue0}.up::<Object>()", c, c.IsTryCast);
        }
        if (conv.IsBoxing && !genericRef) { var bx = Box(s, E(c.Operand, true), c); var brt = M.RT(t); return brt == null || brt == "Obj" || !IsGc(brt) ? bx : $"{bx}.up::<{Inner(brt)}>()"; }
        if (conv.IsUnboxing && !genericRef)
        {
            var ur = M.RT(s);
            if (ur == null || !IsGc(ur)) return Fail(c, "unboxing from a type with no handle representation");
            var ue = E(c.Operand);
            return Unbox(t, ur == "Obj" ? ue : $"{ue}.up::<Object>()", c, c.IsTryCast);
        }
        if (conv.IsReference || genericRef || (s.IsReferenceType && t.IsReferenceType) || s is ITypeParameterSymbol || t is ITypeParameterSymbol)
        {
            if ((c.IsImplicit || conv.IsImplicit) && !(genericRef && conv.IsUnboxing)) return ConvRef(c, E(c.Operand), s, t);
            var rs = M.RT(s); var rt = M.RT(t);
            if (rs == null || rt == null) return Fail(c, "cast involving a type with no Rust representation");
            var tt = M.Get(t);
            if (tt != null && tt.WholeTypeReason != null) tt = null;
            if (tt != null && tt.IsInterface && IsGc(rs))
            {
                var ie = E(c.Operand);
                return $"rt::{(c.IsTryCast ? "as_iface" : "cast_iface")}({(rs == "Obj" ? ie : ie + ".up::<Object>()")}, {tt.ModPath}::implements)";
            }
            if (tt == null || !tt.IsClass || tt.TypeId == 0 || !IsGc(rs))
            {
                // cast towards a type the tool has no type id for
                // two C# classes behind one native Rust type (Stream -> MemoryStream): the table's "is" decides
                if (IsGc(rs) && M.Native(t) is { Kind: "class", Is: not null }) { var ne = E(c.Operand); return RtCast(rt, t, rs == "Obj" ? ne : $"{ne}.up::<Object>()", c.IsTryCast); }
                if (rs == rt && s is ITypeParameterSymbol or IArrayTypeSymbol) return E(c.Operand);
                if (rs == rt)
                {
                    // the same Rust type for two C# types (`object` and an unmapped library class are both `Obj`): an
                    // upcast needs nothing; `x as LibraryClass` must still be decided, by the host
                    var cc = M.Comp.ClassifyConversion(s, t);
                    if (cc.IsIdentity || cc.IsImplicit || !(c.IsTryCast && rt == "Obj" && t is INamedTypeSymbol tl0 && Model.IsLibrary(tl0))) return E(c.Operand);
                }
                // object <-> an unconstrained type parameter
                if (t is ITypeParameterSymbol && rt == Names.Ident(t.Name) && IsGc(rs) && !c.IsTryCast) { var ge = E(c.Operand); return $"<{rt} as rt::Trace>::from_obj({(rs == "Obj" ? ge : ge + ".up::<Object>()")})"; }
                if (s is ITypeParameterSymbol && rs == Names.Ident(s.Name) && rt == "Obj") return $"rt::Trace::to_obj(&{E(c.Operand)})";
                if (IsGc(rs) && !(t is ITypeParameterSymbol) && !(s is ITypeParameterSymbol))
                {
                    var oe = E(c.Operand);
                    var o = rs == "Obj" ? oe : $"{oe}.up::<Object>()";
                    // downcast to string / array / runtime collection / delegate
                    if (t.SpecialType == SpecialType.System_String || !IsGc(rt) || IsRtCast(rt, t))
                    {
                        if (rt.StartsWith("Seq<")) return Fail(c, "cast from object to IEnumerable<T>");
                        return Unbox(t, o, c, c.IsTryCast);
                    }
                    // a library exception class: every one of them is the runtime's Exception object with its kind
                    if (rt == "Gc<Exception>") return $"rt::{(c.IsTryCast ? "as_exception" : "cast_exception")}({o}, {Lit(Model.FullName(t))}, types::is_exception)";
                    // opaque library class / interface: only the host can tell
                    if (t is INamedTypeSymbol lt && Model.IsLibrary(lt) && rt == "Obj")
                    {
                        if (M.Cfg.TypeTests.TryGetValue(M.DocId(lt.OriginalDefinition), out var ctest))
                        {
                            var cv = Tmp("tc"); var ok = ctest.Replace("{0}", cv);
                            return c.IsTryCast
                                ? $"{{ let {cv}: Obj = {o}; if !{cv}.is_null() && {ok} {{ {cv} }} else {{ Gc::NULL }} }}"
                                : $"{{ let {cv}: Obj = {o}; if {cv}.is_null() || {ok} {{ {cv} }} else {{ rt::throw(rt::exc(\"System.InvalidCastException\", Str::lit(\"Specified cast is not valid.\"))) }} }}";
                        }
                        var name = Lit(M.Host.TypeTest(lt, OwnerName));
                        return $"{M.Cfg.HostRoot}::{(c.IsTryCast ? "as_instance" : "cast_instance")}({o}, {name})";
                    }
                    // a game class that is not translated (opaque): nothing can check it
                    if (rt == "Obj" && t is INamedTypeSymbol gt && !Model.IsLibrary(gt)) { Unported.Add("cast to " + Model.ShortMember(gt) + " (type not translated)"); return $"rt::unported::<Obj>({Lit("cast to " + Model.ShortMember(gt) + " (type not translated)")})"; }
                }
                return Fail(c, (c.IsTryCast ? "as" : "cast") + " to a type that is not a translated class (" + Model.Short(t.OriginalDefinition) + ")");
            }
            return $"rt::{(c.IsTryCast ? "as_class" : "cast_class")}::<_, {tt.StructPath}>({E(c.Operand)}, {tt.ModPath}::TYPE_ID, &types::PARENT)";
        }
        return Fail(c, "conversion " + Model.Short(s) + " -> " + Model.Short(t));
    }

    string Coalesce(ICoalesceOperation cl)
    {
        var vt = cl.Value.Type; var t = cl.Type;
        var rt = RTf(cl, t);
        var v = Tmp("c");
        if (vt != null && vt.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T)
        {
            bool resNullable = t.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T;
            return $"{{ let {v} = {E(cl.Value)}; if {v}.has_value {{ {(resNullable ? v : v + ".value")} }} else {{ {E(cl.WhenNull)} }} }}";
        }
        var val = vt == null ? "rt::zero()" : E(cl.Value);
        var conv = vt == null ? v : ConvRef(cl, v, vt, t);
        return $"{{ let {v} = {val}; if !{v}.is_null() {{ {conv} }} else {{ {E(cl.WhenNull)} }} }}";
    }

    string CondAccess(IConditionalAccessOperation ca, bool stmt)
    {
        var vt = ca.Operation.Type;
        var vr = M.RT(vt);
        if (vr == null) return Fail(ca, "conditional access on a type with no Rust representation");
        if (vt.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T) return Fail(ca, "conditional access on a nullable value type");
        var v = Tmp("ca");
        var recv = E(ca.Operation);
        _condAccess.Push(v);
        string when;
        try { when = E(ca.WhenNotNull); }
        finally { _condAccess.Pop(); }
        if (stmt) return $"{{ let {v} = {recv}; if !{v}.is_null() {{ let _ = {when}; }} }}";
        var wt = ca.WhenNotNull.Type; var t = ca.Type;
        if (t.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T && wt.OriginalDefinition.SpecialType != SpecialType.System_Nullable_T) when = $"Nullable::some({when})";
        return $"{{ let {v} = {recv}; if !{v}.is_null() {{ {when} }} else {{ rt::zero() }} }}";
    }

    string Pattern(IOperation value, IPatternOperation pat, IOperation at)
    {
        switch (pat)
        {
            case INegatedPatternOperation np: return $"(!{Pattern(value, np.Pattern, at)})";
            case IConstantPatternOperation cp:
                {
                    if (cp.Value.ConstantValue.HasValue && cp.Value.ConstantValue.Value == null)
                    {
                        var vt = value.Type;
                        if (vt != null && vt.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T) return $"(!{E(value)}.has_value)";
                        return $"{E(value)}.is_null()";
                    }
                    var st = Under(value.Type);
                    if (value.Type?.SpecialType == SpecialType.System_String) return $"rt::str_eq({E(value)}, {E(cp.Value)})";
                    if (Model.Prim(st) != null) return $"({E(value)} == {E(cp.Value)})";
                    return Fail(at, "constant pattern on " + (value.Type == null ? "?" : Model.Short(value.Type)));
                }
            case IDeclarationPatternOperation dp when dp.MatchedType != null:
                {
                    if (value.Type == null) return Fail(at, "type pattern on an untyped value");
                    var mt = dp.MatchedType;
                    var mr = M.RT(mt);
                    if (dp.DeclaredSymbol is not ILocalSymbol loc)
                    {
                        var only = IsType(E(value), value.Type, mt);
                        return only ?? Fail(at, "type pattern with no rule (" + Model.Short(value.Type) + " is " + Model.Short(mt) + ")");
                    }
                    if (mr == null || IsCell(loc)) return Fail(at, "type pattern declaring a variable with no Rust representation / a captured variable");
                    var v = Tmp("pv"); var ln = LocalName(loc);
                    _pendingDecls.Add($"let mut {ln}: {mr} = {Zero(mt)};");
                    // same static type (or a base of it): `x is T v` is a null test
                    var vr = M.RT(value.Type);
                    // (decided on the C# types: two different classes can share one Rust type, e.g. `object` and an
                    // unmapped library class are both `Obj`, and then the type test is NOT implied)
                    var upConv = M.Comp.ClassifyConversion(Mono.Sub(value.Type), Mono.Sub(mt));
                    if (vr == mr && !mt.IsValueType && (upConv.IsIdentity || upConv.IsImplicit && upConv.IsReference)) return $"{{ let {v} = {E(value)}; if !{v}.is_null() {{ {ln} = {v}; true }} else {{ false }} }}";
                    var test = IsType(v, value.Type, mt);
                    if (test == null) return Fail(at, "type pattern with no rule (" + Model.Short(value.Type) + " is " + Model.Short(mt) + ")");
                    string conv;
                    var o = vr == "Obj" ? v : $"{v}.up::<Object>()";
                    if (mt.SpecialType == SpecialType.System_String || mt.IsValueType || !IsGc(mr)) conv = Unbox(mt, o, at, false);
                    else conv = $"{v}.up::<{Inner(mr)}>()";
                    return $"{{ let {v} = {E(value)}; if {test} {{ {ln} = {conv}; true }} else {{ false }} }}";
                }
            case ITypePatternOperation tp:
                {
                    var only = value.Type == null ? null : IsType(E(value), value.Type, tp.MatchedType);
                    return only ?? Fail(at, "type pattern with no rule");
                }
        }
        return Fail(at, "pattern: " + pat.Kind);
    }

    // ---- assignment
    /// May `place = value` be emitted as is? Rust evaluates the right side before the place, C# the place
    /// first (DESIGN.md section 4). Direct form is exact when the order cannot be observed.
    bool DirectOk(IOperation target, IOperation value)
    {
        if (value.ConstantValue.HasValue) return true;
        if (Pure(target) && Pure(value)) return true;
        var locals = new HashSet<ISymbol>(SymbolEqualityComparer.Default);
        if (!LocalRooted(target, locals)) return false;
        foreach (var d in value.DescendantsAndSelf())
        {
            IOperation w = d switch
            {
                IAssignmentOperation asg => asg.Target,
                IIncrementOrDecrementOperation inc => inc.Target,
                IArgumentOperation { Parameter.RefKind: RefKind.Ref or RefKind.Out } arg => arg.Value,
                _ => null,
            };
            while (w is IFieldReferenceOperation { Instance.Type.IsValueType: true } f) w = f.Instance;
            if (w is ILocalReferenceOperation l && locals.Contains(l.Local)) return false;
            if (w is IParameterReferenceOperation p && locals.Contains(p.Parameter)) return false;
            if (d is IAnonymousFunctionOperation) return false;
        }
        return true;
    }

    /// A place whose address depends only on locals / parameters / `this` (no indexing, no statics read, no calls).
    static bool LocalRooted(IOperation o, HashSet<ISymbol> locals)
    {
        switch (o)
        {
            case ILocalReferenceOperation l: locals.Add(l.Local); return true;
            case IParameterReferenceOperation p: locals.Add(p.Parameter); return true;
            case IInstanceReferenceOperation: return true;
            case IFieldReferenceOperation f: return f.Instance == null || LocalRooted(f.Instance, locals);
            default: return false;
        }
    }

    /// The store barrier of the incremental collector (DESIGN.md section 9): `rt::wbv(value)` around the value side
    /// of a store when a value of type `t` can hold a handle. Identity for everything else.
    string Wb(ITypeSymbol t, string val)
    {
        if (t == null || val == null) return val;
        t = Mono.Sub(t);
        if (t is IPointerTypeSymbol or ITypeParameterSymbol || t.TypeKind == TypeKind.Enum) return val;
        var r = M.RT(t);
        if (r == null || r.StartsWith("*") || !Gen.NeedsTrace(r)) return val;
        return $"rt::wbv({val})";
    }

    /// A store that stays in the method's own frame: a local or by-value parameter that is not a closure cell, or
    /// a field of such a struct variable. Frames are never traced (no generated code is on the stack when the
    /// collector ends a mark phase; an iterator method's frame is an object that is traced again then).
    bool FrameStore(IOperation target)
    {
        while (target is IFieldReferenceOperation { Instance.Type.IsValueType: true } f) target = f.Instance;
        return target is ILocalReferenceOperation { Local.RefKind: RefKind.None } l && !IsCell(l.Local)
            || target is IParameterReferenceOperation { Parameter.RefKind: RefKind.None } pp && !IsCell(pp.Parameter);
    }
    string WbTo(IOperation target, string val) => FrameStore(target) ? val : Wb(target.Type, val);

    string Assign(ISimpleAssignmentOperation a, bool asValue)
    {
        if (a.IsRef)
        {
            // `r = ref place;` re-points the ref local (a `*mut T`); `_ = ref a[i];` only evaluates the place (bounds check)
            if (asValue) return Fail(a, "ref assignment used as a value");
            if (a.Target is IDiscardOperation) return $"let _ = {Addr(a.Value)};";
            if (a.Target is ILocalReferenceOperation { Local.RefKind: not RefKind.None } rl) return $"{LocalName(rl.Local)} = {Addr(a.Value)};";
            return Fail(a, "ref assignment to something other than a ref local");
        }
        var target = a.Target;
        if (target is IPropertyReferenceOperation pr && !(NativeTemplate(pr.Property) is { IsPlace: true }))
        {
            if (!asValue) return PropSet(pr, Wb(pr.Type, E(a.Value))) + ";";
            var v = Tmp("v");
            return $"{{ let {v} = {Wb(pr.Type, E(a.Value))}; {PropSet(pr, v)}; {v} }}";
        }
        if (target is IFieldReferenceOperation sf0 && M.Get(sf0.Field.ContainingType) == null && M.Stub(sf0.Field.OriginalDefinition) != null && NativeTemplate(sf0.Field.OriginalDefinition) == null)
        {
            // a field the config replaces by a value: a write has nowhere to go; recorded, the value is still evaluated
            var gapName = Model.ShortMember(sf0.Field) + " (write to a field the config stubs)";
            Unported.Add(gapName); Gaps.Add(gapName);
            var sv = Tmp("v");
            return asValue ? $"{{ let {sv} = {E(a.Value)}; rt::gap({Lit(gapName)}); {sv} }}" : $"{{ let _ = {E(a.Value)}; rt::gap({Lit(gapName)}); }}";
        }
        if (target is IFieldReferenceOperation hf && IsHostField(hf.Field))
        {
            if (!asValue) return HostCall(hf.Field, "field-set", hf.Instance, new[] { Wb(hf.Type, E(a.Value)) }, a) + ";";
            var hv = Tmp("v");
            return $"{{ let {hv} = {Wb(hf.Type, E(a.Value))}; {HostCall(hf.Field, "field-set", hf.Instance, new[] { hv }, a)}; {hv} }}";
        }
        if (target is IDiscardOperation) return asValue ? E(a.Value) : $"let _ = {E(a.Value)};";
        if (target is ITupleOperation or IDeclarationExpressionOperation { Expression: ITupleOperation }) return Fail(a, "deconstruction");
        if (target is IEventReferenceOperation ter)
        {
            // `Event = null` / `Event = handler` inside the declaring class: the delegate field
            var ep = EventPlace(ter);
            if (ep == null) return Fail(a, "event assignment (not a field-like event of a translated class)");
            if (!asValue) return $"{ep} = rt::wbv({E(a.Value)});";
            var ev0 = Tmp("v");
            return $"{{ let {ev0} = rt::wbv({E(a.Value)}); {ep} = {ev0}; {ev0} }}";
        }
        if (target is ILocalReferenceOperation wt && IsWide(wt.Local))
        {
            var wn = LocalName(wt.Local);
            return asValue ? $"{{ {wn} = {F64(a.Value)}; ({wn} as f32) }}" : $"{wn} = {F64(a.Value)};";
        }
        if (!DirectOk(target, a.Value))
        {
            var p = Tmp("p"); var v = Tmp("v");
            var addr = Addr(target); var val = WbTo(target, E(a.Value));
            return asValue ? $"{{ let {p} = {addr}; let {v} = {val}; *{p} = {v}; {v} }}" : $"{{ let {p} = {addr}; let {v} = {val}; *{p} = {v}; }}";
        }
        if (asValue)
        {
            var v = Tmp("v");
            var val = WbTo(target, E(a.Value)); var pl = P(target);
            return $"{{ let {v} = {val}; {pl} = {v}; {v} }}";
        }
        {
            var val = WbTo(target, E(a.Value)); var pl = P(target);
            return $"{pl} = {val};";
        }
    }

    string Compound(ICompoundAssignmentOperation ca, bool asValue)
    {
        var target = ca.Target;
        if (ca.IsLifted) return Fail(ca, "lifted (nullable) compound assignment");
        if (target is IEventReferenceOperation) return Fail(ca, "event subscription (+= / -=)");
        if (target.Type?.TypeKind == TypeKind.Delegate)
        {
            var dr = M.RT(target.Type);
            if (dr == null || !dr.StartsWith("Delegate<") || M.RT(ca.Value.Type) != dr || !IsPlace(target) || ca.OperatorKind is not (BinaryOperatorKind.Add or BinaryOperatorKind.Subtract))
                return Fail(ca, "delegate combine / remove on a delegate with no Rust representation or a non-place");
            var dp = Tmp("p");
            var dbody = $"let {dp} = {Addr(target)}; *{dp} = rt::wbv(Delegate::{(ca.OperatorKind == BinaryOperatorKind.Add ? "combine" : "remove")}(*{dp}, {E(ca.Value)}));";
            return asValue ? $"{{ {dbody} *{dp} }}" : $"{{ {dbody} }}";
        }
        if (target.Type is IPointerTypeSymbol)
        {
            if (!Pure(target) || ca.OperatorKind is not (BinaryOperatorKind.Add or BinaryOperatorKind.Subtract)) return Fail(ca, "pointer compound assignment on an impure place / with operator " + ca.OperatorKind);
            var ppl = P(target); var pn = E(ca.Value, true);
            var pupd = ca.OperatorKind == BinaryOperatorKind.Add ? $"{ppl} = {ppl}.wrapping_offset(({pn}) as isize);" : $"{ppl} = {ppl}.wrapping_offset((({pn}) as isize).wrapping_neg());";
            return asValue ? $"{{ {pupd} {ppl} }}" : pupd;
        }
        if (target is IFieldReferenceOperation hfc && IsHostField(hfc.Field)) return Fail(ca, "compound assignment on a library field with no native mapping");
        var inC = ca.GetInConversion(); var outC = ca.GetOutConversion();
        if (inC.IsUserDefined || outC.IsUserDefined) return Fail(ca, "compound assignment with user-defined conversion");
        var tt = target.Type;
        bool isString = tt?.SpecialType == SpecialType.System_String && ca.OperatorKind == BinaryOperatorKind.Add && ca.OperatorMethod == null;
        ITypeSymbol opType = inC.IsIdentity ? tt : ca.Value.Type;
        if (ca.OperatorKind is BinaryOperatorKind.LeftShift or BinaryOperatorKind.RightShift && !inC.IsIdentity)
            opType = M.Comp.GetSpecialType(SpecialType.System_Int32);
        string Op(string cur)
        {
            if (isString)
            {
                var parts = new List<IOperation>(); FlattenConcat(ca.Value, parts);
                return $"rt::concat(&[{cur}, {string.Join(", ", parts.Select(ToStr))}])";
            }
            if (ca.OperatorMethod != null && VecInlined(ca.OperatorMethod) is bool cround)
            {
                // `v op= w`: `v = v op w` with the operator inlined (VecX87); the target's value is loaded first
                var pre = new StringBuilder();
                var lt = Tmp("v");
                pre.Append($"let {lt}: terraria_core::xna::Vector2 = {cur}; ");
                var (cx, cy) = VecCombine(ca.OperatorMethod, ($"({lt}.x as f64)", $"({lt}.y as f64)"), null, ca.Value, pre, cround);
                return $"{{ {pre}terraria_core::xna::Vector2::new(({cx}) as f32, ({cy}) as f32) }}";
            }
            if (ca.OperatorMethod != null) return CallMethod(ca.OperatorMethod, null, new[] { cur, E(ca.Value) }, ca, false, new[] { tt, ca.Value.Type });
            // x87: `x op= tree` is one x87 expression `x op tree`, rounded by the store, also where the target is not
            // a pure place (an array element, `localAI[0] += (Math.Abs(velocity.X) + Math.Abs(velocity.Y)) * 0.01f *
            // (float)direction`: refdata/entities, falling stars, localAI[0] at tick 3 is 0.21 (0x3e570a3d), with the
            // tree rounded first 0x3e570a3e) or a property (getter, x op tree, setter argument)
            if (X87 && tt?.SpecialType == SpecialType.System_Single && FloatArith(ca.OperatorKind) && inC.IsIdentity && outC.IsIdentity && IsF32(ca.Value))
            {
                string fsym = ca.OperatorKind switch { BinaryOperatorKind.Add => "+", BinaryOperatorKind.Subtract => "-", BinaryOperatorKind.Multiply => "*", BinaryOperatorKind.Divide => "/", _ => "%" };
                // a property getter with an unrounded twin (Model.TwinF64): `Opacity += 0.1f` adds to the getter's
                // unrounded `1f - (float)alpha / 255f` (refdata/entities, projectile 961: alpha 151, not 150)
                var curF = _f64Alt is { } alt && alt.plain == cur ? alt.alt : $"(({cur}) as f64)";
                _f64Alt = null;
                return $"(({curF}) {fsym} {F64(ca.Value)}) as f32";
            }
            var l = inC.IsIdentity ? cur : NumCast(cur, Under(tt), Under(opType));
            var r = BinExpr(ca, ca.OperatorKind, l, E(ca.Value, true), opType, opType, ca.Value.Type);
            return outC.IsIdentity ? r : NumCast(r, Under(opType), Under(tt));
        }
        if (!isString && ca.OperatorMethod == null && !(IsNum(Under(tt)) || Under(tt) == SpecialType.System_Boolean)) return Fail(ca, "compound assignment on " + (tt == null ? "?" : Model.Short(tt)));
        if (target is IPropertyReferenceOperation pr && !(NativeTemplate(pr.Property) is { IsPlace: true }))
        {
            if (pr.Instance != null && !Pure(pr.Instance) && pr.Arguments.Length == 0 && pr.Instance.Type != null && pr.Instance.Type.IsReferenceType)
            {
                // `writer.BaseStream.Position += 2`: C# evaluates the receiver once, then calls the getter and the
                // setter on that object
                var tmp = Tmp("r");
                var rr = R(tmp, pr.Instance.Type, pr.Instance);
                var pct = pr.Property.ContainingType;
                string get, setc;
                var natp = NativeFor(pr.Property, pr.Instance);
                if (natp != null)
                {
                    if (natp.Get == null || natp.Set == null) return Fail(ca, "compound assignment on a native property without a getter / setter template");
                    get = Subst(natp.Get, pr, NativeThis(rr, pct), new List<string>(), null, null, pct);
                    setc = Subst(natp.Set, pr, NativeThis(rr, pct), new List<string>(), Wb(pr.Type, Op(get)), null, pct);
                }
                else
                {
                    if (pr.Property.GetMethod == null || pr.Property.SetMethod == null) return Fail(ca, "compound assignment on a property without getter / setter");
                    get = CallMethod(pr.Property.GetMethod, rr, new List<string>(), pr, false, null);
                    setc = CallMethod(pr.Property.SetMethod, rr, new List<string> { Op(get) }, pr, false, null);
                }
                return asValue ? $"{{ let {tmp} = {E(pr.Instance)}; {setc}; {get} }}" : $"{{ let {tmp} = {E(pr.Instance)}; {setc}; }}";
            }
            if (!Pure(pr.Instance) || pr.Arguments.Any(x => !Pure(x.Value))) return Fail(ca, "compound assignment on a property with an impure receiver");
            var set = PropSet(pr, Wb(pr.Type, Op(PropGet(pr))));
            if (!asValue) return set + ";";
            return $"{{ {set}; {PropGet(pr)} }}";
        }
        if (target is ILocalReferenceOperation cwt && IsWide(cwt.Local) && ca.OperatorMethod == null && FloatArith(ca.OperatorKind) && inC.IsIdentity && outC.IsIdentity && IsF32(ca.Value))
        {
            var wn = LocalName(cwt.Local);
            string sym = ca.OperatorKind switch { BinaryOperatorKind.Add => "+", BinaryOperatorKind.Subtract => "-", BinaryOperatorKind.Multiply => "*", BinaryOperatorKind.Divide => "/", _ => "%" };
            // C05 for `a += F(z)` (IL: ldloc a, the call, add): a is loaded before the call and spilled to a 4-byte
            // slot across it (ApplyWallLight: `num4 += (float)localRandom.Next(0, 50) * 0.0005f`, `fstp dword
            // [ebp-0x1c]` before the call in the lighting reference run's code). Only where a reference run's code
            // proves the call instruction (config `jitRefCalls`): elsewhere the size rule's guess decides nothing here
            var cur = _fa != null && _fa.ContainsCall(ca.Value, provenOnly: true) ? $"({wn} as f32 as f64)" : wn;
            var upd = $"{wn} = ({cur} {sym} {F64(ca.Value)});";
            return asValue ? $"{{ {upd} ({wn} as f32) }}" : upd;
        }
        if (target is ILocalReferenceOperation cwt2 && IsWide(cwt2.Local)) return Fail(ca, "compound assignment on an x87 local with a conversion");
        if (X87 && ca.OperatorMethod == null && tt?.SpecialType == SpecialType.System_Single && FloatArith(ca.OperatorKind) && inC.IsIdentity && outC.IsIdentity && IsF32(ca.Value) && Pure(target) && DirectOk(target, ca.Value))
        {
            // `x op= tree`: one x87 expression `x op tree`, rounded by the store
            var pl = P(target);
            string sym = ca.OperatorKind switch { BinaryOperatorKind.Add => "+", BinaryOperatorKind.Subtract => "-", BinaryOperatorKind.Multiply => "*", BinaryOperatorKind.Divide => "/", _ => "%" };
            var upd = $"{pl} = ((({pl} as f64) {sym} {F64(ca.Value)}) as f32);";
            return asValue ? $"{{ {upd} {pl} }}" : upd;
        }
        if (Pure(target) && !asValue && DirectOk(target, ca.Value))
        {
            var pl = P(target);
            return $"{pl} = {WbTo(target, Op(pl))};";
        }
        var p = Tmp("p");
        var addr = Addr(target);
        var body = $"let {p} = {addr}; *{p} = {WbTo(target, Op("(*" + p + ")"))};";
        return asValue ? $"{{ {body} *{p} }}" : $"{{ {body} }}";
    }

    string IncDec(IIncrementOrDecrementOperation inc, bool asValue)
    {
        if (inc.OperatorMethod != null) return Fail(inc, "user-defined ++ / --");
        if (inc.IsLifted) return Fail(inc, "lifted (nullable) ++ / --");
        var t = inc.Target.Type;
        if (t is IPointerTypeSymbol)
        {
            if (!Pure(inc.Target)) return Fail(inc, "pointer ++ / -- on an impure place");
            var ppl = P(inc.Target);
            var pupd = $"{ppl} = {ppl}.{(inc.Kind == OperationKind.Increment ? "wrapping_add" : "wrapping_sub")}(1);";
            if (!asValue) return pupd;
            return inc.IsPostfix ? $"{{ let __o = {ppl}; {pupd} __o }}" : $"{{ {pupd} {ppl} }}";
        }
        var st = Under(t);
        if (!IsNum(st)) return Fail(inc, "++ / -- on " + (t == null ? "?" : Model.Short(t)));
        if (inc.Target is IFieldReferenceOperation hfi && IsHostField(hfi.Field)) return Fail(inc, "++ / -- on a library field with no native mapping");
        bool up = inc.Kind == OperationKind.Increment;
        string Step(string cur) => IsFloat(st) ? $"({cur} {(up ? "+" : "-")} 1.0)" : $"{cur}.{(up ? "wrapping_add" : "wrapping_sub")}(1)";
        if (inc.Target is IPropertyReferenceOperation pr && !(NativeTemplate(pr.Property) is { IsPlace: true }))
        {
            if (pr.Instance != null && !Pure(pr.Instance) && pr.Arguments.Length == 0 && pr.Instance.Type != null && pr.Instance.Type.IsReferenceType)
            {
                // `((CustomIntCondition)x.GetCondition(..)).Value++`: the receiver is evaluated once, then its getter
                // and its setter are called (as the compound assignment above)
                var tmp = Tmp("r");
                var rr = R(tmp, pr.Instance.Type, pr.Instance);
                var pct = pr.Property.ContainingType;
                Func<string> get; Func<string, string> setc;
                var natp = NativeFor(pr.Property, pr.Instance);
                if (natp != null)
                {
                    if (natp.Get == null || natp.Set == null) return Fail(inc, "++ / -- on a native property without a getter / setter template");
                    get = () => Subst(natp.Get, pr, NativeThis(rr, pct), new List<string>(), null, null, pct);
                    setc = v => Subst(natp.Set, pr, NativeThis(rr, pct), new List<string>(), v, null, pct);
                }
                else
                {
                    if (pr.Property.GetMethod == null || pr.Property.SetMethod == null) return Fail(inc, "++ / -- on a property without getter / setter");
                    get = () => CallMethod(pr.Property.GetMethod, rr, new List<string>(), pr, false, null);
                    setc = v => CallMethod(pr.Property.SetMethod, rr, new List<string> { v }, pr, false, null);
                }
                var ro = Tmp("o");
                var head = $"let {tmp} = {E(pr.Instance)}; let {ro} = {get()};";
                if (!asValue) return $"{{ {head} {setc(Step(ro))}; }}";
                if (inc.IsPostfix) return $"{{ {head} {setc(Step(ro))}; {ro} }}";
                var rn = Tmp("n");
                return $"{{ {head} let {rn} = {Step(ro)}; {setc(rn)}; {rn} }}";
            }
            if (!Pure(pr.Instance) || pr.Arguments.Any(x => !Pure(x.Value))) return Fail(inc, "++ / -- on a property with an impure receiver");
            if (!asValue) return PropSet(pr, Step(PropGet(pr))) + ";";
            var o = Tmp("o");
            return inc.IsPostfix ? $"{{ let {o} = {PropGet(pr)}; {PropSet(pr, Step(o))}; {o} }}" : $"{{ let {o} = {Step(PropGet(pr))}; {PropSet(pr, o)}; {o} }}";
        }
        if (inc.Target is ILocalReferenceOperation iwt && IsWide(iwt.Local))
        {
            var wn = LocalName(iwt.Local);
            var upd = $"{wn} = ({wn} {(up ? "+" : "-")} 1.0f64);";
            if (!asValue) return upd;
            return inc.IsPostfix ? $"{{ let __o = ({wn} as f32); {upd} __o }}" : $"{{ {upd} ({wn} as f32) }}";
        }
        if (!asValue && Pure(inc.Target))
        {
            var pl = P(inc.Target);
            return $"{pl} = {Step(pl)};";
        }
        var p = Tmp("p");
        var addr = Addr(inc.Target);
        if (!asValue) return $"{{ let {p} = {addr}; *{p} = {Step("(*" + p + ")")}; }}";
        if (inc.IsPostfix) { var o = Tmp("o"); return $"{{ let {p} = {addr}; let {o} = *{p}; *{p} = {Step(o)}; {o} }}"; }
        return $"{{ let {p} = {addr}; *{p} = {Step("(*" + p + ")")}; *{p} }}";
    }

    // ---- members
    string PropGet(IPropertyReferenceOperation p)
    {
        var prop = p.Property;
        var ct = prop.ContainingType;
        if (p.Instance != null && p.Instance.Type is IArrayTypeSymbol)
        {
            if (prop.Name == "Length")
            {
                if (p.Instance is IFieldReferenceOperation af && M.Cfg.NativeSlices.TryGetValue(M.DocId(af.Field), out var slice)) return $"rt::slice_len(&{slice})";
                return $"{E(p.Instance)}.len()";
            }
        }
        if (ct.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T)
        {
            if (prop.Name == "HasValue") return $"{StructPlace(p.Instance)}.has_value";
            if (prop.Name == "Value") return $"{StructPlace(p.Instance)}.value()";
        }
        // `Message` of a library exception type (ArgumentException ... override it): the runtime keeps one message
        if (prop.Name == "Message" && p.Instance != null && M.Get(ct) == null && M.IsBclException(ct)) return $"{NativeThis(p.Instance)}.message";
        if (ct.IsAnonymousType && p.Instance != null)
        {
            // a property of an anonymous object: the boxed value by position
            int ak = ct.GetMembers().OfType<IPropertySymbol>().ToList().FindIndex(x => SymbolEqualityComparer.Default.Equals(x.OriginalDefinition, prop.OriginalDefinition));
            var ub = ak < 0 ? null : UnboxOf(prop.Type, $"rt::anon_get({E(p.Instance)}, {ak})");
            return ub ?? Fail(p, "property of an anonymous type with no unboxing rule (" + Model.Short(prop.Type) + ")");
        }
        var nat = NativeFor(prop, p.Instance);
        if (nat != null)
        {
            if (nat.Get == null) return Fail(p, "native property without a getter template");
            return Subst(nat.Get, p, NativeThis(p.Instance, ct), p.Arguments.Select(x => E(x.Value)).ToList(), null, null, ct);
        }
        if (prop.GetMethod == null) return Fail(p, "property without getter");
        return CallMethod(prop.GetMethod, p.Instance, ArgList(p.Arguments, prop.GetMethod, p), p, IsBase(p.Instance), null);
    }

    string PropSet(IPropertyReferenceOperation p, string value)
    {
        var prop = p.Property;
        var nat = NativeFor(prop, p.Instance);
        if (nat != null)
        {
            if (nat.Set == null) return Fail(p, "native property without a setter template");
            return Subst(nat.Set, p, NativeThis(p.Instance, prop.ContainingType), p.Arguments.Select(x => E(x.Value)).ToList(), value, null, prop.ContainingType);
        }
        if (prop.SetMethod == null)
        {
            // get-only auto-property assigned in a constructor: write the backing field
            var tt = M.Get(prop.ContainingType);
            var bf = prop.ContainingType.GetMembers().OfType<IFieldSymbol>().FirstOrDefault(f => SymbolEqualityComparer.Default.Equals(f.AssociatedSymbol, prop));
            var fi = tt == null || bf == null ? null : M.Field(tt, bf);
            if (fi == null) return Fail(p, "assignment to a property without setter");
            if (prop.IsStatic) return $"(*{tt.ModPath}::s()).{fi.Name} = {value}";
            return tt.IsStruct ? $"{StructPlace(p.Instance)}.{fi.Name} = {value}" : $"(*{ClassPtr(p.Instance, tt)}).{fi.Name} = {value}";
        }
        var args = ArgList(p.Arguments, prop.SetMethod, p);
        args.Add(value);
        return CallMethod(prop.SetMethod, p.Instance, args, p, IsBase(p.Instance), null);
    }

    static bool IsBase(IOperation inst) => inst is IInstanceReferenceOperation && inst.Syntax is BaseExpressionSyntax;

    string _argPre;
    string[] _rawArgs;
    HashSet<int> _rawOrdinals;
    string WrapArgs(string pre, string call) => pre == null ? call : $"{{ {pre}{call} }}";

    List<string> ArgList(ImmutableArray<IArgumentOperation> args, IMethodSymbol m, IOperation at, bool allowTemps = false)
    {
        // arguments arrive in evaluation (source) order; emit in parameter order
        var res = new string[args.Length];
        bool inOrder = true; int last = -1;
        foreach (var a in args) { int o = a.Parameter?.Ordinal ?? -1; if (o < last) inOrder = false; last = o; }
        // named arguments written out of order: evaluate into temporaries in source order
        bool temps = !inOrder && args.Count(a => !Pure(a.Value)) > 1;
        // (`new T(b: .., a: ..)`: New() wraps the creation in the block that holds the temporaries)
        if (temps && !allowTemps && ReferenceEquals(at, _ctorTempsFor)) allowTemps = true;
        if (temps && !allowTemps) { Fail(at, "named arguments out of order with side effects (in an indexer / property call)"); temps = false; }
        var pre = new StringBuilder();
        var def = m.OriginalDefinition;
        // a callee that is not translated code (runtime, host) may store an argument in an object: store barrier
        bool nativeCallee = m.MethodKind != MethodKind.DelegateInvoke && M.Get((m.ReducedFrom ?? m).ContainingType) == null;
        foreach (var a in args)
        {
            var par = a.Parameter;
            if (par == null) { Fail(a, "argument without parameter (__arglist)"); continue; }
            string s;
            if (par.RefKind is RefKind.Ref or RefKind.Out)
            {
                s = Addr(a.Value);
                // `out T` where T is erased to its class constraint: same representation, different handle type
                var pot = par.OriginalDefinition.Type;
                if (pot is ITypeParameterSymbol && Model.IsLibrary(def.ContainingType) && a.Value.Type != null && M.RT(pot) is { } prt && IsGc(prt) && M.RT(a.Value.Type) is { } art && art != prt && IsGc(art))
                    s = $"({s} as *mut {prt})";
            }
            else if (_rawOrdinals != null && _rawOrdinals.Contains(par.Ordinal))
            {
                // only `{rawN}` is used: the converted form is never written
                var rv = a.Value;
                while (rv is IConversionOperation { IsImplicit: true, OperatorMethod: null } rcv && rcv.Operand.Type != null && rcv.Operand.Type.IsReferenceType && rcv.GetConversion().IsReference) rv = rcv.Operand;
                s = E(rv);
            }
            else
            {
                s = E(a.Value);
                // generic method whose type parameter is erased to its class constraint
                var op = par.OriginalDefinition.Type;
                if (op is ITypeParameterSymbol && Model.IsLibrary(def.ContainingType) && a.Value.Type != null && !(Mono.Sub(a.Value.Type) is ITypeParameterSymbol))
                    s = Coerce(s, M.RT(a.Value.Type), M.RT(op));
                if (nativeCallee) s = Wb(par.Type, s);
            }
            if (temps)
            {
                var t = Tmp("a");
                pre.Append($"let {t} = {s}; ");
                s = t;
            }
            if (par.Ordinal < res.Length) res[par.Ordinal] = s;
        }
        if (ReferenceEquals(at, _ctorTempsFor)) { if (temps) _ctorPre = pre.ToString(); _argPre = null; }
        else _argPre = temps ? pre.ToString() : null;
        return res.Select(x => x ?? "rt::bad()").ToList();
    }

    /// The topmost translated declaration of virtual method `mi` of `tt`: its module has the dispatcher.
    (TType, MethodInfo2) LocalRoot(TType tt, MethodInfo2 mi)
    {
        var open = mi.Open ?? mi.Sym;
        while (open.OverriddenMethod != null)
        {
            var o = open.OverriddenMethod;
            var ut = M.Get(o.ContainingType);
            var umi = ut == null ? null : M.MethodInst(ut, o, mi.TypeArgs);
            if (umi == null || !umi.Emittable) break;
            open = o; tt = ut; mi = umi;
        }
        return (tt, mi);
    }

    string Invoke(IInvocationOperation inv)
    {
        // a call instruction (proven by the reference run's code) evaluated only on some paths of its statement: the
        // x87 values live across it are rounded there, on those paths only (ApplyHellLight: num4 at Tile.slope())
        if (_spillInline != null && _spillInline.TryGetValue(inv.Syntax, out var sv))
        {
            var pre = string.Concat(sv.Where(v => _wide.Contains(v) && DeclaredBefore(v, inv.Syntax)).Select(v => $"{LocalName(v)} = {LocalName(v)} as f32 as f64; "));
            if (pre.Length > 0) return "{ " + pre + "/* x87 stack variables live across the call: spilled as floats */ " + Invoke0(inv) + " }";
        }
        return Invoke0(inv);
    }

    string Invoke0(IInvocationOperation inv)
    {
        var m = inv.TargetMethod;
        if (m.MethodKind == MethodKind.DelegateInvoke)
        {
            var dr = M.RT(inv.Instance.Type);
            if (dr == null || !dr.StartsWith("Delegate<")) return Fail(inv, "invoke of a delegate with no Rust representation");
            string d = inv.Instance is IConditionalAccessInstanceOperation && _condAccess.Count > 0 ? _condAccess.Peek() : E(inv.Instance);
            // the delegate, then the arguments, are evaluated once; every target of a multicast delegate gets the same values
            var dargs = ArgList(inv.Arguments, m, inv);
            var dn = Tmp("d");
            var bind = new StringBuilder($"let {dn} = {d}; ");
            var names = new List<string>();
            foreach (var a in dargs) { var an = Tmp("a"); bind.Append($"let {an} = {a}; "); names.Add(an); }
            return $"{{ {bind}{dn}.invoke(|__f| __f({string.Join(", ", names)})) }}";
        }
        if (m.MethodKind == MethodKind.LocalFunction) return Fail(inv, "local function call");
        if (Resumable)
        {
            // DESIGN.md section 8f: the suspension points of a resumable form
            if (m.Name == "Sleep" && m.IsStatic && m.Parameters.Length == 1 && m.Parameters[0].Type.SpecialType == SpecialType.System_Int32 && m.ContainingType.ToDisplayString() == "System.Threading.Thread")
            {
                var ms = ArgList(inv.Arguments, m, inv);
                return $"{M.Cfg.Resumable.Sleep}({ms[0]}).await";
            }
            if (M.IsResumable(m))
            {
                var rt0 = M.Get(m.ContainingType);
                var rmi = rt0 == null ? null : M.Method(rt0, m.OriginalDefinition);
                if (rmi == null || !rmi.Emittable) return Fail(inv, "call of a resumable method that is not emitted");
                if (!m.IsStatic) return Fail(inv, "call of a resumable instance method inside a resumable form (only thread bodies may be instance methods)");
                return $"{rt0.ModPath}::{rmi.Name}__resumable({string.Join(", ", ArgList(inv.Arguments, m, inv))}).await";
            }
        }
        // `list.Sort()` of a translated class that implements IComparable<T>: Comparer<T>.Default (null sorts first,
        // otherwise a.CompareTo(b)). Primitives and structs go through rt::sys::CsOrd (the native template).
        if (m.Name == "Sort" && m.Parameters.Length == 0 && inv.Instance != null && m.ContainingType.OriginalDefinition.ToDisplayString() == "System.Collections.Generic.List<T>"
            && Mono.Sub(m.ContainingType.TypeArguments[0]) is INamedTypeSymbol { IsReferenceType: true, TypeKind: TypeKind.Class } cet && cet.SpecialType != SpecialType.System_String && M.Get(cet) != null)
        {
            var ci = cet.AllInterfaces.FirstOrDefault(i => i.OriginalDefinition.ToDisplayString() == "System.IComparable<T>" && SymbolEqualityComparer.Default.Equals(i.TypeArguments[0], cet));
            var cim = ci?.GetMembers("CompareTo").OfType<IMethodSymbol>().FirstOrDefault();
            if (cim == null || cet.FindImplementationForInterfaceMember(cim) is not IMethodSymbol cimpl) return Fail(inv, "List<T>.Sort() of a class without IComparable<T>");
            var cer = RTf(inv, cet);
            var ccall = CallMethod(cimpl, R("__a", cet, inv, true), new List<string> { "__b" }, inv, false, null);
            return $"rt::sys::list_sort_with({E(inv.Instance)}, |__a: {cer}, __b: {cer}| -> i32 {{ if __a.is_null() {{ if __b.is_null() {{ 0 }} else {{ -1 }} }} else if __b.is_null() {{ 1 }} else {{ unsafe {{ {ccall} }} }} }})";
        }
        if (m.Name is "OfType" or "Cast" && m.TypeArguments.Length == 1 && inv.Arguments.Length == 1 && m.ContainingType.ToDisplayString() == "System.Linq.Enumerable" && OfType(inv, m) is { } oft) return oft;
        if (m.Name == "ToString" && m.Parameters.Length == 0 && inv.Instance?.Type is INamedTypeSymbol { TypeKind: TypeKind.Enum } ent) return EnumToString(ent, E(inv.Instance));
        // Nullable<T>
        if (m.ContainingType.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T && inv.Instance != null)
        {
            if (m.Name == "GetValueOrDefault") return m.Parameters.Length == 0 ? $"{StructPlace(inv.Instance)}.value_or_default()" : $"{StructPlace(inv.Instance)}.value_or({E(inv.Arguments[0].Value)})";
        }
        // members of System.Array on a typed array (Roslyn may show the receiver converted to Array)
        var arrInst = inv.Instance;
        while (arrInst is IConversionOperation { IsImplicit: true, OperatorMethod: null } aic && aic.Operand.Type is IArrayTypeSymbol) arrInst = aic.Operand;
        if (arrInst != null && arrInst.Type is IArrayTypeSymbol at2)
        {
            if (m.Name == "GetLength" && inv.Arguments.Length == 1) return $"{E(arrInst)}.dim({E(inv.Arguments[0].Value)})";
            // CLR arrays made by `new T[..]` start at 0 in every dimension
            if (m.Name == "GetUpperBound" && inv.Arguments.Length == 1) return $"({E(arrInst)}.dim({E(inv.Arguments[0].Value)}) - 1)";
            if (m.Name == "GetLowerBound" && inv.Arguments.Length == 1) return $"{{ let _ = {E(arrInst)}.dim({E(inv.Arguments[0].Value)}); 0i32 }}";
            if (m.Name == "Clone" && at2.Rank <= 2 && m.Parameters.Length == 0) return $"{E(arrInst)}.clone_arr().up::<Object>()";
        }
        // arguments a native template takes raw (`{rawN}` and no `{N}`): rendered without the implicit reference
        // conversion to the parameter type, which may have no rule (`token.Value<T>()`: JToken -> IEnumerable<JToken>)
        _rawOrdinals = null;
        if (NativeTemplate((m.ReducedFrom ?? m).OriginalDefinition) is { Get: { } tg0 } && tg0.Contains("{raw"))
        {
            _rawOrdinals = new HashSet<int>();
            for (int i = 0; i < m.Parameters.Length; i++)
                if (tg0.Contains("{raw" + i + "}") && !tg0.Contains("{" + i + "}") && !tg0.Contains("{args}")) _rawOrdinals.Add(i);
        }
        var argl = ArgList(inv.Arguments, m, inv, true);
        _rawOrdinals = null;
        var argPre = _argPre; _argPre = null;
        // `{rawN}` in a native template: argument N without the implicit reference conversion to the parameter type
        // (Array.Clear(Array, ..) receives the typed array, so the runtime function can be generic over the element)
        _rawArgs = null;
        if (NativeTemplate((m.ReducedFrom ?? m).OriginalDefinition) is { Get: { } tg } && tg.Contains("{raw"))
        {
            var raw = new string[m.Parameters.Length];
            foreach (var a in inv.Arguments)
            {
                if (a.Parameter == null || a.Parameter.Ordinal >= raw.Length) continue;
                var v = a.Value;
                while (v is IConversionOperation { IsImplicit: true, OperatorMethod: null } cv && cv.Operand.Type != null && cv.Operand.Type.IsReferenceType && cv.GetConversion().IsReference) v = cv.Operand;
                raw[a.Parameter.Ordinal] = a.Parameter.Type.ToDisplayString() == "System.Array" && v.Type is not IArrayTypeSymbol ? null : E(v);
            }
            _rawArgs = raw;
        }
        if (argPre != null && inv.Instance != null && !Pure(inv.Instance)) return Fail(inv, "named arguments out of order on a call with an impure receiver");
        return WrapArgs(argPre, CallMethod(m, inv.Instance, argl, inv, IsBase(inv.Instance), null));
    }

    /// `source.OfType<R>()` / `source.Cast<R>()` over a collection whose element type is known statically: the
    /// type test and the cast are the translated ones (`is R`, `(R)x`), applied lazily when the result is enumerated.
    string OfType(IInvocationOperation inv, IMethodSymbol m)
    {
        var src = inv.Arguments[0].Value;
        while (src is IConversionOperation { IsImplicit: true, OperatorMethod: null } cv && cv.Operand.Type != null && cv.GetConversion().IsReference) src = cv.Operand;
        var st = Mono.Sub(src.Type);
        ITypeSymbol elem = st is IArrayTypeSymbol at0 && at0.Rank == 1 ? at0.ElementType : null;
        if (elem == null && st is INamedTypeSymbol sn)
            foreach (var i in sn.AllInterfaces.Concat(new[] { sn }))
                if (i.OriginalDefinition.SpecialType == SpecialType.System_Collections_Generic_IEnumerable_T) { elem = i.TypeArguments[0]; break; }
        if (elem == null) return null;
        var target = Mono.Sub(m.TypeArguments[0]);
        var er = M.RT(elem); var tr = M.RT(target);
        if (er == null || tr == null) return null;
        var seqType = M.Comp.GetSpecialType(SpecialType.System_Collections_Generic_IEnumerable_T).Construct(elem);
        var seq = ConvRef(inv, E(src), st, seqType);
        string test, cast;
        // no test only when every element is a `target` statically (identity or an implicit reference conversion):
        // two different interfaces (or object and an interface) share the Rust type `Obj` but not the test
        var conv = M.Comp.ClassifyConversion(elem, target);
        bool always = SymbolEqualityComparer.Default.Equals(elem, target) || (conv.IsImplicit && (conv.IsIdentity || conv.IsReference));
        if (er == tr && always) { test = "true"; cast = "__x"; }
        else if (IsGc(er) && (IsGc(tr) || target.IsValueType || target.SpecialType == SpecialType.System_String))
        {
            test = IsType("__x", elem, target);
            if (test == null) return null;
            var o = er == "Obj" ? "__x" : "__x.up::<Object>()";
            cast = IsGc(tr) ? (tr == "Obj" ? o : $"__x.up::<{Inner(tr)}>()") : Unbox(target, o, inv, false);
        }
        else return null;
        bool isCast = m.Name == "Cast";
        return $"rt::sys::of_type::<{er}, {tr}>({seq}, {(isCast ? "true" : "false")}, |__x: {er}| unsafe {{ {test} }}, |__x: {er}| unsafe {{ {cast} }})";
    }

    /// Call of `m` with rendered arguments in parameter order.
    string CallMethod(IMethodSymbol m, IOperation inst, IList<string> args, IOperation at, bool baseCall, ITypeSymbol[] operandTypes)
    {
        var cm = m.ReducedFrom ?? m;
        var def = cm.OriginalDefinition;
        var retT = m.ReturnsVoid ? "()" : M.RT(m.ReturnType);
        // constrained call (`where T : IComparable<T>`): Roslyn may show the receiver boxed to the interface
        if (inst is IConversionOperation { IsImplicit: true, OperatorMethod: null } icv && icv.Operand.Type is ITypeParameterSymbol
            && def.ContainingType.TypeKind == TypeKind.Interface)
            inst = icv.Operand;
        // a call through a constraint on a type parameter (`where T : IFoo`, `t.Foo()`), instantiated: a struct
        // is called directly on its place (the CLR's constrained call does not box), a class through the interface
        if (inst != null && inst.Type is ITypeParameterSymbol && Mono.Sub(inst.Type) is INamedTypeSymbol crt && !def.IsStatic
            && Mono.Sub(cm.ContainingType) is INamedTypeSymbol { TypeKind: TypeKind.Interface } cit && (crt.IsValueType || M.Get(cit) == null))
        {
            IMethodSymbol im = null;
            foreach (var x in cit.GetMembers(def.Name)) if (x is IMethodSymbol xm && SymbolEqualityComparer.Default.Equals(xm.OriginalDefinition, def)) { im = xm; break; }
            // the implementation in the instantiating type, when it is translated or natively mapped; else the
            // interface member's own mapping (`IComparable<T>.CompareTo` -> `rt::Trace::cs_compare`); else the host
            if (im != null && crt.FindImplementationForInterfaceMember(im) is IMethodSymbol impl && !SymbolEqualityComparer.Default.Equals(impl.OriginalDefinition, def)
                && (M.Get(impl.ContainingType) != null || NativeTemplate(impl) != null || NativeTemplate(def) == null))
            {
                if (cm.IsGenericMethod && impl.IsGenericMethod) impl = impl.Construct(cm.TypeArguments.Select(Mono.Sub).ToArray());
                return CallMethod(impl, inst, args, at, false, operandTypes);
            }
        }
        var nat = NativeFor(def, inst);
        if (nat != null)
        {
            var thisStr = inst == null ? null : NativeThis(inst, cm.ContainingType);
            var tmpl = nat.Get;
            // float mode "jit32": where the dumped run called this library member (a call instruction at every IL site
            // of this caller, jit_bodies.tsv), its body is evaluated as its own machine code does (config `jitBodies`)
            if (M.Cfg.FloatMode == "jit32" && M.Cfg.JitBodies.Count > 0 && Method != null && M.Cfg.JitBodies.TryGetValue(M.DocId(nat.Sym), out var xb)
                && nat.Sym is IMethodSymbol nms && (M.JitBodyCalled(Model.ClrMethodKey(Method.OriginalDefinition), Model.ClrMethodKey(nms))
                    // where the JIT inlined it, its body is the same x87 code inside the caller: the same for a body whose
                    // result is unrounded anyway ("f64:") or is stored per component (a vector, or `this`); not for a float
                    // the call rounds before returning (Length, Distance). NPC.AI_111: one of the two `Vector2 / float` is
                    // inlined, the reciprocal stays unrounded there too (refdata/entities npc type 578)
                    || ((xb.StartsWith("f64:", StringComparison.Ordinal) || nms.ReturnsVoid || nms.ReturnType.TypeKind == TypeKind.Struct)
                        && M.JitBodySeen(Model.ClrMethodKey(Method.OriginalDefinition), Model.ClrMethodKey(nms)))))
            {
                tmpl = xb;
                if (!Resumable && !TwinF64) System.Threading.Interlocked.Increment(ref M.JitBodiesUsed);
                // inlined here (no call instruction at any site): a float argument that is a local, a field or a
                // parameter enters the body as it is, unrounded (VecScalar; WorldItem.
                // TryCombiningIntoNearbyItems, `Vector2.Lerp(worldItem.velocity, velocity, amount)` with
                // `amount = (float)num2 / (float)stack` never stored: `fdivp`, then `fmul st(4)` in both inlined Lerps,
                // refdata/entities proj 797, the world item it drops, velocity.X at tick 71). Only for a placeholder the
                // body reads as `({i}) as f64`.
                if (at is IInvocationOperation xinv && M.JitBodyCalls(Model.ClrMethodKey(Method.OriginalDefinition), Model.ClrMethodKey(nms)) == 0)
                {
                    var xargs = args.ToList();
                    foreach (var xa in xinv.Arguments)
                    {
                        if (xa.Parameter == null || xa.Parameter.Type.SpecialType != SpecialType.System_Single || xa.ArgumentKind != ArgumentKind.Explicit) continue;
                        int pi = xa.Parameter.Ordinal;
                        string ph = "{" + pi + "}", phf = "(" + ph + ") as f64";
                        if (pi >= xargs.Count || !tmpl.Contains(phf) || tmpl.Replace(phf, "").Contains(ph)) continue;
                        var v = xa.Value;
                        while (v is IParenthesizedOperation vp) v = vp.Operand;
                        // (the argument was rendered already: only operands whose rendering has no side effect)
                        if (v is ILocalReferenceOperation or IFieldReferenceOperation { Instance: null or IInstanceReferenceOperation or ILocalReferenceOperation or IParameterReferenceOperation } or IParameterReferenceOperation)
                            xargs[pi] = F64(v);
                    }
                    args = xargs;
                }
                // a body whose float result stays in ST0 (`ret` without a store: MathHelper.Lerp, Vector2.Dot,
                // LengthSquared ...): "f64:" + an f64 expression. Here (a float value is expected) it is rounded; F64()
                // takes the unrounded expression instead (FLOAT_RULE.md section 2: the caller sees the unrounded
                // return value)
                if (tmpl.StartsWith("f64:", StringComparison.Ordinal))
                {
                    var body = Subst(tmpl.Substring(4), at, thisStr, args, null, m, def.IsStatic || def.MethodKind == MethodKind.Constructor || Mono.Sub(inst?.Type) is ITypeParameterSymbol or null ? (inst?.Type ?? m.ContainingType) : (M.Get(inst.Type) != null ? cm.ContainingType : inst.Type));
                    _f64Alt = ($"({body} as f32)", body);
                    return _f64Alt.Value.plain;
                }
            }
            return Subst(tmpl, at, thisStr, args, null, m, def.IsStatic || def.MethodKind == MethodKind.Constructor || Mono.Sub(inst?.Type) is ITypeParameterSymbol or null ? (inst?.Type ?? m.ContainingType) : (M.Get(inst.Type) != null ? cm.ContainingType : inst.Type));
        }
        string display = Model.ShortMember(def);
        // the virtuals of System.Object reached without a translated override in the static type: through the
        // object header (`rt::obj_to_string` ...), or the value-type rule for a value receiver
        if (!def.IsStatic && inst != null && def.MethodKind == MethodKind.Ordinary && def.ContainingType.SpecialType is SpecialType.System_Object or SpecialType.System_ValueType or SpecialType.System_Enum
            && ObjectVirtual(def, inst, args, at, baseCall) is { } ov) return ov;
        if (M.Stub(def) is { } stub)
        {
            if (retT == null) return Fail(at, "stub returning a type with no Rust representation");
            // the callee is replaced by a value from the config; receiver and arguments are still evaluated
            Stubs.Add(display);
            var eval = new StringBuilder();
            if (inst != null && !Pure(inst)) eval.Append($"let _ = {E(inst)}; ");
            if (args.Count > 0) eval.Append($"let _ = ({string.Join(", ", args)}{(args.Count == 1 ? "," : "")}); ");
            return eval.Length == 0 ? $"rt::stub::<{retT}>({Lit(display)}, {stub})" : $"{{ {eval}rt::stub::<{retT}>({Lit(display)}, {stub}) }}";
        }
        // P/Invoke (`static extern` with [DllImport]): a member of the host boundary like any library member,
        // listed in HOST_API.md under the declaring class and mappable through nativeMembers
        if (def.IsExtern && def.IsStatic && def.MethodKind == MethodKind.Ordinary && def.GetDllImportData() != null)
            return HostCall(m, "call", null, args, at);
        var tt = M.Get(cm.ContainingType);
        var mi = tt == null ? null : M.Method(tt, cm);
        if (def.MethodKind == MethodKind.Constructor && inst is IInstanceReferenceOperation)
        {
            // `: base(...)` / `: this(...)`
            if (def.ContainingType.SpecialType == SpecialType.System_Object) return "()";
            if (tt != null && tt.IsStruct && def.IsImplicitlyDeclared) return "*this = rt::zero()";
            if (tt == null && M.IsBclException(def.ContainingType) && Owner.ExcKind != null)
            {
                // `: base(message)` of a game exception class: the runtime exception object is the first field
                string msg = "Str::NULL";
                for (int i = 0; i < def.Parameters.Length && i < args.Count; i++) if (def.Parameters[i].Type.SpecialType == SpecialType.System_String) { msg = args[i]; break; }
                return $"rt::exc_init(this as *mut rt::Exception, {Lit(Owner.ExcKind)}, {msg})";
            }
            if (tt == null && Owner.NativeBase != null && SymbolEqualityComparer.Default.Equals(Mono.Sub(cm.ContainingType), Owner.NativeBase)) return NativeBaseInit(cm, args, at);
            if (tt == null && Model.IsLibrary(def.ContainingType) && M.Cfg.Inert.ContainsKey(M.DocId(def))) return $"{M.Cfg.HostRoot}::inert({Lit(display)})";
            if (tt == null && Model.IsLibrary(def.ContainingType)) { Unported.Add(display + " (library base class)"); Gaps.Add(display + " (library base class)"); return $"rt::gap({Lit(display + " (library base class)")})"; }
            if (mi == null || !mi.Emittable) { Unported.Add(display); Gaps.Add(display); return $"rt::gap({Lit(display)})"; }
            Calls.Add($"{tt.ModPath}::{mi.Name}");
            var cargs = new List<string> { tt == Owner ? "this" : $"(this as *mut {tt.StructPath})" };
            cargs.AddRange(args);
            return $"{tt.ModPath}::{mi.Name}({string.Join(", ", cargs)})";
        }
        if (tt == null && Model.IsLibrary(def.ContainingType))
            return HostCall(m, def.MethodKind == MethodKind.Constructor ? "new" : "call", inst, args, at);
        if (mi == null || !mi.Emittable)
        {
            if (retT == "()") { Unported.Add(display); Gaps.Add(display); return $"rt::gap({Lit(display)})"; }
            if (retT == null) return Fail(at, "call returning a type with no Rust representation: " + KindOfType(m.ReturnType));
            return Unp(display, retT);
        }
        if (retT == null) return Fail(at, "call returning a type with no Rust representation");
        var a = new List<string>();
        string pre = null;
        string fn = mi.Name;
        var path = tt.ModPath;
        if (!def.IsStatic && def.MethodKind != MethodKind.Constructor)
        {
            if (inst == null) return Fail(at, "instance call without receiver");
            var target = tt; var tmi = mi;
            bool virt = (def.IsVirtual || def.IsAbstract || def.IsOverride) && !baseCall && !tt.IsStruct;
            if (tt.IsInterface)
            {
                // dispatch by the object's type id (Gen.EmitInterfaceDispatcher)
                if (Mono.Sub(inst.Type) is not { } irt || irt is ITypeParameterSymbol || irt.IsValueType) return Fail(at, "interface method call on a value / type parameter (constrained call)");
                fn = mi.Name + "__v";
                virt = false;
            }
            if (virt)
            {
                (target, tmi) = LocalRoot(tt, mi);
                fn = tmi.Name + "__v"; path = target.ModPath;
            }
            if (tt.IsStruct)
            {
                if (inst is IInstanceReferenceOperation or IConditionalAccessInstanceOperation || IsPlace(inst))
                {
                    var pl = StructPlace(inst);
                    a.Add(pl == "(*this)" ? "this" : $"core::ptr::addr_of_mut!({pl})");
                }
                else
                {
                    var t = Tmp("s");
                    pre = $"let mut {t} = {E(inst)}; ";
                    a.Add($"core::ptr::addr_of_mut!({t})");
                }
            }
            else
            {
                if (Mono.Sub(inst.Type) is ITypeParameterSymbol || !IsGc(M.RT(inst.Type) ?? "")) return Fail(at, "call of a class method on a receiver that is not a handle (" + (inst.Type == null ? "?" : Model.Short(Mono.Sub(inst.Type))) + ")");
                a.Add(ClassPtr(inst, target));
            }
        }
        a.AddRange(args);
        // a generic method is one Rust function per instantiation (Mono.cs): `fn` already names it
        Calls.Add($"{path}::{fn}");
        if (def.IsStatic || def.MethodKind == MethodKind.Constructor) Calls.Add($"{tt.ModPath}::cctor");
        var vtw = !fn.EndsWith("__v", StringComparison.Ordinal) && M.VecTwin(def) ? VecTwinSuffix(def, at) : null;
        if (vtw == null && !fn.EndsWith("__v", StringComparison.Ordinal) && InlinedArgsTwin(def, at, a, args.Count) is { } ua)
        {
            // the JIT inlined `def` here and a float local of this method on the x87 stack is one of its arguments:
            // the callee's `__u` twin takes it unrounded (FLOAT_RULE.md section 10, item 26)
            var ucall = $"{path}::{fn}__u({string.Join(", ", ua)})";
            string utwin = M.TwinF64(def) ? $"{path}::{fn}__u__x87({string.Join(", ", ua)})" : null;
            if (pre != null) { ucall = $"{{ {pre}{ucall} }}"; if (utwin != null) utwin = $"{{ {pre}{utwin} }}"; }
            if (utwin != null) _f64Alt = (ucall, utwin);
            return ucall;
        }
        var call = $"{path}::{fn}{vtw}({string.Join(", ", a)})";
        string twin = vtw == null && !fn.EndsWith("__v", StringComparison.Ordinal) && M.TwinF64(def) ? $"{path}::{fn}__x87({string.Join(", ", a)})" : null;
        if (pre != null) { call = $"{{ {pre}{call} }}"; if (twin != null) twin = $"{{ {pre}{twin} }}"; }
        if (twin != null) _f64Alt = (call, twin);
        return call;
    }

    string OwnerName => Model.FullName(Owner.Sym);

    /// `x.ToString()` / `x.Equals(o)` / `x.GetHashCode()` bound to System.Object / ValueType / Enum; null: not one of them.
    string ObjectVirtual(IMethodSymbol def, IOperation inst, IList<string> args, IOperation at, bool baseCall)
    {
        bool toString = def.Name == "ToString" && def.Parameters.Length == 0;
        bool equals = def.Name == "Equals" && def.Parameters.Length == 1 && def.Parameters[0].Type.SpecialType == SpecialType.System_Object;
        bool hash = def.Name == "GetHashCode" && def.Parameters.Length == 0;
        if (!toString && !equals && !hash) return null;
        var t = Mono.Sub(inst.Type);
        if (t == null) return null;
        if (baseCall)
        {
            // `base.X()` where the base is System.Object / ValueType: the default behaviour, not the override
            if (OwnerIsStruct)
            {
                if (toString) return $"Str::lit({Lit(Gen.ClrName(Owner.Sym))})";
                if (hash) return $"{Owner.ModPath}::__fieldwise_hash(&*this)";
                return $"{{ let __o = {args[0]}; rt::is_boxed(__o, {Owner.ModPath}::TYPE_ID) && {Owner.ModPath}::__fieldwise_eq(&*this, &*(__o.0 as *const {Owner.StructPath})) }}";
            }
            if (toString) return "Str::lit(types::NAMES[rt::type_id_of(this as *const u8) as usize])";
            if (hash) return "rt::identity_hash(this as usize)";
            return $"(this as usize == {args[0]}.addr())";
        }
        var r = M.RT(t);
        if (r == null) return Fail(at, "receiver type with no Rust representation: " + KindOfType(t));
        string e = inst is IConditionalAccessInstanceOperation && _condAccess.Count > 0 ? _condAccess.Peek() : E(inst, true);
        if (toString) return ToStringOf(t, e, at, false);
        bool handle = IsGc(r) || IsHandleLike(r);
        string asObj = r == "Obj" ? e : $"{e}.up::<Object>()";
        if (t.SpecialType == SpecialType.System_String) asObj = $"rt::str_obj({e}.not_null())";
        if (equals) return handle || t.SpecialType == SpecialType.System_String ? $"rt::obj_equals({asObj}, {args[0]})" : $"rt::obj_equals({Box(t, e, at)}, {args[0]})";
        return handle || t.SpecialType == SpecialType.System_String ? $"rt::obj_hash_code({asObj})" : $"rt::CsKey::cs_hash_code(&{e})";
    }

    /// Call of a library member that native.json does not map: a generated function of the host boundary (Host.cs).
    /// `constructed` carries the actual type arguments (of the method and of its containing type).
    string HostCall(ISymbol constructed, string kind, IOperation inst, IList<string> args, IOperation at)
    {
        var def = constructed is IMethodSymbol cm ? (cm.ReducedFrom ?? cm).OriginalDefinition : constructed.OriginalDefinition;
        var f = M.Host.Get(def, kind, out var reason);
        if (f == null) return Fail(at, reason);
        var a = new List<string>();
        string pre = null;
        bool isStatic = def.IsStatic || def is IMethodSymbol { MethodKind: MethodKind.Constructor };
        if (!isStatic)
        {
            if (inst == null) return Fail(at, "instance host call without receiver");
            var want = f.Params[0].type;
            if (def.ContainingType.IsValueType)
            {
                if (Mono.Sub(inst.Type) is ITypeParameterSymbol) return Fail(at, "host call on an unconstrained type parameter");
                if (inst is IInstanceReferenceOperation or IConditionalAccessInstanceOperation || IsPlace(inst))
                {
                    var pl = StructPlace(inst);
                    a.Add(pl == "(*this)" ? $"(this as {want})" : $"(core::ptr::addr_of_mut!({pl}) as {want})");
                }
                else
                {
                    var t = Tmp("s");
                    pre = $"let mut {t} = {E(inst)}; ";
                    a.Add($"(core::ptr::addr_of_mut!({t}) as {want})");
                }
            }
            else
            {
                var have = M.RT(inst.Type);
                if (have == null) return Fail(at, "receiver type with no Rust representation: " + KindOfType(inst.Type));
                string e = inst is IConditionalAccessInstanceOperation && _condAccess.Count > 0 ? _condAccess.Peek() : E(inst);
                // `want` is written with the type parameters of the definition; compare with the constructed type
                var wantC = M.RT(constructed.ContainingType) ?? want;
                if (have == want || have == wantC) a.Add(e);
                else if (IsGc(have) && IsGc(wantC)) a.Add($"{e}.up::<{Inner(wantC)}>()");
                else if (IsGc(wantC)) a.Add(Box(inst.Type, e, at));
                else if (wantC.StartsWith("Seq<") && Mono.Sub(inst.Type) is { } hrt && Mono.Sub(constructed.ContainingType) is { } hct) a.Add(ConvRef(at, e, hrt, hct));
                else return Fail(at, $"host receiver {have} where {wantC} is expected");
            }
        }
        if (args != null) a.AddRange(args);
        // type arguments, in the order Host.Get lists the generics
        var targs = new List<string>();
        var seen = new HashSet<string>();
        void AddT(ImmutableArray<ITypeParameterSymbol> tps, ImmutableArray<ITypeSymbol> actual)
        {
            for (int i = 0; i < tps.Length; i++)
            {
                var n = Names.Ident(tps[i].Name);
                if (M.RT(tps[i]) != n || !seen.Add(n)) continue;
                targs.Add(i < actual.Length ? RTf(at, actual[i]) : "()");
            }
        }
        var chain = new List<INamedTypeSymbol>();
        for (var c = constructed.ContainingType; c != null; c = c.ContainingType) chain.Insert(0, c);
        foreach (var c in chain) AddT(c.OriginalDefinition.TypeParameters, c.TypeArguments);
        if (constructed is IMethodSymbol gm && def is IMethodSymbol dm) AddT(dm.TypeParameters, gm.TypeArguments);
        if (targs.Count != f.Generics.Count) return Fail(at, "host call: type argument count mismatch");
        if (!isStatic && def.ContainingType.TypeKind == TypeKind.Interface && def is IMethodSymbol idm && LibIface(f, idm) is { } disp)
        {
            HostHits.Add(f);
            HostCalls.Add(f.Display);
            var dcall = $"ifaces::{disp}{(targs.Count > 0 ? "::<" + string.Join(", ", targs) + ">" : "")}({string.Join(", ", a)})";
            return pre != null ? $"{{ {pre}{dcall} }}" : dcall;
        }
        HostHits.Add(f);
        HostCalls.Add(f.Display);
        var call = $"{f.Path(M.Cfg.HostModule)}{(targs.Count > 0 ? "::<" + string.Join(", ", targs) + ">" : "")}({string.Join(", ", a)})";
        return pre != null ? $"{{ {pre}{call} }}" : call;
    }

    /// Dispatcher for a method of a library interface (IDisposable.Dispose, IComparable<T>.CompareTo ...) that
    /// translated types implement: by type id to the translated implementation, otherwise the host function.
    /// Returns the function name in `gen::ifaces`, or null when no translated type implements the interface.
    string LibIface(HostFn f, IMethodSymbol def)
    {
        var name = f.Module + "__" + f.Name;
        if (f.Params.Count == 0 || !IsGc(f.Params[0].type)) return null;
        var text = M.IfaceFns.GetOrAdd(f.Key, _ =>
        {
            var arms = new SortedDictionary<string, List<string>>(StringComparer.Ordinal);
            bool generic = f.Generics.Count > 0;
            foreach (var x in M.Ordered)
            {
                if (x.TypeId == 0 || x.Sym.IsAbstract || x.WholeTypeReason != null) continue;
                foreach (var iface in x.Sym.AllInterfaces)
                {
                    if (!SymbolEqualityComparer.Default.Equals(iface.OriginalDefinition, def.ContainingType)) continue;
                    var member = iface.GetMembers(def.Name).OfType<IMethodSymbol>().FirstOrDefault(m => SymbolEqualityComparer.Default.Equals(m.OriginalDefinition, def));
                    if (member == null || x.Sym.FindImplementationForInterfaceMember(member) is not IMethodSymbol impl0) continue;
                    var impl = Gen.MostDerived(x.Sym, impl0);
                    var it = M.Get(impl.ContainingType);
                    var imi = it == null ? null : M.Method(it, impl.OriginalDefinition);
                    if (imi == null || !imi.Emittable || impl.IsAbstract || impl.IsGenericMethod) continue;
                    // arguments and result are the same C# types on both sides once the interface's type arguments are
                    // fixed; the dispatcher is written over the type parameters, hence the size-checked retype
                    var args = new List<string> { $"this.0 as *mut {it.StructPath}" };
                    for (int i = 1; i < f.Params.Count; i++) args.Add(generic ? $"rt::retype({f.Params[i].name})" : f.Params[i].name);
                    var call = $"{it.ModPath}::{imi.Name}({string.Join(", ", args)})";
                    if (f.Ret != "()" && generic) call = $"rt::retype({call})";
                    if (!arms.TryGetValue(call, out var ids)) arms[call] = ids = new List<string>();
                    if (!ids.Contains($"{x.ModPath}::TYPE_ID")) ids.Add($"{x.ModPath}::TYPE_ID");
                }
            }
            if (arms.Count == 0) return "";
            f.Forced = true;
            var g = f.Generics.Count > 0 ? "<" + string.Join(", ", f.Generics.Select(x => x + ": rt::Val")) + ">" : "";
            var tf = f.Generics.Count > 0 ? "::<" + string.Join(", ", f.Generics) + ">" : "";
            var sbx = new StringBuilder($"/// `{f.Display}`: translated implementations by type id, any other object through the host boundary.\n");
            sbx.Append($"pub unsafe fn {name}{g}({string.Join(", ", f.Params.Select(p => p.name + ": " + p.type))}){(f.Ret == "()" ? "" : " -> " + f.Ret)} {{\n");
            sbx.Append("    if !this.is_null() {\n        match rt::type_id_of(this.0 as *const u8) {\n");
            foreach (var kv in arms) sbx.Append($"            {string.Join(" | ", kv.Value)} => return {kv.Key},\n");
            sbx.Append("            _ => {}\n        }\n    }\n");
            sbx.Append($"    {f.Path(M.Cfg.HostModule)}{tf}({string.Join(", ", f.Params.Select(p => p.name))})\n}}\n\n");
            return sbx.ToString();
        });
        return text == "" ? null : name;
    }

    static string LibGroup(IMethodSymbol def)
    {
        var ns = def.ContainingNamespace?.ToDisplayString() ?? "";
        return def.ContainingType.Name + "." + def.Name + " [" + ns + "]";
    }

    /// The object creation whose out-of-order named arguments may be evaluated into temporaries (see `New`).
    IOperation _ctorTempsFor;
    string _ctorPre;

    /// `new T(args) { initialisers }`. Named arguments written out of parameter order are evaluated into temporaries
    /// in source order (C#'s rule); the creation is then a block: `{ let __a1 = ..; let __a2 = ..; T::new(__a2, __a1) }`.
    string New(IObjectCreationOperation oc)
    {
        var (savedFor, savedPre) = (_ctorTempsFor, _ctorPre);
        _ctorTempsFor = oc; _ctorPre = null;
        var e = NewInner(oc);
        var pre = _ctorPre;
        (_ctorTempsFor, _ctorPre) = (savedFor, savedPre);
        return pre == null ? e : $"{{ {pre}{e} }}";
    }

    string NewInner(IObjectCreationOperation oc)
    {
        var t = Mono.Sub(oc.Type) as INamedTypeSymbol;
        var ctor = oc.Constructor;
        if (t == null || ctor == null) return Fail(oc, "object creation without constructor");
        string e;
        var r = M.RT(t);
        var def = ctor.OriginalDefinition;
        var nat = NativeTemplate(def);
        var tt = M.Get(t);
        if (r == null) return Fail(oc, "creation of a type with no Rust representation: " + KindOfType(t));
        if (nat != null) e = Subst(nat.Get, oc, null, ArgList(oc.Arguments, ctor, oc), null, ctor, t);
        else if (M.Stub(def) is { } stub)
        {
            Stubs.Add(Model.ShortMember(def));
            var cargs = ArgList(oc.Arguments, ctor, oc);
            var ev = cargs.Count == 0 ? "" : $"let _ = ({string.Join(", ", cargs)}{(cargs.Count == 1 ? "," : "")}); ";
            e = $"{{ {ev}rt::stub::<{r}>({Lit(Model.ShortMember(def))}, {stub}) }}";
        }
        else if (r == "Gc<Exception>")
        {
            string msg = "Str::NULL";
            foreach (var a in oc.Arguments) if (a.Parameter?.Type.SpecialType == SpecialType.System_String) { msg = E(a.Value); break; }
            e = $"rt::exc({Lit(Model.FullName(t))}, {msg})";
        }
        else if (t.SpecialType == SpecialType.System_Object) e = "rt::new_object()";
        else if (tt != null && tt.WholeTypeReason == null)
        {
            if (tt.IsStruct && ctor.IsImplicitlyDeclared && ctor.Parameters.Length == 0) e = $"rt::zero::<{tt.StructPath}>()";
            else
            {
                var mi = M.Method(tt, def);
                if (mi == null || !mi.Emittable) e = Unp(Model.ShortMember(def), r);
                else
                {
                    Calls.Add($"{tt.ModPath}::{mi.Name}"); Calls.Add($"{tt.ModPath}::cctor");
                    e = $"{tt.ModPath}::new{mi.Name.Substring(4)}({string.Join(", ", ArgList(oc.Arguments, ctor, oc))})";
                }
            }
        }
        else if (t.IsValueType && ctor.Parameters.Length == 0 && (M.Native(t) != null || Model.IsLibrary(t))) e = $"rt::zero::<{r}>()";
        else if (Model.IsLibrary(t)) e = HostCall(ctor, "new", null, ArgList(oc.Arguments, ctor, oc), oc);
        else if (r == "Obj" && t.TypeKind == TypeKind.Class && tt == null)
        {
            // a game class outside the translated set: its objects are opaque, every member is unported. The
            // constructor is recorded as a gap; the arguments are still evaluated.
            var nm = Model.ShortMember(def);
            Unported.Add(nm); Gaps.Add(nm);
            var cargs = ArgList(oc.Arguments, ctor, oc);
            var ev = cargs.Count == 0 ? "" : $"let _ = ({string.Join(", ", cargs)}{(cargs.Count == 1 ? "," : "")}); ";
            e = $"{{ {ev}rt::unported_new({Lit(nm)}) }}";
        }
        else e = Unp(Model.ShortMember(def), r);
        if (oc.Initializer == null) return e;
        return WithInitializer(e, r, t.IsValueType, oc.Initializer);
    }

    /// `new X(..) { A = 1, B = { .. } }` / a collection initialiser: the members are set on the fresh object.
    string WithInitializer(string e, string r, bool isStruct, IObjectOrCollectionInitializerOperation initializer)
    {
        var o = Tmp("o");
        var parts = new StringBuilder($"{{ let mut {o}: {r} = {e}; ");
        _implicit.Push((o, isStruct));
        try
        {
            foreach (var init in initializer.Initializers)
            {
                switch (init)
                {
                    case ISimpleAssignmentOperation sa when sa.Value is not IObjectOrCollectionInitializerOperation: parts.Append(Assign(sa, false)).Append(' '); break;
                    case IInvocationOperation iv: parts.Append(E(iv)).Append("; "); break;
                    case IMemberInitializerOperation mi: parts.Append(MemberInit(mi)); break;
                    default: parts.Append(Fail(init, "object initializer member: " + init.Kind)).Append("; "); break;
                }
            }
        }
        finally { _implicit.Pop(); }
        return parts.Append(o).Append(" }").ToString();
    }

    /// `new T()` in generic code, for the instantiation being emitted.
    string NewT(ITypeParameterObjectCreationOperation op)
    {
        if (Mono.Sub(op.Type) is not INamedTypeSymbol t) return Fail(op, "new T()");
        var r = M.RT(t);
        if (r == null) return Fail(op, "new T(): type with no Rust representation (" + Model.Short(t) + ")");
        if (op.Initializer != null)
        {
            var bare = NewT0(op, t, r);
            return WithInitializer(bare, r, t.IsValueType, op.Initializer);
        }
        return NewT0(op, t, r);
    }

    string NewT0(ITypeParameterObjectCreationOperation op, INamedTypeSymbol t, string r)
    {
        if (t.IsValueType) return $"rt::zero::<{r}>()";
        var tt = M.Get(t);
        var ctor = t.InstanceConstructors.FirstOrDefault(c => c.Parameters.Length == 0);
        if (tt != null && tt.WholeTypeReason == null && ctor != null && M.Method(tt, ctor) is { Emittable: true } mi)
        {
            Calls.Add($"{tt.ModPath}::{mi.Name}"); Calls.Add($"{tt.ModPath}::cctor");
            return $"{tt.ModPath}::new{mi.Name.Substring(4)}()";
        }
        if (ctor != null && tt == null && Model.IsLibrary(t)) return HostCall(ctor, "new", null, new List<string>(), op);
        return Fail(op, "new T(): no parameterless constructor of " + Model.Short(t));
    }

    /// `Member = { a, b }` / `Member = { X = 1 }` inside an object initialiser: the nested initialiser works on the
    /// object the member already holds (C# does not create one).
    string MemberInit(IMemberInitializerOperation mi)
    {
        var mt = mi.InitializedMember.Type;
        var mr = M.RT(mt);
        if (mr == null) return Fail(mi, "member initializer on a type with no Rust representation") + "; ";
        var sbm = new StringBuilder();
        string name; bool isStruct = mt.IsValueType;
        if (isStruct)
        {
            if (!IsPlace(mi.InitializedMember)) return Fail(mi, "member initializer on a struct-typed property") + "; ";
            name = P(mi.InitializedMember);
        }
        else
        {
            name = Tmp("m");
            sbm.Append($"let {name}: {mr} = {E(mi.InitializedMember)}; ");
        }
        _implicit.Push((name, isStruct));
        try
        {
            foreach (var init in mi.Initializer.Initializers)
            {
                switch (init)
                {
                    case ISimpleAssignmentOperation sa when sa.Value is not IObjectOrCollectionInitializerOperation: sbm.Append(Assign(sa, false)).Append(' '); break;
                    case IInvocationOperation iv: sbm.Append(E(iv)).Append("; "); break;
                    case IMemberInitializerOperation inner: sbm.Append(MemberInit(inner)); break;
                    default: sbm.Append(Fail(init, "object initializer member: " + init.Kind)).Append("; "); break;
                }
            }
        }
        finally { _implicit.Pop(); }
        return sbm.ToString();
    }

    string NewArray(IArrayCreationOperation ac)
    {
        var at = (IArrayTypeSymbol)ac.Type;
        var et = RTf(ac, at.ElementType);
        if (at.Rank == 1)
        {
            if (ac.Initializer != null)
            {
                var elems = ac.Initializer.ElementValues.Select(x => E(x, true)).ToList();
                if (elems.Count == 0) return $"Arr::<{et}>::new(0)";
                return $"Arr::<{et}>::from_slice(&[{string.Join(", ", elems)}])";
            }
            return $"Arr::<{et}>::new({Index(ac.DimensionSizes[0])})";
        }
        if (at.Rank == 2)
        {
            if (ac.Initializer != null)
            {
                var rows = new List<string>();
                foreach (var row in ac.Initializer.ElementValues)
                {
                    if (row is not IArrayInitializerOperation ri) return Fail(ac, "2-D array initializer shape");
                    rows.Add("&[" + string.Join(", ", ri.ElementValues.Select(x => E(x, true))) + "]");
                }
                return $"Arr2::<{et}>::from_rows(&[{string.Join(", ", rows)}])";
            }
            return $"Arr2::<{et}>::new({Index(ac.DimensionSizes[0])}, {Index(ac.DimensionSizes[1])})";
        }
        if (at.Rank == 3)
        {
            if (ac.Initializer != null)
            {
                var planes = new List<string>();
                foreach (var plane in ac.Initializer.ElementValues)
                {
                    if (plane is not IArrayInitializerOperation pi) return Fail(ac, "3-D array initializer shape");
                    var rows = new List<string>();
                    foreach (var row in pi.ElementValues)
                    {
                        if (row is not IArrayInitializerOperation ri) return Fail(ac, "3-D array initializer shape");
                        rows.Add("&[" + string.Join(", ", ri.ElementValues.Select(x => E(x, true))) + "]");
                    }
                    planes.Add("&[" + string.Join(", ", rows) + "]");
                }
                return $"Arr3::<{et}>::from_nested(&[{string.Join(", ", planes)}])";
            }
            return $"Arr3::<{et}>::new({E(ac.DimensionSizes[0])}, {E(ac.DimensionSizes[1])}, {E(ac.DimensionSizes[2])})";
        }
        return Fail(ac, "array of rank " + at.Rank);
    }

    // ---- delegates
    HashSet<ISymbol> _cells;
    /// Locals / parameters that a lambda captures and that are assigned after their declaration (by the method
    /// or by the lambda): they live in a heap cell shared by both, as in the C# compiler's closure class.
    HashSet<ISymbol> Cells()
    {
        if (_cells != null) return _cells;
        _cells = new HashSet<ISymbol>(SymbolEqualityComparer.Default);
        if (_root == null) return _cells;
        var mut = Mutated();
        foreach (var lam in _root.DescendantsAndSelf().OfType<IAnonymousFunctionOperation>())
        {
            var inside = new HashSet<ISymbol>(SymbolEqualityComparer.Default);
            foreach (var d in lam.Body.DescendantsAndSelf())
            {
                if (d is IVariableDeclaratorOperation v) inside.Add(v.Symbol);
                if (d is IDeclarationExpressionOperation { Expression: ILocalReferenceOperation dl }) inside.Add(dl.Local);
                if (d is IDeclarationPatternOperation { DeclaredSymbol: not null } dp) inside.Add(dp.DeclaredSymbol);
                if (d is IAnonymousFunctionOperation nl) foreach (var np in nl.Symbol.Parameters) inside.Add(np);
                if (d is ILocalFunctionOperation nlf) foreach (var np in nlf.Symbol.Parameters) inside.Add(np);
            }
            foreach (var p in lam.Symbol.Parameters) inside.Add(p);
            foreach (var d in lam.Body.DescendantsAndSelf())
            {
                ISymbol s = d is ILocalReferenceOperation l ? l.Local : d is IParameterReferenceOperation p ? p.Parameter : null;
                if (s == null || inside.Contains(s) || !mut.Contains(s)) continue;
                if (s is IParameterSymbol { RefKind: not RefKind.None } || s is ILocalSymbol { RefKind: not RefKind.None }) continue;
                _cells.Add(s);
            }
        }
        return _cells;
    }
    bool IsCell(ISymbol s) => _root != null && Cells().Contains(s);

    HashSet<ISymbol> Mutated()
    {
        if (_mutated != null) return _mutated;
        _mutated = new HashSet<ISymbol>(SymbolEqualityComparer.Default);
        void Mark(IOperation target)
        {
            while (true)
            {
                if (target is ILocalReferenceOperation l) { _mutated.Add(l.Local); return; }
                if (target is IParameterReferenceOperation p) { _mutated.Add(p.Parameter); return; }
                if (target is IFieldReferenceOperation f && f.Instance != null && f.Instance.Type != null && f.Instance.Type.IsValueType) { target = f.Instance; continue; }
                if (target is IDeclarationExpressionOperation de) { target = de.Expression; continue; }
                return;
            }
        }
        if (_root != null)
            foreach (var d in _root.DescendantsAndSelf())
            {
                switch (d)
                {
                    case IAssignmentOperation a: Mark(a.Target); break;
                    case IIncrementOrDecrementOperation i: Mark(i.Target); break;
                    case IArgumentOperation { Parameter.RefKind: RefKind.Ref or RefKind.Out } arg: Mark(arg.Value); break;
                    case IVariableDeclaratorOperation v when v.GetVariableInitializer() == null && !(v.Parent?.Parent is IForEachLoopOperation) && v.Parent is not IForEachLoopOperation && v.Parent is not ICatchClauseOperation: _mutated.Add(v.Symbol); break;
                    case IInvocationOperation { Instance: { } inst } when Mono.Sub(inst.Type)?.IsValueType == true: Mark(inst); break;
                }
            }
        return _mutated;
    }

    string DelegateNew(IDelegateCreationOperation dc)
    {
        var dr = M.RT(dc.Type);
        if (dr == null || !dr.StartsWith("Delegate<")) return Fail(dc, "delegate type with no Rust representation");
        var sig = dr.Substring("Delegate<".Length, dr.Length - "Delegate<".Length - 1);
        if (dc.Target is IAnonymousFunctionOperation lam)
        {
            var sym = lam.Symbol;
            if (sym.IsAsync) return Fail(dc, "async lambda");
            // captured variables must never be reassigned (then capture by value is exact)
            var inside = new HashSet<ISymbol>(SymbolEqualityComparer.Default);
            foreach (var d in lam.Body.DescendantsAndSelf())
            {
                if (d is IVariableDeclaratorOperation v) inside.Add(v.Symbol);
                if (d is IDeclarationExpressionOperation { Expression: ILocalReferenceOperation dl }) inside.Add(dl.Local);
                if (d is IDeclarationPatternOperation { DeclaredSymbol: not null } dp) inside.Add(dp.DeclaredSymbol);
                if (d is IAnonymousFunctionOperation nl) foreach (var np in nl.Symbol.Parameters) inside.Add(np);
                if (d is ILocalFunctionOperation nlf) foreach (var np in nlf.Symbol.Parameters) inside.Add(np);
            }
            foreach (var p in sym.Parameters) inside.Add(p);
            // what the closure captures: the collector has to see those handles (rt::Delegate::new_traced)
            var caps = new List<string>();
            // captured by value: rebound inside the closure, so that the body may take their address (`ref` arguments,
            // struct method calls) as it may for any local; a variable that is assigned lives in a heap cell instead
            var byValue = new List<string>();
            bool capThis = false;
            foreach (var d in lam.Body.DescendantsAndSelf())
            {
                if (d is IInstanceReferenceOperation { ReferenceKind: InstanceReferenceKind.ContainingTypeInstance }) { capThis = true; continue; }
                ISymbol s = d is ILocalReferenceOperation l ? l.Local : d is IParameterReferenceOperation p ? p.Parameter : null;
                if (s == null || inside.Contains(s)) continue;
                if (!IsCell(s) && !byValue.Contains(LocalName(s))) byValue.Add(LocalName(s));
                if (s is IParameterSymbol { RefKind: not RefKind.None } || s is ILocalSymbol { RefKind: not RefKind.None }) return Fail(dc, "lambda captures a ref parameter / ref local");
                // a captured variable that is reassigned lives in a heap cell (Cells()); the closure copies the cell handle
                var st = s is ILocalSymbol ls ? ls.Type : ((IParameterSymbol)s).Type;
                var srt = M.RT(st);
                var cn = LocalName(s);
                if (srt != null && (Cells().Contains(s) || Gen.NeedsTrace(srt)) && !caps.Contains(cn)) caps.Add(cn);
            }
            if (capThis && OwnerIsStruct) return Fail(dc, "lambda captures `this` of a struct");
            var ps = new List<string>();
            foreach (var p in sym.Parameters)
            {
                var pt = RTf(dc, p.Type);
                ps.Add($"mut {LocalName(p)}: {(p.RefKind is RefKind.Ref or RefKind.Out ? "*mut " + pt : pt)}");
            }
            var ret = sym.ReturnsVoid ? "()" : RTf(dc, sym.ReturnType);
            // nested function context
            // (a closure cannot await: inside it the method's resumable form calls the plain functions)
            var sSb = sb; var sInd = ind; var sJumps = _jumps; var sTries = _tries; var sRet = _retType; var sDecl = _pendingDecls; var sRes = Resumable;
            sb = new StringBuilder(); ind = sInd + 1; _jumps = new(SymbolEqualityComparer.Default); _tries = new(); _retType = ret; _pendingDecls = new(); Resumable = false;
            string body;
            try
            {
                foreach (var bv in byValue) Line($"let mut {bv} = {bv};");
                foreach (var p in sym.Parameters.Where(p => IsCell(p)))
                    Line($"let mut {LocalName(p)}: Gc<{M.RT(p.Type)}> = rt::new_cell::<{M.RT(p.Type)}>({LocalName(p)});");
                Top(lam.Body);
                if (ret != "()") Line("rt::unreachable()");
                body = sb.ToString();
            }
            finally { sb = sSb; ind = sInd; _jumps = sJumps; _tries = sTries; _retType = sRet; _pendingDecls = sDecl; Resumable = sRes; }
            var pad = new string(' ', ind * 4);
            var clo = $"Box::new(move |{string.Join(", ", ps)}|{(ret == "()" ? "" : " -> " + ret)} {{ unsafe {{\n{body}{pad}}} }})";
            if (capThis && !Owner.IsStatic) caps.Insert(0, "rt::Gc(this)");
            if (caps.Count == 0) return $"Delegate::<{sig}>::new({clo})";
            return $"Delegate::<{sig}>::new_traced({clo}, Box::new(move |__m: &mut rt::Marker| unsafe {{ {string.Join(" ", caps.Select(c => $"rt::Trace::trace(&{c}, __m);"))} }}))";
        }
        if (dc.Target is IMethodReferenceOperation mr)
        {
            var m = mr.Method;
            if (m.ReducedFrom != null) return Fail(dc, "delegate from an extension method group");
            var def = m.OriginalDefinition;
            var inv = (dc.Type as INamedTypeSymbol)?.DelegateInvokeMethod;
            if (inv == null || inv.Parameters.Length != m.Parameters.Length) return Fail(dc, "delegate from a method group with a different parameter list");
            var names = m.Parameters.Select((p, k) => $"__a{k}").ToList();
            // arguments: the delegate's parameter types may be more derived than the method's (contravariance)
            var args = new List<string>();
            for (int k = 0; k < names.Count; k++)
            {
                var from = M.RT(inv.Parameters[k].Type); var to = M.RT(m.Parameters[k].Type);
                if (from == null || to == null) return Fail(dc, "delegate from a method with an unrepresentable parameter");
                args.Add(from == to || m.Parameters[k].RefKind != RefKind.None ? names[k] : IsGc(from) && IsGc(to) ? $"{names[k]}.up::<{Inner(to)}>()" : IsGc(to) ? Box(inv.Parameters[k].Type, names[k], dc) : names[k]);
            }
            string cap = "", tracer = "None", recvAddr = "0";
            IOperation recvOp = null;
            if (!m.IsStatic)
            {
                if (mr.Instance == null || mr.Instance.Type == null) return Fail(dc, "delegate from an instance method without receiver");
                var it = mr.Instance.Type; var ir = M.RT(it);
                if (ir == null || it is ITypeParameterSymbol) return Fail(dc, "delegate from a method of a receiver with no Rust representation");
                var rn = Tmp("d");
                if (it.IsValueType)
                {
                    // C# boxes a copy of the struct when the delegate is created
                    cap = $"let {rn} = rt::new_cell::<{ir}>({E(mr.Instance)}); ";
                    recvOp = R($"(*{rn}.0)", it, dc, true);
                    tracer = $"Some(Box::new(move |__m: &mut rt::Marker| __m.mark({rn})))";
                }
                else if (IsGc(ir))
                {
                    cap = $"let {rn} = {E(mr.Instance)}; ";
                    recvOp = R(rn, it, dc);
                    tracer = $"Some(Box::new(move |__m: &mut rt::Marker| __m.mark({rn})))";
                    recvAddr = $"{rn}.addr()";
                }
                else return Fail(dc, "delegate from a method of a string / array receiver");
            }
            var call = CallMethod(m, recvOp, args, dc, mr.Instance != null && IsBase(mr.Instance), null);
            var dret = inv.ReturnsVoid ? "()" : M.RT(inv.ReturnType);
            var mret = m.ReturnsVoid ? "()" : M.RT(m.ReturnType);
            if (dret == null || mret == null) return Fail(dc, "delegate from a method with an unrepresentable result");
            if (dret != mret) call = IsGc(dret) && IsGc(mret) ? $"{call}.up::<{Inner(dret)}>()" : dret == "()" ? $"{{ let _ = {call}; }}" : IsGc(dret) ? Box(m.ReturnType, call, dc) : call;
            // a method-group delegate has value identity (method + receiver): `evt -= Handler` relies on it.
            // The method is identified by a hash of its documentation id (never 0).
            ulong h = 14695981039346656037UL;
            foreach (var ch in M.DocId(def) + "|" + Mono.Key(Mono.Sub(m.ContainingType)) + "|" + string.Join(",", m.TypeArguments.Select(x => Mono.Key(Mono.Sub(x))))) { h ^= ch; h *= 1099511628211UL; }
            M.MethodNames.TryAdd(h | 1, Gen.ClrName(Mono.Sub(m.ContainingType)) + "." + m.Name);
            var d = $"Delegate::<{sig}>::from_method(Box::new(move |{string.Join(", ", names)}| unsafe {{ {call} }}), 0x{(h | 1):x}u64, {recvAddr}, {tracer})";
            return cap == "" ? d : $"{{ {cap}{d} }}";
        }
        if (dc.Target.Type != null && dc.Target.Type.TypeKind == TypeKind.Delegate) return Fail(dc, "delegate created from another delegate");
        return Fail(dc, "delegate target " + dc.Target.Kind);
    }
}
