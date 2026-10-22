// The probe run, part J (Probe.cs; port: tests/draw/probe_j.rs, the same steps): Player methods the earlier runs'
// play did not reach, called on the local player with the state that makes them do their work (equipment set,
// items held, NPCs / tiles / projectiles placed), with their results and the state they leave.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    /// <summary>Every active projectile: "index:type:scale:position" (diagnosis of a drawn projectile's values).</summary>
    static string ProjList()
    {
        var sb = new System.Text.StringBuilder();
        Array ps = (Array)SF(TMain, "projectile");
        for (int i = 0; i < ps.Length; i++)
        {
            object p = ps.GetValue(i);
            if (!(bool)IF(p, "active")) continue;
            sb.Append(i + ":" + IF(p, "type") + ":" + PV(IF(p, "scale")) + ":" + PV(IF(p, "position")) + ";");
        }
        return sb.ToString();
    }
    static object PCall(string name, params object[] args) { return Unwrap(MI(T("Terraria.Player"), name, args.Length), LocalPlayer, args); }
    static object PCallSig(string name, string sig, params object[] args) { return Unwrap(MI(T("Terraria.Player"), name, args.Length, sig), LocalPlayer, args); }

    static void ProbeJ(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        int me = (int)SF(TMain, "myPlayer");
        // ---- combat helpers: damage to an NPC, the counterweight, the blood butcherer, the volcano, the horseman's
        //      pumpkins, the jellyfish's sting, collisions with NPCs, the paladin's shield, the minecart's damage
        if (Group("playercombat"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            object z = NpcNear(3, 60f, 0f);
            int zi = (int)IF(z, "whoAmI");
            PCallSig("ApplyDamageToNPC", "NPC,Int32,Single,Int32,Boolean,String,Int32,Int32", z, 25, 2f, 1, false, "probe", 0, 0);
            P("applydamage", IF(z, "life"));
            ISet(pl, "counterWeight", 556);
            int yo = NewProj(pp.X, pp.Y, 0f, 0f, 541);
            ((float[])IF(ProjAt(yo), "ai"))[0] = 0f;
            PCall("Counterweight", (Vector2)IF(z, "Center"), 20, 3f);
            ISet(pl, "counterWeight", 0);
            object blood = NewItemOf(5094);
            ISet(pl, "_spawnBloodButcherer", true);
            PCall("BloodButcherer_TryButchering", z, blood, 30f, 2f);
            P("butcher", ((int[])IF(z, "buffType"))[0]);
            object vol = NewItemOf(5097);
            ISet(pl, "_spawnVolcanoExplosion", true);
            PCall("Volcano_TrySpawningVolcano", z, vol, 40f, 3f, new Rectangle((int)pp.X, (int)pp.Y, 40, 40));
            PCall("HorsemansBlade_SpawnPumpkin", zi, 30, 2f);
            object jelly = NpcNear(63, -60f, 0f);
            PCall("TakeDamageFromJellyfish", (int)IF(jelly, "whoAmI"));
            P("jelly", IF(pl, "statLife"));
            ISet(pl, "immune", false);
            P("collide", PCall("CollideWithNPCs", new Rectangle((int)pp.X - 40, (int)pp.Y - 20, 120, 60), 20f, 4f, 10, 10, 0));
            P("paladin", PCall("CanDefendWithPaladinsShield", 0), PCall("CanDefendWithPaladinsShield", 1));
            object[] md = { 8f, 0, 0f };
            PCall("GetMinecartDamage", md);
            P("minecartdamage", md[1], md[2]);
            Immune();
            ISet(pl, "statLife", 100);
            StateLine("playercombat");
            Frames(3);
            Record("playercombat", 1);
            ClearEntities();
            Frames(2);
        }
        // ---- movement and placement helpers: snapping, rope, fitting, the boots' flames, paint and coating, herbs,
        //      a weapon on a rack, a cannon, the queen slime hook's teleport, the dead cells ground pound, the minecart
        //      hook and the quick minecart on a track
        if (Group("playermove"))
        {
            ClearEntities();
            PlacePlayer(x0 + 10, gy - 1);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            P("snaprange", PCall("IsWithinSnappngRangeToTile", x0 + 12, gy - 1, 100), PCall("IsWithinSnappngRangeToTile", x0 + 40, gy - 1, 100));
            object[] snap = { true, pp, new Vector2(pp.X + 30f, pp.Y) };
            PCall("TryAllowingSnappingToPosition", snap);
            P("snap", snap[0]);
            for (int y = gy - 12; y < gy; y++) PlaceTile(x0 + 14, y, 213, 0);
            P("rope", PCall("CanMoveForwardOnRope", 1, x0 + 14, gy - 6), PCall("CanMoveForwardOnRope", -1, x0 + 14, gy - 6));
            P("fitspace", PCall("CanFitSpace", 0), PCall("CanFitSpace", 3));
            P("bootsflames", PCall("DoBootsEffect_PlaceFlamesOnTile", x0 + 11, gy - 1), PCall("DoBootsEffect_PlaceFlamesOnTile", x0 + 12, gy - 1));
            object paint = NewItemOf(1073);
            PCall("ApplyPaint", x0 + 9, gy, false, false, paint);
            object coat = NewItemOf(4668);
            PCall("ApplyCoating", x0 + 9, gy, false, false, coat);
            PTiles("paint", x0 + 8, gy - 1, 3, 3);
            PlaceTile(x0 + 18, gy - 1, 82, 0);
            PCall("TryReplantingHerbs", 0);
            PTiles("herbs", x0 + 17, gy - 2, 3, 3);
            TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { x0 + 22, gy - 2, 471, true, 0, 0, -1, 1 });
            GiveItem(0, 3507);
            SelectItem(0);
            PCall("PlaceWeapon", x0 + 22, gy - 2);
            GiveItem(0, 0);
            PlaceTile(x0 + 26, gy - 1, 209, 0);
            PCall("ShootFromCannon", x0 + 26, gy - 1);
            PCall("DoQueenSlimeHookTeleport", new Vector2(pp.X + 64f, pp.Y - 32f));
            P("qshook", IF(pl, "position"));
            PCall("DoDeadCellsGroundPoundEffect");
            for (int x = x0 + 30; x <= x0 + 45; x++) PlaceTile(x, gy - 1, 314, 0);
            PlacePlayer(x0 + 36, gy - 2);
            Frames(2);
            PCall("LaunchMinecartHook", x0 + 38, gy - 1);
            PCall("QuickMinecart");
            P("quickminecart", IF(IF(pl, "mount"), "Active"), IF(IF(pl, "mount"), "Type"));
            object mount = IF(pl, "mount");
            if ((bool)IF(mount, "Active")) mount.GetType().GetMethods().First(m => m.Name == "Dismount").Invoke(mount, new object[] { pl, false });
            StateLine("playermove");
            Frames(3);
            Record("playermove", 1);
            ClearEntities();
            ClearArea(x0, x0 + 48, gy);
            Frames(2);
        }
        // ---- small player helpers: coin luck, the respawn timer, luck, prices, the void bag, mana heat, slow magic,
        //      shadow dodge, the nebula level-up, drums, the shimmer-free spot, minion aiming, spectating, the toilet,
        //      the campfire styles, the range compensation, tile entity reach
        if (Group("playermisc"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            PCall("AddCoinLuck", new Vector2(pp.X + 20f, pp.Y), 12345);
            P("coinluck", IF(pl, "coinLuck"), IF(pl, "NormalizedLuck"));
            Unwrap(MI(T("Terraria.Player"), "AdjustRespawnTimerForWorldJoining", 1), null, new object[] { pl });
            P("respawn", IF(pl, "respawnTimer"));
            foreach (int t in new[] { 1, 46, 3507, 5000 })
            {
                object[] pr = { NewItemOf(t), 0L, 0L };
                PCall("GetItemExpectedPrice", pr);
                P("price_" + t, pr[1], pr[2]);
            }
            object vault = IF(pl, "bank4");
            Array vinv = (Array)IF(vault, "item");
            CallI(vinv.GetValue(0), "SetDefaults", 9, null);
            ISet(vinv.GetValue(0), "stack", 5);
            object wood = NewItemOf(9);
            ISet(wood, "stack", 20);
            object settings = SF(T("Terraria.GetItemSettings"), "PickupItemFromWorld");
            P("voidbag", Unwrap(MI(T("Terraria.Player"), "GetItem_FillIntoOccupiedSlot_VoidBag", 5), pl, new object[] { vinv, wood, settings, NewItemOf(0), 0 }), IF(vinv.GetValue(0), "stack"), IF(wood, "stack"));
            CallI(vinv.GetValue(0), "TurnToAir");
            PCall("TryApplyManaHeat", 0.5f, 120f);
            P("manaheat", IF(pl, "manaSick"), IF(pl, "manaSickReduction"));
            ISet(pl, "slowMagicUse", true);
            PCall("UndoSlowMagicUse");
            PCall("ShadowDodge");
            PCall("NebulaLevelup", 178);
            P("nebula", IF(pl, "nebulaLevelDamage"), IF(pl, "nebulaLevelLife"), IF(pl, "nebulaLevelMana"));
            PCall("PlayDrums", 0.75f);
            P("shimmerfree", PCall("TryFindingShimmerFreeSpot"));
            NpcNear(3, 80f, 0f);
            Aim(pp.X + 90f, pp.Y + 10f);
            Frames(1);
            PCall("MinionNPCTargetAim", false);
            PCall("MinionRestTargetAim");
            P("miniontarget", IF(pl, "MinionAttackTargetNPC"), IF(pl, "MinionRestTargetPoint"));
            InMouseX = 640; InMouseY = 360;
            P("spectate", PCall("ClosestSpectatablePlayerTo", new Vector2(pp.X, pp.Y)));
            PCall("TryToPoop");
            P("campfire_hold", PCall("BiomeCampfireHoldStyle", 4689), PCall("BiomeCampfireHoldStyle", 966));
            object[] cs = { 215, 0 };
            PCall("BiomeCampfirePlaceStyle", cs);
            P("campfire_place", cs[0], cs[1]);
            P("rangecomp", PCall("ApplyRangeCompensation", 0.2f, new Vector2(pp.X, pp.Y), new Vector2(pp.X + 300f, pp.Y - 120f)));
            object reach = SF(T("Terraria.DataStructures.TileReachCheckSettings"), "Simple");
            P("tereach", PCall("InTileEntityInteractionRange", sx + 2, sy - 2, 2, 2, reach), PCall("InTileEntityInteractionRange", sx + 60, sy - 2, 2, 2, reach));
            Immune();
            ISet(pl, "statLife", 100);
            StateLine("playermisc");
            Frames(3);
            Record("playermisc", 1);
            ClearEntities();
            Frames(2);
        }
        // ---- the forbidden armour's storm (Player.CommandForbiddenStorm, StrayMethods.CanSpawnSandstormFriendly), a
        //      fishing bobber pulled with bait (ItemCheck_CheckFishingBobber_ConsumeBait / PullBobber), an item pulled to
        //      the player (PullItem_Pickup)
        if (Group("playerstorm"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            SetArmor(0, 3776); SetArmor(1, 3777); SetArmor(2, 3778);
            Frames(3);
            var pp = (Vector2)IF(pl, "position");
            Aim(pp.X + 100f, pp.Y - 60f);
            Frames(1);
            PCall("CommandForbiddenStorm");
            P("sandstormfriendly", CallS("Terraria.StrayMethods", "CanSpawnSandstormFriendly", new Vector2(pp.X + 100f, pp.Y), 3, 3));
            Frames(10);
            P("storm_projs", ProjList());
            Record("forbiddenstorm", 1);
            P("storm_projs2", ProjList());
            for (int k = 0; k < 3; k++) SetArmor(k, 0);
            InMouseX = 640; InMouseY = 360;
            SetInv(10, 2002, 5);
            int bob = NewProj(pp.X + 60f, pp.Y, 0f, 0f, 360);
            object bobber = ProjAt(bob);
            float[] bai = (float[])IF(bobber, "ai");
            bai[1] = -1f;
            ((float[])IF(bobber, "localAI"))[1] = 2002;
            object[] cb = { bobber, 0 };
            P("bait", PCall("ItemCheck_CheckFishingBobber_ConsumeBait", cb), cb[1]);
            PCall("ItemCheck_CheckFishingBobber_PullBobber", bobber, 2002);
            GiveItem(10, 0);
            MethodInfo newItem = T("Terraria.Item").GetMethods(AllStatic).First(m => m.Name == "NewItem" && m.GetParameters().Length == 12);
            int wi = (int)newItem.Invoke(null, new object[] { Src(), (int)pp.X + 150, (int)pp.Y, 16, 16, 9, 1, false, 0, Enum.ToObject(newItem.GetParameters()[9].ParameterType, 0), null, null });
            object w = ((Array)SF(TMain, "item")).GetValue(wi);
            PCall("PullItem_Pickup", w, 12f, 5);
            P("pullitem", IF(w, "velocity"));
            ClearItems();
            StateLine("playerstorm");
            Frames(3);
            ClearEntities();
            Frames(2);
        }
    }
}
