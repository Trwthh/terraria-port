// The probe run (`RefDumpDraw.exe <dir> probe`, refdata\probe; port: tests/draw.rs DRAW_MODE=probe, tests/draw/probe*.rs,
// the same steps): the float methods that are reachable and that no earlier reference run executed
// (tools\cs2rs\float_uncovered.tsv, reason "notrun"), each reached in the original: by the game state that makes the game
// call it (armour sets worn, pets previewed, town NPCs asked for prices, ...) or, where that state is a long way off, by a
// direct call with fixed arguments. What is compared besides the draw scenes / digests / state lines of the other runs:
//   - probe lines in scenes.state ("probe <key> <values>": return values and out / ref values, floats as their bits; the
//     hash of a tile rectangle after a world generation step; selected fields of the objects the method changed)
//   - direct draw scenes (<scene>.trace.zlib with a "frame direct" header): the draw calls a method makes when it is
//     called outside the frame loop (inside a SpriteBatch Begin / End of the harness)
// PROBE_ONLY=<group>[,<group>..]: only these groups (diagnosis; reference and port must be run with the same value).
using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;
using Microsoft.Xna.Framework;

public static partial class WG
{
    static bool ProbeMode;
    static string[] ProbeOnly;

    /// <summary>Is the group on (PROBE_ONLY)? Writes a note when it is.</summary>
    static bool Group(string name)
    {
        if (ProbeOnly == null)
        {
            string e = Environment.GetEnvironmentVariable("PROBE_ONLY");
            ProbeOnly = string.IsNullOrEmpty(e) ? new string[0] : e.Split(',');
        }
        bool on = ProbeOnly.Length == 0 || Array.IndexOf(ProbeOnly, name) >= 0;
        if (on) Note("probe group " + name);
        return on;
    }

    // ---------------------------------------------------------------- probe lines (port: probe.rs `pv` / Run::probe)
    static string PV(object v)
    {
        if (v == null) return "-";
        if (v is float) return ((uint)BitConverter.ToInt32(BitConverter.GetBytes((float)v), 0)).ToString("x8");
        if (v is double) return ((ulong)BitConverter.DoubleToInt64Bits((double)v)).ToString("x16");
        if (v is bool) return (bool)v ? "1" : "0";
        if (v is Vector2) { var w = (Vector2)v; return PV(w.X) + "," + PV(w.Y); }
        if (v is Vector3) { var w = (Vector3)v; return PV(w.X) + "," + PV(w.Y) + "," + PV(w.Z); }
        if (v is Vector4) { var w = (Vector4)v; return PV(w.X) + "," + PV(w.Y) + "," + PV(w.Z) + "," + PV(w.W); }
        if (v is Color) return ((Color)v).PackedValue.ToString("x8");
        if (v is Rectangle) { var r = (Rectangle)v; return r.X + "," + r.Y + "," + r.Width + "," + r.Height; }
        if (v is Point) { var p = (Point)v; return p.X + "," + p.Y; }
        if (v is string) return "\"" + ((string)v).Replace("\n", "\\n") + "\"";
        if (v.GetType().IsEnum) return Convert.ToInt64(v).ToString(Inv);
        Type t = v.GetType();
        if (t.FullName == "Terraria.DataStructures.Point16") return IF(v, "X") + "," + IF(v, "Y");
        if (t.FullName == "Terraria.Utilities.Vector2D") return PV(IF(v, "X")) + "," + PV(IF(v, "Y"));
        if (t.IsPrimitive) return Convert.ToString(v, Inv);
        if (t.IsGenericType && t.GetGenericTypeDefinition() == typeof(Nullable<>)) return PV(t.GetProperty("Value").GetValue(v, null));
        return "<" + t.Name + ">";
    }
    /// <summary>One probe line (compared with the port's, in scenes.state).</summary>
    static void P(string key, params object[] vals)
    {
        StateLines.Append("probe " + key);
        foreach (object v in vals) StateLines.Append(' ').Append(PV(v));
        StateLines.Append('\n');
    }

    // ---------------------------------------------------------------- tiles
    static FieldInfo[] TileFields;
    /// <summary>FNV-1a 64 of the tiles of a rectangle (type, wall, liquid, the four headers, frameX, frameY; inactive
    /// rows included), clipped to the world.</summary>
    static ulong TileHash(int x, int y, int w, int h)
    {
        Type tt = T("Terraria.Tile");
        if (TileFields == null) TileFields = new[] { "type", "wall", "liquid", "sTileHeader", "bTileHeader", "bTileHeader2", "bTileHeader3", "frameX", "frameY" }.Select(n => tt.GetField(n, AllInst)).ToArray();
        int mx = (int)SF(TMain, "maxTilesX"), my = (int)SF(TMain, "maxTilesY");
        Array tiles = (Array)SF(TMain, "tile");
        ulong hsh = 14695981039346656037UL;
        for (int i = Math.Max(0, x); i < Math.Min(mx, x + w); i++)
            for (int j = Math.Max(0, y); j < Math.Min(my, y + h); j++)
            {
                object t = tiles.GetValue(i, j);
                if (t == null) { hsh ^= 0xff; hsh *= 1099511628211UL; continue; }
                foreach (FieldInfo f in TileFields)
                {
                    long v = Convert.ToInt64(f.GetValue(t));
                    for (int k = 0; k < 2; k++) { hsh ^= (byte)(v >> (8 * k)); hsh *= 1099511628211UL; }
                }
            }
        return hsh;
    }
    static void PTiles(string key, int x, int y, int w, int h) { StateLines.Append("probe " + key + " tiles " + x + "," + y + "," + w + "," + h + " " + TileHash(x, y, w, h).ToString("x16") + "\n"); }

