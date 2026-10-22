//! Sections after the tiles: chests, signs, NPCs, tile entities, weighted pressure plates, town manager,
//! bestiary, creative powers and the footer. Each reader/writer pair transcribes the matching
//! `WorldFile.Load*` / `Save*` pair (and the `Save`/`Load` of the object they delegate to).
//!
//! The loader also repairs the loaded world (drops duplicate chests and signs, signs without a sign tile,
//! tile entities on invalid tiles, rehomes NPCs). Those steps need the live game state and are not part
//! of the byte format, so the values here are exactly what the file holds.

use super::binio::{Reader, Writer};
use super::error::WldError;

// ---------------------------------------------------------------------------------------------- chests

/// `ItemID.Sets.ItemsThatShouldNotBeInInventory` (ID/ItemID.cs 72): `SaveChests` writes these as an empty slot.
const ITEMS_THAT_SHOULD_NOT_BE_IN_INVENTORY: [i32; 13] =
    [58, 184, 1734, 1735, 1867, 1868, 3453, 3454, 3455, 4143, 5013, 6135, 6136];

/// One chest slot as stored by `SaveChests`: `short stack`, then (only when `stack > 0`) `int type`, `byte prefix`.
/// An empty slot is `stack == 0` with `item_type == 0` and `prefix == 0` (`new Item()`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ChestItem {
    /// `Item.stack`
    pub stack: i16,
    /// `Item.type`
    pub item_type: i32,
    /// `Item.prefix`
    pub prefix: u8,
}

/// `Terraria.Chest` as saved.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Chest {
    /// `Chest.x`
    pub x: i32,
    /// `Chest.y`
    pub y: i32,
    /// `Chest.name`
    pub name: String,
    /// `Chest.item`; its length is `Chest.maxItems`
    pub items: Vec<ChestItem>,
}

pub(crate) fn read_chests(r: &mut Reader) -> Result<Vec<Chest>, WldError> {
    // version >= 294: no second short with the per-chest slot count
    let at = r.pos();
    let n = r.i16()?;
    let n = r.checked_count(at, n as i64, 13, "chest")?;
    let mut chests = Vec::with_capacity(n);
    for _ in 0..n {
        let x = r.i32()?;
        let y = r.i32()?;
        let name = r.string()?;
        let max_items = r.count(2, "chest slot")?;
        let mut items = Vec::with_capacity(max_items);
        for _ in 0..max_items {
            let stack = r.i16()?;
            let mut item = ChestItem::default();
            if stack > 0 {
                item.item_type = r.i32()?;
                item.stack = stack;
                item.prefix = r.u8()?;
            } else if stack < 0 {
                // `item.stack = 1` after netDefaults + Prefix
                item.item_type = r.i32()?;
                item.prefix = r.u8()?;
                item.stack = 1;
            }
            items.push(item);
        }
        chests.push(Chest { x, y, name, items });
    }
    Ok(chests)
}

pub(crate) fn write_chests(w: &mut Writer, chests: &[Chest]) -> Result<(), WldError> {
    w.len_i16("Chests", chests.len(), "chest")?;
    for chest in chests {
        w.i32(chest.x);
        w.i32(chest.y);
        w.string(&chest.name);
        w.len_i32("Chests", chest.items.len(), "chest slot")?;
        for item in &chest.items {
            if ITEMS_THAT_SHOULD_NOT_BE_IN_INVENTORY.contains(&item.item_type) {
                w.i16(0);
                continue;
            }
            let stack = if item.stack < 0 { 1 } else { item.stack };
            w.i16(stack);
            if stack > 0 {
                w.i32(item.item_type);
                w.u8(item.prefix);
            }
        }
    }
    Ok(())
}

// ----------------------------------------------------------------------------------------------- signs

/// `Terraria.Sign` as saved.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Sign {
    /// `Sign.text`
    pub text: String,
    /// `Sign.x`
    pub x: i32,
    /// `Sign.y`
    pub y: i32,
}

pub(crate) fn read_signs(r: &mut Reader) -> Result<Vec<Sign>, WldError> {
    let at = r.pos();
    let n = r.i16()?;
    let n = r.checked_count(at, n as i64, 9, "sign")?;
    let mut signs = Vec::with_capacity(n);
    for _ in 0..n {
        let text = r.string()?;
        let x = r.i32()?;
        let y = r.i32()?;
        signs.push(Sign { text, x, y });
    }
    Ok(signs)
}

