// The probe run, part R (Probe.cs; port: tests/draw/probe_r.rs, the same steps): gamepad UI page ends (the banner
// pips list and grid pages' ReachEndEvent handlers) and the fancy UI's back command, the menu list items' delete
// buttons and the world selection's new world button (each only sets the menu mode, restored after), the time
// logger's counter format, and NetMessage.ResyncTiles (a send to a client, which single player does not send).
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    static void ProbeR(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        if (Group("uilinks"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            Type nav = T("Terraria.UI.Gamepad.UILinkPointNavigator");
            var pages = (IDictionary)SF(nav, "Pages");
            for (int id = 0; id <= 100; id++)
            {
                if (!pages.Contains(id)) continue;
                object page = pages[id];
                var ev = (Delegate)page.GetType().GetField("ReachEndEvent", AllInst).GetValue(page);
                if (ev == null) continue;
                foreach (int next in new[] { -1, -2, -3, -4 }) ev.DynamicInvoke(0, next);
            }
            Type sc = T("Terraria.UI.Gamepad.UILinkPointNavigator+Shortcuts");
            int back = (int)SF(sc, "BackButtonCommand");
            int menu = (int)SF(TMain, "menuMode");
            SSet(sc, "BackButtonCommand", 3);
            CallS("Terraria.Initializers.UILinksInitializer", "FancyExit");
            SSet(sc, "BackButtonCommand", back);
            SSet(TMain, "menuMode", menu);
            Frames(2);
            StateLine("uilinks");
        }
        if (Group("menuclicks"))
        {
            int menu = (int)SF(TMain, "menuMode");
            object pli = New("Terraria.GameContent.UI.Elements.UICharacterListItem", SF(TMain, "ActivePlayerFileData"), 0);
            CallI(pli, "DeleteButtonClick", MouseEvt(pli), pli);
            P("menu_deleteplayer", SF(TMain, "menuMode"));
            SSet(TMain, "menuMode", menu);
            object wli = New("Terraria.GameContent.UI.Elements.UIWorldListItem", SF(TMain, "ActiveWorldFileData"), 0, true, true, false);
            CallI(wli, "DeleteButtonClick", MouseEvt(wli), wli);
            P("menu_deleteworld", SF(TMain, "menuMode"));
            SSet(TMain, "menuMode", menu);
            object ws = New("Terraria.GameContent.UI.States.UIWorldSelect");
            string nwn = (string)SF(TMain, "newWorldName");
            CallI(ws, "NewWorldClick", MouseEvt(ws), ws);
            P("menu_newworld", SF(TMain, "menuMode"), SF(TMain, "newWorldName"));
            CallI(SF(TMain, "MenuUI"), "SetState", new object[] { null });
            SSet(TMain, "menuMode", menu);
            SSet(TMain, "newWorldName", nwn);
            Frames(2);
            StateLine("menuclicks");
        }
        if (Group("timelogfmt"))
        {
            object fmt = SF(T("Terraria.TimeLogger"), "_intFormat");
            MethodInfo f = fmt.GetType().GetMethods(AllInst).First(m => m.Name == "Format" && m.GetParameters()[0].ParameterType == typeof(double));
            P("timelog_format", f.Invoke(fmt, new object[] { 42.0 }), f.Invoke(fmt, new object[] { 1234.6 }), f.Invoke(fmt, new object[] { -3.0 }));
            Unwrap(MI(T("Terraria.NetMessage"), "ResyncTiles", 2), null, new object[] { 0, new Rectangle(sx - 300, sy - 200, 600, 400) });
            StateLine("timelogfmt");
        }
    }
}
