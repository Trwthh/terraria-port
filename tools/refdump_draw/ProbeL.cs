// The probe run, part L (Probe.cs; port: tests/draw/probe_l.rs, the same steps): Mount / Minecart, Main, ItemSlot,
// ChestUI, ItemSorting and smaller helpers the earlier runs' play did not reach, called directly with the state that
// makes them do their work.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    static object MCall(string name, params object[] args) { return Unwrap(MI(TMain, name, args.Length), null, args); }
    static object MICall(string name, params object[] args) { return Unwrap(MI(TMain, name, args.Length), SF(TMain, "instance"), args); }
    static object SlotCall(string name, params object[] args) { return Unwrap(MI(T("Terraria.UI.ItemSlot"), name, args.Length), null, args); }
    static object SlotCallSig(string name, string sig, params object[] args) { return Unwrap(MI(T("Terraria.UI.ItemSlot"), name, args.Length, sig), null, args); }
    static Array PInv() { return (Array)IF(LocalPlayer, "inventory"); }
    static object InvAt(int k) { return PInv().GetValue(k); }
    static void InvSet(int k, int type, int stack)
    {
        object it = InvAt(k);
        CallI(it, "SetDefaults", type, null);
        if (type != 0) ISet(it, "stack", stack);
    }
    static int FindNpc(int type)
    {
        Array npcs = (Array)SF(TMain, "npc");
        for (int i = 0; i < npcs.Length; i++) { object n = npcs.GetValue(i); if ((bool)IF(n, "active") && (int)IF(n, "type") == type) return i; }
        return -1;
    }

    static void ProbeL(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        int me = (int)SF(TMain, "myPlayer");
        Type tMinecart = T("Terraria.Minecart"), tMount = T("Terraria.Mount");
        // ---- mounts: the minecart's magnet, track, switch, rotation and sparks, the mech cart's laser and mech point,
        //      the meowcart's sounds, the failed dismount dust, the scutlix's ability charge, the drill's wall cursor
        if (Group("mountdirect"))
        {
            ClearEntities();
            ClearFx();
            for (int x = x0 + 4; x <= x0 + 30; x++) PlaceTile(x, gy - 1, 314, 0);
            PlacePlayer(x0 + 8, gy - 2);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            object ms = IF(pl, "MinecartSettings");
            P("magnet", Unwrap(MI(tMinecart, "GetMagnetPosition", 4), null, new object[] { pp, 20, 42, ms }));
            object[] ot = { x0 + 10, gy - 1, pp, 20, 42, ms };
            P("ontrack", Unwrap(MI(tMinecart, "GetOnTrack", 6), null, ot), ot[2]);
            Unwrap(MI(tMinecart, "HitTrackSwitch", 4), null, new object[] { pp, 20, 42, ms });
            object mount = IF(pl, "mount");
            CallI(mount, "SetMount", 11, pl);
            Frames(2);
            pp = (Vector2)IF(pl, "position");
            object dlg = IF(mount, "Delegations");
            object[] tr = { pl, 0f, pp, 20, 42, false, false, dlg };
            P("trackrot", Unwrap(MI(tMinecart, "TrackRotation", 8), null, tr), tr[1]);
            Unwrap(MI(tMinecart, "WheelSparks", 6), null, new object[] { IF(dlg, "MinecartDust"), pp, 20, 42, 3, ms });
            P("mechpoint", Unwrap(MI(tMount, "GetMinecartMechPoint", 3), null, new object[] { pl, 20, -19 }));
            Unwrap(MI(tMount, "MeowcartBumperSound", 4), null, new object[] { pl, pp, 20, 42 });
            Unwrap(MI(tMount, "MeowcartLandingSound", 4), null, new object[] { pl, pp, 20, 42 });
            NpcNear(3, 60f, 0f);
            ISet(pl, "velocity", new Vector2(4f, 0f));
            CallI(mount, "CastSuperCartLaser", pl);
            ISet(pl, "velocity", Vector2.Zero);
            CallI(mount, "DoFailedDismountDust", pl, 20);
            CallI(mount, "Dismount", pl, false);
            CallI(mount, "SetMount", 9, pl);
            Frames(1);
            InMouseX = 800; InMouseY = 300;
            Frames(1);
            CallI(mount, "StartAbilityCharge", pl);
            P("abilitycharge", IF(mount, "AbilityCharge"));
            CallI(mount, "Dismount", pl, false);
            CallI(mount, "SetMount", 8, pl);
            Frames(1);
            P("drillwalls", CallI(mount, "DrillSmartCursor_Walls", pl, IF(mount, "_mountSpecificData")));
            CallI(mount, "Dismount", pl, false);
            InMouseX = 640; InMouseY = 360;
            PFx("mountdirect");
            StateLine("mountdirect");
            Frames(3);
            Record("mountdirect", 1);
            ClearEntities();
            ClearFx();
            ClearArea(x0, x0 + 40, gy);
            Frames(2);
        }
        // ---- Main helpers: PvP damage, the reverse gravity rectangle, a loaded invasion's start, the nurse, the tax
        //      collector, the angler, the dye trader, a reforge
        if (Group("maindirect"))
        {
            ClearEntities();
            ClearFx();
            PlacePlayer(sx, sy);
            Frames(2);
            P("pvpdamage", MCall("CalculateDamagePlayersTakeInPVP", 50, 10), MCall("CalculateDamagePlayersTakeInPVP", 5, 40));
            P("reversegravity", Unwrap(MI(TMain, "ReverseGravitySupport", 1, "Rectangle"), null, new object[] { new Rectangle(100, 200, 30, 40) }));
            SSet(TMain, "invasionType", 1);
            SSet(TMain, "invasionSize", 130);
            MCall("FakeLoadInvasionStart");
            P("invasionstart", SF(TMain, "invasionSizeStart"));
            SSet(TMain, "invasionType", 0);
            SSet(TMain, "invasionSize", 0);
            SSet(TMain, "invasionSizeStart", 0);
            InvSet(49, 73, 80);
            NpcNear(18, 60f, 0f);
            CallI(pl, "SetTalkNPC", FindNpc(18));
            ISet(pl, "statLife", 60);
            int cost = (int)MCall("GetNurseHealCost");
            object[] cc = { Color.White, cost };
            P("nurse", cost, MCall("GetCoinValueText_Nurse", cc), cc[0]);
            MCall("NPCChatText_DoNurseHeal", cost);
            P("nurseheal", IF(pl, "statLife"), SF(TMain, "npcChatText"));
            NpcNear(441, -60f, 0f);
            CallI(pl, "SetTalkNPC", FindNpc(441));
            ISet(pl, "taxMoney", 123456);
            object[] tc = { Color.White, 123456 };
            P("taxtext", MCall("GetCoinValueText_TaxCollector", tc), tc[0]);
            MCall("NPCChatText_DoTaxCollector");
            P("tax", IF(pl, "taxMoney"), SF(TMain, "npcChatText"));
            NpcNear(369, 90f, 0f);
            CallI(pl, "SetTalkNPC", FindNpc(369));
            int quest = (int)SF(TMain, "anglerQuest");
            InvSet(10, ((int[])SF(TMain, "anglerQuestItemNetIDs"))[quest], 1);
            MCall("NPCChatText_DoAnglerQuest");
            P("angler", SF(TMain, "anglerQuestFinished"), SF(TMain, "npcChatText"));
            SSet(TMain, "anglerQuestFinished", false);
            NpcNear(207, -90f, 0f);
            CallI(pl, "SetTalkNPC", FindNpc(207));
            bool[] exotic = (bool[])SF(T("Terraria.ID.ItemID+Sets"), "ExoticPlantsForDyeTrade");
            int plant = Array.IndexOf(exotic, true);
            InvSet(11, plant, 1);
            MCall("NPCChatText_DyeTraderRarePlant");
            P("dyetrader", plant, SF(TMain, "npcChatText"));
            CallI(pl, "SetTalkNPC", -1);
            SSet(TMain, "npcChatText", "");
            object reforge = NewItemOf(3507);
            SSet(TMain, "reforgeItem", reforge);
            MCall("ReforgeItemInReforgeSlot");
            P("reforge", IF(reforge, "prefix"));
            SSet(TMain, "reforgeItem", NewItemOf(0));
            SSet(TMain, "reforgeCooldown", 0);
            for (int k = 0; k < 58; k++) InvSet(k, 0, 0);
            ClearItems();
            ClearPlayerBuffs();
            ISet(pl, "statLife", 100);
            ISet(pl, "taxMoney", 0);
            PFx("maindirect");
            StateLine("maindirect");
            Frames(3);
            Record("maindirect", 1);
            ClearEntities();
            ClearFx();
            Frames(2);
        }
        // ---- Main draws called outside the frame: the guide's craft text, the death marker and NPC icons on the map,
        //      the worm health bar, the queen slime's wings, the paladin's shield boundary, the stardust guardian's
        //      punches, the good / evil desert background, the torch biome swap icon, the simple surface background, the
        //      wall of flesh's tongue
        if (Group("maindraw"))
        {
            ClearEntities();
            ClearFx();
            PlacePlayer(sx, sy);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            object z = NpcNear(3, 80f, 0f);
            object qs = NpcNear(657, -120f, -40f);
            int guard = NewProj(pp.X - 40f, pp.Y - 20f, 0f, 0f, 623);
            ISet(pl, "lastDeathPostion", new Vector2(pp.X + 100f, pp.Y));
            SSet(TMain, "guideItem", NewItemOf(9));
            var screen = (Vector2)SF(TMain, "screenPosition");
            object sb = SF(TMain, "spriteBatch");
            DirectDraw("maindraw", () => InBatch(() =>
            {
                MCall("DrawGuideCraftText", 100, 300);
                P("deathmarker", MCall("DrawPlayerDeathMarker", 300f, 200f, 2f, 0f, 0f, 1f, me, 255));
                MCall("DrawNPCMapIcons3", 300f, 200f, 2f, (byte)255, 1f);
                MICall("DrawInterface_Healthbar_Worm", z, 15, 1f);
                MICall("DrawNPCDirect_QueenSlimeWings", qs, sb, screen, (Vector2)IF(qs, "Center"), Color.White);
                MICall("DrawPaladinsShieldBoundary", pp, pp + new Vector2(200f, -60f));
                MICall("DrawProj_StardustGuardianPunching", ProjAt(guard));
                MICall("DrawSurfaceBG_GoodEvilDesert", 0f, 1f, 0, 0);
                P("torchswap", MICall("DrawTorchBiomeSwapIcon", 400, 300, false, 0));
                SSet(TMain, "wofNPCIndex", (int)IF(z, "whoAmI"));
                MCall("DrawWOFTongueToPlayer", me);
                SSet(TMain, "wofNPCIndex", -1);
            }));
            object tb = SF(TMain, "tileBatch");
            DirectDraw("maindraw_surface", () =>
            {
                CallI(tb, "Begin");
                MICall("DrawSimpleSurfaceBackground", pp - new Vector2(200f, 150f), 400, 300);
                CallI(tb, "End");
            });
            SSet(TMain, "guideItem", NewItemOf(0));
            StateLine("maindraw");
            ClearEntities();
            ClearFx();
            Frames(2);
        }
        // ---- ItemSlot: the trusty pal glow, armour and dye swaps, gamepad instructions, a shop slot bought from, a left
        //      click override (favourite), an item picked into the mouse, a vanity swap, a container opened, money drawn;
        //      ChestUI: deposit all, placing in a chest, scrolling; ItemSorting: a chest sorted, ammo filled
        if (Group("itemslot"))
        {
            ClearEntities();
            ClearFx();
            PlacePlayer(x0 + 2, gy - 1);
            Frames(2);
            P("trustypal", SlotCall("GetTrustyPalGlowColor"));
            object[] asw = { NewItemOf(90), false, 0 };
            object back = SlotCall("ArmorSwap", asw);
            P("armorswap", asw[1], asw[2], IF(back, "type"));
            object dye = NewItemOf(1007);
            object[] dsw = { dye, false, 0 };
            back = SlotCall("DyeSwap", dsw);
            P("dyeswap", dsw[1], dsw[2], IF(back, "type"));
            InvSet(0, 3507, 1);
            P("gamepad", SlotCallSig("GetGamepadInstructions", "Item[],Int32,Int32", PInv(), 0, 0));
            object shop = ((Array)IF(SF(TMain, "instance"), "shop")).GetValue(1);
            Array shopItems = (Array)IF(shop, "item");
            CallI(shopItems.GetValue(0), "SetDefaults", 8, null);
            InvSet(49, 73, 10);
            SSet(TMain, "npcShop", 1);
            SSet(TMain, "mouseLeft", true);
            SSet(TMain, "stackSplit", 0);
            SlotCall("HandleShopSlot", shopItems, 0, false, true);
            P("shopbuy", IF(SF(TMain, "mouseItem"), "type"), IF(SF(TMain, "mouseItem"), "stack"));
            SSet(TMain, "mouseLeft", false);
            SSet(TMain, "npcShop", 0);
            CallI(SF(TMain, "mouseItem"), "TurnToAir");
            CallI(shopItems.GetValue(0), "TurnToAir");
            int co = (int)SF(TMain, "cursorOverride");
            SSet(TMain, "cursorOverride", 3);
            P("overrideclick", SlotCall("OverrideLeftClick", PInv(), 0, 0), IF(InvAt(0), "favorited"));
            SSet(TMain, "cursorOverride", co);
            SlotCall("PickupItemIntoMouse", PInv(), 0, 0, pl);
            P("pickup", IF(SF(TMain, "mouseItem"), "type"), IF(SF(TMain, "mouseItem"), "stack"));
            CallI(SF(TMain, "mouseItem"), "TurnToAir");
            Array armor = (Array)IF(pl, "armor");
            CallI(armor.GetValue(10), "SetDefaults", 90, null);
            SlotCall("SwapVanityEquip", armor, 8, 10, pl);
            P("vanityswap", IF(armor.GetValue(0), "type"), IF(armor.GetValue(10), "type"));
            InvSet(1, 3318, 1);
            SlotCall("TryOpenContainer", PInv(), 0, 1, pl);
            P("container", IF(InvAt(1), "type"), IF(InvAt(1), "stack"));
            object sb = SF(TMain, "spriteBatch");
            DirectDraw("itemslot_money", () => InBatch(() =>
                SlotCall("DrawMoney", sb, "Savings", 500f, 300f, new[] { 12, 34, 5, 1 }, false, false)));
            int cx = x0 + 6;
            PlaceTile(cx, gy - 1, 21, 0);
            int ci = (int)CallS("Terraria.Chest", "FindChest", cx, gy - 2);
            P("chest", ci);
            if (ci >= 0)
            {
                object chest = ((Array)SF(TMain, "chest")).GetValue(ci);
                Array citems = (Array)IF(chest, "item");
                CallI(citems.GetValue(3), "SetDefaults", 9, null);
                ISet(citems.GetValue(3), "stack", 5);
                CallI(citems.GetValue(0), "SetDefaults", 2, null);
                ISet(citems.GetValue(0), "stack", 3);
                InvSet(12, 9, 20);
                Unwrap(MI(T("Terraria.UI.ChestUI"), "DepositAll_IntoLocalChest", 3), null, new object[] { pl, chest, 12 });
                ISet(pl, "chest", ci);
                InvSet(13, 2, 7);
                P("tryplace", CallS("Terraria.UI.ChestUI", "TryPlacingInChest", PInv(), 13, false, 0));
                CallS("Terraria.UI.ChestUI", "Scroll", 1);
                CallS("Terraria.UI.ChestUI", "Scroll", -1);
                Unwrap(MI(T("Terraria.UI.ItemSorting"), "SortInventory", 3), null, new object[] { chest, false, false });
                P("chestsorted", IF(citems.GetValue(0), "type"), IF(citems.GetValue(0), "stack"), IF(citems.GetValue(1), "type"), IF(citems.GetValue(1), "stack"));
                ISet(pl, "chest", -1);
                for (int k = 0; k < citems.Length; k++) CallI(citems.GetValue(k), "TurnToAir");
            }
            InvSet(20, 40, 50);
            InvSet(21, 97, 30);
            CallS("Terraria.UI.ItemSorting", "FillAmmoFromInventory");
            P("ammo", IF(InvAt(54), "type"), IF(InvAt(55), "type"));
            for (int k = 0; k < 58; k++) InvSet(k, 0, 0);
            for (int k = 0; k < armor.Length; k++) CallI(armor.GetValue(k), "TurnToAir");
            Array dyes = (Array)IF(pl, "dye");
            for (int k = 0; k < dyes.Length; k++) CallI(dyes.GetValue(k), "TurnToAir");
            ClearItems();
            StateLine("itemslot");
            Frames(3);
            Record("itemslot", 1);
            ClearEntities();
            ClearFx();
            ClearArea(x0, x0 + 40, gy);
            Frames(2);
        }
    }
}
