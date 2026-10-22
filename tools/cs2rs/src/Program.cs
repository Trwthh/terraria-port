// cs2rs: C# -> Rust translator driver. See ..\DESIGN.md and ..\README.md.
using System.Diagnostics;
using System.Text;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;

namespace Cs2Rs;

public static class Program
{
    static readonly UTF8Encoding Utf8 = new(false);

    /// Stack of every thread that parses, binds or emits. Roslyn's parser and binder and this emitter recurse once
    /// per nesting level of the source (`Projectile.AI` nests `else if` 880 deep). Roslyn guards its recursion with
    /// `EnsureSufficientExecutionStack`: on a thread with too little stack left it does not crash, it reports
    /// CS8078 and replaces the WHOLE compilation unit by skipped text, so the file declares no types. On pool
    /// threads (`Parallel.For`, 1.5 MB, with a call depth that depends on work stealing) that happened to
    /// Projectile.cs now and then: "type not found: Terraria.Projectile". DESIGN.md section 12.
    public const int WorkerStack = 512 << 20;

    public static int Main(string[] args)
    {
        int rc = 0;
        var t = new Thread(() => rc = Main2(args), WorkerStack);
        t.Start(); t.Join();
        return rc;
    }

    /// `body(order[k])` for every k on `dop` dedicated threads with `WorkerStack` each (never the thread pool).
    /// The first exception is rethrown on the caller after all workers stopped.
    public static void ParallelBig(IReadOnlyList<int> order, int dop, Action<int> body)
    {
        int next = -1; Exception failure = null;
        var threads = new List<Thread>();
        for (int w = 0; w < Math.Max(1, Math.Min(dop, order.Count)); w++)
        {
            var th = new Thread(() =>
            {
                while (Volatile.Read(ref failure) == null)
                {
                    int k = Interlocked.Increment(ref next);
                    if (k >= order.Count) return;
                    try { body(order[k]); }
                    catch (Exception e) { Interlocked.CompareExchange(ref failure, e, null); return; }
                }
            }, WorkerStack);
            th.Start(); threads.Add(th);
        }
        foreach (var th in threads) th.Join();
        if (failure != null) System.Runtime.ExceptionServices.ExceptionDispatchInfo.Capture(failure).Throw();
    }
    public static int Dop => Math.Max(1, Environment.ProcessorCount - 2);

    static int Main2(string[] args)
    {
        string config = null, outDir = null, coverage = null, report = null, snippets = null;
        HostOut = null; HostMd = null;
        var dryTypes = new List<string>();
        bool diagnostics = false;
        for (int i = 0; i < args.Length; i++)
        {
            switch (args[i])
            {
                case "--config": config = args[++i]; break;
                case "--out": outDir = args[++i]; break;
                case "--dry-run": dryTypes.AddRange(args[++i].Split(',', StringSplitOptions.RemoveEmptyEntries)); break;
                case "--coverage": coverage = args[++i]; break;
                case "--report": report = args[++i]; break;
                case "--diagnostics": diagnostics = true; break;
                case "--snippets": snippets = args[++i]; break;
                case "--host-out": HostOut = args[++i]; break;
                case "--host-api": HostMd = args[++i]; break;
                case "--all": DryAll = true; break;
                case "--reach": Reach.Add(args[++i]); break;
                case "--float-mode": FloatModeOverride = args[++i]; break;
                case "--skip-native": Config.SkipNativePrefix = args[++i]; break;
                case "--parse-stack-kb": return ParseOnSmallStack(int.Parse(args[++i]), args[++i]);
                default: Console.Error.WriteLine("unknown argument " + args[i]); return Usage();
            }
        }
        if (config == null) return Usage();
        var sw = Stopwatch.StartNew();
        Config cfg;
        try { cfg = Config.Load(config); }
        catch (ConfigException e) { Console.Error.WriteLine("error: " + e.Message); Console.WriteLine("error: " + e.Message); return 1; }
        try { return Run(cfg, sw, snippets, outDir, diagnostics, dryTypes, coverage, report); }
        catch (ConfigException e) { Console.Error.WriteLine("error: " + e.Message); Console.WriteLine("error: " + e.Message); return 1; }
    }

