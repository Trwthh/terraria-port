//! `Terraria.Star` (Star.cs): the stars of the night sky. `Main.star` is `new Star[400]`, `Main.numStars` the number in use.

use terraria_core::xna::Vector2;

use crate::game::Game;
use crate::light_support::FastRandom;
use crate::unported;

/// `Star` (Star.cs:7). A `new Star()` has every field zero / false.
#[derive(Clone, Copy, Debug, Default)]
pub struct Star {
    pub position: Vector2,
    pub scale: f32,
    pub rotation: f32,
    pub type_: i32,
    pub twinkle: f32,
    pub twinkle_speed: f32,
    pub rotation_speed: f32,
    pub falling: bool,
    pub hidden: bool,
    pub fall_speed: Vector2,
    pub fall_time: i32,
    pub velocity: Vector2,
    pub fade_in: f32,
}

/// `Star.NightSetup()` (Star.cs:44)
pub fn night_setup(g: &mut Game) {
    g.main.starfall_boost = 1.0;
    let mut max_value = 10;
    let mut max_value2 = 3;
    if g.main.tenth_anniversary_world {
        max_value = 5;
        max_value2 = 2;
    }
    if g.main.rand.next(max_value) == 0 {
        g.main.starfall_boost = g.main.rand.next_range(300, 501) as f32 * 0.01;
    } else if g.main.rand.next(max_value2) == 0 {
        g.main.starfall_boost = g.main.rand.next_range(100, 151) as f32 * 0.01;
    }
    g.main.star_fall_count = 0;
}

/// `Star.StarFall(float positionX)` (Star.cs:63)
pub fn star_fall(g: &mut Game, position_x: f32) {
    g.main.star_fall_count += 1;
    let mut num: i32 = -1;
    let mut num2 = -1.0_f32;
    let num3 = position_x / g.main.right_world * 1920.0_f32; // Main.MaxWorldViewSize.X
    for i in 0..g.main.num_stars as usize {
        if !g.main.star[i].hidden && !g.main.star[i].falling {
            let num4 = (g.main.star[i].position.x - num3).abs();
            if num2 == -1.0 || num4 < num2 {
                num = i as i32;
                num2 = num4;
            }
        }
    }
    if num >= 0 {
        fall(g, num as usize);
    }
}

/// `Star.SpawnStars(int s = -1)` (Star.cs:83). `FastRandom.CreateWithRandomSeed()` makes the layout differ per run, as in the game.
pub fn spawn_stars(g: &mut Game, s: i32) {
    let mut fast_random = FastRandom::create_with_random_seed();
    spawn_stars_with(g, s, &mut fast_random);
}

/// The body of `Star.SpawnStars` with the random generator passed in (so a test can fix the seed).
pub fn spawn_stars_with(g: &mut Game, s: i32, fast_random: &mut FastRandom) {
    let num = fast_random.next_range(200, 400);
    let mut num2 = 0;
    let mut num3 = num;
    if s >= 0 {
        num2 = s;
        num3 = s + 1;
    }
    for i in num2..num3 {
        let i = i as usize;
        g.main.star[i] = Star::default();
        if s >= 0 {
            g.main.star[i].fade_in = 1.0;
            let num4 = 10;
            let mut num5 = -2000;
            for _j in 0..num4 {
                let num6 = fast_random.next(1921) as f32;
                let mut num7 = 2000;
                for k in 0..g.main.num_stars as usize {
                    if k as i32 != s && !g.main.star[k].hidden && !g.main.star[k].falling {
                        let num8 = (num6 - g.main.star[k].position.x).abs() as i32;
                        if num8 < num7 {
                            num7 = num8;
                        }
                    }
                }
                if s == 0 || num7 > num5 {
                    num5 = num7;
                    g.main.star[i].position.x = num6;
                }
            }
        } else {
            g.main.star[i].position.x = fast_random.next(1921) as f32;
        }
        g.main.star[i].position.y = fast_random.next(1201) as f32;
        g.main.star[i].rotation = fast_random.next(628) as f32 * 0.01;
        g.main.star[i].scale = fast_random.next_range(70, 130) as f32 * 0.006;
        g.main.star[i].type_ = fast_random.next_range(0, 4);
        g.main.star[i].twinkle = fast_random.next_range(60, 101) as f32 * 0.01;
        g.main.star[i].twinkle_speed = fast_random.next_range(30, 110) as f32 * 0.0001;
        g.main.star[i].velocity = g.main.star[i].velocity * 0.0;
        if fast_random.next(2) == 0 {
            g.main.star[i].twinkle_speed *= -1.0;
        }
        g.main.star[i].rotation_speed = fast_random.next_range(5, 50) as f32 * 0.0001;
        if fast_random.next(2) == 0 {
            g.main.star[i].rotation_speed *= -1.0;
        }
        if fast_random.next(40) == 0 {
            g.main.star[i].scale *= 2.0;
            g.main.star[i].twinkle_speed /= 2.0;
            g.main.star[i].rotation_speed /= 2.0;
        }
    }
    if s == -1 {
        g.main.num_stars = num;
    }
}

