//! This module displays everything related to audio playback. This includes the play/pause
//! indicator and a track position indicator.

use wgpu_text::glyph_brush::Layout;
use wgpu_text::glyph_brush::OwnedSection;
use wgpu_text::glyph_brush::Section;
use winit::dpi::PhysicalSize;

use crate::graphics::camera_2d;
use crate::graphics::geometry_2d::ToVertices;
use crate::graphics::geometry_2d::Triangle;
use crate::graphics::geometry_2d::VertexBuffer;
use crate::graphics::geometry_2d::make_circle;
use crate::graphics::geometry_2d::make_line;
use crate::graphics::geometry_2d::vertex_buffer_from_geometry;
use crate::graphics::text::COLOR_WHITE;
use crate::shaders::{pipelines, tris_render_shader as render_shader};

enum PlayState {
    Playing,
    Paused,
}

pub struct Pipeline {
    /// Our play/pause state
    state: PlayState,
    /// Text for the position indicator
    section: OwnedSection,

    /// Uniforms for the play/pause indicator.
    render_uniforms_buffer_play: wgpu::Buffer,
    /// Uniforms for the position indicator
    render_uniforms_buffer_position: wgpu::Buffer,

    /// Vertex buffer for the "play" variant of the play/pause indicator
    vertex_buffer_play: VertexBuffer,
    /// Vertex buffer for the "pause" variant of the play/pause indicator
    vertex_buffer_pause: VertexBuffer,
    /// Vertex buffer for the position indicator.
    vertex_buffer_position: VertexBuffer,

    /// The offsets to apply to the position geometry. It contains things type glam::Vec2, and has
    /// length 2.
    offset_buffer: wgpu::Buffer,

    /// Bind group for the play/pause indicator
    bind_group_play: render_shader::bind_groups::BindGroup0,
    /// Bind group for the position indicator.
    bind_group_position: render_shader::bind_groups::BindGroup0,
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        pipelines::initialize(device, surface_format);

        // Construct the play button
        let vertex_buffer_play = vertex_buffer_from_geometry(
            device,
            queue,
            "play vertex buffer",
            Triangle {
                p0: glam::vec2(0.0, 0.0),
                p1: glam::vec2(1.0, 0.5),
                p2: glam::vec2(0.0, 1.0),
            }
            .to_vertices(0),
        );
        // Construct the pause button
        let vertex_buffer_pause = vertex_buffer_from_geometry(
            device,
            queue,
            "pause vertex buffer",
            [
                make_line(glam::vec2(0.3, 0.0), glam::vec2(0.3, 1.0), 0.3),
                make_line(glam::vec2(0.7, 0.0), glam::vec2(0.7, 1.0), 0.3),
            ]
            .into_iter()
            .flat_map(|line| line.to_vertices((0, 0))),
        );
        // Construct the position line/seek head
        let vertex_buffer_position = vertex_buffer_from_geometry(
            device,
            queue,
            "position vertex buffer",
            make_line(glam::vec2(0.0, 3.0), glam::vec2(100.0, 3.0), 2.0)
                .to_vertices((0, 0))
                .chain(make_circle(glam::vec2(0.0, 3.0), 0.0, 3.0).to_vertices(1)),
        );

