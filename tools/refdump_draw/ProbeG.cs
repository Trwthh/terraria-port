// The probe run, part G (Probe.cs; port: tests/draw/probe_g.rs, the same steps): the character and world creation
// screens used (colour pickers, a random character, secret seeds typed and toggled), the world generation progress
// screen, the virtual keyboard's modes, the reports page.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    static object MouseEvt(object target) { return Activator.CreateInstance(T("Terraria.UI.UIMouseEvent"), new object[] { target, new Vector2(640f, 360f) }); }

    static void ProbeG(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        // ---- character creation: every colour picker (SelectColorPicker, Click_ColorPicker: the HSL sliders), a random
        //      character (Click_RandomizePlayer), the voice pitch helpers, the colour conversions
        if (Group("charcreate2"))
        {
            object np = Activator.CreateInstance(T("Terraria.Player"));
            Type cct = T("Terraria.GameContent.UI.States.UICharacterCreation");
            object cc = Activator.CreateInstance(cct, new object[] { np });
            Fancy(cc);
            Frames(5);
            Type cat = cct.GetNestedType("CategoryId", BindingFlags.NonPublic);
            for (int c = 0; c < 10; c++)
            {
                Unwrap(MI(cct, "SelectColorPicker", 1), cc, new object[] { Enum.ToObject(cat, c) });
                Frames(2);
                if (c % 3 == 0) Record("charcreate2_picker_" + c, 1);
            }
            Array pickers = (Array)IF(cc, "_colorPickers");
            for (int i = 0; i < pickers.Length; i++)
            {
                object p = pickers.GetValue(i);
                if (p == null) continue;
                Unwrap(MI(cct, "Click_ColorPicker", 2), cc, new object[] { MouseEvt(p), p });
                Frames(1);
            }
            // (in the world the hairstyle list is the stylist's: Main.hairWindow, HairstyleUnlocksHelper.UpdateUnlocks)
            SSet(TMain, "hairWindow", true);
            CallI(SF(TMain, "Hairstyles"), "UpdateUnlocks");
            for (int k = 0; k < 3; k++)
            {
                Unwrap(MI(cct, "Click_RandomizePlayer", 2), cc, new object[] { null, null });
                P("charcreate2_random_" + k, IF(np, "hair"), IF(np, "skinVariant"), IF(np, "hairColor"), IF(np, "eyeColor"), IF(np, "skinColor"), IF(np, "shirtColor"), IF(np, "pantsColor"), IF(np, "voiceVariant"), IF(np, "voicePitchOffset"));
                Frames(2);
            }
            Record("charcreate2_random", 1);
            SSet(TMain, "hairWindow", false);
            CallI(SF(TMain, "Hairstyles"), "UpdateUnlocks");
            foreach (float x in new[] { 0f, 0.13f, 0.5f, 0.9f })
                P("voicepitch_" + PV(x), Unwrap(MI(cct, "GetVoicePitchColorAt", 1), cc, new object[] { x }), Unwrap(MI(cct, "RemapPitchSliderKnob", 1), cc, new object[] { x }));
            foreach (Color c in new[] { new Color(255, 0, 0), new Color(12, 200, 99), new Color(240, 240, 250) })
                P("rgbhsl_" + PV(c), Unwrap(MI(cct, "RgbToScaledHsl", 1), null, new object[] { c }));
            foreach (float[] h in new[] { new[] { 0f, 1f, 0.5f }, new[] { 0.33f, 0.2f, 0.9f }, new[] { 0.95f, 0.7f, 0f } })
                P("hslrgb_" + PV(h[0]) + "_" + PV(h[1]) + "_" + PV(h[2]), Unwrap(MI(cct, "ScaledHslToRgb", 3), null, new object[] { h[0], h[1], h[2] }));
            FancyClose();
            Frames(3);
        }
        // ---- world creation: seeds typed in (OnFinishedSettingSeed: secret seeds, their particles and the rainbow
        //      ring), the secret seed description, the secret seeds list's buttons toggled (their particles, the glow ring)
        if (Group("worldcreate2"))
        {
            string ns = "Terraria.GameContent.UI.States.";
            Type wct = T(ns + "UIWorldCreation");
            object wc = Activator.CreateInstance(wct);
            Fancy(wc);
            Frames(5);
            // three secret seeds given a plain text of our own (SecretSeed._plaintext: the texts are stored only as hashes,
            // and CheckInputForSecretSeed compares the plain text first), typed in and pasted as a copied seed
            Type sst = T("Terraria.WorldGen+SecretSeed");
            FieldInfo plain = sst.GetField("_plaintext", AllInst);
            string[][] secrets = { new[] { "paintEverythingGray", "probegray" }, new[] { "extraLivingTrees", "probetrees" }, new[] { "errorWorld", "probeerror" } };
            foreach (string[] sc in secrets) plain.SetValue(SF(sst, sc[0]), sc[1]);
            foreach (string seed in new[] { "12345", "not the bees", "for the worthy", "get fixed boi", "05162020", "celebrationmk10", "the constant", "no traps", "dont dig up", "Probe seed", "probegray", "1.1.1.0.probetrees|probe seed", "probeerror" })
            {
                Unwrap(MI(wct, "OnFinishedSettingSeed", 1), wc, new object[] { seed });
                Frames(4);
                P("worldseed_" + seed.Replace(' ', '_'), IF(wc, "_optionSeed"), IF(wc, "HasEnteredSpecialSeed"));
            }
            Record("worldcreate2_seeds", 1);
            P("worldseed_joined", CallI(wc, "GetJoinedSecretSeedString", SF(T("Terraria.GameContent.FontAssets"), "MouseText").GetType().GetProperty("Value").GetValue(SF(T("Terraria.GameContent.FontAssets"), "MouseText"), null), 300f, 200f));
            object adv = Activator.CreateInstance(T(ns + "UIWorldCreationAdvanced"), new object[] { wc, false });
            Fancy(adv);
            Frames(3);
            Unwrap(MI(T(ns + "UIWorldCreationAdvanced"), "ShowSecretSeedDescription", 2), adv, new object[] { null, null });
            Frames(3);
            Record("worldcreate2_adv", 1);
            Type slt = T(ns + "UIWorldCreationAdvancedSecretSeedsList");
            object sl = Activator.CreateInstance(slt, new object[] { adv, wc });
            Fancy(sl);
            Frames(3);
            IList items = (IList)IF(IF(sl, "_worldList"), "_items");
            Note("secret seed buttons: " + items.Count);
            for (int i = 0; i < items.Count && i < 6; i++)
            {
                Unwrap(MI(slt, "ClickSecretSeed", 2), sl, new object[] { null, items[i] });
                Frames(2);
            }
            Frames(3);
            Record("worldcreate2_secret", 1);
            for (int i = 0; i < items.Count && i < 6; i++) Unwrap(MI(slt, "ClickSecretSeed", 2), sl, new object[] { null, items[i] });
            FancyClose();
            CallS("Terraria.WorldGen+SecretSeed", "ClearAllSeeds");
            foreach (string[] sc in secrets) plain.SetValue(SF(sst, sc[0]), null);
            Frames(3);
        }
        // ---- the world generation screen (UIWorldLoad: progress bar, header) with a generation in progress
        if (Group("worldload"))
        {
            Type wg = T("Terraria.WorldBuilding.WorldGenerator");
            object prog = New("Terraria.WorldBuilding.GenerationProgress");
            ISet(prog, "TotalWeight", 10.0);
            object ctl = Activator.CreateInstance(T("Terraria.WorldBuilding.WorldGenerator+Controller"), new object[] { null });
            SSet(wg, "CurrentGenerationProgress", prog);
            SSet(wg, "CurrentController", ctl);
            object wl = New("Terraria.GameContent.UI.States.UIWorldLoad");
            Fancy(wl);
            foreach (double v in new[] { 0.0, 0.35, 0.8 })
            {
                CallI(prog, "Set", v);
                PSet(prog, "Message", "Probe pass " + PV(v));
                Frames(2);
                P("worldload_" + PV(v), IF(prog, "TotalProgress"), IF(prog, "Value"));
            }
            Record("worldload", 1);
            FancyClose();
            SSet(wg, "CurrentGenerationProgress", null);
            SSet(wg, "CurrentController", null);
            Frames(3);
        }
        // ---- the virtual keyboard's modes (UIVirtualKeyboard.SetKeyState), the reports page (UIReportsPage)
        if (Group("uimisc"))
        {
            Type vk = T("Terraria.GameContent.UI.States.UIVirtualKeyboard");
            object kb = Activator.CreateInstance(vk, new object[] { "Probe", "abc", null, null, 0, false, 20 });
            Fancy(kb);
            Frames(3);
            Type ks = vk.GetNestedType("KeyState");
            foreach (int s in new[] { 2, 1, 0, 1, 2 })
            {
                CallI(kb, "SetKeyState", Enum.ToObject(ks, s));
                Frames(2);
            }
            Record("vkeyboard_modes", 1);
            FancyClose();
            Frames(2);
            Type rt = T("Terraria.GameContent.UI.States.UIReportsPage");
            Type ipr = T("Terraria.DataStructures.IProvideReports");
            object reporters = Activator.CreateInstance(typeof(List<>).MakeGenericType(ipr));
            object rp = Activator.CreateInstance(rt, new object[] { null, 0, reporters });
            Fancy(rp);
            Frames(5);
            Record("reports", 1);
            FancyClose();
            Frames(3);
        }
    }
}
