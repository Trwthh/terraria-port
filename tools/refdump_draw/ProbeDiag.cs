// Diagnosis only (PROBE_DIAG=1 with the probe run): the entry bytes and method descriptors of methods the JIT dump
// does not see (struct methods reached through interfaces, shared generic methods). Writes to the console log.
using System;
using System.Linq;
using System.Reflection;
using System.Runtime.InteropServices;

public static partial class WG
{
    static void ProbeDiag()
    {
        if (Environment.GetEnvironmentVariable("PROBE_DIAG") != "1") return;
        var ms = new System.Collections.Generic.List<MethodInfo>();
        foreach (string tn in new[] { "Terraria.ID.TorchID+DemonTorchLight", "Terraria.ID.TorchID+ConstantTorchLight" }) ms.Add(T(tn).GetMethod("GetRGB"));
        MethodInfo gm = T("Terraria.GameContent.UI.States.UICreativePowersMenu").GetMethods(AllInst).First(x => x.Name == "CreateSubcategoryButton");
        ms.Add(gm.MakeGenericMethod(T("Terraria.GameContent.Creative.CreativePowers+DifficultySliderPower")));
        Type canon = typeof(object).Assembly.GetType("System.__Canon");
        Console.WriteLine("diag canon type " + (canon == null ? "null" : canon.FullName));
        if (canon != null) ms.Add(gm.MakeGenericMethod(canon));
        foreach (MethodInfo m in ms)
        {
            string tn = m.DeclaringType.Name + "." + m.Name;
            IntPtr fp = m.MethodHandle.GetFunctionPointer();
            IntPtr md = m.MethodHandle.Value;
            var b = new byte[16];
            for (int i = 0; i < 16; i++) b[i] = Marshal.ReadByte(fp, i);
            Console.WriteLine("diag " + tn + " md " + md.ToString("x") + " fp " + fp.ToString("x") + " bytes " + BitConverter.ToString(b));
            IntPtr p = fp;
            for (int hop = 0; hop < 6; hop++)
            {
                byte b0 = Marshal.ReadByte(p);
                int rel;
                if (b0 == 0xE9) { rel = Marshal.ReadInt32(p, 1); p = new IntPtr(p.ToInt32() + 5 + rel); }
                else if (b0 == 0x83 && Marshal.ReadByte(p, 1) == 0xC1 && Marshal.ReadByte(p, 3) == 0xE9) { rel = Marshal.ReadInt32(p, 4); p = new IntPtr(p.ToInt32() + 8 + rel); }
                else if (b0 == 0x83 && Marshal.ReadByte(p, 1) == 0xC1 && Marshal.ReadByte(p, 3) == 0xFF && Marshal.ReadByte(p, 4) == 0x25) { IntPtr slot = new IntPtr(Marshal.ReadInt32(p, 5)); p = new IntPtr(Marshal.ReadInt32(slot)); }
                else if (b0 == 0xB8 && Marshal.ReadByte(p, 5) == 0x90 && Marshal.ReadByte(p, 6) == 0xE9) { rel = Marshal.ReadInt32(p, 7); p = new IntPtr(p.ToInt32() + 11 + rel); }
                else break;
                var c = new byte[12];
                for (int i = 0; i < 12; i++) c[i] = Marshal.ReadByte(p, i);
                Console.WriteLine("diag   hop " + hop + " -> " + p.ToString("x") + " hdr " + Marshal.ReadInt32(new IntPtr(p.ToInt32() - 4)).ToString("x") + " bytes " + BitConverter.ToString(c));
            }
        }
    }
}
