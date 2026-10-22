// The probe run, part Q (Probe.cs; port: tests/draw/probe_q.rs, the same steps): two pylons (their map icons on the
// full-screen map, a teleport request between them), and the capture mode: its interface in each of its three modes
// (edge selection, dragged bounds, settings), the map coordinates of a pinned edge, and a capture set up and one of
// its chunks drawn (Main.DrawCapture).
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    static void ProbeQ(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        int me = (int)SF(TMain, "myPlayer");
        if (Group("pylonmap"))
        {
            ClearEntities();
            PlacePlayer(x0 + 2, gy - 1);
            Frames(2);
            var placed = new List<Point>();
            foreach (int[] p in new[] { new[] { x0 + 8, 0 }, new[] { x0 + 20, 1 } })
            {
                TWorldGen.GetMethod("PlaceObject", AllStatic).Invoke(null, new object[] { p[0], gy - 1, 597, false, p[1], 0, -1, 1 });
                Point tl = TopLeftOf(597, p[0] + 1, gy - 1);
                int id = (int)CallS("Terraria.GameContent.Tile_Entities.TETeleportationPylon", "PlacementPreviewHook_AfterPlacement", p[0], gy - 1, 597, p[1], 1, 0);
                P("pylon_" + p[1], tl.X, tl.Y, id);
                placed.Add(tl);
            }
            object sys = SF(TMain, "PylonSystem");
            CallI(sys, "RequestImmediateUpdate");
            var pylons = (IList)IF(sys, "Pylons");
            P("pylons", pylons.Count);
            // the full-screen map as the pylon's map opens it (Main.MapPylonTile: the lines to the other pylons)
            if (pylons.Count > 0) SSet(TMain, "MapPylonTile", IF(pylons[0], "PositionInTiles"));
            // the inventory closed first: Main.DoDraw closes the full-screen map with it (Player.ToggleInv)
            bool inv = (bool)SF(TMain, "playerInventory");
            SSet(TMain, "playerInventory", false);
            SSet(TMain, "mapFullscreen", true);
            SSet(TMain, "mapFullscreenScale", 1f);
            SSet(TMain, "mapFullscreenPos", new Vector2(x0 + 14, gy - 2));
            InMouseX = 640; InMouseY = 360;
            Frames(3);
            Record("pylonmap", 1);
            P("pylonmap_open", SF(TMain, "mapFullscreen"));
            SSet(TMain, "mapFullscreen", false);
            SSet(TMain, "playerInventory", inv);
            SSet(TMain, "MapPylonTile", Activator.CreateInstance(T("Terraria.DataStructures.Point16"), -1, -1));
            // two town NPCs by the second pylon (HowManyNPCsDoesPylonNeed), so the request reaches the biome check
            NewNpc((x0 + 19) * 16, (gy - 2) * 16, 22);
            NewNpc((x0 + 23) * 16, (gy - 2) * 16, 17);
            Frames(1);
            if (pylons.Count > 1)
            {
                CallI(sys, "HandleTeleportRequest", pylons[1], me);
                P("pylon_teleport", IF(pl, "position"));
            }
            PlacePlayer(x0 + 2, gy - 1);
            Frames(2);
            ClearArea(x0, x0 + 30, gy);
            CallI(sys, "RequestImmediateUpdate");
            Frames(2);
            StateLine("pylonmap");
        }
        if (Group("capturemode"))
        {
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(2);
            Type tci = T("Terraria.Graphics.Capture.CaptureInterface");
            object cmgr = SF(T("Terraria.Graphics.Capture.CaptureManager"), "Instance");
            object ci = IF(cmgr, "_interface");
            PSet(cmgr, "Active", true);
            SSet(tci, "EdgeA", new Point(sx - 10, sy - 8));
            SSet(tci, "EdgeB", new Point(sx + 12, sy + 6));
            SSet(tci, "EdgeAPinned", true);
            SSet(tci, "EdgeBPinned", true);
            var modes = (IDictionary)SF(tci, "Modes");
            for (int mode = 0; mode < 3; mode++)
            {
                ISet(ci, "SelectedMode", mode);
                InMouseX = 600; InMouseY = 300;
                Frames(2);
                Record("capture_mode" + mode, 1);
            }
            ISet(ci, "SelectedMode", 1);
            CallI(modes[1], "DragBounds", new Vector2(700f, 340f));
            P("dragbounds", SF(tci, "EdgeA"), SF(tci, "EdgeB"));
            object[] mc = { sx, sy, 0, Point.Zero };
            P("mapcoords", Unwrap(MI(tci, "GetMapCoords", 4), null, mc), mc[3]);
            ISet(ci, "SelectedMode", 0);
            PSet(cmgr, "Active", false);
            Frames(2);
            InMouseX = 640; InMouseY = 360;
            // a capture is set up (CaptureCamera.Capture -> _Capture: the chunk queue, the buffers) and one chunk is
            // drawn as CaptureCamera.DrawTick draws it (Main.DrawCapture into the camera's frame buffer); the rest of
            // DrawTick reads the frame buffer back from the GPU, which the XNA stand-in has no pixels for, so the
            // capture is then abandoned the way FinishCapture leaves the camera
            object area = Unwrap(MI(tci, "FullScreenArea", 0), null, new object[0]);
            object settings = New("Terraria.Graphics.Capture.CaptureSettings");
            ISet(settings, "Area", area);
            ISet(settings, "Biome", CallS("Terraria.Graphics.Capture.CaptureBiome", "GetCaptureBiome", -1));
            // the purity choice of the capture settings (GetPurityForPlayer: the player's surface or underground purity)
            CallS("Terraria.Graphics.Capture.CaptureBiome", "GetCaptureBiome", 1);
            ISet(settings, "CaptureBackground", true);
            ISet(settings, "OutputName", "probe");
            object cam = IF(cmgr, "_camera");
            CallI(cam, "Capture", settings);
            P("capture", area, CallI(cam, "GetProgress"), IF(cmgr, "IsCapturing"), ((ICollection)IF(cam, "_renderQueue")).Count);
            object inst = SF(TMain, "instance");
            object gd = IF(inst, "GraphicsDevice");
            var chunk = new Rectangle(sx - 20, sy - 15, 40, 30);
            DirectDraw("capture_draw", () =>
            {
                CallI(IF(inst, "TilesRenderer"), "PrepareForAreaDrawing", chunk.Left, chunk.Right, chunk.Top, chunk.Bottom, false);
                CallI(IF(inst, "TilePaintSystem"), "PrepareAllRequests");
                gd.GetType().GetMethods().First(m => m.Name == "SetRenderTarget" && m.GetParameters().Length == 1 && m.GetParameters()[0].ParameterType.Name == "RenderTarget2D").Invoke(gd, new object[] { IF(cam, "_frameBuffer") });
                MICall("DrawCapture", chunk, settings, cam);
                gd.GetType().GetMethods().First(m => m.Name == "SetRenderTarget" && m.GetParameters().Length == 1 && m.GetParameters()[0].ParameterType.Name == "RenderTarget2D").Invoke(gd, new object[] { null });
            });
            ((IEnumerable)IF(cam, "_renderQueue")).GetType().GetMethod("Clear").Invoke(IF(cam, "_renderQueue"), null);
            ISet(cam, "_outputData", null);
            ISet(cam, "_scaledFrameData", null);
            object sfb = IF(cam, "_scaledFrameBuffer");
            if (sfb != null) { CallI(sfb, "Dispose"); ISet(cam, "_scaledFrameBuffer", null); }
            ISet(cam, "_activeSettings", null);
            SSet(TMain, "GlobalTimerPaused", false);
            CallS("Terraria.Graphics.Capture.CaptureInterface", "EndCamera");
            Frames(2);
            Record("capture_done", 1);
            StateLine("capturemode");
        }
    }
}
