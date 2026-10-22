// The probe run, part A (Probe.cs; port: tests/draw/probe_a.rs, the same steps).
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Microsoft.Xna.Framework;

public static partial class WG
{
    /// <summary>The player's combat / movement numbers after a frame (armour set bonuses change them).</summary>
    static void PPlayerStats(string key)
    {
        object pl = LocalPlayer;
        P(key, IF(pl, "statDefense"), IF(pl, "meleeDamage"), IF(pl, "rangedDamage"), IF(pl, "magicDamage"), IF(pl, "minionDamage"),
            IF(pl, "meleeCrit"), IF(pl, "rangedCrit"), IF(pl, "magicCrit"), IF(pl, "endurance"), IF(pl, "moveSpeed"), IF(pl, "meleeSpeed"),
            IF(pl, "whipRangeMultiplier"), IF(pl, "whipUseTimeMultiplier"), IF(pl, "maxMinions"), IF(pl, "manaCost"), IF(pl, "lifeRegen"),
            IF(pl, "statLifeMax2"), IF(pl, "statManaMax2"), IF(pl, "dashType"), IF(pl, "thorns"), IF(pl, "pickSpeed"), IF(pl, "armorPenetration"));
    }

    /// <summary>The town NPC types (townNPC after SetDefaults, not a town pet), in type order.</summary>
    static List<int> TownTypes()
    {
        var list = new List<int>();
        object n = Activator.CreateInstance(T("Terraria.NPC"));
        MethodInfo sd = T("Terraria.NPC").GetMethods().First(m => m.Name == "SetDefaults" && m.GetParameters().Length == 2 && m.GetParameters()[0].ParameterType == typeof(int));
        bool[] pet = (bool[])SF(T("Terraria.ID.NPCID+Sets"), "IsTownPet");
        for (int t = 1; t < NpcCount; t++)
        {
            sd.Invoke(n, new object[] { t, Activator.CreateInstance(sd.GetParameters()[1].ParameterType) });
            if ((bool)IF(n, "townNPC") && !pet[t]) list.Add(t);
        }
        return list;
    }

    static void ProbeA(int sx, int sy, int x0, int gy)
    {
        object pl = LocalPlayer;
        // ---- every armour set worn (ArmorSetBonuses.All: head, body, legs), its set bonus applied by Player.UpdateArmorSets
        if (Group("armor"))
        {
            IList all = (IList)SF(T("Terraria.DataStructures.ArmorSetBonuses"), "All");
            Note("armour sets: " + all.Count);
            PlacePlayer(sx, sy);
            for (int k = 0; k < all.Count; k++)
            {
                object s = all[k];
                SetArmor(0, (int)IF(s, "Head"));
                SetArmor(1, (int)IF(s, "Body"));
                SetArmor(2, (int)IF(s, "Legs"));
                Frames(2);
                PPlayerStats("armor_" + k);
                if (k % 8 == 0) Record("armor_" + k, 1);
            }
            for (int k = 0; k < 3; k++) SetArmor(k, 0);
            Frames(3);
        }
        // ---- town NPCs asked for their prices (ShopHelper.GetShoppingSettings: their happiness with the neighbours and the
        //      biome, ShopHelper.Love / Like / Dislike / HateNPC / Biome), housed where they stand, in nine biomes
        if (Group("shop"))
        {
            List<int> town = TownTypes();
            Note("town NPCs: " + string.Join(",", town.Select(t => t.ToString()).ToArray()));
            object shop = SF(TMain, "ShopHelper");
            MethodInfo gss = MI(shop.GetType(), "GetShoppingSettings", 2);
            foreach (object[] b in Biomes)
            {
                ClearEntities();
                int bx = (int)b[1], by = (int)b[2];
                PlacePlayer(bx, by);
                var ids = new List<int>();
                for (int k = 0; k < town.Count; k++)
                {
                    // two groups: the first eight close together, the rest along the row
                    int tx = bx + (k < 8 ? k - 4 : (k - 8) * 3 - 30);
                    int id = NewNpc(tx * 16 + 8, by * 16, town[k]);
                    object n = NpcAt(id);
                    ISet(n, "homeless", false);
                    ISet(n, "homeTileX", tx);
                    ISet(n, "homeTileY", by);
                    ids.Add(id);
                }
                Frames(4);
                foreach (int id in ids)
                {
                    object n = NpcAt(id);
                    object res = gss.Invoke(shop, new object[] { pl, n });
                    P("shop_" + b[0] + "_" + IF(n, "type"), IF(res, "PriceAdjustment"), IF(res, "HappinessReport"));
                }
                Record("shop_" + b[0], 1);
            }
            ClearEntities();
            PlacePlayer(sx, sy);
            Frames(5);
        }
        // ---- a desert floating island (WorldGen.DesertCloudIsland: a world generation step of drunk worlds), in the sky
        //      left of the spawn, and the tiles it wrote
        if (Group("island"))
        {
            int ix = sx - 300, iy = 160;
            PTiles("island_before", ix - 200, iy - 80, 400, 200);
            CallS("Terraria.WorldGen", "DesertCloudIsland", ix, iy);
            PTiles("island_after", ix - 200, iy - 80, 400, 200);
            PlacePlayer(ix, iy - 20);
            Frames(10);
            Record("island", 1);
            PlacePlayer(sx, sy);
            Frames(10);
        }
    }
}
