// RefDumpEntities: the entity reference scenario. Every NPC type / projectile type of the real dedicated server is
// spawned at fixed positions on a scratch copy of ref_small.wld and simulated for N ticks by the game's own per-tick
// entity updates; the full state of every active entity and every RNG stream is dumped after every tick.
//
//   RefDumpEntities.exe npc|proj|item|citem <outdir> [ticks=120] [max batches]
//   (citem: client-side item use by the owning player, ClientItems.cs)
//
// item: every item type with a use style (Item.useStyle != 0 after SetDefaults) is put into inventory slot 0 of its
// slot's player (the selected slot), and before every tick the player's controls say "use item", facing right: what
// the server receives from a client that holds the mouse button. The server runs ItemCheck for that remote player
// (use animation, item rotation / location, hold and use styles); what only the owning client does (shooting,
// placing tiles, consuming) does not happen on a server.
//
// Same loading as tools\refdump_ticks\Ticks.cs (its Common.cs is compiled in): InitGame, `-world` SetAsActive, the load
// on a thread-pool thread with that thread's Main.rand pinned (1357), the server state without sockets, gameMenu =
// false. Headless, no window, no socket; the game folder is never written (work\refserver is a private copy).
//
// Batches. The types are laid out in slots of 31 (solo: a boss, alone in its batch). Slot k stands at tile column
// SlotX0 + k * SlotDX, on the first solid tile from the top (the scan below, done once on the loaded world; the
// positions are written to the dump, the port reads them). Each slot has a remote player standing PlayerDX tiles to the
// right of it (Player.Spawn(SpawningIntoWorld) with SpawnX / SpawnY set: the bed-spawn path, Player.cs:37975), so
// targeting AI has a target and hostile projectiles something to hit; slots are 115 tiles apart so they do not meet.
// Before every batch (both sides do exactly this, in this order):
//   1. tiles: every tile whose 14-byte record differs from the loaded world is counted, hashed (dumped) and restored
//   2. NPC.ClearAll(); Projectile.ClearAll(); Main.item[i] = new WorldItem { whoAmI = i },
//      Main.timeItemSlotCannotBeReusedFor[i] = 0 (WorldGen.clearWorld, WorldGen.cs:7163-7170)
//   3. Main.player[k] = new Player { whoAmI = k } for k < 31 (Main.cs:6889-6893)
//   4. Main.rand = new UnifiedRandom(EntSeedBase + batch); Main._rngs = new Dictionary (its streams are re-created from
//      the world seed on first use, Main.cs:3385-3395)
//   5. the slot players (active, 100 life, 20 mana, SpawnX / SpawnY, Spawn(SpawningIntoWorld)), slot by slot
//   6. the entities, slot by slot: NPC.NewNPC(new EntitySource_SpawnNPC(), x, y, type) with the NPC's bottom on the
//      ground; Projectile.NewProjectile(new EntitySource_DebugCommand(), pos, (5, -2), type, 20, 2f, 255): the server
//      owns them, as the projectiles NPCs fire (on a server NewProjectile refuses any other owner: "NewProjectile owner
//      (n) != myPlayer (255). Will not sync or do damage!", Projectile.cs:10875-10880). Player-owned behaviour (minions,
//      held projectiles) therefore runs its "owner is not active" path; that is what a server does with them
// One tick = the entity part of Main.DoUpdateInWorld (Main.cs:17942-18030), the game's own private methods in its
// order, inside its SwapRandom scopes: SwapRandom("DoUpdateInWorld") { _gameUpdateCount++; tileSolid[379] = false;
// SwapRandom("UpdatePlayers") UpdateWorld_Players(); NPC.RevengeManager.Update(); SwapRandom("UpdateNPCs")
// UpdateWorld_NPCs(); SwapRandom("UpdateProjectiles") UpdateWorld_Projectiles(); SwapRandom("UpdateItems")
// UpdateWorld_Items(); tileSolid[379] = true }. Left out: natural NPC spawning, time, weather, world / liquid updates,
// pressure plates and the client-only parts (particles, gore, dust and floating text do nothing on a server).
//
// Output (<outdir>): <kind>.ents.zlib (format: refdata\entities\README.md), <kind>.summary.json (types, batches,
// spawn results, skipped types, leaf schema notes), <kind>.timing.json (wall clock; the only file that differs between
// two runs).
using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Reflection;
using System.Text;
using System.Threading.Tasks;

public static partial class WG
{
    const int EntSeedBase = 777000;
    const int LoadSeedE = 1357;
    const int SlotCount = 31, SlotX0 = 350, SlotDX = 115, PlayerDX = 20;
    const ulong Missing = 0xDEADBEEFDEADBEEFUL;   // an element leaf past the end of a shorter array
    static string EOut;
    static Type TNPC, TProj, TPlayer, TWorldItem, TTile;
    static List<string> ENotes = new List<string>();

