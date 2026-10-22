// The probe run, part N (Probe.cs; port: tests/draw/probe_n.rs, the same steps): in-game menus the earlier runs did
// not open far enough: the controls menu's gamepad pages (its sliders, their getters and setters), the achievements
// list scrolled and sent to an achievement, a bestiary entry with its kill count, stats and portrait.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    /// <summary>Every element under `e` (depth first, children in order), `e` included.</summary>
    static void UIWalk(object e, List<object> into)
    {
        into.Add(e);
        foreach (object c in (IEnumerable)IF(e, "Elements")) UIWalk(c, into);
    }

    static void ProbeN(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        if (Group("uikeybinds"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            CallS("Terraria.UI.IngameFancyUI", "OpenKeybinds");
            Frames(3);
            object mc = SF(TMain, "ManageControlsMenu");
            CallI(mc, "GamepadButtonClick", MouseEvt(mc), mc);
            Frames(2);
            Record("keybinds_gamepad", 1);
            CallI(mc, "VsMenuButtonClick", MouseEvt(mc), mc);
            Frames(2);
            Record("keybinds_gamepad_ui", 1);
            var all = new List<object>();
            UIWalk(mc, all);
            int n = 0;
            foreach (object e in all)
            {
                if (e.GetType().Name != "UIKeybindingSliderItem") continue;
                float st = (float)((Delegate)IF(e, "_GetStatusFunction")).DynamicInvoke();
                ((Delegate)IF(e, "_SlideKeyboardAction")).DynamicInvoke(st);
                ((Delegate)IF(e, "_SlideGamepadAction")).DynamicInvoke();
                P("slider_" + n, ((Delegate)IF(e, "_TextDisplayFunction")).DynamicInvoke(), st);
                n++;
            }
            CallI(mc, "KeyboardButtonClick", MouseEvt(mc), mc);
            CallI(mc, "VsGameplayButtonClick", MouseEvt(mc), mc);
            CallS("Terraria.UI.IngameFancyUI", "Close", false);
            Frames(2);
            StateLine("uikeybinds");
        }
        if (Group("uiachieve"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            CallS("Terraria.UI.IngameFancyUI", "OpenAchievements");
            Frames(3);
            object menu = SF(TMain, "AchievementsMenu");
            object ach = CallI(SF(TMain, "Achievements"), "GetAchievement", "TIMBER");
            CallI(menu, "GotoAchievement", ach);
            Frames(2);
            Record("achievements_goto", 1);
            object list = IF(menu, "_achievementsList");
            CallI(list, "ScrollWheel", New("Terraria.UI.UIScrollWheelEvent", list, new Vector2(640f, 360f), -240));
            Frames(2);
            Record("achievements_scrolled", 1);
            CallS("Terraria.UI.IngameFancyUI", "Close", false);
            Frames(2);
            StateLine("uiachieve");
        }
        if (Group("uibestiary"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            object z = NpcNear(3, 200f, 0f);
            object kills = IF(SF(TMain, "BestiaryTracker"), "Kills");
            for (int k = 0; k < 60; k++) CallI(kills, "RegisterKill", z);
            ClearEntities();
            object ui = SF(TMain, "BestiaryUI");
            CallS("Terraria.UI.IngameFancyUI", "OpenUIState", ui);
            Frames(3);
            object entry = CallI(SF(TMain, "BestiaryDB"), "FindEntryByNPCID", 3);
            object info = IF(ui, "_infoSpace");
            object extra = MakeStruct("Terraria.GameContent.UI.Elements.ExtraBestiaryInfoPageInformation", "BestiaryProgressReport", IF(ui, "_progressReport"));
            CallI(info, "FillInfoForEntry", entry, extra);
            Frames(2);
            Record("bestiary_entry", 1);
            CallI(info, "UpdateScrollbar", -120);
            Frames(2);
            Record("bestiary_scrolled", 1);
            CallS("Terraria.UI.IngameFancyUI", "Close", false);
            Frames(2);
            StateLine("uibestiary");
        }
    }
}
