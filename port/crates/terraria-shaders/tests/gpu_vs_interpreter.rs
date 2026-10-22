//! The correctness proof of the WGSL path: every pass of the game's three effects is rendered twice, by the CPU
//! interpreter and on the GPU through the generated WGSL (a headless wgpu device, no window), and the pixels are
//! compared.
//!
//! Set-up (documented in `docs/effects/README.md`): a 64 x 64 `Rgba32Float` target (the shader's output is read
//! back before any 8-bit rounding: this GPU's float -> `Rgba8Unorm` conversion is not exact round-to-nearest, it
//! differs from it for fractions between .50 and .55 of a step, which would hide or fake errors), one quad
//! covering the target, no blending; RGBA8 textures of 64 / 32 / 16 / 32 pixels on samplers 0..3; sampler 0
//! clamps, the others wrap; uniform values from `support::TEST_VALUES`; vertex colour (230, 204, 179, 217).
//! Both results are clamped to 0..1 (what an `Rgba8Unorm` target keeps) and the error of a pixel is the largest
//! channel difference, in 8-bit steps (1.0 = 1/255). Two runs per pass:
//! - `linear`: bilinear filtering of smooth textures;
//! - `point`: point sampling of per-texel noise (any wrong texel shows).
//!
//! A pixel whose error is above one step is examined again, to tell a discontinuity of the shader (where the
//! last bits of a GPU's arithmetic decide the side) from a wrong translation. It counts as "on an edge" when
//! - `coordinate`: the GPU's value lies inside the range (plus one step) of the interpreter's results with the
//!   texture coordinates moved by +-1/128 of a texel of the 64 pixel texture (a texel boundary or a `frc` wrap
//!   of a computed coordinate), or
//! - `value`: the interpreter gives the GPU's value (within one step) when every texture sample is moved by
//!   +-1e-6 (a `cmp` threshold that the texel values hit exactly), or
//! - `transcendental`: the interpreter gives the GPU's value (within one step) when the results of `sincos`,
//!   `rsq`, `exp`, `log`, `pow`, `nrm` are moved by some amount up to +-1e-4 (the game hashes with
//!   `frac(sin(x) * 43758.5)`, which turns the last bits of any implementation's `sin` into noise).
//! Anything else is a mismatch and fails the test.
//!
//! Results go to `docs/effects/verification.tsv` (read by the documentation generator).

mod support;

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use support::{Pass, SIZE, TEXTURE_SIZES, VERTEX_COLOR};
use terraria_shaders::bytecode::{Extra, Usage};
use terraria_shaders::interp::{self, Address, Perturb, PixelInputs, PixelProgram, Sampler, Texture};
use terraria_shaders::wgsl::Translated;
use terraria_shaders::{check, uniforms, wgsl};

fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: String,
}

fn gpu() -> Option<Gpu> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = match block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, force_fallback_adapter: false, compatible_surface: None })) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("SKIPPED: no GPU adapter without a surface on this machine ({e})");
            return None;
        }
    };
    let info = adapter.get_info();
    let (device, queue) = match block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("SKIPPED: no device from {} ({e})", info.name);
            return None;
        }
    };
    Some(Gpu { device, queue, info: format!("{} ({:?}, driver {})", info.name, info.backend, info.driver_info) })
}

#[derive(Clone, Copy)]
struct Mode {
    linear: bool,
}
const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
const MODES: [Mode; 2] = [Mode { linear: true }, Mode { linear: false }];

/// Quarter-texel offset of the raw quad's texture coordinates: with Direct3D 9's pixel centres a 1:1 quad samples
/// exactly on texel boundaries, where point sampling is unstable; a quarter texel in is well defined.
const RAW_UV_OFFSET: f32 = 0.25 / SIZE as f32;

fn address_of(unit: usize) -> Address {
    if unit == 0 {
        Address::Clamp
    } else {
        Address::Wrap
    }
}

/// Bind group layout entries a translated pass needs: (group 0, group 1).
fn layouts(device: &wgpu::Device, t: &Translated) -> (wgpu::BindGroupLayout, wgpu::BindGroupLayout) {
    let mut entries = Vec::new();
    for unit in &t.samplers {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: unit * 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry { binding: unit * 2 + 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None });
    }
    let textures = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &entries });
    let uniform = |binding, visibility| wgpu::BindGroupLayoutEntry { binding, visibility, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None };
    let mut entries = Vec::new();
    if t.ps_registers > 0 {
        entries.push(uniform(0, wgpu::ShaderStages::FRAGMENT));
    }
    if t.vs_registers.is_some() {
        entries.push(uniform(1, wgpu::ShaderStages::VERTEX));
    }
    let uniforms = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &entries });
    (textures, uniforms)
}