pub(crate) fn write_signs(w: &mut Writer, signs: &[Sign]) -> Result<(), WldError> {
    w.len_i16("Signs", signs.len(), "sign")?;
    for s in signs {
        w.string(&s.text);
        w.i32(s.x);
        w.i32(s.y);
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------ NPCs

/// A town NPC record of `SaveNPCs` (`nPC.active && nPC.townNPC && nPC.type != 368`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TownNpc {
    /// `NPC.netID`
    pub net_id: i32,
    /// `NPC.GivenName`
    pub given_name: String,
    /// `NPC.position.X`
    pub position_x: f32,
    /// `NPC.position.Y`
    pub position_y: f32,
    /// `NPC.homeless`
    pub homeless: bool,
    /// `NPC.homeTileX`
    pub home_tile_x: i32,
    /// `NPC.homeTileY`
    pub home_tile_y: i32,
    /// `NPC.townNpcVariationIndex` (stored after a `BitsByte` whose bit 0 is `NPC.townNPC`)
    pub town_npc_variation_index: i32,
    /// `NPC.homelessDespawn`
    pub homeless_despawn: bool,
}

/// A record of the second NPC list (`NPCID.Sets.SavesAndLoads`, e.g. the lunar pillars).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SavedNpc {
    /// `NPC.netID`
    pub net_id: i32,
    /// `NPC.position.X`
    pub position_x: f32,
    /// `NPC.position.Y`
    pub position_y: f32,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Npcs {
    /// `NPC.ShimmeredTownNPCs`: the NPC types whose flag is set, ascending as `SaveNPCs` writes them.
    pub shimmered_town_npcs: Vec<i32>,
    pub town_npcs: Vec<TownNpc>,
    pub saves_and_loads_npcs: Vec<SavedNpc>,
}

pub(crate) fn read_npcs(r: &mut Reader) -> Result<Npcs, WldError> {
    let n = r.count(4, "shimmered town NPC")?;
    let mut shimmered_town_npcs = Vec::with_capacity(n);
    for _ in 0..n {
        shimmered_town_npcs.push(r.i32()?);
    }
    let mut town_npcs = Vec::new();
    while r.bool()? {
        let net_id = r.i32()?;
        let given_name = r.string()?;
        let position_x = r.f32()?;
        let position_y = r.f32()?;
        let homeless = r.bool()?;
        let home_tile_x = r.i32()?;
        let home_tile_y = r.i32()?;
        let at = r.pos();
        let bits = r.u8()?;
        if bits != 1 {
            return Err(r.invalid_at(at, format!("NPC BitsByte {bits:#04x}: SaveNPCs only writes bit 0 (townNPC)")));
        }
        let town_npc_variation_index = r.i32()?;
        let homeless_despawn = r.bool()?;
        town_npcs.push(TownNpc {
            net_id,
            given_name,
            position_x,
            position_y,
            homeless,
            home_tile_x,
            home_tile_y,
            town_npc_variation_index,
            homeless_despawn,
        });
    }
    let mut saves_and_loads_npcs = Vec::new();
    while r.bool()? {
        let net_id = r.i32()?;
        let position_x = r.f32()?;
        let position_y = r.f32()?;
        saves_and_loads_npcs.push(SavedNpc { net_id, position_x, position_y });
    }
    Ok(Npcs { shimmered_town_npcs, town_npcs, saves_and_loads_npcs })
}

pub(crate) fn write_npcs(w: &mut Writer, npcs: &Npcs) -> Result<(), WldError> {
    w.len_i32("NPCs", npcs.shimmered_town_npcs.len(), "shimmered town NPC")?;
    for t in &npcs.shimmered_town_npcs {
        w.i32(*t);
    }
    for n in &npcs.town_npcs {
        w.bool(true); // nPC.active
        w.i32(n.net_id);
        w.string(&n.given_name);
        w.f32(n.position_x);
        w.f32(n.position_y);
        w.bool(n.homeless);
        w.i32(n.home_tile_x);
        w.i32(n.home_tile_y);
        w.u8(1); // BitsByte, bit 0 = townNPC (always true for these records)
        w.i32(n.town_npc_variation_index);
        w.bool(n.homeless_despawn);
    }
    w.bool(false);
    for n in &npcs.saves_and_loads_npcs {
        w.bool(true); // nPC2.active
        w.i32(n.net_id);
        w.f32(n.position_x);
        w.f32(n.position_y);
    }
    w.bool(false);
    Ok(())
}

// ---------------------------------------------------------------------------------------- tile entities

/// The `short type, byte prefix, short stack` triple that the tile entities write for an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TeItem {
    /// `Item.type` (stored as a short)
    pub item_type: i16,
    /// `Item.prefix`
    pub prefix: u8,
    /// `Item.stack` (stored as a short)
    pub stack: i16,
}