    static int Run(Config cfg, Stopwatch sw, string snippets, string outDir, bool diagnostics, List<string> dryTypes, string coverage, string report)
    {
        if (FloatModeOverride != null) cfg.FloatMode = FloatModeOverride;
        if (snippets != null) return Snippets.Run(cfg, snippets, outDir);
        var comp = BuildCompilation(cfg, cfg.Sources.Select(s => Path.GetFullPath(Path.Combine(cfg.BaseDir, s))), out int files);
        Console.WriteLine($"parsed {files} files, {comp.References.Count()} references in {sw.ElapsedMilliseconds} ms");
        int errorCount = -1;
        if (diagnostics)
        {
            var errs = comp.GetDiagnostics().Where(d => d.Severity == DiagnosticSeverity.Error).ToList();
            errorCount = errs.Count;
            Console.WriteLine($"compilation errors: {errs.Count} ({sw.ElapsedMilliseconds} ms)");
            foreach (var g in errs.GroupBy(d => d.Id).OrderByDescending(g => g.Count()).Take(15))
                Console.WriteLine($"  {g.Key} x{g.Count()}: {g.First().GetMessage()} @ {g.First().Location.GetLineSpan().Path}:{g.First().Location.GetLineSpan().StartLinePosition.Line + 1}");
        }
        if (dryTypes.Count > 0 || DryAll) return DryRun.Run(cfg, comp, dryTypes, coverage, errorCount, sw, DryAll);
        if (outDir == null) return Usage();
        return Translate(cfg, comp, outDir, report, errorCount, sw);
    }

    /// Directory of the generated host boundary (default: `<out>/../host/api`) and the path of HOST_API.md.
    public static string HostOut, HostMd;
    public static bool DryAll;
    /// `--float-mode f32|x87|x87-locals`: overrides "floatMode" of the config (for the comparison runs of DESIGN.md section 5).
    public static string FloatModeOverride;
    /// Rust paths (or prefixes ending with `*`) whose reachable gaps are reported (`--reach terraria::main::initialize`).
    public static List<string> Reach = new();

    /// Diagnostic for the failure `WorkerStack` prevents: parses one file on a thread with `kb` of stack and
    /// prints what Roslyn made of it. `cs2rs --parse-stack-kb 256 ../../decompile/Terraria/Terraria/Projectile.cs`
    static int ParseOnSmallStack(int kb, string file)
    {
        string result = null;
        var th = new Thread(() =>
        {
            var tree = CSharpSyntaxTree.ParseText(File.ReadAllText(file), new CSharpParseOptions(LanguageVersion.Latest), file, Utf8);
            var errs = tree.GetDiagnostics().Where(d => d.Severity == DiagnosticSeverity.Error).ToList();
            int types = tree.GetRoot().DescendantNodes(n => n is not Microsoft.CodeAnalysis.CSharp.Syntax.MethodDeclarationSyntax).OfType<Microsoft.CodeAnalysis.CSharp.Syntax.BaseTypeDeclarationSyntax>().Count();
            result = $"stack {kb} KB: {types} type declaration(s), {errs.Count} error(s){(errs.Count > 0 ? ": " + errs[0].Id + " " + errs[0].GetMessage() : "")}";
        }, kb * 1024);
        th.Start(); th.Join();
        Console.WriteLine(result);
        return 0;
    }

    static int Usage()
    {
        Console.Error.WriteLine("usage: cs2rs --config <file> --out <dir> [--report <md>] [--diagnostics]\n       cs2rs --config <file> --dry-run <Type,Type,...> --coverage <md> [--diagnostics]\n       cs2rs --config <file> --snippets <dir> --out <dir>");
        return 2;
    }

    public static CSharpCompilation BuildCompilation(Config cfg, IEnumerable<string> sourceDirs, out int fileCount)
    {
        var files = new List<string>();
        foreach (var d in sourceDirs)
        {
            if (File.Exists(d)) files.Add(d);
            else files.AddRange(Directory.EnumerateFiles(d, "*.cs", SearchOption.AllDirectories).Where(f => !f.Contains(Path.DirectorySeparatorChar + "obj" + Path.DirectorySeparatorChar) && !f.Contains(Path.DirectorySeparatorChar + "bin" + Path.DirectorySeparatorChar)));
        }
        files.Sort(StringComparer.Ordinal);
        fileCount = files.Count;
        var po = new CSharpParseOptions(LanguageVersion.Latest);
        var trees = new SyntaxTree[files.Count];
        ParallelBig(Enumerable.Range(0, files.Count).ToList(), Dop, i => trees[i] = CSharpSyntaxTree.ParseText(File.ReadAllText(files[i]), po, files[i], Utf8));
        // "An expression is too long or complex to compile": Roslyn ran out of stack and dropped (part of) a file.
        // Never continue with a compilation that silently lacks declarations.
        foreach (var t in trees)
            foreach (var d in t.GetDiagnostics())
                if (d.Id == "CS8078") throw new ConfigException($"{t.FilePath}: {d.Id} {d.GetMessage()} (parser ran out of stack; see Program.WorkerStack)");
        var refs = new List<MetadataReference>();
        foreach (var r in cfg.References)
        {
            var full = Path.GetFullPath(Path.Combine(cfg.BaseDir, r));
            var dir = Path.GetDirectoryName(full); var pat = Path.GetFileName(full);
            if (!Directory.Exists(dir)) { Console.Error.WriteLine("warning: reference directory missing: " + dir); continue; }
            foreach (var f in Directory.GetFiles(dir, pat).OrderBy(x => x, StringComparer.Ordinal)) refs.Add(MetadataReference.CreateFromFile(f));
        }
        var opts = new CSharpCompilationOptions(OutputKind.DynamicallyLinkedLibrary, allowUnsafe: true, concurrentBuild: true, assemblyIdentityComparer: DesktopAssemblyIdentityComparer.Default);
        return CSharpCompilation.Create("Terraria", trees, refs, opts);
    }

