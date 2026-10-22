// The probe run, part B: tools/refdump_draw/ProbeB.cs, step for step. Included by tests/draw.rs.

impl<T: Copy + Pv> Pv for Nullable<T> {
    fn pv(&self) -> String {
        if self.has_value {
            self.value.pv()
        } else {
            "-".to_owned()
        }
    }
}

type V2 = terraria_core::xna::Vector2;
fn v2(x: f32, y: f32) -> V2 {
    V2 { x, y }
}
fn rect(x: i32, y: i32, width: i32, height: i32) -> terraria_core::xna::Rectangle {
    terraria_core::xna::Rectangle { x, y, width, height }
}
fn rgb(r: u8, g: u8, b: u8) -> terraria_core::xna::Color {
    terraria_core::xna::Color::new(r, g, b, 255)
}

/// Probe.cs `PFx`.
unsafe fn probe_fx(r: &mut Run, key: &str) {
    fn i4(mut h: u64, v: i32) -> u64 {
        for k in 0..4 {
            h ^= ((v >> (8 * k)) & 0xff) as u64;
            h = h.wrapping_mul(1099511628211);
        }
        h
    }
    let fb = |f: f32| f.to_bits() as i32;
    let m = terraria::main::s();
    let (mut dh, mut gh) = (14695981039346656037u64, 14695981039346656037u64);
    let (mut dn, mut gn) = (0, 0);
    for (i, d) in (*m).dust.as_slice().iter().enumerate() {
        if d.is_null() || !(*d.ptr()).active {
            continue;
        }
        dn += 1;
        let d = &*d.ptr();
        dh = i4(i4(i4(i4(i4(i4(i4(i4(i4(dh, i as i32), d.type_), fb(d.position.x)), fb(d.position.y)), fb(d.velocity.x)), fb(d.velocity.y)), fb(d.scale)), fb(d.rotation)), d.alpha);
    }
    for (i, g) in (*m).gore.as_slice().iter().enumerate() {
        if g.is_null() || !(*g.ptr()).active {
            continue;
        }
        gn += 1;
        let g = &*g.ptr();
        gh = i4(i4(i4(i4(i4(i4(i4(i4(i4(gh, i as i32), g.type_), fb(g.position.x)), fb(g.position.y)), fb(g.velocity.x)), fb(g.velocity.y)), fb(g.rotation)), fb(g.scale)), g.alpha);
    }
    r.state.push_str(&format!("probe {key} fx dust {dn}:{dh:016x} gore {gn}:{gh:016x}\n"));
}
/// Probe.cs `ClearFx`.
unsafe fn clear_fx() {
    let m = terraria::main::s();
    for d in (*m).dust.as_slice() {
        if !d.is_null() {
            (*d.ptr()).active = false;
        }
    }
    for g in (*m).gore.as_slice() {
        if !g.is_null() {
            (*g.ptr()).active = false;
        }
    }
}
/// ProbeB.cs `NewDustAt`.
unsafe fn new_dust_at(x: f32, y: f32, ty: i32) -> Gc<terraria::dust::Dust> {
    let i = terraria::dust::new_dust(v2(x, y), 10, 10, ty, 0.0, 0.0, 0, rt::zero(), 1.0);
    *(*terraria::main::s()).dust.at(i)
}

