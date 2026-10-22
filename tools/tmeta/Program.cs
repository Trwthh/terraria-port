// tmeta - ground truth extractor for the Terraria reference database.
//
//   tmeta meta <assembly.exe|dll> <outdir>   metadata + IL facts (what the binary really contains)
//   tmeta src  <decompile-root>   <outdir>   exact C# declarations with line spans (what the decompile contains)
//   tmeta pdblocals <assembly> <pdb> <out.tsv>  which decompiled local is which IL local slot (ILSpy's portable PDB)
//   tmeta pdbseq <assembly> <pdb> <decompile dir> <out.tsv>  the sequence points of the same PDB (IL offset -> decompiled source span)
//
// Output: one JSON array per line (*.jsonl). Rows are written in metadata / path order, so the
// output is deterministic for the same input.
using System.Collections.Immutable;
using System.Reflection;
using System.Reflection.Emit;
using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;

static class Program
{
    static int Main(string[] a)
    {
        if (a.Length == 3 && a[0] == "meta") return Meta.Run(a[1], a[2]);
        if (a.Length == 3 && a[0] == "src")
        {
            // Roslyn's parser recurses on nested expressions; Projectile.cs needs more than the default 1 MB stack.
            int rc = 0;
            var t = new Thread(() => rc = Src.Run(a[1], a[2]), 512 * 1024 * 1024);
            t.Start(); t.Join();
            return rc;
        }
        if (a.Length == 4 && a[0] == "embedded") return Embedded.Run(a[1], a[2], a[3]);
        if (a.Length == 4 && a[0] == "pdblocals") return PdbLocals.Run(a[1], a[2], a[3]);
        if (a.Length == 5 && a[0] == "pdbseq") return PdbSeq.Run(a[1], a[2], a[3], a[4]);
        Console.Error.WriteLine("usage: tmeta meta <assembly> <outdir> | tmeta src <root> <outdir> | tmeta embedded <assembly> <outdir> <refsdir> | tmeta pdblocals <assembly> <pdb> <out.tsv> | tmeta pdbseq <assembly> <pdb> <decompile dir> <out.tsv>");
        return 2;
    }
}

sealed class Jsonl : IDisposable
{
    readonly StreamWriter w;
    public int Rows;
    public Jsonl(string path) { w = new StreamWriter(path, false, new UTF8Encoding(false)) { NewLine = "\n" }; }
    public void Row(params object[] cols) { w.WriteLine(JsonSerializer.Serialize(cols)); Rows++; }
    public void Dispose() => w.Dispose();
}

static class Names
{
    // C# keyword for a primitive, so metadata signatures and source signatures compare equal.
    public static string Prim(PrimitiveTypeCode c) => c switch
    {
        PrimitiveTypeCode.Boolean => "bool", PrimitiveTypeCode.Byte => "byte", PrimitiveTypeCode.SByte => "sbyte",
        PrimitiveTypeCode.Char => "char", PrimitiveTypeCode.Int16 => "short", PrimitiveTypeCode.UInt16 => "ushort",
        PrimitiveTypeCode.Int32 => "int", PrimitiveTypeCode.UInt32 => "uint", PrimitiveTypeCode.Int64 => "long",
        PrimitiveTypeCode.UInt64 => "ulong", PrimitiveTypeCode.Single => "float", PrimitiveTypeCode.Double => "double",
        PrimitiveTypeCode.String => "string", PrimitiveTypeCode.Object => "object", PrimitiveTypeCode.Void => "void",
        PrimitiveTypeCode.IntPtr => "IntPtr", PrimitiveTypeCode.UIntPtr => "UIntPtr",
        PrimitiveTypeCode.TypedReference => "TypedReference", _ => c.ToString()
    };
    public static string StripArity(string n) { int i = n.IndexOf('`'); return i < 0 ? n : n.Substring(0, i); }
    static readonly Dictionary<string, string> Sys = new()
    {
        ["Boolean"] = "bool", ["Byte"] = "byte", ["SByte"] = "sbyte", ["Char"] = "char", ["Int16"] = "short",
        ["UInt16"] = "ushort", ["Int32"] = "int", ["UInt32"] = "uint", ["Int64"] = "long", ["UInt64"] = "ulong",
        ["Single"] = "float", ["Double"] = "double", ["String"] = "string", ["Object"] = "object", ["Decimal"] = "decimal",
    };
    public static string Alias(string simple) => Sys.TryGetValue(simple, out var k) ? k : simple;
}

// ---------------------------------------------------------------------------------------------
// meta: assembly -> types, methods, fields, properties, events, call refs, field refs, strings
// ---------------------------------------------------------------------------------------------
static class Meta
{
    sealed class Ctx { public ImmutableArray<string> TypeParams = ImmutableArray<string>.Empty, MethodParams = ImmutableArray<string>.Empty; }

    // Short C#-like type names: simple name only, generic arguments kept, by-reference shown as `ref T`.
    sealed class Sig : ISignatureTypeProvider<string, Ctx>
    {
        readonly MetadataReader r;
        public Sig(MetadataReader r) { this.r = r; }
        public string GetPrimitiveType(PrimitiveTypeCode c) => Names.Prim(c);
        public string GetTypeFromDefinition(MetadataReader rd, TypeDefinitionHandle h, byte k) => Names.Alias(Names.StripArity(rd.GetString(rd.GetTypeDefinition(h).Name)));
        public string GetTypeFromReference(MetadataReader rd, TypeReferenceHandle h, byte k)
        {
            var t = rd.GetTypeReference(h);
            string n = Names.StripArity(rd.GetString(t.Name));
            return rd.GetString(t.Namespace) == "System" ? Names.Alias(n) : n;
        }
        public string GetTypeFromSpecification(MetadataReader rd, Ctx c, TypeSpecificationHandle h, byte k) => rd.GetTypeSpecification(h).DecodeSignature(this, c);
        public string GetSZArrayType(string e) => e + "[]";
        public string GetArrayType(string e, ArrayShape s) => e + "[" + new string(',', s.Rank - 1) + "]";
        public string GetByReferenceType(string e) => "ref " + e;
        public string GetPointerType(string e) => e + "*";
        public string GetPinnedType(string e) => e;
        public string GetGenericInstantiation(string g, ImmutableArray<string> args) => g == "Nullable" && args.Length == 1 ? "Nullable<" + args[0] + ">" : g + "<" + string.Join(",", args) + ">";
        public string GetGenericTypeParameter(Ctx c, int i) => c != null && i < c.TypeParams.Length ? c.TypeParams[i] : "!" + i;
        public string GetGenericMethodParameter(Ctx c, int i) => c != null && i < c.MethodParams.Length ? c.MethodParams[i] : "!!" + i;
        public string GetModifiedType(string m, string u, bool req) => u;
        public string GetFunctionPointerType(MethodSignature<string> s) => "fnptr";
    }