impl TeItem {
    /// `Item.IsAir`: `type <= 0 || stack <= 0`.
    pub fn is_air(&self) -> bool {
        self.item_type <= 0 || self.stack <= 0
    }
    fn read(r: &mut Reader) -> Result<Self, WldError> {
        let item_type = r.i16()?;
        let prefix = r.u8()?;
        let stack = r.i16()?;
        Ok(TeItem { item_type, prefix, stack })
    }
    fn write(&self, w: &mut Writer) {
        w.i16(self.item_type);
        w.u8(self.prefix);
        w.i16(self.stack);
    }
}

/// Per-type payload of a tile entity (`WriteExtraData` / `ReadExtraData`). The `u8` type ids are the
/// registration order in `TileEntitiesManager.RegisterAll` (DataStructures/TileEntitiesManager.cs 26-38).
#[derive(Clone, Debug, PartialEq)]
pub enum TileEntityData {
    /// type 0, `TETrainingDummy`: `short npc`
    TrainingDummy { npc: i16 },
    /// type 1, `TEItemFrame`
    ItemFrame { item: TeItem },
    /// type 2, `TELogicSensor`: `byte logicCheck`, `bool On`
    LogicSensor { logic_check: u8, on: bool },
    /// type 3, `TEDisplayDoll`: pose byte and the `_equip[9]`, `_dyes[9]`, `_misc[1]` slots (`None` = air)
    DisplayDoll { pose: u8, equip: [Option<TeItem>; 9], dyes: [Option<TeItem>; 9], misc: [Option<TeItem>; 1] },
    /// type 4, `TEWeaponsRack`
    WeaponsRack { item: TeItem },
    /// type 5, `TEHatRack`: `_items[2]` and `_dyes[2]` (`None` = air)
    HatRack { items: [Option<TeItem>; 2], dyes: [Option<TeItem>; 2] },
    /// type 6, `TEFoodPlatter`
    FoodPlatter { item: TeItem },
    /// type 7, `TETeleportationPylon`: no extra data
    TeleportationPylon,
    /// type 8, `TEDeadCellsDisplayJar`
    DeadCellsDisplayJar { item: TeItem },
    /// type 9, `TEKiteAnchor`: `short itemType`
    KiteAnchor { item_type: i16 },
    /// type 10, `TECritterAnchor`: `short itemType`
    CritterAnchor { item_type: i16 },
}

impl TileEntityData {
    /// `TileEntity.type`
    pub fn type_id(&self) -> u8 {
        match self {
            TileEntityData::TrainingDummy { .. } => 0,
            TileEntityData::ItemFrame { .. } => 1,
            TileEntityData::LogicSensor { .. } => 2,
            TileEntityData::DisplayDoll { .. } => 3,
            TileEntityData::WeaponsRack { .. } => 4,
            TileEntityData::HatRack { .. } => 5,
            TileEntityData::FoodPlatter { .. } => 6,
            TileEntityData::TeleportationPylon => 7,
            TileEntityData::DeadCellsDisplayJar { .. } => 8,
            TileEntityData::KiteAnchor { .. } => 9,
            TileEntityData::CritterAnchor { .. } => 10,
        }
    }
}

/// `Terraria.DataStructures.TileEntity` as saved by `TileEntity.Write`.
#[derive(Clone, Debug, PartialEq)]
pub struct TileEntity {
    /// `TileEntity.ID`
    pub id: i32,
    /// `TileEntity.Position.X`
    pub x: i16,
    /// `TileEntity.Position.Y`
    pub y: i16,
    pub data: TileEntityData,
}

fn read_slot(r: &mut Reader, present: bool, what: &str) -> Result<Option<TeItem>, WldError> {
    if !present {
        return Ok(None);
    }
    let at = r.pos();
    let item = TeItem::read(r)?;
    if item.is_air() {
        return Err(r.invalid_at(at, format!("{what} slot flagged as filled but the item is air; the original writer would drop it")));
    }
    Ok(Some(item))
}

fn write_slot(w: &mut Writer, slot: &Option<TeItem>) {
    if let Some(item) = slot {
        if !item.is_air() {
            item.write(w);
        }
    }
}