struct GpuTextures {
    views: Vec<wgpu::TextureView>,
    samplers: Vec<wgpu::Sampler>,
}

fn gpu_textures(gpu: &Gpu, mode: Mode, pixels: &[Vec<u8>]) -> GpuTextures {
    let mut views = Vec::new();
    let mut samplers = Vec::new();
    for (unit, size) in TEXTURE_SIZES.iter().enumerate() {
        let extent = wgpu::Extent3d { width: *size, height: *size, depth_or_array_layers: 1 };
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &pixels[unit],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(size * 4), rows_per_image: Some(*size) },
            extent,
        );
        views.push(texture.create_view(&wgpu::TextureViewDescriptor::default()));
        let address = match address_of(unit) {
            Address::Clamp => wgpu::AddressMode::ClampToEdge,
            Address::Wrap => wgpu::AddressMode::Repeat,
            Address::Mirror => wgpu::AddressMode::MirrorRepeat,
        };
        let filter = if mode.linear { wgpu::FilterMode::Linear } else { wgpu::FilterMode::Nearest };
        samplers.push(gpu.device.create_sampler(&wgpu::SamplerDescriptor { address_mode_u: address, address_mode_v: address, mag_filter: filter, min_filter: filter, ..Default::default() }));
    }
    GpuTextures { views, samplers }
}

fn f32_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// The clear colour: what a discarded pixel shows.
const SENTINEL: [f32; 4] = [1.0, 0.0, 1.0, 1.0];

fn render_gpu(gpu: &Gpu, t: &Translated, ps_regs: &[[f32; 4]], vs_regs: Option<&[[f32; 4]]>, tex: &GpuTextures) -> Result<Vec<[f32; 4]>, String> {
    let device = &gpu.device;
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(t.source.as_str().into()) });
    let (layout0, layout1) = layouts(device, t);
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout0), Some(&layout1)], immediate_size: 0 });
    let sprite_attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4];
    let raw_attributes = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Unorm8x4];
    let raw = t.has_vertex_shader();
    let (stride, attributes): (u64, &[wgpu::VertexAttribute]) = if raw { (36, &raw_attributes) } else { (20, &sprite_attributes) };
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&layout),
        vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[wgpu::VertexBufferLayout { array_stride: stride, step_mode: wgpu::VertexStepMode::Vertex, attributes }] },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format: TARGET_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    // the quad: top-left, top-right, bottom-right, bottom-left as two triangles
    let mut vertices = Vec::new();
    let corners = [(0.0f32, 0.0f32), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
    for k in [0usize, 1, 2, 0, 2, 3] {
        let (cx, cy) = corners[k];
        if raw {
            vertices.extend(f32_bytes(&[cx * SIZE as f32, cy * SIZE as f32, 0.0, 1.0, cx + RAW_UV_OFFSET, cy + RAW_UV_OFFSET, 1.0, 1.0]));
        } else {
            vertices.extend(f32_bytes(&[cx * 2.0 - 1.0, 1.0 - cy * 2.0, cx, cy]));
        }
        vertices.extend(VERTEX_COLOR);
    }
    let buffer = |usage, contents: &[u8]| {
        let b = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: contents.len().max(16) as u64, usage: usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        gpu.queue.write_buffer(&b, 0, contents);
        b
    };
    let vertex_buffer = buffer(wgpu::BufferUsages::VERTEX, &vertices);
    let ps_buffer = buffer(wgpu::BufferUsages::UNIFORM, &f32_bytes(&ps_regs.concat()));
    let vs_buffer = vs_regs.map(|regs| {
        let mut data = vec![1.0 / SIZE as f32, -1.0 / SIZE as f32, 0.0, 0.0];
        data.extend(regs.concat());
        buffer(wgpu::BufferUsages::UNIFORM, &f32_bytes(&data))
    });

    let mut entries = Vec::new();
    for unit in &t.samplers {
        let u = *unit as usize;
        entries.push(wgpu::BindGroupEntry { binding: unit * 2, resource: wgpu::BindingResource::TextureView(&tex.views[u]) });
        entries.push(wgpu::BindGroupEntry { binding: unit * 2 + 1, resource: wgpu::BindingResource::Sampler(&tex.samplers[u]) });
    }
    let group0 = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &layout0, entries: &entries });
    let mut entries = Vec::new();
    if t.ps_registers > 0 {
        entries.push(wgpu::BindGroupEntry { binding: 0, resource: ps_buffer.as_entire_binding() });
    }
    if let Some(b) = &vs_buffer {
        entries.push(wgpu::BindGroupEntry { binding: 1, resource: b.as_entire_binding() });
    }
    let group1 = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &layout1, entries: &entries });

    let extent = wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TARGET_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 1.0, g: 0.0, b: 1.0, a: 1.0 }), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group0, &[]);
        pass.set_bind_group(1, &group1, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..6, 0..1);
    }
    // 64 * 16 = 1024 bytes per row: already a multiple of the required row alignment
    let readback = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (SIZE * SIZE * 16) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &target, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(SIZE * 16), rows_per_image: Some(SIZE) } },
        extent,
    );
    gpu.queue.submit([encoder.finish()]);
    if let Some(e) = block_on(scope.pop()) {
        return Err(format!("wgpu validation: {e}"));
    }
    readback.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| format!("poll: {e:?}"))?;
    let data = readback.slice(..).get_mapped_range().to_vec();
    Ok(data.chunks_exact(16).map(|p| [0, 4, 8, 12].map(|o| f32::from_le_bytes([p[o], p[o + 1], p[o + 2], p[o + 3]]))).collect())
}

