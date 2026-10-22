// The probe run, part I (Probe.cs; port: tests/draw/probe_i.rs, the same steps): NPC methods that the earlier runs'
// fights and towns did not reach, each called on an NPC of the kind that uses it (boss attacks, town NPC helpers,
// spawn helpers), with its results, the NPCs / projectiles / effects it made, and a frame drawn after each part.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    static object NpcNear(int type, float dx, float dy)
    {
        var pp = (Vector2)IF(LocalPlayer, "position");
        object n = NpcAt(NewNpc((int)(pp.X + dx), (int)(pp.Y + dy), type));
        ISet(n, "target", (int)SF(TMain, "myPlayer"));
        return n;
    }
    static object NCall(object n, string name, params object[] args) { return Unwrap(MI(T("Terraria.NPC"), name, args.Length), n, args); }

    static void ProbeI(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        int me = (int)SF(TMain, "myPlayer");
        // ---- boss and enemy attacks
        if (Group("npcattacks"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            object ks = NpcNear(50, 200f, -100f);
            object[] a = { false, 0f, 0f };
            P("ks_teleport", NCall(ks, "AI_015_KingSlime_FindTeleportSpot", a), a[1], a[2]);
            a = new object[] { true, 0f, 0f };
            P("ks_teleport_anticheese", NCall(ks, "AI_015_KingSlime_FindTeleportSpot", a), a[1], a[2]);
            NCall(ks, "AddKingSlimeTeleportCacheTiles", sx - 10, sx + 10, sy - 7, sy + 3);
            P("ks_cache", SF(T("Terraria.NPC"), "kingSlimePointCacheSize"));
            object eol = NpcNear(636, -200f, -200f);
            ClearFx();
            for (int e = 0; e < 4; e++) NCall(eol, "AI_120_HallowBoss_DoMagicEffect", (Vector2)IF(eol, "Center"), e, 0.5f);
            P("eol_canshoot", NCall(eol, "Boss_CanShootExtraAt", me, 0, 3, 2000f, false), NCall(eol, "Boss_CanShootExtraAt", me, 1, 3, 2000f, true));
            PFx("eol_magic");
            object qs = NpcNear(657, 300f, -300f);
            for (int k = 0; k < 5; k++) NCall(qs, "AI_121_QueenSlime_FlyMovement");
            P("qs_fly", IF(qs, "velocity"), IF(qs, "noGravity"));
            object dc = NpcNear(668, 150f, -50f);
            object td = NCall(dc, "GetTargetData", true);
            float[] dai = (float[])IF(dc, "ai");
            dai[0] = 56f;
            NCall(dc, "AI_123_Deerclops_MakeSpikesBothSides", 0, td);
            object[] r = { td, new Point(sx, sy), 3, 2, 10f, 1 };
            NCall(dc, "AI_123_Deerclops_ShootRubbleUp", r);
            NCall(dc, "SpawnPassiveShadowHands", 0.4f, 20);
            object mimic = NpcNear(473, -150f, -30f);
            NCall(mimic, "AI_87_BigMimic_ShootItem", 3005);
            NCall(mimic, "BigMimicSpawnSmoke");
            object elder = NpcNear(1, 60f, -30f);
            NCall(elder, "AI_124_ElderSlimeChest");
            object naut = NpcNear(618, 250f, 0f);
            NCall(naut, "BloodNautilus_CallForHelp");
            object worm = NpcNear(402, -300f, -100f);
            NCall(worm, "SpawnStardustMark_StardustWorm");
            object hornet = NpcNear(42, 90f, -60f);
            NCall(hornet, "StingerExplosion");
            NCall(hornet, "ViolentlySpawnNerdySlime");
            StateLine("npcattacks");
            PFx("npcattacks");
            Frames(3);
            Record("npcattacks", 1);
            ClearEntities();
            ClearFx();
            Frames(2);
        }
        // ---- town NPC helpers: the shimmered town NPC's landing spot, the lost girl's books, swing stats, petting,
        //      variations, tombstones, the dryad's animation, the merchant's spawn rule
        if (Group("npctown"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            object guide = NpcNear(22, 40f, 0f);
            P("shimmer_spot", NCall(guide, "AI_007_TownEntities_Shimmer_ScanForBestSpotToLandOn"));
            object[] a = { new Point(sx, sy), 30, default(Vector2) };
            P("books_teleport", NCall(guide, "AI_AttemptToFindTeleportSpotNearBooks", a), a[2]);
            object[] w = { sx, sy - 5, true, default(Vector2), false };
            P("books_wall", NCall(guide, "AI_AttemptToFindTeleportSpotNearBooks_SearchWall", w), w[3], w[4]);
            object[] b = { new Point(sx - 20, sy - 15), 40, 30, default(Point), true, false };
            P("books_near", Unwrap(MI(T("Terraria.NPC"), "AI_FindNearbyBook", 6), null, b), b[3]);
            foreach (int[] s in new[] { new[] { 20, 5, 1, 30, 30 }, new[] { 20, 15, -1, 40, 20 } })
            {
                object tup = NCall(guide, "GetSwingStats", s[0], s[1], s[2], s[3], s[4]);
                object[] t = { s[0], s[1], s[2], new Rectangle(10, 20, 30, 40) };
                NCall(guide, "TweakSwingStats", t);
                P("swing_" + s[1], IF(tup, "Item1"), IF(tup, "Item2"), t[3]);
            }
            object cat = NpcNear(637, -40f, 0f);
            object pi = NCall(cat, "GetPettingInfo", pl);
            P("petting", IF(pi, "isPetting"), IF(pi, "npc"), IF(pi, "type"), IF(pi, "offsetFromPet"), IF(pi, "isPetSmall"));
            CallI(pl, "PetAnimal", pi);
            P("reroll", NCall(cat, "RerollVariation"), IF(cat, "townNpcVariationIndex"));
            object nt = CallS("Terraria.Localization.NetworkText", "FromLiteral", "Probe was slain");
            NCall(guide, "DropTombstoneTownNPC", nt);
            NpcNear(20, 80f, 0f);
            CallS("Terraria.NPC", "HaveDryadDoStardewAnimation");
            P("merchant_allowed", CallS("Terraria.NPC", "SpawnAllowed_Merchant"));
            P("dryadsbane", NCall(guide, "GetDryadsBaneDamagePerSecond"));
            P("fairycircle", NCall(guide, "GetFairyCircleOffset", 1.5f, 0.3f, 20f), NCall(guide, "GetFairyCircleOffset", 7.25f, 2f, 5f));
            object frog = NpcNear(677, 30f, 0f);
            P("mysticfrog", NCall(frog, "TryTeleportingCaughtMysticFrog"));
            StateLine("npctown");
            Frames(3);
            Record("npctown", 1);
            ClearEntities();
            Frames(2);
        }
        // ---- spawning and death helpers: the Wall of Flesh's brick box, Moon Lord's true eye, mechanical spawns from
        //      statues, Skeletron (and with its red hat), lady bugs, fairies, the eater of worlds' head search, the
        //      spawner's frogs / hornets / lava critters / faelings
        if (Group("npcspawns"))
        {
            ClearEntities();
            PlacePlayer(480, 1009);
            Frames(2);
            object wof = NpcNear(113, 0f, -40f);
            var wp = (Vector2)IF(wof, "position");
            PTiles("wofbox_before", (int)(wp.X / 16) - 30, (int)(wp.Y / 16) - 30, 60, 60);
            NCall(wof, "CreateBrickBoxForWallOfFlesh");
            PTiles("wofbox_after", (int)(wp.X / 16) - 30, (int)(wp.Y / 16) - 30, 60, 60);
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            object core = NpcNear(398, 0f, -300f);
            NCall(core, "MoonLord_SpawnTrueEyeOfCthulhu");
            P("mechspawn", CallS("Terraria.NPC", "MechSpawn", pp.X, pp.Y, 3), CallS("Terraria.NPC", "MechSpawn", pp.X + 500f, pp.Y, 1));
            CallS("Terraria.NPC", "SpawnSkeletron", me, false);
            CallS("Terraria.NPC", "RedHatSkeletron", me);
            ClearFx();
            CallS("Terraria.NPC", "LadyBugKilled", new Vector2(pp.X + 30f, pp.Y), false);
            CallS("Terraria.NPC", "LadyBugKilled", new Vector2(pp.X - 30f, pp.Y), true);
            CallS("Terraria.NPC", "FairyEffects", new Vector2(pp.X, pp.Y - 40f), 583);
            PFx("npcspawns_fx");
            P("anyfairies", CallS("Terraria.NPC", "AnyHelpfulFairies"), CallS("Terraria.NPC", "AnyLifeCrystalSlimes"), CallS("Terraria.NPC", "AnyoneNearCultists"));
            int head = NewNpc((int)pp.X + 200, (int)pp.Y - 200, 13);
            object[] loc = { head, true, true, 0, default(Vector2) };
            P("npclocation", Unwrap(MI(T("Terraria.NPC"), "GetNPCLocation", 5), null, loc), loc[3], loc[4]);
            P("devourer", Unwrap(MI(T("Terraria.NPC+Spawner"), "CanSpawnDevourer", 1), null, new object[] { sx }));
            object sp = Activator.CreateInstance(T("Terraria.NPC+Spawner"));
            Unwrap(MI(T("Terraria.NPC+Spawner"), "SpawnFrog", 2), sp, new object[] { sx + 10, sy });
            object hn = Unwrap(MI(T("Terraria.NPC+Spawner"), "SpawnHornet", 2), sp, new object[] { sx - 10, sy });
            object lc = Unwrap(MI(T("Terraria.NPC+Spawner"), "SpawnLavaBaitCritters", 2), sp, new object[] { sx + 20, sy });
            P("spawner", hn == null ? -1 : IF(hn, "type"), lc == null ? -1 : IF(lc, "type"));
            Unwrap(MI(T("Terraria.NPC+Spawner"), "SpawnFaelings", 1), null, new object[] { pl });
            StateLine("npcspawns");
            Frames(3);
            Record("npcspawns", 1);
            ClearEntities();
            ClearFx();
            Frames(2);
        }
        // ---- small NPC helpers: buffs removed, expert hardmode stats, the key brand, a reflected projectile, the
        //      buff colour, the damage-over-time tally of stacked projectiles
        if (Group("npcmisc"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            object z = NpcNear(3, 120f, 0f);
            CallI(z, "AddBuff", 20, 300, false);
            CallI(z, "AddBuff", 24, 300, false);
            NCall(z, "DelBuff", 0, true);
            P("delbuff", ((int[])IF(z, "buffType"))[0], ((int[])IF(z, "buffTime"))[0]);
            bool hm = (bool)SF(TMain, "hardMode");
            SSet(TMain, "hardMode", true);
            NCall(z, "ScaleStats_ForExpertHardmode");
            P("expert_hm", IF(z, "lifeMax"), IF(z, "damage"), IF(z, "defense"));
            SSet(TMain, "hardMode", hm);
            P("keybrand", NCall(z, "KeyBrandStrike", me, 100, (Vector2)IF(z, "Center")));
            var pp = (Vector2)IF(pl, "position");
            int pr = NewProj(pp.X, pp.Y, 6f, 0f, 1);
            NCall(z, "ReflectProjectile", ProjAt(pr));
            P("reflect", IF(ProjAt(pr), "velocity"), IF(ProjAt(pr), "hostile"), IF(ProjAt(pr), "friendly"));
            foreach (float[] c in new[] { new[] { 1f, 0.5f, 0.2f, 1f }, new[] { 0.1f, 0.9f, 0.3f, 0.5f } })
                P("buffcolor_" + PV(c[0]), CallS("Terraria.NPC", "buffColor", new Color(200, 150, 100, 255), c[0], c[1], c[2], c[3]));
            object tally = Activator.CreateInstance(T("Terraria.NPC+DOTTally"));
            T("Terraria.NPC+DOTTally").GetField("npc").SetValue(tally, z);
            for (int k = 0; k < 3; k++) NewProj(((Vector2)IF(z, "position")).X, ((Vector2)IF(z, "position")).Y, 0f, 0f, 598);
            foreach (object p in (Array)SF(TMain, "projectile"))
                if ((bool)IF(p, "active") && (int)IF(p, "type") == 598) { float[] ai = (float[])IF(p, "ai"); ai[0] = 1f; ai[1] = (int)IF(z, "whoAmI"); }
            Unwrap(MI(T("Terraria.NPC+DOTTally"), "AddStackedProjecitle", 3), tally, new object[] { (short)598, 3, 1 });
            P("dottally", IF(tally, "LifeRegenLoss"), IF(tally, "LifeRegenLossThatCanBeBoosted"), IF(tally, "ExpectedDamagePerSecond"));
            StateLine("npcmisc");
            ClearEntities();
            Frames(2);
        }
        // ---- the dual dungeons seed's dungeon zones (SceneMetrics.CalculateZones -> NPCSpawningFlagsForDualDungeons.ScanZonesFor)
        if (Group("dualdungeons"))
        {
            SSet(TMain, "dualDungeonsSeed", true);
            PlacePlayer(3352, 295);
            Frames(30);
            Record("dualdungeons", 1);
            SSet(TMain, "dualDungeonsSeed", false);
            PlacePlayer(sx, sy);
            Frames(10);
        }
    }
}
