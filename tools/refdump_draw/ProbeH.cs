// The probe run, part H (Probe.cs; port: tests/draw/probe_h.rs, the same steps): hair dyes, torches of every kind,
// every gore type, the legacy lighting engine, a ride on a minecart track, small direct calls (dialogue texts, input
// helpers, the gamepad lock-on).
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    /// <summary>Every active combat text: "index:position:velocity:scale:alpha:rotation:lifeTime" (diagnosis).</summary>
    static string CombatList()
    {
        var sb = new System.Text.StringBuilder();
        Array cts = (Array)SF(TMain, "combatText");
        for (int i = 0; i < cts.Length; i++)
        {
            object c = cts.GetValue(i);
            if (!(bool)IF(c, "active")) continue;
            sb.Append(i + ":" + PV(IF(c, "position")) + ":" + PV(IF(c, "velocity")) + ":" + PV(IF(c, "scale")) + ":" + PV(IF(c, "alpha")) + ":" + PV(IF(c, "rotation")) + ":" + IF(c, "lifeTime") + ";");
        }
        return sb.ToString();
    }
    static void ProbeH(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        // ---- every hair dye worn (GameShaders.Hair: the legacy hair dye colour functions)
        if (Group("hairdye"))
        {
            PlacePlayer(sx, sy);
            object it = Activator.CreateInstance(T("Terraria.Item"));
            MethodInfo sd = ItemSetDefaults;
            int count = (short)SF(T("Terraria.ID.ItemID"), "Count");
            var dyes = new List<int>();
            for (int t = 1; t < count; t++)
            {
                sd.Invoke(it, new object[] { t, null });
                if ((short)IF(it, "hairDye") >= 0) dyes.Add((short)IF(it, "hairDye"));
            }
            Note("hair dyes: " + string.Join(",", dyes.Select(d => d.ToString()).ToArray()));
            for (int k = 0; k < dyes.Count; k++)
            {
                ISet(pl, "hairDye", (byte)dyes[k]);
                ISet(pl, "statLife", 20 + k * 7);
                Frames(2);
                if (k % 4 == 0) Record("hairdye_" + k, 1);
            }
            ISet(pl, "hairDye", (byte)0);
            ISet(pl, "statLife", 100);
            Frames(2);
        }
        // ---- every torch at night (TorchID: the lights of the demon, shimmer and disco torches)
        if (Group("torches"))
        {
            SetTime(false, 3600.0);
            PlacePlayer(x0 + 20, gy - 1);
            int n = (short)SF(T("Terraria.ID.TorchID"), "Count");
            for (int k = 0; k < n; k++) PlaceTile(x0 + 2 + k * 2, gy - 1, 4, k);
            Frames(30);
            Record("torches", 1);
            ClearArea(x0, x0 + 2 + n * 2, gy);
            SetTime(true, 27000.0);
            Frames(3);
        }
        // ---- every gore type, sixty at a time, flying up next to the player (Gore.Update's special behaviours: sails,
        //      fog machine clouds, lightning bunny sparks, chum in water)
        if (Group("gores"))
        {
            PlacePlayer(sx, sy);
            var pp = (Vector2)IF(pl, "position");
            int count = (int)SF(T("Terraria.ID.GoreID"), "Count");
            MethodInfo ng = T("Terraria.Gore").GetMethods(AllStatic).First(m => m.Name == "NewGore" && m.GetParameters().Length == 4);
            for (int b = 1; b < count; b += 60)
            {
                ClearFx();
                for (int t = b; t < b + 60 && t < count; t++)
                    ng.Invoke(null, new object[] { new Vector2(pp.X - 300f + (t - b) * 10f, pp.Y - 100f), new Vector2(((t % 7) - 3) * 0.5f, -2f - (t % 3)), t, 1f });
                Frames(15);
                PFx("gores_" + b);
                if ((b / 60) % 4 == 0) Record("gores_" + b, 1);
            }
            ClearFx();
            Frames(2);
        }
        // ---- the legacy lighting engine: cleared (LegacyLighting.Clear) and following the sky colour through dawn
        //      (UpdateLightToSkyColor), in the white, retro and trippy modes
        if (Group("legacylight"))
        {
            Type lm = T("Terraria.Graphics.Light.LightMode");
            PropertyInfo mode = T("Terraria.Lighting").GetProperty("Mode", AllStatic);
            foreach (int md in new[] { 0, 1, 2 })
            {
                mode.SetValue(null, Enum.ToObject(lm, md), null);
                CallS("Terraria.Lighting", "Clear");
                SetTime(false, 31000.0);
                for (int k = 0; k < 4; k++) { Frames(10); SSet(TMain, "time", 31000.0 + k * 300.0); }
                SetTime(true, 0.0);
                Frames(10);
                Record("legacylight_" + md, 1);
            }
            mode.SetValue(null, Enum.ToObject(lm, 3), null);
            SetTime(true, 27000.0);
            Frames(10);
        }
        // ---- a minecart ride: a long track with a booster and a bumper, every minecart (Minecart.TrackRotation,
        //      WheelSparks, HitTrackSwitch, the carts' sparks, sounds and the mechanical cart's laser)
        if (Group("minecart"))
        {
            for (int x = x0 - 2; x <= x0 + BuildW; x++) PlaceTile(x, gy - 1, 314, 0);
            Frames(3);
            int mc = (int)SF(T("Terraria.ID.MountID"), "Count");
            object mt = Activator.CreateInstance(T("Terraria.Mount"));
            var carts = new List<int>();
            bool[] isCart = (bool[])SF(T("Terraria.ID.MountID+Sets"), "Cart");
            for (int k = 0; k < mc; k++) if (isCart[k]) carts.Add(k);
            Note("carts: " + string.Join(",", carts.Select(c => c.ToString()).ToArray()));
            foreach (int cart in carts)
            {
                ClearEntities();
                PlacePlayer(x0 + 2, gy - 2);
                object mount = IF(pl, "mount");
                mount.GetType().GetMethod("SetMount").Invoke(mount, new object[] { cart, pl });
                ISet(pl, "velocity", V2(12f, 0f));
                InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.D };
                Frames(20);
                InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.A };
                Frames(10);
                InKeys = new[] { Microsoft.Xna.Framework.Input.Keys.Space };
                Frames(10);
                InKeys = null;
                Frames(10);
                P("cart_" + cart, IF(pl, "position"), IF(pl, "velocity"), IF(pl, "fullRotation"));
                P("cart_ct_" + cart, CombatList());
                if (cart % 3 == 0) Record("cart_" + cart, 1);
                mount = IF(pl, "mount");
                mount.GetType().GetMethods().First(m => m.Name == "Dismount").Invoke(mount, new object[] { pl, false });
                Frames(3);
            }
            ClearArea(x0 - 2, x0 + BuildW, gy);
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(10);
        }
        // ---- small direct calls: the dryad's world status, the world's seed text, gamepad slider input, the
        //      lighting of a point, an alpha-darkened colour, the gamepad lock-on, the D-pad snap, the smart select pointer
        //      (the input profiles: PlayerInputProfile.Load at start-up, from the "input profiles.json" the probe run puts in
        //      the save folder, RefDumpDraw.cs Run)
        if (Group("direct2"))
        {
            object[] a = { false };
            P("dryadstatus", Unwrap(MI(T("Terraria.Lang"), "GetDryadWorldStatusDialog", 1), null, a), a[0]);
            object wfd = SF(TMain, "ActiveWorldFileData");
            P("seedtext", CallI(wfd, "GetFullSeedText", false), CallI(wfd, "GetFullSeedText", true));
            foreach (float v in new[] { 0f, 0.3f, 0.99f })
                P("slider_" + PV(v), CallS("Terraria.Initializers.UILinksInitializer", "HandleSliderHorizontalInput", v, 0f, 1f, 0.2f, 0.5f), CallS("Terraria.Initializers.UILinksInitializer", "HandleSliderVerticalInput", v, 0f, 1f, 0.2f, 0.5f));
            var pp = (Vector2)IF(pl, "position");
            foreach (Vector2 v in new[] { pp, new Vector2(pp.X + 37.5f, pp.Y - 19.25f), new Vector2(31999.7f, 9000.3f) })
                P("sublight_" + PV(v), CallS("Terraria.Lighting", "GetSubLight", v));
            P("alphadarken", CallS("Terraria.ID.Colors", "AlphaDarken", new Color(200, 100, 50, 255)));
            // the gamepad lock-on: zombies near the cursor, targets refreshed, focus cycled, the predicted position
            ClearEntities();
            Type lo = T("Terraria.GameInput.LockOnHelper");
            for (int k = 0; k < 3; k++) NewNpc((int)pp.X + 100 + k * 60, (int)pp.Y, 3);
            Frames(2);
            Unwrap(MI(lo, "RefreshTargets", 2), null, new object[] { new Vector2(pp.X + 100f, pp.Y), 2000f });
            IList targets = (IList)SF(lo, "_targets");
            Note("lock-on targets " + targets.Count);
            for (int k = 0; k < 3; k++)
            {
                Unwrap(MI(lo, "CycleTargetFocus", 0), null, new object[0]);
                P("lockon_" + k, SF(lo, "_pickedTarget"), SF(lo, "PredictedPosition"));
            }
            ClearEntities();
            Unwrap(MI(T("Terraria.GameInput.PlayerInput"), "HandleDpadSnap", 0), null, new object[0]);
            P("dpadsnap", SF(TMain, "mouseX"), SF(TMain, "mouseY"));
            object ssp = SF(T("Terraria.GameInput.PlayerInput"), "smartSelectPointer");
            object[] st = { pl, 0, 0 };
            Unwrap(MI(ssp.GetType(), "SmartSelectLookup_GetTargetTile", 3), ssp, st);
            P("smartselect", st[1], st[2]);
            Frames(2);
        }
    }
}
