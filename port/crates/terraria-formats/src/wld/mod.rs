//! World file (.wld). See decompile/Terraria/Terraria/IO/WorldFile.cs.
//!
//! `read_world` is `WorldFile.LoadWorld_Version2` for a byte slice; `write_world` is
//! `WorldFile.SaveWorld_Version2`. Layout of the file:
//!
//! ```text
//! FileFormatHeader  version i32 (326) | FileMetadata (20 bytes) | pointer count i16 (11) | 11 x i32 pointers |
//!                   tile type count u16 | "tile frame important" bit array
//! WorldHeader       name, seed text, generator version, guid, world id, bounds, tile size, flags (header.rs)
//! Tiles             run-length coded columns (tiles.rs)
//! Chests, Signs, NPCs, TileEntities, WeightedPressurePlates, TownManager, Bestiary, CreativePowers
//! Footer            true, name, world id
//! ```
//!
//! Pointer `n` is the stream position at the end of section `n` (pointers[0] ends the FileFormatHeader,
//! pointers[10] ends CreativePowers); the footer has no pointer.
//!
//! Supported versions: the loader's `versionNumber >= N` branches are kept as a table
//! ([`LOADER_BRANCHES`]). The highest threshold is 323, so versions 323..=326 share one layout and are
//! read; anything older returns [`WldError::UnsupportedVersion`] naming the first branch that differs.
//! Only version 326 (what the 1.4.5.8 game writes) is exercised against a real file. Files below version
//! 88 belong to `LoadWorld_Version1_Old_BeforeRelease88` and are not supported.
//!
//! The writer always writes version 326, like `SaveFileFormatHeader`.
//!
//! Round trip: `SaveWorld` increments the metadata revision (`FileMetadata.IncrementAndWrite`) and stores
//! `DateTime.UtcNow` as `last_played`. [`write_world`] does neither, so `read_world` -> `write_world`
//! reproduces the file byte for byte; [`write_world_with`] with [`WriteOptions`] does what the game does.

mod binio;
mod error;
mod header;
mod sections;
mod sets;
mod tiles;
mod wire;

pub use error::WldError;
pub use header::{BannerSystem, ExtraSpawnPoints, TreeTops, WorldFlags, WorldHeader, TREE_TOPS_AREA_COUNT};
pub use sections::{
    Bestiary, Chest, ChestItem, CreativePower, CreativePowerValue, Footer, Npcs, SavedNpc, Sign, TeItem, TileEntity,
    TileEntityData, TownNpc, TownRoom,
};
pub use sets::{allows_save_compression_batching, save_slopes, CURRENT_VERSION, TILE_COUNT, WALL_COUNT};
pub use tiles::{is_the_same_as, MAX_TILES};

use binio::{Reader, Writer};
use sets::FILE_TYPE_WORLD;
use terraria_core::tile::TileMap;

/// `FileMetadata.MAGIC_NUMBER` (IO/FileMetadata.cs): "relogic" in the low 7 bytes.
const MAGIC_NUMBER: u64 = 27981915666277746;
/// Number of section pointers `SaveWorld_Version2` writes.
const POINTER_COUNT: usize = 11;

/// Section names in file order; entry `n` ends at pointer `n`, the last one is the footer.
pub const SECTION_NAMES: [&str; 12] = [
    "FileFormatHeader",
    "WorldHeader",
    "Tiles",
    "Chests",
    "Signs",
    "NPCs",
    "TileEntities",
    "WeightedPressurePlates",
    "TownManager",
    "Bestiary",
    "CreativePowers",
    "Footer",
];