    public static INamedTypeSymbol FindType(CSharpCompilation comp, string name)
    {
        var t = comp.GetTypeByMetadataName(name);
        if (t != null) return t;
        // Outer.Inner written with dots
        var parts = name.Split('.');
        for (int k = parts.Length - 1; k > 0; k--)
        {
            t = comp.GetTypeByMetadataName(string.Join(".", parts.Take(k)) + "+" + string.Join("+", parts.Skip(k)));
            if (t != null) return t;
        }
        // GetTypeByMetadataName gives up when the name is ambiguous across assemblies; the source declaration wins
        var simple = parts[^1].Split('+')[^1];
        var want = name.Replace('+', '.');
        foreach (var s in comp.GetSymbolsWithName(simple, SymbolFilter.Type).OfType<INamedTypeSymbol>())
            if (Model.FullName(s) == want) return s;
        Console.Error.WriteLine($"FindType: no declaration of {name} among {comp.SyntaxTrees.Count()} syntax trees");
        return null;
    }

    /// Types selected by rule: `namespaces` (whole namespaces) and `autoStructs` (every struct / interface / small
    /// data class the sources declare). Added after the listed types, in a fixed order.
    public static void AddByRule(Config cfg, CSharpCompilation comp, Model model)
    {
        if (cfg.Namespaces.Count == 0 && !cfg.AutoStructs) return;
        var all = new List<INamedTypeSymbol>();
        void Walk(INamespaceOrTypeSymbol s)
        {
            foreach (var m in s.GetMembers())
            {
                if (m is INamespaceSymbol ns) Walk(ns);
                else if (m is INamedTypeSymbol t && t.Locations.Any(l => l.IsInSource))
                {
                    if (t.TypeKind is TypeKind.Class or TypeKind.Struct or TypeKind.Interface) all.Add(t);
                    Walk(t);
                }
            }
        }
        Walk(comp.Assembly.GlobalNamespace);
        bool InNs(INamedTypeSymbol t)
        {
            var ns = t.ContainingNamespace?.ToDisplayString() ?? "";
            foreach (var e in cfg.Namespaces)
            {
                if (e.EndsWith(".*")) { var p = e.Substring(0, e.Length - 2); if (ns == p || ns.StartsWith(p + ".")) return true; }
                else if (ns == e) return true;
            }
            return false;
        }
        foreach (var t in all.OrderBy(t => t.ToDisplayString(), StringComparer.Ordinal))
        {
            if (model.Types.ContainsKey(t)) continue;
            if (t.IsImplicitlyDeclared || t.Name.StartsWith("<")) continue;
            var full = Model.FullName(t);
            if (cfg.Exclude.Any(x => x == full || (x.EndsWith(".*") && full.StartsWith(x.Substring(0, x.Length - 1))))) continue;
            // a game class that hand-written Rust provides (`nativeTypes`: UnifiedRandom) is not translated as well
            if (cfg.NativeTypes.ContainsKey(model.DocId(t))) continue;
            bool take = InNs(t) || (cfg.AutoStructs && t.TypeKind is TypeKind.Struct or TypeKind.Interface);
            // nested types follow their container when it is translated whole
            if (!take && t.ContainingType != null && model.Types.TryGetValue(t.ContainingType, out var outer) && outer.Cfg?.Members == null) take = true;
            if (!take && t.ContainingType != null && model.GenericDefs.Contains(t.ContainingType)) take = true;
            if (!take) continue;
            // a generic class (or a type nested in one) is emitted once per closed instantiation (Mono.cs)
            if (t.IsGenericType) model.GenericDefs.Add(t);
            else model.AddConfigured(t, new TypeCfg { Type = full });
        }
    }

    public static bool WriteIfChanged(string path, string text)
    {
        path = Path.GetFullPath(path);
        var bytes = Utf8.GetBytes(text.Replace("\r\n", "\n"));
        if (File.Exists(path) && File.ReadAllBytes(path).AsSpan().SequenceEqual(bytes)) return false;
        Directory.CreateDirectory(Path.GetDirectoryName(path));
        File.WriteAllBytes(path, bytes);
        return true;
    }

    sealed class Node { public SortedDictionary<string, Node> Kids = new(StringComparer.Ordinal); public TType Type; public string Text; }

