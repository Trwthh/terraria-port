// The gallery run (`RefDumpDraw.exe <dir> gallery`, refdata\gallery; port: tests/draw.rs DRAW_MODE=gallery,
// tests/draw/gallery.rs, the same steps): the float methods the draw / audio / scenes runs left without a reference
// (tools\cs2rs\FLOAT_COVERAGE.md), by area: the old background drawing (Main.BackgroundEnabled off), every particle
// of ParticleOrchestrator, every ambient sky entity, EVERY projectile type and EVERY NPC type drawn next to the
// player, world events (a meteor, a shaken tree, the hardmode conversion, an altar smashed, the lunar pillars) and the
// local player's death and respawn. Scenes recorded 1 - 2 frames each, every frame's digest, and per scene the state
// line of the scenes run (StateLine).
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;

public static partial class WG
{
    static bool GalleryMode;
    /// <summary>REFDRAW_GALLERY_SKIP=bg,particles,ambient,proj,npc (diagnosis of a later part only; the port's test has
    /// DRAW_GALLERY_SKIP, the same parts).</summary>
    static bool GallerySkip(string part)
    {
        string v = Environment.GetEnvironmentVariable("REFDRAW_GALLERY_SKIP");
        return !string.IsNullOrEmpty(v) && Array.IndexOf(v.Split(','), part) >= 0;
    }

    /// <summary>ProjectileID.Count / NPCID.Count of 1.4.5.8 (checked against the game).</summary>
    const int ProjCount = 1136, NpcCount = 697;
    /// <summary>Types per batch of the projectile and NPC galleries.</summary>
    const int ProjBatch = 30, NpcBatch = 20;

    /// <summary>The local player cannot be hurt for the next ticks (a gallery batch has hostile things in it).</summary>
    static void Immune()
    {
        object pl = LocalPlayer;
        ISet(pl, "immune", true);
        ISet(pl, "immuneTime", 100000);
    }

    static object V2Of(float x, float y) { return V2(x, y); }

    /// <summary>ParticleOrchestrator.SpawnParticlesDirect(type, settings) as the game's RequestParticleSpawn(clientOnly:
    /// true, ..) does it in single player (the invoking player = Main.myPlayer).</summary>
    static void SpawnParticle(int type, float x, float y, float vx, float vy, int unique)
    {
        Type po = T("Terraria.GameContent.Drawing.ParticleOrchestrator");
        Type st = T("Terraria.GameContent.Drawing.ParticleOrchestraSettings");
        Type pt = T("Terraria.GameContent.Drawing.ParticleOrchestraType");
        object s = Activator.CreateInstance(st);
        st.GetField("PositionInWorld").SetValue(s, V2(x, y));
        st.GetField("MovementVector").SetValue(s, V2(vx, vy));
        st.GetField("UniqueInfoPiece").SetValue(s, unique);
        MethodInfo req = po.GetMethod("RequestParticleSpawn", AllStatic);
        req.Invoke(null, new object[] { true, Enum.ToObject(pt, (byte)type), s, null });
    }

