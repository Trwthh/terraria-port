// RefDump "framed" mode: runs the real game's tile framing over a real world file, headless, and dumps the result.
//
//   RefDump.exe framed <world.wld> <outdir> [scramble]
//
// What it does, in order (every step is the game's own code, called by reflection):
//   1. Same initialisation as the default mode (InitGame: Program.SavePath, dedServ, language, new Main(), Lang, Main.rand, main.Initialize()).
//   2. Copies <world.wld> to work\framed-scratch\ (the original is never opened for writing), sets Main.worldPathName and
//      Main.ActiveWorldFileData = WorldFile.GetAllMetadata(path, false), calls WorldFile.LoadWorld() [WorldFile.cs:650].
//      LoadWorld also runs its post-load fix-ups (CheckSavedOreTiers, ClearTempTiles, liquid settle, WaterCheck). The state
//      right after it is dumped as the "pre" state (sha256 + one FNV-1a 64 hash per column) so the Rust test can prove it
//      started from the same tiles.
//   3. Flags for the framing run:
//        Main.netMode = 1            -> WorldGen.TileFrame skips Liquid.AddWater (WorldGen.cs:82255 `Main.netMode != 1`) and
//                                       SpawnFallingBlockProjectile returns false at once (WorldGen.cs:86956), so falling sand
//                                       is framed instead of turned into projectiles.
//        WorldGen.generatingWorld = false (its default) -> TileFrame calls TileFrameCosmetic (WorldGen.cs:82291) and
//                                       Reframe uses TileFrame(.., noBreak: true) (WorldGen.cs:81245).
//        noBreak = true (set by Reframe) -> TileFrameImportant runs only for torches and echo-culled tiles (WorldGen.cs:82266).
//      Main.rand = new UnifiedRandom(FramedSeed)  (WorldGen.genRand is `=> Main.rand`, WorldGen.cs:4389).
//   4. ONE call: WorldGen.SectionTileFrame(0, 0, Main.maxTilesX / 200 - 1, Main.maxTilesY / 150 - 1) [WorldGen.cs:81254].
//      That is Reframe(i, j, resetFrame: true) = TileFrame(i, j, true, noBreak: true) then Framing.WallFrame(i, j, true), for
//      i = tileStartX-1 .. tileEndX (exclusive tileEndX+1), j = tileStartY-1 .. tileEndY, column by column (i outer, j inner),
//      with tileStartX = tileStartY = 1 and tileEndX = maxTilesX-2, tileEndY = maxTilesY-2, so i in 0..maxTilesX-2, j in 0..maxTilesY-2.
//   5. Dumps every tile (column-major, x outer, y inner) and the next Main.rand.Next() value.
//
// Output files (outdir):
//   <name>.post.bin   header: "TFRM" | u32 version=1 | i32 width | i32 height | u32 recordSize=14, then width*height records:
//                     u16 type | u16 wall | u8 liquid | u16 sTileHeader | u8 bTileHeader | u8 bTileHeader2 | u8 bTileHeader3 |
//                     i16 frameX | i16 frameY      (little endian, exactly the fields of Terraria.Tile)
//   <name>.liquid.bin the liquid state after LoadWorld's liquid settle (the settle is not ported): 8-byte records for every tile
//                     that has any: i32 x*height+y | u8 liquid | u8 bTileHeader&0x60 | u8 bTileHeader3&0x18 | u8 0
//   <name>.pre.bin    only with REFDUMP_KEEP_PRE=1: the whole state right after LoadWorld, same layout as post.bin
//   <name>.pre_columns.bin   width * u64 (FNV-1a 64 of the 14-byte records of each column, in the same layout) of the pre state
//   <name>.json       counts, seed, flags, sha256 of post.bin, sha256 of the pre state, the next random value, timings
using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Linq.Expressions;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Threading;

static partial class RefDump
{
    // Seed of Main.rand / WorldGen.genRand for the framing run. Fixed so the dump is repeatable.
    const int FramedSeed = 424242;
    const int FramedRecordSize = 14;

