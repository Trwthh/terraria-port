// The probe run, part F (Probe.cs; port: tests/draw/probe_f.rs, the same steps): luck rolls, Lucy the axe, the digger
// cart, boss damage reports, beds, portals, the potion of return's gates, quick stacking, weather skies, the Martian
// madness sky, shimmer-free spots, the Eye of Cthulhu in the sky.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    /// <summary>Every Eye of Cthulhu entity of the ambient sky (slot order): position, velocity, depth, state, life time.</summary>
    static void PEocEntities(object sky, string key)
    {
        object sv = IF(sky, "_entities");
        Array arr = (Array)IF(sv, "_array");
        uint used = (uint)IF(sv, "_usedSpaceLength");
        MethodInfo has = MI(sv.GetType(), "Has", 1, "Int32");
        for (int i = 0; i < used; i++)
        {
            if (!(bool)has.Invoke(sv, new object[] { i })) continue;
            object e = IF(arr.GetValue(i), "Value");
            if (e == null || e.GetType().Name != "EOCSkyEntity") continue;
            P(key + "_" + i, IF(e, "Position"), IF(e, "Velocity"), IF(e, "Depth"), IF(e, "_state"), IF(e, "LifeTime"));
        }
    }

    static void ProbeF(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        // ---- luck rolls (Luck.RollBadLuck / RollBadLuckExtreme)
        if (Group("luck"))
        {
            foreach (float luck in new[] { -0.5f, -0.1f, 0f, 0.3f, 1f })
                foreach (int range in new[] { 10, 50 })
                {
                    var a = new List<object>();
                    for (int k = 0; k < 4; k++)
                    {
                        a.Add(CallS("Terraria.GameContent.Luck", "RollBadLuck", luck, range));
                        a.Add(CallS("Terraria.GameContent.Luck", "RollBadLuckExtreme", luck, range));
                    }
                    P("luck_" + PV(luck) + "_" + range, a.ToArray());
                }
        }
        // ---- Lucy the axe moved between the inventory and storage (LucyAxeMessage.ItemSlot_OnItemTransferred)
        if (Group("lucy"))
        {
            Type iti = T("Terraria.UI.ItemSlot+ItemTransferInfo");
            MethodInfo h = MI(T("Terraria.GameContent.LucyAxeMessage"), "ItemSlot_OnItemTransferred", 1);
            foreach (int[] c in new[] { new[] { 0, 3 }, new[] { 3, 0 }, new[] { 0, 0 }, new[] { 4, 0 } })
            {
                object info = Activator.CreateInstance(iti);
                iti.GetField("ItemType").SetValue(info, 5095);
                iti.GetField("TransferAmount").SetValue(info, 1);
                iti.GetField("FromContenxt").SetValue(info, c[0]);
                iti.GetField("ToContext").SetValue(info, c[1]);
                Unwrap(h, null, new object[] { info });
                Frames(1);
            }
            Frames(30);
            Record("lucy", 1);
        }
        // ---- the digger cart digging along a track underground (MinecartDiggerHelper.TryDigging)
        if (Group("digger"))
        {
            int tx = 1999, ty = 547;
            for (int x = tx - 6; x <= tx + 6; x++) { KillTile(x, ty); PlaceTile(x, ty, 314, 0); }
            object dh = SF(T("Terraria.GameContent.MinecartDiggerHelper"), "Instance");
            PlacePlayer(tx, ty - 1);
            PTiles("digger_before", tx - 10, ty - 6, 21, 10);
            foreach (int dir in new[] { 1, -1 })
                CallI(dh, "TryDigging", pl, new Vector2(tx * 16 + 8 + dir * 96, ty * 16 + 8), dir, 0);
            PTiles("digger_after", tx - 10, ty - 6, 21, 10);
            Frames(5);
            Record("digger", 1);
            PlacePlayer(sx, sy);
            Frames(5);
        }
        // ---- a boss damage report (NPCDamageTracker: King Slime hit by the player, the report and its percentages)
        if (Group("dmgreport"))
        {
            ClearEntities();
            int boss = NewNpc(sx * 16 + 300, sy * 16 - 200, 50);
            Type tr = T("Terraria.GameContent.NPCDamageTracker");
            foreach (int d in new[] { 120, 37, 455 }) CallS("Terraria.GameContent.NPCDamageTracker", "AddDamage", NpcAt(boss), (int)SF(TMain, "myPlayer"), d);
            IList active = (IList)SF(tr, "_activeTrackers");
            Note("trackers " + active.Count);
            object rep = CallI(active[0], "GetReport", pl);
            P("dmgreport", rep.ToString());
            P("dmgpercent", ((int[])CallS("Terraria.GameContent.NPCDamageTracker", "CalculatePercentages", new int[] { 120, 37, 455, 1 })).Select(x => (object)x).ToArray());
            CallS("Terraria.GameContent.NPCDamageTracker", "Reset");
            ClearEntities();
            Frames(2);
        }
        // ---- beds of every style: where the player would lie (PlayerSleepingHelper.GetSleepingTargetInfo ->
        //      SetOffsetbyBed), the player asleep in one (GetSleepingOffsetInfo)
        if (Group("beds"))
        {
            int bx = x0 + 4;
            for (int style = 0; style < 48; style++)
            {
                if (MI(T("Terraria.ObjectData.TileObjectData"), "GetTileData", 3, "Int32,Int32,Int32").Invoke(null, new object[] { 79, style, 0 }) == null) continue;
                ClearArea(x0, x0 + 20, gy);
                bool ok = (bool)TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { bx, gy - 1, 79, true, style, 0, -1, 1 });
                object[] a = { bx, gy - 1, 0, default(Vector2), default(Vector2) };
                object r = Unwrap(MI(T("Terraria.GameContent.PlayerSleepingHelper"), "GetSleepingTargetInfo", 5), null, a);
                P("bed_" + style, ok, r, a[2], a[3], a[4]);
            }
            ClearArea(x0, x0 + 20, gy);
            TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { bx, gy - 1, 79, true, 0, 0, -1, 1 });
            PlacePlayer(bx + 2, gy - 1);
            Frames(2);
            // (Player.sleeping is a struct: the call on a copy, written back, as the game's own `sleeping.StartSleeping`)
            FieldInfo sf = pl.GetType().GetField("sleeping");
            object box = sf.GetValue(pl);
            CallI(box, "StartSleeping", pl, bx, gy - 1);
            sf.SetValue(pl, box);
            Frames(30);
            Record("bed_sleep", 1);
            box = sf.GetValue(pl);
            CallI(box, "StopSleeping", pl, true);
            sf.SetValue(pl, box);
            ClearArea(x0, x0 + 20, gy);
            Frames(3);
        }
        // ---- portals: two portal gun bolts into two walls (PortalHelper.TryPlacingPortal -> AddPortal ->
        //      RemoveMyOldPortal / RemoveIntersectingPortals), the player walking through (GetPortalOutingPoint)
        if (Group("portal"))
        {
            ClearEntities();
            for (int y = gy - 8; y < gy; y++) { PlaceTile(x0 + 2, y, 1, 0); PlaceTile(x0 + 30, y, 1, 0); }
            PlacePlayer(x0 + 16, gy - 1);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            foreach (float[] b in new[] { new[] { 12f, 0f }, new[] { -12f, 1f }, new[] { 12f, 0f } })
            {
                int i = NewProj(pp.X + 10f, pp.Y + 10f, b[0], 0f, 601);
                ((float[])IF(ProjAt(i), "ai"))[0] = b[1];
                Frames(15);
            }
            int portals = 0;
            foreach (object pr in (Array)SF(TMain, "projectile")) if ((bool)IF(pr, "active") && (int)IF(pr, "type") == 602) portals++;
            P("portals", portals);
            InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.D };
            Frames(40);
            InKeys = null;
            P("portal_player", IF(pl, "position"));
            Record("portal", 1);
            ClearEntities();
            ClearArea(x0, x0 + 34, gy);
            PlacePlayer(sx, sy);
            Frames(5);
        }
        // ---- the potion of return's gates (ReturnGatePlayerRenderer: PotionOfReturnGateHelper, its dust and drawing)
        if (Group("returngate"))
        {
            PlacePlayer(sx, sy);
            var pp = (Vector2)IF(pl, "position");
            ISet(pl, "PotionOfReturnOriginalUsePosition", (Vector2?)new Vector2(pp.X + 100f, pp.Y + 40f));
            ISet(pl, "PotionOfReturnHomePosition", (Vector2?)new Vector2(pp.X - 120f, pp.Y + 40f));
            Frames(30);
            Record("returngate", 1);
            ISet(pl, "PotionOfReturnOriginalUsePosition", null);
            ISet(pl, "PotionOfReturnHomePosition", null);
            Frames(3);
        }
        // ---- quick stacking into a chest and a piggy bank next to the player (QuickStacking.QuickStackToNearbyInventories)
        if (Group("quickstack"))
        {
            PlacePlayer(x0 + 4, gy - 1);
            int cx = x0 + 6;
            PlaceTile(cx, gy - 1, 21, 0);
            PlaceTile(x0 + 9, gy - 1, 19, 0);
            PlaceTile(x0 + 10, gy - 1, 19, 0);
            TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { x0 + 9, gy - 2, 29, true, 0, 0, -1, 1 });
            int ci = (int)CallS("Terraria.Chest", "FindChest", cx, gy - 2);
            Note("quickstack chest " + ci + " " + CoverDiag(x0 + 4, gy - 3, 8, 3));
            object chest = ((Array)SF(TMain, "chest")).GetValue(ci);
            Array citems = (Array)IF(chest, "item");
            CallI(citems.GetValue(0), "SetDefaults", 9, null);
            ISet(citems.GetValue(0), "stack", 5);
            CallI(citems.GetValue(1), "SetDefaults", 3, null);
            ISet(citems.GetValue(1), "stack", 990);
            object bank = IF(pl, "bank");
            Array bitems = (Array)IF(bank, "item");
            CallI(bitems.GetValue(0), "SetDefaults", 2, null);
            ISet(bitems.GetValue(0), "stack", 7);
            SetInv(10, 9, 20); SetInv(11, 3, 30); SetInv(12, 2, 15);
            CallS("Terraria.GameContent.QuickStacking", "QuickStackToNearbyInventories", pl, false);
            Frames(5);
            P("quickstack", CountItem(9), CountItem(3), CountItem(2), IF(citems.GetValue(0), "stack"), IF(citems.GetValue(1), "stack"), IF(bitems.GetValue(0), "stack"));
            Record("quickstack", 1);
            for (int k = 10; k < 13; k++) GiveItem(k, 0);
            foreach (object it in citems) CallI(it, "TurnToAir");
            foreach (object it in bitems) CallI(it, "TurnToAir");
            ClearArea(x0, x0 + 12, gy);
            ClearItems();
            Frames(3);
        }
        // ---- weather skies: a blizzard in the snow (BlizzardSky, BlizzardShaderData), a sandstorm in the desert
        //      (SandstormSky, SandstormShaderData), the Martian madness (MartianSky: its UFOs)
        if (Group("weather"))
        {
            CallS("Terraria.Main", "StartRain", true, (float?)1f, false);
            PlacePlayer(2499, 298);
            Frames(150);
            Record("blizzard", 1);
            CallS("Terraria.Main", "StopRain", true);
            Frames(150);
            Type ss = T("Terraria.GameContent.Events.Sandstorm");
            Unwrap(MI(ss, "StartSandstorm", 0), null, new object[0]);
            SSet(ss, "Severity", 1f);
            SSet(ss, "IntendedSeverity", 1f);
            PlacePlayer(1720, 352);
            Frames(150);
            Record("sandstorm", 1);
            Unwrap(MI(ss, "StopSandstorm", 0), null, new object[0]);
            SSet(ss, "Severity", 0f);
            Frames(150);
            PlacePlayer(sx, sy);
            int life = (int)IF(pl, "statLifeMax");
            ISet(pl, "statLifeMax", 400);
            CallS("Terraria.Main", "StartInvasion", 4);
            Note("invasion " + SF(TMain, "invasionType"));
            Frames(60);
            Record("martian_sky", 1);
            SSet(TMain, "invasionType", 0);
            SSet(TMain, "invasionSize", 0);
            ISet(pl, "statLifeMax", life);
            ClearEntities();
            Frames(60);
        }
        // ---- shimmer-free landing spots (ShimmerHelper.FindSpotWithoutShimmer / IsSpotShimmerFree)
        if (Group("shimmerspot"))
        {
            foreach (int[] s in new[] { new[] { sx, sy, 5 }, new[] { sx + 40, sy - 20, 10 }, new[] { 1999, 547, 8 } })
                foreach (bool top in new[] { false, true })
                    P("shimmerspot_" + s[0] + "_" + s[1] + "_" + top, CallS("Terraria.GameContent.ShimmerHelper", "FindSpotWithoutShimmer", pl, s[0], s[1], s[2], top));
        }
        // ---- the Eye of Cthulhu in the ambient sky, chasing the player (AmbientSky.EOCSkyEntity: BeginChasingPlayer,
        //      ChasePlayerTop), several seeds
        if (Group("eocsky"))
        {
            object sky = SkyByName("Ambience");
            Type et = T("Terraria.GameContent.Ambience.SkyEntityType");
            MethodInfo spawn = sky.GetType().GetMethod("Spawn");
            SetTime(false, 16200.0);
            for (int seed = 2000; seed < 2012; seed++) spawn.Invoke(sky, new object[] { pl, Enum.ToObject(et, (byte)4), seed });
            PEocEntities(sky, "eocsky_spawned");
            Frames(1);
            PEocEntities(sky, "eocsky_1");
            Frames(119);
            PEocEntities(sky, "eocsky_120");
            Record("eocsky", 1);
            SetTime(true, 27000.0);
            Frames(30);
        }
    }
}