    static MetadataReader md;
    static Sig sig;
    static readonly Dictionary<TypeDefinitionHandle, string> typeFull = new();

    static string TypeFullName(TypeDefinitionHandle h)
    {
        if (typeFull.TryGetValue(h, out var s)) return s;
        var t = md.GetTypeDefinition(h);
        string name = md.GetString(t.Name);
        var outer = t.GetDeclaringType();
        if (!outer.IsNil) s = TypeFullName(outer) + "+" + name;
        else { string ns = md.GetString(t.Namespace); s = ns.Length == 0 ? name : ns + "." + name; }
        return typeFull[h] = s;
    }

    static string TypeRefFullName(TypeReferenceHandle h)
    {
        var t = md.GetTypeReference(h);
        string name = md.GetString(t.Name), ns = md.GetString(t.Namespace);
        if (t.ResolutionScope.Kind == HandleKind.TypeReference) return TypeRefFullName((TypeReferenceHandle)t.ResolutionScope) + "+" + name;
        return ns.Length == 0 ? name : ns + "." + name;
    }

    static ImmutableArray<string> GenericNames(GenericParameterHandleCollection c) => c.Select(g => md.GetString(md.GetGenericParameter(g).Name)).ToImmutableArray();

    static Ctx TypeCtx(TypeDefinitionHandle h) => new Ctx { TypeParams = GenericNames(md.GetTypeDefinition(h).GetGenericParameters()) };

    // Owner type of a member reference, as (full name, definition handle or nil).
    static (string name, TypeDefinitionHandle def) Parent(EntityHandle p)
    {
        switch (p.Kind)
        {
            case HandleKind.TypeDefinition: return (TypeFullName((TypeDefinitionHandle)p), (TypeDefinitionHandle)p);
            case HandleKind.TypeReference: return (TypeRefFullName((TypeReferenceHandle)p), default);
            case HandleKind.TypeSpecification:
                {
                    // Generic instantiation or array: find the underlying definition if it is local.
                    var br = md.GetBlobReader(md.GetTypeSpecification((TypeSpecificationHandle)p).Signature);
                    var code = br.ReadSignatureTypeCode();
                    if (code == SignatureTypeCode.GenericTypeInstance)
                    {
                        br.ReadSignatureTypeCode(); // class / valuetype
                        var th = br.ReadTypeHandle();
                        if (th.Kind == HandleKind.TypeDefinition) return (TypeFullName((TypeDefinitionHandle)th), (TypeDefinitionHandle)th);
                        if (th.Kind == HandleKind.TypeReference) return (TypeRefFullName((TypeReferenceHandle)th), default);
                    }
                    return (md.GetTypeSpecification((TypeSpecificationHandle)p).DecodeSignature(sig, new Ctx()), default);
                }
            case HandleKind.MethodDefinition:
                {
                    var dt = md.GetMethodDefinition((MethodDefinitionHandle)p).GetDeclaringType();
                    return (TypeFullName(dt), dt);
                }
            case HandleKind.ModuleReference: return ("<module:" + md.GetString(md.GetModuleReference((ModuleReferenceHandle)p).Name) + ">", default);
            default: return ("?", default);
        }
    }

    static readonly OpCode[] one = new OpCode[256], two = new OpCode[256];
    static readonly bool[] oneOk = new bool[256], twoOk = new bool[256];

    static string Sha(byte[] b) => Convert.ToHexString(SHA256.HashData(b)).ToLowerInvariant();

