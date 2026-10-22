// The interaction run (`RefDumpDraw.exe <dir> interact`, refdata\interact; port: tests/draw.rs DRAW_MODE=interact,
// tests/draw/interact.rs, the same steps): what the gallery cannot reach by spawning things, because it needs the local
// player to USE something or the world to be built for it: every held / channelled weapon kind (whips, kites, yo-yos,
// flails, spears, drills and chainsaws, held projectiles, fishing poles) used for a while, a wired contraption with a
// switch, chest operations (quick stack, deposit all, loot all, restock), the Old One's Army at difficulty 1 - 3,
// shimmer (an item, a projectile and the player in it), the travelling merchant and a homeless town NPC arriving, the
// player sitting on a chair, dodges (black belt, mystic sash), a grappling hook, rocket boots, armor sets, the ghost,
// day / night turning over, a buff tooltip, the classic resource display, the recipe grid, the item-pickup popup
// text and furniture placement previews. Scenes 1 - 2 frames, every frame's digest, state lines as the scenes run.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;

public static partial class WG
{
    static bool InteractMode;

    /// <summary>Projectile aiStyles of held / channelled weapons: flails 15, spears 19, drills 20, held projectiles
    /// 75, yo-yos 99, 140 / 142 / 161 / 182 / 190 / 191 (newer melee kinds), whips 165, kites 160.</summary>
    static readonly int[] HeldAiStyles = { 15, 19, 20, 75, 99, 140, 142, 160, 161, 165, 182, 190, 191 };

