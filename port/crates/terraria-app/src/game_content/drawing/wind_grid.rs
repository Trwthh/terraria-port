//! `Terraria.GameContent.Drawing.WindGrid` (GameContent/Drawing/WindGrid.cs): where and when a player last brushed past a
//! tile, so grass and vines near them bend for a moment.

use terraria_sim::unported;
use terraria_sim::Game;

/// `WindGrid.WindCoord`
#[derive(Clone, Copy, Debug, Default)]
struct WindCoord {
    time: i32,
    direction_x: i32,
    direction_y: i32,
}

/// `WindGrid`
pub struct WindGrid {
    /// `_grid`, indexed `[x][y]`
    grid: Vec<Vec<WindCoord>>,
    width: i32,
    height: i32,
    game_time: i32,
}

impl WindGrid {
    /// `new WindGrid()`: `_grid = new WindCoord[1, 1]`, `_width = 1`, `_height = 1`
    pub fn new() -> Self {
        Self { grid: vec![vec![WindCoord::default(); 1]; 1], width: 1, height: 1, game_time: 0 }
    }

    /// `SetSize(int targetWidth, int targetHeight)` (WindGrid.cs:27)
    pub fn set_size(&mut self, target_width: i32, target_height: i32) {
        self.width = self.width.max(target_width);
        self.height = self.height.max(target_height);
        self.resize_grid();
    }

    /// `Update()` (WindGrid.cs:34)
    pub fn update(&mut self, g: &Game, settings_enabled_tiles_sway_in_wind: bool) {
        self.game_time += 1;
        if settings_enabled_tiles_sway_in_wind {
            self.scan_players(g);
        }
    }

    /// `GetWindTime(int tileX, int tileY, int timeThreshold, out int windTimeLeft, out int directionX, out int directionY)`
    /// (WindGrid.cs:42). Returns `(windTimeLeft, directionX, directionY)`.
    pub fn get_wind_time(&self, tile_x: i32, tile_y: i32, time_threshold: i32) -> (i32, i32, i32) {
        let wind_coord = self.grid[(tile_x % self.width) as usize][(tile_y % self.height) as usize];
        let direction_x = wind_coord.direction_x;
        let direction_y = wind_coord.direction_y;
        let wind_time_left = if wind_coord.time + time_threshold < self.game_time { 0 } else { self.game_time - wind_coord.time };
        (wind_time_left, direction_x, direction_y)
    }

    /// `ResizeGrid()` (WindGrid.cs:56)
    fn resize_grid(&mut self) {
        if self.width > self.grid.len() as i32 || self.height > self.grid[0].len() as i32 {
            self.grid = vec![vec![WindCoord::default(); self.height as usize]; self.width as usize];
        }
    }

    /// `SetWindTime(int tileX, int tileY, int directionX, int directionY)` (WindGrid.cs:64). Only `ScanPlayer` calls it.
    #[allow(dead_code)]
    fn set_wind_time(&mut self, tile_x: i32, tile_y: i32, direction_x: i32, direction_y: i32) {
        let num = (tile_x % self.width) as usize;
        let num2 = (tile_y % self.height) as usize;
        self.grid[num][num2].time = self.game_time;
        self.grid[num][num2].direction_x = direction_x;
        self.grid[num][num2].direction_y = direction_y;
    }

    /// `ScanPlayers()` (WindGrid.cs:73). `ScanPlayer(i)` reads `Main.player[i]` (position, velocity, hitbox), which is not ported;
    /// with no player nothing is brushed, so the grid stays untouched (the neutral continuation).
    fn scan_players(&mut self, g: &Game) {
        if g.main.net_mode == 0 {
            unported!("WindGrid.ScanPlayer(Main.myPlayer)");
        } else if g.main.net_mode == 1 {
            unported!("WindGrid.ScanPlayer(0..255)");
        }
    }
}

impl Default for WindGrid {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wind_time_counts_up_after_a_brush_and_expires_at_the_threshold() {
        let mut w = WindGrid::new();
        w.set_size(10, 8);
        w.game_time = 100;
        w.set_wind_time(13, 3, -1, 1); // cell (13 % 10, 3 % 8) = (3, 3)
        // right after the brush: 0 ticks elapsed
        assert_eq!(w.get_wind_time(3, 3, 20), (0, -1, 1));
        assert_eq!(w.get_wind_time(23, 11, 20), (0, -1, 1)); // the same cell
        w.game_time = 105;
        assert_eq!(w.get_wind_time(13, 3, 20), (5, -1, 1));
        // time + threshold < gameTime: expired, direction kept
        w.game_time = 121;
        assert_eq!(w.get_wind_time(13, 3, 20), (0, -1, 1));
        w.game_time = 120;
        assert_eq!(w.get_wind_time(13, 3, 20), (20, -1, 1));
    }

    #[test]
    fn set_size_only_grows() {
        let mut w = WindGrid::new();
        w.set_size(10, 8);
        w.set_size(4, 4);
        assert_eq!((w.width, w.height), (10, 8));
    }
}