/// Every `versionNumber` comparison of `LoadWorld_Version2` and its callees whose "older file" side changes
/// the byte layout, in the order the loader meets them, as (smallest version that has the current layout,
/// description).
pub const LOADER_BRANCHES: &[(i32, &str)] = &[
    (135, "LoadFileFormatHeader: version >= 135 reads FileMetadata"),
    (179, "LoadHeader: version >= 179 stores the seed text and generator version"),
    (181, "LoadHeader: version >= 181 stores the world Guid"),
    (209, "LoadWorldFlags: version >= 209 stores GameMode as int"),
    (222, "LoadWorldFlags: version >= 222 stores drunkWorld"),
    (227, "LoadWorldFlags: version >= 227 stores getGoodWorld"),
    (238, "LoadWorldFlags: version >= 238 stores tenthAnniversaryWorld"),
    (239, "LoadWorldFlags: version >= 239 stores dontStarveWorld"),
    (241, "LoadWorldFlags: version >= 241 stores notTheBeesWorld"),
    (249, "LoadWorldFlags: version >= 249 stores remixWorld"),
    (266, "LoadWorldFlags: version >= 266 stores noTrapsWorld"),
    (267, "LoadWorldFlags: version >= 267 stores zenithWorld"),
    (302, "LoadWorldFlags: version >= 302 stores skyblockWorld"),
    (141, "LoadWorldFlags: version >= 141 stores CreationTime"),
    (284, "LoadWorldFlags: version >= 284 stores LastPlayed"),
    (118, "LoadWorldFlags: version >= 118 stores downedSlimeKing and slimeRainTime"),
    (257, "LoadWorldFlags: version >= 257 stores afterPartyOfDoom"),
    (113, "LoadWorldFlags: version >= 113 stores sundialCooldown"),
    (95, "LoadWorldFlags: version >= 95 stores anglerWhoFinishedToday (early return below)"),
    (99, "LoadWorldFlags: version >= 99 stores savedAngler (early return below)"),
    (101, "LoadWorldFlags: version >= 101 stores anglerQuest (early return below)"),
    (104, "LoadWorldFlags: version >= 104 stores savedStylist (early return below)"),
    (129, "LoadWorldFlags: version >= 129 stores savedTaxCollector"),
    (201, "LoadWorldFlags: version >= 201 stores savedGolfer"),
    (107, "LoadWorldFlags: version >= 107 stores invasionSizeStart"),
    (108, "LoadWorldFlags: version >= 108 stores cultistDelay"),
    (109, "LoadWorldFlags: version >= 109 stores BannerSystem (early return below)"),
    (289, "BannerSystem.Load: version >= 289 stores claimableBanners"),
    (128, "LoadWorldFlags: version >= 128 stores fastForwardTimeToDawn (early return below)"),
    (131, "LoadWorldFlags: version >= 131 stores the downed* boss flags (early return below)"),
    (140, "LoadWorldFlags: version >= 140 stores tower flags (early return below)"),
    (170, "LoadWorldFlags: version >= 170 stores party state"),
    (174, "LoadWorldFlags: version >= 174 stores sandstorm state"),
    (178, "DD2Event.Load: version >= 178 stores savedBartender and downed invasions"),
    (195, "LoadWorldFlags: version > 194 stores mushroomBG"),
    (215, "LoadWorldFlags: version >= 215 stores underworldBG"),
    (196, "LoadWorldFlags: version > 195 stores treeBG2-4"),
    (204, "LoadWorldFlags: version >= 204 stores combatBookWasUsed"),
    (207, "LoadWorldFlags: version >= 207 stores lantern night state"),
    (211, "TreeTopsInfo.Load: version >= 211 stores the variations"),
    (212, "LoadWorldFlags: version >= 212 stores forceHalloween/XMasForToday"),
    (216, "LoadWorldFlags: version >= 216 stores copper/iron/silver/gold ore tiers"),
    (217, "LoadWorldFlags: version >= 217 stores boughtCat/Dog/Bunny"),
    (223, "LoadWorldFlags: version >= 223 stores downedEmpressOfLight/QueenSlime"),
    (240, "LoadWorldFlags: version >= 240 stores downedDeerclops"),
    (250, "LoadWorldFlags: version >= 250 stores unlockedSlimeBlueSpawn"),
    (251, "LoadWorldFlags: version >= 251 stores the unlocked*Spawn NPC flags"),
    (259, "LoadWorldFlags: version >= 259 stores combatBookVolumeTwoWasUsed"),
    (260, "LoadWorldFlags: version >= 260 stores peddlersSatchelWasUsed"),
    (261, "LoadWorldFlags: version >= 261 stores the unlockedSlime*Spawn flags"),
    (264, "LoadWorldFlags: version >= 264 stores fastForwardTimeToDusk and moondialCooldown"),
    (287, "LoadWorldFlags: version >= 287 stores forceHalloween/XMasForever"),
    (288, "LoadWorldFlags: version >= 288 stores vampireSeed"),
    (296, "LoadWorldFlags: version >= 296 stores infectedSeed"),
    (291, "LoadWorldFlags: version >= 291 stores meteor shower count and coin rain"),
    (297, "LoadWorldFlags: version >= 297 stores teamBasedSpawnsSeed and extra spawn points"),
    (304, "LoadWorldFlags: version >= 304 stores dualDungeonsSeed"),
    (323, "LoadWorldFlags: version >= 323 stores moreLightningSeed and noLightningSeed"),
    (313, "LoadWorldFlags: version 299..312 stores an extra uint before the manifest"),
    (299, "LoadWorldFlags: version >= 299 stores the world manifest string"),
    (294, "LoadChests: version >= 294 stores the slot count per chest (no shared short)"),
    (268, "LoadNPCs: version >= 268 stores ShimmeredTownNPCs"),
    (190, "LoadNPCs: version >= 190 stores NPC netIDs instead of legacy names"),
    (213, "LoadNPCs: version >= 213 stores townNpcVariationIndex"),
    (315, "LoadNPCs: version >= 315 stores homelessDespawn"),
    (140, "LoadNPCs: version >= 140 stores the SavesAndLoads NPC list"),
    (122, "LoadWorld_Version2: version >= 122 stores TileEntities (116..121 store LoadDummies)"),
    (170, "LoadWorld_Version2: version >= 170 stores WeightedPressurePlates"),
    (189, "LoadWorld_Version2: version >= 189 stores TownManager"),
    (210, "LoadWorld_Version2: version >= 210 stores the Bestiary"),
    (220, "LoadWorld_Version2: version >= 220 stores CreativePowers"),
    (307, "TEDisplayDoll.ReadExtraData: version >= 307 stores the pose byte"),
    (308, "TEDisplayDoll.ReadExtraData: version >= 308 stores the third BitsByte"),
    (312, "TEDisplayDoll.ReadExtraData: version 311 stores the ninth equip slot after the others"),
];

