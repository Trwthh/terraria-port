// RefDumpEntities citem: client-side item use. Every item with a use style is used by the OWNING player of a single
// player game (Main.myPlayer = 0, Main.netMode = 0) for N ticks at a fixed target: what only the owning client does
// (shooting projectiles, placing / digging tiles, consuming potions and buffs, mana and ammo) runs here, which the
// server-side `item` scenario cannot show. Compiled into RefDumpEntities.exe / RefJitEntities.exe with Entities.cs.
//
//   RefDumpEntities.exe citem <outdir> [ticks=60] [max batches]
//
// Load as the other kinds (Entities.cs EntRun: ref_small.wld scratch copy, the ticks harness's pins), then:
//   Main.myPlayer = 0; Main.netMode = 0; Main.dedServ stays true (no content is loaded in the server build: sound and
//   drawing stay off; the gameplay code reads dedServ in 11 places of Player.cs, none in shooting / placing / using);
//   Main.screenWidth = 1600, Main.screenHeight = 900 (Dust.NewDust culls by the screen rectangle).
// One batch per item type (the 4,629 types of the `item` scenario), all at the same place: the player stands at the
// middle slot of Entities.cs (tile column SlotX0 + 15 * SlotDX + PlayerDX, on the ground). Before every batch, in
// this order (the port does exactly the same):
//   1. tiles of the reset region (columns [player - 150, + 150], rows [player - 150, + 100]) that differ from the loaded
//      world restored (count + hash dumped, as Entities.cs TilesBack does for the whole world); after the last batch the
//      whole world is compared and restored as in the other kinds (a change outside the region would show there)
//   2. chests, signs and tile entities created since the load removed: Main.chest[i] / Main.sign[i] = null where they
//      were null after the load; TileEntity.ByID / ByPosition entries whose id was not there after the load removed,
//      TileEntity.TileEntitiesNextID set back
//   3. NPC.ClearAll(); Projectile.ClearAll(); Main.item[i] = new WorldItem { whoAmI = i },
//      Main.timeItemSlotCannotBeReusedFor[i] = 0; Main.player[0] = new Player { whoAmI = 0 }
//   4. Main.rand = new UnifiedRandom(CItemSeedBase + batch); Main._rngs = new Dictionary
//   5. the player: active, name "Ref", statLifeMax / statLifeMax2 100, statLife 60 (healing can be used), statManaMax /
//      statManaMax2 200, statMana 100, SpawnX / SpawnY, Spawn(SpawningIntoWorld) with Main.myPlayer = 255 meanwhile (the
//      local-player part of Spawn moves the camera, clears the lighting engine, starts the play timer: client parts the
//      server build cannot run), then Main.myPlayer = 0 again; inventory slot 0 = the item
//      (SetDefaults(type), stack = maxStack), slots 10.. = the ammo list below (stack = maxStack); the selected slot of a new Player is 0; releaseUseItem = true (the button was up before)
//   6. the target: an item with pick / axe / hammer power aims at the ground tile two columns right of the player's
//      spawn column, any other item at the air tile three columns right and one row up; Main.screenPosition = the
//      player's spawn point (pixels) - (800, 450); Main.mouseX / mouseY = the target tile's centre - screenPosition
// Per tick: PlayerInput.Triggers.Current.MouseLeft = true, Main.mouseLeft = true, Main.mouseLeftRelease = (tick == 1),
// Main.mouseX / mouseY as above (the player holds the button on the target), then the same five parts of
// Main.DoUpdateInWorld as the other kinds (Entities.cs EntityTick). After the tick: the RNG streams and the entities as
// in the other kinds, then the tiles of the box (columns [player column - 30, + 30], rows [player row - 45, + 30])
// that differ from the loaded world: i32 count, u64 FNV-1a 64 of (x, y, record).
using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;

public static partial class WG
{
    const int CItemSeedBase = 888000, CItemSlot = 15, ScreenW = 1600, ScreenH = 900;
    // the per-tick box and the per-batch reset region, relative to the player's spawn tile: x0, x1, y0, y1
    static readonly int[] CItemBox = { -30, 30, -45, 30 }, CItemReset = { -150, 150, -150, 100 };
    // ammo in inventory slots 10.. (ItemID names; a name the game does not have is skipped and listed in the summary)
    static readonly string[] CItemAmmo = { "WoodenArrow", "MusketBall", "RocketI", "Seed", "PoisonDart", "Gel", "FallenStar", "SandBlock",
        "Stake", "Nail", "Snowball", "Flare", "GreenSolution", "CandyCorn", "ExplosiveJackOLantern", "StyngerBolt", "SilverCoin", "Bone" };