fn present(slot: &Option<TeItem>) -> bool {
    matches!(slot, Some(item) if !item.is_air())
}

fn read_extra(r: &mut Reader, type_id: u8) -> Result<TileEntityData, WldError> {
    Ok(match type_id {
        0 => TileEntityData::TrainingDummy { npc: r.i16()? },
        1 => TileEntityData::ItemFrame { item: TeItem::read(r)? },
        2 => TileEntityData::LogicSensor { logic_check: r.u8()?, on: r.bool()? },
        3 => {
            // gameVersion >= 308: pose byte and the third BitsByte are both present
            let bits = r.u8()?;
            let bits2 = r.u8()?;
            let pose = r.u8()?;
            let at = r.pos();
            let bits3 = r.u8()?;
            if bits3 & 0xF8 != 0 {
                return Err(r.invalid_at(at, format!("display doll BitsByte 3 {bits3:#04x} has unused bits set")));
            }
            let equip_mask = bits as u32 | if bits3 & 2 != 0 { 256 } else { 0 };
            let dyes_mask = bits2 as u32 | if bits3 & 4 != 0 { 256 } else { 0 };
            let mut equip = [None; 9];
            for (i, slot) in equip.iter_mut().enumerate() {
                *slot = read_slot(r, equip_mask & (1 << i) != 0, "display doll equip")?;
            }
            let mut dyes = [None; 9];
            for (i, slot) in dyes.iter_mut().enumerate() {
                *slot = read_slot(r, dyes_mask & (1 << i) != 0, "display doll dye")?;
            }
            let misc = [read_slot(r, bits3 & 1 != 0, "display doll misc")?];
            TileEntityData::DisplayDoll { pose, equip, dyes, misc }
        }
        4 => TileEntityData::WeaponsRack { item: TeItem::read(r)? },
        5 => {
            let at = r.pos();
            let bits = r.u8()?;
            if bits & 0xF0 != 0 {
                return Err(r.invalid_at(at, format!("hat rack BitsByte {bits:#04x} has unused bits set")));
            }
            let items = [read_slot(r, bits & 1 != 0, "hat rack item")?, read_slot(r, bits & 2 != 0, "hat rack item")?];
            let dyes = [read_slot(r, bits & 4 != 0, "hat rack dye")?, read_slot(r, bits & 8 != 0, "hat rack dye")?];
            TileEntityData::HatRack { items, dyes }
        }
        6 => TileEntityData::FoodPlatter { item: TeItem::read(r)? },
        7 => TileEntityData::TeleportationPylon,
        8 => TileEntityData::DeadCellsDisplayJar { item: TeItem::read(r)? },
        9 => TileEntityData::KiteAnchor { item_type: r.i16()? },
        10 => TileEntityData::CritterAnchor { item_type: r.i16()? },
        other => {
            return Err(r.invalid(format!("tile entity type {other}: TileEntitiesManager.GenerateInstance has no such type")));
        }
    })
}

fn write_extra(w: &mut Writer, data: &TileEntityData) {
    match data {
        TileEntityData::TrainingDummy { npc } => w.i16(*npc),
        TileEntityData::ItemFrame { item }
        | TileEntityData::WeaponsRack { item }
        | TileEntityData::FoodPlatter { item }
        | TileEntityData::DeadCellsDisplayJar { item } => item.write(w),
        TileEntityData::LogicSensor { logic_check, on } => {
            w.u8(*logic_check);
            w.bool(*on);
        }
        TileEntityData::DisplayDoll { pose, equip, dyes, misc } => {
            let mut bits = 0u8;
            for (i, s) in equip.iter().take(8).enumerate() {
                bits |= (present(s) as u8) << i;
            }
            let mut bits2 = 0u8;
            for (i, s) in dyes.iter().take(8).enumerate() {
                bits2 |= (present(s) as u8) << i;
            }
            let bits3 = (present(&misc[0]) as u8) | ((present(&equip[8]) as u8) << 1) | ((present(&dyes[8]) as u8) << 2);
            w.u8(bits);
            w.u8(bits2);
            w.u8(*pose);
            w.u8(bits3);
            for s in equip.iter().chain(dyes.iter()).chain(misc.iter()) {
                write_slot(w, s);
            }
        }
        TileEntityData::HatRack { items, dyes } => {
            let bits = (present(&items[0]) as u8)
                | ((present(&items[1]) as u8) << 1)
                | ((present(&dyes[0]) as u8) << 2)
                | ((present(&dyes[1]) as u8) << 3);
            w.u8(bits);
            for s in items.iter().chain(dyes.iter()) {
                write_slot(w, s);
            }
        }
        TileEntityData::TeleportationPylon => {}
        TileEntityData::KiteAnchor { item_type } | TileEntityData::CritterAnchor { item_type } => w.i16(*item_type),
    }
}