    static int MainFramed(string[] args)
    {
        if (args.Length < 3) { Console.Error.WriteLine("usage: RefDump.exe framed <world.wld> <outdir>"); return 2; }
        // tools\refdump\bin\RefDump.exe -> project root three levels up
        string root = Path.GetFullPath(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", ".."));
        GameDir = Path.Combine(root, "work", "refserver");
        SaveDir = Path.Combine(root, "work", "refdump-save");
        OutDir = Path.GetFullPath(args[2]);
        string worldSrc = Path.GetFullPath(args[1]);
        Directory.CreateDirectory(SaveDir);
        Directory.CreateDirectory(OutDir);

        var wd = new Thread(() => { Thread.Sleep(540000); Console.Error.WriteLine("WATCHDOG: timeout"); Environment.Exit(3); });
        wd.IsBackground = true; wd.Start();

        int rc = 0;
        try { RunFramed(root, worldSrc, args.Length > 3 && args[3] == "scramble"); }
        catch (Exception e) { Console.Error.WriteLine("FATAL: " + e); rc = 1; }
        Console.Out.Flush();
        Environment.Exit(rc);
        return rc;
    }

    // Typed readers for every field of Terraria.Tile, compiled once (5 million tiles x 9 fields through FieldInfo would be slow).
    class TileReader
    {
        public Func<Array, int, int, ushort> Type, Wall, STileHeader;
        public Func<Array, int, int, byte> Liquid, BTileHeader, BTileHeader2, BTileHeader3;
        public Func<Array, int, int, short> FrameX, FrameY;
        public Func<Array, int, int, bool> Null;
    }

    static TileReader MakeTileReader()
    {
        Type tileT = T("Terraria.Tile");
        Type arrT = tileT.MakeArrayType(2);
        ParameterExpression pa = Expression.Parameter(typeof(Array), "a"), px = Expression.Parameter(typeof(int), "x"), py = Expression.Parameter(typeof(int), "y");
        Expression tile = Expression.ArrayAccess(Expression.Convert(pa, arrT), px, py);
        var r = new TileReader();
        r.Type = Expression.Lambda<Func<Array, int, int, ushort>>(Expression.Field(tile, "type"), pa, px, py).Compile();
        r.Wall = Expression.Lambda<Func<Array, int, int, ushort>>(Expression.Field(tile, "wall"), pa, px, py).Compile();
        r.STileHeader = Expression.Lambda<Func<Array, int, int, ushort>>(Expression.Field(tile, "sTileHeader"), pa, px, py).Compile();
        r.Liquid = Expression.Lambda<Func<Array, int, int, byte>>(Expression.Field(tile, "liquid"), pa, px, py).Compile();
        r.BTileHeader = Expression.Lambda<Func<Array, int, int, byte>>(Expression.Field(tile, "bTileHeader"), pa, px, py).Compile();
        r.BTileHeader2 = Expression.Lambda<Func<Array, int, int, byte>>(Expression.Field(tile, "bTileHeader2"), pa, px, py).Compile();
        r.BTileHeader3 = Expression.Lambda<Func<Array, int, int, byte>>(Expression.Field(tile, "bTileHeader3"), pa, px, py).Compile();
        r.FrameX = Expression.Lambda<Func<Array, int, int, short>>(Expression.Field(tile, "frameX"), pa, px, py).Compile();
        r.FrameY = Expression.Lambda<Func<Array, int, int, short>>(Expression.Field(tile, "frameY"), pa, px, py).Compile();
        r.Null = Expression.Lambda<Func<Array, int, int, bool>>(Expression.Equal(tile, Expression.Constant(null, tileT)), pa, px, py).Compile();
        return r;
    }

    static void Put16(byte[] b, int o, int v) { b[o] = (byte)v; b[o + 1] = (byte)(v >> 8); }
    static void Put32(byte[] b, int o, int v) { Put16(b, o, v); Put16(b, o + 2, v >> 16); }

    // Serialises Main.tile in the layout described at the top of this file. Returns (records, per-column FNV-1a 64 hashes).
    static byte[] SnapshotTiles(TileReader rd, int w, int h, out ulong[] colHash, out int nullTiles)
    {
        Array tile = (Array)SF(T("Terraria.Main"), "tile");
        byte[] buf = new byte[(long)w * h * FramedRecordSize];
        colHash = new ulong[w];
        nullTiles = 0;
        long o = 0;
        for (int x = 0; x < w; x++)
        {
            long start = o;
            for (int y = 0; y < h; y++)
            {
                if (rd.Null(tile, x, y)) { nullTiles++; o += FramedRecordSize; continue; }
                int p = (int)o;
                Put16(buf, p, rd.Type(tile, x, y));
                Put16(buf, p + 2, rd.Wall(tile, x, y));
                buf[p + 4] = rd.Liquid(tile, x, y);
                Put16(buf, p + 5, rd.STileHeader(tile, x, y));
                buf[p + 7] = rd.BTileHeader(tile, x, y);
                buf[p + 8] = rd.BTileHeader2(tile, x, y);
                buf[p + 9] = rd.BTileHeader3(tile, x, y);
                Put16(buf, p + 10, rd.FrameX(tile, x, y));
                Put16(buf, p + 12, rd.FrameY(tile, x, y));
                o += FramedRecordSize;
            }
            ulong hash = 14695981039346656037UL;
            for (long k = start; k < o; k++) { hash ^= buf[k]; hash *= 1099511628211UL; }
            colHash[x] = hash;
        }
        return buf;
    }

    static string Sha(byte[] b) { return Hex(SHA256.Create().ComputeHash(b)); }

    static void RunFramed(string root, string worldSrc, bool scramble)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        InitGame();
        Type main = T("Terraria.Main"), worldGen = T("Terraria.WorldGen"), worldFile = T("Terraria.IO.WorldFile");

        // 2. scratch copy + the game's own LoadWorld
        string scratchDir = Path.Combine(root, "work", "framed-scratch");
        Directory.CreateDirectory(scratchDir);
        string scratch = Path.Combine(scratchDir, Path.GetFileName(worldSrc));
        File.Copy(worldSrc, scratch, true);
        string srcSha = Hex(SHA256.Create().ComputeHash(File.ReadAllBytes(scratch)));
        Step("Main.ActiveWorldFileData = WorldFile.GetAllMetadata(scratch copy, false)   (Main.worldPathName => ActiveWorldFileData.Path)", () =>
        {
            object wfd = null;
            foreach (MethodInfo m in worldFile.GetMethods(AllStatic))
                if (m.Name == "GetAllMetadata" && m.GetParameters().Length == 2) wfd = Unwrap(m, null, new object[] { scratch, false });
            if (wfd == null) throw new Exception("GetAllMetadata returned null (world unreadable)");
            SSet(main, "ActiveWorldFileData", wfd);
        });
        Step("WorldFile.LoadWorld()   [WorldFile.cs:650]", () =>
        {
            CallStatic(worldFile, "LoadWorld");
            if ((bool)SF(worldGen, "loadFailed"))
            {
                object ex = null;
                try { ex = SF(worldFile, "LastThrownLoadException"); } catch { }
                throw new Exception("WorldGen.loadFailed after LoadWorld: " + ex);
            }
        });
        int w = (int)SF(main, "maxTilesX"), h = (int)SF(main, "maxTilesY");
        Console.WriteLine("loaded " + w + " x " + h + " in " + sw.ElapsedMilliseconds + " ms");

        if (scramble) ScrambleRegion(main, w, h);
        TileReader rd = MakeTileReader();
        ulong[] preCols; int preNull;
        byte[] pre = SnapshotTiles(rd, w, h, out preCols, out preNull);
        string preSha = Sha(pre);
        Console.WriteLine("pre state sha256 " + preSha + " nulls " + preNull);

        // 3. flags and seed
        // WorldGen.ClearWorld creates the section manager when netMode != 2 (WorldGen.cs:7125-7128); LoadWorld does not
        Step("Main.sectionManager = new WorldSections(maxTilesX / 200, maxTilesY / 150)   [WorldGen.cs:7127]", () =>
        {
            SSet(main, "sectionManager", Activator.CreateInstance(T("Terraria.WorldSections"), new object[] { w / 200, h / 150 }));
        });
        Step("Main.netMode = 1; WorldGen.generatingWorld = false", () => { SSet(main, "netMode", 1); SSet(worldGen, "generatingWorld", false); });
        Step("Main.rand = new UnifiedRandom(" + FramedSeed + ")", () =>
        {
            SSet(main, "rand", Activator.CreateInstance(T("Terraria.Utilities.UnifiedRandom"), new object[] { FramedSeed }));
        });

        // 4. the framing run
        int sx = w / 200 - 1, sy = h / 150 - 1;
        var t2 = System.Diagnostics.Stopwatch.StartNew();
        Step("WorldGen.SectionTileFrame(0, 0, " + sx + ", " + sy + ")   [WorldGen.cs:81254]", () => CallStatic(worldGen, "SectionTileFrame", 0, 0, sx, sy));
        long frameMs = t2.ElapsedMilliseconds;

        // 5. dump
        object rand = SF(main, "rand");
        MethodInfo next0 = rand.GetType().GetMethod("Next", Type.EmptyTypes);
        int nextRand = (int)next0.Invoke(rand, null);
        ulong[] postCols; int postNull;
        byte[] post = SnapshotTiles(rd, w, h, out postCols, out postNull);
        string name = Path.GetFileNameWithoutExtension(worldSrc) + (scramble ? "_scrambled" : "");
        string postPath = Path.Combine(OutDir, name + ".post.bin");
        using (var fs = new FileStream(postPath, FileMode.Create))
        {
            byte[] head = new byte[20];
            head[0] = (byte)'T'; head[1] = (byte)'F'; head[2] = (byte)'R'; head[3] = (byte)'M';
            Put32(head, 4, 1); Put32(head, 8, w); Put32(head, 12, h); Put32(head, 16, FramedRecordSize);
            fs.Write(head, 0, head.Length);
            fs.Write(post, 0, post.Length);
        }
        if (Environment.GetEnvironmentVariable("REFDUMP_KEEP_PRE") == "1")   // debugging aid: the whole pre state, same layout as post.bin
            using (var fs = new FileStream(Path.Combine(OutDir, name + ".pre.bin"), FileMode.Create))
            {
                byte[] head = new byte[20];
                head[0] = (byte)'T'; head[1] = (byte)'F'; head[2] = (byte)'R'; head[3] = (byte)'M';
                Put32(head, 4, 1); Put32(head, 8, w); Put32(head, 12, h); Put32(head, 16, FramedRecordSize);
                fs.Write(head, 0, head.Length);
                fs.Write(pre, 0, pre.Length);
            }
        // liquid state after LoadWorld's liquid settle: one 8-byte record per tile that has any: i32 index (x * height + y) |
        // u8 liquid | u8 bTileHeader & 0x60 (liquid type) | u8 bTileHeader3 & 0x18 (checkingLiquid, skipLiquid) | u8 0
        using (var fs = new FileStream(Path.Combine(OutDir, name + ".liquid.bin"), FileMode.Create))
            for (long t = 0; t < (long)w * h; t++)
            {
                int o = (int)(t * FramedRecordSize);
                byte liq = pre[o + 4], lt = (byte)(pre[o + 7] & 0x60), fl = (byte)(pre[o + 9] & 0x18);
                if (liq == 0 && lt == 0 && fl == 0) continue;
                fs.Write(BitConverter.GetBytes((int)t), 0, 4);
                fs.Write(new byte[] { liq, lt, fl, 0 }, 0, 4);
            }
        using (var fs = new FileStream(Path.Combine(OutDir, name + ".pre_columns.bin"), FileMode.Create))
            foreach (ulong c in preCols) fs.Write(BitConverter.GetBytes(c), 0, 8);
        string postSha = Hex(SHA256.Create().ComputeHash(File.ReadAllBytes(postPath)));

        // counts
        long changed = 0, typeChanged = 0, activeChanged = 0, framedNeg = 0;
        bool[] important = (bool[])SF(main, "tileFrameImportant");
        for (long t = 0; t < (long)w * h; t++)
        {
            int o = (int)(t * FramedRecordSize);
            bool same = true;
            for (int k = 0; k < FramedRecordSize; k++) if (pre[o + k] != post[o + k]) { same = false; break; }
            if (!same) changed++;
            if (pre[o] != post[o] || pre[o + 1] != post[o + 1]) typeChanged++;
            if (((pre[o + 5] ^ post[o + 5]) & 0x20) != 0) activeChanged++;
            bool active = (post[o + 5] & 0x20) != 0;
            int type = post[o] | (post[o + 1] << 8);
            if (active && !important[type])
            {
                short fx = (short)(post[o + 10] | (post[o + 11] << 8)), fy = (short)(post[o + 12] | (post[o + 13] << 8));
                if (fx < 0 || fy < 0) framedNeg++;
            }
        }

        var json = new StringBuilder("{\n");
        json.Append("\"generator\":\"tools/refdump/RefDumpFramed.cs (RefDump.exe framed)\",\n");
        json.Append("\"scrambled\":" + (scramble ? "true" : "false") + ",\n");
        json.Append("\"world\":" + Q(Path.GetFileName(worldSrc)) + ",\"worldSha256\":" + Q(srcSha) + ",\n");
        json.Append("\"width\":" + w + ",\"height\":" + h + ",\"tiles\":" + ((long)w * h) + ",\"recordSize\":" + FramedRecordSize + ",\n");
        json.Append("\"seed\":" + FramedSeed + ",\n");
        json.Append("\"flags\":{\"Main.netMode\":1,\"WorldGen.generatingWorld\":false,\"noBreak\":true},\n");
        json.Append("\"call\":\"WorldGen.SectionTileFrame(0, 0, " + sx + ", " + sy + ")\",\n");
        json.Append("\"loopOrder\":\"i outer, j inner; i = 0..maxTilesX-2, j = 0..maxTilesY-2; Reframe(i, j, true) = TileFrame(i, j, true, true) then Framing.WallFrame(i, j, true)\",\n");
        json.Append("\"preSha256\":" + Q(preSha) + ",\"postSha256\":" + Q(postSha) + ",\n");
        json.Append("\"nextRandAfterRun\":" + nextRand.ToString(Inv) + ",\n");
        json.Append("\"tilesChangedByRun\":" + changed + ",\"tilesWithTypeChanged\":" + typeChanged + ",\"tilesWithActiveChanged\":" + activeChanged + ",\n");
        json.Append("\"activeNonImportantTilesStillNegativeFrame\":" + framedNeg + ",\n");
        json.Append("\"nullTilesPre\":" + preNull + ",\"nullTilesPost\":" + postNull + ",\n");
        json.Append("\"frameRunMs\":" + frameMs + ",\"totalMs\":" + sw.ElapsedMilliseconds + "\n}\n");
        File.WriteAllBytes(Path.Combine(OutDir, name + ".json"), new UTF8Encoding(false).GetBytes(json.ToString()));
        Console.WriteLine(json.ToString());
    }

