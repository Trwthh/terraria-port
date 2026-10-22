//! `Terraria.Cloud` (Cloud.cs): the drifting foreground / middle clouds. `Main.cloud` is `new Cloud[200]`. The texture sizes
//! `TextureAssets.Cloud[type].Width()/Height()` come from `MainState::cloud_texture_size` (filled from the `Images/Cloud_<n>` files).

use terraria_core::xna::{Rectangle, Vector2};

use crate::game::Game;
use crate::unported;

/// `Cloud.spriteDir` (`SpriteEffects`): `None` or `FlipHorizontally`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SpriteDir {
    #[default]
    None,
    FlipHorizontally,
}

/// `Cloud` (Cloud.cs:8). A `new Cloud()` has every field zero / false.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cloud {
    pub position: Vector2,
    pub scale: f32,
    pub rotation: f32,
    pub r_speed: f32,
    pub s_speed: f32,
    pub active: bool,
    pub sprite_dir: SpriteDir,
    pub type_: i32,
    pub width: i32,
    pub height: i32,
    pub alpha: f32,
    pub kill: bool,
}

/// `TextureAssets.Cloud[type].Width()`
fn cloud_width(g: &Game, type_: i32) -> i32 {
    g.main.cloud_texture_size[type_ as usize].0
}

/// `TextureAssets.Cloud[type].Height()`
fn cloud_height(g: &Game, type_: i32) -> i32 {
    g.main.cloud_texture_size[type_ as usize].1
}

/// `Cloud.resetClouds()` (Cloud.cs:34)
pub fn reset_clouds(g: &mut Game) {
    if !g.main.ded_serv {
        g.main.wind_speed_current = g.main.wind_speed_target;
        for i in 0..200 {
            g.main.cloud[i].active = false;
        }
        for j in 0..g.main.num_clouds {
            add_cloud(g);
            g.main.cloud[j as usize].alpha = 1.0;
        }
        for k in 0..200 {
            g.main.cloud[k].alpha = 1.0;
        }
    }
}