pub(crate) fn read_tile_entities(r: &mut Reader) -> Result<Vec<TileEntity>, WldError> {
    // 1 type + 4 id + 2 x + 2 y
    let n = r.count(9, "tile entity")?;
    let mut list = Vec::with_capacity(n);
    for _ in 0..n {
        let type_id = r.u8()?;
        let id = r.i32()?;
        let x = r.i16()?;
        let y = r.i16()?;
        let data = read_extra(r, type_id)?;
        list.push(TileEntity { id, x, y, data });
    }
    Ok(list)
}

pub(crate) fn write_tile_entities(w: &mut Writer, list: &[TileEntity]) -> Result<(), WldError> {
    w.len_i32("TileEntities", list.len(), "tile entity")?;
    for e in list {
        w.u8(e.data.type_id());
        w.i32(e.id);
        w.i16(e.x);
        w.i16(e.y);
        write_extra(w, &e.data);
    }
    Ok(())
}

// ----------------------------------------------------------------------- weighted pressure plates, town

pub(crate) fn read_pressure_plates(r: &mut Reader) -> Result<Vec<(i32, i32)>, WldError> {
    let n = r.count(8, "weighted pressure plate")?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let x = r.i32()?;
        let y = r.i32()?;
        v.push((x, y));
    }
    Ok(v)
}

pub(crate) fn write_pressure_plates(w: &mut Writer, plates: &[(i32, i32)]) -> Result<(), WldError> {
    w.len_i32("WeightedPressurePlates", plates.len(), "weighted pressure plate")?;
    for (x, y) in plates {
        w.i32(*x);
        w.i32(*y);
    }
    Ok(())
}

/// `TownRoomManager._roomLocationPairs` entry: NPC type and the room's home tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TownRoom {
    /// `Tuple.Item1`: NPC type (`_hasRoom` index)
    pub npc_type: i32,
    /// `Point.X`
    pub x: i32,
    /// `Point.Y`
    pub y: i32,
}

pub(crate) fn read_town_manager(r: &mut Reader) -> Result<Vec<TownRoom>, WldError> {
    let n = r.count(12, "town room")?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let npc_type = r.i32()?;
        let x = r.i32()?;
        let y = r.i32()?;
        v.push(TownRoom { npc_type, x, y });
    }
    Ok(v)
}

pub(crate) fn write_town_manager(w: &mut Writer, rooms: &[TownRoom]) -> Result<(), WldError> {
    w.len_i32("TownManager", rooms.len(), "town room")?;
    for room in rooms {
        w.i32(room.npc_type);
        w.i32(room.x);
        w.i32(room.y);
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------- bestiary

/// `BestiaryUnlocksTracker`: kills (`NPCKillsTracker`), sights (`NPCWasNearPlayerTracker`),
/// chats (`NPCWasChatWithTracker`). Persistent ids are the NPC `FullName` strings; file order is kept.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Bestiary {
    /// `NPCKillsTracker._killCountsByNpcId`
    pub kills: Vec<(String, i32)>,
    /// `NPCWasNearPlayerTracker._wasNearPlayer`
    pub sights: Vec<String>,
    /// `NPCWasChatWithTracker._chattedWithPlayer`
    pub chats: Vec<String>,
}

fn read_string_list(r: &mut Reader, what: &str) -> Result<Vec<String>, WldError> {
    let n = r.count(1, what)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(r.string()?);
    }
    Ok(v)
}

fn write_string_list(w: &mut Writer, list: &[String]) -> Result<(), WldError> {
    w.len_i32("Bestiary", list.len(), "bestiary entry")?;
    for s in list {
        w.string(s);
    }
    Ok(())
}

pub(crate) fn read_bestiary(r: &mut Reader) -> Result<Bestiary, WldError> {
    let n = r.count(5, "bestiary kill entry")?;
    let mut kills = Vec::with_capacity(n);
    for _ in 0..n {
        let key = r.string()?;
        let value = r.i32()?;
        kills.push((key, value));
    }
    let sights = read_string_list(r, "bestiary sight entry")?;
    let chats = read_string_list(r, "bestiary chat entry")?;
    Ok(Bestiary { kills, sights, chats })
}