    public static int Translate(Config cfg, CSharpCompilation comp, string outDir, string report, int errorCount, Stopwatch sw, string header = null, string rootExtra = null)
    {
        var model = new Model(cfg, comp, false);
        foreach (var tc in cfg.Types)
        {
            var sym = FindType(comp, tc.Type);
            if (sym == null) { Console.Error.WriteLine("error: type not found: " + tc.Type); return 1; }
            if (sym.IsGenericType) model.GenericDefs.Add(sym); else model.AddConfigured(sym, tc);
        }
        AddByRule(cfg, comp, model);
        var gen = new Gen(model);
        // closed instantiations of generic classes and generic methods (Mono.cs), before anything is named
        var disc = new Discovery(model, gen);
        disc.Run();
        foreach (var deep in disc.TooDeep.Distinct().OrderBy(x => x, StringComparer.Ordinal)) Console.Error.WriteLine("warning: instantiation nested deeper than " + Discovery.MaxDepth + " not generated: " + deep);
        model.AddInstances(disc);
        Console.WriteLine($"generic instantiations: {disc.TypeInsts.Count} types of {model.GenericDefs.Count} generic definitions ({model.Unused.Count} never instantiated), {disc.MethodInsts.Count} methods ({sw.ElapsedMilliseconds} ms)");
        model.PopulateAll();
        foreach (var u in model.Unused) gen.ReportOnly(u);
        var root = new Node();
        string srcRoot = cfg.Sources.Count > 0 ? Path.GetFullPath(Path.Combine(cfg.BaseDir, cfg.Sources[0])) : "";
        var texts = new string[model.Ordered.Count];
        // big classes first: they dominate the wall time
        var order = Enumerable.Range(0, model.Ordered.Count).OrderByDescending(i => model.Ordered[i].Sym.Locations.Where(l => l.IsInSource).Sum(l => l.SourceSpan.Length)).ToList();
        ParallelBig(order, Dop, i =>
        {
            var t = model.Ordered[i];
            var loc = t.Sym.Locations.FirstOrDefault(l => l.IsInSource);
            string rel = loc == null ? "?" : Path.GetRelativePath(Path.GetDirectoryName(srcRoot) ?? srcRoot, loc.GetLineSpan().Path).Replace('\\', '/');
            texts[i] = gen.EmitType(t, cfg.GenModule, rel);
        });
        for (int i = 0; i < model.Ordered.Count; i++)
        {
            var t = model.Ordered[i];
            var n = root;
            foreach (var seg in t.Module) { if (!n.Kids.TryGetValue(seg, out var k)) n.Kids[seg] = k = new Node(); n = k; }
            n.Type = t; n.Text = texts[i];
        }
        // files
        var written = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        int changed = 0;
        void Emit(Node n, string dir, string name)
        {
            var mods = string.Concat(n.Kids.Keys.Select(k => $"pub mod {k};\n"));
            string text;
            if (n.Text != null) text = n.Text.Replace("//@@MODS@@\n", mods == "" ? "" : mods + "\n");
            else text = "// @generated by tools/cs2rs. Do not edit.\n" + mods;
            string path;
            if (name == null) path = Path.Combine(dir, "mod.rs");
            else path = Path.Combine(dir, name + ".rs");
            if (name == null) text = (header ?? GenHeader) + mods + "pub mod types;\npub mod ifaces;\npub mod enums;\npub mod meta;\npub mod resumable;\n" + (rootExtra ?? "");
            if (WriteIfChanged(path, text)) changed++;
            written.Add(Path.GetFullPath(path));
            foreach (var kv in n.Kids) Emit(kv.Value, name == null ? dir : Path.Combine(dir, name), kv.Key);
        }
        Directory.CreateDirectory(outDir);
        Emit(root, outDir, null);
        // type table
        var tt = new StringBuilder("// @generated by tools/cs2rs. Do not edit.\n//! Type ids of translated classes and their parent table (0 = no translated base).\n\n");
        var classes = model.Ordered.Where(t => t.TypeId != 0).OrderBy(t => t.TypeId).ToList();
        int nIds = Model.FirstTypeId + classes.Count;
        tt.Append($"/// Ids below {Model.FirstTypeId} are the runtime's (`rt::tid`).\npub static PARENT: [u32; {nIds}] = [\n    {string.Join(", ", Enumerable.Repeat("0", Model.FirstTypeId))},\n");
        foreach (var c in classes) tt.Append($"    {(c.Base != null ? c.Base.TypeId : 0)}, // {c.TypeId} {Model.FullName(c.Sym)}\n");
        tt.Append("];\n\npub static NAMES: [&str; " + nIds + "] = [\n    " + string.Join(", ", Enumerable.Repeat("\"\"", Model.FirstTypeId)) + ",\n");
        foreach (var c in classes) tt.Append($"    \"{Model.FullName(c.Sym)}\",\n");
        tt.Append("];\n");
        // translated classes that derive from System.Exception: their layout starts with the runtime exception
        bool ExcClass(TType c) { for (var x = c; x != null; x = x.Base) if (x.ExcKind != null) return true; return false; }
        var excIds = classes.Where(ExcClass).Select(c => c.TypeId.ToString()).ToList();
        tt.Append("\n/// Is the class with this type id a translated exception class (`(Exception)o`, `o is Exception`)?\npub fn is_exception(id: u32) -> bool {\n    " + (excIds.Count == 0 ? "let _ = id;\n    false" : "matches!(id, " + string.Join(" | ", excIds) + ")") + "\n}\n");
        // `Assembly.GetTypes()` of the game's executable, as far as static initialisation goes
        if (cfg.TypeOrder != null)
        {
            var byName = new Dictionary<string, TType>(StringComparer.Ordinal);
            foreach (var c in model.Ordered.Concat(model.Unused)) if (!c.Sym.IsGenericType && c.WholeTypeReason == null) byName.TryAdd(Gen.ClrName(c.Sym), c);
            var inits = new StringBuilder(); int total = 0, listed = 0;
            foreach (var line in File.ReadLines(Path.Combine(cfg.BaseDir, cfg.TypeOrder)))
            {
                var p = line.Split('\t');
                if (p.Length != 3) continue;
                total++;
                // generic types are skipped by the game itself (`!type.IsGenericType`)
                if (p[1] != "0" || !byName.TryGetValue(p[0], out var c) || !c.HasStatics) continue;
                if (cfg.ForceInitSkip.TryGetValue(p[0], out var skipWhy)) { inits.Append($"    // not here: {p[0]} ({skipWhy.Replace("\n", " ")})\n"); continue; }
                inits.Append($"    ({Gen.Q(p[0])}, {{ unsafe fn f() {{ let _ = crate::gen::{c.ModPath}::s(); }} f }}),\n");
                listed++;
            }
            tt.Append($"\n/// Number of types `Assembly.GetTypes()` returns for the game's executable (metadata of Terraria.exe).\npub const ASSEMBLY_TYPE_COUNT: i32 = {total};\n");
            tt.Append($"\n/// The translated non-generic types of the game's executable that have a static initialiser, in metadata order\n/// (the order of `Assembly.GetTypes()`): `(CLR name, run the initialiser if it has not run)`. For\n/// `Program.ForceStaticInitializers`. Types without static state have nothing to run.\npub static STATIC_INITS: [(&str, unsafe fn()); {listed}] = [\n{inits}];\n");
        }
        var tp = Path.Combine(outDir, "types.rs");
        if (WriteIfChanged(tp, tt.ToString())) changed++;
        written.Add(Path.GetFullPath(tp));

        var orderOf = new Dictionary<TType, int>();
        foreach (var t in model.Ordered.Concat(model.Unused)) orderOf[t] = orderOf.Count;
        var reports = gen.Reports.OrderBy(r => orderOf[r.Type]).ThenBy(r => r.Sym?.Locations.FirstOrDefault()?.SourceSpan.Start ?? -1).ThenBy(r => r.Display, StringComparer.Ordinal).ToList();
        // gap list
        var up = new StringBuilder("# Every call / field the generated code reaches that is neither translated nor mapped.\n# `rt::unported` throws at run time, `rt::gap` (void calls) logs once and continues, `rt::stub` returns the value given in the config.\n");
        foreach (var g in reports.SelectMany(r => r.Unported.Select(u => (u, r))).GroupBy(x => x.u).OrderBy(g => g.Key, StringComparer.Ordinal))
            up.Append($"unported  {g.Key}    <- {string.Join(", ", g.Select(x => x.r.Display).Distinct().Take(6))}{(g.Select(x => x.r.Display).Distinct().Count() > 6 ? ", ..." : "")}\n");
        foreach (var g in reports.SelectMany(r => r.Stubs.Select(u => (u, r))).GroupBy(x => x.u).OrderBy(g => g.Key, StringComparer.Ordinal))
            up.Append($"stub      {g.Key}    <- {string.Join(", ", g.Select(x => x.r.Display).Distinct().Take(6))}\n");
        foreach (var r in reports.Where(r => r.Status == MStatus.Failed))
            up.Append($"failed    {r.Type.Sym.Name}: {r.Display}    ({r.Fails[0].kind} at {r.Fails[0].where})\n");
        foreach (var r in reports.Where(r => r.Status == MStatus.NotEmitted))
            up.Append($"skipped   {r.Type.Sym.Name}: {r.Display}    ({r.NotEmittedReason})\n");
        var upPath = Path.Combine(outDir, "UNPORTED.txt");
        if (WriteIfChanged(upPath, up.ToString())) changed++;
        written.Add(Path.GetFullPath(upPath));
        // generated support modules: interface dispatchers of library interfaces, enum name tables
        foreach (var (name, text) in gen.SupportModules(cfg.GenModule))
        {
            var sp = Path.Combine(outDir, name + ".rs");
            if (WriteIfChanged(sp, text)) changed++;
            written.Add(Path.GetFullPath(sp));
        }
        // host boundary
        var hostDir = HostOut ?? Path.Combine(outDir, "..", "host", "api");
        var (hfiles, hchanged) = model.Host.WriteRust(hostDir, cfg.GenModule, cfg.HostRoot);
        changed += hchanged;
        foreach (var f in hfiles) written.Add(f);
        if (HostMd != null) WriteIfChanged(HostMd, model.Host.Markdown());
        // stale generated files
        foreach (var dir in new[] { outDir, hostDir }.Select(Path.GetFullPath).Distinct())
            foreach (var f in Directory.EnumerateFiles(dir, "*.rs", SearchOption.AllDirectories))
                if (!written.Contains(Path.GetFullPath(f)) && File.ReadLines(f).FirstOrDefault()?.StartsWith("// @generated by tools/cs2rs") == true) { File.Delete(f); changed++; }

        foreach (var start in Reach) Console.WriteLine(ReachReport(model, reports.Concat(gen.Dispatchers).ToList(), start));
        var md = PilotReport(model, reports, errorCount);
        if (cfg.FloatMode == "jit32")
        {
            var und = model.FloatUndetermined.Distinct().OrderBy(x => x, StringComparer.Ordinal).ToList();
            md += $"\n## Float mode jit32 (FloatAlloc.cs; rule: port\\crates\\terraria-x87\\FLOAT_RULE.md)\n\nBodies with float locals analysed: {model.FloatMethods}; compiled without optimisation in the original (IL >= {FloatAlloc.MinOptsIlBytes} bytes: every local in memory): {model.FloatMinOpts}; float locals kept on the x87 stack: {model.FloatStackLocals}; locals kept in memory because a call they are live across sits in a loop header or another place the emitter cannot spill at: {model.FloatDemoted}. Call sites decided by the reference runs' own machine code (jit_refcalls.tsv, game callees included): {model.JitRefCallsUsed}; one by one where a callee is a call instruction at some sites only (jit_refcall_sites.tsv): {model.JitRefCallSitesUsed}. Call sites decided by the real JIT's code (jit_calls.tsv): inlined {model.JitTableInlined}, calls {model.JitTableCalls}; by the size rule: mixed in the JIT {model.JitTableMixed}, not in the table {model.JitTableMissing}. Library calls evaluated by their x87 body (config jitBodies at the sites of jit_bodies.tsv): {model.JitBodiesUsed}. XNA Vector2 operators the JIT inlined, evaluated as its code does (jit_bodies.tsv, jit_vec_temps.tsv): {model.VecInlinedSites} operator sites, {model.VecInlinedStackSites} of them with the results on the x87 stack; call sites of game methods a caller inlined with other operator answers, calling a `__vcN` twin (jit_vec_ctx.tsv): {model.VecCtxSites}. Bodies whose float locals follow the real JIT's allocation (config jitFloatAlloc, jit_float_alloc.tsv; bodies counted per emitted form): {model.JitFloatAllocMethods} (locals in memory {model.JitFloatAllocMemory}, on the x87 stack {model.JitFloatAllocStack}, evaluation-stack values {model.JitFloatAllocTemps}); left to the rule because the names do not match the decompiled method: {model.JitFloatAllocMismatch}.\n\nMethods where more float variables interfere than there are x87 registers and the JIT's choice among equally weighted ones is not determined (FLOAT_RULE.md 4.1): {und.Count}\n\n" + string.Concat(und.Select(x => "- `" + x + "`\n"));
        }
        if (report != null) WriteIfChanged(report, md);
        if (Environment.GetEnvironmentVariable("CS2RS_FLOAT_REPORT") is { Length: > 0 } frep)
            File.WriteAllText(frep, string.Concat(model.FloatReport.Distinct().OrderBy(x => x, StringComparer.Ordinal).Select(x => x + "\n")));
        Console.WriteLine(Summary(model, reports));
        Console.WriteLine($"wrote {written.Count} files ({changed} changed) to {outDir} in {sw.ElapsedMilliseconds} ms");
        return 0;
    }