fn lerp4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
}

/// The interpreter side of one pass: the pixel program plus the inputs of every pixel.
struct Cpu<'a> {
    program: PixelProgram<'a>,
    /// vertex shader outputs at the corners (top-left, top-right, bottom-right, bottom-left), when the pass has one
    corners: Option<Vec<interp::VertexOutputs>>,
    color: [f32; 4],
    /// the shader uses an instruction of `check::INEXACT`
    inexact: bool,
}

impl Cpu<'_> {
    /// Inputs of pixel (px, py) with the texture coordinates moved by (du, dv).
    fn inputs(&self, px: u32, py: u32, du: f32, dv: f32) -> PixelInputs {
        let mut inputs = PixelInputs::default();
        match &self.corners {
            None => {
                inputs.t[0] = [(px as f32 + 0.5) / SIZE as f32 + du, (py as f32 + 0.5) / SIZE as f32 + dv, 0.0, 1.0];
                inputs.v[0] = self.color;
            }
            Some(c) => {
                // Direct3D 9 pixel centres: pixel (px, py) is the quad at exactly (px, py) / size
                let (s, t) = (px as f32 / SIZE as f32, py as f32 / SIZE as f32);
                for k in 0..8 {
                    inputs.t[k] = lerp4(lerp4(c[0].t[k], c[1].t[k], s), lerp4(c[3].t[k], c[2].t[k], s), t);
                    inputs.t[k][0] += du;
                    inputs.t[k][1] += dv;
                }
                for k in 0..2 {
                    inputs.v[k] = lerp4(lerp4(c[0].d[k], c[1].d[k], s), lerp4(c[3].d[k], c[2].d[k], s), t);
                }
            }
        }
        inputs
    }

    fn pixel(&self, px: u32, py: u32, du: f32, dv: f32) -> Result<[f32; 4], String> {
        self.perturbed(px, py, du, dv, Perturb::default())
    }

    fn perturbed(&self, px: u32, py: u32, du: f32, dv: f32, perturb: Perturb) -> Result<[f32; 4], String> {
        Ok(self.program.run_perturbed(&self.inputs(px, py, du, dv), perturb)?.unwrap_or(SENTINEL).map(clamp01))
    }
}

