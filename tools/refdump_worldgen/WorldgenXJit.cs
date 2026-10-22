// WorldgenXJit: a world of the generation matrix (GenX.cs, "genx" mode of RefDumpWorldgenX.exe, compiled in unchanged
// with Common.cs, build_refjit.cmd; large-address-aware like it), then the machine code of every game method that ran in
// that process (JitDump.cs DumpRanGame). The body below is MainGenX of GenX.cs, copied, with the jit folder taken from
// the first argument and the dump after RunGenX.
//
//   RefJitWorldgenX.exe <jit outdir> genx <data outdir> <tag> <seedtext> <size> <mode> <evil> <flags|-> passes none <watchdog s>
//
// run_matrix.sh wrote refdata\worldgen\matrix with the arguments of its table. Proof that it is the same run:
// <tag>.passes.jsonl and <tag>.columns.bin of the data folder against refdata\worldgen\matrix (refjit_compare.py).
using System;
using System.Diagnostics;
using System.IO;
using System.Threading;

public static partial class WG
{
    public static int WorldgenXJitMain(string[] argv)
    {
        // argv: <jit outdir> genx <genx arguments as RefDumpWorldgenX.exe>
        if (argv.Length < 2 || argv[1] != "genx") { Console.Error.WriteLine("usage: RefJitWorldgenX.exe <jit outdir> genx <outdir> <tag> ..."); return 2; }
        string jitDir = Path.GetFullPath(argv[0]);
        string[] args = new string[argv.Length - 1];
        Array.Copy(argv, 1, args, 0, args.Length);
        if (Path.GetFullPath(args[1]).IndexOf("\\refdata", StringComparison.OrdinalIgnoreCase) >= 0) { Console.Error.WriteLine("refusing to write below refdata"); return 2; }
        if (args.Length < 10) { Console.Error.WriteLine("usage: RefDumpWorldgenX.exe genx <outdir> <tag> <seedtext> <small|medium|large> <classic|expert|master|journey> <random|corruption|crimson> <flags|-> <passes|real> <none|default|i,j,..> [watchdog seconds]"); return 2; }
        string outdir = Path.GetFullPath(args[1]);
        XTag = args[2]; XSeedText = args[3];
        int size = Array.IndexOf(XSizes, args[4]), gameMode = Array.IndexOf(XModes, args[5]), evil = Array.IndexOf(XEvils, args[6]) - 1;
        if (size < 0 || gameMode < 0 || evil < -1) { Console.Error.WriteLine("unknown size / game mode / evil"); return 2; }
        string[] flags = args[7] == "-" ? new string[0] : args[7].Split(',');
        string mode = args[8];
        if (mode != "passes" && mode != "real") { Console.Error.WriteLine("mode is passes or real"); return 2; }
        if (args[9] == "default") XSnapDefault = true;
        else if (args[9] != "none") foreach (string s in args[9].Split(',')) XSnapSet.Add(int.Parse(s, Inv));
        int watchdog = args.Length > 10 ? int.Parse(args[10], Inv) : 1800;
        if (XTag.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0 || XTag.Length == 0) { Console.Error.WriteLine("the tag must be a file name"); return 2; }

        Root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(Root, "work", "refserver");
        string scratch = Dir(Path.Combine(Root, "work", "worldgen-scratch", XTag));
        SaveDir = Dir(Path.Combine(scratch, "save"));
        XOut = Dir(outdir);
        // never below the private server copy or the game folder
        foreach (string p in new[] { XOut, scratch })
            if (Path.GetFullPath(p).StartsWith(Path.GetFullPath(GameDir), StringComparison.OrdinalIgnoreCase) || p.IndexOf("\\Documents\\Games\\", StringComparison.OrdinalIgnoreCase) >= 0)
            { Console.Error.WriteLine("refusing to write below the game folder: " + p); return 2; }
        string wldPath = Path.Combine(Dir(Path.Combine(scratch, "Worlds")), XTag + ".wld");
        if (File.Exists(wldPath)) File.Delete(wldPath);
        Watchdog(watchdog);
        Trace.Listeners.Add(new XCapture());
        var mem = new Thread(() => { for (;;) { try { long ws = Process.GetCurrentProcess().WorkingSet64; if (ws > XPeakWorkingSet) XPeakWorkingSet = ws; } catch { } Thread.Sleep(500); } });
        mem.IsBackground = true; mem.Start();
        int rc = 0;
        try { RunGenX(mode, size, gameMode, evil, flags, wldPath); Console.Out.Flush(); Dir(jitDir); DumpRanGame(jitDir); }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }
}

static class WorldgenXJitProgram
{
    [STAThread]
    static int Main(string[] args) { return WG.WorldgenXJitMain(args); }
}