    public const string GenHeader =
        "// @generated by tools/cs2rs. Do not edit.\n" +
        "//! Generated modules: one per C# class, path by namespace. See tools/cs2rs/DESIGN.md.\n" +
        "// Lints that mechanical translation cannot satisfy (each one is a property of the C# source, not a bug):\n" +
        "#![allow(non_snake_case)]          // locals and parameters keep the decompiler's spelling (PORTING.md)\n" +
        "#![allow(non_camel_case_types)]    // C# type names with underscores\n" +
        "#![allow(non_upper_case_globals)]\n" +
        "#![allow(unused_variables)]        // C# parameters / locals that the body never reads\n" +
        "#![allow(unused_mut)]              // every local is `let mut`: C# locals are assignable\n" +
        "#![allow(unused_assignments)]      // locals are zero-initialised where C# leaves them unassigned\n" +
        "#![allow(unused_parens)]           // every operator result is parenthesised instead of tracking precedence\n" +
        "#![allow(unused_braces)]\n" +
        "#![allow(unused_labels)]           // every loop is labelled; most labels are never targeted\n" +
        "#![allow(unused_unsafe)]           // lambdas open their own unsafe block\n" +
        "#![allow(unused_imports)]\n" +
        "#![allow(unreachable_code)]        // code after return / break that C# also has, and the trailing rt::unreachable()\n" +
        "#![allow(unreachable_patterns)]\n" +
        "#![allow(dead_code)]\n" +
        "#![allow(static_mut_refs)]\n" +
        "#![allow(clippy::all)]\n\n";

