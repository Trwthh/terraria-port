//! Synthetic worlds that exercise the sections the reference world leaves empty (tile entities, signs,
//! pressure plates, town rooms, bestiary, shimmered NPCs, pillars) plus the error paths.

use terraria_core::tile::TileMap;
use terraria_formats::wld::*;

fn item(t: i16, prefix: u8, stack: i16) -> TeItem {
    TeItem { item_type: t, prefix, stack }
}

fn sample() -> World {
    let mut header = WorldHeader::default();
    header.world_name = "synthetic \u{00e9}".into();
    header.seed_text = "seed".into();
    header.world_generator_version = 0x0102_0304_0506_0708;
    header.unique_id = [7; 16];
    header.world_id = 424242;
    header.max_tiles_x = 6;
    header.max_tiles_y = 5;
    header.flags.angler_who_finished_today = vec!["Ann".into(), "Bob".into()];
    header.flags.temp_party_celebrating_npcs = vec![17, 18];
    header.flags.banner_system = BannerSystem { kill_count: vec![1, 2, 3], claimable_banners: vec![4, 5] };
    header.flags.tree_tops = TreeTops { variations: vec![1; TREE_TOPS_AREA_COUNT] };
    header.flags.extra_spawn_points = ExtraSpawnPoints { points: vec![(10, 20), (-3, 4)] };
    header.flags.world_manifest = "{\"Version\":\"x\"}".into();
    header.flags.temp_time = 13500.5;
    header.flags.temp_max_rain = 0.25;

    let mut tiles = TileMap::new(6, 5);
    for x in 0..6 {
        for y in 2..5 {
            let t = tiles.get_mut(x, y);
            t.set_active(true);
            t.type_ = 1;
            t.frame_x = -1;
            t.frame_y = -1;
        }
    }
    tiles.get_mut(3, 1).liquid = 80;

    let mut importance = vec![false; 754];
    importance[21] = true;

    let equip = |n: usize| {
        let mut e = [None; 9];
        for (i, slot) in e.iter_mut().enumerate() {
            if n & (1 << i) != 0 {
                *slot = Some(item(100 + i as i16, i as u8, 1 + i as i16));
            }
        }
        e
    };
    World {
        version: 326,
        metadata: FileMetadata { file_type: 2, revision: 9, is_favorite: true },
        section_pointers: vec![0; 11],
        importance,
        header,
        tiles,
        chests: vec![Chest {
            x: 3,
            y: 1,
            name: "Loot".into(),
            items: vec![
                ChestItem { stack: 3, item_type: 5, prefix: 2 },
                ChestItem::default(),
                ChestItem { stack: 999, item_type: 4000, prefix: 0 },
            ],
        }],
        signs: vec![Sign { text: "hello\nworld".into(), x: 1, y: 2 }],
        npcs: Npcs {
            shimmered_town_npcs: vec![17, 18],
            town_npcs: vec![TownNpc {
                net_id: 17,
                given_name: "Merchanty".into(),
                position_x: 160.5,
                position_y: 320.25,
                homeless: false,
                home_tile_x: 10,
                home_tile_y: 11,
                town_npc_variation_index: 2,
                homeless_despawn: true,
            }],
            saves_and_loads_npcs: vec![SavedNpc { net_id: 517, position_x: 1.0, position_y: 2.0 }],
        },
        tile_entities: vec![
            TileEntity { id: 0, x: 1, y: 1, data: TileEntityData::TrainingDummy { npc: -1 } },
            TileEntity { id: 1, x: 2, y: 1, data: TileEntityData::ItemFrame { item: item(9, 1, 1) } },
            TileEntity { id: 2, x: 3, y: 1, data: TileEntityData::LogicSensor { logic_check: 3, on: true } },
            TileEntity {
                id: 3,
                x: 4,
                y: 1,
                data: TileEntityData::DisplayDoll { pose: 5, equip: equip(0x1FF), dyes: equip(0x105), misc: [Some(item(7, 0, 1))] },
            },
            TileEntity {
                id: 4,
                x: 5,
                y: 1,
                data: TileEntityData::DisplayDoll { pose: 0, equip: equip(0), dyes: equip(0), misc: [None] },
            },
            TileEntity { id: 5, x: 6, y: 1, data: TileEntityData::WeaponsRack { item: item(3, 4, 5) } },
            TileEntity {
                id: 6,
                x: 7,
                y: 1,
                data: TileEntityData::HatRack {
                    items: [Some(item(1, 0, 1)), None],
                    dyes: [None, Some(item(2, 3, 4))],
                },
            },
            TileEntity { id: 7, x: 8, y: 1, data: TileEntityData::FoodPlatter { item: item(5, 6, 7) } },
            TileEntity { id: 8, x: 9, y: 1, data: TileEntityData::TeleportationPylon },
            TileEntity { id: 9, x: 10, y: 1, data: TileEntityData::DeadCellsDisplayJar { item: item(8, 0, 2) } },
            TileEntity { id: 10, x: 11, y: 1, data: TileEntityData::KiteAnchor { item_type: 4000 } },
            TileEntity { id: 11, x: 12, y: 1, data: TileEntityData::CritterAnchor { item_type: -5 } },
        ],
        weighted_pressure_plates: vec![(4, 5), (6, 7)],
        town_rooms: vec![TownRoom { npc_type: 17, x: 3, y: 4 }],
        bestiary: Bestiary {
            kills: vec![("Blue Slime".into(), 12), ("Zombie".into(), 1)],
            sights: vec!["Blue Slime".into()],
            chats: vec!["Guide".into(), "Merchant".into()],
        },
        creative_powers: vec![
            CreativePower { id: 0, value: CreativePowerValue::Toggle(true) },
            CreativePower { id: 8, value: CreativePowerValue::Slider(0.5) },
            CreativePower { id: 9, value: CreativePowerValue::Toggle(false) },
            CreativePower { id: 10, value: CreativePowerValue::Toggle(true) },
            CreativePower { id: 12, value: CreativePowerValue::Slider(1.0) },
            CreativePower { id: 13, value: CreativePowerValue::Toggle(false) },
        ],
        footer: Footer { world_name: "synthetic \u{00e9}".into(), world_id: 424242 },
    }
}