fn clamp01(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

fn cpu_side<'a>(p: &'a Pass, ps_regs: &[[f32; 4]], vs_regs: Option<&[[f32; 4]]>, samplers: &'a [Sampler<'a>]) -> Result<Cpu<'a>, String> {
    let program = PixelProgram::new(&p.pixel, ps_regs, samplers)?;
    let color = VERTEX_COLOR.map(|b| b as f32 / 255.0);
    let corners = match (&p.vertex, vs_regs) {
        (Some(vs), Some(regs)) => {
            let mut out = Vec::new();
            for (cx, cy) in [(0.0f32, 0.0f32), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                let mut inputs = [[0f32; 4]; 16];
                for (d, e) in vs.declarations() {
                    let Extra::DclUsage { usage, .. } = e else { continue };
                    inputs[d.reg.num as usize] = match usage {
                        Usage::Position => [cx * SIZE as f32, cy * SIZE as f32, 0.0, 1.0],
                        Usage::TexCoord => [cx + RAW_UV_OFFSET, cy + RAW_UV_OFFSET, 1.0, 1.0],
                        Usage::Color => color,
                        other => return Err(format!("vertex input {}", other.name())),
                    };
                }
                let o = interp::run_vertex(vs, regs, &inputs)?;
                // the test matrix maps the quad onto the whole target with w = 1
                let expect = [cx * 2.0 - 1.0, 1.0 - cy * 2.0];
                if (o.position[0] - expect[0]).abs() > 1e-5 || (o.position[1] - expect[1]).abs() > 1e-5 || o.position[3] != 1.0 {
                    return Err(format!("vertex shader put the corner ({cx}, {cy}) at {:?}", o.position));
                }
                out.push(o);
            }
            Some(out)
        }
        _ => None,
    };
    Ok(Cpu { program, corners, color, inexact: check::is_inexact(&p.pixel) })
}

#[derive(Default)]
struct Outcome {
    /// largest error of a pixel that is not on an edge, in 8-bit steps
    max: f32,
    /// pixels above one step explained as an edge: by coordinate jitter, by sample value, by transcendental precision
    on_edge: [usize; 3],
    /// pixels above one step that nothing explains
    mismatched: usize,
    /// largest error of a mismatched pixel
    worst: f32,
    distinct: usize,
}

/// Jitter of the texture coordinates for the edge test: 1/128 of a texel of the 64 pixel texture.
const JITTER: f32 = 1.0 / (128.0 * SIZE as f32);

