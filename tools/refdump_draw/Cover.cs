// The coverage run (`RefDumpDraw.exe <dir> cover`, refdata\cover; port: tests/draw.rs DRAW_MODE=cover,
// tests/draw/cover.rs, the same steps): float methods that are reachable in the client and that no earlier reference run
// executed (tools\cs2rs\float_uncovered.tsv, reason "notrun", largest first, tools\cs2rs\FLOAT_COVERAGE.md): display
// dolls holding every kind of weapon in every pose they support and hat racks, doors and a trapdoor opened and closed, a
// decorated Christmas tree and minecart tracks, two wired teleporters and a logic gate, the inventory's banner and recipe
// pages and its housing page, the map saved and loaded again, every accessory worn (five at a time), every mount ridden,
// the party, lantern-night and Martian skies, the Wall of Flesh summoned. Scenes 1 frame each, every frame's digest,
// state lines as the scenes run.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;

public static partial class WG
{
    static bool CoverMode;

    static object NewItemOf(int type)
    {
        object it = Activator.CreateInstance(T("Terraria.Item"));
        it.GetType().GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int)).Invoke(it, new object[] { type, null });
        return it;
    }

    /// <summary>The weapons of the display-doll gallery: for every projectile aiStyle, the first item (by type) that has a
    /// use style the dolls support (TEDisplayDoll.SupportedUseStylePoses) and shoots a projectile of that aiStyle; with
    /// every pose its use style supports (port: cover_doll_items, the same rule).</summary>
    static List<int[]> CoverDollItems()
    {
        var list = new List<int[]>();
        Type itemT = T("Terraria.Item"), projT = T("Terraria.Projectile");
        object it = Activator.CreateInstance(itemT);
        object pr = Activator.CreateInstance(projT);
        MethodInfo isd = itemT.GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int));
        MethodInfo psd = projT.GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 1);
        IDictionary poses = (IDictionary)SF(T("Terraria.GameContent.Tile_Entities.TEDisplayDoll"), "SupportedUseStylePoses");
        var seen = new HashSet<int>();
        int count = (short)SF(T("Terraria.ID.ItemID"), "Count");
        for (int t = 1; t < count; t++)
        {
            isd.Invoke(it, new object[] { t, null });
            int us = (int)IF(it, "useStyle"), shoot = (int)IF(it, "shoot");
            if (shoot <= 0 || shoot >= ProjCount || !poses.Contains(us)) continue;
            psd.Invoke(pr, new object[] { shoot });
            int ai = (int)IF(pr, "aiStyle");
            if (!seen.Add(ai)) continue;
            foreach (object p in (IEnumerable)poses[us]) list.Add(new[] { t, Convert.ToInt32(IF(p, "Pose")) });
        }
        return list;
    }

    /// <summary>The active tile types of a rectangle, row by row ("." = empty), for the notes.</summary>
    static string CoverDiag(int x, int y, int w, int h)
    {
        var sb = new System.Text.StringBuilder();
        for (int j = y; j < y + h; j++)
        {
            sb.Append(j == y ? "" : "|");
            for (int i = x; i < x + w; i++)
            {
                object t = TileAt(i, j);
                bool act = t != null && (bool)t.GetType().GetMethod("active", Type.EmptyTypes).Invoke(t, null);
                sb.Append(i == x ? "" : " ").Append(act ? Convert.ToString(IF(t, "type")) : ".");
            }
        }
        return sb.ToString();
    }

    static void CoverWorld()
    {
        int sx = (int)SF(TMain, "spawnTileX"), sy = (int)SF(TMain, "spawnTileY");
        object pl = LocalPlayer;
        SetTime(true, 27000.0);
        PlacePlayer(sx, sy);
        Immune();
        Frames(30);
        int x0 = sx + BuildDX;
        int gy = GroundBelow(x0, sy - 12);
        Note("cover ground at " + x0 + "," + gy);
        for (int x = x0 - 2; x <= x0 + BuildW; x++)
        {
            for (int y = gy - 12; y < gy; y++) KillTile(x, y);
            for (int y = gy; y <= gy + 1; y++) PlaceTile(x, y, 0, 0);
        }
        Frames(5);
        // ---- 1. display dolls: six side by side, each holding a weapon in one of its poses
        {
            Type tdd = T("Terraria.GameContent.Tile_Entities.TEDisplayDoll");
            MethodInfo po = TWorldGen.GetMethod("PlaceObject", AllStatic);
            var ids = new List<object>();
            for (int k = 0; k < 6; k++)
            {
                int x = x0 + 40 + k * 3;
                Note("doll " + k + ": " + po.Invoke(null, new object[] { x, gy - 1, 470, true, 0, 0, -1, 1 }));
                int id = (int)CallStatic(tdd, "Hook_AfterPlacement", x, gy - 1, 470, 0, 1, 0);
                ids.Add(((IDictionary)SF(T("Terraria.DataStructures.TileEntity"), "ByID"))[id]);
            }
            var equip = new[] { 231, 232, 233 };
            foreach (object d in ids)
            {
                Array eq = (Array)IF(d, "_equip");
                for (int k = 0; k < 3; k++) eq.SetValue(NewItemOf(equip[k]), k);
            }
            PlacePlayer(x0 + 38, gy - 1);
            Frames(5);
            List<int[]> items = CoverDollItems();
            Note("doll items: " + items.Count + ": " + string.Join(",", items.Select(i => i[0] + "/" + i[1]).ToArray()));
            for (int b = 0; b < items.Count; b += 6)
            {
                for (int k = 0; k < 6; k++)
                {
                    object d = ids[k];
                    int[] e = b + k < items.Count ? items[b + k] : new[] { 0, 0 };
                    ((Array)IF(d, "_misc")).SetValue(NewItemOf(e[0]), 0);
                    ISet(d, "_pose", (byte)e[1]);
                }
                Frames(2);
                Record("doll_" + b, 1);
            }
            // hat racks with three hats each, next to the dolls
            Type thr = T("Terraria.GameContent.Tile_Entities.TEHatRack");
            for (int k = 0; k < 2; k++)
            {
                int x = x0 + 60 + k * 4;
                Note("hatrack " + k + ": " + po.Invoke(null, new object[] { x, gy - 1, 475, true, 0, 0, -1, 1 }));
                int id = (int)CallStatic(thr, "Hook_AfterPlacement", x, gy - 1, 475, 0, 1, 0);
                object h = ((IDictionary)SF(T("Terraria.DataStructures.TileEntity"), "ByID"))[id];
                Array hi = (Array)IF(h, "_items");
                hi.SetValue(NewItemOf(k == 0 ? 1243 : 3884), 0);
                hi.SetValue(NewItemOf(k == 0 ? 1282 : 869), 1);
            }
            Frames(3);
            Record("hatracks", 1);
            // (a display doll or hat rack holding items cannot be broken: they stay, right of the area used below)
            Frames(3);
        }
        // ---- 2. doors: a wooden door opened to both sides and closed, a trapdoor opened and closed
        {
            int dx = x0 + 10;
            PlaceTile(dx, gy - 4, 0, 0);
            Note("door: " + PlaceTile(dx, gy - 1, 10, 0) + " " + CoverDiag(dx - 1, gy - 4, 3, 5));
            PlacePlayer(dx - 3, gy - 1);
            Frames(3);
            Note("open right: " + CallStatic(TWorldGen, "OpenDoor", dx, gy - 2, 1));
            Frames(5);
            Record("door_open", 1);
            Note("close: " + CallStatic(TWorldGen, "CloseDoor", dx, gy - 2, false));
            Frames(5);
            Note("open left: " + CallStatic(TWorldGen, "OpenDoor", dx, gy - 2, -1));
            Frames(5);
            Note("close: " + CallStatic(TWorldGen, "CloseDoor", dx, gy - 2, false));
            Frames(3);
            int tx = x0 + 20;
            for (int x = tx; x <= tx + 1; x++) { KillTile(x, gy); KillTile(x, gy + 1); }
            Note("trapdoor: " + TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { tx, gy, 387, true, 0, 0, -1, 1 }) + " " + CoverDiag(tx - 1, gy - 1, 4, 4));
            Frames(3);
            Note("trapdoor open: " + CallStatic(TWorldGen, "ShiftTrapdoor", tx, gy, false, -1));
            Frames(5);
            Record("trapdoor_open", 1);
            // the open trapdoor (tile 386): closed again from its first tile
            int ox = -1, oy = -1;
            for (int y = gy; y <= gy + 2 && ox < 0; y++)
                for (int x = tx; x <= tx + 1 && ox < 0; x++)
                {
                    object t = TileAt(x, y);
                    if (t != null && (bool)t.GetType().GetMethod("active", Type.EmptyTypes).Invoke(t, null) && Convert.ToInt32(IF(t, "type")) == 386) { ox = x; oy = y; }
                }
            Note("trapdoor open at " + ox + "," + oy + " " + CoverDiag(tx - 1, gy - 1, 4, 4));
            Note("trapdoor close: " + (ox >= 0 && (bool)CallStatic(TWorldGen, "ShiftTrapdoor", ox, oy, true, -1)));
            Frames(5);
            for (int x = x0; x <= x0 + 24; x++) for (int y = gy - 4; y < gy; y++) KillTile(x, y);
            for (int x = tx; x <= tx + 1; x++) { KillTile(x, gy); PlaceTile(x, gy, 0, 0); PlaceTile(x, gy + 1, 0, 0); }
            Frames(3);
        }
        // ---- 3. a Christmas tree with its decorations, minecart tracks
        {
            int xx = x0 + 6;
            CallStatic(TWorldGen, "PlaceXmasTree", xx, gy - 1, (ushort)171);
            object t = TileAt(xx - 1, gy - 8);
            Note("xmas tree: " + (t != null && (bool)t.GetType().GetMethod("active", Type.EmptyTypes).Invoke(t, null)) + " " + CoverDiag(xx - 1, gy - 8, 4, 9));
            // the decorations live in the top-left tile's frameY bits (WorldGen.dropXmasTree reads them): star, garland,
            // bulbs, lights
            if (t != null) ISet(t, "frameY", (short)(1 | (1 << 3) | (1 << 7) | (1 << 11)));
            for (int x = x0 + 14; x <= x0 + 24; x++) PlaceTile(x, gy - 1, 314, 0);
            PlacePlayer(x0 + 2, gy - 1);
            Frames(5);
            Record("xmas_tracks", 1);
            KillTile(xx, gy - 1);
            Frames(5);
            Record("xmas_killed", 1);
            for (int x = x0; x <= x0 + 26; x++) for (int y = gy - 9; y < gy; y++) KillTile(x, y);
            ClearEntities();
            Frames(3);
        }
        // ---- 4. two teleporters on one wire with a switch, and a logic gate with a lamp
        {
            Type wiring = T("Terraria.Wiring");
            int a = x0 + 4, b = x0 + 20;
            Note("teleporter a: " + PlaceTile(a, gy - 1, 235, 0) + ", b: " + PlaceTile(b, gy - 1, 235, 0) + ", switch: " + PlaceTile(x0 + 12, gy - 1, 136, 0) + " " + CoverDiag(a - 1, gy - 2, 5, 3));
            for (int x = a; x <= b + 2; x++) CallStatic(TWorldGen, "PlaceWire", x, gy);
            for (int x = a; x <= b + 2; x++) CallStatic(TWorldGen, "PlaceWire", x, gy - 1);
            PlacePlayer(a + 1, gy - 1);
            Frames(5);
            CallStatic(wiring, "HitSwitch", x0 + 12, gy - 1);
            Frames(10);
            Record("teleport", 1);
            int lx = x0 + 28;
            Note("gate: " + PlaceTile(lx, gy - 1, 420, 0) + ", lamp: " + PlaceTile(lx, gy - 2, 419, 0) + " " + CoverDiag(lx - 1, gy - 3, 3, 4));
            for (int y = gy - 1; y <= gy - 1; y++) CallStatic(TWorldGen, "PlaceWire", lx - 1, y);
            CallStatic(TWorldGen, "PlaceWire", lx, gy - 1);
            CallStatic(TWorldGen, "PlaceWire", lx - 1, gy - 1);
            CallStatic(TWorldGen, "PlaceWire", lx - 2, gy - 1);
            Note("switch 2: " + PlaceTile(lx - 2, gy - 1, 136, 0));
            CallStatic(wiring, "HitSwitch", lx - 2, gy - 1);
            Frames(10);
            Record("logicgate", 1);
            for (int x = x0 - 2; x <= x0 + 34; x++) for (int y = gy - 6; y < gy; y++) { KillTile(x, y); CallStatic(TWorldGen, "KillWire", x, y); }
            for (int x = x0 - 2; x <= x0 + 34; x++) CallStatic(TWorldGen, "KillWire", x, gy);
            Frames(3);
        }
        // ---- 5. the inventory: the banner page, the recipe grid, the banner grid, the housing page
        {
            PlacePlayer(sx, sy);
            SSet(TMain, "playerInventory", true);
            Type pp = T("Terraria.Main+PipPage");
            // banners to claim (BannerSystem.claimableBanners: the banner page's grid needs some)
            Array cb = (Array)SF(T("Terraria.GameContent.BannerSystem"), "claimableBanners");
            for (int k = 1; k <= 6; k++) cb.SetValue((ushort)3, k);
            CallStatic(TMain, "TryChangePipsPage", Enum.ToObject(pp, 1));
            Frames(3);
            Record("inv_banners", 1);
            SSet(TMain, "PipsUseGrid", true);
            Frames(3);
            Record("inv_banners_grid", 1);
            CallStatic(TMain, "TryChangePipsPage", Enum.ToObject(pp, 0));
            SSet(TMain, "PipsUseGrid", true);
            Frames(3);
            Record("inv_recipes_grid", 1);
            SSet(TMain, "PipsUseGrid", false);
            SSet(TMain, "EquipPageSelected", 1);
            SSet(TMain, "EquipPage", 1);
            Frames(3);
            Record("inv_housing", 1);
            SSet(TMain, "EquipPageSelected", 0);
            SSet(TMain, "EquipPage", 0);
            SSet(TMain, "playerInventory", false);
            for (int k = 1; k <= 6; k++) cb.SetValue((ushort)0, k);
            Frames(3);
        }
        // ---- 6. the map saved and loaded again (WorldMap.Save / Load: the compressed format)
        {
            object map = SF(TMain, "Map");
            map.GetType().GetMethod("Save").Invoke(map, null);
            map.GetType().GetMethod("Load").Invoke(map, null);
            Frames(3);
            SSet(TMain, "mapFullscreen", true);
            Frames(3);
            Record("map_loaded", 1);
            SSet(TMain, "mapFullscreen", false);
            Frames(3);
        }
        // ---- 7. every accessory worn, five at a time (the five usable accessory slots 3 - 7)
        {
            Type itemT = T("Terraria.Item");
            object it = Activator.CreateInstance(itemT);
            MethodInfo isd = itemT.GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int));
            var acc = new List<int>();
            int count = (short)SF(T("Terraria.ID.ItemID"), "Count");
            for (int t = 1; t < count; t++)
            {
                isd.Invoke(it, new object[] { t, null });
                if ((bool)IF(it, "accessory")) acc.Add(t);
            }
            Note("accessories: " + acc.Count);
            for (int b = 0; b < acc.Count; b += 5)
            {
                PlacePlayer(sx, sy);
                for (int k = 0; k < 5; k++) SetArmor(3 + k, b + k < acc.Count ? acc[b + k] : 0);
                Frames(3);
                Record("acc_" + b, 1);
            }
            for (int k = 0; k < 5; k++) SetArmor(3 + k, 0);
            ClearEntities();
            Frames(5);
        }
        // ---- 8. every mount ridden, walking right
        {
            int mc = (int)SF(T("Terraria.ID.MountID"), "Count");
            for (int mt = 0; mt < mc; mt++)
            {
                PlacePlayer(sx, sy);
                object mount = IF(pl, "mount");
                mount.GetType().GetMethod("SetMount").Invoke(mount, new object[] { mt, pl });
                InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.D };
                Frames(12);
                Record("mount_" + mt, 1);
                InKeys = null;
                mount = IF(pl, "mount");
                mount.GetType().GetMethods().First(m => m.Name == "Dismount").Invoke(mount, new object[] { pl, false });
                Frames(3);
            }
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(10);
        }
        // ---- 9. skies: a party, lantern night, the Martian madness
        {
            CallStatic(T("Terraria.GameContent.Events.BirthdayParty"), "ToggleManualParty");
            Frames(60);
            Record("sky_party", 1);
            CallStatic(T("Terraria.GameContent.Events.BirthdayParty"), "ToggleManualParty");
            SetTime(false, 3600.0);
            CallStatic(T("Terraria.GameContent.Events.LanternNight"), "ToggleManualLanterns");
            Frames(90);
            Record("sky_lanterns", 1);
            CallStatic(T("Terraria.GameContent.Events.LanternNight"), "ToggleManualLanterns");
            SetTime(true, 27000.0);
            Frames(30);
            CallStatic(TMain, "StartInvasion", 4);
            Frames(90);
            Record("sky_martian", 1);
            SSet(TMain, "invasionType", 0);
            SSet(TMain, "invasionSize", 0);
            ClearEntities();
            Frames(30);
        }
        // ---- 10. the Wall of Flesh summoned (NPC.SpawnWOF at the player)
        {
            var p = (Microsoft.Xna.Framework.Vector2)IF(pl, "position");
            Type npcT = T("Terraria.NPC");
            npcT.GetMethod("SpawnWOF", AllStatic).Invoke(null, new object[] { p });
            Frames(30);
            Record("wof_spawn", 1);
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(20);
        }
        CoverWorld2(sx, sy, x0, gy);
    }

    /// <summary>The second part: in-game UI states (character list, achievements, controls, world creation and its
    /// secret seeds, the virtual keyboard), the four lighting modes, the torch god,
    /// the mystic sash's dodge, every sentry carried (Projectile.ai[2] = 1).</summary>
    static void CoverWorld2(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        // ---- 11. UI states shown in game (IngameFancyUI.OpenUIState)
        {
            string ns = "Terraria.GameContent.UI.States.";
            object wc = Activator.CreateInstance(T(ns + "UIWorldCreation"));
            object adv = Activator.CreateInstance(T(ns + "UIWorldCreationAdvanced"), new object[] { wc, false });
            var states = new List<KeyValuePair<string, object>>
            {
                new KeyValuePair<string, object>("ui_charselect", Activator.CreateInstance(T(ns + "UICharacterSelect"))),
                new KeyValuePair<string, object>("ui_achievements", SF(TMain, "AchievementsMenu")),
                new KeyValuePair<string, object>("ui_controls", SF(TMain, "ManageControlsMenu")),
                new KeyValuePair<string, object>("ui_worldcreation", wc),
                new KeyValuePair<string, object>("ui_worldcreation_adv", adv),
                new KeyValuePair<string, object>("ui_secretseeds", Activator.CreateInstance(T(ns + "UIWorldCreationAdvancedSecretSeedsList"), new object[] { adv, wc })),
                new KeyValuePair<string, object>("ui_keyboard", Activator.CreateInstance(T(ns + "UIVirtualKeyboard"), new object[] { "Test", "abc", null, null, 0, false, 20 })),
            };
            foreach (var kv in states)
            {
                Fancy(kv.Value);
                Frames(10);
                Record(kv.Key, 1);
                FancyClose();
                Frames(3);
            }
        }
        // ---- 12. the four lighting modes (Lighting.Mode), back to Color
        {
            Type lm = T("Terraria.Graphics.Light.LightMode");
            PropertyInfo mode = T("Terraria.Lighting").GetProperty("Mode", AllStatic);
            for (int k = 0; k < 4; k++)
            {
                mode.SetValue(null, Enum.ToObject(lm, k), null);
                Frames(10);
                Record("light_" + k, 1);
            }
            mode.SetValue(null, Enum.ToObject(lm, 3), null);
            Frames(5);
        }
        // ---- 13. the torch god: torches around the player, the event switched on
        {
            PlacePlayer(x0 + 10, gy - 1);
            for (int k = 0; k < 12; k++) PlaceTile(x0 + k * 2, gy - 1, 4, 0);
            Frames(3);
            ISet(pl, "happyFunTorchTime", true);
            Frames(90);
            Record("torchgod", 1);
            ISet(pl, "happyFunTorchTime", false);
            ClearEntities();
            for (int k = 0; k < 12; k++) KillTile(x0 + k * 2, gy - 1);
            Frames(5);
        }
        // ---- 14. the mystic sash (item 6189): Player.Hurt until it dodges
        {
            Type pdr = T("Terraria.DataStructures.PlayerDeathReason");
            MethodInfo hurt = pl.GetType().GetMethod("Hurt", new[] { pdr, typeof(int), typeof(int), typeof(bool), typeof(bool), typeof(bool), typeof(int), typeof(bool) });
            PlacePlayer(sx, sy);
            SetArmor(3, 6189);
            Frames(5);
            for (int i = 0; i < 60; i++)
            {
                ISet(pl, "immune", false);
                ISet(pl, "immuneTime", 0);
                ISet(pl, "statLife", 100);
                hurt.Invoke(pl, new object[] { pdr.GetMethod("LegacyDefault", AllStatic).Invoke(null, null), 1, 1, false, false, false, -1, true });
                Frames(2);
            }
            Record("player_mysticsash", 1);
            SetArmor(3, 0);
            Immune();
            Frames(5);
        }
        // ---- 15. every sentry, carried (ai[2] = 1), ten at a time
        {
            Type projT = T("Terraria.Projectile");
            object pr = Activator.CreateInstance(projT);
            MethodInfo psd = projT.GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 1);
            var sentries = new List<int>();
            for (int t = 1; t < ProjCount; t++)
            {
                psd.Invoke(pr, new object[] { t });
                if ((bool)IF(pr, "sentry")) sentries.Add(t);
            }
            Note("sentries: " + string.Join(",", sentries.Select(i => i.ToString()).ToArray()));
            Array projs = (Array)SF(TMain, "projectile");
            var p0 = (Microsoft.Xna.Framework.Vector2)IF(pl, "position");
            for (int b = 0; b < sentries.Count; b += 10)
            {
                ClearEntities();
                PlacePlayer(sx, sy);
                for (int k = 0; k < 10 && b + k < sentries.Count; k++)
                {
                    int i = NewProj(p0.X - 160f + k * 32f, p0.Y - 40f, 0f, 0f, sentries[b + k]);
                    object q = projs.GetValue(i);
                    float[] ai = (float[])IF(q, "ai");
                    ai[2] = 1f;
                }
                Frames(20);
                Record("sentry_" + b, 1);
            }
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(10);
        }
        // ---- 16. a drunk world's music choice (Main.UpdateAudio_DecideOnTOWMusic: drunkWorld, not remix / for the worthy)
        {
            SSet(TMain, "drunkWorld", true);
            Frames(30);
            Record("drunk_music", 1);
            SSet(TMain, "drunkWorld", false);
            Frames(5);
        }
        // ---- 17. the gamepad user interface (PlayerInput.CurrentInputMode = XBoxGamepadUI): the inventory, a chest,
        //          the controls and the emotes menus
        {
            Type pi = T("Terraria.GameInput.PlayerInput");
            PropertyInfo cim = pi.GetProperty("CurrentInputMode", AllStatic);
            object mouseMode = cim.GetValue(null, null);
            cim.SetValue(null, Enum.ToObject(T("Terraria.GameInput.InputMode"), 4), null);
            SSet(TMain, "playerInventory", true);
            Frames(5);
            Record("pad_inventory", 1);
            SSet(TMain, "playerInventory", false);
            Frames(3);
            Fancy(SF(TMain, "ManageControlsMenu"));
            Frames(5);
            Record("pad_controls", 1);
            FancyClose();
            Frames(3);
            Fancy(Activator.CreateInstance(T("Terraria.GameContent.UI.States.UIEmotesMenu")));
            Frames(5);
            Record("pad_emotes", 1);
            FancyClose();
            Frames(3);
            Note("input mode at the end: " + cim.GetValue(null, null));
            cim.SetValue(null, mouseMode, null);
            Frames(5);
        }
        // ---- 18. the surface backgrounds of the corruption, the hallow and the crimson: 333 blocks of the biome's stone
        //          under the flattened ground (SceneMetrics' thresholds: 300 evil, 125 holy tiles), the player on it
        {
            foreach (int[] bio in new[] { new[] { 25, 0 }, new[] { 117, 1 }, new[] { 203, 2 } })
            {
                for (int x = x0 - 2; x <= x0 + 34; x++) for (int y = gy + 1; y <= gy + 9; y++) { KillTile(x, y); PlaceTile(x, y, bio[0], 0); }
                PlacePlayer(x0 + 16, gy - 1);
                Frames(150);
                Record("bg_biome_" + bio[1], 1);
                for (int x = x0 - 2; x <= x0 + 34; x++) for (int y = gy + 1; y <= gy + 9; y++) { KillTile(x, y); PlaceTile(x, y, 0, 0); }
                Frames(30);
            }
            ClearEntities();
            Frames(5);
        }
        // ---- 19. an item left in shimmer until it transforms (WorldItem.UpdateShimmer: shimmerTime above 0.9)
        {
            int px = x0 + 4;
            for (int x = px; x < px + 6; x++)
                for (int y = gy; y <= gy + 1; y++)
                {
                    KillTile(x, y);
                    object t = TileAt(x, y);
                    ISet(t, "liquid", (byte)255);
                    t.GetType().GetMethod("liquidType", new[] { typeof(int) }).Invoke(t, new object[] { 3 });
                }
            for (int x = px - 1; x <= px + 6; x++) PlaceTile(x, gy + 2, 0, 0);
            PlaceTile(px - 1, gy + 1, 0, 0); PlaceTile(px + 6, gy + 1, 0, 0);
            MethodInfo newItem = T("Terraria.Item").GetMethods(AllStatic).First(m => m.Name == "NewItem" && m.GetParameters().Length == 12);
            foreach (int type in new[] { 9, 75 })
                newItem.Invoke(null, new object[] { Activator.CreateInstance(T("Terraria.DataStructures.EntitySource_DebugCommand")), (px + 2) * 16, gy * 16, 16, 16, type, 1, false, 0, Enum.ToObject(newItem.GetParameters()[9].ParameterType, 0), null, null });
            PlacePlayer(px - 3, gy - 1);
            Frames(240);
            Record("shimmer_item_long", 1);
        }
    }
}