    public static int Run(string asmPath, string outDir)
    {
        Directory.CreateDirectory(outDir);
        foreach (var f in typeof(OpCodes).GetFields(BindingFlags.Public | BindingFlags.Static))
        {
            var oc = (OpCode)f.GetValue(null);
            ushort v = unchecked((ushort)oc.Value);
            if (oc.Size == 1) { one[v & 0xff] = oc; oneOk[v & 0xff] = true; } else { two[v & 0xff] = oc; twoOk[v & 0xff] = true; }
        }
        byte[] image = File.ReadAllBytes(asmPath);
        using var pe = new PEReader(new MemoryStream(image));
        md = pe.GetMetadataReader();
        sig = new Sig(md);

        using var types = new Jsonl(Path.Combine(outDir, "types.jsonl"));
        using var methods = new Jsonl(Path.Combine(outDir, "methods.jsonl"));
        using var fields = new Jsonl(Path.Combine(outDir, "fields.jsonl"));
        using var props = new Jsonl(Path.Combine(outDir, "props.jsonl"));
        using var calls = new Jsonl(Path.Combine(outDir, "calls.jsonl"));
        using var frefs = new Jsonl(Path.Combine(outDir, "fieldrefs.jsonl"));
        using var strs = new Jsonl(Path.Combine(outDir, "strings.jsonl"));
        using var res = new Jsonl(Path.Combine(outDir, "resources.jsonl"));
        int ilErrors = 0;

        foreach (var th in md.TypeDefinitions)
        {
            var t = md.GetTypeDefinition(th);
            string full = TypeFullName(th);
            string baseName = "";
            if (!t.BaseType.IsNil) baseName = Parent(t.BaseType).name;
            var at = t.Attributes;
            string kind = (at & TypeAttributes.Interface) != 0 ? "interface"
                : baseName == "System.Enum" ? "enum"
                : baseName == "System.ValueType" ? "struct"
                : baseName == "System.MulticastDelegate" ? "delegate" : "class";
            var ifaces = t.GetInterfaceImplementations().Select(i => Parent(md.GetInterfaceImplementation(i).Interface).name).ToArray();
            types.Row(MetadataTokens.GetToken(th), full, md.GetString(t.Namespace), md.GetString(t.Name), kind, baseName,
                t.GetDeclaringType().IsNil ? 0 : MetadataTokens.GetToken(t.GetDeclaringType()),
                (int)at, t.GetGenericParameters().Count, string.Join(";", ifaces));

            var tctx = TypeCtx(th);
            foreach (var fh in t.GetFields())
            {
                var f = md.GetFieldDefinition(fh);
                string ftype;
                try { ftype = f.DecodeSignature(sig, tctx); } catch { ftype = "?"; }
                string cst = null;
                var ch = f.GetDefaultValue();
                if (!ch.IsNil)
                {
                    var c = md.GetConstant(ch);
                    var br = md.GetBlobReader(c.Value);
                    object v = c.TypeCode == ConstantTypeCode.NullReference ? null : br.ReadConstant(c.TypeCode);
                    cst = v switch { null => "null", string s => JsonSerializer.Serialize(s), bool b => b ? "true" : "false",
                        char ch2 => ((int)ch2).ToString(), IFormattable fm => fm.ToString(null, System.Globalization.CultureInfo.InvariantCulture), _ => v.ToString() };
                }
                fields.Row(MetadataTokens.GetToken(fh), MetadataTokens.GetToken(th), md.GetString(f.Name), ftype, (int)f.Attributes, cst);
            }
            foreach (var ph in t.GetProperties())
            {
                var p = md.GetPropertyDefinition(ph);
                var acc = p.GetAccessors();
                string ptype;
                try { ptype = p.DecodeSignature(sig, tctx).ReturnType; } catch { ptype = "?"; }
                props.Row("property", MetadataTokens.GetToken(th), md.GetString(p.Name), ptype,
                    acc.Getter.IsNil ? 0 : MetadataTokens.GetToken(acc.Getter), acc.Setter.IsNil ? 0 : MetadataTokens.GetToken(acc.Setter));
            }
            foreach (var eh in t.GetEvents())
            {
                var e = md.GetEventDefinition(eh);
                var acc = e.GetAccessors();
                props.Row("event", MetadataTokens.GetToken(th), md.GetString(e.Name), e.Type.IsNil ? "" : Parent(e.Type).name,
                    acc.Adder.IsNil ? 0 : MetadataTokens.GetToken(acc.Adder), acc.Remover.IsNil ? 0 : MetadataTokens.GetToken(acc.Remover));
            }
            foreach (var mh in t.GetMethods())
            {
                var m = md.GetMethodDefinition(mh);
                int tok = MetadataTokens.GetToken(mh);
                var ctx = new Ctx { TypeParams = tctx.TypeParams, MethodParams = GenericNames(m.GetGenericParameters()) };
                string ret = "?", ps = "?"; int pc = -1;
                try
                {
                    var s = m.DecodeSignature(sig, ctx);
                    ret = s.ReturnType; ps = string.Join(",", s.ParameterTypes); pc = s.ParameterTypes.Length;
                }
                catch { }
                int ilSize = 0; string ilSha = null; int maxStack = 0, locals = 0;
                if (m.RelativeVirtualAddress != 0 && (m.ImplAttributes & MethodImplAttributes.CodeTypeMask) == MethodImplAttributes.IL)
                {
                    try
                    {
                        var body = pe.GetMethodBody(m.RelativeVirtualAddress);
                        byte[] il = body.GetILBytes();
                        ilSize = il.Length; ilSha = Sha(il); maxStack = body.MaxStack;
                        if (!body.LocalSignature.IsNil)
                            locals = md.GetBlobReader(md.GetStandaloneSignature(body.LocalSignature).Signature).Length;
                        ScanIL(tok, il, calls, frefs, strs);
                    }
                    catch (Exception ex) { ilErrors++; Console.Error.WriteLine($"IL error {full}::{md.GetString(m.Name)}: {ex.Message}"); ilSize = -1; }
                }
                methods.Row(tok, MetadataTokens.GetToken(th), md.GetString(m.Name), ret, ps, pc, m.GetGenericParameters().Count,
                    (int)m.Attributes, (int)m.ImplAttributes, m.RelativeVirtualAddress, ilSize, ilSha, maxStack);
            }
        }
        foreach (var rh in md.ManifestResources)
        {
            var r = md.GetManifestResource(rh);
            int size = -1; string sha = null;
            if (r.Implementation.IsNil)
            {
                var dir = pe.PEHeaders.CorHeader.ResourcesDirectory;
                var sec = pe.GetSectionData(dir.RelativeVirtualAddress);
                var br = sec.GetReader((int)r.Offset, sec.Length - (int)r.Offset);
                size = br.ReadInt32();
                sha = Sha(br.ReadBytes(size));
            }
            res.Row(md.GetString(r.Name), size, sha);
        }
        var asm = md.GetAssemblyDefinition();
        File.WriteAllText(Path.Combine(outDir, "assembly.json"), JsonSerializer.Serialize(new Dictionary<string, object>
        {
            ["path"] = asmPath, ["sha256"] = Sha(image), ["size"] = image.Length, ["name"] = md.GetString(asm.Name),
            ["version"] = asm.Version.ToString(), ["metadata_version"] = md.MetadataVersion,
            ["references"] = md.AssemblyReferences.Select(h => { var ar = md.GetAssemblyReference(h); return md.GetString(ar.Name) + " " + ar.Version; }).ToArray(),
            ["types"] = types.Rows, ["methods"] = methods.Rows, ["fields"] = fields.Rows, ["props_events"] = props.Rows,
            ["calls"] = calls.Rows, ["fieldrefs"] = frefs.Rows, ["strings"] = strs.Rows, ["resources"] = res.Rows, ["il_errors"] = ilErrors,
        }, new JsonSerializerOptions { WriteIndented = true }));
        Console.WriteLine($"meta {Path.GetFileName(asmPath)}: types={types.Rows} methods={methods.Rows} fields={fields.Rows} props/events={props.Rows} calls={calls.Rows} fieldrefs={frefs.Rows} strings={strs.Rows} resources={res.Rows} il_errors={ilErrors}");
        return ilErrors == 0 ? 0 : 1;
    }

