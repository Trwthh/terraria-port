//! `Terraria.Collision` (decompile/Terraria/Terraria/Collision.cs, 4,637 lines), split by topic:
//!
//! | file | methods |
//! |---|---|
//! | `geometry.rs` | `CheckLinevLine`, `CheckAABBvAABBCollision`, `CheckAABBvLineCollision` (two overloads), `CheckAABBvLineCollision2` |
//! | `line_of_sight.rs` | `CanHit` (four overloads), `CanHitWithCheck`, `CanHitLine`, `HitLine`, `AnyWallOfTypeOnLine`, `HitLineWall`, `EmptyTile` |
//! | `liquids.rs` | `DrownCollision`, `IsWorldPointSolid`, `GetWaterLine*`, `WetCollision`, `LavaCollision` |
//! | `slopes.rs` | `WalkDownSlope`, `SlopeCollision`, `noSlopeCollision`, `BuildTileContacts` |
//! | `tile_collision.rs` | `TileCollision`, `TileCollisionInStepsOf16`, `TryChangingSizeFromBottomCenter`, `IsClearSpotTest`, `FindCollisionTile`, `FindCollisionDirection`, `SolidCollision` (two overloads), `WaterCollision`, `AnyCollisionWithSpecificTiles`, `AnyCollision` |
//! | `tiles_misc.rs` | `HitTilesInACircle`, `HitTiles`, `AnyHurtingTiles`, `HurtTiles`, `CanTileHurt`, `SwitchTiles`, `StickyTiles`, `SolidTilesVersatile`, `SolidTiles` (four overloads), `SolidFullTiles` |
//! | `stepping.rs` | `StepDown`, `StepUp`, `InTileBounds`, `GetTileRotation`, `GetEntityEdgeTiles`, `GetTilesIn`, `ExpandVertically`, `AdvancedTileCollision`, `LaserScan`, `AimingLaserScan` |
//! | `conveyors.rs` | `ApplyConveyorBeltMovementToVelocity`, `StepConveyorBelt`, `TryFindingConveyorBeltRising` |
//!
//! The static fields of the class are [`CollisionState`] (`Game::collision`). Default arguments are written out at the
//! call; `out` / `ref` results that are not also arguments are returned as tuples (see each doc comment).
//! `Entity` / `WorldItem` / `NPCAimedTarget` / `Player` parameters that are only read for a few fields are mirrored by
//! [`EntityBox`] / [`PlayerConveyorFields`] (no entity system is introduced).

use terraria_core::xna::{Point, Vector2};

mod conveyors;
mod geometry;
mod line_of_sight;
mod liquids;
mod slopes;
mod stepping;
mod tile_collision;
mod tiles_misc;
pub mod utils;

pub use conveyors::*;
pub use geometry::*;
pub use line_of_sight::*;
pub use liquids::*;
pub use slopes::*;
pub use stepping::*;
pub use tile_collision::*;
pub use tiles_misc::*;
pub use utils::Exception;

/// `Collision.TileContactSide` (Collision.cs:11)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileContactSide {
    Left,
    Right,
    Top,
    Bottom,
    BottomLeft,
    BottomRight,
}

/// `Collision.TileContact` (Collision.cs:21)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileContact {
    /// `Side`
    pub side: TileContactSide,
    /// `Overlap`
    pub overlap: i32,
    /// `X`
    pub x: i32,
    /// `Y`
    pub y: i32,
    /// `Slope`
    pub slope: i32,
    /// `Type`
    pub type_: i32,
}

impl TileContact {
    /// `TileContact(TileContactSide side, int x, int y, int type, int slope, int overlap)` (Collision.cs:35)
    pub fn new(side: TileContactSide, x: i32, y: i32, type_: i32, slope: i32, overlap: i32) -> Self {
        Self { side, overlap, x, y, slope, type_ }
    }
}

/// `Collision.HurtTile` (Collision.cs:46). `type` is `type_`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HurtTile {
    pub type_: i32,
    pub x: i32,
    pub y: i32,
}

