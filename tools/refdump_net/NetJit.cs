// NetJit: the net run's server (NetServer.cs, compiled in unchanged), then the machine code of every TerrariaServer /
// ReLogic / XNA method that ran in that process (tools\refdump_worldgen\JitDump.cs DumpRan; the methods the harness
// replaced with a jump, Detour.Replaced, are left out).
//
//   RefJitNet.exe <jit outdir> server <data outdir> <pipe name> [watchdog seconds]
//
// Proof that it is the run that wrote refdata\net: its data folder against refdata\net (run_net.sh).
using System;
using System.IO;
using System.Linq;

public static partial class WG
{
    public static int NetJitMain(string[] args)
    {
        if (args.Length < 4) { Console.Error.WriteLine("usage: RefJitNet.exe <jit outdir> server <data outdir> <pipe name> [watchdog seconds]"); return 2; }
        string jitDir = Path.GetFullPath(args[0]);
        if (Path.GetFullPath(args[2]).IndexOf("\refdata", StringComparison.OrdinalIgnoreCase) >= 0) { Console.Error.WriteLine("refusing to write below refdata"); return 2; }
        Directory.CreateDirectory(jitDir);
        NetAfterRun = () => DumpRan(jitDir, AllMethods("TerrariaServer", "ReLogic", "Microsoft.Xna.Framework").Where(m => !Detour.Replaced.Contains(m)).ToList());
        return NetServerMain(args.Skip(1).ToArray());
    }
}

static class NetJitProgram
{
    [STAThread]
    static int Main(string[] args) { return WG.NetJitMain(args); }
}