fn compare(cpu: &Cpu, gpu: &[[f32; 4]]) -> Result<Outcome, String> {
    let mut o = Outcome::default();
    let mut seen = std::collections::HashSet::new();
    for py in 0..SIZE {
        for px in 0..SIZE {
            let g = gpu[(py * SIZE + px) as usize].map(clamp01);
            let c = cpu.pixel(px, py, 0.0, 0.0)?;
            seen.insert(c.map(interp::to_unorm8));
            let err = (0..4).map(|k| (c[k] - g[k]).abs() * 255.0).fold(0.0, f32::max);
            if err <= 1.0 {
                o.max = o.max.max(err);
                continue;
            }
            let (mut lo, mut hi) = (c, c);
            for (du, dv) in [(-1.0, -1.0), (0.0, -1.0), (1.0, -1.0), (-1.0, 0.0), (1.0, 0.0), (-1.0, 1.0), (0.0, 1.0), (1.0, 1.0)] {
                let v = cpu.pixel(px, py, du * JITTER, dv * JITTER)?;
                for k in 0..4 {
                    lo[k] = lo[k].min(v[k]);
                    hi[k] = hi[k].max(v[k]);
                }
            }
            let matches = |v: [f32; 4]| (0..4).all(|k| (v[k] - g[k]).abs() * 255.0 <= 1.0);
            let hit = |perturb: Perturb| -> Result<bool, String> { Ok(matches(cpu.perturbed(px, py, 0.0, 0.0, perturb)?)) };
            if (0..4).all(|k| g[k] >= lo[k] - 1.0 / 255.0 && g[k] <= hi[k] + 1.0 / 255.0) {
                o.on_edge[0] += 1;
            } else if hit(Perturb { sample: 1e-6, transcendental: 0.0 })? || hit(Perturb { sample: -1e-6, transcendental: 0.0 })? {
                o.on_edge[1] += 1;
            } else if cpu.inexact && {
                let mut found = false;
                for n in 1..=2000 {
                    // 4000 values from -1e-4 to 1e-4
                    let d = n as f32 * 5e-8;
                    if hit(Perturb { sample: 0.0, transcendental: d })? || hit(Perturb { sample: 0.0, transcendental: -d })? {
                        found = true;
                        break;
                    }
                }
                found
            } {
                o.on_edge[2] += 1;
            } else {
                o.mismatched += 1;
                o.worst = o.worst.max(err);
            }
        }
    }
    o.distinct = seen.len();
    Ok(o)
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "needs a GPU adapter: WASI has none; the browser stage runs it on WebGPU / WebGL2")]
fn every_pass_matches_the_interpreter_on_the_gpu() {
    let Some(effects) = support::load_effects() else { return };
    let Some(gpu) = gpu() else { return };
    eprintln!("GPU: {}", gpu.info);
    let mut report = String::from("effect\tindex\tpass\tclass\tlinear_max\tlinear_edges\tpoint_max\tpoint_edges\tmismatched\tdistinct_colors\tresult\n");
    let mut failures = Vec::new();
    let mut verified = 0;
    let pixel_sets: Vec<Vec<Vec<u8>>> = MODES.iter().map(|m| (0..4).map(|unit| support::test_texture(unit, TEXTURE_SIZES[unit], m.linear)).collect()).collect();
    let gpu_sets: Vec<GpuTextures> = MODES.iter().zip(&pixel_sets).map(|(m, p)| gpu_textures(&gpu, *m, p)).collect();
    for e in &effects {
        let params = support::test_parameters(&e.fx);
        for p in &e.passes {
            let class = if check::is_inexact(&p.pixel) { "transcendental" } else { "arithmetic" };
            let result = (|| -> Result<Vec<Outcome>, String> {
                let t = wgsl::translate(&p.label(), &p.pixel, p.vertex.as_ref())?;
                let ps_regs = uniforms::fill(&p.pixel, &params)?;
                let vs_regs = match &p.vertex {
                    Some(vs) => {
                        let mut r = uniforms::fill(vs, &params)?;
                        r.resize(t.vs_registers.unwrap() as usize, [0.0; 4]);
                        Some(r)
                    }
                    None => None,
                };
                let mut outcomes = Vec::new();
                for (k, mode) in MODES.iter().enumerate() {
                    let samplers: Vec<Sampler> = (0..4)
                        .map(|unit| Sampler { texture: Some(Texture { width: TEXTURE_SIZES[unit], height: TEXTURE_SIZES[unit], rgba: &pixel_sets[k][unit] }), linear: mode.linear, address_u: address_of(unit), address_v: address_of(unit) })
                        .collect();
                    let cpu = cpu_side(p, &ps_regs, vs_regs.as_deref(), &samplers)?;
                    let on_gpu = render_gpu(&gpu, &t, &ps_regs, vs_regs.as_deref(), &gpu_sets[k])?;
                    outcomes.push(compare(&cpu, &on_gpu)?);
                }
                Ok(outcomes)
            })();
            match result {
                Ok(o) => {
                    let (linear, point) = (&o[0], &o[1]);
                    let mismatched = linear.mismatched + point.mismatched;
                    let trivial = linear.distinct <= 1;
                    let verdict = if mismatched > 0 { "MISMATCH" } else if trivial { "match (constant image)" } else { "match" };
                    let edges = |e: [usize; 3]| if e == [0; 3] { "0".to_owned() } else { format!("{} coordinate, {} value, {} transcendental", e[0], e[1], e[2]) };
                    report.push_str(&format!("{}\t{}\t{}\t{class}\t{:.3}\t{}\t{:.3}\t{}\t{mismatched}\t{}\t{verdict}\n", p.effect, p.index, p.name, linear.max, edges(linear.on_edge), point.max, edges(point.on_edge), linear.distinct));
                    if mismatched == 0 {
                        verified += 1;
                    } else {
                        failures.push(format!("{}: linear {} px mismatched (worst {:.1} steps), point {} px mismatched (worst {:.1} steps)", p.label(), linear.mismatched, linear.worst, point.mismatched, point.worst));
                    }
                }
                Err(err) => {
                    report.push_str(&format!("{}\t{}\t{}\t{class}\t\t\t\t\t\t\tERROR {}\n", p.effect, p.index, p.name, err.lines().next().unwrap_or("")));
                    failures.push(format!("{}: {err}", p.label()));
                }
            }
        }
    }
    let path = support::docs_dir().join("verification.tsv");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, &report).unwrap();
    eprintln!("{verified} passes verified, {} failed; report: {}", failures.len(), path.display());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ------------------------------------------------------------------------------------------------------------
