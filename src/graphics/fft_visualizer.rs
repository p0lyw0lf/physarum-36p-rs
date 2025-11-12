use winit::dpi::PhysicalSize;

use crate::{
    constants::HEADER_HEIGHT, graphics::camera_2d, shaders::tris_render_shader as render_shader,
};

pub struct Pipeline {
    render_uniforms_buffer: wgpu::Buffer,

    render_bind_group: render_shader::bind_groups::BindGroup0,
    static_vertex_buffer: wgpu::Buffer,
    dynamic_vertex_buffer: wgpu::Buffer,

    render_pipeline: wgpu::RenderPipeline,
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let render_shader_module = render_shader::create_shader_module(device);
        let render_pipeline_layout = render_shader::create_pipeline_layout(device);
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fft render pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: render_shader::vertex_state(
                &render_shader_module,
                &render_shader::vs_entry(
                    wgpu::VertexStepMode::Vertex,
                    wgpu::VertexStepMode::Vertex,
                ),
            ),
            fragment: Some(render_shader::fragment_state(
                &render_shader_module,
                &render_shader::fs_entry([(Some(wgpu::ColorTargetState {
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

        let static_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fft static vertex buffer"),
            size: size_of::<render_shader::StaticVertex>() as u64 * 3,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Fill in the buffer with dummy data.
        // TODO: convert to real data, drawing circles & lines w/ triangles
        queue.write_buffer(
            &static_vertex_buffer,
            0,
            bytemuck::bytes_of(&[
                render_shader::StaticVertex {
                    base_position: glam::vec4(0.5, 0.0, 0.0, 0.0),
                    color: glam::vec4(1.0, 0.0, 0.0, 0.0), // red
                },
                render_shader::StaticVertex {
                    base_position: glam::vec4(0.0, 1.0, 0.0, 0.0),
                    color: glam::vec4(0.0, 1.0, 0.0, 0.0), // green
                },
                render_shader::StaticVertex {
                    base_position: glam::vec4(1.0, 1.0, 0.0, 0.0),
                    color: glam::vec4(0.0, 0.0, 1.0, 0.0), // blue
                },
            ]),
        );

        let dynamic_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fft dynamic vertex buffer"),
            size: size_of::<render_shader::DynamicVertex>() as u64 * 3,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // filled in during each prepare()

        let render_uniforms_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fft render uniforms"),
            size: size_of::<render_shader::Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Filled in when screen is resized

        let render_bind_group = render_shader::bind_groups::BindGroup0::from_bindings(
            device,
            render_shader::bind_groups::BindGroupLayout0 {
                uni: render_uniforms_buffer.as_entire_buffer_binding(),
            },
        );

        Self {
            render_uniforms_buffer,
            render_bind_group,
            static_vertex_buffer,
            dynamic_vertex_buffer,
            render_pipeline,
        }
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, new_size: PhysicalSize<u32>) {
        let render_uniforms = Self::calculate_uniforms(new_size);
        queue.write_buffer(
            &self.render_uniforms_buffer,
            0,
            bytemuck::bytes_of(&render_uniforms),
        );
    }

    fn calculate_uniforms(size: PhysicalSize<u32>) -> render_shader::Uniforms {
        camera_2d::Uniforms::source_to_screen(
            size.into(),
            // TODO: make proper source rect
            camera_2d::SourceRect {
                width: 1.0,
                height: 1.0,
            },
            // TODO: figure out where we actually want to render
            camera_2d::DestinationRect {
                x: size.width as f32 - HEADER_HEIGHT as f32,
                y: 0.0,
                width: HEADER_HEIGHT as f32,
                height: HEADER_HEIGHT as f32,
            },
            camera_2d::Mode::Fit,
        )
        .into()
    }

    pub fn prepare(&mut self, queue: &wgpu::Queue) {
        // TODO: calculate real offsets
        queue.write_buffer(
            &self.dynamic_vertex_buffer,
            0,
            bytemuck::bytes_of(&[
                render_shader::DynamicVertex {
                    offset: glam::Vec2::ZERO,
                },
                render_shader::DynamicVertex {
                    offset: glam::Vec2::ZERO,
                },
                render_shader::DynamicVertex {
                    offset: glam::Vec2::ZERO,
                },
            ]),
        );
    }

    pub fn render_pass(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_shader::set_bind_groups(render_pass, &self.render_bind_group);
        render_pass.set_vertex_buffer(0, self.static_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.dynamic_vertex_buffer.slice(..));

        render_pass.draw(0..3, 0..1);
    }
}