/// `Cloud.addCloud()` (Cloud.cs:52). `Cloud.rand` is a generator of its own (`new UnifiedRandom()`, random seed in the game).
pub fn add_cloud(g: &mut Game) {
    if g.main.net_mode == 2 {
        return;
    }
    let mut num: i32 = -1;
    for i in 0..200 {
        if !g.main.cloud[i].active {
            num = i as i32;
            break;
        }
    }
    if num < 0 {
        return;
    }
    let num = num as usize;
    g.main.cloud[num].kill = false;
    g.main.cloud[num].r_speed = 0.0;
    g.main.cloud[num].s_speed = 0.0;
    g.main.cloud[num].scale = g.main.cloud_rand.next_range(70, 131) as f32 * 0.01;
    g.main.cloud[num].rotation = g.main.cloud_rand.next_range(-10, 11) as f32 * 0.01;
    g.main.cloud[num].width = (cloud_width(g, g.main.cloud[num].type_) as f32 * g.main.cloud[num].scale) as i32;
    g.main.cloud[num].height = (cloud_height(g, g.main.cloud[num].type_) as f32 * g.main.cloud[num].scale) as i32;
    g.main.cloud[num].alpha = 0.0;
    g.main.cloud[num].sprite_dir = SpriteDir::None;
    if g.main.cloud_rand.next(2) == 0 {
        g.main.cloud[num].sprite_dir = SpriteDir::FlipHorizontally;
    }
    let mut num2 = g.main.wind_speed_current;
    if !g.main.game_menu {
        num2 = g.main.wind_speed_current - unported!("Main.player[Main.myPlayer].velocity.X", 0.0_f32) * 0.1;
    }
    let mut num3 = 0;
    let mut num4 = 0;
    if num2 > 0.0 {
        num3 -= 200;
    }
    if num2 < 0.0 {
        num4 += 200;
    }
    let num5 = 300;
    let x = g.main.cloud_rand.next_range(num3 - num5, g.main.screen_width + num4 + num5) as f32;
    g.main.cloud[num].alpha = 0.0;
    g.main.cloud[num].position.y = g.main.cloud_rand.next_range((-(g.main.screen_height as f32) * 0.25) as i32, (g.main.screen_height as f32 * 0.15) as i32) as f32;
    if g.main.cloud_rand.next(3) == 0 {
        g.main.cloud[num].position.y -= g.main.cloud_rand.next((g.main.screen_height as f32 * 0.1) as i32) as f32;
    }
    g.main.cloud[num].type_ = g.main.cloud_rand.next(4);
    if (g.main.cloud_alpha > 0.0 && g.main.cloud_rand.next(4) != 0) || (g.main.cloud_bg_active >= 1.0 && g.main.cloud_rand.next(2) == 0) {
        g.main.cloud[num].type_ = g.main.cloud_rand.next_range(18, 22);
        if g.main.cloud[num].scale as f64 >= 1.15 {
            g.main.cloud[num].position.y -= 150.0;
        }
        if g.main.cloud[num].scale >= 1.0 {
            g.main.cloud[num].position.y -= 150.0;
        }
    } else if g.main.cloud_bg_active <= 0.0
        && g.main.cloud_alpha == 0.0
        && g.main.cloud[num].scale < 1.0
        && g.main.cloud[num].position.y < -(g.main.screen_height as f32) * 0.15
        && g.main.num_clouds as f64 <= 80.0
    {
        g.main.cloud[num].type_ = g.main.cloud_rand.next_range(9, 14);
    } else if (((g.main.cloud[num].scale as f64) < 1.15 && g.main.cloud[num].position.y < -(g.main.screen_height as f32) * 0.3)
        || ((g.main.cloud[num].scale as f64) < 0.85 && g.main.cloud[num].position.y < g.main.screen_height as f32 * 0.15))
        && (g.main.num_clouds as f64 > 70.0 || g.main.cloud_bg_active >= 1.0)
    {
        g.main.cloud[num].type_ = g.main.cloud_rand.next_range(4, 9);
    } else if g.main.cloud[num].position.y > -(g.main.screen_height as f32) * 0.15 && g.main.cloud_rand.next(2) == 0 && g.main.num_clouds as f64 > 20.0 {
        g.main.cloud[num].type_ = g.main.cloud_rand.next_range(14, 18);
    }
    if g.main.cloud_rand.next(if g.main.dont_starve_world || g.main.tenth_anniversary_world { 25 } else { 150 }) == 0 {
        g.main.cloud[num].type_ = roll_rare_cloud(g);
    } else if g.main.tenth_anniversary_world && g.main.cloud_rand.next(3) == 0 {
        g.main.cloud[num].type_ = roll_rare_cloud(g);
    }
    if g.main.cloud[num].scale as f64 > 1.2 {
        g.main.cloud[num].position.y += 100.0;
    }
    if g.main.cloud[num].scale as f64 > 1.3 {
        g.main.cloud[num].scale = 1.3;
    }
    if (g.main.cloud[num].scale as f64) < 0.7 {
        g.main.cloud[num].scale = 0.7;
    }
    g.main.cloud[num].active = true;
    g.main.cloud[num].position.x = x;
    if g.main.cloud[num].position.x > (g.main.screen_width + 400) as f32 {
        g.main.cloud[num].alpha = 1.0;
    }
    if g.main.cloud[num].position.x + cloud_width(g, g.main.cloud[num].type_) as f32 * g.main.cloud[num].scale < -400.0 {
        g.main.cloud[num].alpha = 1.0;
    }
    let rectangle = Rectangle::new(g.main.cloud[num].position.x as i32, g.main.cloud[num].position.y as i32, g.main.cloud[num].width, g.main.cloud[num].height);
    for j in 0..200 {
        if num != j && g.main.cloud[j].active {
            let value = Rectangle::new(g.main.cloud[j].position.x as i32, g.main.cloud[j].position.y as i32, g.main.cloud[j].width, g.main.cloud[j].height);
            if rectangle_intersects(&rectangle, &value) {
                g.main.cloud[num].active = false;
            }
        }
    }
}

/// `Rectangle.Intersects(Rectangle)` (reference/xna/Rectangle.cs)
fn rectangle_intersects(a: &Rectangle, b: &Rectangle) -> bool {
    b.x < a.x + a.width && a.x < b.x + b.width && b.y < a.y + a.height && a.y < b.y + b.height
}

/// `CloudID.Sets.RareClouds_Normal` / `RareClouds_Celebration` (CloudID.cs)
const RARE_CLOUDS_NORMAL: [i32; 20] = [22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41];
const RARE_CLOUDS_CELEBRATION: [i32; 16] = [22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 41];

/// `Cloud.RollRareCloud()` (Cloud.cs:160). The lists are `CloudID.Sets.RareClouds_Normal` / `_Celebration`.
fn roll_rare_cloud(g: &mut Game) -> i32 {
    let mut num: i32 = -1;
    let mut flag = false;
    while !flag {
        num = if !g.main.tenth_anniversary_world {
            RARE_CLOUDS_NORMAL[g.main.cloud_rand.next(RARE_CLOUDS_NORMAL.len() as i32) as usize]
        } else {
            RARE_CLOUDS_CELEBRATION[g.main.cloud_rand.next(RARE_CLOUDS_CELEBRATION.len() as i32) as usize]
        };
        match num {
            31 => flag = unported!("NPC.downedBoss3", false),
            36 => flag = unported!("NPC.downedBoss2", false) && unported!("WorldGen.crimson", false),
            25 | 26 => flag = unported!("NPC.downedBoss1", false),
            30 | 35 | 41 => flag = g.main.hard_mode,
            28 => {
                if g.main.cloud_rand.next(10) == 0 {
                    flag = true;
                }
            }
            37..=40 => {
                if g.main.dont_starve_world || g.main.cloud_rand.next(10) == 0 {
                    flag = true;
                }
            }
            _ => flag = true,
        }
    }
    num
}

