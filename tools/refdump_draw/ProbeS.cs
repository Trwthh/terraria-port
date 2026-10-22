// The probe run, part S (Probe.cs; port: tests/draw/probe_s.rs, the same steps): menus shown further: the
// achievements list at an achievement with a progress tracker (its progress bar), the controls menu's gamepad list
// scrolled to its sliders, the new crafting window with recipes available, the defender medals' savings (a piggy
// bank's), the sun low and on screen beside a grown tree (the lit nature shader), the resource pack menus with the
// probe's pack listed as enabled, the stylist's hairstyle page, a legacy dungeon room asked for a dual dungeons biome
// chest, the journey research page with a stack big enough to research (its cogs turning), and seaweed in a pool.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    static void ProbeS(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        if (Group("achieve2"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            CallS("Terraria.UI.IngameFancyUI", "OpenAchievements");
            Frames(3);
            object menu = SF(TMain, "AchievementsMenu");
            CallI(menu, "GotoAchievement", CallI(SF(TMain, "Achievements"), "GetAchievement", "BULLDOZER"));
            Frames(2);
            Record("achievements_bulldozer", 1);
            CallS("Terraria.UI.IngameFancyUI", "Close", false);
            Frames(2);
            StateLine("achieve2");
        }
        if (Group("keybinds2"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            CallS("Terraria.UI.IngameFancyUI", "OpenKeybinds");
            Frames(3);
            object mc = SF(TMain, "ManageControlsMenu");
            CallI(mc, "GamepadButtonClick", MouseEvt(mc), mc);
            CallI(mc, "VsMenuButtonClick", MouseEvt(mc), mc);
            Frames(2);
            object list = IF(mc, "_uilist");
            for (int k = 0; k < 6; k++)
            {
                CallI(list, "ScrollWheel", New("Terraria.UI.UIScrollWheelEvent", list, new Vector2(640f, 360f), -360));
                Frames(2);
                Record("keybinds_scroll_" + k, 1);
            }
            CallI(mc, "KeyboardButtonClick", MouseEvt(mc), mc);
            CallI(mc, "VsGameplayButtonClick", MouseEvt(mc), mc);
            CallS("Terraria.UI.IngameFancyUI", "Close", false);
            Frames(2);
            StateLine("keybinds2");
        }
        if (Group("crafting2"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            SetInv(10, 9, 99);
            SetInv(11, 3, 99);
            SetInv(12, 23, 99);
            SSet(TMain, "playerInventory", true);
            CallS("Terraria.Recipe", "UpdateRecipeList");
            Frames(2);
            P("recipes", SF(TMain, "numAvailableRecipes"));
            Unwrap(MI(T("Terraria.GameContent.UI.NewCraftingUI"), "Open", 2), null, new object[] { true, null });
            Frames(4);
            Record("newcrafting2", 1);
            Unwrap(MI(T("Terraria.GameContent.UI.NewCraftingUI"), "Close", 2), null, new object[] { true, false });
            SSet(TMain, "playerInventory", false);
            for (int k = 10; k <= 12; k++) GiveItem(k, 0);
            Frames(2);
            StateLine("crafting2");
        }
        if (Group("currency2"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            SetInv(20, 3817, 37);
            // the savings count the piggy bank's medals (CustomCurrencySingleCoin.DrawSavingsMoney draws them)
            object bk = ((Array)IF(IF(pl, "bank"), "item")).GetValue(0);
            bk.GetType().GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int)).Invoke(bk, new object[] { 3817, null });
            ISet(bk, "stack", 37);
            object sb = SF(TMain, "spriteBatch");
            DirectDraw("currency_savings", () => InBatch(() =>
                Unwrap(MI(T("Terraria.GameContent.UI.CustomCurrencyManager"), "DrawSavings", 5), null, new object[] { sb, SF(T("Terraria.ID.CustomCurrencyID"), "DefenderMedals"), 500f, 300f, false })));
            DirectDraw("money_savings", () => InBatch(() =>
                SlotCall("DrawMoney", sb, "Savings", 500f, 300f, new[] { 12, 34, 5, 1 }, false, true)));
            GiveItem(20, 0);
            CallI(bk, "TurnToAir");
            StateLine("currency2");
        }
        // ---- the sun low on the horizon (NextHorizonRenderer's lens flares; NextNatureRenderer's lit nature shader where
        //      the sun is on screen: 6000 and 48000)
        if (Group("sunrise"))
        {
            ClearEntities();
            // a tree grown in the build area, the player beside it (its top and branches are what NextNatureRenderer
            // draws, lit by the low sun)
            ClearArea(x0, x0 + 40, gy);
            for (int x = x0 + 8; x <= x0 + 12; x++) ISet(TileAt(x, gy), "type", (ushort)2);
            P("sunrise_tree", CallStatic(TWorldGen, "GrowTree", x0 + 10, gy, 0, true));
            PlacePlayer(x0 + 4, gy - 1);
            bool day = (bool)SF(TMain, "dayTime");
            double time = (double)SF(TMain, "time");
            foreach (double t in new[] { 400.0, 1500.0, 6000.0, 48000.0, 52500.0, 53600.0 })
            {
                SetTime(true, t);
                Frames(20);
                Record("sun_" + (int)t, 1);
            }
            SetTime(day, time);
            ClearArea(x0, x0 + 40, gy);
            ClearEntities();
            ClearFx();
            ClearItems();
            Frames(5);
            StateLine("sunrise");
        }
        // ---- the resource pack menus with the probe's pack (its list item, icon, offset buttons, info page)
        if (Group("respacks"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            int menu = (int)SF(TMain, "menuMode");
            CallS("Terraria.Main", "OpenResourcePacksMenu", new object[] { null });
            object ui = SF(TMain, "MenuUI");
            object state = IF(ui, "CurrentState");
            CallI(ui, "SetState", new object[] { null });
            SSet(TMain, "menuMode", menu);
            CallS("Terraria.UI.IngameFancyUI", "OpenUIState", state);
            Frames(3);
            Record("respacks", 1);
            object packs = IF(state, "_packsList");
            object pack = ((IEnumerable)IF(packs, "AllPacks")).Cast<object>().FirstOrDefault();
            P("respacks", pack == null ? "-" : (string)IF(pack, "Name"));
            if (pack != null)
            {
                // the pack listed as enabled (only the menu's flag: nothing is loaded from it), its order buttons shown
                pack.GetType().GetProperty("IsEnabled").SetValue(pack, true, null);
                CallI(state, "PopulatePackList");
                Frames(3);
                Record("respacks_enabled", 1);
                pack.GetType().GetProperty("IsEnabled").SetValue(pack, false, null);
                CallI(state, "PopulatePackList");
                object info = New("Terraria.GameContent.UI.States.UIResourcePackInfoMenu", state, pack);
                CallS("Terraria.UI.IngameFancyUI", "OpenUIState", info);
                Frames(3);
                Record("respack_info", 1);
            }
            CallS("Terraria.UI.IngameFancyUI", "Close", false);
            Frames(2);
            StateLine("respacks");
        }
        // ---- the character creation's hairstyle page with the stylist's list (its buttons drawn, the gamepad points
        //      of the hairstyle container culled: UIGamepadHelper.CullPointsOutOfElementArea)
        if (Group("hairstyles"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            SSet(TMain, "hairWindow", true);
            CallI(SF(TMain, "Hairstyles"), "UpdateUnlocks");
            object np = Activator.CreateInstance(T("Terraria.Player"));
            Type cct = T("Terraria.GameContent.UI.States.UICharacterCreation");
            object cc = Activator.CreateInstance(cct, new object[] { np });
            Fancy(cc);
            Frames(3);
            Unwrap(MI(cct, "Click_HairStyles", 2), cc, new object[] { null, null });
            Frames(3);
            Record("hairstyles", 1);
            FancyClose();
            SSet(TMain, "hairWindow", false);
            CallI(SF(TMain, "Hairstyles"), "UpdateUnlocks");
            Frames(2);
            StateLine("hairstyles");
        }
        // ---- a legacy dungeon room asked for a dual dungeons biome chest (LegacyDungeonRoom.DualDungeons_TryGenerateBiomeChestInRoom:
        //      what the dual dungeons pass asks of a style's ordinary rooms when its biome room gives no chest)
        if (Group("dungeonchest"))
        {
            ClearEntities();
            PlacePlayer(x0 + 2, gy - 1);
            Frames(2);
            string ns = "Terraria.GameContent.Generation.Dungeon.";
            object rs = New(ns + "Rooms.LegacyDungeonRoomSettings");
            ISet(rs, "StyleData", SF(T(ns + "DungeonGenerationStyles"), "Snow"));
            object room = New(ns + "Rooms.LegacyDungeonRoom", rs);
            // below the ground at strength 2 (the chest's spot is the room's end, its 3 by 3 check area around it): the
            // check area's first tile is given the wall 350 for the call, so DungeonData.CanGenerateFeatureAt refuses it
            // before it reads the dungeon's entrance (which this room's data has none of) and nothing is placed
            int cx = x0 + 10, cy = gy + 20;
            ISet(room, "EndPosition", Activator.CreateInstance(room.GetType().GetField("EndPosition").FieldType, new object[] { (double)cx, (double)cy }));
            ISet(room, "Strength", 2);
            object wt = TileAt(cx - 1, cy - 1);
            ushort wall = (ushort)IF(wt, "wall");
            ISet(wt, "wall", (ushort)350);
            object data = New(ns + "DungeonData");
            // the feature registers itself with DungeonCrawler.CurrentDungeonData (dungeonData[GenVars.CurrentDungeon])
            var datas = (IList)SF(T(ns + "DungeonCrawler"), "dungeonData");
            Type gv = T("Terraria.WorldBuilding.GenVars");
            int cur = (int)SF(gv, "CurrentDungeon");
            int before = datas.Count;
            SSet(gv, "CurrentDungeon", datas.Count);
            datas.Add(data);
            object feat = New(ns + "Features.DungeonGlobalBiomeChests", New(ns + "Features.DungeonFeatureSettings"));
            P("dungeonchest", CallI(room, "DualDungeons_TryGenerateBiomeChestInRoom", data, feat));
            ISet(wt, "wall", wall);
            datas.RemoveAt(before);
            SSet(gv, "CurrentDungeon", cur);
            Frames(2);
            StateLine("dungeonchest");
        }
        if (Group("creative2"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            byte diff = (byte)IF(pl, "difficulty");
            ISet(pl, "difficulty", (byte)3);
            SSet(TMain, "playerInventory", true);
            object cm = SF(TMain, "CreativeMenu");
            // opened (ToggleMenu is a toggle: an earlier group may have left the menu open)
            System.Reflection.PropertyInfo cmEnabled = cm.GetType().GetProperty("Enabled");
            if (!(bool)cmEnabled.GetValue(cm, null)) CallI(cm, "ToggleMenu");
            Frames(3);
            object st = IF(cm, "_uiState");
            // the research page (a toggle: an earlier group may have left it open)
            if ((int)IF(IF(st, "_mainCategory"), "CurrentOption") != 2) CallI(st, "ToggleMainCategory", 2);
            // as UICreativePowersMenu.MainCategoryButtonClick does: the research page shown (its window's OnUpdate turns the cogs)
            CallI(st, "RefreshElementsOrder");
            Frames(3);
            object it = NewItemOf(9);
            ISet(it, "stack", 999);
            CallI(cm, "SetItembyIndex", it, 0);
            object[] nums = { 0, 0, 0 };
            P("research_numbers", CallI(cm, "GetSacrificeNumbers", nums), nums[0], nums[1], nums[2], CallI(cm, "IsShowingResearchMenu"));
            CallI(cm, "SacrificeItemInSacrificeSlot");
            for (int k = 0; k < 4; k++)
            {
                Frames(15);
                Record("creative_research2_" + k, 1);
            }
            if ((bool)cmEnabled.GetValue(cm, null)) CallI(cm, "ToggleMenu");
            SSet(TMain, "playerInventory", false);
            ISet(pl, "difficulty", diff);
            ClearItems();
            Frames(2);
            StateLine("creative2");
        }
        // ---- seaweed (tile 549, TileDrawing.DrawReverseVines / DrawRisingVineStrip): a pool six deep dug into the build
        //      area's ground over sand, three seaweed stalks of three tiles each grown from the sand (WorldGen.PlaceUnderwaterPlant)
        if (Group("seaweed"))
        {
            ClearEntities();
            PlacePlayer(x0 + 24, gy - 1);
            Frames(2);
            int wx = x0 + 30;
            var walls = new ushort[4, 7];
            for (int x = wx; x < wx + 4; x++)
            {
                for (int y = gy; y <= gy + 6; y++) KillTile(x, y);
                for (int y = gy; y <= gy + 5; y++)
                {
                    object t = TileAt(x, y);
                    // the ground's dirt wall would keep the seaweed from growing (CanUnderwaterPlantGrowHere)
                    walls[x - wx, y - gy] = (ushort)IF(t, "wall");
                    ISet(t, "wall", (ushort)0);
                    ISet(t, "liquid", (byte)255);
                    t.GetType().GetMethod("liquidType", new[] { typeof(int) }).Invoke(t, new object[] { 0 });
                }
                PlaceTile(x, gy + 6, 53, 0);
            }
            for (int x = wx; x < wx + 4; x += 1)
            {
                if (x == wx + 1) continue;
                for (int y = gy + 5; y >= gy + 3; y--) CallStatic(TWorldGen, "PlaceUnderwaterPlant", (ushort)549, x, y);
            }
            PTiles("seaweed", wx, gy, 4, 7);
            Frames(10);
            Record("seaweed", 1);
            for (int x = wx; x < wx + 4; x++)
                for (int y = gy; y <= gy + 6; y++)
                {
                    KillTile(x, y);
                    ISet(TileAt(x, y), "liquid", (byte)0);
                    if (y <= gy + 5) ISet(TileAt(x, y), "wall", walls[x - wx, y - gy]);
                    PlaceTile(x, y, 0, 0);
                }
            ClearEntities();
            ClearFx();
            ClearItems();
            Frames(2);
            StateLine("seaweed");
        }
    }
}