    static void GalleryWorld()
    {
        int sx = (int)SF(TMain, "spawnTileX"), sy = (int)SF(TMain, "spawnTileY");
        object pl = LocalPlayer;
        if ((short)SF(T("Terraria.ID.ProjectileID"), "Count") != ProjCount || (short)SF(T("Terraria.ID.NPCID"), "Count") != NpcCount) throw new Exception("ProjectileID.Count / NPCID.Count differ from the gallery's");
        SetTime(true, 27000.0);
        PlacePlayer(sx, sy);
        Frames(30);
        // ---- 1. the old background drawing: Main.BackgroundEnabled off (the setting "Background: Off";
        //         Main.DrawBackground calls OldDrawBackground then)
        if (!GallerySkip("bg"))
        {
        SSet(TMain, "BackgroundEnabled", false);
        foreach (object[] place in new[] { new object[] { "day", sx, sy, true }, new object[] { "night", sx, sy, false }, new object[] { "cave", 1999, 547, true },
                                           new object[] { "desert", 1720, 352, true }, new object[] { "snow", 2499, 298, true }, new object[] { "crimson", 1408, 471, true },
                                           new object[] { "jungle", 309, 340, true }, new object[] { "ocean", 150, 249, true }, new object[] { "underworld", 480, 1009, true } })
        {
            SetTime((bool)place[3], (bool)place[3] ? 27000.0 : 16200.0);
            PlacePlayer((int)place[1], (int)place[2]);
            Frames(30);
            Record("bgoff_" + place[0], 2);
        }
        SSet(TMain, "BackgroundEnabled", true);
        }
        SetTime(true, 27000.0);
        PlacePlayer(sx, sy);
        Frames(30);
        // ---- 2. every particle of ParticleOrchestrator (ParticleOrchestraType 0 .. Count - 1), 8 per batch, around the player
        if (!GallerySkip("particles"))
        {
            int count = (int)(byte)Enum.Parse(T("Terraria.GameContent.Drawing.ParticleOrchestraType"), "Count");
            for (int b = 0; b * 8 < count; b++)
            {
                var c = (Microsoft.Xna.Framework.Vector2)IF(pl, "position");
                for (int k = 0; k < 8 && b * 8 + k < count; k++)
                {
                    int t = b * 8 + k;
                    SpawnParticle(t, c.X - 280f + 70f * k, c.Y - 60f, 2f, -1.5f, ParticleUnique(t));
                }
                Frames(12);
                Record("particles_" + b, 1);
                Frames(20);
            }
        }
        // ---- 3. every ambient sky entity (AmbientSky.Spawn, as NetAmbienceModule does for the AmbienceServer's choice)
        if (!GallerySkip("ambient"))
        {
            object sky = SkyByName("Ambience");
            Type et = T("Terraria.GameContent.Ambience.SkyEntityType");
            MethodInfo spawn = sky.GetType().GetMethod("Spawn");
            int n = Enum.GetValues(et).Length;
            for (int t = 0; t < n; t++)
            {
                // night for the night creatures (bats, hellbats, eyes, ..), day for the rest; the sky entity reads the time
                bool night = t == 4 || t == 6 || t == 7 || t == 16 || t == 17 || t == 18;
                SetTime(!night, night ? 16200.0 : 27000.0);
                spawn.Invoke(sky, new object[] { pl, Enum.ToObject(et, (byte)t), 1000 + t });
                Frames(60);
                Record("ambient_" + Enum.GetName(et, (byte)t).ToLowerInvariant(), 1);
            }
            SetTime(true, 27000.0);
            Frames(30);
        }
        // ---- 4. every projectile type, ProjBatch per batch, on a grid around the player (owner: the local player)
        for (int b = 0; 1 + b * ProjBatch < ProjCount && !GallerySkip("proj"); b++)
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Immune();
            var c = (Microsoft.Xna.Framework.Vector2)IF(pl, "position");
            for (int k = 0; k < ProjBatch; k++)
            {
                int type = 1 + b * ProjBatch + k;
                if (type >= ProjCount) break;
                NewProj(c.X - 450f + 90f * (k % 10), c.Y - 180f + 70f * (k / 10), 1.5f, -0.5f, type);
            }
            Frames(8);
            Record("proj_" + b, 1);
        }
        ClearEntities();
        PlacePlayer(sx, sy);
        Frames(20);
        // ---- 5. every NPC type, NpcBatch per batch, on a grid around the player
        for (int b = 0; 1 + b * NpcBatch < NpcCount && !GallerySkip("npc"); b++)
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Immune();
            var c = (Microsoft.Xna.Framework.Vector2)IF(pl, "position");
            for (int k = 0; k < NpcBatch; k++)
            {
                int type = 1 + b * NpcBatch + k;
                if (type >= NpcCount) break;
                NewNpc((int)(c.X - 500f + 100f * (k % 10)), (int)(c.Y - 160f + 120f * (k / 10)), type);
            }
            Frames(8);
            Record("npc_" + b, 1);
        }
        ClearEntities();
        SetTime(true, 27000.0);
        PlacePlayer(sx, sy);
        Frames(30);
        GalleryWorldEvents();
        ClearEntities();
        Frames(10);
        Record("gallery_end", 2);
    }

    /// <summary>A UniqueInfoPiece that the particle's spawner reads as an item / NPC / projectile type or a colour.</summary>
    static int ParticleUnique(int t)
    {
        // ItemTransfer: the item type; PetExchange / TownSlimeTransform / ShimmerTownNPC(Send): an NPC type; the rest: 0
        string name = Enum.GetName(T("Terraria.GameContent.Drawing.ParticleOrchestraType"), (byte)t);
        switch (name)
        {
            case "ItemTransfer": return 4;
            case "PetExchange": return 0;
            case "TownSlimeTransform": return 0;
            default: return 0;
        }
    }

    static object SkyByName(string name)
    {
        object mgr = SF(T("Terraria.Graphics.Effects.SkyManager"), "Instance");
        PropertyInfo item = mgr.GetType().GetProperties().First(p => p.GetIndexParameters().Length == 1 && p.GetIndexParameters()[0].ParameterType == typeof(string));
        return item.GetValue(mgr, new object[] { name });
    }

    /// <summary>World events: a meteor (WorldGen.meteor at a fixed spot), a tree shaken (WorldGen.ShakeTree), the
    /// hardmode conversion (WorldGen.initializeHardMode on this thread: StartHardmode runs it on a pool thread,
    /// WorldGen.cs:31907-31944, whose timing against the frames is not repeatable), a demon altar smashed
    /// (WorldGen.SmashAltar), the lunar pillars (WorldGen.TriggerLunarApocalypse), the local player's death and
    /// respawn (Player.KillMe; Player.UpdateDead until the respawn).</summary>
    static void GalleryWorldEvents()
    {
        int sx = (int)SF(TMain, "spawnTileX"), sy = (int)SF(TMain, "spawnTileY");
        object pl = LocalPlayer;
        // a meteor 60 tiles right of the spawn, on the surface (ignorePlayers: the spawn is near)
        CallStatic(TWorldGen, "meteor", sx + 60, sy, true);
        PlacePlayer(sx + 60, sy - 20);
        Frames(30);
        Record("event_meteor", 2);
        // the nearest tree to the right of the spawn, shaken
        int tx = -1, ty = -1;
        Snap snap = TakeSnapshot(null);
        Func<int, int, int> at = (x, y) => (x * snap.H + y) * RecSize;
        Func<int, int, bool> act = (x, y) => (snap.Buf[at(x, y) + 5] & 0x20) != 0;
        Func<int, int, int> type = (x, y) => snap.Buf[at(x, y)] | (snap.Buf[at(x, y) + 1] << 8);
        Func<int, int, int> frameY = (x, y) => (short)(snap.Buf[at(x, y) + 12] | (snap.Buf[at(x, y) + 13] << 8));
        for (int x = sx + 2; x < sx + 400 && tx < 0; x++)
            for (int y = sy - 60; y < sy + 20 && tx < 0; y++)
                if (act(x, y) && type(x, y) == 5 && frameY(x, y) < 198) { tx = x; ty = y; }
        Note("tree at " + tx + "," + ty);
        PlacePlayer(tx - 3, sy);
        CallStatic(TWorldGen, "ShakeTree", tx, ty);
        Frames(30);
        Record("event_shaketree", 2);
        // the hardmode conversion
        SSet(TMain, "hardMode", true);
        CallStatic(TWorldGen, "initializeHardMode");
        PlacePlayer(sx, sy);
        Frames(30);
        Record("event_hardmode", 2);
        // a demon altar: the first one of the world (tile 26), smashed with the player next to it
        int ax = -1, ay = -1;
        int mx = (int)SF(TMain, "maxTilesX"), my = (int)SF(TMain, "maxTilesY");
        snap = TakeSnapshot(snap);
        for (int x = 50; x < mx - 50 && ax < 0; x++)
            for (int y = 50; y < my - 50 && ax < 0; y++)
                if (act(x, y) && type(x, y) == 26) { ax = x; ay = y; }
        Note("altar at " + ax + "," + ay);
        PlacePlayer(ax, ay);
        Frames(20);
        CallStatic(TWorldGen, "SmashAltar", ax, ay);
        Frames(30);
        Record("event_altar", 2);
        PlacePlayer(sx, sy);
        Frames(20);
        CallStatic(TWorldGen, "TriggerLunarApocalypse");
        Frames(60);
        Record("event_pillars", 2);
        ClearEntities();
        // the local player dies (KillMe) and respawns (UpdateDead counts respawnTimer down, then Spawn)
        SSet(TMain, "hardMode", false);
        ISet(pl, "immune", false);
        ISet(pl, "immuneTime", 0);
        Type pdr = T("Terraria.DataStructures.PlayerDeathReason");
        object reason = pdr.GetMethod("ByCustomReason", AllStatic).Invoke(null, new object[] { "the gallery" });
        pl.GetType().GetMethod("KillMe").Invoke(pl, new object[] { reason, 1000.0, 1, false });
        Frames(30);
        Record("player_dead", 2);
        for (int i = 0; i < 1200 && (bool)IF(pl, "dead"); i++) Frames(1);
        Note("respawned: dead " + IF(pl, "dead") + " life " + IF(pl, "statLife"));
        Frames(30);
        Record("player_respawn", 2);
    }

    /// <summary>Main.tile[x, y] (a Tile[,] of objects; never null in a loaded world).</summary>
    static object TileAt(int x, int y) { return ((Array)SF(TMain, "tile")).GetValue(x, y); }
}