    // One pass over a method body: call targets, field accesses, string literals.
    static void ScanIL(int srcTok, byte[] il, Jsonl calls, Jsonl frefs, Jsonl strs)
    {
        int i = 0;
        while (i < il.Length)
        {
            int at = i;
            OpCode oc;
            byte b = il[i++];
            if (b == 0xFE) { byte b2 = il[i++]; if (!twoOk[b2]) throw new InvalidDataException($"bad opcode FE {b2:X2} at {at}"); oc = two[b2]; }
            else { if (!oneOk[b]) throw new InvalidDataException($"bad opcode {b:X2} at {at}"); oc = one[b]; }
            switch (oc.OperandType)
            {
                case OperandType.InlineNone: break;
                case OperandType.ShortInlineBrTarget: case OperandType.ShortInlineI: case OperandType.ShortInlineVar: i += 1; break;
                case OperandType.InlineVar: i += 2; break;
                case OperandType.InlineI8: case OperandType.InlineR: i += 8; break;
                case OperandType.InlineSwitch: { int n = BitConverter.ToInt32(il, i); i += 4 + 4 * n; break; }
                case OperandType.InlineString:
                    {
                        int tok = BitConverter.ToInt32(il, i); i += 4;
                        strs.Row(srcTok, at, md.GetUserString(MetadataTokens.UserStringHandle(tok & 0xFFFFFF)));
                        break;
                    }
                case OperandType.InlineMethod: case OperandType.InlineField: case OperandType.InlineTok:
                    {
                        int tok = BitConverter.ToInt32(il, i); i += 4;
                        Ref(srcTok, at, oc.Name, tok, calls, frefs);
                        break;
                    }
                default: i += 4; break; // InlineBrTarget, InlineI, InlineSig, InlineType, ShortInlineR
            }
            if (oc.OperandType == OperandType.ShortInlineR) { /* 4 bytes, handled by default */ }
        }
        if (i != il.Length) throw new InvalidDataException("IL walk overran the body");
    }

    static void Ref(int srcTok, int at, string op, int tok, Jsonl calls, Jsonl frefs)
    {
        var h = (EntityHandle)MetadataTokens.Handle(tok);
        if (h.Kind == HandleKind.MethodSpecification) { h = md.GetMethodSpecification((MethodSpecificationHandle)h).Method; }
        switch (h.Kind)
        {
            case HandleKind.MethodDefinition:
                {
                    var m = md.GetMethodDefinition((MethodDefinitionHandle)h);
                    calls.Row(srcTok, at, op, MetadataTokens.GetToken(h), TypeFullName(m.GetDeclaringType()), md.GetString(m.Name), -1);
                    break;
                }
            case HandleKind.FieldDefinition:
                {
                    var f = md.GetFieldDefinition((FieldDefinitionHandle)h);
                    frefs.Row(srcTok, at, op, MetadataTokens.GetToken(h), TypeFullName(f.GetDeclaringType()), md.GetString(f.Name));
                    break;
                }
            case HandleKind.MemberReference:
                {
                    var mr = md.GetMemberReference((MemberReferenceHandle)h);
                    var (pname, pdef) = Parent(mr.Parent);
                    string name = md.GetString(mr.Name);
                    if (mr.GetKind() == MemberReferenceKind.Field)
                    {
                        // A field of a local generic type is referenced through a MemberRef; resolve it by name.
                        int ftok = 0;
                        if (!pdef.IsNil)
                            foreach (var fh in md.GetTypeDefinition(pdef).GetFields())
                                if (md.GetString(md.GetFieldDefinition(fh).Name) == name) { ftok = MetadataTokens.GetToken(fh); break; }
                        frefs.Row(srcTok, at, op, ftok, pname, name);
                    }
                    else
                    {
                        // Local generic type: resolve to the definition when name + parameter count is unique.
                        int pc = -1, mtok = 0;
                        try { var br = md.GetBlobReader(mr.Signature); var hd = br.ReadSignatureHeader(); if (hd.IsGeneric) br.ReadCompressedInteger(); pc = br.ReadCompressedInteger(); } catch { }
                        if (!pdef.IsNil)
                        {
                            int hits = 0, cand = 0;
                            foreach (var mh in md.GetTypeDefinition(pdef).GetMethods())
                            {
                                var m = md.GetMethodDefinition(mh);
                                if (md.GetString(m.Name) != name) continue;
                                var br = md.GetBlobReader(m.Signature); var hd = br.ReadSignatureHeader(); if (hd.IsGeneric) br.ReadCompressedInteger();
                                if (br.ReadCompressedInteger() == pc) { hits++; cand = MetadataTokens.GetToken(mh); }
                            }
                            if (hits == 1) mtok = cand;
                        }
                        calls.Row(srcTok, at, op, mtok, pname, name, pc);
                    }
                    break;
                }
            default: break; // ldtoken <type>
        }
    }
}

// ---------------------------------------------------------------------------------------------
// embedded: write the .dll resources stored inside an assembly. <outdir> keeps the resource name,
// <refsdir> gets a copy under the real assembly name so a decompiler can resolve references to it.
// ---------------------------------------------------------------------------------------------
static class Embedded
{
    public static int Run(string asmPath, string outDir, string refsDir)
    {
        Directory.CreateDirectory(outDir); Directory.CreateDirectory(refsDir);
        using var pe = new PEReader(File.OpenRead(asmPath));
        var md = pe.GetMetadataReader();
        var dir = pe.PEHeaders.CorHeader.ResourcesDirectory;
        var sec = pe.GetSectionData(dir.RelativeVirtualAddress);
        int n = 0;
        foreach (var rh in md.ManifestResources)
        {
            var r = md.GetManifestResource(rh);
            string name = md.GetString(r.Name);
            if (!r.Implementation.IsNil || !name.EndsWith(".dll", StringComparison.OrdinalIgnoreCase)) continue;
            var br = sec.GetReader((int)r.Offset, sec.Length - (int)r.Offset);
            byte[] bytes = br.ReadBytes(br.ReadInt32());
            WriteIfChanged(Path.Combine(outDir, name), bytes);
            string real = null;
            try
            {
                using var inner = new PEReader(new MemoryStream(bytes));
                if (inner.HasMetadata) { var im = inner.GetMetadataReader(); real = im.GetString(im.GetAssemblyDefinition().Name); }
            }
            catch { }
            if (real != null) WriteIfChanged(Path.Combine(refsDir, real + ".dll"), bytes);
            Console.WriteLine($"{name}	{bytes.Length}	{real ?? "(native)"}");
            n++;
        }
        Console.WriteLine($"embedded: {n} libraries");
        return 0;
    }
    static void WriteIfChanged(string path, byte[] bytes)
    {
        if (File.Exists(path) && File.ReadAllBytes(path).AsSpan().SequenceEqual(bytes)) return;
        File.WriteAllBytes(path, bytes);
    }
}

