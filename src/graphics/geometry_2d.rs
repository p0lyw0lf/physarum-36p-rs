use crate::shaders::tris_render_shader::Vertex;

pub trait ToVertices {
    type ShapeIndex;
    fn to_vertices(self, index: Self::ShapeIndex) -> impl Iterator<Item = Vertex>;
}

#[derive(Debug)]
pub struct Triangle {
    pub p0: glam::Vec2,
    pub p1: glam::Vec2,
    pub p2: glam::Vec2,
}

impl IntoIterator for Triangle {
    type Item = glam::Vec2;
    type IntoIter = std::array::IntoIter<Self::Item, 3>;
    fn into_iter(self) -> Self::IntoIter {
        let Triangle { p0, p1, p2 } = self;
        [p0, p1, p2].into_iter()
    }
}

impl ToVertices for Triangle {
    type ShapeIndex = u32;
    fn to_vertices(self, index: Self::ShapeIndex) -> impl Iterator<Item = Vertex> {
        self.into_iter().map(move |vertex| Vertex {
            base_position: vertex,
            color_index: index,
            offset_index: index,
        })
    }
}

const NUM_CIRCLE_SUBDIVISIONS: usize = 24;
pub struct Circle([Triangle; NUM_CIRCLE_SUBDIVISIONS * 2]);

pub fn make_circle(center: glam::Vec2, inner_radius: f32, outer_radius: f32) -> Circle {
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
    fn to_vertices(self, index: Self::ShapeIndex) -> impl Iterator<Item = Vertex> {
        self.0
            .into_iter()
            .flat_map(move |tri| tri.to_vertices(index))
    }
}

pub struct Line {
    start: Triangle,
    end: Triangle,
}

pub fn make_line(start: glam::Vec2, end: glam::Vec2, width: f32) -> Line {
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
    fn to_vertices(self, (a, b): Self::ShapeIndex) -> impl Iterator<Item = Vertex> {
        let indices = [a, a, b, b, a, b].into_iter();
        let vertices = [self.start, self.end]
            .into_iter()
            .flat_map(Triangle::into_iter);
        vertices.zip(indices).map(|(vertex, index)| Vertex {
            base_position: vertex,
            color_index: index,
            offset_index: index,
        })
    }
}

pub struct VertexBuffer {
    /// The vertices to be rendered. Contains type `[tris_render_shader::Vertex]`
    pub buffer: wgpu::Buffer,
    /// The number of vertices in the buffer.
    pub num_vertices: usize,
}

/// Helper function to construct a vertex buffer given geometry.
pub fn vertex_buffer_from_geometry(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    geometry: impl Iterator<Item = Vertex>,
) -> VertexBuffer {
    let vertex_data: Vec<Vertex> = geometry.collect();
    let num_vertices = vertex_data.len();
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (size_of::<Vertex>() * num_vertices) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&vertex_data[..]));

    VertexBuffer {
        buffer,
        num_vertices,
    }
}
