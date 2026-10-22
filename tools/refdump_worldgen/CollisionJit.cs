// CollisionJit: the machine code of the collision reference run, read in that run's own process.
//
//   RefDumpCollisionJit.exe <data outdir> <jit outdir>
//
// The values in refdata\collision were written by ONE run of tools\refdump_collision (RefDumpCollision.exe). The legacy
// x86 JIT's inlining of game methods depends on what was compiled before (plan.md, 2026-10-13), so a separate jitdump
// does not show the code that produced them. This executable is RefDumpCollision itself (its three source files are
// compiled in unchanged, build_colljit.cmd) with another entry point: it performs the harness's whole sequence
// (InitGame, LoadWorld, Stamps, every family in its order, 20,000 calls each) writing the data to <data outdir> (never
// refdata), and only then reads the code the JIT made for every method of the game, ReLogic and XNA that ran in the
// process (JitDump.cs DumpRanGame, nothing compiled for the dump; before 2026-10-14 the Collision methods, two levels
// of callees and XNA Vector2 / Vector4 were read, compiling the ones that had not run).
//
// Proof that it is the same run: the <data outdir>\*.bin files must be byte-identical to refdata\collision\*.bin
// (tools\refdump_worldgen\colljit_compare.py).
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;

static class CollisionJitProgram
{
    [STAThread]
    static int Main(string[] args) { return RefDumpCollision.JitMain(args); }
}

static partial class RefDumpCollision
{
    public static int JitMain(string[] args)
    {
        if (args.Length < 2) { Console.Error.WriteLine("usage: RefDumpCollisionJit.exe <data outdir> <jit outdir>"); return 2; }
        // the same paths as RefDumpCollision.Main (bin\ is three levels below the root here as well)
        Root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(Root, "work", "refserver");
        SaveDir = Path.Combine(Root, "work", "refdump-save");
        OutDir = Path.GetFullPath(args[0]);
        string jitDir = Path.GetFullPath(args[1]);
        if (OutDir.TrimEnd('\\').EndsWith(Path.Combine("refdata", "collision"), StringComparison.OrdinalIgnoreCase)) { Console.Error.WriteLine("refusing to write refdata"); return 2; }
        Directory.CreateDirectory(SaveDir);
        Directory.CreateDirectory(OutDir);
        Directory.CreateDirectory(jitDir);
        WG.Watchdog(1500);
        int rc = 0;
        try
        {
            Run();
            // every method of the game / ReLogic / XNA that ran in this process (JitDump.cs DumpRanGame; until
            // 2026-10-14 the Collision methods, two levels of callees and XNA Vector2 / Vector4, compiling those that had not run)
            Console.Out.Flush();
            WG.DumpRanGame(jitDir);
        }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }
}
