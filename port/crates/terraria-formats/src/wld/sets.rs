//! Constant sets and counts taken from the decompile.

/// `TileID.Count` (decompile/Terraria/Terraria/ID/TileID.cs: `public static readonly ushort Count = 754;`).
pub const TILE_COUNT: u16 = 754;
/// `WallID.Count` (decompile/Terraria/Terraria/ID/WallID.cs: `public static readonly ushort Count = 367;`).
pub const WALL_COUNT: u16 = 367;
/// `FileType.World` (decompile/Terraria/Terraria/IO/FileType.cs).
pub const FILE_TYPE_WORLD: u8 = 2;
/// Version written by `SaveFileFormatHeader` (`writer.Write(326)`) and the newest `LoadWorld` accepts.
pub const CURRENT_VERSION: i32 = 326;

/// `TileID.Sets.AllowsSaveCompressionBatching = Factory.CreateBoolSet(true, 520, 423, 723, 724)`:
/// true for every tile type except these four.
pub fn allows_save_compression_batching(tile_type: u16) -> bool {
    !matches!(tile_type, 520 | 423 | 723 | 724)
}

/// `TileID.Sets.SaveSlopes` is built in `TileID.Sets.PostSetupContent()` as
/// `Main.tileSolid[i] || TileID.Sets.NonSolidSaveSlopes[i]`.
/// `NonSolidSaveSlopes` = 131, 351, 336, 340, 342, 341, 343, 344 (TileID.cs).
/// `Main.tileSolid` is the state after `Main.Initialize_TileAndNPCData2()` and `Initialize_TileAndNPCData1()`
/// (both run before `TileID.Sets.PostSetupContent()` in `Main.Initialize_AlmostEverything`, Main.cs 6797-6818):
/// every `tileSolid[n] = true/false;` statement of those two methods applied in source order, including the
/// three `for` loops (727..=732, 435..=439, 255..=268). The runtime toggles of `tileSolid` elsewhere
/// (Liquid.cs, WorldGen.cs, Player.cs, ...) happen after setup and are not part of the set.
/// Sorted, 332 tile types.
const SAVE_SLOPE_TYPES: [u16; 332] = [
    0, 1, 2, 6, 7, 8, 9, 10, 19, 22, 23, 25, 30, 37, 38, 39, 40, 41, 43, 44, 45, 46, 47, 48, 53,
    54, 56, 57, 58, 59, 60, 63, 64, 65, 66, 67, 68, 70, 75, 76, 107, 108, 109, 111, 112, 116, 117,
    118, 119, 120, 121, 122, 123, 127, 130, 131, 137, 138, 140, 145, 146, 147, 148, 150, 151, 152,
    153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 166, 167, 168, 169, 170, 175, 176,
    177, 179, 180, 181, 182, 183, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200,
    202, 203, 204, 206, 208, 211, 221, 222, 223, 224, 225, 226, 229, 230, 232, 234, 235, 239, 248,
    249, 250, 251, 252, 253, 255, 256, 257, 258, 259, 260, 261, 262, 263, 264, 265, 266, 267, 268,
    272, 273, 274, 284, 311, 312, 313, 315, 321, 322, 325, 326, 327, 328, 329, 336, 340, 341, 342,
    343, 344, 345, 346, 347, 348, 350, 351, 357, 367, 368, 369, 370, 371, 379, 380, 381, 383, 384,
    385, 387, 388, 396, 397, 398, 399, 400, 401, 402, 403, 404, 407, 408, 409, 415, 416, 417, 418,
    421, 422, 426, 427, 430, 431, 432, 433, 434, 435, 436, 437, 438, 439, 446, 447, 448, 458, 459,
    460, 472, 473, 474, 476, 477, 478, 479, 481, 482, 483, 484, 492, 495, 496, 498, 500, 501, 502,
    503, 507, 508, 512, 513, 514, 515, 516, 517, 534, 535, 536, 537, 539, 540, 541, 546, 557, 562,
    563, 566, 618, 625, 626, 627, 628, 633, 635, 641, 659, 661, 662, 664, 666, 667, 668, 669, 670,
    671, 672, 673, 674, 675, 676, 677, 678, 679, 680, 681, 682, 683, 684, 685, 686, 687, 688, 689,
    690, 691, 692, 708, 711, 712, 713, 714, 715, 716, 717, 718, 719, 722, 726, 727, 728, 729, 730,
    731, 732, 734, 735, 736, 737, 738, 739, 740, 741, 742, 743, 744, 745, 746, 747, 748, 749, 750,
];

/// `TileID.Sets.SaveSlopes[tile_type]`; false outside the table (the original array has `TileID.Count` entries).
pub fn save_slopes(tile_type: u16) -> bool {
    SAVE_SLOPE_TYPES.binary_search(&tile_type).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_slope_table_is_sorted_and_in_range() {
        assert!(SAVE_SLOPE_TYPES.windows(2).all(|w| w[0] < w[1]));
        assert!(SAVE_SLOPE_TYPES.iter().all(|&t| t < TILE_COUNT));
        // dirt, stone, platform-like non-solid entries from NonSolidSaveSlopes, and a plain non-solid tile
        assert!(save_slopes(0) && save_slopes(1) && save_slopes(131) && save_slopes(351));
        assert!(!save_slopes(3) && !save_slopes(4) && !save_slopes(5));
    }

    #[test]
    fn batching_set() {
        assert!(allows_save_compression_batching(0));
        assert!(!allows_save_compression_batching(520) && !allows_save_compression_batching(724));
    }
}
