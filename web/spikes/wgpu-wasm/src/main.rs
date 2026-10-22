//! Spike: terraria-app's rendering pattern (port/crates/terraria-app/src/gfx/render.rs) on wasm32: a Bevy app whose
//! render world replays a draw list with raw wgpu calls into an image that Bevy shows. Here the "draw list" is one
//! triangle. Compile check only (`cargo check --target wasm32-unknown-unknown`); it also builds natively.
use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;
use bevy::render::{Extract, ExtractSchedule, Render, RenderApp, RenderSystems};

const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

const SHADER: &str = r#"
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) color: vec4<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> VOut {
    var p = array<vec2<f32>, 3>(vec2<f32>(-0.8, -0.8), vec2<f32>(0.8, -0.8), vec2<f32>(0.0, 0.8));
    var o: VOut;
    o.pos = vec4<f32>(p[i], 0.0, 1.0);
    o.color = vec4<f32>(1.0, 0.5, 0.1, 1.0);
    return o;
}
@fragment fn fs(i: VOut) -> @location(0) vec4<f32> { return i.color; }
"#;

#[derive(Resource, Clone)]
struct BackBufferImage(Handle<Image>);

#[derive(Resource, Default)]
struct Extracted(Option<Handle<Image>>);

#[derive(Resource, Default)]
struct Gpu(Option<wgpu::RenderPipeline>);

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window { title: "wgpu-wasm spike".into(), canvas: Some("#terraria".into()), fit_canvas_to_parent: true, ..default() }),
        ..default()
    }));
    app.add_systems(Startup, setup);
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app
            .init_resource::<Extracted>()
            .init_resource::<Gpu>()
            .add_systems(ExtractSchedule, extract)
            .add_systems(Render, replay.in_set(RenderSystems::Queue));
    }
    app.run();
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let image = Image::new_target_texture(320, 180, wgpu::TextureFormat::Rgba8UnormSrgb, None);
    let _ = RenderAssetUsages::default();
    let handle = images.add(image);
    commands.insert_resource(BackBufferImage(handle.clone()));
    commands.spawn(Camera2d);
    commands.spawn(Sprite::from_image(handle));
}

fn extract(back: Extract<Option<Res<BackBufferImage>>>, mut out: ResMut<Extracted>) {
    out.0 = back.as_ref().map(|b| b.0.clone());
}

fn replay(ext: Res<Extracted>, images: Res<RenderAssets<GpuImage>>, device: Res<RenderDevice>, queue: Res<RenderQueue>, mut gpu: ResMut<Gpu>) {
    let Some(g) = ext.0.as_ref().and_then(|h| images.get(h)) else { return };
    let device = device.wgpu_device();
    let pipeline = gpu.0.get_or_insert_with(|| {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("spike"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[], immediate_size: 0 });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("spike"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: TARGET_FORMAT, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    });
    let view = g.texture.create_view(&wgpu::TextureViewDescriptor { format: Some(TARGET_FORMAT), ..Default::default() });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("spike") });
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("spike"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.draw(0..3, 0..1);
    }
    queue.submit([enc.finish()]);
}