    [STAThread]
    static int Main(string[] args)
    {
        if (args.Length < 2) { Console.Error.WriteLine("usage: RefDumpEntities.exe npc|proj <outdir> [ticks] [max batches]"); return 2; }
        Root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(Root, "work", "refserver");
        string kind = args[0];
        EOut = Dir(Path.GetFullPath(args[1]));
        int ticks = args.Length > 2 ? int.Parse(args[2]) : 120;
        int maxBatches = args.Length > 3 ? int.Parse(args[3]) : int.MaxValue;
        string scratch = Dir(Path.Combine(Root, "work", "entities-scratch", kind + "_" + new DirectoryInfo(EOut).Name));
        SaveDir = Dir(Path.Combine(scratch, "save"));
        Watchdog(kind == "citem" ? 7200 : 1500);
        int rc = 0;
        try { EntRun(kind, ticks, scratch, maxBatches); }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }

    // ------------------------------------------------------------------------------------------------ leaf schema
    // Every instance field of the class and its bases (FieldsOf order, names with '<' left out) that is
    //   primitive / enum                      -> one leaf `name`
    //   a struct of primitives (Vector2, Rectangle, Color, Point16 ...) -> `name.Field` per field (FieldsOf order)
    //   a 1-D array of primitives / enums / Vector2 -> `name[]` (FNV-1a 64 of u32 length + the elements' raw bytes),
    //                                           plus `name[i]` for i < the template's length when that is <= 8
    // Anything else (references to other objects, strings, delegates, nested structs) is not a leaf; the summary lists it.
    // Leaf values are u64: integers sign-extended (unsigned zero-extended), bool 0 / 1, float bits, double bits.
    const byte KInt = 0, KF32 = 1, KF64 = 2, KHash = 3, KBool = 4;
    class Leaf { public string Name; public byte Kind; }
    class FieldPlan
    {
        public FieldInfo F; public int Mode;   // 0 scalar, 1 struct, 2 array
        public FieldInfo[] Subs; public int NElem; public Type Elem; public PropertyInfo P;
    }
    class Schema { public Type T; public List<Leaf> Leaves = new List<Leaf>(); public List<FieldPlan> Plans = new List<FieldPlan>(); public List<string> Skipped = new List<string>(); }

    static bool IsPrim(Type t) { return t.IsPrimitive || t.IsEnum; }
    static byte KindOf(Type t)
    {
        if (t == typeof(float)) return KF32;
        if (t == typeof(double)) return KF64;
        if (t == typeof(bool)) return KBool;
        return KInt;
    }
    static bool IsFlatStruct(Type t)
    {
        if (!t.IsValueType || t.IsPrimitive || t.IsEnum) return false;
        if (t.IsGenericType) return false;
        FieldInfo[] fs = FieldsOf(t);
        if (fs.Length == 0) return false;
        foreach (FieldInfo f in fs) if (!IsPrim(f.FieldType)) return false;
        return true;
    }

    static Schema MakeSchema(Type t, object template, string[] props)
    {
        var s = new Schema { T = t };
        var names = new HashSet<string>();
        foreach (FieldInfo f in FieldsOf(t))
        {
            Type ft = f.FieldType;
            if (!names.Add(f.Name)) { s.Skipped.Add(f.Name + " (a base class declares the same name)"); continue; }
            if (IsPrim(ft))
            {
                s.Plans.Add(new FieldPlan { F = f, Mode = 0 });
                s.Leaves.Add(new Leaf { Name = f.Name, Kind = KindOf(ft.IsEnum ? Enum.GetUnderlyingType(ft) : ft) });
            }
            else if (IsFlatStruct(ft))
            {
                FieldInfo[] subs = FieldsOf(ft);
                s.Plans.Add(new FieldPlan { F = f, Mode = 1, Subs = subs });
                foreach (FieldInfo sf in subs) s.Leaves.Add(new Leaf { Name = f.Name + "." + sf.Name, Kind = KindOf(sf.FieldType.IsEnum ? Enum.GetUnderlyingType(sf.FieldType) : sf.FieldType) });
            }
            else if (ft.IsArray && ft.GetArrayRank() == 1 && (IsPrim(ft.GetElementType()) || ft.GetElementType().FullName == "Microsoft.Xna.Framework.Vector2"))
            {
                Type et = ft.GetElementType();
                Array a = (Array)f.GetValue(template);
                int n = (a != null && a.Length <= 8 && IsPrim(et)) ? a.Length : 0;
                s.Plans.Add(new FieldPlan { F = f, Mode = 2, NElem = n, Elem = et });
                s.Leaves.Add(new Leaf { Name = f.Name + "[]", Kind = KHash });
                for (int i = 0; i < n; i++) s.Leaves.Add(new Leaf { Name = f.Name + "[" + i + "]", Kind = KindOf(et.IsEnum ? Enum.GetUnderlyingType(et) : et) });
            }
            else s.Skipped.Add(f.Name + " : " + ft.Name);
        }
        if (props != null)
            foreach (string pn in props)
            {
                PropertyInfo p = t.GetProperty(pn, AllInst);
                s.Plans.Add(new FieldPlan { P = p, Mode = 3 });
                s.Leaves.Add(new Leaf { Name = "=" + pn, Kind = KindOf(p.PropertyType) });
            }
        return s;
    }