/// `Star.Fall()` (Star.cs:162) on `Main.star[index]`
pub fn fall(g: &mut Game, index: usize) {
    if !unported!("WorldGen.SecretSeed.anySecretSeedIsActive", false) || unported!("Main.starGame", false) {
        g.main.star[index].fall_time = 0;
        g.main.star[index].falling = true;
        g.main.star[index].fall_speed.y = g.main.rand.next_range(700, 1001) as f32 * 0.01;
        g.main.star[index].fall_speed.x = g.main.rand.next_range(-400, 401) as f32 * 0.01;
    }
}

/// `Star.Update()` (Star.cs:173) on `Main.star[index]`
pub fn update(g: &mut Game, index: usize) {
    let day_rate = g.main.day_rate;
    let star_game = unported!("Main.starGame", false);
    let s = &mut g.main.star[index];
    if s.falling && !s.hidden {
        s.fall_time += day_rate;
        s.position = s.position + s.fall_speed * (day_rate + 99) as f32 / 100.0;
        if s.position.y > 1500.0 {
            s.hidden = true;
        }
        if star_game && s.position.length() > 99999.0 {
            s.hidden = true;
        }
        s.twinkle += s.twinkle_speed * 3.0;
        if s.twinkle > 1.0 {
            s.twinkle = 1.0;
            s.twinkle_speed *= -1.0;
        } else if (s.twinkle as f64) < 0.6 {
            s.twinkle = 0.6;
            s.twinkle_speed *= -1.0;
        }
        s.rotation += 0.5;
        if (s.rotation as f64) > 6.28 {
            s.rotation -= 6.28;
        }
        if s.rotation < 0.0 {
            s.rotation += 6.28;
        }
        return;
    }
    if s.fade_in > 0.0 {
        let mut num = 6.1728395E-05_f32 * day_rate as f32;
        num *= 10.0;
        s.fade_in -= num;
        if s.fade_in < 0.0 {
            s.fade_in = 0.0;
        }
    }
    s.twinkle += s.twinkle_speed;
    if s.twinkle > 1.0 {
        s.twinkle = 1.0;
        s.twinkle_speed *= -1.0;
    } else if (s.twinkle as f64) < 0.6 {
        s.twinkle = 0.6;
        s.twinkle_speed *= -1.0;
    }
    s.rotation += s.rotation_speed;
    if (s.rotation as f64) > 6.28 {
        s.rotation -= 6.28;
    }
    if s.rotation < 0.0 {
        s.rotation += 6.28;
    }
}

/// `Star.UpdateStars()` (Star.cs:259). `Main.onlyDrawFancyUI` is false in play.
pub fn update_stars(g: &mut Game) {
    if !g.main.day_time {
        g.main.star_day_check = false;
    } else if !g.main.star_day_check && g.main.time >= 27000.0 {
        for i in 0..g.main.num_stars {
            if g.main.star[i as usize].hidden {
                spawn_stars(g, i);
            }
        }
    }
    for j in 0..g.main.num_stars as usize {
        update(g, j);
    }
    if !g.main.game_menu || !g.world_gen.generating_world || !unported!("WorldGen.SecretSeed.anySecretSeedIsActive", false) {
        return;
    }
    unported!("Star.UpdateStars secret-seed star gravity");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_creates_between_200_and_399_stars_inside_the_virtual_screen() {
        let mut g = Game::new();
        let mut rng = FastRandom::new(12345);
        spawn_stars_with(&mut g, -1, &mut rng);
        assert!((200..400).contains(&g.main.num_stars));
        for i in 0..g.main.num_stars as usize {
            let s = &g.main.star[i];
            assert!(s.position.x >= 0.0 && s.position.x <= 1920.0 && s.position.y >= 0.0 && s.position.y <= 1200.0);
            assert!(s.twinkle >= 0.599 && s.twinkle <= 1.001);
            assert!(s.type_ >= 0 && s.type_ < 4);
        }
    }

    #[test]
    fn twinkle_bounces_between_0_6_and_1() {
        let mut g = Game::new();
        g.main.num_stars = 1;
        g.main.star[0].twinkle = 0.99;
        g.main.star[0].twinkle_speed = 0.02;
        update(&mut g, 0);
        assert_eq!(g.main.star[0].twinkle, 1.0);
        assert_eq!(g.main.star[0].twinkle_speed, -0.02);
    }
}
