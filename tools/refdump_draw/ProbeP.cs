// The probe run, part P (Probe.cs; port: tests/draw/probe_p.rs, the same steps): the automatic door helpers called on
// a door and a tall gate, the stardew valley animation (projectile 995) through its whole length, leashed critters
// and a kite sent and received over a byte stream (with a firefly's light, a frog's jump and a recall's dust), and
// effects: the sandstorm sky and overlay, a camera modifier, the moon lord's explosions, a reflective dye, the torch
// colours of every torch, fake fish magnetised to a bobber.
using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    static void ProbeP(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        int me = (int)SF(TMain, "myPlayer");
        if (Group("doordirect"))
        {
            ClearEntities();
            PlacePlayer(x0 + 8, gy - 1);
            Frames(2);
            object helper = IF(pl, "doorHelper");
            var handlers = (IDictionary)IF(helper, "_handlerByTileType");
            for (int x = x0; x <= x0 + 30; x++) PlaceTile(x, gy - 6, 30, 0);
            PlaceTile(x0 + 10, gy - 4, 30, 0);
            PlaceTile(x0 + 10, gy - 5, 30, 0);
            P("door", PlaceTile(x0 + 10, gy - 1, 10, 0), PlaceTile(x0 + 16, gy - 1, 388, 0));
            int n = 0;
            foreach (int[] d in new[] { new[] { 10, x0 + 10 }, new[] { 388, x0 + 16 } })
            {
                object h = handlers[d[0]];
                PlacePlayer(d[1] - 2, gy - 1);
                ISet(pl, "controlRight", true);
                object info = CallI(h, "ProvideInfo", new Point(d[1], gy - 2));
                object open = CallI(helper, "GetPlayerInfoForOpeningDoor", pl);
                bool opened = (bool)CallI(h, "TryOpenDoor", info, open);
                PTiles("door_open_" + n, d[1] - 2, gy - 6, 5, 6);
                PlacePlayer(d[1] - 6, gy - 1);
                ISet(pl, "controlRight", false);
                object close = CallI(helper, "GetPlayerInfoForClosingDoor", pl);
                object closed = CallI(h, "TryCloseDoor", info, close);
                P("door_" + n, opened, closed);
                PTiles("door_closed_" + n, d[1] - 2, gy - 6, 5, 6);
                n++;
            }
            StateLine("doordirect");
            ClearArea(x0, x0 + 30, gy);
            Frames(2);
        }
        if (Group("stardew"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            int pi = NewProj(pp.X + 40f, pp.Y - 40f, 0f, 0f, 995);
            object p = ProjAt(pi);
            for (int t = 0, k = 0; t < 600; t += 15, k++)
            {
                if (!(bool)IF(p, "active")) break;
                ((float[])IF(p, "ai"))[0] = t;
                ISet(p, "timeLeft", 100);
                Frames(1);
                if (k % 8 == 0) Record("stardew_" + t, 1);
            }
            StateLine("stardew");
            ClearEntities();
            Frames(2);
        }
        if (Group("leashednet"))
        {
            ClearEntities();
            PlacePlayer(x0 + 20, gy - 1);
            bool day = (bool)SF(TMain, "dayTime");
            double time = (double)SF(TMain, "time");
            SetTime(false, 3600.0);
            int ax = x0;
            foreach (int[] e in new[] { new[] { 1992, 724 }, new[] { 2740, 724 }, new[] { 2019, 724 }, new[] { Array.IndexOf((bool[])SF(T("Terraria.ID.ItemID+Sets"), "IsAKite"), true), 723 } })
            {
                GiveItem(0, e[0]);
                SelectItem(0);
                TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { ax, gy - 1, e[1], false, 0, 0, -1, 1 });
                Point tl = TopLeftOf(e[1], ax, gy - 1);
                CallS(e[1] == 724 ? "Terraria.GameContent.Tile_Entities.TECritterAnchor" : "Terraria.GameContent.Tile_Entities.TEKiteAnchor", "Hook_AfterPlacement", tl.X, tl.Y, e[1], 0, 1, 0);
                ax += 3;
            }
            GiveItem(0, 0);
            Frames(120);
            Record("leashednet", 1);
            Type tle = T("Terraria.GameContent.LeashedEntity");
            var all = (IList)SF(tle, "ByWhoAmI");
            int k2 = 0;
            foreach (object e in all.Cast<object>().ToList())
            {
                if (e == null || !(bool)IF(e, "active")) continue;
                var ms = new MemoryStream();
                var w = new BinaryWriter(ms);
                CallI(e, "NetSend", w, true);
                w.Flush();
                byte[] bytes = ms.ToArray();
                CallI(e, "NetReceive", new BinaryReader(new MemoryStream(bytes)), true);
                string tn = e.GetType().Name;
                if (tn == "FireflyLeashedCritter") CallI(e, "AddLight");
                if (tn == "JumperLeashedCritter") P("jumper_jump", CallI(e, "TryStartJump"));
                if (e.GetType().IsSubclassOf(T("Terraria.GameContent.LeashedEntities.LeashedCritter"))) CallI(e, "Recall_CreateDustAtLocation");
                P("leashed_" + k2, IF(e, "Type"), bytes.Length, (uint)bytes.Aggregate(2166136261u, (h, b) => (h ^ b) * 16777619u), IF(e, "position"));
                k2++;
            }
            ClearArea(x0 - 1, ax + 1, gy);
            SetTime(day, time);
            Frames(30);
            StateLine("leashednet");
        }
        if (Group("effects2"))
        {
            ClearEntities();
            ClearFx();
            PlacePlayer(sx, sy);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            object skyMgr = SF(T("Terraria.Graphics.Effects.SkyManager"), "Instance");
            skyMgr.GetType().GetMethods().First(m => m.Name == "Activate" && m.GetParameters().Length == 3).Invoke(skyMgr, new object[] { "Sandstorm", pp, new object[0] });
            object overlays = SF(T("Terraria.Graphics.Effects.Overlays"), "Scene");
            overlays.GetType().GetMethods().First(m => m.Name == "Activate" && m.GetParameters().Length == 3).Invoke(overlays, new object[] { "Sandstorm", pp, new object[0] });
            object mods = IF(SF(TMain, "instance"), "CameraModifiers");
            CallI(mods, "Add", New("Terraria.Graphics.CameraModifiers.FixedMovePerFrameCameraModifier", pp, new Vector2(40f, -30f), 2f, 1.5f, "probe"));
            CallS("Terraria.GameContent.Events.MoonlordDeathDrama", "AddExplosion", new Vector2(pp.X + 80f, pp.Y - 60f));
            CallS("Terraria.GameContent.Events.MoonlordDeathDrama", "AddExplosion", new Vector2(pp.X - 80f, pp.Y - 20f));
            SetArmor(1, 3777);
            Array dyes = (Array)IF(pl, "dye");
            CallI(dyes.GetValue(1), "SetDefaults", 3026, null);
            Frames(20);
            P("dye_body", IF(pl, "cBody"), IF(dyes.GetValue(1), "dye"));
            Record("effects2", 1);
            CallI(skyMgr, "Deactivate", "Sandstorm", new object[0]);
            CallI(overlays, "Deactivate", "Sandstorm", new object[0]);
            CallI(dyes.GetValue(1), "TurnToAir");
            SetArmor(1, 0);
            int count = (short)SF(T("Terraria.ID.TorchID"), "Count");
            var cs = new List<object>();
            for (int t = 0; t < count; t++)
            {
                object[] a = { t, 0f, 0f, 0f };
                Unwrap(MI(T("Terraria.ID.TorchID"), "TorchColor", 4), null, a);
                cs.Add(PV(a[1]) + "," + PV(a[2]) + "," + PV(a[3]));
            }
            P("torchcolors", string.Join(";", cs.Select(x => (string)x).ToArray()));
            // the three torch lights that are structs of their own, called directly (through the interface the runtime
            // runs them by an unboxing stub, which the JIT dump does not see as their code)
            foreach (string tn in new[] { "DemonTorchLight", "DiscoTorchLight", "ShimmerTorchLight" })
            {
                Type lt = T("Terraria.ID.TorchID+" + tn);
                object box = Activator.CreateInstance(lt);
                object[] a = { 0f, 0f, 0f };
                lt.GetMethod("GetRGB").Invoke(box, a);
                P("torch_" + tn, a[0], a[1], a[2]);
            }
            ProbeDiag();
            Type tPO = T("Terraria.GameContent.Drawing.ParticleOrchestrator");
            object bob = ProjAt(NewProj(pp.X + 70f, pp.Y, 0f, 0f, 360));
            object pos = MakeStruct("Terraria.GameContent.Drawing.ParticleOrchestraSettings", "PositionInWorld", new Vector2(pp.X + 60f, pp.Y + 8f), "MovementVector", new Vector2(1f, 0f), "UniqueInfoPiece", 2290);
            Unwrap(MI(tPO, "Spawn_FakeFish", 2), null, new object[] { pos, false });
            CallS("Terraria.GameContent.Drawing.ParticleOrchestrator", "MagnetFakeFish", bob, 2290);
            Frames(30);
            PFx("effects2");
            StateLine("effects2");
            ClearEntities();
            ClearFx();
            Frames(10);
        }
    }
}
