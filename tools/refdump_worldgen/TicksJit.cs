// TicksJit: the server-tick reference run (tools\refdump_ticks\Ticks.cs, compiled in unchanged with its Common.cs,
// build_refjit.cmd), then the machine code of every game method that ran in that process (JitDump.cs DumpRanGame).
//
//   RefJitTicks.exe <jit outdir> world|player <data outdir> [ticks]
//
// Same statics as RefDumpTicks.exe's Main (the scratch folder is named after the data folder, as there), then its Run.
// Proof that it is the run that wrote refdata\ticks: the data folder against refdata\ticks (refjit_compare.py; the
// timing file is the only one that may differ).
using System;
using System.IO;

public static partial class WG
{
    public static int TicksJitMain(string[] args)
    {
        if (args.Length < 3) { Console.Error.WriteLine("usage: RefJitTicks.exe <jit outdir> world|player <data outdir> [ticks]"); return 2; }
        string jitDir = Path.GetFullPath(args[0]);
        Root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(Root, "work", "refserver");
        string mode = args[1];
        TOut = Dir(Path.GetFullPath(args[2]));
        if (TOut.IndexOf("\\refdata", StringComparison.OrdinalIgnoreCase) >= 0) { Console.Error.WriteLine("refusing to write below refdata"); return 2; }
        int ticks = args.Length > 3 ? int.Parse(args[3]) : (mode == "world" ? 3600 : 600);
        string scratch = Dir(Path.Combine(Root, "work", "ticks-scratch", mode + "_" + new DirectoryInfo(TOut).Name));
        SaveDir = Dir(Path.Combine(scratch, "save"));
        Dir(jitDir);
        Watchdog(1500);
        int rc = 0;
        try { Run(mode, ticks, scratch); Console.Out.Flush(); DumpRanGame(jitDir); }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }
}

static class TicksJitProgram
{
    [STAThread]
    static int Main(string[] args) { return WG.TicksJitMain(args); }
}