    // ---------------------------------------------------------------- effects
    /// <summary>FNV-1a 64 of every active dust (index, type, position, velocity, scale, rotation, alpha) and gore
    /// (index, type, position, velocity, rotation, scale, alpha), with the counts.</summary>
    static void PFx(string key)
    {
        Func<ulong, int, ulong> i4 = (h, v) => { for (int k = 0; k < 4; k++) { h ^= (byte)(v >> (8 * k)); h *= 1099511628211UL; } return h; };
        Func<float, int> fb = f => BitConverter.ToInt32(BitConverter.GetBytes(f), 0);
        ulong dh = 14695981039346656037UL, gh = dh;
        int dn = 0, gn = 0;
        Array dust = (Array)SF(TMain, "dust");
        for (int i = 0; i < dust.Length; i++)
        {
            object d = dust.GetValue(i);
            if (d == null || !(bool)IF(d, "active")) continue;
            dn++;
            var p = (Vector2)IF(d, "position"); var v = (Vector2)IF(d, "velocity");
            dh = i4(i4(i4(i4(i4(i4(i4(i4(i4(dh, i), (int)IF(d, "type")), fb(p.X)), fb(p.Y)), fb(v.X)), fb(v.Y)), fb((float)IF(d, "scale"))), fb((float)IF(d, "rotation"))), (int)IF(d, "alpha"));
        }
        Array gore = (Array)SF(TMain, "gore");
        for (int i = 0; i < gore.Length; i++)
        {
            object g = gore.GetValue(i);
            if (g == null || !(bool)IF(g, "active")) continue;
            gn++;
            var p = (Vector2)IF(g, "position"); var v = (Vector2)IF(g, "velocity");
            gh = i4(i4(i4(i4(i4(i4(i4(i4(i4(gh, i), (int)IF(g, "type")), fb(p.X)), fb(p.Y)), fb(v.X)), fb(v.Y)), fb((float)IF(g, "rotation"))), fb((float)IF(g, "scale"))), (int)IF(g, "alpha"));
        }
        StateLines.Append("probe " + key + " fx dust " + dn + ":" + dh.ToString("x16") + " gore " + gn + ":" + gh.ToString("x16") + "\n");
    }
    /// <summary>Every dust and gore inactive (a probe's effects start from nothing).</summary>
    static void ClearFx()
    {
        foreach (object d in (Array)SF(TMain, "dust")) if (d != null) ISet(d, "active", false);
        foreach (object g in (Array)SF(TMain, "gore")) if (g != null) ISet(g, "active", false);
    }

    // ---------------------------------------------------------------- direct draws
    /// <summary>The draw calls of `a`, called outside the frame loop, as a scene of its own (port: Run::direct).</summary>
    static void DirectDraw(string scene, Action a)
    {
        var ms = new MemoryStream();
        var w = new StreamWriter(ms, new UTF8Encoding(false));
        TextWriter old = Microsoft.Xna.Framework.XnaStandIn.Rec.Out;
        Microsoft.Xna.Framework.XnaStandIn.Rec.Out = w;
        long before = Microsoft.Xna.Framework.XnaStandIn.Rec.Lines;
        w.Write("frame direct\n");
        Microsoft.Xna.Framework.XnaStandIn.Rec.Full = true;
        try { a(); }
        finally
        {
            Microsoft.Xna.Framework.XnaStandIn.Rec.Full = false;
            w.Flush();
            Microsoft.Xna.Framework.XnaStandIn.Rec.Out = old;
        }
        byte[] body = ms.ToArray();
        WriteZlib(Path.Combine(OutRoot, scene + ".trace.zlib"), new byte[0], body);
        string line = scene + ": direct, " + (Microsoft.Xna.Framework.XnaStandIn.Rec.Lines - before) + " calls, " + body.Length + " bytes";
        StateLine(scene);
        Summary.Add(line);
        Console.WriteLine(line);
    }
    /// <summary>Main.spriteBatch.Begin() (deferred, alpha blend) ... End() around `a`.</summary>
    static void InBatch(Action a)
    {
        object sb = SF(TMain, "spriteBatch");
        sb.GetType().GetMethod("Begin", Type.EmptyTypes).Invoke(sb, null);
        try { a(); }
        finally { sb.GetType().GetMethod("End").Invoke(sb, null); }
    }