    /// What a call of `start` can reach that is missing: over-approximation by the static call graph of the
    /// translated code (every branch, every override, every static initialiser a body can trigger).
    static string ReachReport(Model model, List<MethodReport> all, string start)
    {
        var byKey = new Dictionary<string, MethodReport>(StringComparer.Ordinal);
        foreach (var r in all) if (r.Key != null) byKey.TryAdd(r.Key, r);
        var parent = new Dictionary<string, string>(StringComparer.Ordinal);
        var queue = new Queue<string>();
        foreach (var k in byKey.Keys.OrderBy(k => k, StringComparer.Ordinal))
            if (k == start || (start.EndsWith("*") && k.StartsWith(start.TrimEnd('*')))) { parent[k] = null; queue.Enqueue(k); }
        var sb = new StringBuilder($"\n== reach {start}: {queue.Count} start function(s)\n");
        var values = new SortedDictionary<string, string>(StringComparer.Ordinal); var gaps = new SortedDictionary<string, string>(StringComparer.Ordinal);
        var hostV = new SortedDictionary<string, string>(StringComparer.Ordinal); var hostVoid = new SortedDictionary<string, string>(StringComparer.Ordinal);
        var refused = new SortedDictionary<string, string>(StringComparer.Ordinal); var hostNew = new SortedDictionary<string, string>(StringComparer.Ordinal);
        var hostFns = model.Host.Fns.Values.GroupBy(f => f.Display).ToDictionary(g => g.Key, g => g.First());
        int n = 0;
        static string Tail(string c) { int a = c.LastIndexOf("::"); int b = a > 0 ? c.LastIndexOf("::", a - 1) : -1; return b >= 0 ? c.Substring(b + 2) : c; }
        string Path(string k) { var parts = new List<string>(); for (var c = k; c != null && parts.Count < 7; c = parent[c]) parts.Add(Tail(c)); parts.Reverse(); return string.Join(" > ", parts); }
        while (queue.Count > 0)
        {
            var k = queue.Dequeue(); n++;
            if (!byKey.TryGetValue(k, out var r)) continue;
            if (r.Status == MStatus.Failed) { refused.TryAdd(r.Display + ": " + r.Fails[0].kind, Path(k)); continue; }
            foreach (var u in r.Unported) { if (r.DeadUnported.Contains(u)) continue; if (r.Gaps.Contains(u)) gaps.TryAdd(u, Path(k)); else values.TryAdd(u, Path(k)); }
            foreach (var h in r.HostCalls)
                if (!r.DeadHostCalls.Contains(h) && hostFns.TryGetValue(h, out var f)) { if (f.Ret == "()") hostVoid.TryAdd(h, Path(k)); else if (f.Kind == "new" && f.Ret == "Obj") hostNew.TryAdd(h, Path(k)); else if (f.Value == null) hostV.TryAdd(h, Path(k)); }
            foreach (var c in r.Calls) if (!r.DeadCalls.Contains(c) && !parent.ContainsKey(c)) { parent[c] = k; queue.Enqueue(c); }
        }
        sb.Append($"functions reachable: {n}\n");
        void Sec(string title, SortedDictionary<string, string> d) { sb.Append($"-- {title}: {d.Count}\n"); foreach (var kv in d) sb.Append($"   {kv.Key}    <- {kv.Value}\n"); }
        Sec("refused bodies (panic when called)", refused);
        Sec("unported VALUES (panic when reached)", values);
        Sec("host VALUES without a configured value (panic when reached)", hostV);
        Sec("gaps (void, logged, continue)", gaps);
        Sec("host void members (logged, continue)", hostVoid);
        Sec("constructors of opaque library classes", hostNew);
        if (model.Cfg.ReachAssume.Count > 0) sb.Append("assumed: " + string.Join(", ", model.Cfg.ReachAssume.OrderBy(k => k.Key, StringComparer.Ordinal).Select(k => k.Key + " = " + (k.Value ? "true" : "false"))) + "\n");
        return sb.ToString();
    }

