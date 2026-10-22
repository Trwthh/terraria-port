// WorldgenJit: the world generation reference run of refdata\worldgen\ref_small.* (Gen.cs, "gen" mode of
// RefDumpWorldgen.exe; compiled in unchanged with Common.cs, X87.cs, FloatProbe.cs, build_refjit.cmd), then the
// machine code of every game method that ran in that process (JitDump.cs DumpRanGame).
//
//   RefJitWorldgen.exe <jit outdir> <data outdir> <tag> [passes|real] [task|main] [nosnap]
//
// run.cmd wrote refdata\worldgen with `gen <refdata\worldgen> main passes task`. Same statics as MainGen, then RunGen.
// Proof that it is the same run: ref_small.passes.jsonl, ref_small.columns.bin and the *.tfrm.z snapshots of the data
// folder against refdata\worldgen (refjit_compare.py).
using System;
using System.Diagnostics;
using System.IO;

public static partial class WG
{
    public static int WorldgenJitMain(string[] args)
    {
        if (args.Length < 3) { Console.Error.WriteLine("usage: RefJitWorldgen.exe <jit outdir> <data outdir> <tag> [passes|real] [task|main] [nosnap]"); return 2; }
        string jitDir = Path.GetFullPath(args[0]);
        string outdir = Path.GetFullPath(args[1]);
        if (outdir.IndexOf("\\refdata", StringComparison.OrdinalIgnoreCase) >= 0) { Console.Error.WriteLine("refusing to write below refdata"); return 2; }
        string tag = args[2];
        string mode = args.Length > 3 ? args[3] : "passes";
        string thread = args.Length > 4 ? args[4] : "task";
        DoSnap = !(args.Length > 5 && args[5] == "nosnap");
        Root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(Root, "work", "refserver");
        string scratch = Dir(Path.Combine(Root, "work", "worldgen-scratch", tag));
        SaveDir = Dir(Path.Combine(scratch, "save"));
        GenOut = Dir(outdir);
        string wldPath = Path.Combine(Dir(Path.Combine(scratch, "Worlds")), WorldNameConst + ".wld");
        if (File.Exists(wldPath)) File.Delete(wldPath);
        Dir(jitDir);
        Watchdog(1800);
        Trace.Listeners.Add(new Capture());
        int rc = 0;
        try { RunGen(tag, mode, thread, wldPath, scratch); Console.Out.Flush(); DumpRanGame(jitDir); }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }
}

static class WorldgenJitProgram
{
    [STAThread]
    static int Main(string[] args) { return WG.WorldgenJitMain(args); }
}