    // ---------------------------------------------------------------- calls
    /// <summary>The method `name` of `type` with `n` parameters (the first in declaration order when there are several;
    /// `sig`: the parameter type names, comma separated, to pick an overload).</summary>
    static MethodInfo MI(Type t, string name, int n = -1, string sig = null)
    {
        foreach (MethodInfo m in t.GetMethods(AllStatic | AllInst | BindingFlags.DeclaredOnly))
        {
            if (m.Name != name) continue;
            ParameterInfo[] ps = m.GetParameters();
            if (n >= 0 && ps.Length != n) continue;
            if (sig != null && string.Join(",", ps.Select(p => p.ParameterType.Name).ToArray()) != sig) continue;
            return m;
        }
        throw new Exception("method not found: " + t.FullName + "." + name + "/" + n + " " + sig);
    }
    /// <summary>Calls a method (static: target null); exceptions are thrown on (a probe must not throw in either game).</summary>
    static object Call(object target, Type t, string name, params object[] args) { return Unwrap(MI(t, name, args.Length), target, args); }
    static object CallS(string type, string name, params object[] args) { return Unwrap(MI(T(type), name, args.Length), null, args); }
    static object CallI(object target, string name, params object[] args)
    {
        for (Type t = target.GetType(); t != null; t = t.BaseType)
            foreach (MethodInfo m in t.GetMethods(AllInst | BindingFlags.DeclaredOnly))
                if (m.Name == name && m.GetParameters().Length == args.Length) return Unwrap(m, target, args);
        throw new Exception("instance method not found: " + target.GetType().FullName + "." + name);
    }
    static object New(string type, params object[] args)
    {
        Type t = T(type);
        if (args.Length == 0) return Activator.CreateInstance(t, true);
        return Activator.CreateInstance(t, AllInst, null, args, Inv);
    }
    /// <summary>An instance field or property set.</summary>
    static void PSet(object o, string name, object v)
    {
        for (Type t = o.GetType(); t != null; t = t.BaseType)
        {
            FieldInfo f = t.GetField(name, AllInst | BindingFlags.DeclaredOnly);
            if (f != null) { f.SetValue(o, v); return; }
            PropertyInfo p = t.GetProperty(name, AllInst | BindingFlags.DeclaredOnly);
            if (p != null && p.CanWrite) { p.SetValue(o, v, null); return; }
        }
        throw new Exception("instance member not found: " + o.GetType().FullName + "." + name);
    }
    /// <summary>The local player's selected inventory slot (Player.selectedItemState.Select).</summary>
    static void SelectItem(int slot)
    {
        object pl = LocalPlayer;
        FieldInfo f = pl.GetType().GetField("selectedItemState", AllInst);
        object st = f.GetValue(pl);
        st.GetType().GetMethod("Select").Invoke(st, new object[] { slot });
        f.SetValue(pl, st);
    }
    static object NpcAt(int i) { return ((Array)SF(TMain, "npc")).GetValue(i); }
    static object ProjAt(int i) { return ((Array)SF(TMain, "projectile")).GetValue(i); }
    static object Src() { return Activator.CreateInstance(T("Terraria.DataStructures.EntitySource_DebugCommand")); }

    // ================================================================== the run
    static void ProbeWorld()
    {
        int sx = (int)SF(TMain, "spawnTileX"), sy = (int)SF(TMain, "spawnTileY");
        SetTime(true, 27000.0);
        PlacePlayer(sx, sy);
        Immune();
        Frames(30);
        int x0 = sx + BuildDX;
        int gy = GroundBelow(x0, sy - 12);
        Note("probe ground at " + x0 + "," + gy);
        for (int x = x0 - 2; x <= x0 + BuildW; x++)
        {
            for (int y = gy - 12; y < gy; y++) KillTile(x, y);
            for (int y = gy; y <= gy + 1; y++) PlaceTile(x, y, 0, 0);
        }
        Frames(5);
        ProbeA(sx, sy, x0, gy);
        ProbeB(sx, sy, x0, gy);
        ProbeC(sx, sy, x0, gy);
        ProbeD(sx, sy, x0, gy);
        ProbeE(sx, sy, x0, gy);
        ProbeF(sx, sy, x0, gy);
        ProbeG(sx, sy, x0, gy);
        ProbeH(sx, sy, x0, gy);
        ProbeI(sx, sy, x0, gy);
        ProbeJ(sx, sy, x0, gy);
        ProbeK(sx, sy, x0, gy);
        ProbeL(sx, sy, x0, gy);
        ProbeM(sx, sy, x0, gy);
        ProbeN(sx, sy, x0, gy);
        ProbeO(sx, sy, x0, gy);
        ProbeP(sx, sy, x0, gy);
        ProbeQ(sx, sy, x0, gy);
        ProbeR(sx, sy, x0, gy);
        ProbeS(sx, sy, x0, gy);
    }
}
