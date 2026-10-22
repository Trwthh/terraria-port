// LightJit: the machine code of the lighting reference run, read in that run's own process.
//
//   RefDumpLightJit.exe <data outdir> <jit outdir>
//
// The values in refdata\light were written by ONE run of tools\refdump_light (RefDumpLight.exe). The legacy x86 JIT's
// inlining of game methods depends on what was compiled before (plan.md, 2026-10-13), so a separate jitdump does not
// show the code that produced them. This executable is RefDumpLight itself (RefDumpLight.cs compiled in unchanged,
// build_lightjit.cmd) with another entry point: it sets the paths RefDumpLight.Main would set and calls its Run (the
// whole sequence: InitGame, LoadWorld, every area with its scramble, Lighting.Initialize, AddLight, the Scan / Blur
// cycles, GetColor; the shimmer table), writing the data to <data outdir> (never refdata), and only then reads the
// code the JIT made for every method of the game, ReLogic and XNA that ran in the process (JitDump.cs DumpRanGame,
// nothing compiled for the dump; before 2026-10-14 the lighting types, two levels of callees and XNA Vector2/3/4 /
// Color were read, compiling the ones that had not run).
//
// Proof that it is the same run: the <data outdir>\*.bin files must be byte-identical to refdata\light\*.bin
// (python tools\refdump_worldgen\colljit_compare.py <data outdir> refdata\light).
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;

static class LightJitProgram
{
    [STAThread]
    static int Main(string[] args)
    {
        if (args.Length < 2) { Console.Error.WriteLine("usage: RefDumpLightJit.exe <data outdir> <jit outdir>"); return 2; }
        const BindingFlags st = BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic;
        const BindingFlags all = BindingFlags.Instance | BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.DeclaredOnly;
        Type rl = typeof(RefDumpLight);
        // the same paths as RefDumpLight.Main (bin\ is three levels below the root here as well)
        string root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        string outDir = Path.GetFullPath(args[0]), jitDir = Path.GetFullPath(args[1]);
        if (outDir.TrimEnd(Path.DirectorySeparatorChar).EndsWith(Path.Combine("refdata", "light"), StringComparison.OrdinalIgnoreCase)) { Console.Error.WriteLine("refusing to write refdata"); return 2; }
        rl.GetField("GameDir", st).SetValue(null, Path.Combine(root, "work", "refserver"));
        rl.GetField("SaveDir", st).SetValue(null, Path.Combine(root, "work", "refdump-save"));
        rl.GetField("OutDir", st).SetValue(null, outDir);
        Directory.CreateDirectory(Path.Combine(root, "work", "refdump-save"));
        Directory.CreateDirectory(outDir);
        Directory.CreateDirectory(jitDir);
        WG.Watchdog(1500);
        int rc = 0;
        try
        {
            MethodInfo run = rl.GetMethod("Run", st);
            try { run.Invoke(null, new object[] { root, Path.Combine(root, "work", "refsave", "Worlds", "ref_small.wld") }); }
            catch (TargetInvocationException e) { throw e.InnerException; }
            // every method of the game / ReLogic / XNA that ran in this process (JitDump.cs DumpRanGame; until
            // 2026-10-14 the methods of the lighting types, their callees and XNA, compiling those that had not run)
            Console.Out.Flush();
            WG.DumpRanGame(jitDir);
        }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }
}