// Instructions and modifiers the translator implements but the game's files never use: `sub`, `slt`, `sge`,
// `texkill`, `texld` with the projected and the bias flag, the `abs` / `-abs` source modifiers. A pixel shader
// assembled by hand from the token layout exercises them through the same comparison.

mod asm {
    pub const TEMP: u32 = 0;
    pub const INPUT: u32 = 1;
    pub const CONST: u32 = 2;
    pub const TEXTURE: u32 = 3;
    pub const COLOROUT: u32 = 8;
    pub const SAMPLER: u32 = 10;
    pub const XYZW: [u32; 4] = [0, 1, 2, 3];

    fn reg(ty: u32, num: u32) -> u32 {
        0x8000_0000 | num | ((ty & 7) << 28) | ((ty & 0x18) << 8)
    }
    pub fn dst(ty: u32, num: u32, mask: u32) -> u32 {
        reg(ty, num) | (mask << 16)
    }
    /// modifier: 0 none, 1 negate, 11 abs, 12 -abs
    pub fn src(ty: u32, num: u32, swizzle: [u32; 4], modifier: u32) -> u32 {
        reg(ty, num) | ((swizzle[0] | swizzle[1] << 2 | swizzle[2] << 4 | swizzle[3] << 6) << 16) | (modifier << 24)
    }
    pub fn ins(out: &mut Vec<u32>, opcode: u32, control: u32, operands: &[u32]) {
        out.push(opcode | (control << 16) | ((operands.len() as u32) << 24));
        out.extend_from_slice(operands);
    }
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "needs a GPU adapter: WASI has none; the browser stage runs it on WebGPU / WebGL2")]
fn instructions_the_game_does_not_use_match_too() {
    use asm::*;
    let Some(gpu) = gpu() else { return };
    let r = |n: u32| src(TEMP, n, XYZW, 0);
    let c0 = |comp: u32| src(CONST, 0, [comp; 4], 0);
    let mut t: Vec<u32> = vec![0xFFFF_0200];
    ins(&mut t, 81, 0, &[dst(CONST, 0, 15), 0.5f32.to_bits(), 0.25f32.to_bits(), 2f32.to_bits(), (-1f32).to_bits()]); // def c0, 0.5, 0.25, 2, -1
    ins(&mut t, 31, 0, &[0x8000_0000, dst(TEXTURE, 0, 3)]); // dcl t0.xy
    ins(&mut t, 31, 0, &[0x8000_0000, dst(INPUT, 0, 15)]); // dcl v0
    ins(&mut t, 31, 0, &[0x9000_0000, dst(SAMPLER, 0, 15)]); // dcl_2d s0
    ins(&mut t, 66, 0, &[dst(TEMP, 0, 15), src(TEXTURE, 0, XYZW, 0), src(SAMPLER, 0, XYZW, 0)]); // texld r0, t0, s0
    ins(&mut t, 3, 0, &[dst(TEMP, 1, 15), r(0), c0(0)]); // sub r1, r0, c0.x
    ins(&mut t, 12, 0, &[dst(TEMP, 2, 15), r(1), c0(1)]); // slt r2, r1, c0.y
    ins(&mut t, 13, 0, &[dst(TEMP, 3, 15), r(1), src(CONST, 0, [1; 4], 1)]); // sge r3, r1, -c0.y
    ins(&mut t, 5, 0, &[dst(TEMP, 2, 15), r(2), r(3)]); // mul r2, r2, r3
    ins(&mut t, 2, 0, &[dst(TEMP, 2, 7), r(2), src(TEMP, 1, XYZW, 12)]); // add r2.xyz, r2, -r1_abs
    ins(&mut t, 2, 0, &[dst(TEMP, 2, 8), r(2), src(TEMP, 1, [0; 4], 11)]); // add r2.w, r2, r1_abs.x
    ins(&mut t, 5, 0, &[dst(TEMP, 4, 3), src(TEXTURE, 0, XYZW, 0), c0(2)]); // mul r4.xy, t0, c0.z
    ins(&mut t, 1, 0, &[dst(TEMP, 4, 12), c0(2)]); // mov r4.zw, c0.z
    ins(&mut t, 66, 1, &[dst(TEMP, 5, 15), r(4), src(SAMPLER, 0, XYZW, 0)]); // texldp r5, r4, s0
    ins(&mut t, 2, 0, &[dst(TEMP, 2, 15), r(2), r(5)]); // add r2, r2, r5
    ins(&mut t, 1, 0, &[dst(TEMP, 7, 3), src(TEXTURE, 0, XYZW, 0)]); // mov r7.xy, t0
    ins(&mut t, 1, 0, &[dst(TEMP, 7, 12), c0(2)]); // mov r7.zw, c0.z
    ins(&mut t, 66, 2, &[dst(TEMP, 5, 15), r(7), src(SAMPLER, 0, XYZW, 0)]); // texldb r5, r7, s0
    ins(&mut t, 4, 0, &[dst(TEMP, 2, 15), r(5), c0(1), r(2)]); // mad r2, r5, c0.y, r2
    ins(&mut t, 1, 0, &[dst(TEMP, 6, 15), src(TEMP, 1, [0; 4], 0)]); // mov r6, r1.x
    ins(&mut t, 65, 0, &[dst(TEMP, 6, 15)]); // texkill r6
    ins(&mut t, 5, 0, &[dst(TEMP, 2, 15), r(2), src(INPUT, 0, XYZW, 0)]); // mul r2, r2, v0
    ins(&mut t, 1, 0, &[dst(COLOROUT, 0, 15), r(2)]); // mov oC0, r2
    t.push(0x0000_FFFF);
    let bytes: Vec<u8> = t.iter().flat_map(|w| w.to_le_bytes()).collect();
    let pixel = terraria_shaders::bytecode::parse(&bytes).unwrap();
    let listing = terraria_shaders::disasm::listing(&pixel);
    for line in ["sub r1, r0, c0.x", "slt r2, r1, c0.y", "sge r3, r1, -c0.y", "add r2.xyz, r2, -r1_abs", "add r2.w, r2, r1_abs.x", "texldp r5, r4, s0", "texldb r5, r7, s0", "texkill r6"] {
        assert!(listing.contains(line), "{line} missing in:\n{listing}");
    }
    let pass = Pass { effect: "synthetic", technique: "T".into(), index: 0, name: "unused instructions".into(), pixel_bytes: bytes, pixel, vertex_bytes: None, vertex: None };
    let translated = wgsl::translate(&pass.label(), &pass.pixel, None).unwrap();
    for line in ["if (any(r6 < vec4<f32>(0.0))) { discard; }", "textureSample(tex0, smp0, r4.xy / r4.w)", "textureSampleBias(tex0, smp0, r7.xy, r7.w)", "-abs(r1.xyz)"] {
        assert!(translated.source.contains(line), "{line} missing in:\n{}", translated.source);
    }
    let regs = uniforms::fill(&pass.pixel, &|_| None).unwrap();
    assert!(regs.is_empty());
    for mode in MODES {
        let pixels: Vec<Vec<u8>> = (0..4).map(|unit| support::test_texture(unit, TEXTURE_SIZES[unit], mode.linear)).collect();
        let textures = gpu_textures(&gpu, mode, &pixels);
        let samplers: Vec<Sampler> = (0..4).map(|unit| Sampler { texture: Some(Texture { width: TEXTURE_SIZES[unit], height: TEXTURE_SIZES[unit], rgba: &pixels[unit] }), linear: mode.linear, address_u: address_of(unit), address_v: address_of(unit) }).collect();
        let cpu = cpu_side(&pass, &regs, None, &samplers).unwrap();
        let on_gpu = render_gpu(&gpu, &translated, &regs, None, &textures).unwrap();
        let o = compare(&cpu, &on_gpu).unwrap();
        // some pixels are discarded, some are not
        let killed = (0..SIZE * SIZE).filter(|n| cpu.pixel(n % SIZE, n / SIZE, 0.0, 0.0).unwrap() == SENTINEL).count();
        eprintln!("synthetic shader ({}): max {:.3} steps, {:?} pixels on an edge, {} mismatched, {killed} of {} discarded", if mode.linear { "linear" } else { "point" }, o.max, o.on_edge, o.mismatched, SIZE * SIZE);
        assert_eq!(o.mismatched, 0, "worst {} steps", o.worst);
        assert!(killed > 100 && killed < (SIZE * SIZE - 100) as usize, "{killed} pixels discarded");
        assert!(o.distinct > 100);
    }
}
