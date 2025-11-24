use winit::dpi::PhysicalSize;

use crate::{
    audio::NUM_BINS,
    constants::{FFT_BIN_WIDTH, FFT_WIDTH, HEADER_HEIGHT},
    graphics::{
        Mode, camera_2d,
        geometry_2d::{
            ToVertices, VertexBuffer, make_circle, make_line, vertex_buffer_from_geometry,
        },
    },
    shaders::{pipelines, tris_render_shader as render_shader},
};

pub struct Pipeline {
    render_uniforms_buffer: wgpu::Buffer,

    // The geometry to draw. It contains things type render_shader::Vertex, and has length
    // num_vertices.
    vertex_buffer: VertexBuffer,
    // The colors to apply to the geometry. It contains things type glam::Vec4, and has length
    // NUM_BINS.
    color_buffer: wgpu::Buffer,
    // The offsets to apply to the geometry. It contains things type glam::Vec2, and has length
    // NUM_BINS.
    offset_buffer: wgpu::Buffer,

    render_bind_group: render_shader::bind_groups::BindGroup0,
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        pipelines::initialize(device, surface_format);

        // Create the base visualizer geometry
        let vertex_buffer = vertex_buffer_from_geometry(
            device,
            queue,
            "fft vertex buffer",
            (0..NUM_BINS).flat_map(|i| -> Box<dyn Iterator<Item = render_shader::Vertex>> {
                const W: f32 = FFT_BIN_WIDTH as f32;
                const H: f32 = HEADER_HEIGHT as f32;

                let i = i as u32;
                let x = i as f32;

                // add circle in this bin
                let center = glam::vec2(W * x + W / 2.0, H - 10.0);
                let circle = make_circle(center, 8.0, 10.0);
                let circle = circle.to_vertices(i);

                if i > 0 {
                    // add line from previous circle
                    let h = i - 1;
                    let prev_center = center - glam::vec2(W, 0.0);
                    let line = make_line(prev_center, center, 1.0);
                    Box::new(circle.chain(line.to_vertices((h, i))))
                } else {
                    Box::new(circle)
                }
            }),
        );

        // The previous geometry created exactly `NUM_BINS` indexes that we need to
        // fill with colors and offsets.
        let color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fft color buffer"),
            size: (size_of::<glam::Vec4>() * NUM_BINS) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // filled in during each set_mode()

        let offset_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fft offset buffer"),
            size: (size_of::<glam::Vec2>() * NUM_BINS) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // filled in during each prepare()

        let render_uniforms_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fft render uniforms"),
            size: size_of::<render_shader::Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Filled in during each resize()

        let render_bind_group = render_shader::bind_groups::BindGroup0::from_bindings(
            device,
            render_shader::bind_groups::BindGroupLayout0 {
                colors: color_buffer.as_entire_buffer_binding(),
                offsets: offset_buffer.as_entire_buffer_binding(),
                uni: render_uniforms_buffer.as_entire_buffer_binding(),
            },
        );

        Self {
            render_uniforms_buffer,
            vertex_buffer,
            color_buffer,
            offset_buffer,
            render_bind_group,
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
            camera_2d::SourceRect {
                width: FFT_WIDTH as f32,
                height: HEADER_HEIGHT as f32,
            },
            // pin to the left edge of the header
            camera_2d::DestinationRect {
                x: size.width as f32 - FFT_WIDTH as f32,
                y: 0.0,
                width: FFT_WIDTH as f32,
                height: HEADER_HEIGHT as f32,
            },
            camera_2d::Mode::Fit,
        )
        .into()
    }

    pub fn set_mode(&mut self, queue: &wgpu::Queue, mode: Mode) {
        let highlighted_index = match mode {
            Mode::Fft { index, param: _ } => Some(index.0),
            Mode::Normal | Mode::Base(_) => None,
        };
        let color_data: Vec<glam::Vec4> = (0..NUM_BINS)
            .map(|index| {
                if Some(index) == highlighted_index {
                    // red
                    glam::vec4(1.0, 0.0, 0.0, 1.0)
                } else {
                    // white
                    glam::vec4(1.0, 1.0, 1.0, 1.0)
                }
            })
            .collect();
        queue.write_buffer(&self.color_buffer, 0, bytemuck::cast_slice(&color_data[..]));
    }

    pub fn prepare(&mut self, queue: &wgpu::Queue, bins: &[f32; NUM_BINS]) {
        let offset_data: Vec<glam::Vec2> =
            bins.iter().map(|v| glam::vec2(0.0, *v * -0.2)).collect();
        queue.write_buffer(
            &self.offset_buffer,
            0,
            bytemuck::cast_slice(&offset_data[..]),
        );
    }

    pub fn render_pass(&self, render_pass: &mut wgpu::RenderPass) {
        pipelines::render_tris(render_pass);

        render_shader::set_bind_groups(render_pass, &self.render_bind_group);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.buffer.slice(..));
        render_pass.draw(0..self.vertex_buffer.num_vertices as u32, 0..1);
    }
}