    static ulong Bits(object v)
    {
        if (v is bool) return (bool)v ? 1UL : 0UL;
        if (v is float) return (ulong)(uint)BitConverter.ToInt32(BitConverter.GetBytes((float)v), 0);
        if (v is double) return (ulong)BitConverter.DoubleToInt64Bits((double)v);
        if (v is char) return (ulong)(char)v;
        if (v is Enum) v = Convert.ChangeType(v, Enum.GetUnderlyingType(v.GetType()), Inv);
        if (v is byte) return (byte)v;
        if (v is sbyte) return (ulong)(long)(sbyte)v;
        if (v is short) return (ulong)(long)(short)v;
        if (v is ushort) return (ushort)v;
        if (v is int) return (ulong)(long)(int)v;
        if (v is uint) return (uint)v;
        if (v is long) return (ulong)(long)v;
        if (v is ulong) return (ulong)v;
        throw new Exception("not a primitive: " + v.GetType());
    }
    static ulong FnvBytes(ulong h, byte[] b) { for (int i = 0; i < b.Length; i++) { h ^= b[i]; h *= FnvPrime; } return h; }
    static byte[] RawBytes(object e)
    {
        if (e is Enum) e = Convert.ChangeType(e, Enum.GetUnderlyingType(e.GetType()), Inv);
        if (e is bool) return new byte[] { (byte)((bool)e ? 1 : 0) };
        if (e is byte) return new byte[] { (byte)e };
        if (e is sbyte) return new byte[] { (byte)(sbyte)e };
        if (e is short) return BitConverter.GetBytes((short)e);
        if (e is ushort) return BitConverter.GetBytes((ushort)e);
        if (e is char) return BitConverter.GetBytes((ushort)(char)e);
        if (e is int) return BitConverter.GetBytes((int)e);
        if (e is uint) return BitConverter.GetBytes((uint)e);
        if (e is long) return BitConverter.GetBytes((long)e);
        if (e is ulong) return BitConverter.GetBytes((ulong)e);
        if (e is float) return BitConverter.GetBytes((float)e);
        if (e is double) return BitConverter.GetBytes((double)e);
        // Vector2
        Type t = e.GetType();
        var ms = new MemoryStream();
        foreach (FieldInfo f in FieldsOf(t)) { byte[] b = RawBytes(f.GetValue(e)); ms.Write(b, 0, b.Length); }
        return ms.ToArray();
    }

    static ulong[] ReadLeaves(Schema s, object o)
    {
        var r = new ulong[s.Leaves.Count];
        int k = 0;
        foreach (FieldPlan p in s.Plans)
        {
            switch (p.Mode)
            {
                case 0: r[k++] = Bits(p.F.GetValue(o)); break;
                case 1:
                    {
                        object sv = p.F.GetValue(o);
                        foreach (FieldInfo sf in p.Subs) r[k++] = Bits(sf.GetValue(sv));
                        break;
                    }
                case 2:
                    {
                        Array a = (Array)p.F.GetValue(o);
                        if (a == null) { r[k++] = 0; for (int i = 0; i < p.NElem; i++) r[k++] = Missing; break; }
                        ulong h = FnvBytes(FnvBasis, BitConverter.GetBytes(a.Length));
                        if (a is float[]) { foreach (float x in (float[])a) h = FnvBytes(h, BitConverter.GetBytes(x)); }
                        else if (a is int[]) { foreach (int x in (int[])a) h = FnvBytes(h, BitConverter.GetBytes(x)); }
                        else if (a is bool[]) { foreach (bool x in (bool[])a) { h ^= (byte)(x ? 1 : 0); h *= FnvPrime; } }
                        else if (a is byte[]) { foreach (byte x in (byte[])a) { h ^= x; h *= FnvPrime; } }
                        else foreach (object e in a) h = FnvBytes(h, RawBytes(e));
                        r[k++] = h;
                        for (int i = 0; i < p.NElem; i++) r[k++] = i < a.Length ? Bits(a.GetValue(i)) : Missing;
                        break;
                    }
                case 3: r[k++] = Bits(p.P.GetValue(o, null)); break;
            }
        }
        return r;
    }

