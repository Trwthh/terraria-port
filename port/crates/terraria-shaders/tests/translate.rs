//! Every pass of the game's three effects: decodes, has a consistent constant table, fills its registers,
//! translates, and the WGSL passes naga's validator.

mod support;

use terraria_host::gfx::fx_binary::{ParamClass, ParamType};
use terraria_shaders::ctab::{Class, RegisterSet, Type};
use terraria_shaders::{uniforms, wgsl};

fn validate(source: &str) -> Result<(), String> {
    let module = naga::front::wgsl::parse_str(source).map_err(|e| e.emit_to_string(source))?;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty()).validate(&module).map_err(|e| format!("{e:?}"))?;
    Ok(())
}

/// Every constant of every shader (and of every preshader) names an effect parameter of the same shape; every
/// sampler constant names a sampler parameter. This is what lets the uniform block be filled by name.
#[test]
fn every_constant_maps_to_an_effect_parameter() {
    let Some(effects) = support::load_effects() else { return };
    let mut checked = 0;
    for e in &effects {
        for p in &e.passes {
            let shaders = [Some(&p.pixel), p.vertex.as_ref()];
            for s in shaders.into_iter().flatten() {
                let tables = [s.ctab.as_ref(), s.preshader.as_ref().map(|pre| &pre.ctab)];
                assert!(s.ctab.is_some(), "{}: {} without a constant table", p.label(), s.model());
                for c in tables.into_iter().flatten().flat_map(|t| &t.constants) {
                    let param = e.fx.parameter(&c.name).unwrap_or_else(|| panic!("{}: constant {} is not an effect parameter", p.label(), c.name));
                    match c.register_set {
                        RegisterSet::Sampler => assert!(param.ty.ty.is_sampler(), "{}: {} is a sampler in the shader, {:?} in the effect", p.label(), c.name, param.ty.ty),
                        RegisterSet::Float4 => {
                            assert!(matches!(param.ty.ty, ParamType::Float | ParamType::Bool | ParamType::Int), "{}: {}", p.label(), c.name);
                            assert_eq!((c.rows, c.columns, c.elements), (param.ty.rows, param.ty.columns, param.ty.elements.max(1)), "{}: shape of {}", p.label(), c.name);
                            let same_class = match (c.class, param.ty.class) {
                                (Class::Scalar, ParamClass::Scalar) | (Class::Vector, ParamClass::Vector) => true,
                                // the effect stores every matrix row major; the shader's table says how it is packed
                                (Class::MatrixRows | Class::MatrixColumns, ParamClass::MatrixRows | ParamClass::MatrixColumns) => true,
                                _ => false,
                            };
                            assert!(same_class, "{}: class of {}: {:?} in the shader, {:?} in the effect", p.label(), c.name, c.class, param.ty.class);
                            assert!(matches!(c.ty, Type::Float | Type::Bool | Type::Int));
                        }
                        other => panic!("{}: constant {} in register set {other:?}", p.label(), c.name),
                    }
                    checked += 1;
                }
            }
        }
    }
    eprintln!("{checked} constants checked against their effect parameters");
}

#[test]
fn every_pass_translates_and_validates() {
    let Some(effects) = support::load_effects() else { return };
    let mut failures = Vec::new();
    let mut count = 0;
    for e in &effects {
        let params = support::test_parameters(&e.fx);
        for p in &e.passes {
            count += 1;
            let result = (|| -> Result<(), String> {
                let regs = uniforms::fill(&p.pixel, &params)?;
                let t = wgsl::translate(&p.label(), &p.pixel, p.vertex.as_ref())?;
                assert_eq!(t.ps_registers as usize, regs.len());
                if let Some(vs) = &p.vertex {
                    uniforms::fill(vs, &params)?;
                }
                validate(&t.source).map_err(|err| format!("naga: {err}\n{}", t.source))
            })();
            if let Err(err) = result {
                failures.push(format!("{}: {err}", p.label()));
            }
        }
    }
    eprintln!("{count} passes, {} failed", failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