        // The pervious geometry created exactly 2 indexes that we need to fill with colors and
        // offsets.
        let color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fft color buffer"),
            size: (size_of::<glam::Vec4>() * 2) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &color_buffer,
            0,
            bytemuck::cast_slice(&[COLOR_WHITE, COLOR_WHITE]),
        );

        let offset_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fft offset buffer"),
            size: (size_of::<glam::Vec2>() * 2) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Fill in the first (constant) slot. The other slot will be filled in during each call to
        // prepare()
        queue.write_buffer(&offset_buffer, 0, bytemuck::bytes_of(&glam::Vec2::ZERO));

        let render_uniforms_buffer_play = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("play/pause render uniforms"),
            size: size_of::<render_shader::Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Filled in during each resize()

        let render_uniforms_buffer_position = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("position render uniforms"),
            size: size_of::<render_shader::Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Filled in during each resize()

        let bind_group_play = render_shader::bind_groups::BindGroup0::from_bindings(
            device,
            render_shader::bind_groups::BindGroupLayout0 {
                colors: color_buffer.as_entire_buffer_binding(),
                offsets: offset_buffer.as_entire_buffer_binding(),
                uni: render_uniforms_buffer_play.as_entire_buffer_binding(),
            },
        );

        let bind_group_position = render_shader::bind_groups::BindGroup0::from_bindings(
            device,
            render_shader::bind_groups::BindGroupLayout0 {
                colors: color_buffer.as_entire_buffer_binding(),
                offsets: offset_buffer.as_entire_buffer_binding(),
                uni: render_uniforms_buffer_position.as_entire_buffer_binding(),
            },
        );

        Self {
            // We always start out playing
            state: PlayState::Playing,
            section: Section::default()
                .with_layout(Layout::default_wrap())
                .to_owned(),
            render_uniforms_buffer_play,
            render_uniforms_buffer_position,
            vertex_buffer_play,
            vertex_buffer_pause,
            vertex_buffer_position,
            offset_buffer,
            bind_group_play,
            bind_group_position,
        }
    }

    pub fn section(&self) -> &OwnedSection {
        &self.section
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, size: PhysicalSize<u32>) {
        let play_uniforms: render_shader::Uniforms = camera_2d::Uniforms::source_to_screen(
            size.into(),
            camera_2d::SourceRect {
                width: 1.0,
                height: 1.0,
            },
            // TODO: real position
            camera_2d::DestinationRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            camera_2d::Mode::Fit,
        )
        .into();
        queue.write_buffer(
            &self.render_uniforms_buffer_play,
            0,
            bytemuck::bytes_of(&play_uniforms),
        );

        let position_uniforms: render_shader::Uniforms = camera_2d::Uniforms::source_to_screen(
            size.into(),
            camera_2d::SourceRect {
                width: 100.0,
                height: 6.0,
            },
            // TODO: real position
            camera_2d::DestinationRect {
                x: 100.0,
                y: 0.0,
                width: 100.0,
                height: 6.0,
            },
            camera_2d::Mode::Fit,
        )
        .into();
        queue.write_buffer(
            &self.render_uniforms_buffer_position,
            0,
            bytemuck::bytes_of(&position_uniforms),
        );
    }

    /// `position` is a number in the range 0-100.
    pub fn prepare(&mut self, queue: &wgpu::Queue, position: f32) {
        // TODO: figure out a better way to write this that can also set the text.
        queue.write_buffer(
            &self.offset_buffer,
            // write to second slot only
            size_of::<glam::Vec2>() as u64,
            bytemuck::bytes_of(&glam::vec2(position, 0.0)),
        );
    }

    pub fn render_pass(&self, render_pass: &mut wgpu::RenderPass) {
        pipelines::render_tris(render_pass);

        render_shader::set_bind_groups(render_pass, &self.bind_group_play);
        match self.state {
            PlayState::Playing => {
                render_pass.set_vertex_buffer(0, self.vertex_buffer_play.buffer.slice(..));
                render_pass.draw(0..self.vertex_buffer_play.num_vertices as u32, 0..1);
            }
            PlayState::Paused => {
                render_pass.set_vertex_buffer(0, self.vertex_buffer_pause.buffer.slice(..));
                render_pass.draw(0..self.vertex_buffer_pause.num_vertices as u32, 0..1);
            }
        };

        render_shader::set_bind_groups(render_pass, &self.bind_group_position);
        render_pass.set_vertex_buffer(0, self.vertex_buffer_position.buffer.slice(..));
        render_pass.draw(0..self.vertex_buffer_position.num_vertices as u32, 0..1);
    }
}