#[test]
fn every_section_round_trips() {
    let world = sample();
    let bytes = write_world(&world).expect("write");
    let back = read_world(&bytes).expect("read");
    assert_eq!(back.version, 326);
    assert_eq!(back.metadata, world.metadata);
    assert_eq!(back.importance, world.importance);
    assert_eq!(back.header, world.header);
    assert_eq!(back.tiles.as_slice(), world.tiles.as_slice());
    assert_eq!(back.chests, world.chests);
    assert_eq!(back.signs, world.signs);
    assert_eq!(back.npcs, world.npcs);
    assert_eq!(back.tile_entities, world.tile_entities);
    assert_eq!(back.weighted_pressure_plates, world.weighted_pressure_plates);
    assert_eq!(back.town_rooms, world.town_rooms);
    assert_eq!(back.bestiary, world.bestiary);
    assert_eq!(back.creative_powers, world.creative_powers);
    assert_eq!(back.footer, world.footer);
    assert_eq!(write_world(&back).expect("rewrite"), bytes);
    // pointers read back are the real section ends
    assert_eq!(*back.section_pointers.last().expect("pointers") as usize, bytes.len() - 1 - 1 - "synthetic \u{00e9}".len() - 4 - 0);
}

#[test]
fn negative_chest_stack_is_normalised_like_the_original() {
    let mut world = sample();
    world.chests[0].items[0].stack = -4; // SaveChests: `if (item.stack < 0) item.stack = 1`
    let back = read_world(&write_world(&world).expect("write")).expect("read");
    assert_eq!(back.chests[0].items[0], ChestItem { stack: 1, item_type: 5, prefix: 2 });
}

#[test]
fn items_that_should_not_be_in_inventory_are_saved_empty() {
    let mut world = sample();
    world.chests[0].items[0] = ChestItem { stack: 10, item_type: 58, prefix: 0 }; // heart
    let back = read_world(&write_world(&world).expect("write")).expect("read");
    assert_eq!(back.chests[0].items[0], ChestItem::default());
}

#[test]
fn unknown_tile_entity_type_is_an_error_with_section_and_offset() {
    let mut bytes = write_world(&sample()).expect("write");
    let world = read_world(&bytes).expect("read");
    let start = world.section_pointers[5] as usize + 4; // after the entity count
    bytes[start] = 99;
    match read_world(&bytes) {
        Err(WldError::Invalid { section, offset, .. }) => {
            assert_eq!(section, "TileEntities");
            assert!(offset > start);
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn unknown_creative_power_is_an_error() {
    let mut world = sample();
    world.creative_powers = vec![CreativePower { id: 3, value: CreativePowerValue::Toggle(true) }];
    assert!(matches!(write_world(&world), Err(WldError::Unrepresentable { section: "CreativePowers", .. })));
}

#[test]
fn damaged_pointer_reports_the_section() {
    let bytes = write_world(&sample()).expect("write");
    let mut bad = bytes.clone();
    let at = 26 + 4 * 3; // pointer 3 (end of Chests)
    let p = i32::from_le_bytes([bad[at], bad[at + 1], bad[at + 2], bad[at + 3]]);
    bad[at..at + 4].copy_from_slice(&(p + 1).to_le_bytes());
    assert!(matches!(read_world(&bad), Err(WldError::BadSectionPointer { section: "Chests", .. })));
}

#[test]
fn trailing_bytes_and_bad_footer_are_errors() {
    let bytes = write_world(&sample()).expect("write");
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(read_world(&extra).is_err());
    let mut renamed = bytes;
    let n = renamed.len();
    renamed[n - 1] ^= 1; // world id in the footer
    assert!(matches!(read_world(&renamed), Err(WldError::BadFooter { .. })));
}

#[test]
fn version_gate_names_the_first_unsupported_branch() {
    for v in [0, -1, 327, 1000] {
        assert!(matches!(check_version(v), Err(WldError::UnsupportedVersion { .. })), "version {v}");
    }
    match check_version(50) {
        Err(WldError::UnsupportedVersion { branch, .. }) => assert!(branch.contains("BeforeRelease88")),
        other => panic!("{other:?}"),
    }
    match check_version(300) {
        Err(WldError::UnsupportedVersion { branch, .. }) => assert!(branch.contains("version >= 302 stores skyblockWorld"), "{branch}"),
        other => panic!("{other:?}"),
    }
    for v in 323..=326 {
        assert!(check_version(v).is_ok());
    }
}

#[test]
fn mismatched_footer_or_tile_map_is_refused_on_write() {
    let mut w = sample();
    w.footer.world_id = 1;
    assert!(matches!(write_world(&w), Err(WldError::Unrepresentable { .. })));
    let mut w = sample();
    w.header.max_tiles_x = 7;
    assert!(matches!(write_world(&w), Err(WldError::Unrepresentable { .. })));
}
