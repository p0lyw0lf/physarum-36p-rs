//! Module that contains "full" definitions for all the shaders, so we don't end up constructing
//! more than one of each.
//! I'm being a bit lazy here by not including _every_ pipeline, just the ones that need to be
//! shared between modules.

use std::sync::OnceLock;

use super::tris_render_shader;

static PIPELINES: OnceLock<Pipelines> = OnceLock::new();

struct Pipelines {
    render_tris: wgpu::RenderPipeline,
}

/// Initializes all the pipelines. MUST be called before
pub fn initialize(device: &wgpu::Device, surface_format: wgpu::TextureFormat) {
    let _ = PIPELINES.get_or_init(|| {
        let tris_render_module = tris_render_shader::create_shader_module(device);
        let tris_render_layout = tris_render_shader::create_pipeline_layout(device);
        let render_tris = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tris render pipeline"),
            layout: Some(&tris_render_layout),
            vertex: tris_render_shader::vertex_state(
                &tris_render_module,
                &tris_render_shader::vs_entry(wgpu::VertexStepMode::Vertex),
            ),
            fragment: Some(tris_render_shader::fragment_state(
                &tris_render_module,
                &tris_render_shader::fs_entry([(Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }))]),
            )),
            // Ideally, we'd like to use LineList for inputting vertexes along with
            // PolygonMode::Line, so we don't have to construct lines w/ triangles manually, but
            // unfortunately that isn't universally supported. So instead we'll just do lines w/
            // triangles :)
            primitive: Default::default(),
            depth_stencil: Default::default(),
            multisample: Default::default(),
            multiview: Default::default(),
            cache: Default::default(),
        });

        Pipelines { render_tris }
    });
}

pub fn render_tris(render_pass: &mut wgpu::RenderPass) {
    render_pass.set_pipeline(
        &PIPELINES
            .get()
            .expect("pipelines not initialized")
            .render_tris,
    );
}