// ---------------------------------------------------------------------------------------------
// src: decompiled C# -> exact declarations with line spans (Roslyn syntax trees, no semantic model)
// ---------------------------------------------------------------------------------------------
static class Src
{
    static string NormType(TypeSyntax t)
    {
        switch (t)
        {
            case null: return "";
            case PredefinedTypeSyntax p: return p.Keyword.ValueText;
            case IdentifierNameSyntax id: return Names.Alias(id.Identifier.ValueText);
            case GenericNameSyntax g: return g.Identifier.ValueText + "<" + string.Join(",", g.TypeArgumentList.Arguments.Select(NormType)) + ">";
            case QualifiedNameSyntax q: return NormType(q.Right);
            case AliasQualifiedNameSyntax aq: return NormType(aq.Name);
            case ArrayTypeSyntax arr:
                return NormType(arr.ElementType) + string.Concat(arr.RankSpecifiers.Select(r => "[" + new string(',', r.Rank - 1) + "]"));
            case NullableTypeSyntax n: return "Nullable<" + NormType(n.ElementType) + ">";
            case PointerTypeSyntax pt: return NormType(pt.ElementType) + "*";
            case RefTypeSyntax rt: return NormType(rt.Type);
            case TupleTypeSyntax tu: return "ValueTuple<" + string.Join(",", tu.Elements.Select(e => NormType(e.Type))) + ">";
            default: return t.ToString().Replace(" ", "");
        }
    }

    static readonly Dictionary<string, string> OpNames = new()
    {
        ["+"] = "op_Addition", ["-"] = "op_Subtraction", ["*"] = "op_Multiply", ["/"] = "op_Division", ["%"] = "op_Modulus",
        ["=="] = "op_Equality", ["!="] = "op_Inequality", ["<"] = "op_LessThan", [">"] = "op_GreaterThan",
        ["<="] = "op_LessThanOrEqual", [">="] = "op_GreaterThanOrEqual", ["&"] = "op_BitwiseAnd", ["|"] = "op_BitwiseOr",
        ["^"] = "op_ExclusiveOr", ["<<"] = "op_LeftShift", [">>"] = "op_RightShift", ["!"] = "op_LogicalNot",
        ["~"] = "op_OnesComplement", ["++"] = "op_Increment", ["--"] = "op_Decrement", ["true"] = "op_True", ["false"] = "op_False",
    };
    static readonly Dictionary<string, string> UnaryOps = new() { ["+"] = "op_UnaryPlus", ["-"] = "op_UnaryNegation" };

    // ref / out / in all compile to a by-reference parameter
    static string ParamType(ParameterSyntax p) =>
        (p.Modifiers.Any(m => m.IsKind(SyntaxKind.RefKeyword) || m.IsKind(SyntaxKind.OutKeyword) || m.IsKind(SyntaxKind.InKeyword)) ? "ref " : "") + NormType(p.Type);
    static string Params(BaseParameterListSyntax pl) => pl == null ? "" : string.Join(",", pl.Parameters.Select(ParamType));
    static int Count(BaseParameterListSyntax pl) => pl == null ? 0 : pl.Parameters.Count;

    static string Container(SyntaxNode n)
    {
        var parts = new List<string>();
        string ns = "";
        for (var p = n.Parent; p != null; p = p.Parent)
        {
            if (p is BaseTypeDeclarationSyntax td)
            {
                int ar = td is TypeDeclarationSyntax tds && tds.TypeParameterList != null ? tds.TypeParameterList.Parameters.Count : 0;
                parts.Insert(0, td.Identifier.ValueText + (ar > 0 ? "`" + ar : ""));
            }
            else if (p is BaseNamespaceDeclarationSyntax nd) ns = ns.Length == 0 ? nd.Name.ToString() : nd.Name + "." + ns;
        }
        string inner = string.Join("+", parts);
        return ns.Length == 0 ? inner : inner.Length == 0 ? ns : ns + "." + inner;
    }