    // ------------------------------------------------------------------------------------------------ zlib writer
    // RFC 1950 stream written as it goes (header 78 9C, raw deflate, big-endian Adler-32 of the uncompressed bytes).
    class ZOut : Stream
    {
        FileStream fs; DeflateStream ds; uint a = 1, b = 0; public long Total;
        public ZOut(string path) { fs = new FileStream(path, FileMode.Create); fs.WriteByte(0x78); fs.WriteByte(0x9C); ds = new DeflateStream(fs, CompressionLevel.Optimal, true); }
        public override void Write(byte[] buf, int off, int n)
        {
            for (int i = 0; i < n; i++) { a += buf[off + i]; if (a >= 65521) a -= 65521; b += a; if (b >= 65521) b -= 65521; }
            ds.Write(buf, off, n); Total += n;
        }
        public override void Close()
        {
            if (ds != null) { ds.Close(); ds = null; uint ad = (b << 16) | a; fs.WriteByte((byte)(ad >> 24)); fs.WriteByte((byte)(ad >> 16)); fs.WriteByte((byte)(ad >> 8)); fs.WriteByte((byte)ad); fs.Close(); }
            base.Close();
        }
        public override bool CanRead { get { return false; } }
        public override bool CanSeek { get { return false; } }
        public override bool CanWrite { get { return true; } }
        public override long Length { get { return Total; } }
        public override long Position { get { return Total; } set { throw new NotSupportedException(); } }
        public override void Flush() { }
        public override int Read(byte[] buffer, int offset, int count) { throw new NotSupportedException(); }
        public override long Seek(long offset, SeekOrigin origin) { throw new NotSupportedException(); }
        public override void SetLength(long value) { throw new NotSupportedException(); }
    }
    static BinaryWriter BW;
    static void WStr(string s) { byte[] b = Encoding.UTF8.GetBytes(s); BW.Write((ushort)b.Length); BW.Write(b); }

    // ------------------------------------------------------------------------------------------------ scenario
    class Slot { public int Type, Ex, Ey, Px, Py, Pl, Owner = -1, Index = -1; public float Vx, Vy; public string Error; }
    static Schema[] Schemas;
    static string[] ClassArrays = { "npc", "projectile", "player", "item" };
    static int[] ClassCounts = { 200, 1000, 255, 400 };
    static Dictionary<int, ulong[]>[] Last;

    static int Ground(int x)
    {
        Array tile = (Array)SF(TMain, "tile");
        bool[] solid = (bool[])SF(TMain, "tileSolid"), solidTop = (bool[])SF(TMain, "tileSolidTop");
        int maxY = (int)(double)SF(TMain, "worldSurface") + 100;
        for (int y = 50; y < maxY; y++)
        {
            object t = tile.GetValue(x, y);
            if (t == null) continue;
            ushort hdr = (ushort)IF(t, "sTileHeader"), ty = (ushort)IF(t, "type");
            if ((hdr & 0x20) != 0 && solid[ty] && !solidTop[ty]) return y;
        }
        throw new Exception("no ground in column " + x);
    }

    static MethodInfo MSwap, MPlayers, MNpcs, MProjs, MItems, MRevenge;
    static object Revenge;
    static void InvokeIn(string key, MethodInfo m, object o)
    {
        var d = (IDisposable)MSwap.Invoke(null, new object[] { key });
        try { Unwrap(m, o, new object[0]); }
        finally { d.Dispose(); }
    }
    static void EntityTick()
    {
        var outer = (IDisposable)MSwap.Invoke(null, new object[] { "DoUpdateInWorld" });
        try
        {
            FieldInfo guc = TMain.GetField("_gameUpdateCount", AllStatic);
            guc.SetValue(null, (uint)guc.GetValue(null) + 1);
            bool[] solid = (bool[])SF(TMain, "tileSolid");
            solid[379] = false;
            InvokeIn("UpdatePlayers", MPlayers, MainInst);
            Unwrap(MRevenge, Revenge, new object[0]);
            InvokeIn("UpdateNPCs", MNpcs, MainInst);
            InvokeIn("UpdateProjectiles", MProjs, MainInst);
            InvokeIn("UpdateItems", MItems, MainInst);
            solid[379] = true;
        }
        finally { outer.Dispose(); }
    }

    static void DumpRand()
    {
        uint inext; int[] sa;
        RandState(SF(TMain, "rand"), out inext, out sa);
        BW.Write(inext); BW.Write(FnvInts(sa));
        var rngs = (IDictionary)TMain.GetField("_rngs", AllStatic).GetValue(null);
        var keys = rngs.Keys.Cast<string>().OrderBy(x => x, StringComparer.Ordinal).ToList();
        BW.Write((ushort)keys.Count);
        foreach (string k in keys) { RandState(rngs[k], out inext, out sa); WStr(k); BW.Write(inext); BW.Write(FnvInts(sa)); }
    }
    static ulong FnvInts(int[] a) { ulong h = FnvBasis; foreach (int x in a) h = FnvBytes(h, BitConverter.GetBytes(x)); return h; }

