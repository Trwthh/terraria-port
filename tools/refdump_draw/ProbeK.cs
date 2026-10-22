// The probe run, part K (Probe.cs; port: tests/draw/probe_k.rs, the same steps): Projectile, Utils and WorldGen
// methods the earlier runs' play did not reach, called directly with the state that makes them do their work.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    static object JCall(object proj, string name, params object[] args) { return Unwrap(MI(T("Terraria.Projectile"), name, args.Length), proj, args); }
    static object JCallS(string name, params object[] args) { return Unwrap(MI(T("Terraria.Projectile"), name, args.Length), null, args); }
    static object UCall(string name, params object[] args) { return Unwrap(MI(T("Terraria.Utils"), name, args.Length), null, args); }
    static object UCallSig(string name, string sig, params object[] args) { return Unwrap(MI(T("Terraria.Utils"), name, args.Length, sig), null, args); }
    static object WCall(string name, params object[] args) { return Unwrap(MI(TWorldGen, name, args.Length), null, args); }
    static object WCallSig(string name, string sig, params object[] args) { return Unwrap(MI(TWorldGen, name, args.Length, sig), null, args); }
    static bool TileActive(object t) { return t != null && (bool)Unwrap(MI(t.GetType(), "active", 0), t, new object[0]); }
    static int TileType(object t) { return (ushort)IF(t, "type"); }
    /// <summary>Every buff of the local player removed.</summary>
    static void ClearPlayerBuffs()
    {
        int[] bt = (int[])IF(LocalPlayer, "buffType"), bm = (int[])IF(LocalPlayer, "buffTime");
        for (int k = 0; k < bt.Length; k++) { bt[k] = 0; bm[k] = 0; }
    }
    /// <summary>The liquid of every tile in the box removed.</summary>
    static void ClearLiquid(int x, int y, int w, int h)
    {
        for (int i = x; i < x + w; i++)
            for (int j = y; j < y + h; j++)
            {
                object t = TileAt(i, j);
                if (t != null) ISet(t, "liquid", (byte)0);
            }
    }
    static int CountProj(int type)
    {
        int n = 0;
        foreach (object p in (Array)SF(TMain, "projectile")) if ((bool)IF(p, "active") && (int)IF(p, "type") == type) n++;
        return n;
    }

    static void ProbeK(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        int me = (int)SF(TMain, "myPlayer");
        // ---- projectile helpers: the bobber's splash, interception, the pirates and the tiger, the mystic snake's rope,
        //      the flint's spikes, betsy's shrapnel, boulders, the constellation whip, impact explosions, cracked tiles,
        //      petting, javelins, the stellar tune, insanity shadows, lightning particles, the moon lord whip, webs, PvP
        //      status, the monk's ghast, the star slash, the snapping stone, the gas trap, ghost and vampire heals
        if (Group("projdirect"))
        {
            ClearEntities();
            ClearFx();
            PlacePlayer(x0 + 2, gy - 1);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            object z = NpcNear(3, 80f, 0f);
            int zi = (int)IF(z, "whoAmI");
            var zc = (Vector2)IF(z, "Center");
            object p = ProjAt(NewProj(pp.X + 40f, pp.Y - 40f, 0f, 0f, 360));
            JCall(p, "AI_061_FishingBobber_DoASplash");
            p = ProjAt(NewProj(pp.X, pp.Y - 40f, 0f, 0f, 388));
            JCall(p, "AI_066_TryInterceptingTarget", new Vector2(80f, -10f), new Vector2(1.5f, 0f), 12f);
            P("intercept", IF(p, "velocity"));
            object pirate = ProjAt(NewProj(pp.X + 30f, pp.Y, 0f, 0f, 1093));
            JCall(pirate, "AI_067_FreakingPirates_HitIntention", new Vector2((x0 + 3) * 16 + 8, gy * 16 + 8));
            PTiles("pirate_hit", x0 + 2, gy - 1, 3, 3);
            p = ProjAt(NewProj(pp.X - 30f, pp.Y, 0f, 0f, 1022));
            JCall(p, "AI_067_FreakingPirates_TryJumpingToTarget", Vector2.Zero, new Vector2(pp.X, pp.Y - 200f), new Vector2(pp.X, pp.Y - 200f));
            P("pirate_jump", IF(p, "velocity"));
            p = ProjAt(NewProj(pp.X - 10f, pp.Y, 0f, 0f, 834));
            P("tiger", JCall(p, "AI_067_TigerSpecialAttack"), IF(p, "velocity"));
            JCall(p, "AI_155_SpawnRopeIn", x0 + 5, gy - 3);
            JCall(p, "AI_198_Flint_EmitSpikes", pp, 1);
            JCall(p, "BetsySharpnel", zi);
            JCall(p, "BoulderExplosion");
            ISet(pl, "itemAnimationMax", 30);
            object whip = ProjAt(NewProj(pp.X, pp.Y, 0f, 0f, 847));
            ((float[])IF(whip, "ai"))[0] = 10f;
            JCall(whip, "ConstellationWhip_SpawnConstellationStarsTowards", z, 50);
            JCallS("SpawnMoonLordWhipProc", whip, z, 30, 0);
            ISet(pl, "itemAnimationMax", 0);
            var origin = new Vector2((x0 + 5) * 16, gy * 16);
            object[] ie = { 1, origin, new Point(x0 + 2, gy - 2), new Point(x0 + 8, gy + 1), 80, false };
            JCall(p, "CreateImpactExplosion", ie);
            P("impact", ie[2], ie[3], ie[5]);
            JCall(p, "CreateImpactExplosion2_FlailTileCollision", origin, true, new Vector2(3f, 4f));
            for (int x = x0 + 12; x <= x0 + 14; x++) PlaceTile(x, gy - 1, 481, 0);
            JCall(p, "ExplodeCrackedTiles", new Vector2((x0 + 13) * 16, (gy - 1) * 16), 3, x0 + 10, x0 + 16, gy - 3, gy);
            PTiles("cracked", x0 + 10, gy - 3, 7, 4);
            JCall(pirate, "GetPettingInfo", pl);
            for (int k = 0; k < 3; k++)
            {
                object jv = ProjAt(NewProj(zc.X, zc.Y, 0f, 0f, 598));
                float[] jai = (float[])IF(jv, "ai");
                jai[0] = 1f;
                jai[1] = zi;
                ISet(jv, "timeLeft", 100 + k * 50);
            }
            JCallS("KillOldestJavelin", -1, 598, zi, new Point[2]);
            P("javelins", CountProj(598));
            object lb = ProjAt(NewProj((x0 + 20) * 16, (gy - 3) * 16, 0f, 0f, 1));
            JCall(lb, "LavaBoulderExplosion");
            PTiles("lava", x0 + 15, gy - 8, 11, 9);
            ClearLiquid(x0 + 15, gy - 8, 11, 9);
            object st = ProjAt(NewProj(pp.X, pp.Y - 100f, 0f, 0f, 856));
            object[] dmg = { 100 };
            JCall(st, "ModifyStellarTuneDamage", dmg);
            P("stellartune", dmg[0]);
            object[] ins = { pl, true, Vector2.Zero, Vector2.Zero, 0f, 0f };
            JCallS("RandomizeInsanityShadowFor", ins);
            P("insanity", ins[2], ins[3], ins[4], ins[5]);
            JCall(p, "SpawnLightningLiquidElectrificationParticles");
            object web = ProjAt(NewProj((x0 + 30) * 16, (gy - 4) * 16, 0f, 0f, 1));
            JCall(web, "SpawnWebs");
            PTiles("webs", x0 + 23, gy - 11, 15, 12);
            ISet(pl, "meleeEnchant", (byte)1);
            ISet(p, "melee", true);
            JCall(p, "StatusPvP", me);
            ISet(pl, "meleeEnchant", (byte)0);
            P("statuspvp", ((int[])IF(pl, "buffType"))[0]);
            ClearPlayerBuffs();
            JCall(p, "SummonMonkGhast");
            JCall(p, "SummonSuperStarSlash", zc);
            object[] snap = { Src(), pl, me, z, 50, 0 };
            JCallS("TryMakingSnappingStoneStrike", snap);
            P("snapping", snap[4]);
            int cx = x0 + 34;
            PlaceTile(cx, gy - 1, 21, 0);
            int ci = (int)CallS("Terraria.Chest", "FindChest", cx, gy - 2);
            P("gaschest", ci);
            if (ci >= 0)
            {
                object chest = ((Array)SF(TMain, "chest")).GetValue(ci);
                Array citems = (Array)IF(chest, "item");
                CallI(citems.GetValue(0), "SetDefaults", 9, null);
                ISet(citems.GetValue(0), "stack", 2);
                JCallS("UseGasTrapInChest", ci, chest, 0, 0, 0);
                P("gastrap", IF(citems.GetValue(0), "stack"), IF(citems.GetValue(1), "type"));
                for (int k = 0; k < citems.Length; k++) CallI(citems.GetValue(k), "TurnToAir");
            }
            ISet(pl, "lifeSteal", 100f);
            ISet(p, "magic", true);
            ISet(p, "damage", 40);
            JCall(p, "ghostHeal", 50, zc, z);
            JCall(p, "ghostHurt", 50, zc, z);
            JCall(p, "vampireHeal", 50, zc, z);
            P("heals", IF(pl, "lifeSteal"), IF(pl, "ghostDmg"));
            PFx("projdirect");
            Immune();
            ISet(pl, "statLife", 100);
            StateLine("projdirect");
            Frames(3);
            Record("projdirect", 1);
            ClearEntities();
            ClearFx();
            ClearItems();
            ClearArea(x0, x0 + 40, gy);
            ClearLiquid(x0, gy - 12, 41, 12);
            Frames(2);
        }
        // ---- Utils helpers: cones, easing, jumps, ping-pong curves, a random float, hue shifts, quadratics, the poof of
        //      smoke, lines and rectangles drawn
        if (Group("utilsdirect"))
        {
            ClearEntities();
            ClearFx();
            PlacePlayer(sx, sy);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            P("fitcone", UCall("DoesFitInCone", new Vector2(100f, 20f), Vector2.Zero, 200f, 0.1f, 0.6f), UCall("DoesFitInCone", new Vector2(-50f, 80f), Vector2.Zero, 200f, 0.1f, 0.6f));
            P("conefast", UCall("IntersectsConeFastInaccurate", new Rectangle(90, 0, 20, 20), Vector2.Zero, 200f, 0.2f, 0.5f), UCall("IntersectsConeFastInaccurate", new Rectangle(-90, 60, 20, 20), Vector2.Zero, 200f, 0.2f, 0.5f));
            P("easeincirc", UCall("EaseInCirc", 0.3), UCall("EaseInCirc", 0.85));
            P("jump", UCall("GetJumpForce", 64f, 0.3f), UCall("GetJumpTimeToApex", 64f, 0.3f));
            P("pingpong", UCall("PingPongFrom01To010", 0.3f), UCall("PingPongFrom01To010", 0.8f), UCall("Turn01ToCyclic010", 0.3f), UCall("Turn01ToCyclic010", 0.7f));
            object[] seed = { 12345678901UL };
            P("randomfloat", UCall("RandomFloat", seed), seed[0]);
            P("shifthue", UCall("ShiftHue", new Color(200, 100, 50, 255), 0.3f, 0.1f, 0.2f));
            object[] q = { 1f, -3f, 2f, 0f, 0f };
            P("quadratic", UCall("SolveQuadratic", q), q[3], q[4]);
            object[] q2 = { 1f, 0f, 5f, 0f, 0f };
            P("quadratic_none", UCall("SolveQuadratic", q2), q2[3], q2[4]);
            UCall("PoofOfSmoke", pp);
            PFx("poof");
            ClearFx();
            object sb = SF(TMain, "spriteBatch");
            DirectDraw("utils_lines", () => InBatch(() =>
            {
                UCallSig("DrawLine", "SpriteBatch,Vector2,Vector2,Color", sb, pp, pp + new Vector2(120f, -40f), Color.Red);
                UCallSig("DrawLine", "SpriteBatch,Point,Point,Color", sb, new Point(sx, sy), new Point(sx + 5, sy - 3), Color.Yellow);
                UCallSig("DrawLine", "SpriteBatch,Vector2,Vector2,Color,Color,Single", sb, pp, pp + new Vector2(100f, -50f), Color.Red, Color.Blue, 2f);
                UCallSig("DrawRect", "SpriteBatch,Rectangle,Color", sb, new Rectangle(sx, sy - 4, 4, 3), Color.Green);
            }));
            StateLine("utilsdirect");
            Frames(2);
        }
        // ---- WorldGen helpers: wires 2-4, the tall gate shifted, the cannon, plantera's thorns, the gem lock, tree
        //      growth effects, a locked door, paint effects, the tree and ground conversion, the spider wall remover,
        //      lightning, the travelling merchant leaving, plantera's bulb
        if (Group("wgdirect"))
        {
            ClearEntities();
            ClearFx();
            PlacePlayer(x0 + 2, gy - 1);
            Frames(2);
            P("wires", WCall("PlaceWire2", x0 + 5, gy - 2), WCall("PlaceWire3", x0 + 6, gy - 2), WCall("PlaceWire4", x0 + 7, gy - 2), WCall("PlaceWire2", x0 + 5, gy - 2));
            P("gate", PlaceTile(x0 + 10, gy - 1, 388, 0));
            P("tallgate", WCall("ShiftTallGate", x0 + 10, gy - 1, false, false), WCall("ShiftTallGate", x0 + 10, gy - 1, true, false));
            PTiles("tallgate", x0 + 9, gy - 6, 3, 6);
            WCall("ShootFromCannon", x0 + 14, gy - 3, 2, 1, 50, 4f, me, false);
            WCall("ShootFromCannon", x0 + 14, gy - 3, 0, 4, 0, 0f, me, true);
            WCall("SpawnPlanteraThorns", new Vector2((x0 + 20) * 16, (gy - 6) * 16));
            PTiles("thorns", x0 + 0, gy - 12, 41, 12);
            for (int x = x0 + 22; x <= x0 + 26; x++) for (int y = gy - 5; y < gy; y++) CallStatic(TWorldGen, "PlaceWall", x, y, 4, true);
            P("gemlock", TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { x0 + 24, gy - 2, 440, true, 0, 0, -1, 1 }));
            WCall("ToggleGemLock", x0 + 24, gy - 2, true);
            WCall("ToggleGemLock", x0 + 24, gy - 2, false);
            PTiles("gemlock", x0 + 22, gy - 5, 5, 5);
            int tx = -1, ty = -1;
            for (int x = sx - 80; x <= sx + 80 && tx < 0; x++)
                for (int y = sy - 40; y <= sy + 20; y++)
                {
                    object t = TileAt(x, y), up = TileAt(x, y - 1), dn = TileAt(x, y + 1);
                    if (TileActive(t) && TileType(t) == 5 && TileActive(up) && TileType(up) == 5 && TileActive(dn) && TileType(dn) != 5) { tx = x; ty = y; break; }
                }
            P("tree", tx, ty);
            if (tx >= 0)
            {
                WCall("TreeGrowFXCheck", tx, ty);
                WCall("ConvertTreeAndGround", tx, ty, 2, 5, 5);
            }
            WCall("TreeGrowFX_AshShake", x0 + 28, gy - 1, 8, false);
            WCall("TreeGrowFX_AshShake", x0 + 28, gy - 1, 8, true);
            PlaceTile(x0 + 32, gy - 1, 10, 11);
            WCall("UnlockDoor", x0 + 32, gy - 1);
            PTiles("door", x0 + 31, gy - 4, 3, 4);
            WCall("paintEffect", x0 + 5, gy, (byte)3, (byte)0);
            WCall("paintEffect", x0 + 6, gy, (byte)0, (byte)7);
            WCall("paintCoatEffect", x0 + 7, gy, (byte)1, new List<Color>());
            WCall("paintCoatEffect", x0 + 8, gy, (byte)0, new List<Color> { Color.Red, Color.Blue });
            WCall("SpiderWallRemover", x0 + 60, gy + 30);
            PTiles("spiderwalls", x0 + 30, gy, 61, 61);
            var safe = new List<Rectangle>();
            object[] lb = { safe, x0 + 8, gy - 1, false };
            P("lightning", WCallSig("TrySpawnLightningBolt", "List`1,Int32,Int32,Boolean&", lb), lb[3]);
            WCall("SpawnStormLightningNearPlayer", safe, pl);
            P("lightning_proj", CountProj(1091));
            NpcNear(368, 200f, 0f);
            WCall("UnspawnTravelNPC");
            int tn = 0;
            foreach (object n in (Array)SF(TMain, "npc")) if ((bool)IF(n, "active") && (int)IF(n, "type") == 368) tn++;
            P("travelnpc", tn);
            P("planterabulb", WCall("GeneratePlanteraBulbOnAllMechsDefeated"));
            PFx("wgdirect");
            P("wg_projs", ProjList());
            StateLine("wgdirect");
            Frames(3);
            Record("wgdirect", 1);
            P("wg_projs3", ProjList());
            ClearEntities();
            ClearFx();
            ClearItems();
            ClearArea(x0, x0 + 40, gy);
            Frames(2);
        }
    }
}
