// Method detours for the draw reference harness (x86 CLR 4): the machine code of a method is overwritten with a jump
// to a replacement with the same calling convention. Used for the few members of the REAL XNA Framework / mscorlib /
// game the scenario has to pin (idle input, content file opening, the tick counter), nothing else.
using System;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

static unsafe class Detour
{
    [DllImport("kernel32.dll")] static extern bool VirtualProtect(IntPtr address, UIntPtr size, uint protect, out uint old);
    [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll")] static extern bool FlushInstructionCache(IntPtr process, IntPtr address, UIntPtr size);

    /// <summary>The start of the method's machine code: its entry point, through any jump stubs.</summary>
    public static byte* Code(MethodBase m)
    {
        try { RuntimeHelpers.PrepareMethod(m.MethodHandle); } catch (Exception) { }
        byte* p = (byte*)m.MethodHandle.GetFunctionPointer();
        for (int i = 0; i < 4 && p[0] == 0xE9; i++) p = p + 5 + *(int*)(p + 1);
        return p;
    }

    /// <summary>Every call of `from` (also already compiled callers) runs `to` instead.</summary>
    /// <summary>Every method replaced so far (their machine code starts with the jump: not a fact of the game).</summary>
    public static readonly System.Collections.Generic.HashSet<MethodBase> Replaced = new System.Collections.Generic.HashSet<MethodBase>();

    public static void Install(MethodBase from, MethodBase to)
    {
        Replaced.Add(from);
        if (from == null || to == null) throw new ArgumentNullException(from == null ? "from" : "to");
        byte* src = Code(from);
        RuntimeHelpers.PrepareMethod(to.MethodHandle);
        byte* dst = (byte*)to.MethodHandle.GetFunctionPointer();
        uint old;
        if (!VirtualProtect((IntPtr)src, (UIntPtr)5, 0x40, out old)) throw new Exception("VirtualProtect failed for " + from);
        src[0] = 0xE9;
        *(int*)(src + 1) = (int)(dst - src - 5);
        VirtualProtect((IntPtr)src, (UIntPtr)5, old, out old);
        FlushInstructionCache(GetCurrentProcess(), (IntPtr)src, (UIntPtr)5);
    }
}