    static void DumpEntities()
    {
        for (int c = 0; c < ClassArrays.Length; c++)
        {
            Array arr = (Array)SF(TMain, ClassArrays[c]);
            var act = new List<int>();
            for (int i = 0; i < ClassCounts[c]; i++)
            {
                object o = arr.GetValue(i);
                if (o != null && (bool)IF(o, "active")) act.Add(i);
            }
            BW.Write((ushort)act.Count);
            foreach (int i in act)
            {
                ulong[] cur = ReadLeaves(Schemas[c], arr.GetValue(i));
                ulong[] prev;
                if (!Last[c].TryGetValue(i, out prev)) prev = new ulong[cur.Length];
                var ch = new List<int>();
                for (int k = 0; k < cur.Length; k++) if (cur[k] != prev[k]) ch.Add(k);
                BW.Write((ushort)i); BW.Write((ushort)ch.Count);
                foreach (int k in ch) { BW.Write((ushort)k); BW.Write(cur[k]); }
                Last[c][i] = cur;
            }
        }
    }

    static Snap Loaded;
    static FieldInfo[] TileFields;
    // tiles that differ from the loaded world: counted, hashed (x, y, old record, new record), then restored
    static void TilesBack(out int changed, out ulong hash)
    {
        Snap now = TakeSnapshot(null);
        changed = 0; hash = FnvBasis;
        Array tile = (Array)SF(TMain, "tile");
        int h = now.H;
        for (long i = 0; i < (long)now.W * h; i++)
        {
            long o = i * RecSize;
            bool same = true;
            for (int k = 0; k < RecSize; k++) if (now.Buf[o + k] != Loaded.Buf[o + k]) { same = false; break; }
            if (same) continue;
            changed++;
            int x = (int)(i / h), y = (int)(i % h);
            hash = FnvBytes(hash, BitConverter.GetBytes(x)); hash = FnvBytes(hash, BitConverter.GetBytes(y));
            for (int k = 0; k < RecSize; k++) { hash ^= now.Buf[o + k]; hash *= FnvPrime; }
            object t = tile.GetValue(x, y);
            byte[] r = Loaded.Buf;
            Func<int, int> u16 = off => r[o + off] | (r[o + off + 1] << 8);
            TileFields[0].SetValue(t, (ushort)u16(0));
            TileFields[1].SetValue(t, (ushort)u16(2));
            TileFields[2].SetValue(t, r[o + 4]);
            TileFields[3].SetValue(t, (ushort)u16(5));
            TileFields[4].SetValue(t, r[o + 7]);
            TileFields[5].SetValue(t, r[o + 8]);
            TileFields[6].SetValue(t, r[o + 9]);
            TileFields[7].SetValue(t, (short)u16(10));
            TileFields[8].SetValue(t, (short)u16(12));
        }
    }

