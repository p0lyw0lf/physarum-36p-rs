use winit::dpi::PhysicalSize;

use crate::{
    constants::HEADER_HEIGHT, graphics::camera_2d, shaders::tris_render_shader as render_shader,
};

pub struct Pipeline {
    render_uniforms_buffer: wgpu::Buffer,

    render_bind_group: render_shader::bind_groups::BindGroup0,
    static_vertex_buffer: wgpu::Buffer,
    dynamic_vertex_buffer: wgpu::Buffer,
    num_vertices: usize,

    render_pipeline: wgpu::RenderPipeline,
}

const NUM_CIRCLE_SUBDIVISIONS: usize = 24;

#[repr(C)]
struct Triangle {
    p0: glam::Vec2,
    p1: glam::Vec2,
    p2: glam::Vec2,
}

const _: () = assert!(
    std::mem::size_of::<Triangle>() == 24,
    "Triangle is not packed"
);
const _: () = assert!(
    std::mem::offset_of!(Triangle, p0) == 0,
    "Triangle.p0 has wrong offset"
);
const _: () = assert!(
    std::mem::offset_of!(Triangle, p1) == 8,
    "Triangle.p1 has wrong offset"
);
const _: () = assert!(
    std::mem::offset_of!(Triangle, p2) == 16,
    "Triangle.p2 has wrong offset"
);

fn create_circle_vertices(
    center: glam::Vec2,
    inner_radius: f32,
    outer_radius: f32,
) -> impl Iterator<Item = Triangle> {
    const RADS_PER_SUBDIVISION: f32 = std::f32::consts::TAU / (NUM_CIRCLE_SUBDIVISIONS as f32);

    // 2 triangles per subdivision
    //
    // 0--1 4
    // | / /|
    // |/ / |
    // 2 3--5
    (0..NUM_CIRCLE_SUBDIVISIONS).flat_map(move |i| {
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
}

fn create_line_vertices(
    start: glam::Vec2,
    end: glam::Vec2,
    width: f32,
) -> impl Iterator<Item = Triangle> {
    /*        p0                  p1
     *         +------------------+
     *         | \__              |
     *         |    \___          |
     * start > .-  -  - \___  -  -. < end
     *         |            \___  |
     *         |                \_|
     *         +------------------+
     *        p3                 p2
     */
    let direction = end - start;
    let orthogonal = glam::vec2(direction.y, -direction.x);
    let orthonormal = orthogonal.normalize() * width;
    let offset = orthonormal / 2.0;

    let p0 = start + offset;
    let p1 = end + offset;
    let p2 = end - offset;
    let p3 = start - offset;

    [
        // first triangle
        Triangle { p0: p3, p1: p0, p2 },
        // second triangle
        Triangle {
            p0: p2,
            p1: p0,
            p2: p1,
        },
    ]
    .into_iter()
}

fn with_color(
    vertices: impl Iterator<Item = Triangle>,
    color: glam::Vec4,
) -> impl Iterator<Item = render_shader::StaticVertex> {
    let xy_to_pos = |xy: glam::Vec2| -> glam::Vec4 { glam::vec4(xy.x, xy.y, 0.0, 1.0) };
    vertices.flat_map(move |tri| {
        [
            render_shader::StaticVertex {
                base_position: xy_to_pos(tri.p0),
                color,
            },
            render_shader::StaticVertex {
                base_position: xy_to_pos(tri.p1),
                color,
            },
            render_shader::StaticVertex {
                base_position: xy_to_pos(tri.p2),
                color,
            },
        ]
    })
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

        // Fill in the buffer with dummy data, in a 100x100 box
        // TODO: make these "real"
        let circle = create_circle_vertices(glam::vec2(50.0, 50.0), 25.0, 30.0);
        let circle = with_color(circle, glam::vec4(1.0, 0.0, 0.0, 1.0)); // red
        let line1 = create_line_vertices(glam::vec2(0.0, 0.0), glam::vec2(75.0, 50.0), 2.0);
        let line1 = with_color(line1, glam::vec4(0.0, 1.0, 0.0, 1.0)); // green
        let line2 = create_line_vertices(glam::vec2(80.0, 80.0), glam::vec2(90.0, 90.0), 30.0);
        let line2 = with_color(line2, glam::vec4(0.0, 0.0, 1.0, 1.0)); // blue
        // TODO: this doesn't seem to work, only line1 is rendering for some reason
        let vertex_data: Vec<render_shader::StaticVertex> =
            circle.chain(line1).chain(line2).collect();

        let num_vertices = vertex_data.len();
        let static_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fft static vertex buffer"),
            size: (size_of::<render_shader::StaticVertex>() * num_vertices) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &static_vertex_buffer,
            0,
            bytemuck::cast_slice(&vertex_data[..]),
        );

        let dynamic_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fft dynamic vertex buffer"),
            size: (size_of::<render_shader::DynamicVertex>() * num_vertices) as u64,
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
            num_vertices,
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
                width: 100.0,
                height: 100.0,
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
        let dynamic_vertex_data: Vec<render_shader::DynamicVertex> = (0..self.num_vertices)
            .map(|_| render_shader::DynamicVertex {
                offset: glam::Vec2::ZERO,
            })
            .collect();
        // TODO: calculate real memory offsets to affect specific vertices
        queue.write_buffer(
            &self.dynamic_vertex_buffer,
            0,
            bytemuck::cast_slice(&dynamic_vertex_data[..]),
        );
    }

    pub fn render_pass(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_shader::set_bind_groups(render_pass, &self.render_bind_group);
        render_pass.set_vertex_buffer(0, self.static_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.dynamic_vertex_buffer.slice(..));

        render_pass.draw(0..self.num_vertices as u32, 0..1);
    }
}
