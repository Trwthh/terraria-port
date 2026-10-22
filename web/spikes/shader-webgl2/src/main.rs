//! cargo run --release -- <dir with */*.wgsl>
//! For every translated pass: parse, validate with the capabilities a WebGL2 device has (none of the optional ones),
//! then write GLSL ES 3.00 for WebGL (what wgpu-hal's GLES backend does on the web) for each entry point, and count
//! what the downlevel WebGL2 limits care about (textures + samplers per stage, uniform bytes, inter-stage locations).
use std::fs;
use std::path::{Path, PathBuf};

use naga::back::glsl;

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            files(&p, out);
        } else if p.extension().is_some_and(|x| x == "wgsl") {
            out.push(p);
        }
    }
}

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).expect("dir"));
    let mut all = Vec::new();
    files(&dir, &mut all);
    all.sort();
    let (mut ok, mut failed) = (0, 0);
    let (mut max_tex, mut max_uniform_bytes, mut max_loc) = (0usize, 0u32, 0u32);
    for path in &all {
        let name = path.strip_prefix(&dir).unwrap().display().to_string();
        let src = fs::read_to_string(path).unwrap();
        let module = match naga::front::wgsl::parse_str(&src) {
            Ok(m) => m,
            Err(e) => {
                failed += 1;
                println!("FAIL parse {name}: {}", e.emit_to_string(&src));
                continue;
            }
        };
        let info = match naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty()).validate(&module) {
            Ok(i) => i,
            Err(e) => {
                failed += 1;
                println!("FAIL validate {name}: {e:?}");
                continue;
            }
        };
        // limits that WebGL2 / wgpu's downlevel_webgl2_defaults care about
        let mut tex = 0;
        for (_, g) in module.global_variables.iter() {
            if let naga::TypeInner::Image { .. } = module.types[g.ty].inner {
                tex += 1;
            }
            if g.space == naga::AddressSpace::Uniform {
                let size = module.types[g.ty].inner.size(module.to_ctx());
                max_uniform_bytes = max_uniform_bytes.max(size);
            }
        }
        max_tex = max_tex.max(tex);
        for ep in &module.entry_points {
            if let Some(r) = &ep.function.result {
                if let naga::TypeInner::Struct { members, .. } = &module.types[r.ty].inner {
                    for m in members {
                        if let Some(naga::Binding::Location { location, .. }) = m.binding {
                            max_loc = max_loc.max(location);
                        }
                    }
                }
            }
        }
        let mut pass_ok = true;
        for ep in &module.entry_points {
            let options = glsl::Options {
                version: glsl::Version::Embedded { version: 300, is_webgl: true },
                writer_flags: glsl::WriterFlags::empty(),
                ..Default::default()
            };
            let pipeline = glsl::PipelineOptions { shader_stage: ep.stage, entry_point: ep.name.clone(), multiview: None };
            let mut out = String::new();
            let r = glsl::Writer::new(&mut out, &module, &info, &options, &pipeline, naga::proc::BoundsCheckPolicies::default()).and_then(|mut w| w.write());
            match r {
                Ok(_) => {}
                Err(e) => {
                    pass_ok = false;
                    println!("FAIL glsl {name} {:?} {}: {e}", ep.stage, ep.name);
                }
            }
        }
        if pass_ok {
            ok += 1;
        } else {
            failed += 1;
        }
    }
    println!("{} passes: {ok} translated to GLSL ES 3.00 (WebGL) for every entry point, {failed} failed", all.len());
    println!("most textures in one pass: {max_tex} (wgpu 29 downlevel_webgl2_defaults: 16 sampled textures, 16 samplers per stage)");
    println!("largest uniform block: {max_uniform_bytes} bytes (wgpu 29 downlevel_webgl2_defaults: max_uniform_buffer_binding_size 16384)");
    println!("highest inter-stage location: {max_loc} (wgpu 29 downlevel_webgl2_defaults: max_inter_stage_shader_variables 15)");
}
