// The probe run, part O (Probe.cs; port: tests/draw/probe_o.rs, the same steps): the new crafting window, journey
// mode's powers menu (the infinite items display, research with its cogs), and UI elements drawn outside the frame
// (a hairstyle button, an icon text button, an image with a border, a progress bar, a vertical slider, a scrollbar
// pressed, the net diagnostics, the defender medals' savings and price text), the minimap's zoom buttons, an emote
// bubble's biome probe, the banner and crafting grids scrolled, the virtual keyboard.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    /// <summary>`e` placed at (x, y) with size (w, h) and recalculated (UIElement.Left / Top / Width / Height).</summary>
    static void UIPlace(object e, float x, float y, float w, float h)
    {
        Type sd = T("Terraria.UI.StyleDimension");
        Func<float, object> px = v => Activator.CreateInstance(sd, new object[] { v, 0f });
        PSet(e, "Left", px(x)); PSet(e, "Top", px(y)); PSet(e, "Width", px(w)); PSet(e, "Height", px(h));
        CallI(e, "Recalculate");
    }

    static void ProbeO(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        if (Group("uicrafting"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            SetInv(10, 9, 99);
            SetInv(11, 3, 99);
            Unwrap(MI(T("Terraria.GameContent.UI.NewCraftingUI"), "Open", 2), null, new object[] { true, null });
            Frames(4);
            Record("newcrafting", 1);
            Unwrap(MI(T("Terraria.GameContent.UI.NewCraftingUI"), "Close", 2), null, new object[] { true, false });
            CallI(SF(TMain, "craftingUI"), "ScrollCraftingGrid", -1, 5);
            CallI(SF(TMain, "craftingUI"), "ScrollCraftingGrid", 1, 5);
            object banner = SF(TMain, "bannerUI");
            CallI(banner, "ScrollCraftingGrid", -1, 5);
            CallI(banner, "NavigatePipsList", 1);
            CallI(banner, "NavigatePipsGrid", 1, 1);
            SSet(TMain, "playerInventory", false);
            GiveItem(10, 0);
            GiveItem(11, 0);
            Frames(2);
            StateLine("uicrafting");
        }
        if (Group("uicreative"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            byte diff = (byte)IF(pl, "difficulty");
            ISet(pl, "difficulty", (byte)3);
            SSet(TMain, "playerInventory", true);
            object cm = SF(TMain, "CreativeMenu");
            CallI(cm, "ToggleMenu");
            Frames(3);
            object st = IF(cm, "_uiState");
            CallI(st, "ToggleMainCategory", 1);
            Frames(3);
            Record("creative_items", 1);
            CallI(st, "ToggleMainCategory", 2);
            Frames(2);
            object it = NewItemOf(9);
            ISet(it, "stack", 99);
            CallI(cm, "SetItembyIndex", it, 0);
            CallI(cm, "SacrificeItemInSacrificeSlot");
            Frames(6);
            Record("creative_research", 1);
            CallI(cm, "ToggleMenu");
            SSet(TMain, "playerInventory", false);
            ISet(pl, "difficulty", diff);
            ClearItems();
            for (int k = 0; k < 58; k++) GiveItem(k, 0);
            Frames(2);
            StateLine("uicreative");
        }
        if (Group("uielements"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            var hair = New("Terraria.GameContent.UI.Elements.UIHairStyleButton", pl, 5);
            UIPlace(hair, 100f, 100f, 44f, 44f);
            var icon = New("Terraria.GameContent.UI.Elements.UIIconTextButton", Unwrap(MI(T("Terraria.Localization.Language"), "GetText", 1), null, new object[] { "UI.Back" }), Color.White, "Images/UI/Workshop/Publish", 1f, 0.5f, 10f);
            UIPlace(icon, 160f, 100f, 200f, 40f);
            CallI(icon, "SetText", Unwrap(MI(T("Terraria.Localization.Language"), "GetText", 1), null, new object[] { "UI.Cancel" }), 0.9f, Color.Yellow);
            Type ta = T("Terraria.GameContent.TextureAssets");
            object border = T("Terraria.GameContent.UI.Elements.UIImageWithBorder").GetConstructors().First(c => c.GetParameters()[0].ParameterType.Name.StartsWith("Asset")).Invoke(new object[] { SF(ta, "InventoryBack"), SF(ta, "InventoryBack2") });
            UIPlace(border, 380f, 100f, 40f, 40f);
            object bar = New("Terraria.GameContent.UI.Elements.UIProgressBar");
            UIPlace(bar, 100f, 160f, 300f, 20f);
            CallI(bar, "SetProgress", 0.37f);
            float sv = 0.6f;
            object slider = New("Terraria.GameContent.UI.Elements.UIVerticalSlider", (Func<float>)(() => sv), (Action<float>)(f => sv = f), (Action)(() => { }), Color.Red);
            UIPlace(slider, 420f, 160f, 20f, 120f);
            object scroll = New("Terraria.GameContent.UI.Elements.UIScrollbar", Enum.ToObject(T("Terraria.GameContent.UI.Elements.UIScrollbar+ColorTheme"), 0));
            UIPlace(scroll, 460f, 160f, 20f, 200f);
            CallI(scroll, "SetView", 50f, 400f);
            object diag = New("Terraria.UI.NetDiagnosticsUI");
            object sb = SF(TMain, "spriteBatch");
            DirectDraw("uielements", () => InBatch(() =>
            {
                foreach (object e in new[] { hair, icon, border, bar, slider, scroll }) CallI(e, "Draw", sb);
                CallI(diag, "Draw", sb);
                Unwrap(MI(T("Terraria.GameContent.UI.CustomCurrencyManager"), "DrawSavings", 5), null, new object[] { sb, SF(T("Terraria.ID.CustomCurrencyID"), "DefenderMedals"), 500f, 300f, false });
            }));
            InMouseX = 470; InMouseY = 250;
            Frames(1);
            CallI(scroll, "LeftMouseDown", MouseEvt(scroll));
            P("scrollbar", CallI(scroll, "GetValue"));
            InMouseX = 640; InMouseY = 360;
            string[] lines = new string[4];
            object[] pt = { SF(T("Terraria.ID.CustomCurrencyID"), "DefenderMedals"), lines, 0, 25L };
            Unwrap(MI(T("Terraria.GameContent.UI.CustomCurrencyManager"), "GetPriceText", 4), null, pt);
            P("currencyprice", pt[2], lines[0]);
            object frame = IF(SF(TMain, "MinimapFrameManagerInstance"), "ActiveSelection");
            float ms = (float)SF(TMain, "mapMinimapScale");
            CallI(frame, "ZoomInButton");
            CallI(frame, "ZoomOutButton");
            P("minimapzoom", SF(TMain, "mapMinimapScale"));
            SSet(TMain, "mapMinimapScale", ms);
            object anchor = New("Terraria.GameContent.UI.WorldUIAnchor", (object)pl);
            object bubble = New("Terraria.GameContent.UI.EmoteBubble", 0, anchor, 180);
            var biomes = new List<int>();
            CallI(bubble, "ProbeBiomes", biomes, pl);
            P("emotebiomes", biomes.Count);
            CallS("Terraria.UI.IngameFancyUI", "OpenVirtualKeyboard", 1);
            Frames(2);
            Record("virtualkeyboard", 1);
            CallS("Terraria.UI.IngameFancyUI", "Close", false);
            Frames(2);
            StateLine("uielements");
        }
    }
}