/// The fields of `Terraria.Entity` (also `WorldItem`, `NPCAimedTarget.Position/Width/Height`) that `Collision` reads or
/// writes: `position`, `width`, `height`. The properties of `Entity` it uses are provided below with the original's formulas.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EntityBox {
    pub position: Vector2,
    pub width: i32,
    pub height: i32,
}

impl EntityBox {
    pub fn new(position: Vector2, width: i32, height: i32) -> Self {
        Self { position, width, height }
    }
    /// `Entity.Right` getter (Entity.cs:74)
    pub fn right(&self) -> Vector2 {
        Vector2::new(self.position.x + self.width as f32, self.position.y + self.height as f32 / 2f32)
    }
    /// `Entity.Right` setter (Entity.cs:80)
    pub fn set_right(&mut self, value: Vector2) {
        self.position = Vector2::new(value.x - self.width as f32, value.y - self.height as f32 / 2f32);
    }
    /// `Entity.Bottom` getter (Entity.cs:122)
    pub fn bottom(&self) -> Vector2 {
        Vector2::new(self.position.x + self.width as f32 / 2f32, self.position.y + self.height as f32)
    }
    /// `Entity.Bottom` setter (Entity.cs:128)
    pub fn set_bottom(&mut self, value: Vector2) {
        self.position = Vector2::new(value.x - self.width as f32 / 2f32, value.y - self.height as f32);
    }
    /// `Entity.BottomLeft` getter (Entity.cs:134)
    pub fn bottom_left(&self) -> Vector2 {
        Vector2::new(self.position.x, self.position.y + self.height as f32)
    }
    /// `Entity.BottomRight` getter (Entity.cs:146)
    pub fn bottom_right(&self) -> Vector2 {
        Vector2::new(self.position.x + self.width as f32, self.position.y + self.height as f32)
    }
}

/// The `Player` fields `Collision.StepConveyorBelt` reads when the entity is a player (Collision.cs:4235, 4262).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerConveyorFields {
    /// `Player.gfxOffY`
    pub gfx_off_y: f32,
    /// `Player.grapCount`
    pub grap_count: i32,
    /// `Player.pulley`
    pub pulley: bool,
    /// `Player.onTrack`
    pub on_track: bool,
}

/// `Collision.bottomFluff` (Collision.cs:73, private, not referenced by any method)
pub const BOTTOM_FLUFF: i32 = 40;

/// The static fields of `Terraria.Collision` (Collision.cs:55-77).
pub struct CollisionState {
    /// `Collision.stair` (default false)
    pub stair: bool,
    /// `Collision.stairFall` (default false)
    pub stair_fall: bool,
    /// `Collision.honey` (default false)
    pub honey: bool,
    /// `Collision.shimmer` (default false)
    pub shimmer: bool,
    /// `Collision.sloping` (default false)
    pub sloping: bool,
    /// `Collision.landMine` (`= false`; not read or written by any method of `Collision`)
    pub land_mine: bool,
    /// `Collision.up` (default false)
    pub up: bool,
    /// `Collision.down` (default false)
    pub down: bool,
    /// `Collision.Epsilon` (`= (float)Math.E`)
    pub epsilon: f32,
    /// `Collision.contacts` (private `List<TileContact>`, empty)
    pub contacts: Vec<TileContact>,
    /// `Collision._cacheForConveyorBelts` (private `List<Point>`, empty)
    pub cache_for_conveyor_belts: Vec<Point>,
}

impl CollisionState {
    pub fn new() -> Self {
        Self {
            stair: false,
            stair_fall: false,
            honey: false,
            shimmer: false,
            sloping: false,
            land_mine: false,
            up: false,
            down: false,
            epsilon: std::f64::consts::E as f32,
            contacts: Vec::new(),
            cache_for_conveyor_belts: Vec::new(),
        }
    }
}

impl Default for CollisionState {
    fn default() -> Self {
        Self::new()
    }
}