/// Oldest version that has the current layout (largest threshold in [`LOADER_BRANCHES`]).
const OLDEST_SUPPORTED_VERSION: i32 = 323;

/// Version gate of `LoadWorld` / `LoadWorld_Version2`.
pub fn check_version(version: i32) -> Result<(), WldError> {
    if version <= 0 || version > CURRENT_VERSION {
        return Err(WldError::UnsupportedVersion { version, branch: "LoadWorld: version is 0 or newer than 326 (StatusID.LaterVersion)" });
    }
    if version < 88 {
        return Err(WldError::UnsupportedVersion { version, branch: "LoadWorld_Version1_Old_BeforeRelease88" });
    }
    if version >= OLDEST_SUPPORTED_VERSION {
        return Ok(());
    }
    match LOADER_BRANCHES.iter().find(|(threshold, _)| *threshold > version) {
        Some((_, branch)) => Err(WldError::UnsupportedVersion { version, branch }),
        None => Err(WldError::UnsupportedVersion { version, branch: "unknown" }),
    }
}

/// `FileMetadata`: written after the version by `FileMetadata.Write`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileMetadata {
    /// `FileMetadata.Type` (`FileType.World` = 2)
    pub file_type: u8,
    /// `FileMetadata.Revision`
    pub revision: u32,
    /// `FileMetadata.IsFavorite`
    pub is_favorite: bool,
}

