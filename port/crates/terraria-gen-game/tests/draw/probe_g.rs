// The probe run, part G: tools/refdump_draw/ProbeG.cs, step for step. Included by tests/draw.rs.

impl Pv for terraria_core::xna::Vector3 {
    fn pv(&self) -> String {
        format!("{},{},{}", self.x.pv(), self.y.pv(), self.z.pv())
    }
}

/// ProbeG.cs `MouseEvt`.
unsafe fn mouse_evt(target: Gc<terraria::ui::ui_element::UIElement>) -> Gc<terraria::ui::ui_mouse_event::UIMouseEvent> {
    terraria::ui::ui_mouse_event::new(target, v2(640.0, 360.0))
}
unsafe fn fancy(state: Gc<terraria::ui::ui_state::UIState>) {
    terraria::ui::ingame_fancy_ui::open_ui_state__ui_state_bool(state, true);
}
unsafe fn fancy_close() {
    terraria::ui::ingame_fancy_ui::close(false);
}

/// ProbeG.cs `ProbeG`.
unsafe fn probe_g(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    let m = terraria::main::s();
    let pl = local_player();
    // ---- character creation
    if probe_group(r, "charcreate2") {
        use terraria::game_content::ui::states::ui_character_creation as ucc;
        let np = terraria::player::new();
        let cc = ucc::new(np);
        let _roots = (rt::Rooted::new(np), rt::Rooted::new(cc));
        fancy(cc.up());
        r.frames(5);
        for c in 0..10 {
            ucc::select_color_picker(cc.ptr(), c);
            r.frames(2);
            if c % 3 == 0 {
                r.record(&format!("charcreate2_picker_{c}"), 1);
            }
        }
        let pickers = (*cc.ptr())._color_pickers;
        for i in 0..pickers.len() {
            let p = *pickers.at(i);
            if p.is_null() {
                continue;
            }
            ucc::click_color_picker(cc.ptr(), mouse_evt(p.up()), p.up());
            r.frames(1);
        }
        (*m).hair_window = true;
        terraria::game_content::hairstyle_unlocks_helper::update_unlocks((*m).hairstyles.ptr());
        for k in 0..3 {
            ucc::click_randomize_player(cc.ptr(), Gc::NULL, Gc::NULL);
            let p = &*np.ptr();
            r.probe(&format!("charcreate2_random_{k}"), &[&p.hair, &p.skin_variant, &p.hair_color, &p.eye_color, &p.skin_color, &p.shirt_color, &p.pants_color, &p.voice_variant, &p.voice_pitch_offset]);
            r.frames(2);
        }
        r.record("charcreate2_random", 1);
        (*m).hair_window = false;
        terraria::game_content::hairstyle_unlocks_helper::update_unlocks((*m).hairstyles.ptr());
        for x in [0.0f32, 0.13, 0.5, 0.9] {
            let (c, k) = (ucc::get_voice_pitch_color_at(cc.ptr(), x), ucc::remap_pitch_slider_knob(cc.ptr(), x));
            r.probe(&format!("voicepitch_{}", x.pv()), &[&c, &k]);
        }
        for c in [rgb(255, 0, 0), rgb(12, 200, 99), rgb(240, 240, 250)] {
            let h = ucc::rgb_to_scaled_hsl(c);
            r.probe(&format!("rgbhsl_{}", c.pv()), &[&h]);
        }
        for h in [[0.0f32, 1.0, 0.5], [0.33, 0.2, 0.9], [0.95, 0.7, 0.0]] {
            let c = ucc::scaled_hsl_to_rgb__float_float_float(h[0], h[1], h[2]);
            r.probe(&format!("hslrgb_{}_{}_{}", h[0].pv(), h[1].pv(), h[2].pv()), &[&c]);
        }
        fancy_close();
        r.frames(3);
    }
    // ---- world creation
    if probe_group(r, "worldcreate2") {
        use terraria::game_content::ui::states::{ui_world_creation as uwc, ui_world_creation_advanced as uwa, ui_world_creation_advanced_secret_seeds_list as usl};
        let wc = uwc::new();
        let _r1 = rt::Rooted::new(wc);
        fancy(wc.up());
        r.frames(5);
        use terraria::world_gen::secret_seed as ss;
        let secrets: [(Gc<ss::SecretSeed>, &str); 3] =
            [((*ss::s()).paint_everything_gray, "probegray"), ((*ss::s()).extra_living_trees, "probetrees"), ((*ss::s()).error_world, "probeerror")];
        for (sd, plain) in secrets {
            (*sd.ptr())._plaintext = Str::new(plain);
        }
        for seed in ["12345", "not the bees", "for the worthy", "get fixed boi", "05162020", "celebrationmk10", "the constant", "no traps", "dont dig up", "Probe seed", "probegray", "1.1.1.0.probetrees|probe seed", "probeerror"] {
            uwc::on_finished_setting_seed(wc.ptr(), Str::new(seed));
            r.frames(4);
            let (os, he) = ((*wc.ptr())._option_seed, uwc::has_entered_special_seed(wc.ptr()));
            r.probe(&format!("worldseed_{}", seed.replace(' ', "_")), &[&os, &he]);
        }
        r.record("worldcreate2_seeds", 1);
        let font = re_logic::content::asset__dynamic_sprite_font::value((*terraria::game_content::font_assets::s()).mouse_text.ptr());
        let joined = uwc::get_joined_secret_seed_string(wc.ptr(), font, 300.0, 200.0);
        r.probe("worldseed_joined", &[&joined]);
        let adv = uwa::new(wc, false);
        let _r2 = rt::Rooted::new(adv);
        fancy(adv.up());
        r.frames(3);
        uwa::show_secret_seed_description(adv.ptr(), Gc::NULL, Gc::NULL);
        r.frames(3);
        r.record("worldcreate2_adv", 1);
        let sl = usl::new(adv, wc);
        let _r3 = rt::Rooted::new(sl);
        fancy(sl.up());
        r.frames(3);
        let items = (*(*sl.ptr())._world_list.ptr())._items;
        let n = (*items.ptr()).count();
        r.note(format!("secret seed buttons: {n}"));
        for i in 0..n.min(6) {
            usl::click_secret_seed(sl.ptr(), Gc::NULL, (*items.ptr()).get_item(i));
            r.frames(2);
        }
        r.frames(3);
        r.record("worldcreate2_secret", 1);
        for i in 0..n.min(6) {
            usl::click_secret_seed(sl.ptr(), Gc::NULL, (*items.ptr()).get_item(i));
        }
        fancy_close();
        ss::clear_all_seeds();
        for (sd, _) in secrets {
            (*sd.ptr())._plaintext = Str::NULL;
        }
        r.frames(3);
    }
    // ---- the world generation screen
    if probe_group(r, "worldload") {
        use terraria::world_building::{generation_progress as gp, world_generator as wg};
        let prog = gp::new();
        (*prog.ptr()).total_weight = 10.0;
        let ctl = wg::controller::new(Gc::NULL);
        (*wg::s()).current_generation_progress = prog;
        (*wg::s()).current_controller = ctl;
        let wl = terraria::game_content::ui::states::ui_world_load::new();
        let _r = rt::Rooted::new(wl);
        fancy(wl.up());
        for v in [0.0f64, 0.35, 0.8] {
            gp::set__double(prog.ptr(), v);
            gp::set_message(prog.ptr(), Str::new(&format!("Probe pass {}", v.pv())));
            r.frames(2);
            let (t, val) = (gp::total_progress(prog.ptr()), gp::value(prog.ptr()));
            r.probe(&format!("worldload_{}", v.pv()), &[&t, &val]);
        }
        r.record("worldload", 1);
        fancy_close();
        (*wg::s()).current_generation_progress = Gc::NULL;
        (*wg::s()).current_controller = Gc::NULL;
        r.frames(3);
    }
    // ---- the virtual keyboard's modes, the reports page
    if probe_group(r, "uimisc") {
        use terraria::game_content::ui::states::ui_virtual_keyboard as vk;
        let kb = vk::new(s!("Probe"), s!("abc"), rt::zero(), rt::zero(), 0, false, 20);
        let _r = rt::Rooted::new(kb);
        fancy(kb.up());
        r.frames(3);
        for st in [2, 1, 0, 1, 2] {
            vk::set_key_state(kb.ptr(), st);
            r.frames(2);
        }
        r.record("vkeyboard_modes", 1);
        fancy_close();
        r.frames(2);
        let reporters = List::<Obj>::new_gc();
        let rp = terraria::game_content::ui::states::ui_reports_page::new(Gc::NULL, 0, reporters);
        let _r2 = rt::Rooted::new(rp);
        fancy(rp.up());
        r.frames(5);
        r.record("reports", 1);
        fancy_close();
        r.frames(3);
    }
}
