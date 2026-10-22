// EntitiesJit: the entity reference run (tools\refdump_entities\Entities.cs, compiled in unchanged with
// tools\refdump_ticks\Common.cs, build_refjit.cmd), then the machine code of every game method that ran in that process
// (JitDump.cs DumpRanGame).
//
//   RefJitEntities.exe <jit outdir> npc|proj|item|citem <data outdir> [ticks] [max batches]
//
// Same statics as RefDumpEntities.exe's Main (the scratch folder is named after the data folder, as there), then its
// EntRun. Proof that it is the run that wrote refdata\entities: the data folder against refdata\entities
// (refjit_compare.py; the timing file is the only one that may differ).
using System;
using System.IO;

public static partial class WG
{
    public static int EntitiesJitMain(string[] args)
    {
        if (args.Length < 3) { Console.Error.WriteLine("usage: RefJitEntities.exe <jit outdir> npc|proj|item|citem <data outdir> [ticks] [max batches]"); return 2; }
        string jitDir = Path.GetFullPath(args[0]);
        Root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(Root, "work", "refserver");
        string kind = args[1];
        EOut = Dir(Path.GetFullPath(args[2]));
        if (EOut.IndexOf("\refdata", StringComparison.OrdinalIgnoreCase) >= 0) { Console.Error.WriteLine("refusing to write below refdata"); return 2; }
        int ticks = args.Length > 3 ? int.Parse(args[3]) : 120;
        int maxBatches = args.Length > 4 ? int.Parse(args[4]) : int.MaxValue;
        string scratch = Dir(Path.Combine(Root, "work", "entities-scratch", kind + "_" + new DirectoryInfo(EOut).Name));
        SaveDir = Dir(Path.Combine(scratch, "save"));
        Dir(jitDir);
        Watchdog(kind == "citem" ? 9000 : 2400);
        int rc = 0;
        try { EntRun(kind, ticks, scratch, maxBatches); Console.Out.Flush(); DumpRanGame(jitDir); }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }
}

static class EntitiesJitProgram
{
    [STAThread]
    static int Main(string[] args) { return WG.EntitiesJitMain(args); }
}