/// A whole world file.
pub struct World {
    /// `WorldFile._versionNumber` as read. The writer always writes [`CURRENT_VERSION`].
    pub version: i32,
    /// `Main.WorldFileMetadata`
    pub metadata: FileMetadata,
    /// The 11 section pointers as read (end offset of each section). Informational: the writer recomputes them.
    pub section_pointers: Vec<i32>,
    /// `Main.tileFrameImportant` for tile types `0..importance.len()` (the file's tile type count).
    pub importance: Vec<bool>,
    /// World name, seed, bounds, size and all world flags.
    pub header: WorldHeader,
    /// `Main.tile`
    pub tiles: TileMap,
    /// `Main.chest`
    pub chests: Vec<Chest>,
    /// `Main.sign`
    pub signs: Vec<Sign>,
    /// `NPC.ShimmeredTownNPCs`, `Main.npc` town NPCs and `SavesAndLoads` NPCs
    pub npcs: Npcs,
    /// `TileEntity.ByID` in file order
    pub tile_entities: Vec<TileEntity>,
    /// `PressurePlateHelper.PressurePlatesPressed` keys
    pub weighted_pressure_plates: Vec<(i32, i32)>,
    /// `TownRoomManager._roomLocationPairs`
    pub town_rooms: Vec<TownRoom>,
    /// `Main.BestiaryTracker`
    pub bestiary: Bestiary,
    /// Per-world `CreativePowerManager` powers
    pub creative_powers: Vec<CreativePower>,
    pub footer: Footer,
}

impl std::fmt::Debug for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("World")
            .field("version", &self.version)
            .field("metadata", &self.metadata)
            .field("world_name", &self.header.world_name)
            .field("size", &(self.tiles.width, self.tiles.height))
            .field("chests", &self.chests.len())
            .field("signs", &self.signs.len())
            .field("tile_entities", &self.tile_entities.len())
            .finish_non_exhaustive()
    }
}

/// Options for [`write_world_with`].
#[derive(Clone, Copy, Debug, Default)]
pub struct WriteOptions {
    /// `FileMetadata.IncrementAndWrite`: write `revision + 1` (the game does this on every save).
    pub increment_revision: bool,
    /// Replace `WorldFlags::last_played` (the game writes `DateTime.UtcNow.ToBinary()`).
    pub last_played: Option<i64>,
}

/// Name of the section containing byte `offset`, given the 11 pointers of a file (`SECTION_NAMES` order).
pub fn section_at(pointers: &[i32], offset: usize) -> &'static str {
    for (i, p) in pointers.iter().enumerate().take(POINTER_COUNT) {
        if (offset as i64) < *p as i64 {
            return SECTION_NAMES[i];
        }
    }
    SECTION_NAMES[POINTER_COUNT]
}

fn check_pointer(r: &Reader, section: &'static str, pointers: &[i32], index: usize) -> Result<(), WldError> {
    let actual = r.pos() as i64;
    let expected = pointers[index] as i64;
    if actual != expected {
        return Err(WldError::BadSectionPointer { section, expected, actual });
    }
    Ok(())
}