    static void CItemRun(int ticks, int maxBatches, string srcSha, Stopwatch total, int[] usableTypes)
    {
        Type netplay = T("Terraria.Netplay");
        SSet(TMain, "myPlayer", 0); SSet(TMain, "netMode", 0);
        SSet(TMain, "screenWidth", ScreenW); SSet(TMain, "screenHeight", ScreenH);
        // the game view the client builds from its graphics device (Main.cs:10967): here without a device, with the
        // screen as its viewport (zoom 1: Translation 0); Camera.ScaledSize reads it (ItemCheck_PlayInstruments)
        {
            Type tSvm = T("Terraria.Graphics.SpriteViewMatrix");
            object svm = tSvm.GetConstructors()[0].Invoke(new object[] { null });
            Type tVp = tSvm.GetMethod("SetViewportOverride").GetParameters()[0].ParameterType;
            object vp = Activator.CreateInstance(tVp, new object[] { 0, 0, ScreenW, ScreenH });
            tSvm.GetMethod("SetViewportOverride").Invoke(svm, new object[] { vp });
            SSet(TMain, "GameViewMatrix", svm);
        }
        // the achievements a client creates at start-up (Main.cs:10846, PostContentLoadInitialize -> AchievementInitializer
        // .Load, which registers them when netMode != 2; no achievements.dat in the scratch save folder): the local
        // player's update reports running / mining to them (AchievementsHelper.HandleRunning)
        ISet(MainInst, "_achievements", Activator.CreateInstance(T("Terraria.Achievements.AchievementManager")));
        CallStatic(T("Terraria.Initializers.AchievementInitializer"), "Load");
        // the lighting engine a client creates (Main.cs:6774 Lighting.Initialize): lights added by items and
        // projectiles go to it (Lighting.AddLight); nothing is drawn
        CallStatic(T("Terraria.Lighting"), "Initialize");
        // fonts: the server build loads no content. Combat text (healing numbers) and popup text (picked-up items)
        // measure their string with them (CombatText.NewText, PopupText.NewText); the measurement only places the text,
        // which is not dumped. Both sides use the same stand-in font: characters U+0020 .. U+007E, each glyph 10 x 20,
        // padding 0, kerning (0, 10, 0), default character '*', spacing 0, line spacing 20
        FakeFonts();
        // exceptions of a player / NPC / projectile update are not swallowed (Main.ignoreErrors, default true): a part
        // of the client the server build cannot run then shows as an error of its batch, on both sides
        SSet(TMain, "ignoreErrors", false);
        Type TItem = T("Terraria.Item"), TChest = T("Terraria.Chest"), TSign = T("Terraria.Sign"), TTE = T("Terraria.DataStructures.TileEntity");
        MethodInfo itemDefaults = TItem.GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int));
        Array players = (Array)SF(TMain, "player"), items = (Array)SF(TMain, "item"), chests = (Array)SF(TMain, "chest"), signs = (Array)SF(TMain, "sign");
        int[] noReuse = (int[])SF(TMain, "timeItemSlotCannotBeReusedFor");
        Type ctxT = T("Terraria.PlayerSpawnContext");
        object spawnInto = Enum.Parse(ctxT, "SpawningIntoWorld");
        Type v2 = T("Terraria.Entity").GetField("position").FieldType;
        // load-time state of chests, signs, tile entities
        var chestNull = new bool[chests.Length]; for (int i = 0; i < chests.Length; i++) chestNull[i] = chests.GetValue(i) == null;
        var signNull = new bool[signs.Length]; for (int i = 0; i < signs.Length; i++) signNull[i] = signs.GetValue(i) == null;
        var byId = (IDictionary)SF(TTE, "ByID"); var byPos = (IDictionary)SF(TTE, "ByPosition");
        var teIds = new HashSet<int>(byId.Keys.Cast<int>());
        int teNext = (int)SF(TTE, "TileEntitiesNextID");
        // ammo
        var ammo = new List<int>();
        foreach (string n in CItemAmmo)
        {
            FieldInfo f = T("Terraria.ID.ItemID").GetField(n, AllStatic);
            if (f == null) { ENotes.Add("ammo " + n + ": no such ItemID"); continue; }
            ammo.Add(Convert.ToInt32(f.GetValue(null)));
        }
        // the place
        int px = SlotX0 + CItemSlot * SlotDX + PlayerDX, py = Ground(px);
        // input
        Type tPI = T("Terraria.GameInput.PlayerInput");
        object triggers = SF(tPI, "Triggers");
        PropertyInfo pCurrent = triggers.GetType().GetProperty("Current") ;
        FieldInfo fCurrent = pCurrent == null ? triggers.GetType().GetField("Current") : null;
        Func<object> current = () => pCurrent != null ? pCurrent.GetValue(triggers, null) : fCurrent.GetValue(triggers);

        int nb = Math.Min(usableTypes.Length, maxBatches);
        Console.WriteLine("citem: " + usableTypes.Length + " item types with a use style, " + nb + " batches, " + ticks + " ticks each; player at " + px + ", " + py);
        var zo = new ZOut(Path.Combine(EOut, "citem.ents.zlib"));
        BW = new BinaryWriter(zo);
        BW.Write(Encoding.ASCII.GetBytes("ENTS")); BW.Write(1u);
        WStr("citem"); BW.Write(ticks); BW.Write(nb);
        BW.Write(Schemas.Length);
        for (int c = 0; c < Schemas.Length; c++)
        {
            WStr(ClassArrays[c]);
            BW.Write(Schemas[c].Leaves.Count);
            foreach (Leaf l in Schemas[c].Leaves) { WStr(l.Name); BW.Write(l.Kind); }
        }
        // citem header extension: screen size, ammo list, box half width
        BW.Write(ScreenW); BW.Write(ScreenH);
        foreach (int v in CItemBox) BW.Write(v);
        foreach (int v in CItemReset) BW.Write(v);
        BW.Write(ammo.Count); foreach (int a in ammo) BW.Write(a);

        var errors = new List<string>();
        var summaryBatches = new StringBuilder();
        var tickClock = Stopwatch.StartNew();
        for (int b = 0; b < nb; b++)
        {
            int type = usableTypes[b];
            // 1. tiles
            int tc; ulong th;
            RegionTiles(px + CItemReset[0], px + CItemReset[1], py + CItemReset[2], py + CItemReset[3], true, out tc, out th);
            // 2. chests, signs, tile entities
            int removed = 0;
            for (int i = 0; i < chests.Length; i++) if (chestNull[i] && chests.GetValue(i) != null) { chests.SetValue(null, i); removed++; }
            for (int i = 0; i < signs.Length; i++) if (signNull[i] && signs.GetValue(i) != null) { signs.SetValue(null, i); removed++; }
            foreach (int id in byId.Keys.Cast<int>().Where(k => !teIds.Contains(k)).OrderBy(k => k).ToList()) { byId.Remove(id); removed++; }
            foreach (object k in byPos.Keys.Cast<object>().Where(k => !teIds.Contains((int)IF(byPos[k], "ID"))).ToList()) byPos.Remove(k);
            SSet(TTE, "TileEntitiesNextID", teNext);
            // 3. entities
            CallStatic(TNPC, "ClearAll"); CallStatic(TProj, "ClearAll");
            for (int i = 0; i < 400; i++) { object wi = Activator.CreateInstance(TWorldItem); ISet(wi, "whoAmI", i); items.SetValue(wi, i); noReuse[i] = 0; }
            { object p0 = Activator.CreateInstance(TPlayer); ISet(p0, "whoAmI", 0); players.SetValue(p0, 0); }
            // 4. RNG
            SSet(TMain, "rand", NewRand(CItemSeedBase + b));
            TMain.GetField("_rngs", AllStatic).SetValue(null, Activator.CreateInstance(TMain.GetField("_rngs", AllStatic).FieldType));
            Last = new[] { new Dictionary<int, ulong[]>(), new Dictionary<int, ulong[]>(), new Dictionary<int, ulong[]>(), new Dictionary<int, ulong[]>() };
            // 5. the player
            object p = players.GetValue(0);
            string err = null;
            int tx = 0, ty = 0, mx = 0, my = 0; float spx = 0, spy = 0;
            try
            {
                IF_SetE(p, "active", true); IF_SetE(p, "name", "Ref"); IF_SetE(p, "whoAmI", 0);
                IF_SetE(p, "statLifeMax", 100); IF_SetE(p, "statLifeMax2", 100); IF_SetE(p, "statLife", 60);
                IF_SetE(p, "statManaMax", 200); IF_SetE(p, "statManaMax2", 200); IF_SetE(p, "statMana", 100);
                IF_SetE(p, "SpawnX", px); IF_SetE(p, "SpawnY", py);
                // Spawn as the server spawns a player (Main.myPlayer = 255 meanwhile): its local-player part moves the
                // camera, clears the lighting engine and starts the play timer, which the server build cannot do
                SSet(TMain, "myPlayer", 255);
                try { Unwrap(TPlayer.GetMethod("Spawn"), p, new object[] { spawnInto }); }
                finally { SSet(TMain, "myPlayer", 0); }
                Array inv = (Array)IF(p, "inventory");
                object it = inv.GetValue(0);
                Unwrap(itemDefaults, it, new object[] { type, null });
                IF_SetE(it, "stack", IF(it, "maxStack"));
                for (int k = 0; k < ammo.Count; k++)
                {
                    object am = inv.GetValue(10 + k);
                    Unwrap(itemDefaults, am, new object[] { ammo[k], null });
                    IF_SetE(am, "stack", IF(am, "maxStack"));
                }
                // selectedItem of a new Player is 0 (selectedItemState). The button was up before the first tick:
                // releaseUseItem = true (ItemCheck starts a use on `controlUseItem && releaseUseItem`)
                IF_SetE(p, "releaseUseItem", true);
                // 6. the target
                bool tool = (int)IF(it, "pick") > 0 || (int)IF(it, "axe") > 0 || (int)IF(it, "hammer") > 0;
                tx = tool ? px + 2 : px + 3; ty = tool ? py : py - 1;
                spx = px * 16 - ScreenW / 2; spy = py * 16 - ScreenH / 2;
                SSet(TMain, "screenPosition", Activator.CreateInstance(v2, new object[] { spx, spy }));
                mx = tx * 16 + 8 - (int)spx; my = ty * 16 + 8 - (int)spy;
            }
            catch (Exception e)
            {
                err = "setup: " + (e.InnerException ?? e).GetType().Name + ": " + e.Message;
                if (errors.Count < 3) Console.WriteLine("setup stack: " + (e.InnerException ?? e).StackTrace);
            }
            // batch record (the slot fields of the other kinds: ex, ey = the target tile; px, py = the spawn tile)
            BW.Write(b); BW.Write(CItemSeedBase + b); BW.Write(tc); BW.Write(th); BW.Write(1);
            BW.Write(type); BW.Write(tx); BW.Write(ty); BW.Write(px); BW.Write(py); BW.Write(0); BW.Write(-1); BW.Write(spx); BW.Write(spy); BW.Write(err == null ? 0 : -2);
            BW.Write(mx); BW.Write(my); BW.Write(removed);
            string tickErr = err;
            for (int t = 0; t <= ticks; t++)
            {
                if (t > 0 && tickErr == null)
                {
                    try
                    {
                        object cur = current();
                        cur.GetType().GetProperty("MouseLeft").SetValue(cur, true, null);
                        SSet(TMain, "mouseLeft", true); SSet(TMain, "mouseLeftRelease", t == 1);
                        SSet(TMain, "mouseX", mx); SSet(TMain, "mouseY", my);
                        EntityTick();
                    }
                    catch (Exception e)
                    {
                        Exception ie = e; while (ie.InnerException != null) ie = ie.InnerException;
                        tickErr = "tick " + t + ": " + ie.GetType().Name + ": " + ie.Message + " at " + FirstFrame(ie);
                        if (errors.Count < 3) Console.WriteLine("tick stack: " + ie.StackTrace);
                    }
                }
                BW.Write(t); BW.Write((byte)(tickErr == null ? 0 : 1));
                if (tickErr != null) { errors.Add("citem " + type + " (batch " + b + "): " + tickErr); break; }
                DumpRand();
                DumpEntities();
                int bc; ulong bh;
                RegionTiles(px + CItemBox[0], px + CItemBox[1], py + CItemBox[2], py + CItemBox[3], false, out bc, out bh);
                BW.Write(bc); BW.Write(bh);
            }
            if (summaryBatches.Length > 0) summaryBatches.Append(",\n");
            summaryBatches.Append("{\"batch\":" + b + ",\"type\":" + type + ",\"tiles_restored\":" + tc + ",\"removed\":" + removed + ",\"target\":[" + tx + "," + ty + "]" + (tickErr != null ? ",\"error\":" + Q(tickErr) : "") + "}");
            if (b % 200 == 0) Console.WriteLine("batch " + b + " of " + nb + " (" + tickClock.ElapsedMilliseconds + " ms)");
        }
        int tc2; ulong th2;
        TilesBack(out tc2, out th2);
        BW.Write(-1); BW.Write(tc2); BW.Write(th2);
        BW.Close();

        var sj = new StringBuilder("{\n");
        sj.Append("\"generator\":\"tools/refdump_entities (RefDumpEntities.exe citem)\",\n");
        sj.Append("\"kind\":\"citem\",\"ticks\":" + ticks + ",\"batches\":" + nb + ",\"types\":" + usableTypes.Length + ",\n");
        sj.Append("\"world_sha256\":" + Q(srcSha) + ",\"seed_base\":" + CItemSeedBase + ",\"bytes_uncompressed\":" + zo.Total + ",\n");
        sj.Append("\"player_tile\":[" + px + "," + py + "],\"screen\":[" + ScreenW + "," + ScreenH + "],\"box\":[" +string.Join(",", CItemBox.Select(v => v.ToString()).ToArray()) + "],\"reset_region\":[" + string.Join(",", CItemReset.Select(v => v.ToString()).ToArray()) + "],\n");
        sj.Append("\"ammo\":[" + string.Join(",", ammo.Select(a => a.ToString()).ToArray()) + "],\n");
        for (int c = 0; c < Schemas.Length; c++)
            sj.Append("\"leaves_" + ClassArrays[c] + "\":" + Schemas[c].Leaves.Count + ",\n");
        sj.Append("\"errors\":[" + string.Join(",", errors.Select(Q).ToArray()) + "],\n");
        sj.Append("\"notes\":[" + string.Join(",", ENotes.Select(Q).ToArray()) + "],\n");
        sj.Append("\"batch_list\":[\n" + summaryBatches + "\n]\n}\n");
        File.WriteAllText(Path.Combine(EOut, "citem.summary.json"), sj.ToString(), new UTF8Encoding(false));
        File.WriteAllText(Path.Combine(EOut, "citem.timing.json"), "{\"tick_ms_total\":" + tickClock.ElapsedMilliseconds + ",\"total_ms\":" + total.ElapsedMilliseconds + "}\n", new UTF8Encoding(false));
        Console.WriteLine("citem: done, " + nb + " batches, " + zo.Total + " bytes uncompressed, " + errors.Count + " errors, " + total.ElapsedMilliseconds + " ms");
        foreach (string e in errors.Take(50)) Console.WriteLine("  " + e);
    }

    static void FakeFonts()
    {
        Assembly rl = T("Terraria.GameContent.FontAssets").GetField("MouseText").FieldType.Assembly;
        Type tFont = rl.GetType("ReLogic.Graphics.DynamicSpriteFont"), tPage = rl.GetType("ReLogic.Graphics.FontPage");
        Type tRect = T("Terraria.Entity").GetProperty("Hitbox").PropertyType;
        Type tVec3 = T("Terraria.Lighting").GetMethods(AllStatic).First(m => m.Name == "AddLight" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType.Name == "Vector2").GetParameters()[1].ParameterType;
        object font = Activator.CreateInstance(tFont, new object[] { 0f, 20, '*' });
        var glyphs = (IList)Activator.CreateInstance(typeof(List<>).MakeGenericType(tRect));
        var pads = (IList)Activator.CreateInstance(typeof(List<>).MakeGenericType(tRect));
        var chars = new List<char>();
        var kern = (IList)Activator.CreateInstance(typeof(List<>).MakeGenericType(tVec3));
        for (char c = ' '; c <= '~'; c++)
        {
            glyphs.Add(Activator.CreateInstance(tRect, new object[] { 0, 0, 10, 20 }));
            pads.Add(Activator.CreateInstance(tRect, new object[] { 0, 0, 0, 0 }));
            chars.Add(c);
            kern.Add(Activator.CreateInstance(tVec3, new object[] { 0f, 10f, 0f }));
        }
        object page = tPage.GetConstructors(AllInst)[0].Invoke(new object[] { null, glyphs, pads, chars, kern });
        Array pages = Array.CreateInstance(tPage, 1); pages.SetValue(page, 0);
        tFont.GetMethod("SetPages", AllInst).Invoke(font, new object[] { pages });
        Type tAsset = T("Terraria.GameContent.FontAssets").GetField("MouseText").FieldType;
        Func<object> asset = () =>
        {
            object a = tAsset.GetConstructors(AllInst)[0].Invoke(new object[] { "Fonts/StandIn" });
            tAsset.GetProperty("Value").GetSetMethod(true).Invoke(a, new object[] { font });
            return a;
        };
        Type fa = T("Terraria.GameContent.FontAssets");
        foreach (string n in new[] { "ItemStack", "MouseText", "DeathText" }) fa.GetField(n).SetValue(null, asset());
        Array ct = (Array)fa.GetField("CombatText").GetValue(null);
        for (int i = 0; i < ct.Length; i++) ct.SetValue(asset(), i);
    }
    static string FirstFrame(Exception e)
    {
        string st = e.StackTrace ?? "";
        int i = st.IndexOf(" at ", StringComparison.Ordinal);
        if (i < 0) return "?";
        int j = st.IndexOf((char)10, i);
        return (j < 0 ? st.Substring(i + 4) : st.Substring(i + 4, j - i - 4)).Trim();
    }
    // tiles of columns [x0, x1], rows [y0, y1] that differ from the loaded world: count and FNV-1a 64 of (x, y, record),
    // column by column (x outer, y inner); `restore`: set them back to the loaded record (Entities.cs TilesBack)
    static byte[] _boxCol;
    static void RegionTiles(int x0, int x1, int y0, int y1, bool restore, out int count, out ulong hash)
    {
        if (_fillColumn == null) _fillColumn = MakeFillColumn();
        Array tile = (Array)SF(TMain, "tile");
        int h = Loaded.H;
        if (_boxCol == null || _boxCol.Length != h * RecSize) _boxCol = new byte[h * RecSize];
        count = 0; hash = FnvBasis;
        for (int x = Math.Max(0, x0); x <= x1 && x < Loaded.W; x++)
        {
            Array.Clear(_boxCol, 0, _boxCol.Length);
            _fillColumn(tile, x, h, _boxCol);
            long baseo = (long)x * h * RecSize;
            for (int y = Math.Max(0, y0); y <= y1 && y < h; y++)
            {
                int o = y * RecSize;
                bool same = true;
                for (int k = 0; k < RecSize; k++) if (_boxCol[o + k] != Loaded.Buf[baseo + o + k]) { same = false; break; }
                if (same) continue;
                count++;
                hash = FnvBytes(hash, BitConverter.GetBytes(x)); hash = FnvBytes(hash, BitConverter.GetBytes(y));
                for (int k = 0; k < RecSize; k++) { hash ^= _boxCol[o + k]; hash *= FnvPrime; }
                if (!restore) continue;
                object t = tile.GetValue(x, y);
                byte[] r = Loaded.Buf;
                long lo = baseo + o;
                Func<int, int> u16 = off => r[lo + off] | (r[lo + off + 1] << 8);
                TileFields[0].SetValue(t, (ushort)u16(0));
                TileFields[1].SetValue(t, (ushort)u16(2));
                TileFields[2].SetValue(t, r[lo + 4]);
                TileFields[3].SetValue(t, (ushort)u16(5));
                TileFields[4].SetValue(t, r[lo + 7]);
                TileFields[5].SetValue(t, r[lo + 8]);
                TileFields[6].SetValue(t, r[lo + 9]);
                TileFields[7].SetValue(t, (short)u16(10));
                TileFields[8].SetValue(t, (short)u16(12));
            }
        }
    }
}