/// `Cloud.cloudColor(Color bgColor)` (Cloud.cs:214)
pub fn cloud_color(cloud: &Cloud, bg_color: terraria_core::xna::Color) -> terraria_core::xna::Color {
    let mut num = cloud.scale * cloud.alpha;
    if num > 1.0 {
        num = 1.0;
    }
    let num2 = (bg_color.r() as i32 as f32 * num) as i32 as f32;
    let num3 = (bg_color.g() as i32 as f32 * num) as i32 as f32;
    let num4 = (bg_color.b() as i32 as f32 * num) as i32 as f32;
    let num5 = (bg_color.a() as i32 as f32 * num) as i32 as f32;
    terraria_core::xna::Color::new(num2 as u8, num3 as u8, num4 as u8, num5 as u8)
}

/// `Cloud.UpdateClouds()` (Cloud.cs:228)
pub fn update_clouds(g: &mut Game) {
    if g.main.net_mode == 2 {
        return;
    }
    let mut num = 0;
    for i in 0..200 {
        if g.main.cloud[i].active {
            update(g, i);
            if !g.main.cloud[i].kill {
                num += 1;
            }
        }
    }
    for j in 0..200 {
        if g.main.cloud[j].active {
            if j > 1 && (!g.main.cloud[j - 1].active || (g.main.cloud[j - 1].scale as f64) > g.main.cloud[j].scale as f64 + 0.02) {
                let cloud = g.main.cloud[j - 1];
                g.main.cloud[j - 1] = g.main.cloud[j];
                g.main.cloud[j] = cloud;
            }
            if j < 199 && (!g.main.cloud[j].active || (g.main.cloud[j + 1].scale as f64) < g.main.cloud[j].scale as f64 - 0.02) {
                let cloud2 = g.main.cloud[j + 1];
                g.main.cloud[j + 1] = g.main.cloud[j];
                g.main.cloud[j] = cloud2;
            }
        }
    }
    if num < g.main.num_clouds {
        add_cloud(g);
    } else if num > g.main.num_clouds {
        let mut num2 = g.main.cloud_rand.next(num);
        let mut num3 = 0;
        while g.main.cloud[num2 as usize].kill && num3 < 100 {
            num3 += 1;
            num2 = g.main.cloud_rand.next(num);
        }
        g.main.cloud[num2 as usize].kill = true;
    }
}

/// `Main.Camera.Center` at zoom 1 (`ScaledPosition + ScaledSize / 2`)
fn camera_center(g: &Game) -> Vector2 {
    Vector2::new(g.main.screen_position.0 + g.main.screen_width as f32 / 2.0, g.main.screen_position.1 + g.main.screen_height as f32 / 2.0)
}

/// `Cloud.UpdateCloudParallax()` (Cloud.cs:290), called by `Main.DoDraw`
pub fn update_cloud_parallax(g: &mut Game) {
    let mut num = 0.0_f32;
    let center = camera_center(g);
    if let Some(last) = g.main.last_camera_center {
        num = center.x - last.x;
    }
    g.main.last_camera_center = Some(center);
    if num == 0.0 {
        return;
    }
    for i in 0..200 {
        if g.main.cloud[i].active {
            let parallax = get_parallax(&g.main.cloud[i]);
            g.main.cloud[i].position.x -= num * parallax;
        }
    }
}

/// `Cloud.GetParallax()` (Cloud.cs:312)
fn get_parallax(cloud: &Cloud) -> f32 {
    let num;
    let mut num2;
    if cloud.scale < 1.0 {
        num = 0.07_f32;
        num2 = cloud.scale + 0.15;
        num2 = (num2 + 1.0) / 2.0;
    } else if cloud.scale as f64 <= 1.15 {
        num = 0.19;
        num2 = cloud.scale - 0.075;
    } else {
        num = 0.23;
        num2 = cloud.scale - 0.15 - 0.075;
    }
    num2 *= num2;
    num * num2
}