    static string Summary(Model model, List<MethodReport> reports)
    {
        var sb = new StringBuilder();
        foreach (var t in model.Ordered)
        {
            var rs = reports.Where(r => r.Type == t).ToList();
            sb.Append($"{Model.FullName(t.Sym),-55} clean {rs.Count(r => r.Status == MStatus.Clean),4}  unported-inside {rs.Count(r => r.Status == MStatus.Unported),3}  failed {rs.Count(r => r.Status == MStatus.Failed),3}  not-emitted {rs.Count(r => r.Status == MStatus.NotEmitted),3}\n");
        }
        return sb.ToString();
    }

    static string PilotReport(Model model, List<MethodReport> reports, int errorCount)
    {
        var sb = new StringBuilder("# cs2rs scale report: what was generated\n\nGenerated by `sh run.sh` (`cs2rs --report`). Do not edit.\n\n");
        if (errorCount >= 0) sb.Append($"Roslyn compilation of the whole decompile: {errorCount} errors.\n\n");
        sb.Append("Columns: bodies = methods, accessors, constructors, plus one row each for the static initialiser and the instance field initialisers. ");
        sb.Append("clean = translated with nothing missing; unported inside = translated, but reaches a callee/field that is not translated or mapped (`rt::unported` / `rt::gap`); failed = the emitter met a construct it does not support, the body is `rt::unported_method`; not emitted = no Rust function at all.\n\n");
        sb.Append("| C# type | bodies | clean | unported inside | failed | not emitted |\n|---|---:|---:|---:|---:|---:|\n");
        foreach (var t in model.Ordered)
        {
            var rs = reports.Where(r => r.Type == t).ToList();
            sb.Append($"| `{Model.FullName(t.Sym)}`{(t.Cfg?.Members != null ? " (partial: " + t.Cfg.Members.Count + " members)" : "")} | {rs.Count} | {rs.Count(r => r.Status == MStatus.Clean)} | {rs.Count(r => r.Status == MStatus.Unported)} | {rs.Count(r => r.Status == MStatus.Failed)} | {rs.Count(r => r.Status == MStatus.NotEmitted)} |\n");
        }
        sb.Append($"| **total** | {reports.Count} | {reports.Count(r => r.Status == MStatus.Clean)} | {reports.Count(r => r.Status == MStatus.Unported)} | {reports.Count(r => r.Status == MStatus.Failed)} | {reports.Count(r => r.Status == MStatus.NotEmitted)} |\n\n");
        sb.Append("## Why bodies failed (first unsupported construct per body)\n\n| reason | bodies | example |\n|---|---:|---|\n");
        foreach (var g in reports.Where(r => r.Status == MStatus.Failed).GroupBy(r => DryRun.Group(r.Fails[0].kind)).OrderByDescending(g => g.Count()).ThenBy(g => g.Key, StringComparer.Ordinal))
            sb.Append($"| {g.Key} | {g.Count()} | `{g.First().Display}` {g.First().Fails[0].where} |\n");
        sb.Append("\n## Why members were not emitted\n\n| reason | members | example |\n|---|---:|---|\n");
        foreach (var g in reports.Where(r => r.Status == MStatus.NotEmitted).GroupBy(r => DryRun.Group(r.NotEmittedReason)).OrderByDescending(g => g.Count()).ThenBy(g => g.Key, StringComparer.Ordinal))
            sb.Append($"| {g.Key} | {g.Count()} | `{g.First().Display}` |\n");
        sb.Append("\n## Unported callees reached from translated bodies\n\n| callee | bodies that reach it |\n|---|---:|\n");
        foreach (var g in reports.SelectMany(r => r.Unported.Select(u => (u, r))).GroupBy(x => x.u).OrderByDescending(g => g.Count()).ThenBy(g => g.Key, StringComparer.Ordinal))
            sb.Append($"| `{g.Key}` | {g.Count()} |\n");
        // A class is unusable when its static initialiser panics; these are the library values it needs.
        var needed = model.Host.Fns.Values.Where(f => f.Ret != "()" && f.Value == null && f.Kind != "new").ToDictionary(f => f.Display, f => f);
        sb.Append("\n## Host values that static initialisers and field initialisers need\n\n");
        sb.Append("A value-returning host member that is not implemented panics (`host::unimplemented_value`). When the caller is a static initialiser or a field initialiser the whole class is unusable until the member is mapped natively or given a value in the config (`stubs`).\n\n| host member | initialisers that call it |\n|---|---|\n");
        foreach (var g in reports.Where(r => r.Status != MStatus.Failed && (r.Display.Contains("..cctor") || r.Display.EndsWith("instance field initialisers")))
                     .SelectMany(r => r.HostCalls.Where(h => needed.ContainsKey(h)).Select(h => (h, r))).GroupBy(x => x.h).OrderBy(g => g.Key, StringComparer.Ordinal))
            sb.Append($"| `{g.Key}` | {string.Join(", ", g.Select(x => x.r.Type.Sym.Name + (x.r.Display.Contains("..cctor") ? " (static)" : " (fields)")).Distinct().Take(8))} |\n");
        sb.Append("\n## Stubs (value chosen in the config)\n\n");
        foreach (var g in reports.SelectMany(r => r.Stubs).Distinct().OrderBy(x => x, StringComparer.Ordinal)) sb.Append($"- `{g}`\n");
        return sb.ToString();
    }
}