    // `scramble` mode: the reference world only contains the tile types a small normal world has. To exercise every other
    // type, slope and half brick the harness first overwrites a rectangle with a deterministic pseudo-random pattern (the
    // Rust test applies the identical pattern, see `scramble` in tests/tile_frame_reference_world.rs), then runs the same framing.
    //   region   x in 810..1410, y in 860..1160 (clipped to the world). Chosen so that no vine, cactus, water candle or torch
    //            lies within 3 tiles outside it (those call CheckVines / CheckCactus / CheckOnTable1x1 / TileFrameImportant, which
    //            can kill tiles when their support is replaced, and are not ported); found by scanning a REFDUMP_KEEP_PRE=1 dump
    //   LCG      state = 0x2545F491; each draw: state = state * 1664525 + 1013904223 (u32 wrapping); value = state >> 8
    //   per tile (x outer, y inner): r = draw(); idx = (r >> 4) % (L + L / 3) where L = number of candidate types
    //            (ascending, not Main.tileFrameImportant, not TileID.Sets.IsVine, not 80 cactus, not 49 water candle: those
    //            call CheckVines / CheckCactus / CheckOnTable1x1, which can kill tiles and are not in scope);
    //            idx >= L -> inactive tile of type 0, else active tile of type candidate[idx];
    //            s = draw() % 12: s < 4 -> slope s + 1; s == 4 -> half brick; else plain
    //            sTileHeader = (old & ~(0x20 | 0x400 | 0x7000)) | active << 5 | halfBrick << 10 | slope << 12; frameX = frameY = -1
    static void ScrambleRegion(Type main, int w, int h)
    {
        Type tileT = T("Terraria.Tile");
        FieldInfo fType = tileT.GetField("type"), fHeader = tileT.GetField("sTileHeader"), fx = tileT.GetField("frameX"), fy = tileT.GetField("frameY");
        bool[] important = (bool[])SF(main, "tileFrameImportant");
        bool[] vine = (bool[])SF(T("Terraria.ID.TileID+Sets"), "IsVine");
        var cand = new List<int>();
        for (int t = 0; t < important.Length; t++) if (!important[t] && !vine[t] && t != 80 && t != 49) cand.Add(t);
        int L = cand.Count;
        Array tile = (Array)SF(main, "tile");
        uint state = 0x2545F491u;
        Func<uint> draw = () => { state = unchecked(state * 1664525u + 1013904223u); return state >> 8; };
        int x0 = 810, x1 = Math.Min(1410, w), y0 = 860, y1 = Math.Min(1160, h);
        for (int x = x0; x < x1; x++)
            for (int y = y0; y < y1; y++)
            {
                uint r = draw();
                int idx = (int)((r >> 4) % (uint)(L + L / 3));
                uint s = draw() % 12;
                bool active = idx < L;
                int slope = s < 4 ? (int)s + 1 : 0;
                bool half = s == 4;
                object t = tile.GetValue(x, y);
                fType.SetValue(t, (ushort)(active ? cand[idx] : 0));
                int old = (ushort)fHeader.GetValue(t);
                int hdr = (old & ~(0x20 | 0x400 | 0x7000)) | ((active ? 1 : 0) << 5) | ((half ? 1 : 0) << 10) | (slope << 12);
                fHeader.SetValue(t, (ushort)hdr);
                fx.SetValue(t, (short)-1);
                fy.SetValue(t, (short)-1);
            }
        Console.WriteLine("scrambled region x " + x0 + ".." + x1 + " y " + y0 + ".." + y1 + " with " + L + " candidate tile types");
    }
}
