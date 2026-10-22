// The probe run, part M (Probe.cs; port: tests/draw/probe_m.rs, the same steps): smaller helpers the earlier runs'
// play did not reach (world items, wiring, the map, popup texts, tiles, petting, teleport and portal helpers, gore,
// the random generator, chat, golf, void lenses, whip tag effects, dual dungeon zones, weighted randoms, fake fish,
// pylons, the collision visualizer, display dolls, hat racks, player heads, a stoned player), called directly.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    /// <summary>A struct of `type` with its fields set (name, value pairs).</summary>
    static object MakeStruct(string type, params object[] kv)
    {
        object o = Activator.CreateInstance(T(type));
        for (int i = 0; i < kv.Length; i += 2) o.GetType().GetField((string)kv[i], AllInst).SetValue(o, kv[i + 1]);
        return o;
    }
    static int NewWorldItem(int x, int y, int type, int stack)
    {
        MethodInfo newItem = T("Terraria.Item").GetMethods(AllStatic).First(m => m.Name == "NewItem" && m.GetParameters().Length == 12);
        return (int)newItem.Invoke(null, new object[] { Src(), x, y, 16, 16, type, stack, false, 0, Enum.ToObject(newItem.GetParameters()[9].ParameterType, 0), null, null });
    }
    static object WorldItemAt(int i) { return ((Array)SF(TMain, "item")).GetValue(i); }

    static void ProbeM(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        int me = (int)SF(TMain, "myPlayer");
        if (Group("miscdirect"))
        {
            ClearEntities();
            ClearFx();
            PlacePlayer(x0 + 2, gy - 1);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            Type tWI = T("Terraria.WorldItem");
            // world items: a shimmered item, money picked up by a monster, a bucket filled, a voodoo doll in lava
            Unwrap(MI(tWI, "SpawnShimmeredItem", 5), null, new object[] { Src(), new Vector2(pp.X + 40f, pp.Y), 400, 9, 3 });
            object z = NpcNear(3, 30f, 0f);
            object coin = WorldItemAt(NewWorldItem((int)pp.X + 30, (int)pp.Y, 72, 5));
            CallI(coin, "GetPickedUpByMonsters_Money");
            P("monstermoney", IF(z, "extraValue"), IF(coin, "active"));
            object bucket = WorldItemAt(NewWorldItem((x0 + 20) * 16, (gy - 2) * 16, 205, 1));
            object wt = TileAt(x0 + 20, gy - 1);
            ISet(wt, "liquid", (byte)255);
            Unwrap(MI(wt.GetType(), "liquidType", 1), wt, new object[] { 0 });
            CallI(bucket, "TryFillBucket");
            P("bucket", IF(bucket, "type"), IF(wt, "liquid"));
            ISet(wt, "liquid", (byte)0);
            object doll = WorldItemAt(NewWorldItem((x0 + 22) * 16, (gy - 2) * 16, 267, 1));
            NpcNear(22, 200f, 0f);
            CallI(doll, "VoodooDollLavaDeath");
            P("voodoo", FindNpc(22) >= 0, IF(doll, "active"));
            ClearItems();
            // wiring: an actuated tile deactivated, a hopper chest
            PlaceTile(x0 + 25, gy - 1, 1, 0);
            Unwrap(MI(T("Terraria.Wiring"), "DeActive", 2), null, new object[] { x0 + 25, gy - 1 });
            PTiles("deactive", x0 + 25, gy - 1, 1, 1);
            PlaceTile(x0 + 28, gy - 1, 21, 0);
            Unwrap(MI(T("Terraria.Wiring"), "Hopper", 2), null, new object[] { x0 + 28, gy - 2 });
            // the map: a tile unlocked prettily
            CallI(SF(TMain, "Map"), "UnlockMapTilePretty", sx + 3, sy - 5);
            // popup texts
            int pt = (int)Unwrap(MI(T("Terraria.PopupText"), "NewText", 4, "PopupTextContext,Int32,Vector2,Boolean"), null,
                new object[] { Enum.ToObject(T("Terraria.PopupTextContext"), 2), 1, new Vector2(pp.X, pp.Y - 40f), false });
            P("popup", pt);
            if (pt >= 0) Unwrap(MI(T("Terraria.PopupText"), "EmitFancyFlashDust", 1), null, new object[] { ((Array)SF(T("Terraria.PopupText"), "popupText")).GetValue(pt) });
            // a tile's actuated colour
            P("actcolor", CallI(TileAt(x0 + 25, gy - 1), "actColor", new Color(200, 150, 100, 255)));
            // petting a mount
            object info = MakeStruct("Terraria.GameContent.PlayerPettingInfo");
            CallI(pl, "PetMount", info);
            // teleport and portal helpers, the smart cursor's direction
            object[] tp = { pl, Vector2.Zero };
            P("teleportspot", Unwrap(MI(T("Terraria.GameContent.TeleportHelpers"), "FindClosestTeleportSpotNoSpace", 2), null, tp), tp[1]);
            object[] po = { new Vector2(20f, 42f), new Vector2(pp.X + 100f, pp.Y), 1f, 0, 0 };
            P("portalout", Unwrap(MI(T("Terraria.GameContent.PortalHelper"), "GetPortalOutingPoint", 5), null, po), po[3], po[4]);
            P("smartdir", Unwrap(MI(T("Terraria.GameContent.SmartCursorHelper"), "GetDesiredDirectionFrom", 1), null, new object[] { new Vector2(30f, -50f) }));
            // gore: the chum chunk's water line
            int g = (int)Unwrap(MI(T("Terraria.Gore"), "NewGore", 4), null, new object[] { new Vector2(pp.X, pp.Y - 30f), Vector2.Zero, 1, 1f });
            ISet(wt, "liquid", (byte)200);
            P("chumwater", CallI(((Array)SF(TMain, "gore")).GetValue(g), "ChumFloatingChunk_GetWaterLine", x0 + 20, gy - 1));
            ISet(wt, "liquid", (byte)0);
            // the random generator's large range
            P("largerange", CallI(SF(TMain, "rand"), "Next", int.MinValue + 5, int.MaxValue - 5));
            // chat, golf, a void lens
            P("chattext", CallS("Terraria.UI.Chat.ChatManager", "AddChatText", SF(T("Terraria.GameContent.FontAssets"), "MouseText").GetType().GetProperty("Value").GetValue(SF(T("Terraria.GameContent.FontAssets"), "MouseText"), null), "[i:9]", Vector2.One));
            object club = ProjAt(NewProj(pp.X, pp.Y, 0f, 0f, 721));
            object ball = ProjAt(NewProj(pp.X + 20f, pp.Y, 0f, 0f, 721));
            object strength = Unwrap(MI(T("Terraria.GameContent.Golf.GolfHelper"), "CalculateShotStrength", 2, "Projectile,Entity"), null, new object[] { club, ball });
            P("golfstrength", IF(strength, "AbsoluteStrength"), IF(strength, "RelativeStrength"));
            New("Terraria.GameContent.VoidLensHelper", new Vector2(pp.X, pp.Y - 50f), 0.8f);
            // whip tag effects
            Array tags = (Array)SF(T("Terraria.ID.ItemID+Sets"), "UniqueTagEffects");
            Type tTag = T("Terraria.GameContent.Items.UniqueTagEffect");
            object proj = ProjAt(NewProj(pp.X, pp.Y, 0f, 0f, 1));
            foreach (int wi in new[] { 4680, 5476, 4678, 5477, 5478, 4912 })
            {
                object tag = tags.GetValue(wi);
                MethodInfo modTagged = tTag.GetMethod("ModifyTaggedHit"), modProc = tTag.GetMethod("ModifyProcHit");
                object ch = Activator.CreateInstance(modTagged.GetParameters()[3].ParameterType.GetElementType());
                object[] a1 = { pl, proj, z, ch };
                modTagged.Invoke(tag, a1);
                object[] a2 = { pl, proj, z, a1[3] };
                modProc.Invoke(tag, a2);
                tTag.GetMethod("OnTaggedHit").Invoke(tag, new object[] { pl, proj, z, 40 });
                P("whiptag_" + wi, tTag.GetMethod("OnProcHit").Invoke(tag, new object[] { pl, proj, z, 40 }));
            }
            // the dual dungeons' zone scan
            object flags = Activator.CreateInstance(T("Terraria.NPCSpawningFlagsForDualDungeons"));
            P("dualscan", CallI(flags, "ScanZonesFor", x0 + 3, gy), CallI(flags, "ScanZonesFor", sx, sy + 1));
            // a weighted random of colours
            object wr = Activator.CreateInstance(T("Terraria.Utilities.WeightedRandom`1").MakeGenericType(typeof(Color)));
            MethodInfo wadd = wr.GetType().GetMethods().First(m => m.Name == "Add" && m.GetParameters().Length == 2);
            wadd.Invoke(wr, new object[] { new Color(200, 160, 20, 180), 3.0 });
            wadd.Invoke(wr, new object[] { new Color(103, 98, 122, 180), 5.0 });
            P("weighted", wr.GetType().GetMethod("Get").Invoke(wr, null), wr.GetType().GetMethod("Get").Invoke(wr, null));
            // fake fish: spawned, pinged, pushed away, repelled
            Type tPO = T("Terraria.GameContent.Drawing.ParticleOrchestrator");
            object pos = MakeStruct("Terraria.GameContent.Drawing.ParticleOrchestraSettings", "PositionInWorld", new Vector2(pp.X + 60f, pp.Y + 8f), "MovementVector", new Vector2(1f, 0f), "UniqueInfoPiece", 2290);
            Unwrap(MI(tPO, "Spawn_FakeFish", 2), null, new object[] { pos, false });
            object bob = ProjAt(NewProj(pp.X + 70f, pp.Y, 0f, 0f, 360));
            CallS("Terraria.GameContent.Drawing.ParticleOrchestrator", "PingFakeFish", bob, 2290);
            CallS("Terraria.GameContent.Drawing.ParticleOrchestrator", "PushAwayFakeFish", bob, 2290);
            object fish = null;
            foreach (object pa in (IEnumerable)IF(SF(TMain, "ParticleSystem_World_BehindPlayers"), "Particles")) if (pa.GetType().Name == "FakeFishParticle") fish = pa;
            if (fish != null)
            {
                object[] rd = { MakeStruct("Terraria.Graphics.Renderers.ParticleRepelDetails", "SourcePosition", new Vector2(pp.X + 50f, pp.Y), "Radius", 30f, "IsInWater", true) };
                CallI(fish, "BeRepelled", rd);
                P("fakefish", IF(fish, "Position"), IF(fish, "Velocity"));
            }
            // pylons: dust and a teleport request
            CallS("Terraria.GameContent.TeleportPylonsSystem", "SpawnInWorldDust", 1, new Rectangle((int)pp.X, (int)pp.Y, 48, 64));
            PlaceTile(x0 + 32, gy - 1, 597, 0);
            object pinfo = MakeStruct("Terraria.GameContent.TeleportPylonInfo", "PositionInTiles", Activator.CreateInstance(T("Terraria.DataStructures.Point16"), x0 + 31, gy - 4), "TypeOfPylon", Enum.ToObject(T("Terraria.GameContent.TeleportPylonType"), 0));
            CallI(SF(TMain, "PylonSystem"), "HandleTeleportRequest", pinfo, me);
            P("pylon", IF(pl, "position"));
            PlacePlayer(x0 + 2, gy - 1);
            PFx("miscdirect");
            Immune();
            ISet(pl, "statLife", 100);
            StateLine("miscdirect");
            Frames(3);
            Record("miscdirect", 1);
            ClearEntities();
            ClearFx();
            ClearItems();
            ClearArea(x0, x0 + 40, gy);
            ClearLiquid(x0, gy - 12, 41, 12);
            Frames(2);
        }
        // ---- draws called outside the frame: the collision visualizer, a display doll's and a hat rack's slots, a
        //      player head with outlines; then a stoned player drawn in the frame
        if (Group("drawmisc"))
        {
            ClearEntities();
            ClearFx();
            PlacePlayer(sx, sy);
            Frames(2);
            var pp = (Vector2)IF(pl, "position");
            Type tCV = T("Terraria.Physics.CollisionVisualizer");
            object doll = New("Terraria.GameContent.Tile_Entities.TEDisplayDoll");
            object rack = New("Terraria.GameContent.Tile_Entities.TEHatRack");
            object sb = SF(TMain, "spriteBatch");
            DirectDraw("drawmisc", () => InBatch(() =>
            {
                SSet(tCV, "enabled", true);
                SSet(tCV, "colorSelf", new Color(50, 205, 50, 255));
                CallS("Terraria.Physics.CollisionVisualizer", "VisualizeConeFast", pp, new Rectangle((int)pp.X + 40, (int)pp.Y, 20, 20), 0.3f, 0.6f, 90f, true);
                CallS("Terraria.Physics.CollisionVisualizer", "VisualizeConeSlow", pp, new Rectangle((int)pp.X + 40, (int)pp.Y, 20, 20), -0.3f, 0.5f, 80f, true);
                object hit = New("Terraria.DataStructures.MultiPointHitbox", new Point(4, 4), new[] { pp, pp + new Vector2(30f, -20f), pp + new Vector2(60f, 10f) });
                CallS("Terraria.Physics.CollisionVisualizer", "VisualizeAABBvMultiPoint", new Rectangle((int)pp.X, (int)pp.Y, 20, 20), hit);
                SSet(tCV, "enabled", false);
                CallI(doll, "DrawUI", pl, sb);
                CallI(rack, "DrawInner", pl, sb);
                object renderer = SF(TMain, "PlayerRenderer");
                renderer.GetType().GetMethod("DrawPlayerHead").Invoke(renderer, new object[] { SF(TMain, "Camera"), pl, new Vector2(400f, 300f), 1f, 1.2f, Color.White });
            }));
            CallI(pl, "AddBuff", 156, 120, false);
            Frames(2);
            Record("stoned", 1);
            ClearPlayerBuffs();
            StateLine("drawmisc");
            Frames(2);
        }
    }
}