    public static int Run(string root, string outDir)
    {
        Directory.CreateDirectory(outDir);
        root = Path.GetFullPath(root);
        var files = Directory.EnumerateFiles(root, "*.cs", SearchOption.AllDirectories)
            .Select(f => Path.GetRelativePath(root, f).Replace('\\', '/')).OrderBy(f => f, StringComparer.Ordinal).ToList();
        using var fo = new Jsonl(Path.Combine(outDir, "src_files.jsonl"));
        using var d = new Jsonl(Path.Combine(outDir, "src_decls.jsonl"));
        var opt = new CSharpParseOptions(LanguageVersion.Latest);
        int errFiles = 0;
        foreach (var rel in files)
        {
            string text = File.ReadAllText(Path.Combine(root, rel));
            var tree = CSharpSyntaxTree.ParseText(text, opt);
            var diags = tree.GetDiagnostics().Where(x => x.Severity == DiagnosticSeverity.Error).ToList();
            if (diags.Count > 0) errFiles++;
            fo.Row(rel, diags.Count, diags.Count > 0 ? diags[0].ToString() : null);
            var rootNode = tree.GetRoot();

            void Emit(SyntaxNode n, string kind, string container, string name, int pc, string ps, bool body, int arity = 0, string extra = null)
            {
                var span = n.GetLocation().GetLineSpan();
                d.Row(rel, kind, container, name, pc, ps, span.StartLinePosition.Line + 1, span.EndLinePosition.Line + 1, body ? 1 : 0, arity, extra);
            }

            foreach (var n in rootNode.DescendantNodes())
            {
                switch (n)
                {
                    case TypeDeclarationSyntax td:
                        {
                            string k = td is ClassDeclarationSyntax ? "class" : td is StructDeclarationSyntax ? "struct" : td is InterfaceDeclarationSyntax ? "interface" : "type";
                            int ar = td.TypeParameterList?.Parameters.Count ?? 0;
                            Emit(td, k, Container(td), td.Identifier.ValueText + (ar > 0 ? "`" + ar : ""), 0, "", true, ar, td.BaseList?.ToString());
                            break;
                        }
                    case EnumDeclarationSyntax ed:
                        Emit(ed, "enum", Container(ed), ed.Identifier.ValueText, 0, "", true, 0, ed.BaseList?.ToString());
                        break;
                    case DelegateDeclarationSyntax dd:
                        {
                            int ar = dd.TypeParameterList?.Parameters.Count ?? 0;
                            Emit(dd, "delegate", Container(dd), dd.Identifier.ValueText + (ar > 0 ? "`" + ar : ""), Count(dd.ParameterList), Params(dd.ParameterList), false, ar, NormType(dd.ReturnType));
                            break;
                        }
                    case EnumMemberDeclarationSyntax em:
                        Emit(em, "enum_member", Container(em), em.Identifier.ValueText, 0, "", false, 0, em.EqualsValue?.Value.ToString());
                        break;
                    case MethodDeclarationSyntax m:
                        {
                            string name = (m.ExplicitInterfaceSpecifier != null ? m.ExplicitInterfaceSpecifier.Name + "." : "") + m.Identifier.ValueText;
                            Emit(m, "method", Container(m), name, Count(m.ParameterList), Params(m.ParameterList), m.Body != null || m.ExpressionBody != null,
                                m.TypeParameterList?.Parameters.Count ?? 0, NormType(m.ReturnType));
                            break;
                        }
                    case ConstructorDeclarationSyntax c:
                        Emit(c, "ctor", Container(c), c.Modifiers.Any(SyntaxKind.StaticKeyword) ? ".cctor" : ".ctor", Count(c.ParameterList), Params(c.ParameterList), true);
                        break;
                    case DestructorDeclarationSyntax ds:
                        Emit(ds, "dtor", Container(ds), "Finalize", 0, "", true);
                        break;
                    case OperatorDeclarationSyntax o:
                        {
                            string tokText = o.OperatorToken.ValueText;
                            int pc = Count(o.ParameterList);
                            string name = pc == 1 && UnaryOps.TryGetValue(tokText, out var u) ? u : OpNames.TryGetValue(tokText, out var b) ? b : "op_" + tokText;
                            Emit(o, "operator", Container(o), name, pc, Params(o.ParameterList), true);
                            break;
                        }
                    case ConversionOperatorDeclarationSyntax co:
                        Emit(co, "operator", Container(co), co.ImplicitOrExplicitKeyword.IsKind(SyntaxKind.ImplicitKeyword) ? "op_Implicit" : "op_Explicit",
                            Count(co.ParameterList), Params(co.ParameterList), true, 0, NormType(co.Type));
                        break;
                    case PropertyDeclarationSyntax p:
                        {
                            string name = (p.ExplicitInterfaceSpecifier != null ? p.ExplicitInterfaceSpecifier.Name + "." : "") + p.Identifier.ValueText;
                            string cont = Container(p);
                            Emit(p, "property", cont, name, 0, "", true, 0, NormType(p.Type));
                            string prefix = p.ExplicitInterfaceSpecifier != null ? p.ExplicitInterfaceSpecifier.Name + "." : "";
                            if (p.AccessorList != null)
                                foreach (var acc in p.AccessorList.Accessors)
                                    Emit(acc, "accessor", cont, prefix + acc.Keyword.ValueText + "_" + p.Identifier.ValueText, acc.Keyword.ValueText == "get" ? 0 : 1,
                                        acc.Keyword.ValueText == "get" ? "" : NormType(p.Type), acc.Body != null || acc.ExpressionBody != null, 0,
                                        acc.Keyword.ValueText == "get" ? NormType(p.Type) : "void");
                            else if (p.ExpressionBody != null)
                                Emit(p, "accessor", cont, prefix + "get_" + p.Identifier.ValueText, 0, "", true, 0, NormType(p.Type));
                            break;
                        }
                    case IndexerDeclarationSyntax ix:
                        {
                            string cont = Container(ix);
                            string prefix = ix.ExplicitInterfaceSpecifier != null ? ix.ExplicitInterfaceSpecifier.Name + "." : "";
                            Emit(ix, "property", cont, prefix + "Item", Count(ix.ParameterList), Params(ix.ParameterList), true, 0, NormType(ix.Type));
                            string ps = Params(ix.ParameterList); int pc = Count(ix.ParameterList);
                            if (ix.AccessorList != null)
                                foreach (var acc in ix.AccessorList.Accessors)
                                {
                                    bool get = acc.Keyword.ValueText == "get";
                                    Emit(acc, "accessor", cont, prefix + acc.Keyword.ValueText + "_Item", get ? pc : pc + 1, get ? ps : ps + "," + NormType(ix.Type), acc.Body != null || acc.ExpressionBody != null, 0, get ? NormType(ix.Type) : "void");
                                }
                            else if (ix.ExpressionBody != null)
                                Emit(ix, "accessor", cont, prefix + "get_Item", pc, ps, true, 0, NormType(ix.Type));
                            break;
                        }
                    case EventDeclarationSyntax ev:
                        {
                            string cont = Container(ev);
                            string prefix = ev.ExplicitInterfaceSpecifier != null ? ev.ExplicitInterfaceSpecifier.Name + "." : "";
                            Emit(ev, "event", cont, prefix + ev.Identifier.ValueText, 0, "", true, 0, NormType(ev.Type));
                            if (ev.AccessorList != null)
                                foreach (var acc in ev.AccessorList.Accessors)
                                    Emit(acc, "accessor", cont, prefix + acc.Keyword.ValueText + "_" + ev.Identifier.ValueText, 1, NormType(ev.Type), true);
                            break;
                        }
                    case EventFieldDeclarationSyntax ef:
                        foreach (var v in ef.Declaration.Variables)
                        {
                            // Field-like event: the compiler writes add_/remove_ itself; both map to this line.
                            string cont = Container(ef);
                            Emit(ef, "event", cont, v.Identifier.ValueText, 0, "", false, 0, NormType(ef.Declaration.Type));
                            Emit(ef, "accessor", cont, "add_" + v.Identifier.ValueText, 1, NormType(ef.Declaration.Type), false);
                            Emit(ef, "accessor", cont, "remove_" + v.Identifier.ValueText, 1, NormType(ef.Declaration.Type), false);
                        }
                        break;
                    case FieldDeclarationSyntax f:
                        foreach (var v in f.Declaration.Variables)
                            Emit(f, "field", Container(f), v.Identifier.ValueText, 0, "", v.Initializer != null, 0, NormType(f.Declaration.Type));
                        break;
                }
            }
        }
        Console.WriteLine($"src {root}: files={fo.Rows} decls={d.Rows} files_with_parse_errors={errFiles}");
        return 0;
    }
}

