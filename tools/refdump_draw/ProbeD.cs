// The probe run, part D (Probe.cs; port: tests/draw/probe_d.rs, the same steps): crafting, coins taken back from
// enemies, journey-mode helpers, the constant world's darkness, doors opening by themselves, lava clouds, living tree
// leaves, furniture flames, town slimes transforming, the item cap, small direct calls.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    static void ClearItems()
    {
        foreach (object wi in (Array)SF(TMain, "item")) if (wi != null && (bool)IF(wi, "active")) CallI(wi, "TurnToAir");
    }
    static int CountItem(int type)
    {
        int n = 0;
        foreach (object it in (Array)IF(LocalPlayer, "inventory")) if ((int)IF(it, "type") == type) n += (int)IF(it, "stack");
        return n;
    }

    static void ProbeD(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        // ---- crafting from the inventory (CraftingRequests.CraftItem -> ConsumeItemsFrom), the recipe button hovered
        //      (Main.HoverOverCraftingItemButton)
        if (Group("craft"))
        {
            PlacePlayer(sx, sy);
            SetInv(0, 9, 30);
            SSet(TMain, "playerInventory", true);
            CallS("Terraria.Recipe", "UpdateRecipeList");
            Frames(2);
            Array recipes = (Array)SF(TMain, "recipe");
            int wb = -1;
            for (int i = 0; i < recipes.Length && wb < 0; i++)
            {
                object r = recipes.GetValue(i);
                if (r != null && (int)IF(IF(r, "createItem"), "type") == 36) wb = i;
            }
            Note("work bench recipe " + wb);
            CallS("Terraria.GameContent.CraftingRequests", "CraftItem", recipes.GetValue(wb), 1, false);
            P("craft_wood", CountItem(9), IF(SF(TMain, "mouseItem"), "type"), IF(SF(TMain, "mouseItem"), "stack"));
            CallS("Terraria.Recipe", "UpdateRecipeList");
            int focus = (int)SF(TMain, "focusRecipe");
            Note("focus recipe " + focus + " of " + SF(TMain, "numAvailableRecipes"));
            CallS("Terraria.Main", "HoverOverCraftingItemButton", focus);
            Frames(2);
            Record("craft", 1);
            ((Array)IF(pl, "inventory")).GetValue(0).GetType().GetMethod("TurnToAir").Invoke(((Array)IF(pl, "inventory")).GetValue(0), null);
            object mi = SF(TMain, "mouseItem");
            mi.GetType().GetMethod("TurnToAir").Invoke(mi, null);
            SSet(TMain, "playerInventory", false);
            Frames(2);
        }
        // ---- coins an enemy picked up and took away (CoinLossRevengeSystem.CacheEnemy, a marker), the enemy coming
        //      back when the player is near the marker (CheckRespawns -> WouldNPCBeDiscouraged, SpawnEnemy)
        if (Group("revenge"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            object rm = SF(T("Terraria.NPC"), "RevengeManager");
            int min = (int)SF(rm.GetType(), "MinimumCoinsForCaching");
            foreach (int[] e in new[] { new[] { 3, 900 }, new[] { 1, 1300 }, new[] { 2, -1100 }, new[] { 489, 1600 } })
            {
                int id = NewNpc((int)pp.X + e[1], (int)pp.Y, e[0]);
                object n = NpcAt(id);
                ISet(n, "extraValue", min * 3);
                CallI(rm, "CacheEnemy", n);
                ISet(n, "active", false);
            }
            Frames(3);
            for (int k = 0; k < 6; k++)
            {
                PlacePlayer(sx + k * 20, sy);
                Frames(5);
            }
            Record("revenge", 1);
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(3);
        }
        // ---- journey-mode helpers: the wind slider's hover text (CreativePowers.ModifyWindDirectionAndStrength), the
        //      item browser's search filter (ItemFilters.BySearch)
        if (Group("creative"))
        {
            object wind = New("Terraria.GameContent.Creative.CreativePowers+ModifyWindDirectionAndStrength");
            object el = New("Terraria.UI.UIElement");
            T("Terraria.UI.UIElement").GetField("<IsMouseHovering>k__BackingField", AllInst).SetValue(el, true);
            foreach (float w in new[] { -0.5f, 0f, 0.37f })
            {
                SSet(TMain, "windSpeedCurrent", w);
                CallI(wind, "UpdateSliderAndShowMultiplierMouseOver", el);
                Frames(1);
            }
            Record("creative_wind", 1);
            object search = New("Terraria.GameContent.Creative.ItemFilters+BySearch");
            CallI(search, "SetSearch", "sword");
            foreach (int t in new[] { 1, 4, 9, 46, 121, 757, 3063 })
                P("bysearch_" + t, CallI(search, "FitsFilter", NewItemOf(t)));
            SSet(TMain, "windSpeedCurrent", 0f);
        }
        // ---- the constant world's darkness: its damage in the dark (DontStarveDarknessDamageDealer) and its night sky
        //      colour (DontStarveSeed.ModifyNightColor)
        if (Group("dontstarve"))
        {
            SSet(TMain, "dontStarveWorld", true);
            SetTime(false, 9000.0);
            PlacePlayer(1999, 547);
            ISet(pl, "immune", false);
            ISet(pl, "immuneTime", 0);
            Frames(300);
            Record("dontstarve_dark", 1);
            PlacePlayer(sx, sy);
            Frames(10);
            Record("dontstarve_night", 1);
            SSet(TMain, "dontStarveWorld", false);
            Immune();
            ISet(pl, "statLife", 100);
            SetTime(true, 27000.0);
            Frames(5);
        }
        // ---- doors that open by themselves (DoorOpeningHelper, PreferenceSettings EnabledForEverything): a door and a
        //      tall gate in the way of the player walking right, and walking back
        if (Group("doors"))
        {
            Type doh = T("Terraria.GameContent.DoorOpeningHelper");
            FieldInfo pref = doh.GetField("PreferenceSettings", AllStatic);
            object old = pref.GetValue(null);
            pref.SetValue(null, Enum.ToObject(pref.FieldType, 2));
            int d1 = x0 + 10, d2 = x0 + 20;
            for (int x = x0; x <= x0 + 30; x++) PlaceTile(x, gy - 6, 30, 0);
            PlaceTile(d1, gy - 4, 30, 0);
            PlaceTile(d1, gy - 5, 30, 0);
            Note("door: " + PlaceTile(d1, gy - 1, 10, 0) + ", gate: " + PlaceTile(d2, gy - 1, 388, 0) + " " + CoverDiag(d1 - 1, gy - 6, 3, 6) + " / " + CoverDiag(d2 - 1, gy - 6, 3, 6));
            PlacePlayer(x0 + 4, gy - 1);
            Frames(2);
            InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.D };
            for (int k = 0; k < 8; k++) { Frames(10); P("doors_r_" + k, IF(pl, "position"), TileHash(d1, gy - 5, 2, 5), TileHash(d2, gy - 6, 2, 6)); }
            Record("doors_right", 1);
            InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.A };
            for (int k = 0; k < 8; k++) { Frames(10); P("doors_l_" + k, IF(pl, "position"), TileHash(d1, gy - 5, 2, 5), TileHash(d2, gy - 6, 2, 6)); }
            InKeys = null;
            Frames(30);
            Record("doors_left", 1);
            pref.SetValue(null, old);
            ClearArea(x0, x0 + 30, gy);
            Frames(5);
        }
        // ---- lava clouds (tile 717: TileDrawing.LavaLightA), living tree leaves in the wind (tile 192:
        //      EmitLivingTreeLeaf_Below / _Sideways), furniture flames of every style (TileDrawing.GetTileFlameData)
        if (Group("tiledraw"))
        {
            PlacePlayer(x0 + 20, gy - 1);
            for (int x = x0 + 4; x < x0 + 14; x++) PlaceTile(x, gy - 8, 717, 0);
            for (int x = x0 + 18; x < x0 + 30; x++) PlaceTile(x, gy - 8, 192, 0);
            SSet(TMain, "windSpeedCurrent", 0.8f);
            SSet(TMain, "windSpeedTarget", 0.8f);
            Frames(120);
            Record("tiledraw_wind_e", 1);
            SSet(TMain, "windSpeedCurrent", -0.8f);
            SSet(TMain, "windSpeedTarget", -0.8f);
            Frames(120);
            Record("tiledraw_wind_w", 1);
            SSet(TMain, "windSpeedCurrent", 0f);
            SSet(TMain, "windSpeedTarget", 0f);
            ClearArea(x0, x0 + 34, gy);
            // furniture flames: floor pieces on the ground, hanging ones under a ceiling, twelve styles at a time
            SetTime(false, 3600.0);
            foreach (int[] f in new[] { new[] { 33, 0 }, new[] { 174, 0 }, new[] { 100, 0 }, new[] { 173, 0 }, new[] { 93, 0 }, new[] { 34, 1 }, new[] { 42, 1 } })
            {
                for (int style = 0; style < 72; style += 12)
                {
                    for (int x = x0; x <= x0 + 40; x++) PlaceTile(x, gy - 7, 30, 0);
                    // candles stand on a table (StyleOnTable1x1): a row of platforms
                    bool onTable = f[0] == 33 || f[0] == 174;
                    if (onTable) for (int x = x0; x <= x0 + 40; x++) PlaceTile(x, gy - 1, 19, 0);
                    var placed = new List<string>();
                    for (int k = 0; k < 12; k++)
                    {
                        int x = x0 + 2 + k * 3;
                        // (a style the tile does not have: GetTileData is null; PlaceTile with such a style recurses forever)
                        if (MI(T("Terraria.ObjectData.TileObjectData"), "GetTileData", 3, "Int32,Int32,Int32").Invoke(null, new object[] { f[0], style + k, 0 }) == null) continue;
                        bool ok = (bool)TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { x, f[1] == 1 ? gy - 6 : (onTable ? gy - 2 : gy - 1), f[0], true, style + k, 0, -1, 1 });
                        if (ok) placed.Add((style + k).ToString());
                    }
                    Frames(3);
                    Note("flames " + f[0] + " styles " + string.Join(",", placed.ToArray()));
                    Record("flames_" + f[0] + "_" + style, 1);
                    ClearArea(x0, x0 + 40, gy);
                }
            }
            SetTime(true, 27000.0);
            Frames(5);
        }
        // ---- town slimes transforming (ParticleOrchestrator.Spawn_TownSlimeTransform: the copper and elder slimes)
        if (Group("slimefx"))
        {
            var pp = (Vector2)IF(pl, "position");
            ClearFx();
            for (int u = 0; u < 3; u++) SpawnParticle(26, pp.X + 40f * u, pp.Y - 30f, 1f, -2f, u);
            PFx("townslime");
            Frames(3);
            Record("townslime", 1);
        }
        // ---- the world item cap (400 items): stackable items merged to make room (EmergencyStacking)
        if (Group("itemcap"))
        {
            ClearItems();
            PlacePlayer(sx, sy);
            var pp = (Vector2)IF(pl, "position");
            MethodInfo newItem = T("Terraria.Item").GetMethods(AllStatic).First(m => m.Name == "NewItem" && m.GetParameters().Length == 12);
            object kind = Enum.ToObject(newItem.GetParameters()[9].ParameterType, 0);
            for (int k = 0; k < 410; k++)
                newItem.Invoke(null, new object[] { Src(), (int)pp.X - 300 + (k % 40) * 15, (int)pp.Y - 200 + (k / 40) * 10, 16, 16, k % 2 == 0 ? 2 : 3, 1, false, 0, kind, null, null });
            int active = 0;
            foreach (object wi in (Array)SF(TMain, "item")) if ((bool)IF(wi, "active")) active++;
            P("itemcap_active", active);
            Frames(10);
            Record("itemcap", 1);
            ClearItems();
            Frames(2);
        }
        // ---- small direct calls: the pirates' guess of what the player does (PlayerIntentionGuesser.PrepareUsageProxy),
        //      the nurse achievement counter, two world generation micro-biomes (SpikePitBiome / CorruptionPitBiome.Place),
        //      the credits roll of the main menu (CreditsRollSky.Draw with Main.gameMenu)
        if (Group("direct1"))
        {
            object ig = IF(pl, "IntentionGuesser");
            foreach (int[] e in new[] { new[] { 10, 15, 5 }, new[] { 1, 15, 15 }, new[] { 3509, 4, 4 } })
            {
                CallI(ig, "PrepareUsageProxy", pl, e[0], e[1], e[2]);
                object up = IF(ig, "UsageProxy");
                P("usageproxy_" + e[0], IF(up, "screenTargetX"), IF(up, "screenTargetY"), IF(up, "reachableStartX"), IF(up, "reachableStartY"), IF(up, "reachableEndX"), IF(up, "reachableEndY"));
            }
            CallS("Terraria.GameContent.Achievements.AchievementsHelper", "HandleNurseService", 1234);
            // micro-biomes, right of the probe area
            int bx = x0 + 120;
            foreach (string b in new[] { "SpikePitBiome", "CorruptionPitBiome" })
            {
                foreach (int oy in new[] { gy - 30, 400, 650 })
                {
                    object biome = New("Terraria.GameContent.Biomes." + b);
                    object sm = New("Terraria.WorldBuilding.StructureMap");
                    object gp = New("Terraria.WorldBuilding.GenerationProgress");
                    PTiles("biome_" + b + "_" + oy + "_before", bx - 30, oy - 10, 60, 120);
                    object ok = CallI(biome, "Place", new Point(bx, oy), sm, gp);
                    P("biome_" + b + "_" + oy, ok);
                    PTiles("biome_" + b + "_" + oy + "_after", bx - 30, oy - 10, 60, 120);
                }
                bx += 80;
            }
            // the main menu's credits segments
            object skyMgr = SF(T("Terraria.Graphics.Effects.SkyManager"), "Instance");
            object sky = skyMgr.GetType().GetProperty("Item", new[] { typeof(string) }).GetValue(skyMgr, new object[] { "CreditsRoll" });
            int end = (int)IF(sky, "_endTime");
            SSet(TMain, "gameMenu", true);
            for (int t = 0, k = 0; t < end; t += 600, k++)
            {
                ISet(sky, "_currentTime", t);
                if (k % 8 == 0) DirectDraw("menucredits_" + t, () => InBatch(() => CallI(sky, "Draw", SF(TMain, "spriteBatch"), 0f, 2f)));
                else InBatch(() => CallI(sky, "Draw", SF(TMain, "spriteBatch"), 0f, 2f));
            }
            SSet(TMain, "gameMenu", false);
            ISet(sky, "_currentTime", 0);
            Frames(3);
        }
    }
}
