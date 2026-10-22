// The probe run, part E (Probe.cs; port: tests/draw/probe_e.rs, the same steps): events (the cultists' ritual, the Old
// One's Army at its crystal, Moon Lord's death drama, the fairies of a mystic log), golf, drop rules, whips tagging a
// target for a minion.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    static void ProbeE(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        // ---- the lunatic cultists' ritual at the dungeon (CultistRitual.TrySpawning -> CheckRitual), forced and not
        if (Group("cultist"))
        {
            int dx = (int)SF(TMain, "dungeonX"), dy = (int)SF(TMain, "dungeonY");
            P("cultist_try", CallS("Terraria.GameContent.Events.CultistRitual", "TrySpawning", dx, dy, false));
            P("cultist_force", CallS("Terraria.GameContent.Events.CultistRitual", "TrySpawning", dx, dy, true));
            int n = 0;
            foreach (object npc in (Array)SF(TMain, "npc")) if ((bool)IF(npc, "active")) n++;
            P("cultist_npcs", n);
            ClearEntities();
            Frames(2);
        }
        // ---- the Old One's Army: its stand and crystal (DD2Event.SummonCrystalDirect), the stand's checks, medals, the
        //      wave points of the first two difficulties, goblins raised by a dark mage, the victory scene
        if (Group("dd2"))
        {
            Type dd2 = T("Terraria.GameContent.Events.DD2Event");
            int stx = x0 + 30;
            bool ok = (bool)TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { stx, gy - 1, 466, false, 0, 0, -1, 1 });
            Note("dd2 stand: " + ok + " " + CoverDiag(stx - 2, gy - 4, 5, 4));
            PlacePlayer(stx - 6, gy - 1);
            P("dd2_fail_here", CallS("Terraria.GameContent.Events.DD2Event", "WouldFailSpawningHere", stx, gy - 1));
            CallS("Terraria.GameContent.Events.DD2Event", "SummonCrystalDirect", stx, gy - 1, (int)SF(TMain, "myPlayer"));
            Frames(5);
            P("dd2_stand", CallS("Terraria.GameContent.Events.DD2Event", "IsStandActive", stx, gy - 1), CallS("Terraria.GameContent.Events.DD2Event", "IsStandActive", stx + 20, gy - 1));
            CallS("Terraria.GameContent.Events.DD2Event", "RequestToSkipWaitTime", stx, gy - 1);
            P("dd2_crystals", CallS("Terraria.GameContent.Events.DD2Event", "ShouldDropCrystals"));
            CallS("Terraria.GameContent.Events.DD2Event", "DropMedals", 3);
            foreach (int wave in new[] { 1, 5, 7 })
            {
                SSet(T("Terraria.NPC"), "waveNumber", wave);
                SSet(T("Terraria.NPC"), "waveKills", 300f);
                foreach (int id in new[] { 552, 555, 561, 564, 565, 576, 577 })
                    P("dd2_points_" + wave + "_" + id, Unwrap(MI(dd2, "Difficulty_1_GetMonsterPointsWorth", 1), null, new object[] { id }), Unwrap(MI(dd2, "Difficulty_2_GetMonsterPointsWorth", 1), null, new object[] { id }));
            }
            SSet(T("Terraria.NPC"), "waveNumber", 0);
            SSet(T("Terraria.NPC"), "waveKills", 0f);
            // a dark mage raising goblins where goblins died
            var pp = (Vector2)IF(pl, "position");
            var spots = (IList)SF(dd2, "_deadGoblinSpots");
            for (int k = 0; k < 4; k++) spots.Add(new Vector2(pp.X + 60f * k - 90f, pp.Y));
            int mage = NewNpc((int)pp.X + 100, (int)pp.Y, 564);
            CallS("Terraria.GameContent.Events.DD2Event", "RaiseGoblins", NpcAt(mage), new Vector2(pp.X, pp.Y));
            Frames(10);
            Record("dd2_raise", 1);
            CallS("Terraria.GameContent.Events.DD2Event", "StartVictoryScene");
            Frames(30);
            Record("dd2_victory", 1);
            CallS("Terraria.GameContent.Events.DD2Event", "StopInvasion", false);
            CallS("Terraria.GameContent.Events.DD2Event", "WipeEntities");
            spots.Clear();
            ClearEntities();
            ClearItems();
            KillTile(stx, gy - 1);
            Frames(30);
        }
        // ---- Moon Lord's death drama: the pieces thrown (MoonlordDeathDrama.ThrowPieces), flying and drawn
        if (Group("mldrama"))
        {
            var pp = (Vector2)IF(pl, "position");
            CallS("Terraria.GameContent.Events.MoonlordDeathDrama", "ThrowPieces", new Vector2(pp.X, pp.Y - 200f), 37);
            Frames(20);
            Record("mldrama_a", 1);
            Frames(120);
            Record("mldrama_b", 1);
            Frames(60);
        }
        // ---- the fairies of a mystic log (a fallen log on the surface at night: MysticLogFairiesEvent.TrySpawningFairies)
        if (Group("mysticlog"))
        {
            SetTime(false, 3600.0);
            int lx = x0 + 10;
            bool ok = (bool)TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { lx, gy - 1, 488, false, 0, 0, -1, 1 });
            Note("fallen log: " + ok + " " + CoverDiag(lx - 2, gy - 3, 6, 3));
            object ev = SF(TWorldGen, "mysticLogsEvent");
            CallI(ev, "StartNight");
            for (int k = 0; k < 40; k++) CallI(ev, "TrySpawningFairies");
            int fairies = 0;
            foreach (object npc in (Array)SF(TMain, "npc")) if ((bool)IF(npc, "active")) fairies++;
            P("mysticlog_npcs", fairies);
            Frames(10);
            Record("mysticlog", 1);
            ClearEntities();
            ClearArea(x0, x0 + 20, gy);
            SetTime(true, 27000.0);
            Frames(5);
        }
        // ---- golf: club properties and shot strengths, a resting ball's shot check, a ball hit, its explosion, the
        //      score of a tracked ball, the prediction line updated and drawn
        if (Group("golf"))
        {
            Type gh = T("Terraria.GameContent.Golf.GolfHelper");
            PlacePlayer(x0 + 10, gy - 1);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            foreach (int club in new[] { 4039, 4092, 4093, 4094, 4587, 4588, 4589, 4590, 4591, 4592, 4593, 4594, 4595, 4596, 4597, 4598, 4599, 4600 })
            {
                object cp = Unwrap(MI(gh, "GetClubProperties", 1), null, new object[] { (short)club });
                P("golf_club_" + club, IF(cp, "MinimumStrength"), IF(cp, "MaximumStrength"), IF(cp, "RoughLandResistance"));
                foreach (Vector2 v in new[] { new Vector2(3f, -4f), new Vector2(0.1f, -0.2f), new Vector2(-20f, -9f) })
                {
                    object ss = Unwrap(MI(gh, "CalculateShotStrength", 2, "Vector2,ClubProperties"), null, new object[] { v, cp });
                    P("golf_shot_" + club + "_" + PV(v), IF(ss, "AbsoluteStrength"), IF(ss, "RelativeStrength"), IF(ss, "RoughLandResistance"));
                }
            }
            int bi = NewProj(pp.X + 30f, pp.Y + 20f, 0f, 0f, 721);
            object ball = ProjAt(bi);
            Frames(20);
            GiveItem(0, 4039);
            SelectItem(0);
            P("golf_valid", CallS("Terraria.GameContent.Golf.GolfHelper", "IsGolfShotValid", ball, pl));
            object gs = SF(TMain, "LocalGolfState");
            CallI(gs, "RecordSwing", ball);
            object line = New("Terraria.GameContent.Golf.FancyGolfPredictionLine", 20);
            foreach (float charge in new[] { 0.2f, 0.7f })
            {
                CallI(line, "Update", ball, new Vector2(4f, -6f), 1f);
                DirectDraw("golf_line_" + PV(charge), () => InBatch(() => CallI(line, "Draw", SF(TMain, "Camera"), SF(TMain, "spriteBatch"), charge)));
            }
            CallS("Terraria.GameContent.Golf.GolfHelper", "HitGolfBall", ball, new Vector2(5f, -7f), 1f);
            Frames(40);
            CallI(gs, "RecordSwing", ball);
            P("golf_score", CallI(gs, "GetGolfBallScore", ball), CallI(gs, "get_ScoreAdjustment"));
            ClearFx();
            CallS("Terraria.GameContent.Golf.GolfHelper+ContactListener", "EmitGolfballExplosion", new Vector2(pp.X, pp.Y - 40f));
            PFx("golf_explosion");
            Frames(2);
            Record("golf", 1);
            GiveItem(0, 0);
            ClearEntities();
            Frames(3);
        }
        // ---- drop rules: the frost / pumpkin moon gating chances (Conditions.FrostMoonDropGatingChance /
        //      PumpkinMoonDropGatingChance.CanDrop) through their waves, the mechanical boss summons' drop rates
        //      (MechBossSpawnersDropRule.ReportDroprates), an item dropped per interacting player on the player
        if (Group("droprules"))
        {
            object frost = New("Terraria.GameContent.ItemDropRules.Conditions+FrostMoonDropGatingChance");
            object pumpkin = New("Terraria.GameContent.ItemDropRules.Conditions+PumpkinMoonDropGatingChance");
            object info = Activator.CreateInstance(T("Terraria.GameContent.ItemDropRules.DropAttemptInfo"));
            int npc = NewNpc(sx * 16, sy * 16 - 100, 3);
            info.GetType().GetField("npc").SetValue(info, NpcAt(npc));
            info.GetType().GetField("player").SetValue(info, pl);
            info.GetType().GetField("rng").SetValue(info, SF(TMain, "rand"));
            foreach (string moon in new[] { "snowMoon", "pumpkinMoon" })
            {
                SSet(TMain, moon, true);
                foreach (int wave in new[] { 1, 6, 12, 15, 20 })
                {
                    SSet(T("Terraria.NPC"), "waveNumber", wave);
                    var a = new List<object>();
                    for (int k = 0; k < 8; k++) a.Add(CallI(moon == "snowMoon" ? frost : pumpkin, "CanDrop", info));
                    P("droprule_" + moon + "_" + wave, a.ToArray());
                }
                SSet(TMain, moon, false);
            }
            SSet(T("Terraria.NPC"), "waveNumber", 0);
            object rule = New("Terraria.GameContent.ItemDropRules.MechBossSpawnersDropRule");
            Type dri = T("Terraria.GameContent.ItemDropRules.DropRateInfo");
            IList drops = (IList)Activator.CreateInstance(typeof(List<>).MakeGenericType(dri));
            object feed = Activator.CreateInstance(T("Terraria.GameContent.ItemDropRules.DropRateInfoChainFeed"), new object[] { 0.5f });
            CallI(rule, "ReportDroprates", drops, feed);
            foreach (object d in drops) P("mechdrop", IF(d, "itemId"), IF(d, "stackMin"), IF(d, "stackMax"), IF(d, "dropRate"));
            CallS("Terraria.GameContent.ItemDropRules.CommonCode", "DropItemForEachInteractingPlayerOnThePlayer", NpcAt(npc), 29, SF(TMain, "rand"), 1, 1, 2, false);
            int items = 0;
            foreach (object wi in (Array)SF(TMain, "item")) if ((bool)IF(wi, "active")) items++;
            P("droprule_items", items);
            ClearItems();
            ClearEntities();
            Frames(2);
        }
        // ---- every whip used on a target dummy while a finch minion attacks it (the tags: TagEffectState.ApplyTagToNPC /
        //      EnableProcOnNPC, WhipTagEffect.AddTheBuff and the procs of every whip)
        if (Group("whips"))
        {
            ClearEntities();
            int wx = x0 + 20;
            PlacePlayer(wx - 5, gy - 1);
            Frames(2);
            object staff = NewItemOf(4281);
            int mshoot = (int)IF(staff, "shoot"), mbuff = (int)IF(staff, "buffType");
            Note("minion " + mshoot + " buff " + mbuff);
            // a training dummy (tile and entity: the entity spawns its NPC when the player is near)
            TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { wx, gy - 1, 378, false, 0, 0, -1, 1 });
            Point dtl = TopLeftOf(378, wx + 1, gy - 1);
            int tid = (int)CallS("Terraria.GameContent.Tile_Entities.TETrainingDummy", "Hook_AfterPlacement", dtl.X, dtl.Y, 378, 0, 1, 0);
            Frames(30);
            int dummy = (int)IF(TEByID(tid), "npc");
            Note("whip dummy at " + dtl.X + "," + dtl.Y + " npc " + dummy + " type " + IF(NpcAt(dummy), "type") + " active " + IF(NpcAt(dummy), "active") + " life " + IF(NpcAt(dummy), "life") + " pos " + PV(IF(NpcAt(dummy), "position")) + " player " + PV(IF(pl, "position")));
            object dn = NpcAt(dummy);
            bool[] whip = (bool[])SF(T("Terraria.ID.ItemID+Sets"), "SummonerWeaponThatScalesWithAttackSpeed");
            int count = (short)SF(T("Terraria.ID.ItemID"), "Count");
            var whips = new List<int>();
            for (int t = 1; t < count; t++) if (whip[t]) whips.Add(t);
            Note("whips: " + string.Join(",", whips.Select(t => t.ToString()).ToArray()));
            var dpos = (Vector2)IF(dn, "position");
            foreach (int w in whips)
            {
                CallI(pl, "AddBuff", mbuff, 3600, false);
                var pp = (Vector2)IF(pl, "position");
                NewProj(pp.X, pp.Y - 30f, 0f, 0f, mshoot);
                GiveItem(0, w);
                SelectItem(0);
                Aim(dpos.X + 10f, dpos.Y + 20f);
                InLeft = true;
                Frames(40);
                InLeft = false;
                Frames(60);
                P("whip_" + w, IF(dn, "life"), IF(pl, "statLife"));
                Record("whip_" + w, 1);
                foreach (object pr in (Array)SF(TMain, "projectile")) ISet(pr, "active", false);
                Frames(2);
            }
            GiveItem(0, 0);
            InMouseX = 640; InMouseY = 360;
            KillTile(dtl.X, dtl.Y);
            ClearEntities();
            Frames(5);
        }
    }
}