// ---------------------------------------------------------------------------------------------
// pdblocals: the IL local slot of each variable of the decompiled code, from the portable PDB that ILSpy writes for
// the same decompilation (`ilspycmd -genpdb`, same version and language level as decompile\). A decompiled variable
// with no row is not an IL local: ILSpy made it from a value the IL keeps on the evaluation stack.
// Rows: type full name (nested with '+') | method name | FNV-1a 64 of the IL body with metadata tokens zeroed (hex) |
// IL slot | variable name.
// ---------------------------------------------------------------------------------------------
static class PdbLocals
{
    /// The IL bytes with every metadata token operand set to zero: the same method compiled into Terraria.exe and
    /// TerrariaServer.exe differs only in its tokens. Opcode sizes as tools/refdump_worldgen/jit_calls.py il_calls.
    public static byte[] Normalized(byte[] il)
    {
        var o = (byte[])il.Clone();
        var one = new HashSet<int> { 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x1F, 0xDE };
        for (int k = 0x2B; k < 0x38; k++) one.Add(k);
        var four = new HashSet<int> { 0x20, 0x22, 0x28, 0x29, 0x27, 0x6F, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x79, 0x8C, 0x8D, 0x8F, 0xA3, 0xA4, 0xA5, 0xC2, 0xC6, 0xD0, 0xDD };
        for (int k = 0x38; k < 0x45; k++) four.Add(k);
        for (int k = 0x7B; k < 0x82; k++) four.Add(k);
        var tokens = new HashSet<int> { 0x27, 0x28, 0x29, 0x6F, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x79, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F, 0x80, 0x81, 0x8C, 0x8D, 0x8F, 0xA3, 0xA4, 0xA5, 0xC2, 0xC6, 0xD0 };
        int i = 0;
        while (i < o.Length)
        {
            int op = o[i];
            if (op == 0xFE)
            {
                int op2 = o[i + 1];
                int size = op2 is 0x06 or 0x07 or 0x15 or 0x16 or 0x1C ? 4 : op2 >= 0x09 && op2 <= 0x0E ? 2 : op2 is 0x12 or 0x19 ? 1 : 0;
                if (op2 is 0x06 or 0x07 or 0x15 or 0x16 or 0x1C) for (int k = 0; k < 4; k++) o[i + 2 + k] = 0;
                i += 2 + size;
                continue;
            }
            if (op == 0x45) { int n = BitConverter.ToInt32(o, i + 1); i += 1 + 4 + 4 * n; continue; }
            if (tokens.Contains(op)) for (int k = 0; k < 4; k++) o[i + 1 + k] = 0;
            i += 1 + (one.Contains(op) ? 1 : four.Contains(op) ? 4 : op is 0x21 or 0x23 ? 8 : 0);
        }
        return o;
    }

    public static int Run(string asmPath, string pdbPath, string outPath)
    {
        using var pe = new PEReader(File.OpenRead(asmPath));
        var md = pe.GetMetadataReader();
        using var pdbProvider = MetadataReaderProvider.FromPortablePdbStream(File.OpenRead(pdbPath));
        var pdb = pdbProvider.GetMetadataReader();
        var typeName = new Dictionary<TypeDefinitionHandle, string>();
        string Full(TypeDefinitionHandle h)
        {
            if (typeName.TryGetValue(h, out var n)) return n;
            var t = md.GetTypeDefinition(h);
            var name = md.GetString(t.Name);
            var outer = t.GetDeclaringType();
            n = !outer.IsNil ? Full(outer) + "+" + name : (md.GetString(t.Namespace) is { Length: > 0 } ns ? ns + "." + name : name);
            return typeName[h] = n;
        }
        int rows = 0, methods = 0;
        using var w = new StreamWriter(outPath, false, new UTF8Encoding(false)) { NewLine = "\n" };
        foreach (var th in md.TypeDefinitions)
        {
            var t = md.GetTypeDefinition(th);
            foreach (var mh in t.GetMethods())
            {
                var m = md.GetMethodDefinition(mh);
                if (m.RelativeVirtualAddress == 0) continue;
                var body = pe.GetMethodBody(m.RelativeVirtualAddress);
                var il = body.GetILBytes();
                ulong h = 14695981039346656037UL;
                foreach (var b in Normalized(il)) { h ^= b; h *= 1099511628211UL; }
                var names = new SortedDictionary<int, SortedSet<string>>();
                var mdh = MetadataTokens.MethodDebugInformationHandle(MetadataTokens.GetRowNumber(mh));
                foreach (var sh in pdb.GetLocalScopes(mh))
                {
                    var scope = pdb.GetLocalScope(sh);
                    foreach (var vh in scope.GetLocalVariables())
                    {
                        var v = pdb.GetLocalVariable(vh);
                        if (!names.TryGetValue(v.Index, out var set)) names[v.Index] = set = new SortedSet<string>(StringComparer.Ordinal);
                        set.Add(pdb.GetString(v.Name));
                    }
                }
                if (names.Count == 0) continue;
                methods++;
                foreach (var kv in names)
                    foreach (var n in kv.Value) { w.WriteLine($"{Full(th)}	{md.GetString(m.Name)}	{h:x16}	{kv.Key}	{n}"); rows++; }
            }
        }
        Console.WriteLine($"pdblocals: {rows} variables of {methods} methods");
        return 0;
    }
}


