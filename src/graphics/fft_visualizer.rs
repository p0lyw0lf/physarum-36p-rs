use winit::dpi::PhysicalSize;

use crate::shaders::line_render_shader as render_shader;

pub struct Pipeline {}

impl Pipeline {
    fn new(device: &wgpu::Device) -> Self {
        let render_shader_module = render_shader::create_shader_module(device);
        let render_pipeline_layout = render_shader::create_pipeline_layout(device)
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fft render pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: render_shader::vertex_state(&render_shader_module, &render_shader::vs_entry()),
            fragment: Some(render_shader::fragment_state(
                &render_shader_module,
                &render_shader::fs_entry(),
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
        })
        Self {}
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, new_size: PhysicalSize<u32>) {
        Self::calculate_uniforms(new_size);
    }

    fn calculate_uniforms(size: PhysicalSize<u32>) {}

    pub fn prepare(&mut self) {}

    pub fn render_pass(&self, render_pass: &mut wgpu::RenderPass) {}
}
