// RefDumpWorldgen entry point. Modes: gen | x87 | floatprobe  (see Gen.cs, X87.cs, FloatProbe.cs)
using System;

public static partial class WG
{
    [STAThread]
    static int Main(string[] args)
    {
        if (args.Length == 0) { Console.Error.WriteLine("usage: RefDumpWorldgen.exe gen <outdir> <tag> [passes|real] [task|main] [nosnap] | x87 <outdir> | floatprobe <outdir>"); return 2; }
        switch (args[0])
        {
            case "gen": return MainGen(args);
            case "x87": return MainX87(args);
            case "floatprobe": return MainFloatProbe(args);
            default: Console.Error.WriteLine("unknown mode"); return 2;
        }
    }
}