// ---------------------------------------------------------------------------------------------
// pdbseq: the sequence points of ILSpy's portable PDB (`ilspycmd -genpdb`): which decompiled statement each IL range
// belongs to, placed in the decompilation the translator reads (decompile\Terraria, decompile\ReLogic). The PDB was
// written for a decompilation with one folder per namespace (`Terraria.GameContent\X.cs`) and another file header, so
// each document is found by namespace path and file name, and its line shift is the one that puts the most sequence
// points on the start of a syntax node of that file. Input of tools/cs2rs/gen_jit_refcalls.py (jit_refcall_sites.tsv:
// the call sites of a callee the JIT compiled as a call instruction at some of its sites only).
// Rows: type full name (nested with '+') | method name | FNV-1a 64 of the IL body with metadata tokens zeroed (hex) |
// IL offset | file (relative to the decompile folder, '/') | start line | start column | end line | end column (1-based,
// end exclusive, in that file) | 1 when the start is the start of a syntax node there, else 0. Hidden sequence points
// and documents without a decompiled file are left out.
// ---------------------------------------------------------------------------------------------
static class PdbSeq
{
    public static int Run(string asmPath, string pdbPath, string decompileDir, string outPath)
    {
        int rc = 0;
        var t = new Thread(() => rc = RunInner(asmPath, pdbPath, decompileDir, outPath), 512 * 1024 * 1024);
        t.Start(); t.Join();
        return rc;
    }

    static int RunInner(string asmPath, string pdbPath, string decompileDir, string outPath)
    {
        using var pe = new PEReader(File.OpenRead(asmPath));
        var md = pe.GetMetadataReader();
        using var pdbProvider = MetadataReaderProvider.FromPortablePdbStream(File.OpenRead(pdbPath));
        var pdb = pdbProvider.GetMetadataReader();
        var typeName = new Dictionary<TypeDefinitionHandle, string>();
        string Full(TypeDefinitionHandle h)
        {
            if (typeName.TryGetValue(h, out var n)) return n;
            var t = md.GetTypeDefinition(h);
            var name = md.GetString(t.Name);
            var outer = t.GetDeclaringType();
            n = !outer.IsNil ? Full(outer) + "+" + name : (md.GetString(t.Namespace) is { Length: > 0 } ns ? ns + "." + name : name);
            return typeName[h] = n;
        }
        // decompiled files by the PDB's document key: `<namespace>\<file>` ('-' for the global namespace)
        string root = Path.GetFullPath(decompileDir);
        var byKey = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (var top in Directory.GetDirectories(root))
            foreach (var f in Directory.GetFiles(top, "*.cs", SearchOption.AllDirectories))
            {
                var rel = Path.GetRelativePath(top, f);
                var dir = Path.GetDirectoryName(rel);
                string key = (string.IsNullOrEmpty(dir) ? "-" : dir.Replace(Path.DirectorySeparatorChar, '.')) + "\\" + Path.GetFileName(f);
                byKey.TryAdd(key, f);
            }
        // all sequence points first, grouped by document
        var rowsOf = new Dictionary<string, List<(string type, string meth, string hash, int off, int sl, int sc, int el, int ec)>>();
        foreach (var th in md.TypeDefinitions)
        {
            var t = md.GetTypeDefinition(th);
            foreach (var mh in t.GetMethods())
            {
                var m = md.GetMethodDefinition(mh);
                if (m.RelativeVirtualAddress == 0) continue;
                var dbg = pdb.GetMethodDebugInformation(MetadataTokens.MethodDebugInformationHandle(MetadataTokens.GetRowNumber(mh)));
                if (dbg.SequencePointsBlob.IsNil) continue;
                var il = pe.GetMethodBody(m.RelativeVirtualAddress).GetILBytes();
                ulong h = 14695981039346656037UL;
                foreach (var b in PdbLocals.Normalized(il)) { h ^= b; h *= 1099511628211UL; }
                foreach (var sp in dbg.GetSequencePoints())
                {
                    if (sp.IsHidden) continue;
                    string doc = pdb.GetString(pdb.GetDocument(sp.Document).Name);
                    if (!rowsOf.TryGetValue(doc, out var list)) rowsOf[doc] = list = new();
                    list.Add((Full(th), md.GetString(m.Name), h.ToString("x16"), sp.Offset, sp.StartLine, sp.StartColumn, sp.EndLine, sp.EndColumn));
                }
            }
        }
        int rows = 0, exact = 0, docs = 0, missing = 0;
        var outLines = new List<string>();
        foreach (var (doc, list) in rowsOf.OrderBy(k => k.Key, StringComparer.Ordinal))
        {
            if (!byKey.TryGetValue(doc, out var file)) { missing++; continue; }
            docs++;
            var tree = CSharpSyntaxTree.ParseText(File.ReadAllText(file), path: file);
            var text = tree.GetText();
            var starts = new HashSet<(int, int)>();
            foreach (var node in tree.GetRoot().DescendantNodes())
            {
                var lp = text.Lines.GetLinePosition(node.SpanStart);
                starts.Add((lp.Line + 1, lp.Character + 1));
            }
            int best = 0, bestHits = -1;
            for (int d = -8; d <= 8; d++)
            {
                int hits = list.Count(r => starts.Contains((r.sl - d, r.sc)));
                if (hits > bestHits) { bestHits = hits; best = d; }
            }
            string rel = Path.GetRelativePath(Path.GetDirectoryName(root), file).Replace(Path.DirectorySeparatorChar, '/');
            foreach (var r in list)
            {
                bool hit = starts.Contains((r.sl - best, r.sc));
                if (hit) exact++;
                outLines.Add($"{r.type}\t{r.meth}\t{r.hash}\t{r.off}\t{rel}\t{r.sl - best}\t{r.sc}\t{r.el - best}\t{r.ec}\t{(hit ? 1 : 0)}");
                rows++;
            }
        }
        File.WriteAllText(outPath, string.Concat(outLines.Select(l => l + "\n")), new UTF8Encoding(false));
        Console.WriteLine($"pdbseq: {rows} sequence points in {docs} decompiled files ({exact} on the start of a syntax node); {missing} documents without a decompiled file");
        return 0;
    }
}
