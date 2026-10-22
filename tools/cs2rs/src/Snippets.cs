// Snippet tests: every tests\*.cs file is (1) compiled and RUN with the .NET runtime this tool runs on, to get
// the expected output, and (2) translated to Rust into the crate's `snip` module together with a #[test]
// that runs the translation and compares. So the translator is checked against real C# behaviour, not
// against hand-written expectations.
//
// Where .NET 8 and the game's runtime (.NET Framework 4, x86) differ, the snippet states the Framework
// result in a comment `// framework-expect <Class>: <text>` and that text is used instead (reported as such).
using System.Diagnostics;
using System.Reflection;
using System.Runtime.Loader;
using System.Text;
using System.Text.RegularExpressions;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;

namespace Cs2Rs;

public static class Snippets
{
    public static int Run(Config cfg, string dir, string outDir)
    {
        if (outDir == null) { Console.Error.WriteLine("--snippets needs --out"); return 2; }
        var sw = Stopwatch.StartNew();
        var files = Directory.GetFiles(dir, "*.cs").OrderBy(f => f, StringComparer.Ordinal).ToList();
        var po = new CSharpParseOptions(LanguageVersion.CSharp7_3);
        var trees = files.Select(f => CSharpSyntaxTree.ParseText(File.ReadAllText(f), po, f, new UTF8Encoding(false))).ToList();

        // ---- 1. expected output from the real runtime
        var tpa = ((string)AppContext.GetData("TRUSTED_PLATFORM_ASSEMBLIES")).Split(Path.PathSeparator)
            .Where(p => { var n = Path.GetFileName(p); return n.StartsWith("System.") || n == "mscorlib.dll" || n == "netstandard.dll"; })
            .Select(p => (MetadataReference)MetadataReference.CreateFromFile(p)).ToList();
        var hostComp = CSharpCompilation.Create("snippets_host", trees, tpa, new CSharpCompilationOptions(OutputKind.DynamicallyLinkedLibrary, optimizationLevel: OptimizationLevel.Release, allowUnsafe: true));
        using var ms = new MemoryStream();
        var emit = hostComp.Emit(ms);
        if (!emit.Success)
        {
            foreach (var d in emit.Diagnostics.Where(d => d.Severity == DiagnosticSeverity.Error)) Console.Error.WriteLine(d.ToString());
            return 1;
        }
        ms.Position = 0;
        var asm = new AssemblyLoadContext("snippets", true).LoadFromStream(ms);
        var overrides = new Dictionary<string, string>();
        foreach (var f in files)
            foreach (Match m in Regex.Matches(File.ReadAllText(f), @"^\s*//\s*framework-expect\s+(\w+)\s*:\s?(.*?)\r?$", RegexOptions.Multiline))
                overrides[m.Groups[1].Value] = m.Groups[2].Value;
        var cases = new List<(string cls, string expected, string source)>();
        foreach (var t in asm.GetTypes().OrderBy(t => t.FullName, StringComparer.Ordinal))
        {
            var run = t.GetMethod("Run", BindingFlags.Public | BindingFlags.Static, Type.EmptyTypes);
            if (run == null || run.ReturnType != typeof(string) || t.IsNested) continue;
            string actual;
            try { actual = (string)run.Invoke(null, null); }
            catch (TargetInvocationException e) { Console.Error.WriteLine($"snippet {t.Name} threw on .NET: {e.InnerException}"); return 1; }
            if (overrides.TryGetValue(t.Name, out var ov))
            {
                cases.Add((t.Name, ov, actual == ov ? "run on .NET 8 (same as the stated .NET Framework result)" : "stated .NET Framework result (differs from .NET 8: " + actual + ")"));
            }
            else cases.Add((t.Name, actual, "run on .NET 8"));
        }

        // ---- 2. translate against the game's reference assemblies
        var comp = Program.BuildCompilation(cfg, files, out _);
        var errs = comp.GetDiagnostics().Where(d => d.Severity == DiagnosticSeverity.Error).ToList();
        if (errs.Count > 0) { foreach (var d in errs.Take(20)) Console.Error.WriteLine("framework compile: " + d); return 1; }
        cfg.GenModule = "snip";
        cfg.HostModule = "crate::snip::host_api";
        cfg.HostRoot = "crate::host";
        Program.HostOut = Path.Combine(outDir, "host_api");
        cfg.Types.Clear();
        void Add(INamedTypeSymbol s, string prefix)
        {
            if (s.TypeKind is TypeKind.Class or TypeKind.Struct or TypeKind.Interface)
            {
                var name = prefix + s.MetadataName;
                cfg.Types.Add(new TypeCfg { Type = name });
                foreach (var n in s.GetTypeMembers().OrderBy(x => x.Locations[0].SourceSpan.Start)) Add(n, name + "+");
            }
        }
        foreach (var t in comp.GlobalNamespace.GetTypeMembers().Where(t => t.Locations.Any(l => l.IsInSource)).OrderBy(t => t.Locations[0].SourceTree.FilePath, StringComparer.Ordinal).ThenBy(t => t.Locations[0].SourceSpan.Start))
            Add(t, "");

        var sb = new StringBuilder();
        sb.Append("pub mod host_api;\n");
        sb.Append("\n/// Expected strings come from running the same C# (tools/cs2rs/tests/*.cs); see tools/cs2rs/src/Snippets.cs.\n");
        sb.Append("pub const CASES: &[(&str, unsafe fn() -> crate::rt::Str, &str, &str)] = &[\n");
        foreach (var c in cases)
            sb.Append($"    ({RustStr(c.cls)}, {Names.SnakeIdent(c.cls)}::run, {RustStr(c.expected)}, {RustStr(c.source)}),\n");
        sb.Append("];\n\n");
        sb.Append("/// One test for all snippets: translated code keeps static state and must stay on one thread.\n#[test]\nfn translated_snippets_match_csharp() {\n");
        sb.Append("    let mut failed = Vec::new();\n    for (name, f, expected, source) in CASES {\n");
        sb.Append("        let got = match std::panic::catch_unwind(|| crate::rt::run(|| unsafe { f() })) {\n");
        sb.Append("            Ok(Ok(s)) => s.get().unwrap_or(\"<null>\").to_owned(),\n            Ok(Err(e)) => format!(\"<exception {e}>\"),\n            Err(_) => \"<panic>\".to_owned(),\n        };\n");
        sb.Append("        if got != *expected {\n            failed.push(format!(\"{name}\\n  expected ({source}): {expected}\\n  got:      {got}\"));\n        }\n    }\n");
        sb.Append("    assert!(failed.is_empty(), \"{} of {} snippets differ:\\n{}\", failed.len(), CASES.len(), failed.join(\"\\n\"));\n}\n");
        int rc = Program.Translate(cfg, comp, outDir, null, -1, sw, null, sb.ToString());
        Console.WriteLine($"snippets: {cases.Count} cases ({cases.Count(c => c.source.StartsWith("stated"))} with a stated .NET Framework result)");
        return rc;
    }

    static string RustStr(string s)
    {
        var b = new StringBuilder("\"");
        foreach (var c in s)
        {
            if (c == '"') b.Append("\\\""); else if (c == '\\') b.Append("\\\\"); else if (c == '\n') b.Append("\\n"); else if (c == '\r') b.Append("\\r");
            else if (c < 0x20 || c > 0x7e) b.Append("\\u{").Append(((int)c).ToString("x")).Append('}');
            else b.Append(c);
        }
        return b.Append('"').ToString();
    }
}