    static void EntRun(string kind, int ticks, string scratch, int maxBatches)
    {
        if (kind != "npc" && kind != "proj" && kind != "item" && kind != "citem") throw new Exception("kind must be npc, proj, item or citem");
        var total = Stopwatch.StartNew();
        string world = Path.Combine(Dir(Path.Combine(scratch, "Worlds")), "ref_small.wld");
        File.Copy(Path.Combine(Root, "work", "refsave", "Worlds", "ref_small.wld"), world, true);
        string srcSha = Sha256(File.ReadAllBytes(world));
        InitGame(() =>
        {
            Step("-world <existing>: WorldFile.GetAllMetadata(path, false).SetAsActive()", () =>
            {
                object wfd = null;
                foreach (MethodInfo m in TWorldFile.GetMethods(AllStatic))
                    if (m.Name == "GetAllMetadata" && m.GetParameters().Length == 2) wfd = Unwrap(m, null, new object[] { world, false });
                wfd.GetType().GetMethod("SetAsActive").Invoke(wfd, null);
            });
        }, false);
        Step("Task.Factory.StartNew(WorldGen.serverLoadWorldCallBack) with that thread's Main.rand = new UnifiedRandom(1357)", () =>
        {
            Exception err = null;
            Task.Factory.StartNew(() => { try { SSet(TMain, "rand", NewRand(LoadSeedE)); CallStatic(TWorldGen, "serverLoadWorldCallBack"); } catch (Exception e) { err = e; } }).Wait();
            if (err != null) throw new Exception("load failed: " + err);
            if ((bool)SF(TWorldGen, "loadFailed")) throw new Exception("WorldGen.loadFailed");
        });
        Step("server state: myPlayer = 255; netMode = 2; Netplay.Disconnect = false; Clients[i] = new RemoteClient", () =>
        {
            Type netplay = T("Terraria.Netplay"), rcT = T("Terraria.RemoteClient");
            SSet(TMain, "myPlayer", 255); SSet(TMain, "netMode", 2); SSet(netplay, "Disconnect", false);
            Array clients = (Array)SF(netplay, "Clients");
            for (int i = 0; i < 256; i++)
            {
                object c = Activator.CreateInstance(rcT);
                rcT.GetMethod("Reset").Invoke(c, null);
                rcT.GetField("Id").SetValue(c, i);
                rcT.GetField("ReadBuffer").SetValue(c, new byte[1024]);
                clients.SetValue(c, i);
            }
        });
        SSet(TMain, "gameMenu", false);

        TNPC = T("Terraria.NPC"); TProj = T("Terraria.Projectile"); TPlayer = T("Terraria.Player"); TWorldItem = T("Terraria.WorldItem"); TTile = T("Terraria.Tile");
        TileFields = new[] { "type", "wall", "liquid", "sTileHeader", "bTileHeader", "bTileHeader2", "bTileHeader3", "frameX", "frameY" }.Select(n => TTile.GetField(n)).ToArray();
        MSwap = TMain.GetMethod("SwapRandom", AllStatic);
        MPlayers = TMain.GetMethod("UpdateWorld_Players", AllInst);
        MNpcs = TMain.GetMethod("UpdateWorld_NPCs", AllInst);
        MProjs = TMain.GetMethod("UpdateWorld_Projectiles", AllInst);
        MItems = TMain.GetMethod("UpdateWorld_Items", AllInst);
        Revenge = SF(TNPC, "RevengeManager");
        MRevenge = Revenge.GetType().GetMethod("Update", Type.EmptyTypes);
        Schemas = new[] { MakeSchema(TNPC, Activator.CreateInstance(TNPC), null), MakeSchema(TProj, Activator.CreateInstance(TProj), null),
                          MakeSchema(TPlayer, Activator.CreateInstance(TPlayer), null), MakeSchema(TWorldItem, Activator.CreateInstance(TWorldItem), new[] { "type", "stack" }) };
        Loaded = TakeSnapshot(null);

        if (kind == "citem")
        {
            // client-side item use (ClientItems.cs): the item types with a use style, as the `item` kind finds them
            Type TItemC = T("Terraria.Item");
            MethodInfo defC = TItemC.GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int));
            int countC = Convert.ToInt32(SF(T("Terraria.ID.ItemID"), "Count"));
            var usableC = new List<int>();
            for (int type = 1; type < countC; type++)
            {
                object it = Activator.CreateInstance(TItemC);
                Unwrap(defC, it, new object[] { type, null });
                if ((int)IF(it, "useStyle") != 0) usableC.Add(type);
            }
            CItemRun(ticks, maxBatches, srcSha, total, usableC.ToArray());
            return;
        }
        // ---- types and batches
        int count = Convert.ToInt32(SF(kind == "npc" ? T("Terraria.ID.NPCID") : kind == "proj" ? T("Terraria.ID.ProjectileID") : T("Terraria.ID.ItemID"), "Count"));
        Type TItem = T("Terraria.Item");
        MethodInfo itemDefaults = TItem.GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int));
        int usable = 0;
        var batches = new List<List<Slot>>();
        var cur = new List<Slot>();
        int[] groundX = new int[SlotCount], groundE = new int[SlotCount], groundP = new int[SlotCount];
        for (int k = 0; k < SlotCount; k++) { groundX[k] = SlotX0 + k * SlotDX; groundE[k] = Ground(groundX[k]); groundP[k] = Ground(groundX[k] + PlayerDX); }
        Func<int, Slot> slotFor = (k) => new Slot { Ex = groundX[k], Ey = groundE[k], Px = groundX[k] + PlayerDX, Py = groundP[k], Pl = k };
        for (int type = 1; type < count; type++)
        {
            // a boss gets a batch of its own: new NPC().SetDefaults(type) on a template (the port does the same calls)
            bool solo = false;
            if (kind == "npc")
            {
                object n = Activator.CreateInstance(TNPC);
                Unwrap(TNPC.GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int)), n, new object[] { type, Activator.CreateInstance(T("Terraria.NPCSpawnParams")) });
                solo = (bool)IF(n, "boss");
            }
            // items: only those with a use style (new Item().SetDefaults(type, null) on a template; the port does the same)
            if (kind == "item")
            {
                object it = Activator.CreateInstance(TItem);
                Unwrap(itemDefaults, it, new object[] { type, null });
                if ((int)IF(it, "useStyle") == 0) continue;
                usable++;
            }
            if (solo)
            {
                Slot s = slotFor(SlotCount / 2); s.Type = type;
                batches.Add(new List<Slot> { s });
                continue;
            }
            if (cur.Count == SlotCount) { batches.Add(cur); cur = new List<Slot>(); }
            Slot sl = slotFor(cur.Count); sl.Type = type;
            if (kind == "proj") { sl.Vx = 5f; sl.Vy = -2f; sl.Owner = 255; }
            cur.Add(sl);
        }
        if (cur.Count > 0) batches.Add(cur);
        // a solo slot uses the middle slot's position and that slot's player index
        if (batches.Count > maxBatches) batches.RemoveRange(maxBatches, batches.Count - maxBatches);
        Console.WriteLine(kind + ": " + (count - 1) + " types" + (kind == "item" ? " (" + usable + " with a use style)" : "") + " in " + batches.Count + " batches, " + ticks + " ticks each");

        // ---- header
        var zo = new ZOut(Path.Combine(EOut, kind + ".ents.zlib"));
        BW = new BinaryWriter(zo);
        BW.Write(Encoding.ASCII.GetBytes("ENTS")); BW.Write(1u);
        WStr(kind); BW.Write(ticks); BW.Write(batches.Count);
        BW.Write(Schemas.Length);
        for (int c = 0; c < Schemas.Length; c++)
        {
            WStr(ClassArrays[c]);
            BW.Write(Schemas[c].Leaves.Count);
            foreach (Leaf l in Schemas[c].Leaves) { WStr(l.Name); BW.Write(l.Kind); }
        }

        Array players = (Array)SF(TMain, "player"), items = (Array)SF(TMain, "item");
        int[] noReuse = (int[])SF(TMain, "timeItemSlotCannotBeReusedFor");
        Type ctxT = T("Terraria.PlayerSpawnContext");
        object spawnInto = Enum.Parse(ctxT, "SpawningIntoWorld");
        MethodInfo newNpc = TNPC.GetMethod("NewNPC", AllStatic);
        MethodInfo newProj = TProj.GetMethods(AllStatic).First(m => m.Name == "NewProjectile" && m.GetParameters()[1].ParameterType.Name == "Vector2");
        Type v2 = newProj.GetParameters()[1].ParameterType;
        var summaryBatches = new StringBuilder();
        var errors = new List<string>();
        var tickClock = Stopwatch.StartNew();
        for (int b = 0; b < batches.Count; b++)
        {
            List<Slot> slots = batches[b];
            // 1-4: reset
            int tc; ulong th;
            TilesBack(out tc, out th);
            CallStatic(TNPC, "ClearAll"); CallStatic(TProj, "ClearAll");
            for (int i = 0; i < 400; i++) { object wi = Activator.CreateInstance(TWorldItem); ISet(wi, "whoAmI", i); items.SetValue(wi, i); noReuse[i] = 0; }
            for (int i = 0; i < SlotCount; i++) { object p = Activator.CreateInstance(TPlayer); ISet(p, "whoAmI", i); players.SetValue(p, i); }
            SSet(TMain, "rand", NewRand(EntSeedBase + b));
            TMain.GetField("_rngs", AllStatic).SetValue(null, Activator.CreateInstance(TMain.GetField("_rngs", AllStatic).FieldType));
            Last = new[] { new Dictionary<int, ulong[]>(), new Dictionary<int, ulong[]>(), new Dictionary<int, ulong[]>(), new Dictionary<int, ulong[]>() };
            // 5: players
            for (int k = 0; k < slots.Count; k++)
            {
                int pi = slots[k].Pl;
                object p = players.GetValue(pi);
                IF_SetE(p, "active", true); IF_SetE(p, "name", "Ref"); IF_SetE(p, "whoAmI", pi);
                IF_SetE(p, "statLifeMax", 100); IF_SetE(p, "statLifeMax2", 100); IF_SetE(p, "statLife", 100);
                IF_SetE(p, "statManaMax", 20); IF_SetE(p, "statManaMax2", 20); IF_SetE(p, "statMana", 20);
                IF_SetE(p, "SpawnX", slots[k].Px); IF_SetE(p, "SpawnY", slots[k].Py);
                Unwrap(TPlayer.GetMethod("Spawn"), p, new object[] { spawnInto });
            }
            // 6: entities
            for (int k = 0; k < slots.Count; k++)
            {
                Slot s = slots[k];
                try
                {
                    if (kind == "item")
                    {
                        // the slot's player holds the item in inventory slot 0 (the selected slot of a new Player)
                        object p = players.GetValue(s.Pl);
                        object it = ((Array)IF(p, "inventory")).GetValue(0);
                        Unwrap(itemDefaults, it, new object[] { s.Type, null });
                        s.Index = 0;
                    }
                    else if (kind == "npc")
                        s.Index = (int)Unwrap(newNpc, null, new object[] { Activator.CreateInstance(T("Terraria.DataStructures.EntitySource_SpawnNPC")), s.Ex * 16 + 8, s.Ey * 16, s.Type, 0, 0f, 0f, 0f, 0f, 255 });
                    else
                        s.Index = (int)Unwrap(newProj, null, new object[] { Activator.CreateInstance(T("Terraria.DataStructures.EntitySource_DebugCommand")),
                            Activator.CreateInstance(v2, new object[] { (float)(s.Ex * 16 + 8), (float)((s.Ey - 3) * 16) }), Activator.CreateInstance(v2, new object[] { s.Vx, s.Vy }),
                            s.Type, 20, 2f, s.Owner, 0f, 0f, 0f, null });
                }
                catch (Exception e) { s.Index = -2; s.Error = e.Message; errors.Add(kind + " " + s.Type + ": spawn threw: " + e.Message); }
            }
            // batch record
            BW.Write(b); BW.Write(EntSeedBase + b); BW.Write(tc); BW.Write(th); BW.Write(slots.Count);
            foreach (Slot s in slots) { BW.Write(s.Type); BW.Write(s.Ex); BW.Write(s.Ey); BW.Write(s.Px); BW.Write(s.Py); BW.Write(s.Pl); BW.Write(s.Owner); BW.Write(s.Vx); BW.Write(s.Vy); BW.Write(s.Index); }
            string tickErr = null;
            for (int t = 0; t <= ticks; t++)
            {
                if (t > 0)
                {
                    // items: the remote player's client sends "use item" every tick, facing right (the target side)
                    if (kind == "item") foreach (Slot s in slots) { object p = players.GetValue(s.Pl); IF_SetE(p, "controlUseItem", true); IF_SetE(p, "direction", 1); }
                    try { EntityTick(); }
                    catch (Exception e) { tickErr = "tick " + t + ": " + (e.InnerException ?? e).GetType().Name + ": " + e.Message; }
                }
                BW.Write(t); BW.Write((byte)(tickErr == null ? 0 : 1));
                if (tickErr != null) { errors.Add(kind + " batch " + b + ": " + tickErr); break; }
                DumpRand();
                DumpEntities();
            }
            if (summaryBatches.Length > 0) summaryBatches.Append(",\n");
            summaryBatches.Append("{\"batch\":" + b + ",\"tiles_restored\":" + tc + ",\"types\":[" + string.Join(",", slots.Select(s => s.Type.ToString(Inv)).ToArray()) +
                "],\"index\":[" + string.Join(",", slots.Select(s => s.Index.ToString(Inv)).ToArray()) + "]" + (tickErr != null ? ",\"tick_error\":" + Q(tickErr) : "") + "}");
            if (b % 10 == 0) Console.WriteLine("batch " + b + " of " + batches.Count + " (" + tickClock.ElapsedMilliseconds + " ms)");
        }
        int tc2; ulong th2;
        TilesBack(out tc2, out th2);
        BW.Write(-1); BW.Write(tc2); BW.Write(th2);
        BW.Close();

        var sj = new StringBuilder("{\n");
        sj.Append("\"generator\":\"tools/refdump_entities (RefDumpEntities.exe " + kind + ")\",\n");
        sj.Append("\"kind\":" + Q(kind) + ",\"ticks\":" + ticks + ",\"batches\":" + batches.Count + ",\"types\":" + (count - 1) + ",\n");
        sj.Append("\"world_sha256\":" + Q(srcSha) + ",\"seed_base\":" + EntSeedBase + ",\"bytes_uncompressed\":" + zo.Total + ",\n");
        sj.Append("\"slots\":{\"count\":" + SlotCount + ",\"x0\":" + SlotX0 + ",\"dx\":" + SlotDX + ",\"player_dx\":" + PlayerDX + "},\n");
        for (int c = 0; c < Schemas.Length; c++)
            sj.Append("\"leaves_" + ClassArrays[c] + "\":" + Schemas[c].Leaves.Count + ",\"not_leaves_" + ClassArrays[c] + "\":[" + string.Join(",", Schemas[c].Skipped.Select(Q).ToArray()) + "],\n");
        sj.Append("\"errors\":[" + string.Join(",", errors.Select(Q).ToArray()) + "],\n");
        sj.Append("\"notes\":[" + string.Join(",", ENotes.Select(Q).ToArray()) + "],\n");
        sj.Append("\"batch_list\":[\n" + summaryBatches + "\n]\n}\n");
        File.WriteAllText(Path.Combine(EOut, kind + ".summary.json"), sj.ToString(), new UTF8Encoding(false));
        File.WriteAllText(Path.Combine(EOut, kind + ".timing.json"), "{\"tick_ms_total\":" + tickClock.ElapsedMilliseconds + ",\"total_ms\":" + total.ElapsedMilliseconds + "}\n", new UTF8Encoding(false));
        Console.WriteLine(kind + ": done, " + batches.Count + " batches, " + zo.Total + " bytes uncompressed, " + errors.Count + " errors, " + total.ElapsedMilliseconds + " ms");
        foreach (string e in errors) Console.WriteLine("  " + e);
    }

    static void IF_SetE(object o, string name, object v)
    {
        for (Type t = o.GetType(); t != null; t = t.BaseType)
        {
            FieldInfo f = t.GetField(name, AllInst | BindingFlags.DeclaredOnly);
            if (f != null) { f.SetValue(o, f.FieldType.IsPrimitive ? Convert.ChangeType(v, f.FieldType) : v); return; }
        }
        throw new Exception("no field " + name);
    }
}
