// The probe run, part B (Probe.cs; port: tests/draw/probe_b.rs, the same steps): audio helpers, chests, collision,
// the delegate methods (lasers, the dry bomb, pet previews, minecarts, mounts), dust boxes.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Runtime.Serialization;
using Microsoft.Xna.Framework;

public static partial class WG
{
    static object NewDustAt(float x, float y, int type)
    {
        MethodInfo nd = T("Terraria.Dust").GetMethods(AllStatic).First(m => m.Name == "NewDust" && m.GetParameters().Length == 9);
        int i = (int)nd.Invoke(null, new object[] { V2(x, y), 10, 10, type, 0f, 0f, 0, default(Color), 1f });
        return ((Array)SF(TMain, "dust")).GetValue(i);
    }

    static void ProbeB(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        var pp = (Vector2)IF(pl, "position");
        // ---- audio: ASoundEffectBasedAudioTrack.ReMapVolumeToMatchXact (a music track's volume curve; the method reads
        //      no field, called on a track object without its stream), OGGAudioTrack.ApplyTemporaryBufferTo (float
        //      samples to 16-bit PCM), LegacyAudioSystem.PauseAll (the window losing focus)
        if (Group("audio"))
        {
            object track = FormatterServices.GetUninitializedObject(T("Terraria.Audio.OGGAudioTrack"));
            MethodInfo remap = MI(T("Terraria.Audio.ASoundEffectBasedAudioTrack"), "ReMapVolumeToMatchXact", 1);
            foreach (float v in new[] { 0f, 0.1f, 0.25f, 0.5f, 0.75f, 0.9f, 1f })
                P("remap", v, remap.Invoke(track, new object[] { v }));
            float[] samples = { 0.5f, -0.25f, 1f, -1f, 0.123456f, 0.999f, -0.0001f, 0.33333334f };
            byte[] pcm = new byte[samples.Length * 2];
            CallS("Terraria.Audio.OGGAudioTrack", "ApplyTemporaryBufferTo", samples, pcm);
            P("pcm", pcm.Select(b => (object)b).ToArray());
            object aus = SF(TMain, "audioSystem");
            Note("audio system: " + aus.GetType().Name);
            CallI(aus, "PauseAll");
            CallI(aus, "ResumeAll");
            Frames(2);
        }
        // ---- chests: a gold chest locked and unlocked again (Chest.Lock / Unlock), a blocked chest indicated
        //      (Chest.IndicateBlockedChest), coins flying into a chest (Chest.VisualizeChestTransfer_CoinsBatch)
        if (Group("chest"))
        {
            int cx = x0 + 6;
            Note("chest: " + PlaceTile(cx, gy - 1, 21, 1));
            PTiles("chest_placed", cx - 1, gy - 3, 4, 4);
            P("chest_lock", CallS("Terraria.Chest", "Lock", cx, gy - 2));
            PTiles("chest_locked", cx - 1, gy - 3, 4, 4);
            P("chest_unlock", CallS("Terraria.Chest", "Unlock", cx, gy - 2));
            PTiles("chest_unlocked", cx - 1, gy - 3, 4, 4);
            int ci = (int)CallS("Terraria.Chest", "FindChest", cx, gy - 2);
            Note("chest index " + ci);
            PlacePlayer(cx - 4, gy - 1);
            Frames(2);
            CallS("Terraria.Chest", "IndicateBlockedChest", ci);
            object settings = SF(T("Terraria.Chest+ItemTransferVisualizationSettings"), "PlayerToChest");
            CallS("Terraria.Chest", "VisualizeChestTransfer_CoinsBatch", V2(pp.X, pp.Y), V2(cx * 16f + 16f, gy * 16f - 16f), 1234567L, settings);
            Frames(10);
            Record("chest_fx", 1);
            Frames(60);
            KillTile(cx, gy - 1);
            Frames(3);
        }
        // ---- Collision.HitTilesInACircle (the hit effect on every tile a round projectile touches), at the ground
        if (Group("collision"))
        {
            ClearFx();
            PlacePlayer(x0 + 20, gy - 1);
            CallS("Terraria.Collision", "HitTilesInACircle", V2((x0 + 10) * 16f, gy * 16f - 20f), V2(3f, 6f), 48, 48);
            PFx("hitcircle");
            Frames(5);
            Record("hitcircle", 1);
        }
        // ---- the delegate methods: the rainbow and turret laser segments, the dry bomb's spread, pet previews (the
        //      character list's pets), the minecarts' sparks and sounds, the bat / pixie mount dash dust, the wolf /
        //      velociraptor mouth positions
        if (Group("delegates"))
        {
            Type dm = T("Terraria.DelegateMethods");
            foreach (string laser in new[] { "RainbowLaserDraw", "TurretLaserDraw" })
                for (int stage = 0; stage < 3; stage++)
                {
                    object[] a = { stage, V2(100f, 200f), 37.5f, new Rectangle(0, 0, 26, 22), 0f, default(Rectangle), default(Vector2), default(Color) };
                    Unwrap(MI(dm, laser, 8), null, a);
                    P("laser_" + laser + "_" + stage, a[4], a[5], a[6], a[7]);
                }
            // the dry bomb: a pool of water right of the build area, DelegateMethods.v2_1 / f_1 as Projectile.Kill sets them
            int wx = x0 + 30;
            for (int x = wx; x < wx + 6; x++)
            {
                KillTile(x, gy); KillTile(x, gy + 1);
                foreach (int y in new[] { gy, gy + 1 })
                {
                    object t = TileAt(x, y);
                    ISet(t, "liquid", (byte)255);
                    t.GetType().GetMethod("liquidType", new[] { typeof(int) }).Invoke(t, new object[] { 0 });
                }
            }
            SSet(dm, "v2_1", V2(wx + 3, gy));
            SSet(dm, "f_1", 3.5f);
            ClearFx();
            for (int x = wx - 1; x < wx + 8; x++)
                for (int y = gy - 1; y <= gy + 2; y++)
                    P("spreaddry_" + x + "_" + y, CallS("Terraria.DelegateMethods", "SpreadDry", x, y));
            PTiles("spreaddry", wx - 1, gy - 1, 9, 4);
            PFx("spreaddry");
            for (int x = wx; x < wx + 6; x++) { PlaceTile(x, gy, 0, 0); PlaceTile(x, gy + 1, 0, 0); }
            // pet previews: each preview delegate on a projectile of a type that uses it
            Type cp = T("Terraria.DelegateMethods+CharacterPreview");
            MethodInfo psd = T("Terraria.Projectile").GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 1);
            foreach (object[] e in new[] { new object[] { "WormPet", 883 }, new object[] { "WormPet", 887 }, new object[] { "WormPet", 893 }, new object[] { "Float", 175 },
                new object[] { "FloatAndSpinWhenWalking", 197 }, new object[] { "SlimePet", 881 }, new object[] { "BerniePet", 956 }, new object[] { "EtsyPet", 764 },
                new object[] { "CompanionCubePet", 653 }, new object[] { "SpinWhenWalking", 1056 } })
            {
                foreach (bool walking in new[] { false, true })
                {
                    object pr = Activator.CreateInstance(T("Terraria.Projectile"));
                    psd.Invoke(pr, new object[] { (int)e[1] });
                    ISet(pr, "position", V2(300f, 400f));
                    Unwrap(MI(cp, (string)e[0], 2), null, new object[] { pr, walking });
                    var old = (Vector2[])IF(pr, "oldPos");
                    var ol = new List<object> { IF(pr, "position"), IF(pr, "rotation"), IF(pr, "frame"), IF(pr, "spriteDirection") };
                    ol.AddRange(old.Select(v => (object)v));
                    P("preview_" + e[0] + "_" + e[1] + "_" + walking, ol.ToArray());
                }
            }
            // minecart effects at the player
            Type mc = T("Terraria.DelegateMethods+Minecart");
            PlacePlayer(x0 + 20, gy - 1);
            ClearFx();
            foreach (string s in new[] { "Sparks", "SparksFart", "SparksMech", "SparksMeow", "SparksTerraFart" })
            {
                for (int k = 0; k < 6; k++) Unwrap(MI(mc, s, 1), null, new object[] { V2(pp.X + 8f * k, pp.Y + 30f) });
                PFx("minecart_" + s);
            }
            foreach (string s in new[] { "LandingSound", "BumperSound", "LandingSoundFart", "BumperSoundFart", "JumpingSoundFart" })
            {
                ISet(pl, "fartKartCloudDelay", 0);
                Unwrap(MI(mc, s, 4), null, new object[] { pl, V2(pp.X, pp.Y), 20, 42 });
                PFx("minecart_" + s);
            }
            ISet(pl, "fartKartCloudDelay", 0);
            Unwrap(MI(mc, "SpawnFartCloud", 5), null, new object[] { pl, V2(pp.X, pp.Y), 20, 42, true });
            PFx("minecart_fartcloud");
            Frames(3);
            Record("minecart_fx", 1);
            // mount delegates
            Type md = T("Terraria.DelegateMethods+Mount");
            foreach (string s in new[] { "BatDashDust", "PixieDashDust" })
                for (int k = 0; k < 2; k++)
                {
                    object d = NewDustAt(pp.X, pp.Y, 6);
                    object back = Unwrap(MI(md, s, 3), null, new object[] { pl, k, d });
                    P("dashdust_" + s + "_" + k, back == d, IF(d, "active"), IF(d, "position"), IF(d, "scale"));
                }
            foreach (string s in new[] { "WolfMouthPosition", "VelociraptorMouthPosition" })
            {
                object[] a = { pl, null };
                object ok = Unwrap(MI(md, s, 2), null, a);
                P("mouth_" + s, ok, a[1]);
            }
            Frames(3);
            ClearFx();
        }
        // ---- Dust.QuickBox (a box of dust: the walker's path, StrayMethods.SendWalker), Entity.AngleFrom
        if (Group("dustbox"))
        {
            ClearFx();
            CallS("Terraria.Dust", "QuickBox", V2(pp.X - 40f, pp.Y - 60f), V2(pp.X + 40f, pp.Y + 10f), 6, new Color(255, 120, 40), 1.5f, 0.8f, null);
            PFx("quickbox");
            Frames(2);
            Record("quickbox", 1);
            foreach (Vector2 src in new[] { new Vector2(0f, 0f), new Vector2(pp.X + 100f, pp.Y - 33f), new Vector2(pp.X - 7f, pp.Y + 250f) })
                P("anglefrom", src, Call(pl, T("Terraria.Entity"), "AngleFrom", src));
        }
    }
}
