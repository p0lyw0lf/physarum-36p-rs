use std::borrow::Cow;

pub struct Pipeline {
    render_pipeline: wgpu::RenderPipeline,
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hardcoded red triangle shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(
                "
struct OurVertexShaderOutput {
    @builtin(position) position: vec4f,
    @location(0) color: vec4f,
}

@vertex fn vs(
    @builtin(vertex_index) vertexIndex: u32,
) -> OurVertexShaderOutput {
    let pos = array(
        vec2f( 0.0,  0.5),
        vec2f(-0.5, -0.5),
        vec2f( 0.5, -0.5),
    );
    var color = array<vec4f, 3>(
        vec4f(1, 0, 0, 1), // red
        vec4f(0, 1, 0, 1), // green
        vec4f(0, 0, 1, 1), // blue
    );

    var vsOutput: OurVertexShaderOutput;
    vsOutput.position = vec4f(pos[vertexIndex], 0.0, 1.0);
    vsOutput.color = color[vertexIndex];
    return vsOutput;
}

@fragment fn fs(fsInput: OurVertexShaderOutput) -> @location(0) vec4f {
    let cyan = vec4f(0, 1, 1, 1);

    let grid = vec2u(fsInput.position.xy);
    let checker = (grid.x + grid.y) % 16 > 8;

    return select(fsInput.color, cyan, checker);
}
",
            )),
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("our hardcoded rainbow triangle pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Default::default(),
                    write_mask: Default::default(),
                })],
            }),
            primitive: Default::default(),
            depth_stencil: Default::default(),
            multisample: Default::default(),
            multiview: Default::default(),
            cache: Default::default(),
        });

        Self { render_pipeline }
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_texture: &wgpu::SurfaceTexture,
        surface_format: wgpu::TextureFormat,
    ) {
        let surface_texture_view =
            surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("surface texture view"),
                    format: Default::default(),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    usage: Default::default(),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: None,
                });

        let render_pass_descriptor = wgpu::RenderPassDescriptor {
            label: Some("out basic surface renderPass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &surface_texture_view,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.3,
                        g: 0.3,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                resolve_target: Default::default(),
                depth_slice: Default::default(),
            })],
            depth_stencil_attachment: Default::default(),
            timestamp_writes: Default::default(),
            occlusion_query_set: Default::default(),
        };

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("our encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&render_pass_descriptor);
            pass.set_pipeline(&self.render_pipeline);
            pass.draw(0..3, 0..1);
        }

        let command_buffer = encoder.finish();
        queue.submit([command_buffer]);
    }
}