pub(crate) fn write_bestiary(w: &mut Writer, b: &Bestiary) -> Result<(), WldError> {
    w.len_i32("Bestiary", b.kills.len(), "bestiary kill entry")?;
    for (k, v) in &b.kills {
        w.string(k);
        w.i32(*v);
    }
    write_string_list(w, &b.sights)?;
    write_string_list(w, &b.chats)
}

// ------------------------------------------------------------------------------------ creative powers

/// Value saved by a per-world creative power.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CreativePowerValue {
    /// `ASharedTogglePower.Enabled`
    Toggle(bool),
    /// `ASharedSliderPower._sliderCurrentValueCache`
    Slider(f32),
}

/// One `bool true, ushort id, <power.Save>` record of `CreativePowerManager.SaveToWorld`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CreativePower {
    /// Registration index in `CreativePowerManager.Initialize` (GameContent/Creative/CreativePowerManager.cs 85-99)
    pub id: u16,
    pub value: CreativePowerValue,
}

/// Per-world persistent powers (`IPersistentPerWorldContent`, CreativePowers.cs): id and whether the value is a slider.
/// 0 `FreezeTime`, 8 `ModifyTimeRate` (slider), 9 `FreezeRainPower`, 10 `FreezeWindDirectionAndStrength`,
/// 12 `DifficultySliderPower` (slider), 13 `StopBiomeSpreadPower`.
fn power_is_slider(id: u16) -> Option<bool> {
    match id {
        0 | 9 | 10 | 13 => Some(false),
        8 | 12 => Some(true),
        _ => None,
    }
}

pub(crate) fn read_creative_powers(r: &mut Reader) -> Result<Vec<CreativePower>, WldError> {
    let mut list = Vec::new();
    while r.bool()? {
        let at = r.pos();
        let id = r.u16()?;
        let value = match power_is_slider(id) {
            Some(true) => CreativePowerValue::Slider(r.f32()?),
            Some(false) => CreativePowerValue::Toggle(r.bool()?),
            None => {
                return Err(r.invalid_at(
                    at,
                    format!("creative power id {id} is not a per-world persistent power; LoadFromWorld would stop reading here"),
                ))
            }
        };
        list.push(CreativePower { id, value });
    }
    Ok(list)
}

pub(crate) fn write_creative_powers(w: &mut Writer, powers: &[CreativePower]) -> Result<(), WldError> {
    for p in powers {
        let slider = match power_is_slider(p.id) {
            Some(s) => s,
            None => {
                return Err(WldError::Unrepresentable {
                    section: "CreativePowers",
                    reason: format!("creative power id {} is not a per-world persistent power", p.id),
                })
            }
        };
        w.bool(true);
        w.u16(p.id);
        match (p.value, slider) {
            (CreativePowerValue::Slider(v), true) => w.f32(v),
            (CreativePowerValue::Toggle(v), false) => w.bool(v),
            _ => {
                return Err(WldError::Unrepresentable {
                    section: "CreativePowers",
                    reason: format!("creative power {} has the wrong value kind", p.id),
                })
            }
        }
    }
    w.bool(false);
    Ok(())
}

// -------------------------------------------------------------------------------------------- footer

/// `SaveFooter`: `true`, world name, world id. `LoadFooter` requires both to equal the header's.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Footer {
    /// `Main.worldName`
    pub world_name: String,
    /// `Main.ActiveWorldFileData.WorldId`
    pub world_id: i32,
}

pub(crate) fn read_footer(r: &mut Reader, header_name: &str, header_id: i32) -> Result<Footer, WldError> {
    let at = r.pos();
    if !r.bool()? {
        return Err(WldError::BadFooter { offset: at, reason: "first footer byte is not true" });
    }
    let at = r.pos();
    let world_name = r.string()?;
    if world_name != header_name {
        return Err(WldError::BadFooter { offset: at, reason: "footer world name differs from the header's" });
    }
    let at = r.pos();
    let world_id = r.i32()?;
    if world_id != header_id {
        return Err(WldError::BadFooter { offset: at, reason: "footer world id differs from the header's" });
    }
    Ok(Footer { world_name, world_id })
}

pub(crate) fn write_footer(w: &mut Writer, f: &Footer) {
    w.bool(true);
    w.string(&f.world_name);
    w.i32(f.world_id);
}
