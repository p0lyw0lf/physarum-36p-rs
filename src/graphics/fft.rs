use winit::dpi::PhysicalSize;

use crate::{
    audio::NUM_BINS,
    constants::{FFT_WIDTH, HEADER_HEIGHT},
    graphics::{Mode, camera_2d},
    shaders::tris_render_shader as render_shader,
};

pub struct Pipeline {
    render_uniforms_buffer: wgpu::Buffer,

    // The geometry to draw. It contains things type render_shader::Vertex, and has length
    // num_vertices.
    vertex_buffer: wgpu::Buffer,
    num_vertices: usize,
    // The colors to apply to the geometry. It contains things type glam::Vec4, and has length
    // NUM_BINS.
    color_buffer: wgpu::Buffer,
    // The offsets to apply to the geometry. It contains things type glam::Vec2, and has length
    // NUM_BINS.
    offset_buffer: wgpu::Buffer,

    render_bind_group: render_shader::bind_groups::BindGroup0,
    render_pipeline: wgpu::RenderPipeline,
}

const NUM_CIRCLE_SUBDIVISIONS: usize = 24;

#[derive(Debug)]
struct Triangle {
    p0: glam::Vec2,
    p1: glam::Vec2,
    p2: glam::Vec2,
}

impl IntoIterator for Triangle {
    type Item = glam::Vec2;
    type IntoIter = std::array::IntoIter<Self::Item, 3>;
    fn into_iter(self) -> Self::IntoIter {
        let Triangle { p0, p1, p2 } = self;
        [p0, p1, p2].into_iter()
    }
}

trait ToVertices {
    type ShapeIndex;
    fn to_vertices(self, index: Self::ShapeIndex) -> impl Iterator<Item = render_shader::Vertex>;
}

struct Circle([Triangle; NUM_CIRCLE_SUBDIVISIONS * 2]);

fn make_circle(center: glam::Vec2, inner_radius: f32, outer_radius: f32) -> Circle {
    const RADS_PER_SUBDIVISION: f32 = std::f32::consts::TAU / (NUM_CIRCLE_SUBDIVISIONS as f32);

    // 2 triangles per subdivision
    //
    // 0--1 4 outer_radius
    // | / /|
    // |/ / |
    // 2 3--5 inner_radius
    let triangles = (0..NUM_CIRCLE_SUBDIVISIONS)
        .flat_map(move |i| {
            let i = i as f32;
            let angle0 = i * RADS_PER_SUBDIVISION;
            let angle1 = (i + 1.0) * RADS_PER_SUBDIVISION;

            let v0 = glam::vec2(f32::cos(angle0), f32::sin(angle0));
            let v1 = glam::vec2(f32::cos(angle1), f32::sin(angle1));

            [
                // first triangle
                Triangle {
                    p0: v0 * outer_radius + center,
                    p1: v1 * outer_radius + center,
                    p2: v0 * inner_radius + center,
                },
                // second triangle
                Triangle {
                    p0: v0 * inner_radius + center,
                    p1: v1 * outer_radius + center,
                    p2: v1 * inner_radius + center,
                },
            ]
        })
        .collect::<Vec<_>>();

    Circle(
        triangles
            .try_into()
            .expect("circle generated wrong number of triangles"),
    )
}

impl ToVertices for Circle {
    type ShapeIndex = u32;
    fn to_vertices(self, index: Self::ShapeIndex) -> impl Iterator<Item = render_shader::Vertex> {
        self.0
            .into_iter()
            .flat_map(Triangle::into_iter)
            .map(move |vertex| render_shader::Vertex {
                base_position: vertex,
                color_index: index,
                offset_index: index,
            })
    }
}

struct Line {
    start: Triangle,
    end: Triangle,
}

fn make_line(start: glam::Vec2, end: glam::Vec2, width: f32) -> Line {
    /*        p0                  p3
     *         +------------------+
     *         | \__              |
     *         |    \___          |
     * start > .-  -  - \___  -  -. < end
     *         |            \___  |
     *         |                \_|
     *         +------------------+
     *        p1                 p2
     */
    let direction = end - start;
    let orthogonal = glam::vec2(direction.y, -direction.x);
    let orthonormal = orthogonal.normalize() * width;
    let offset = orthonormal / 2.0;

    let p0 = start + offset;
    let p1 = start - offset;
    let p2 = end - offset;
    let p3 = end + offset;

    Line {
        start: Triangle { p0: p1, p1: p0, p2 },
        end: Triangle {
            p0: p2,
            p1: p0,
            p2: p3,
        },
    }
}

impl ToVertices for Line {
    /// (start_index, end_index)
    type ShapeIndex = (u32, u32);
    fn to_vertices(self, (a, b): Self::ShapeIndex) -> impl Iterator<Item = render_shader::Vertex> {
        let indices = [a, a, b, b, a, b].into_iter();
        let vertices = [self.start, self.end]
            .into_iter()
            .flat_map(Triangle::into_iter);
        vertices
            .zip(indices)
            .map(|(vertex, index)| render_shader::Vertex {
                base_position: vertex,
                color_index: index,
                offset_index: index,
            })
    }
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
                &render_shader::vs_entry(wgpu::VertexStepMode::Vertex),
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

        // Create the base visualizer geometry
        let mut vertex_data = Vec::<render_shader::Vertex>::new();
        for i in 0..NUM_BINS {
            const H: f32 = HEADER_HEIGHT as f32;

            let i = i as u32;
            let x = i as f32;

            // add circle in this bin
            let center = glam::vec2(H * x + H / 2.0, H - 10.0);
            let circle = make_circle(center, 8.0, 10.0);
            vertex_data.extend(circle.to_vertices(i));

            if i > 0 {
                // add line from previous circle
                let h = i - 1;
                let prev_center = center - glam::vec2(H, 0.0);
                let line = make_line(prev_center, center, 1.0);
                vertex_data.extend(line.to_vertices((h, i)));
            }
        }

        let num_vertices = vertex_data.len();
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fft static vertex buffer"),
            size: (size_of::<render_shader::Vertex>() * num_vertices) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertex_data[..]));

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
            num_vertices,
            color_buffer,
            offset_buffer,
            render_bind_group,
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
        render_pass.set_pipeline(&self.render_pipeline);
        render_shader::set_bind_groups(render_pass, &self.render_bind_group);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        render_pass.draw(0..self.num_vertices as u32, 0..1);
    }
}
