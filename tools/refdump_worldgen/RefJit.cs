// RefJit: a reference harness run, then the machine code of every game method that ran in THAT process.
//
//   RefJitP1.exe     <jit outdir> p1     <data outdir>                              (tools\refdump: refdata\*.json, MANIFEST.json)
//   RefJitP1.exe     <jit outdir> framed <world.wld> <data outdir> [scramble]       (tools\refdump RefDumpFramed: refdata\framed)
//   RefJitLiquid.exe <jit outdir> liquid <world.wld> <data outdir> <scenario>       (tools\refdump_liquid: refdata\liquid)
//   RefJitSky.exe    <jit outdir> sky    <world.wld> <data outdir>                  (tools\refdump_sky: refdata\sky)
//
// The legacy x86 JIT's inlining of game methods depends on what was compiled before (plan.md, 2026-10-13), so only the
// process that wrote a reference file shows the code that produced it. Each executable is the harness itself (its
// sources compiled in unchanged, build_refjit.cmd) with this entry point: it sets the statics the harness's Main sets
// (the data folder is the only difference; refdata is refused) and calls the harness's own Run, then JitDump.cs
// DumpRanGame reads the code of every method of TerrariaServer / ReLogic / XNA that has jitted code in the process
// (nothing is compiled for the dump). Proof that it is the same run: the data folder is byte-identical to refdata
// (refjit_compare.py). The ticks and world generation harnesses have their own wrappers (TicksJit.cs, WorldgenJit.cs,
// WorldgenXJit.cs); collision and lighting: CollisionJit.cs, LightJit.cs.
using System;
using System.IO;
using System.Reflection;

static class RefJitProgram
{
    const BindingFlags St = BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic;

    static Type H(string name)
    {
        Type t = typeof(RefJitProgram).Assembly.GetType(name, false);
        if (t == null) throw new Exception("harness " + name + " is not compiled into this executable");
        return t;
    }

    static void Set(Type t, string field, object v) { t.GetField(field, St).SetValue(null, v); }

    static void Call(Type t, string method, params object[] a)
    {
        MethodInfo m = t.GetMethod(method, St);
        try { m.Invoke(null, a); }
        catch (TargetInvocationException e) { throw e.InnerException; }
    }

    [STAThread]
    static int Main(string[] args)
    {
        if (args.Length < 3) { Console.Error.WriteLine("usage: see RefJit.cs"); return 2; }
        string root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        string jitDir = Path.GetFullPath(args[0]), kind = args[1];
        string game = Path.Combine(root, "work", "refserver"), save = Path.Combine(root, "work", "refdump-save");
        string outDir = Path.GetFullPath(kind == "p1" ? args[2] : args[3]);
        if (outDir.TrimEnd('\\').IndexOf("\\refdata", StringComparison.OrdinalIgnoreCase) >= 0) { Console.Error.WriteLine("refusing to write below refdata: " + outDir); return 2; }
        Directory.CreateDirectory(save);
        Directory.CreateDirectory(outDir);
        Directory.CreateDirectory(jitDir);
        WG.Watchdog(1500);
        int rc = 0;
        try
        {
            switch (kind)
            {
                case "p1":
                    {   // RefDump.Main
                        Type t = H("RefDump");
                        Set(t, "GameDir", game); Set(t, "OutDir", outDir); Set(t, "SaveDir", save);
                        Call(t, "Run");
                        break;
                    }
                case "framed":
                    {   // RefDump.MainFramed
                        Type t = H("RefDump");
                        Set(t, "GameDir", game); Set(t, "SaveDir", save); Set(t, "OutDir", outDir);
                        Call(t, "RunFramed", root, Path.GetFullPath(args[2]), args.Length > 4 && args[4] == "scramble");
                        break;
                    }
                case "liquid":
                    {   // RefDumpLiquid.Main
                        Type t = H("RefDumpLiquid");
                        Set(t, "GameDir", game); Set(t, "SaveDir", save); Set(t, "OutDir", outDir);
                        Call(t, "Run", root, Path.GetFullPath(args[2]), args[4]);
                        break;
                    }
                case "sky":
                    {   // RefDumpSky.Main
                        Type t = H("RefDumpSky");
                        Set(t, "GameDir", game); Set(t, "SaveDir", save); Set(t, "OutDir", outDir);
                        Call(t, "Run", root, Path.GetFullPath(args[2]));
                        break;
                    }
                default: Console.Error.WriteLine("unknown harness " + kind); return 2;
            }
            Console.Out.Flush();
            WG.DumpRanGame(jitDir);
        }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }
}