/// `WorldFile.LoadWorld_Version2`.
pub fn read_world(bytes: &[u8]) -> Result<World, WldError> {
    let mut r = Reader::new(bytes);

    // LoadFileFormatHeader
    let version = r.i32()?;
    check_version(version)?;
    let magic_at = r.pos();
    let magic = r.u64()?;
    if magic & 0x00FF_FFFF_FFFF_FFFF != MAGIC_NUMBER {
        return Err(r.invalid_at(magic_at, "Expected Re-Logic file format"));
    }
    let file_type = (magic >> 56) as u8;
    if file_type != FILE_TYPE_WORLD {
        return Err(r.invalid_at(magic_at, format!("file type {file_type}, expected World (2)")));
    }
    let revision = r.u32()?;
    let fav_at = r.pos();
    let favorite = r.u64()?;
    if favorite > 1 {
        return Err(r.invalid_at(fav_at, format!("metadata flags {favorite:#x}: only bit 0 (IsFavorite) is defined")));
    }
    let metadata = FileMetadata { file_type, revision, is_favorite: favorite == 1 };
    let count_at = r.pos();
    let pointer_count = r.i16()?;
    if pointer_count as usize != POINTER_COUNT || pointer_count < 0 {
        return Err(r.invalid_at(count_at, format!("{pointer_count} section pointers, SaveWorld_Version2 writes {POINTER_COUNT}")));
    }
    let mut section_pointers = Vec::with_capacity(POINTER_COUNT);
    for _ in 0..POINTER_COUNT {
        section_pointers.push(r.i32()?);
    }
    let tile_type_count = r.u16()? as usize;
    let mut importance = vec![false; tile_type_count];
    let (mut b, mut b2) = (0u8, 128u8);
    for slot in importance.iter_mut() {
        if b2 == 128 {
            b = r.u8()?;
            b2 = 1;
        } else {
            b2 <<= 1;
        }
        if b & b2 == b2 {
            *slot = true;
        }
    }
    // Bits above the last tile type in the final byte: SaveFileFormatHeader writes them as zero.
    if tile_type_count % 8 != 0 && b >> (tile_type_count % 8) != 0 {
        return Err(r.invalid("padding bits of the tile importance array are set"));
    }
    check_pointer(&r, SECTION_NAMES[0], &section_pointers, 0)?;

    // LoadHeader
    r.section = SECTION_NAMES[1];
    let header = WorldHeader::read_fields(&mut r)?;
    let (width, height) = (header.max_tiles_x, header.max_tiles_y);
    if width <= 0 || height <= 0 || (width as i64) * (height as i64) > MAX_TILES {
        return Err(r.invalid(format!("world size {width} x {height} is outside 1..={MAX_TILES} tiles")));
    }
    check_pointer(&r, SECTION_NAMES[1], &section_pointers, 1)?;

    r.section = SECTION_NAMES[2];
    let tiles = tiles::read_tiles(&mut r, &importance, width, height)?;
    check_pointer(&r, SECTION_NAMES[2], &section_pointers, 2)?;

    r.section = SECTION_NAMES[3];
    let chests = sections::read_chests(&mut r)?;
    check_pointer(&r, SECTION_NAMES[3], &section_pointers, 3)?;

    r.section = SECTION_NAMES[4];
    let signs = sections::read_signs(&mut r)?;
    check_pointer(&r, SECTION_NAMES[4], &section_pointers, 4)?;

    r.section = SECTION_NAMES[5];
    let npcs = sections::read_npcs(&mut r)?;
    check_pointer(&r, SECTION_NAMES[5], &section_pointers, 5)?;

    r.section = SECTION_NAMES[6];
    let tile_entities = sections::read_tile_entities(&mut r)?;
    check_pointer(&r, SECTION_NAMES[6], &section_pointers, 6)?;

    r.section = SECTION_NAMES[7];
    let weighted_pressure_plates = sections::read_pressure_plates(&mut r)?;
    check_pointer(&r, SECTION_NAMES[7], &section_pointers, 7)?;

    r.section = SECTION_NAMES[8];
    let town_rooms = sections::read_town_manager(&mut r)?;
    check_pointer(&r, SECTION_NAMES[8], &section_pointers, 8)?;

    r.section = SECTION_NAMES[9];
    let bestiary = sections::read_bestiary(&mut r)?;
    check_pointer(&r, SECTION_NAMES[9], &section_pointers, 9)?;

    r.section = SECTION_NAMES[10];
    let creative_powers = sections::read_creative_powers(&mut r)?;
    check_pointer(&r, SECTION_NAMES[10], &section_pointers, 10)?;

    r.section = SECTION_NAMES[11];
    let footer = sections::read_footer(&mut r, &header.world_name, header.world_id)?;
    if r.remaining() != 0 {
        return Err(r.invalid(format!("{} unread bytes after the footer", r.remaining())));
    }

    Ok(World {
        version,
        metadata,
        section_pointers,
        importance,
        header,
        tiles,
        chests,
        signs,
        npcs,
        tile_entities,
        weighted_pressure_plates,
        town_rooms,
        bestiary,
        creative_powers,
        footer,
    })
}

/// `WorldFile.SaveWorld_Version2` without touching the revision or the last-played time, so that
/// `write_world(&read_world(bytes)?)` equals `bytes` for a file the game wrote.
pub fn write_world(world: &World) -> Result<Vec<u8>, WldError> {
    write_world_with(world, &WriteOptions::default())
}

