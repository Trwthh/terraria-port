// RefDumpX87Ext entry point (second harness program; RefDumpWorldgen.exe and its outputs are not touched).
// Modes: x87raw | x87dense | x87more (ExtX87.cs), floatprobe2 (ExtProbes.cs, ExtProbesGen.cs). Headless console program, 32-bit, 7-minute watchdog.
using System;
using System.IO;
using System.Reflection;
using System.Threading;

public static class XProgram
{
    public static string Root, GameDir;
    static Assembly OnResolve(object sender, ResolveEventArgs a)
    {
        string p = Path.Combine(GameDir, new AssemblyName(a.Name).Name + ".dll");       // Microsoft.Xna.Framework.dll, read only
        return File.Exists(p) ? Assembly.LoadFrom(p) : null;
    }

    static int Main(string[] args)
    {
        if (args.Length < 2) { Console.Error.WriteLine("usage: RefDumpX87Ext.exe x87raw <outdir> <x87dir> | x87dense <outdir> | x87more <outdir> | floatprobe2 <outdir>"); return 2; }
        if (IntPtr.Size != 4) { Console.Error.WriteLine("must run as a 32-bit process"); return 3; }
        var wd = new Thread(() => { Thread.Sleep(7 * 60 * 1000); Environment.Exit(99); }); wd.IsBackground = true; wd.Start();
        Root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(Root, "work", "refserver");
        AppDomain.CurrentDomain.AssemblyResolve += OnResolve;
        string outdir = Path.GetFullPath(args[1]);
        switch (args[0])
        {
            case "x87raw": return XE.Raw(outdir, Path.GetFullPath(args[2]));
            case "x87dense": return XE.DenseAll(outdir);
            case "x87more": return XE.More(outdir);
            case "floatprobe2": return XP.Run(outdir);
            default: Console.Error.WriteLine("unknown mode"); return 2;
        }
    }
}