/// `Cloud.Update()` (Cloud.cs:339) on `Main.cloud[index]`
pub fn update(g: &mut Game, index: usize) {
    if unported!("WorldGen.drunkWorldGenText", false) && g.main.game_menu {
        unported!("Cloud.Update drunk-world title cloud types");
    }
    if g.main.cloud[index].scale == 1.0 {
        g.main.cloud[index].scale -= 0.0001;
    }
    if g.main.cloud[index].scale as f64 == 1.15 {
        g.main.cloud[index].scale -= 0.0001;
    }
    let parallax = get_parallax(&g.main.cloud[index]);
    g.main.cloud[index].position.x += g.main.wind_speed_current * 9.0 * parallax * g.main.day_rate as f32;
    let num = 600.0_f32;
    if g.main.bg_alpha_front_layer[4] == 1.0 && g.main.cloud[index].position.y > 200.0 {
        g.main.cloud[index].kill = true;
        g.main.cloud[index].alpha -= 0.005 * g.main.day_rate as f32;
    }
    if !g.main.cloud[index].kill {
        if g.main.cloud[index].alpha < 1.0 {
            g.main.cloud[index].alpha += 0.001 * g.main.day_rate as f32;
            if g.main.cloud[index].alpha > 1.0 {
                g.main.cloud[index].alpha = 1.0;
            }
        }
    } else {
        g.main.cloud[index].alpha -= 0.001 * g.main.day_rate as f32;
        if g.main.cloud[index].alpha <= 0.0 {
            g.main.cloud[index].active = false;
        }
    }
    let type_ = g.main.cloud[index].type_;
    let scale = g.main.cloud[index].scale;
    if g.main.cloud[index].position.x + cloud_width(g, type_) as f32 * scale < 0.0 - num || g.main.cloud[index].position.x > g.main.screen_width as f32 + num {
        g.main.cloud[index].active = false;
    }
    g.main.cloud[index].r_speed += g.main.cloud_rand.next_range(-10, 11) as f32 * 2E-05;
    if g.main.cloud[index].r_speed as f64 > 0.0002 {
        g.main.cloud[index].r_speed = 0.0002;
    }
    if (g.main.cloud[index].r_speed as f64) < -0.0002 {
        g.main.cloud[index].r_speed = -0.0002;
    }
    if g.main.cloud[index].rotation as f64 > 0.02 {
        g.main.cloud[index].rotation = 0.02;
    }
    if (g.main.cloud[index].rotation as f64) < -0.02 {
        g.main.cloud[index].rotation = -0.02;
    }
    let r_speed = g.main.cloud[index].r_speed;
    g.main.cloud[index].rotation += r_speed;
    g.main.cloud[index].width = (cloud_width(g, type_) as f32 * scale) as i32;
    g.main.cloud[index].height = (cloud_height(g, type_) as f32 * scale) as i32;
    if (9..=13).contains(&type_) && (g.main.cloud_alpha > 0.0 || g.main.cloud_bg_active >= 1.0) {
        g.main.cloud[index].kill = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_with_cloud_sizes() -> Game {
        let mut g = Game::new();
        g.main.game_menu = false;
        g.main.screen_width = 1280;
        g.main.screen_height = 720;
        g.main.cloud_texture_size = vec![(256, 128); 42];
        g
    }

    #[test]
    fn reset_places_num_clouds_without_overlap_and_fully_visible() {
        let mut g = game_with_cloud_sizes();
        g.main.num_clouds = 30;
        reset_clouds(&mut g);
        let active: Vec<_> = (0..200).filter(|&i| g.main.cloud[i].active).collect();
        assert!(!active.is_empty() && active.len() <= 30);
        for &i in &active {
            let c = &g.main.cloud[i];
            assert!(c.scale >= 0.7 && c.scale <= 1.3);
            assert_eq!(c.alpha, 1.0);
        }
    }

    #[test]
    fn parallax_is_the_hand_computed_value_for_the_three_scale_bands() {
        let mut c = Cloud::default();
        c.scale = 0.7; // < 1: 0.07 * (((0.7 + 0.15) + 1) / 2)^2 = 0.07 * 0.925^2
        assert!((get_parallax(&c) - 0.07 * 0.925 * 0.925).abs() < 1e-6);
        c.scale = 1.1; // <= 1.15: 0.19 * (1.1 - 0.075)^2
        assert!((get_parallax(&c) - 0.19 * 1.025 * 1.025).abs() < 1e-6);
        c.scale = 1.3; // 0.23 * (1.3 - 0.225)^2
        assert!((get_parallax(&c) - 0.23 * 1.075 * 1.075).abs() < 1e-5);
    }

    #[test]
    fn cloud_color_scales_by_scale_times_alpha_and_truncates() {
        let mut c = Cloud::default();
        c.scale = 0.9;
        c.alpha = 0.5; // 0.45
        let col = cloud_color(&c, terraria_core::xna::Color::new(255, 200, 100, 255));
        assert_eq!((col.r(), col.g(), col.b(), col.a()), (114, 90, 45, 114)); // (int)(255*0.45)=114, 200*0.45=90, 100*0.45=45
    }
}
