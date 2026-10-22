// DrawJit: the draw reference run (RefDumpDraw.cs, compiled in unchanged with Detour.cs, tools\refdump_ticks\Common.cs),
// then the machine code of every game, ReLogic and XNA Framework method that ran in that process
// (tools\refdump_worldgen\JitDump.cs DumpRan; methods the harness replaced with a jump are left out).
//
//   RefJitDraw.exe <jit outdir> <data outdir> [scene ...]
//
// Proof that it is the run that wrote refdata\draw: the data folder against refdata\draw (refjit_compare.py).
using System;
using System.IO;
using System.Linq;

public static partial class WG
{
    public static int DrawJitMain(string[] args)
    {
        if (args.Length < 2) { Console.Error.WriteLine("usage: RefJitDraw.exe <jit outdir> <data outdir> [scene ...]"); return 2; }
        string jitDir = Path.GetFullPath(args[0]);
        if (Path.GetFullPath(args[1]).IndexOf("\refdata", StringComparison.OrdinalIgnoreCase) >= 0) { Console.Error.WriteLine("refusing to write below refdata"); return 2; }
        Directory.CreateDirectory(jitDir);
        AfterScenario = () =>
        {
            var ms = AllMethods("Terraria", "ReLogic", "Microsoft.Xna.Framework").Where(m => !Detour.Replaced.Contains(m)).ToList();
            DumpRan(jitDir, ms);
        };
        int rc = Main(args.Skip(1).ToArray());
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }
}

static class DrawJitProgram
{
    static int Main(string[] args) { return WG.DrawJitMain(args); }
}
