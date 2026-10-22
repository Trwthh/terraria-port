// The probe run, part C (Probe.cs; port: tests/draw/probe_c.rs, the same steps): every head / body / legs / wings
// look worn, a house checked by the housing query, the credits roll, a graveyard's wind, critters and kites on their
// anchors, item frames / weapon racks / platters / jars / dolls / hat racks / a training dummy used by the player.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    static MethodInfo ItemSetDefaults { get { return T("Terraria.Item").GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int)); } }

    /// <summary>For each value of an Item field (headSlot, bodySlot, legSlot, wingSlot) above 0: the first item type
    /// that has it, in item type order.</summary>
    static List<int> FirstItemPerSlot(string field)
    {
        var seen = new HashSet<int>();
        var list = new List<int>();
        object it = Activator.CreateInstance(T("Terraria.Item"));
        MethodInfo sd = ItemSetDefaults;
        int count = (short)SF(T("Terraria.ID.ItemID"), "Count");
        for (int t = 1; t < count; t++)
        {
            sd.Invoke(it, new object[] { t, null });
            int v = Convert.ToInt32(IF(it, field));
            if (v > 0 && seen.Add(v)) list.Add(t);
        }
        return list;
    }

    /// <summary>A wooden house right of x (8 x 5 inside), floor on row gy: walls, back wall, a door on the left, a
    /// table, a chair and a torch (port: probe_c.rs build_house, the same tiles).</summary>
    static void BuildHouse(int hx, int gy)
    {
        for (int x = hx; x <= hx + 9; x++)
            for (int y = gy - 7; y < gy; y++) KillTile(x, y);
        for (int x = hx; x <= hx + 9; x++) { PlaceTile(x, gy - 7, 30, 0); PlaceTile(x, gy, 30, 0); }
        for (int y = gy - 6; y < gy; y++) { PlaceTile(hx, y, 30, 0); PlaceTile(hx + 9, y, 30, 0); }
        for (int x = hx + 1; x <= hx + 8; x++)
            for (int y = gy - 6; y < gy; y++) CallStatic(TWorldGen, "PlaceWall", x, y, 4, true);
        for (int y = gy - 3; y < gy; y++) KillTile(hx, y);
        PlaceTile(hx, gy - 1, 10, 0);
        PlaceTile(hx + 4, gy - 1, 14, 0);
        PlaceTile(hx + 7, gy - 1, 15, 0);
        PlaceTile(hx + 2, gy - 4, 4, 0);
    }
    static void ClearArea(int xa, int xb, int gy)
    {
        for (int x = xa; x <= xb; x++)
        {
            for (int y = gy - 12; y < gy; y++) { KillTile(x, y); CallStatic(TWorldGen, "KillWall", x, y, false); }
            PlaceTile(x, gy, 0, 0); PlaceTile(x, gy + 1, 0, 0);
        }
    }
    /// <summary>The top-left tile of `type` in the 3 x 4 box left / up of (x, y) (where PlaceObject put it).</summary>
    static Point TopLeftOf(int type, int x, int y)
    {
        for (int j = y - 4; j <= y; j++)
            for (int i = x - 2; i <= x; i++)
            {
                object t = TileAt(i, j);
                if (t != null && (bool)t.GetType().GetMethod("active", Type.EmptyTypes).Invoke(t, null) && Convert.ToInt32(IF(t, "type")) == type) return new Point(i, j);
            }
        return new Point(-1, -1);
    }
    static object TEByID(int id) { return ((IDictionary)SF(T("Terraria.DataStructures.TileEntity"), "ByID"))[id]; }

    static void ProbeC(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        // ---- every head, body, legs look worn (the first item of each equip slot), and every wings look worn while
        //      falling (the wings that animate only in the air)
        if (Group("wardrobe"))
        {
            string[][] parts = { new[] { "headSlot", "0" }, new[] { "bodySlot", "1" }, new[] { "legSlot", "2" } };
            foreach (string[] part in parts)
            {
                List<int> items = FirstItemPerSlot(part[0]);
                Note("wardrobe " + part[0] + ": " + items.Count);
                int slot = int.Parse(part[1]);
                PlacePlayer(sx, sy);
                for (int k = 0; k < items.Count; k++)
                {
                    SetArmor(slot, items[k]);
                    Frames(1);
                    if (k % 25 == 0) Record("wardrobe_" + part[0] + "_" + k, 1);
                }
                SetArmor(slot, 0);
            }
            List<int> wings = FirstItemPerSlot("wingSlot");
            Note("wardrobe wingSlot: " + wings.Count);
            for (int k = 0; k < wings.Count; k++)
            {
                SetArmor(3, wings[k]);
                PlacePlayer(sx, sy - 30);
                Frames(3);
                if (k % 10 == 0) Record("wardrobe_wings_" + k, 1);
            }
            SetArmor(3, 0);
            PlacePlayer(sx, sy);
            Frames(60);
        }
        // ---- a house checked by the housing query (Main.PerformHousingCheck: RichRoomCheckFeedback's particles), and
        //      the same with its door and table taken out
        if (Group("housing"))
        {
            int hx = x0 + 4;
            BuildHouse(hx, gy);
            PlacePlayer(hx + 12, gy - 1);
            Frames(5);
            object main = SF(TMain, "instance");
            CallI(main, "PerformHousingCheck", hx + 4, gy - 2);
            Frames(20);
            Record("housing_good", 1);
            Frames(150);
            KillTile(hx, gy - 1);
            KillTile(hx + 4, gy - 1);
            Frames(3);
            CallI(main, "PerformHousingCheck", hx + 4, gy - 2);
            Frames(20);
            Record("housing_bad", 1);
            Frames(150);
            ClearArea(hx - 1, hx + 10, gy);
            Frames(3);
        }
        // ---- the credits roll (CreditsRollEvent / CreditsRollSky), its time stepped through the whole roll
        if (Group("credits"))
        {
            PlacePlayer(sx, sy);
            CallS("Terraria.GameContent.Events.CreditsRollEvent", "TryStartingCreditsRoll");
            Frames(2);
            object skyMgr = SF(T("Terraria.Graphics.Effects.SkyManager"), "Instance");
            object sky = skyMgr.GetType().GetProperty("Item", new[] { typeof(string) }).GetValue(skyMgr, new object[] { "CreditsRoll" });
            int end = (int)IF(sky, "_endTime");
            Note("credits: active " + CallI(sky, "IsActive") + ", end " + end);
            for (int t = 0, k = 0; t < end; t += 240, k++)
            {
                ISet(sky, "_currentTime", t);
                Frames(1);
                if (k % 12 == 0) Record("credits_" + t, 1);
            }
            CallS("Terraria.GameContent.Events.CreditsRollEvent", "Reset");
            Frames(150);
        }
        // ---- a graveyard (enough tombstones for SceneMetrics.ZoneGraveyard): its wind clouds (AmbientWindSystem,
        //      Gore.UpdateAmbientFloorCloud / UpdateAmbientAirborneCloud)
        if (Group("graveyard"))
        {
            for (int k = 0; k < 12; k++) PlaceTile(x0 + 4 + k * 3, gy - 1, 85, 0);
            PlacePlayer(x0 + 20, gy - 1);
            Frames(240);
            Record("graveyard", 1);
            PFx("graveyard");
            ClearArea(x0, x0 + 45, gy);
            Frames(60);
        }
        // ---- critters and kites on anchors (TECritterAnchor / TEKiteAnchor, placed as the player places them with the
        //      item in hand: TELeashedEntityAnchorWithItem.PlaceFromPlayerPlacementHook): one critter of every
        //      LeashedCritter prototype, six kites
        if (Group("leashed"))
        {
            Type tca = T("Terraria.GameContent.Tile_Entities.TECritterAnchor");
            Array protos = (Array)SF(tca, "CritterPrototypes");
            var critters = new List<int>();
            var seenProto = new List<object>();
            var kites = new List<int>();
            bool[] isKite = (bool[])SF(T("Terraria.ID.ItemID+Sets"), "IsAKite");
            object it = Activator.CreateInstance(T("Terraria.Item"));
            MethodInfo sd = ItemSetDefaults;
            int count = (short)SF(T("Terraria.ID.ItemID"), "Count");
            for (int t = 1; t < count; t++)
            {
                if (isKite[t]) { if (kites.Count < 6) kites.Add(t); continue; }
                sd.Invoke(it, new object[] { t, null });
                int mn = Convert.ToInt32(IF(it, "makeNPC"));
                if (mn <= 0) continue;
                object proto = protos.GetValue(mn);
                if (seenProto.Any(p => ReferenceEquals(p, proto))) continue;
                seenProto.Add(proto);
                critters.Add(t);
            }
            Note("leashed critters: " + string.Join(",", critters.Select(t => t.ToString()).ToArray()) + "; kites: " + string.Join(",", kites.Select(t => t.ToString()).ToArray()));
            PlacePlayer(x0 + 30, gy - 1);
            // two rounds (the critters, then the kites), anchors three tiles apart
            foreach (var round in new[] { critters.Select(t => new[] { t, 724 }).ToList(), kites.Select(t => new[] { t, 723 }).ToList() })
            {
                int ax = x0;
                foreach (int[] e in round)
                {
                    GiveItem(0, e[0]);
                    SelectItem(0);
                    bool ok = (bool)TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { ax, gy - 1, e[1], false, 0, 0, -1, 1 });
                    Point tl = TopLeftOf(e[1], ax, gy - 1);
                    int id = (int)CallS(e[1] == 724 ? "Terraria.GameContent.Tile_Entities.TECritterAnchor" : "Terraria.GameContent.Tile_Entities.TEKiteAnchor", "Hook_AfterPlacement", tl.X, tl.Y, e[1], 0, 1, 0);
                    Note("anchor " + e[0] + " at " + ax + ": " + ok + " " + tl.X + "," + tl.Y + " id " + id);
                    ax += 3;
                }
                GiveItem(0, 0);
                Frames(60);
                Record("leashed_" + round[0][1] + "_a", 1);
                Frames(120);
                Record("leashed_" + round[0][1] + "_b", 1);
                ClearArea(x0 - 1, ax + 1, gy);
                Frames(30);
            }
        }
        // ---- tile entities used by the player: an item frame, a weapon rack, a food platter and a Dead Cells jar given
        //      an item from the hand and emptied again (OnPlayerInteraction -> PlaceItemInFrame / TryPlacing); a display
        //      doll opened (its slots drawn), fitted with armour by shift-click and its pose changed by a hammer; a hat
        //      rack fitted; a training dummy activated by the player standing by it
        if (Group("tileentities"))
        {
            PlacePlayer(x0 + 2, gy - 1);
            int tx = x0 + 6;
            foreach (object[] e in new[] { new object[] { "TEItemFrame", 395, 4, 1 }, new object[] { "TEWeaponsRack", 471, 4, 2 }, new object[] { "TEFoodPlatter", 520, 4, 0 }, new object[] { "TEDeadCellsDisplayJar", 698, 4, 0 } })
            {
                string name = (string)e[0];
                int type = (int)e[1];
                for (int x = tx - 2; x <= tx + 3; x++) for (int y = gy - 5; y < gy; y++) CallStatic(TWorldGen, "PlaceWall", x, y, 4, true);
                if (type == 520) PlaceTile(tx, gy - 1, 19, 0);
                if (type == 698) PlaceTile(tx, gy - 4, 30, 0);
                int py = type == 698 ? gy - 3 : gy - 2;
                bool ok = (bool)TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { tx, py, type, false, 0, 0, -1, 1 });
                Point tl = TopLeftOf(type, tx + 1, py + 1);
                int id = (int)CallS("Terraria.GameContent.Tile_Entities." + name, "Hook_AfterPlacement", tl.X, tl.Y, type, 0, 1, 0);
                Note(name + ": " + ok + " at " + tl.X + "," + tl.Y + " id " + id + " " + CoverDiag(tx - 1, gy - 5, 5, 6));
                if (tl.X < 0) throw new Exception(name + " was not placed");
                GiveItem(0, type == 520 ? 357 : (type == 471 ? 3507 : 1));
                SelectItem(0);
                ISet(pl, "itemTime", 0);
                CallS("Terraria.GameContent.Tile_Entities." + name, "OnPlayerInteraction", pl, tl.X, tl.Y);
                Frames(3);
                P("te_" + name + "_held", IF(((Array)IF(pl, "inventory")).GetValue(0), "type"), IF(((Array)IF(pl, "inventory")).GetValue(0), "stack"));
                Record("te_" + name, 1);
                GiveItem(0, 0);
                ISet(pl, "itemTime", 0);
                CallS("Terraria.GameContent.Tile_Entities." + name, "OnPlayerInteraction", pl, tl.X, tl.Y);
                Frames(3);
                P("te_" + name + "_emptied", TileHash(tx - 1, gy - 5, 5, 6));
                for (int x = tx - 2; x <= tx + 3; x++) for (int y = gy - 5; y < gy; y++) { KillTile(x, y); CallStatic(TWorldGen, "KillWall", x, y, false); }
                ClearEntities();
                Frames(3);
            }
            // a display doll and a hat rack
            int dx = x0 + 10;
            TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { dx, gy - 1, 470, false, 0, 0, -1, 1 });
            Point dtl = TopLeftOf(470, dx + 1, gy - 1);
            int did = (int)CallS("Terraria.GameContent.Tile_Entities.TEDisplayDoll", "Hook_AfterPlacement", dtl.X, dtl.Y, 470, 0, 1, 0);
            object doll = TEByID(did);
            Note("doll at " + dtl.X + "," + dtl.Y + " id " + did);
            PlacePlayer(dx - 1, gy - 1);
            Frames(2);
            SSet(TMain, "playerInventory", true);
            CallS("Terraria.GameContent.Tile_Entities.TEDisplayDoll", "OnPlayerInteraction", pl, dtl.X, dtl.Y);
            Frames(3);
            Record("te_doll_open", 1);
            Array inv = (Array)IF(pl, "inventory");
            int[] fit = { 231, 232, 233, 1243, 3884 };
            for (int k = 0; k < fit.Length; k++) GiveItem(10 + k, fit[k]);
            SSet(TMain, "cursorOverride", (short)9);
            for (int k = 0; k < fit.Length; k++) P("doll_fit_" + fit[k], CallI(doll, "PerformShiftClickAction", inv, 0, 10 + k));
            SSet(TMain, "cursorOverride", (short)0);
            for (int k = 0; k < 3; k++) P("doll_pose_" + k, CallS("Terraria.GameContent.Tile_Entities.TEDisplayDoll", "TryChangePose", dtl.X, dtl.Y + 1), IF(doll, "_pose"));
            Frames(3);
            Record("te_doll_fitted", 1);
            CallS("Terraria.GameContent.Tile_Entities.TEDisplayDoll", "OnPlayerInteraction", pl, dtl.X, dtl.Y);
            Frames(3);
            int hrx = x0 + 16;
            TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { hrx, gy - 1, 475, false, 0, 0, -1, 1 });
            Point htl = TopLeftOf(475, hrx + 1, gy - 1);
            int hid = (int)CallS("Terraria.GameContent.Tile_Entities.TEHatRack", "Hook_AfterPlacement", htl.X, htl.Y, 475, 0, 1, 0);
            object rack = TEByID(hid);
            PlacePlayer(hrx - 1, gy - 1);
            Frames(2);
            CallS("Terraria.GameContent.Tile_Entities.TEHatRack", "OnPlayerInteraction", pl, htl.X, htl.Y);
            Frames(3);
            GiveItem(10, 1243); GiveItem(11, 3884); GiveItem(12, 869);
            SSet(TMain, "cursorOverride", (short)9);
            for (int k = 0; k < 3; k++) P("hatrack_fit_" + k, CallI(rack, "PerformShiftClickAction", inv, 0, 10 + k));
            SSet(TMain, "cursorOverride", (short)0);
            Frames(3);
            Record("te_hatrack", 1);
            CallS("Terraria.GameContent.Tile_Entities.TEHatRack", "OnPlayerInteraction", pl, htl.X, htl.Y);
            SSet(TMain, "playerInventory", false);
            for (int k = 10; k < 15; k++) GiveItem(k, 0);
            Frames(3);
            // a training dummy
            int ddx = x0 + 24;
            TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { ddx, gy - 1, 378, false, 0, 0, -1, 1 });
            Point ttl = TopLeftOf(378, ddx + 1, gy - 1);
            int tid = (int)CallS("Terraria.GameContent.Tile_Entities.TETrainingDummy", "Hook_AfterPlacement", ttl.X, ttl.Y, 378, 0, 1, 0);
            Note("dummy at " + ttl.X + "," + ttl.Y + " id " + tid);
            PlacePlayer(ddx - 2, gy - 1);
            Frames(30);
            Record("te_dummy", 1);
            P("dummy_npc", IF(TEByID(tid), "npc"));
            // everything taken down again (the doll's and the rack's items first: a doll holding items cannot break)
            foreach (object te in new[] { doll, rack })
                foreach (string f in new[] { "_equip", "_dyes", "_misc", "_items" })
                {
                    FieldInfo fi = te.GetType().GetField(f, AllInst);
                    if (fi == null) continue;
                    foreach (object item in (Array)fi.GetValue(te)) if (item != null) CallI(item, "TurnToAir");
                }
            for (int x = x0; x <= x0 + 30; x++) for (int y = gy - 5; y < gy; y++) KillTile(x, y);
            ClearEntities();
            Frames(10);
        }
    }
}