    /// <summary>The items of the use gallery: a use style and a projectile of a held kind (HeldAiStyles), or a whip,
    /// kite or yo-yo by the game's sets, or a fishing pole; in item type order (port: interact_items, the same rule).</summary>
    static List<int> InteractItems()
    {
        var list = new List<int>();
        Type itemT = T("Terraria.Item"), projT = T("Terraria.Projectile");
        object it = Activator.CreateInstance(itemT);
        object pr = Activator.CreateInstance(projT);
        MethodInfo isd = itemT.GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int));
        MethodInfo psd = projT.GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 1);
        Type sets = T("Terraria.ID.ItemID+Sets");
        var whip = (bool[])SF(sets, "SummonerWeaponThatScalesWithAttackSpeed");
        var kite = (bool[])SF(sets, "IsAKite");
        var yoyo = (bool[])SF(sets, "Yoyo");
        int count = (short)SF(T("Terraria.ID.ItemID"), "Count");
        for (int t = 1; t < count; t++)
        {
            isd.Invoke(it, new object[] { t, null });
            if ((int)IF(it, "useStyle") == 0) continue;
            bool take = whip[t] || kite[t] || yoyo[t] || (int)IF(it, "fishingPole") > 0;
            int shoot = (int)IF(it, "shoot");
            if (!take && shoot > 0 && shoot < ProjCount)
            {
                psd.Invoke(pr, new object[] { shoot });
                take = Array.IndexOf(HeldAiStyles, (int)IF(pr, "aiStyle")) >= 0;
            }
            if (take) list.Add(t);
        }
        return list;
    }

    static void SetInv(int slot, int type, int stack)
    {
        GiveItem(slot, type);
        object item = ((Array)IF(LocalPlayer, "inventory")).GetValue(slot);
        ISet(item, "stack", stack);
    }
    static void SetArmor(int slot, int type)
    {
        object item = ((Array)IF(LocalPlayer, "armor")).GetValue(slot);
        item.GetType().GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int)).Invoke(item, new object[] { type, null });
    }
    static bool PlaceTile(int x, int y, int type, int style)
    {
        return (bool)CallStatic(TWorldGen, "PlaceTile", x, y, type, true, true, -1, style);
    }
    /// <summary>The first solid tile at or below row y0 in column x (the snapshot of the moment).</summary>
    static int GroundBelow(int x, int y0)
    {
        Snap s = TakeSnapshot(null);
        for (int y = y0; y < s.H - 10; y++) if ((s.Buf[(x * s.H + y) * RecSize + 5] & 0x20) != 0) return y;
        return -1;
    }

    static void InteractWorld()
    {
        int sx = (int)SF(TMain, "spawnTileX"), sy = (int)SF(TMain, "spawnTileY");
        object pl = LocalPlayer;
        SetTime(true, 27000.0);
        PlacePlayer(sx, sy);
        ISet(pl, "statManaMax", 200);
        ISet(pl, "statMana", 200);
        // ammo and bait: wooden arrows, musket balls, master bait (ammo slots 54 - 57)
        SetInv(54, 40, 999);
        SetInv(55, 97, 999);
        SetInv(56, 2676, 99);
        Frames(30);
        Press(Microsoft.Xna.Framework.Input.Keys.D1);
        // ---- 1. the use gallery
        List<int> items = InteractItems();
        Note("use gallery: " + items.Count + " items: " + string.Join(",", items.Select(i => i.ToString()).ToArray()));
        foreach (int t in items)
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Immune();
            SetInv(0, t, 1);
            InMouseX = 900; InMouseY = 300;
            InLeft = true;
            Frames(20);
            Record("use_" + t, 1);
            InLeft = false;
            Frames(4);
        }
        InMouseX = 640; InMouseY = 360;
        ClearEntities();
        PlacePlayer(sx, sy);
        Frames(20);
        InteractBuilt(sx, sy);
        InteractPlayer(sx, sy);
    }

    static void KillTile(int x, int y) { CallStatic(TWorldGen, "KillTile", x, y, false, false, true); }

    /// <summary>The flattened ground right of the spawn (columns X0 - 2 .. X0 + 70, its top row GY), then what is built on it:
    /// a wired contraption, the Old One's Army's crystal stand, a shimmer pool, a chair; chest operations; town NPCs.</summary>
    const int BuildDX = 8, BuildW = 70;
    static void InteractBuilt(int sx, int sy)
    {
        object pl = LocalPlayer;
        int x0 = sx + BuildDX;
        int gy = GroundBelow(x0, sy - 12);
        Note("build ground at " + x0 + "," + gy);
        for (int x = x0 - 2; x <= x0 + BuildW; x++)
        {
            for (int y = gy - 10; y < gy; y++) KillTile(x, y);
            for (int y = gy; y <= gy + 1; y++) PlaceTile(x, y, 0, 0);
        }
        Frames(5);
        // ---- 2. a wired contraption: a switch and devices on one wire (the row above the ground), switched on and off
        int[][] devices =
        {
            new[] { 136, 0, 0 }, new[] { 4, 0, 2 }, new[] { 93, 0, 4 }, new[] { 137, 0, 6 }, new[] { 105, 2, 8 }, new[] { 144, 0, 11 },
            new[] { 207, 0, 13 }, new[] { 216, 0, 16 }, new[] { 335, 0, 18 }, new[] { 338, 0, 21 }, new[] { 405, 0, 23 }, new[] { 244, 0, 27 },
            new[] { 455, 0, 31 }, new[] { 565, 0, 35 }, new[] { 130, 0, 38 },
        };
        foreach (int[] d in devices) Note("place " + d[0] + " at +" + d[2] + ": " + PlaceTile(x0 + d[2], gy - 1, d[0], d[1]));
        for (int x = x0; x <= x0 + 39; x++) CallStatic(TWorldGen, "PlaceWire", x, gy - 1);
        PlacePlayer(x0 - 1, gy - 1);
        Frames(10);
        Type wiring = T("Terraria.Wiring");
        CallStatic(wiring, "HitSwitch", x0, gy - 1);
        Frames(30);
        Record("wired_on", 2);
        CallStatic(wiring, "HitSwitch", x0, gy - 1);
        Frames(30);
        Record("wired_off", 2);
        ClearEntities();
        // ---- 3. chest operations: the first chest of the world near the surface, items of its kinds in the inventory
        {
            Array chests = (Array)SF(TMain, "chest");
            int ci = -1;
            for (int i = 0; i < chests.Length && ci < 0; i++) { object c = chests.GetValue(i); if (c != null && (int)IF(c, "y") < 300) ci = i; }
            object ch = chests.GetValue(ci);
            Array citems = (Array)IF(ch, "item");
            int k = 0;
            for (int i = 0; i < citems.Length && k < 5; i++)
            {
                object it = citems.GetValue(i);
                if (it == null || (int)IF(it, "type") == 0) continue;
                SetInv(10 + k, (int)IF(it, "type"), 1);
                k++;
            }
            SetInv(20, 73, 7);
            PlacePlayer((int)IF(ch, "x"), (int)IF(ch, "y") + 2);
            Frames(10);
            // Player.OpenChest (what a right click on the chest does; it also sets chestX / chestY, which the range check
            // of Player.Update reads)
            pl.GetType().GetMethod("OpenChest", AllInst).Invoke(pl, new object[] { (int)IF(ch, "x"), (int)IF(ch, "y"), ci });
            Frames(5);
            Type cui = T("Terraria.UI.ChestUI");
            CallStatic(cui, "QuickStack", false);
            Frames(5);
            Record("chest_quickstack", 1);
            CallStatic(cui, "LootAll");
            Frames(5);
            Record("chest_lootall", 1);
            CallStatic(cui, "DepositAll");
            Frames(5);
            Record("chest_depositall", 1);
            CallStatic(cui, "Restock");
            Frames(5);
            Record("chest_restock", 1);
            ISet(pl, "chest", -1);
            SSet(TMain, "playerInventory", false);
            Frames(5);
        }
        // ---- 4. the Old One's Army at difficulty 1, 2, 3: the crystal stand on the ground, the crystal summoned
        Type dd2 = T("Terraria.GameContent.Events.DD2Event");
        int cx = x0 + 50;
        for (int d = 1; d <= 3; d++)
        {
            ClearEntities();
            PlacePlayer(cx - 6, gy - 1);
            Immune();
            // DD2Event.SummonCrystalDirect without its crystal stand tile (placing tile 466 by WorldGen.PlaceTile recurses
            // until the stack overflows in the original): StartInvasion with the difficulty, the crystal NPC (548) on
            // the ground, DropStarterCrystals
            CallStatic(dd2, "StartInvasion", d);
            NewNpc(cx * 16 + 8, gy * 16, 548);
            CallStatic(dd2, "DropStarterCrystals");
            Frames(420);
            Note("dd2 difficulty " + d + ": ongoing " + SF(dd2, "Ongoing"));
            Record("dd2_" + d, 1);
            CallStatic(dd2, "StopInvasion", false);
            Frames(10);
        }
        ClearEntities();
        // ---- 5. shimmer: a pool of 6 x 2 tiles, an item, an arrow and the player in it
        {
            int px = x0 + 58;
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
            foreach (int type in new[] { 9, 3, 29, 75 })
                newItem.Invoke(null, new object[] { Activator.CreateInstance(T("Terraria.DataStructures.EntitySource_DebugCommand")), (px + 2) * 16, gy * 16, 16, 16, type, 3, false, 0, Enum.ToObject(newItem.GetParameters()[9].ParameterType, 0), null, null });
            NewProj((px + 1) * 16, (gy - 3) * 16, 0f, 3f, 1);
            PlacePlayer(px - 3, gy - 1);
            Frames(60);
            Record("shimmer_items", 1);
            PlacePlayer(px + 2, gy);
            Frames(90);
            Record("shimmer_player", 1);
            PlacePlayer(x0, gy - 1);
            Frames(60);
        }
        // ---- 6. a chair, the player sitting on it
        {
            int chx = x0 + 44;
            Note("chair: " + PlaceTile(chx, gy - 1, 15, 0));
            PlacePlayer(chx + 1, gy - 1);
            Frames(5);
            object sit = IF(pl, "sitting");
            sit.GetType().GetMethod("SitDown").Invoke(sit, new object[] { pl, chx, gy - 1 });
            ISet(pl, "sitting", sit);
            Frames(20);
            Record("player_sit", 1);
            sit = IF(pl, "sitting");
            sit.GetType().GetMethod("SitUp").Invoke(sit, new object[] { pl, true });
            ISet(pl, "sitting", sit);
            Frames(10);
        }
        // ---- 7. town NPCs arriving: the guide given a home (the travelling merchant needs a housed town NPC), the
        //         travelling merchant, then a homeless merchant (WorldGen.prioritizedTownNPCType)
        {
            Array npcs = (Array)SF(TMain, "npc");
            for (int i = 0; i < npcs.Length; i++)
            {
                object n = npcs.GetValue(i);
                if ((bool)IF(n, "active") && (int)IF(n, "type") == 22) { ISet(n, "homeless", false); ISet(n, "homeTileX", sx); ISet(n, "homeTileY", sy); break; }
            }
            SetTime(true, 27000.0);
            CallStatic(TWorldGen, "SpawnTravelNPC");
            Frames(30);
            Record("npc_travel", 1);
            SSet(TWorldGen, "prioritizedTownNPCType", 17);
            CallStatic(TWorldGen, "SpawnHomelessNPC");
            Frames(30);
            Record("npc_homeless", 1);
        }
        PlacePlayer(sx, sy);
        Frames(20);
    }

    /// <summary>The local player's states: dodges (a black belt: Player.Hurt until it dodges), a grappling hook, rocket
    /// boots, two armor sets, the ghost, dusk and dawn, a buff tooltip, every resource display, an item picked up
    /// (its popup text), a placement preview, fishing at the beach.</summary>
    static void InteractPlayer(int sx, int sy)
    {
        object pl = LocalPlayer;
        Type pdr = T("Terraria.DataStructures.PlayerDeathReason");
        MethodInfo hurt = pl.GetType().GetMethod("Hurt", new[] { pdr, typeof(int), typeof(int), typeof(bool), typeof(bool), typeof(bool), typeof(int), typeof(bool) });
        SetArmor(3, 3016);
        Frames(5);
        for (int i = 0; i < 30; i++)
        {
            ISet(pl, "immune", false);
            ISet(pl, "immuneTime", 0);
            ISet(pl, "statLife", 100);
            hurt.Invoke(pl, new object[] { pdr.GetMethod("LegacyDefault", AllStatic).Invoke(null, null), 1, 1, false, false, false, -1, true });
            Frames(2);
        }
        Record("player_dodge", 1);
        SetArmor(3, 0);
        // a grappling hook (miscEquips[4]) thrown up and right
        {
            object hook = ((Array)IF(pl, "miscEquips")).GetValue(4);
            hook.GetType().GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int)).Invoke(hook, new object[] { 84, null });
            InMouseX = 800; InMouseY = 200;
            Frames(2);
            pl.GetType().GetMethod("QuickGrapple").Invoke(pl, null);
            Frames(30);
            Record("player_grapple", 1);
            InMouseX = 640; InMouseY = 360;
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(20);
        }
        // rocket boots, flying
        SetArmor(4, 128);
        InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.Space };
        Frames(60);
        Record("player_rocketboots", 1);
        InKeys = null;
        Frames(60);
        SetArmor(4, 0);
        PlacePlayer(sx, sy);
        // armor sets: molten, solar flare
        foreach (int[] set in new[] { new[] { 231, 232, 233 }, new[] { 2763, 2764, 2765 } })
        {
            for (int k = 0; k < 3; k++) SetArmor(k, set[k]);
            Frames(20);
            Record("player_armor_" + set[0], 1);
        }
        for (int k = 0; k < 3; k++) SetArmor(k, 0);
        // the ghost
        ISet(pl, "ghost", true);
        Frames(30);
        Record("player_ghost", 1);
        ISet(pl, "ghost", false);
        PlacePlayer(sx, sy);
        Frames(10);
        // dusk and dawn
        SetTime(true, 54000.0 - 60.0);
        Frames(90);
        Record("time_dusk", 1);
        SetTime(false, 32400.0 - 60.0);
        Frames(90);
        Record("time_dawn", 1);
        SetTime(true, 27000.0);
        // a buff and its tooltip (the buff bar top left)
        pl.GetType().GetMethod("AddBuff").Invoke(pl, new object[] { 1, 3600, false });
        pl.GetType().GetMethod("AddBuff").Invoke(pl, new object[] { 2, 3600, false });
        InMouseX = 40; InMouseY = 90;
        Frames(10);
        Record("ui_bufftip", 1);
        InMouseX = 640; InMouseY = 360;
        Frames(5);
        // every resource display (PlayerResourceSetsManager.CycleResourceSet, back to the first)
        {
            object mgr = SF(TMain, "ResourceSetsManager");
            for (int k = 0; k < 6; k++)
            {
                mgr.GetType().GetMethod("CycleResourceSet").Invoke(mgr, null);
                Frames(5);
                Record("ui_resources_" + k, 1);
            }
        }
        // an item picked up: its popup text
        {
            MethodInfo newItem = T("Terraria.Item").GetMethods(AllStatic).First(m => m.Name == "NewItem" && m.GetParameters().Length == 12);
            var p = (Microsoft.Xna.Framework.Vector2)IF(pl, "position");
            newItem.Invoke(null, new object[] { Activator.CreateInstance(T("Terraria.DataStructures.EntitySource_DebugCommand")), (int)p.X, (int)p.Y, 16, 16, 73, 5, false, 0, Enum.ToObject(newItem.GetParameters()[9].ParameterType, 0), null, null });
            Frames(20);
            Record("ui_pickup", 1);
        }
        // a placement preview: a work bench held, the cursor on the ground two tiles right
        SetInv(0, 36, 5);
        Frames(2);
        Aim((FeetTileX + 2) * 16 + 8, FeetTileY * 16 - 8);
        Frames(5);
        Record("ui_placepreview", 1);
        InMouseX = 640; InMouseY = 360;
        // fishing at the beach of the left ocean: the first dry column right of the sea, the line cast left into the water
        {
            Snap s = TakeSnapshot(null);
            Func<int, int, int> at = (x, y) => (x * s.H + y) * RecSize;
            int bx = -1, by = -1;
            bool water = false;
            for (int x = 60; x < 700 && bx < 0; x++)
            {
                int y = 50;
                while (y < s.H - 50 && (s.Buf[at(x, y) + 5] & 0x20) == 0 && s.Buf[at(x, y) + 4] == 0) y++;
                bool wet = s.Buf[at(x, y) + 4] != 0;
                if (wet) water = true;
                else if (water) { bx = x; by = y; }
            }
            Note("beach at " + bx + "," + by);
            SetInv(0, 2289, 1);
            PlacePlayer(bx + 1, by - 1);
            Frames(10);
            Aim((bx - 8) * 16, (by - 1) * 16);
            Click();
            Frames(400);
            Record("fishing_wait", 1);
            Click();
            Frames(30);
            Record("fishing_reel", 1);
            InMouseX = 640; InMouseY = 360;
            ClearEntities();
        }
        PlacePlayer(sx, sy);
        Frames(20);
        InteractPlayer2(sx, sy);
    }

    /// <summary>More of the local player (the float methods the first interaction scenes left without a reference):
    /// three developer wings flown (their dust), the black belt's dodge (item 963; Player.Hurt until it dodges),
    /// Yoraiz0r's eye (item 3580 as an accessory), rocket boots used after a jump, the quick actions (heal, mana,
    /// buffs, mount: Player.QuickHeal / QuickMana / QuickBuff / QuickMount), the angler's reward
    /// (Player.GetAnglerReward), the UFO and drill mounts' abilities (the button held).</summary>
    static void InteractPlayer2(int sx, int sy)
    {
        object pl = LocalPlayer;
        Type pdr = T("Terraria.DataStructures.PlayerDeathReason");
        MethodInfo hurt = pl.GetType().GetMethod("Hurt", new[] { pdr, typeof(int), typeof(int), typeof(bool), typeof(bool), typeof(bool), typeof(int), typeof(bool) });
        foreach (int w in new[] { 5586, 5659, 5686 })
        {
            PlacePlayer(sx, sy);
            SetArmor(3, w);
            Frames(5);
            InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.Space };
            Frames(40);
            Record("player_wings_" + w, 1);
            InKeys = null;
            Frames(60);
        }
        SetArmor(3, 963);
        PlacePlayer(sx, sy);
        Frames(5);
        for (int i = 0; i < 40; i++)
        {
            ISet(pl, "immune", false);
            ISet(pl, "immuneTime", 0);
            ISet(pl, "statLife", 100);
            hurt.Invoke(pl, new object[] { pdr.GetMethod("LegacyDefault", AllStatic).Invoke(null, null), 1, 1, false, false, false, -1, true });
            Frames(2);
        }
        Record("player_blackbelt", 1);
        SetArmor(3, 3580);
        Frames(20);
        Record("player_yoraiz0r", 1);
        SetArmor(3, 0);
        // rocket boots: a jump, the key released, then held (the boots fire in the air)
        SetArmor(4, 128);
        PlacePlayer(sx, sy);
        Frames(5);
        InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.Space };
        Frames(8);
        InKeys = null;
        Frames(4);
        InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.Space };
        Frames(50);
        Record("player_rockets", 1);
        InKeys = null;
        Frames(90);
        SetArmor(4, 0);
        PlacePlayer(sx, sy);
        // the quick actions
        SetInv(11, 28, 5);
        SetInv(12, 110, 5);
        SetInv(13, 292, 3);
        SetInv(14, 290, 3);
        ISet(pl, "statLife", 40);
        ISet(pl, "statMana", 10);
        Frames(2);
        pl.GetType().GetMethod("QuickHeal").Invoke(pl, null);
        pl.GetType().GetMethod("QuickMana").Invoke(pl, null);
        pl.GetType().GetMethod("QuickBuff").Invoke(pl, null);
        Frames(20);
        Record("player_quick", 1);
        {
            object saddle = ((Array)IF(pl, "miscEquips")).GetValue(3);
            saddle.GetType().GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int)).Invoke(saddle, new object[] { 2430, null });
            pl.GetType().GetMethod("QuickMount").Invoke(pl, null);
            Frames(30);
            Record("player_quickmount", 1);
            pl.GetType().GetMethod("QuickMount").Invoke(pl, null);
            Frames(10);
        }
        // the angler's reward for a quest fish (an angler next to the player)
        {
            var p = (Microsoft.Xna.Framework.Vector2)IF(pl, "position");
            int a = NewNpc((int)p.X + 40, (int)p.Y + 40, 369);
            object angler = ((Array)SF(TMain, "npc")).GetValue(a);
            pl.GetType().GetMethod("GetAnglerReward").Invoke(pl, new object[] { angler, 2450 });
            Frames(30);
            Record("player_angler", 1);
            ClearEntities();
        }
        // the UFO and drill mounts, the button held at the cursor
        foreach (int mt in new[] { 7, 8 })
        {
            PlacePlayer(sx, sy);
            object mount = IF(pl, "mount");
            mount.GetType().GetMethod("SetMount").Invoke(mount, new object[] { mt, pl });
            Frames(10);
            InMouseX = 800; InMouseY = 420;
            InLeft = true;
            Frames(40);
            Record("player_mountability_" + mt, 1);
            InLeft = false;
            InMouseX = 640; InMouseY = 360;
            mount.GetType().GetMethods().First(m => m.Name == "Dismount").Invoke(mount, new object[] { pl, false });
            Frames(10);
        }
        ClearEntities();
        PlacePlayer(sx, sy);
        Frames(20);
    }
}