/// `WorldFile.SaveWorld_Version2` with the game's per-save changes available through `opts`.
pub fn write_world_with(world: &World, opts: &WriteOptions) -> Result<Vec<u8>, WldError> {
    if world.importance.len() > u16::MAX as usize {
        return Err(WldError::Unrepresentable { section: SECTION_NAMES[0], reason: "more than 65535 tile types".into() });
    }
    if world.tiles.width != world.header.max_tiles_x || world.tiles.height != world.header.max_tiles_y {
        return Err(WldError::Unrepresentable {
            section: SECTION_NAMES[2],
            reason: format!(
                "tile map is {} x {} but the header says {} x {}",
                world.tiles.width, world.tiles.height, world.header.max_tiles_x, world.header.max_tiles_y
            ),
        });
    }
    if world.footer.world_name != world.header.world_name || world.footer.world_id != world.header.world_id {
        return Err(WldError::Unrepresentable {
            section: SECTION_NAMES[11],
            reason: "footer name/id differ from the header; LoadFooter would reject the file".into(),
        });
    }

    let mut w = Writer::default();
    let mut pointers = [0i32; POINTER_COUNT];

    // SaveFileFormatHeader
    w.i32(CURRENT_VERSION);
    let revision = if opts.increment_revision { world.metadata.revision.wrapping_add(1) } else { world.metadata.revision };
    w.u64(MAGIC_NUMBER | ((world.metadata.file_type as u64) << 56));
    w.u32(revision);
    w.u64(world.metadata.is_favorite as u64);
    w.i16(POINTER_COUNT as i16);
    let pointer_table = w.pos();
    for _ in 0..POINTER_COUNT {
        w.i32(0);
    }
    w.u16(world.importance.len() as u16);
    let (mut b, mut b2) = (0u8, 1u8);
    for important in &world.importance {
        if *important {
            b |= b2;
        }
        if b2 == 128 {
            w.u8(b);
            b = 0;
            b2 = 1;
        } else {
            b2 <<= 1;
        }
    }
    if b2 != 1 {
        w.u8(b);
    }
    let mut mark = |w: &Writer, section: &'static str, index: usize| -> Result<(), WldError> {
        match i32::try_from(w.pos()) {
            Ok(p) => {
                pointers[index] = p;
                Ok(())
            }
            Err(_) => Err(WldError::Unrepresentable { section, reason: "file larger than 2 GiB".into() }),
        }
    };
    mark(&w, SECTION_NAMES[0], 0)?;

    // SaveWorldHeader
    if let Some(last_played) = opts.last_played {
        let mut header = world.header.clone();
        header.flags.last_played = last_played;
        header.write_fields(&mut w)?;
    } else {
        world.header.write_fields(&mut w)?;
    }
    mark(&w, SECTION_NAMES[1], 1)?;

    tiles::write_tiles(&mut w, &world.tiles, &world.importance)?;
    mark(&w, SECTION_NAMES[2], 2)?;

    sections::write_chests(&mut w, &world.chests)?;
    mark(&w, SECTION_NAMES[3], 3)?;

    sections::write_signs(&mut w, &world.signs)?;
    mark(&w, SECTION_NAMES[4], 4)?;

    sections::write_npcs(&mut w, &world.npcs)?;
    mark(&w, SECTION_NAMES[5], 5)?;

    sections::write_tile_entities(&mut w, &world.tile_entities)?;
    mark(&w, SECTION_NAMES[6], 6)?;

    sections::write_pressure_plates(&mut w, &world.weighted_pressure_plates)?;
    mark(&w, SECTION_NAMES[7], 7)?;

    sections::write_town_manager(&mut w, &world.town_rooms)?;
    mark(&w, SECTION_NAMES[8], 8)?;

    sections::write_bestiary(&mut w, &world.bestiary)?;
    mark(&w, SECTION_NAMES[9], 9)?;

    sections::write_creative_powers(&mut w, &world.creative_powers)?;
    mark(&w, SECTION_NAMES[10], 10)?;

    sections::write_footer(&mut w, &world.footer);

    // SaveHeaderPointers
    for (i, p) in pointers.iter().enumerate() {
        w.patch_i32(pointer_table + 4 * i, *p);
    }
    Ok(w.buf)
}
