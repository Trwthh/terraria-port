// RefDumpWorldgenX entry point: the parameterised world generation harness (GenX.cs) and the JIT code dumper (JitDump.cs).
// A separate program from RefDumpWorldgen.exe (Program.cs, Gen.cs): that one and its outputs are unchanged.
using System;

public static partial class WG
{
    [STAThread]
    static int Main(string[] args)
    {
        if (args.Length == 0) { Console.Error.WriteLine("usage: RefDumpWorldgenX.exe genx <outdir> <tag> <seedtext> <size> <mode> <evil> <flags|-> <passes|real> <snaps> [watchdog s] | jitdump <outdir> <listfile>"); return 2; }
        switch (args[0])
        {
            case "genx": return MainGenX(args);
            case "jitdump": return MainJitDump(args);
            default: Console.Error.WriteLine("unknown mode"); return 2;
        }
    }
}