/// ProbeB.cs `ProbeB`.
unsafe fn probe_b(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    let m = terraria::main::s();
    let pl = local_player();
    let pp = (*pl.ptr()).base.position;
    // ---- audio
    if probe_group(r, "audio") {
        use terraria::audio::a_sound_effect_based_audio_track as asb;
        let mut track = std::mem::ManuallyDrop::new(std::mem::zeroed::<asb::ASoundEffectBasedAudioTrack>());
        for v in [0.0f32, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
            let res = asb::re_map_volume_to_match_xact(&mut *track, v);
            r.probe("remap", &[&v, &res]);
        }
        let samples = [0.5f32, -0.25, 1.0, -1.0, 0.123456, 0.999, -0.0001, 0.33333334];
        let sa = Arr::<f32>::new(samples.len() as i32);
        for (k, s) in samples.iter().enumerate() {
            *sa.at(k as i32) = *s;
        }
        let pcm = Arr::<u8>::new(samples.len() as i32 * 2);
        terraria::audio::ogg_audio_track::apply_temporary_buffer_to(sa, pcm);
        let vals: Vec<u8> = (0..pcm.len()).map(|k| *pcm.at(k)).collect();
        let refs: Vec<&dyn Pv> = vals.iter().map(|v| v as &dyn Pv).collect();
        r.probe("pcm", &refs);
        let aus = (*m).audio_system;
        assert!(rt::obj_is::<terraria::audio::legacy_audio_system::LegacyAudioSystem>(aus), "the audio system is not a LegacyAudioSystem");
        r.note("audio system: LegacyAudioSystem".to_owned());
        let las = aus.0 as *mut terraria::audio::legacy_audio_system::LegacyAudioSystem;
        terraria::audio::legacy_audio_system::pause_all(las);
        terraria::audio::legacy_audio_system::resume_all(las);
        r.frames(2);
    }
    // ---- chests
    if probe_group(r, "chest") {
        let cx = x0 + 6;
        let ok = place_tile(cx, gy - 1, 21, 1);
        r.note(format!("chest: {}", cs_bool(ok)));
        r.probe_tiles("chest_placed", cx - 1, gy - 3, 4, 4);
        let ok = terraria::chest::lock(cx, gy - 2);
        r.probe("chest_lock", &[&ok]);
        r.probe_tiles("chest_locked", cx - 1, gy - 3, 4, 4);
        let ok = terraria::chest::unlock(cx, gy - 2);
        r.probe("chest_unlock", &[&ok]);
        r.probe_tiles("chest_unlocked", cx - 1, gy - 3, 4, 4);
        let ci = terraria::chest::find_chest(cx, gy - 2);
        r.note(format!("chest index {ci}"));
        place_player(cx - 4, gy - 1);
        r.frames(2);
        terraria::chest::indicate_blocked_chest(ci);
        let settings = (*terraria::chest::item_transfer_visualization_settings::s()).player_to_chest;
        terraria::chest::visualize_chest_transfer_coins_batch(v2(pp.x, pp.y), v2(cx as f32 * 16.0 + 16.0, gy as f32 * 16.0 - 16.0), 1234567, settings);
        r.frames(10);
        r.record("chest_fx", 1);
        r.frames(60);
        kill_tile(cx, gy - 1);
        r.frames(3);
    }
    // ---- Collision.HitTilesInACircle
    if probe_group(r, "collision") {
        clear_fx();
        place_player(x0 + 20, gy - 1);
        terraria::collision::hit_tiles_in_a_circle(v2((x0 + 10) as f32 * 16.0, gy as f32 * 16.0 - 20.0), v2(3.0, 6.0), 48, 48);
        probe_fx(r, "hitcircle");
        r.frames(5);
        r.record("hitcircle", 1);
    }
    // ---- the delegate methods
    if probe_group(r, "delegates") {
        use terraria::delegate_methods as dm;
        for laser in ["RainbowLaserDraw", "TurretLaserDraw"] {
            for stage in 0..3 {
                let (mut dist, mut frame, mut origin, mut color) = (0.0f32, rect(0, 0, 0, 0), v2(0.0, 0.0), terraria_core::xna::Color::new(0, 0, 0, 0));
                let f = if laser == "RainbowLaserDraw" { dm::rainbow_laser_draw } else { dm::turret_laser_draw };
                f(stage, v2(100.0, 200.0), 37.5, rect(0, 0, 26, 22), &mut dist, &mut frame, &mut origin, &mut color);
                r.probe(&format!("laser_{laser}_{stage}"), &[&dist, &frame, &origin, &color]);
            }
        }
        let wx = x0 + 30;
        for x in wx..wx + 6 {
            kill_tile(x, gy);
            kill_tile(x, gy + 1);
            for y in [gy, gy + 1] {
                let t = gallery_tile(x, y);
                (*t.ptr()).liquid = 255;
                terraria::tile::liquid_type__int(t.ptr(), 0);
            }
        }
        (*dm::s()).v2_1 = v2((wx + 3) as f32, gy as f32);
        (*dm::s()).f_1 = 3.5;
        clear_fx();
        for x in wx - 1..wx + 8 {
            for y in gy - 1..=gy + 2 {
                let ok = dm::spread_dry(x, y);
                r.probe(&format!("spreaddry_{x}_{y}"), &[&ok]);
            }
        }
        r.probe_tiles("spreaddry", wx - 1, gy - 1, 9, 4);
        probe_fx(r, "spreaddry");
        for x in wx..wx + 6 {
            place_tile(x, gy, 0, 0);
            place_tile(x, gy + 1, 0, 0);
        }
        // pet previews
        use terraria::delegate_methods::character_preview as cp;
        let previews: [(&str, i32, unsafe fn(Gc<terraria::projectile::Projectile>, bool)); 10] = [
            ("WormPet", 883, cp::worm_pet),
            ("WormPet", 887, cp::worm_pet),
            ("WormPet", 893, cp::worm_pet),
            ("Float", 175, cp::float),
            ("FloatAndSpinWhenWalking", 197, cp::float_and_spin_when_walking),
            ("SlimePet", 881, cp::slime_pet),
            ("BerniePet", 956, cp::bernie_pet),
            ("EtsyPet", 764, cp::etsy_pet),
            ("CompanionCubePet", 653, cp::companion_cube_pet),
            ("SpinWhenWalking", 1056, cp::spin_when_walking),
        ];
        for (name, ty, f) in previews {
            for walking in [false, true] {
                let pr = terraria::projectile::new();
                terraria::projectile::set_defaults(pr.ptr(), ty);
                (*pr.ptr()).base.position = v2(300.0, 400.0);
                f(pr, walking);
                let p = &*pr.ptr();
                let mut vals: Vec<Box<dyn Pv>> = vec![Box::new(p.base.position), Box::new(p.rotation), Box::new(p.frame), Box::new(p.sprite_direction)];
                for k in 0..p.old_pos.len() {
                    vals.push(Box::new(*p.old_pos.at(k)));
                }
                let refs: Vec<&dyn Pv> = vals.iter().map(|b| b.as_ref()).collect();
                r.probe(&format!("preview_{name}_{ty}_{}", if walking { "True" } else { "False" }), &refs);
            }
        }
        // minecart effects
        use terraria::delegate_methods::minecart as mc;
        place_player(x0 + 20, gy - 1);
        clear_fx();
        let sparks: [(&str, unsafe fn(V2)); 5] = [("Sparks", mc::sparks), ("SparksFart", mc::sparks_fart), ("SparksMech", mc::sparks_mech), ("SparksMeow", mc::sparks_meow), ("SparksTerraFart", mc::sparks_terra_fart)];
        for (name, f) in sparks {
            for k in 0..6 {
                f(v2(pp.x + 8.0 * k as f32, pp.y + 30.0));
            }
            probe_fx(r, &format!("minecart_{name}"));
        }
        let sounds: [(&str, unsafe fn(Gc<terraria::player::Player>, V2, i32, i32)); 5] =
            [("LandingSound", mc::landing_sound), ("BumperSound", mc::bumper_sound), ("LandingSoundFart", mc::landing_sound_fart), ("BumperSoundFart", mc::bumper_sound_fart), ("JumpingSoundFart", mc::jumping_sound_fart)];
        for (name, f) in sounds {
            (*pl.ptr()).fart_kart_cloud_delay = 0;
            f(pl, v2(pp.x, pp.y), 20, 42);
            probe_fx(r, &format!("minecart_{name}"));
        }
        (*pl.ptr()).fart_kart_cloud_delay = 0;
        mc::spawn_fart_cloud(pl, v2(pp.x, pp.y), 20, 42, true);
        probe_fx(r, "minecart_fartcloud");
        r.frames(3);
        r.record("minecart_fx", 1);
        // mount delegates
        use terraria::delegate_methods::mount as md;
        let dashes: [(&str, unsafe fn(Gc<terraria::player::Player>, i32, Gc<terraria::dust::Dust>) -> Gc<terraria::dust::Dust>); 2] = [("BatDashDust", md::bat_dash_dust), ("PixieDashDust", md::pixie_dash_dust)];
        for (name, f) in dashes {
            for k in 0..2 {
                let d = new_dust_at(pp.x, pp.y, 6);
                let back = f(pl, k, d);
                let same = back == d;
                r.probe(&format!("dashdust_{name}_{k}"), &[&same, &(*d.ptr()).active, &(*d.ptr()).position, &(*d.ptr()).scale]);
            }
        }
        let mouths: [(&str, unsafe fn(Gc<terraria::player::Player>, *mut Nullable<V2>) -> bool); 2] = [("WolfMouthPosition", md::wolf_mouth_position), ("VelociraptorMouthPosition", md::velociraptor_mouth_position)];
        for (name, f) in mouths {
            let mut pos: Nullable<V2> = rt::zero();
            let ok = f(pl, &mut pos);
            r.probe(&format!("mouth_{name}"), &[&ok, &pos]);
        }
        r.frames(3);
        clear_fx();
    }
    // ---- Dust.QuickBox, Entity.AngleFrom
    if probe_group(r, "dustbox") {
        clear_fx();
        terraria::dust::quick_box(v2(pp.x - 40.0, pp.y - 60.0), v2(pp.x + 40.0, pp.y + 10.0), 6, rgb(255, 120, 40), 1.5, 0.8, rt::zero());
        probe_fx(r, "quickbox");
        r.frames(2);
        r.record("quickbox", 1);
        for src in [v2(0.0, 0.0), v2(pp.x + 100.0, pp.y - 33.0), v2(pp.x - 7.0, pp.y + 250.0)] {
            let a = terraria::entity::angle_from(&mut (*pl.ptr()).base, src);
            r.probe("anglefrom", &[&src, &a]);
        }
    }
}
