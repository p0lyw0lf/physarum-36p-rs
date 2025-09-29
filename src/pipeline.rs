use std::borrow::Cow;
use std::mem::MaybeUninit;

use bytemuck::Pod;
use bytemuck::Zeroable;

const NUM_OBJECTS: usize = 100;

// TODO: use wgsl_bindgen or wgsl_to_wgpu to automatically derive this
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct StaticProps {
    color: [f32; 4],
    offset: [f32; 2],
    _padding: [f32; 2],
}
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct DynamicProps {
    scale: [f32; 2],
}
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
}

struct CircleVertexProps {
    radius: f32,
    num_subdivisions: usize,
    inner_radius: f32,
    start_angle: f32,
    end_angle: f32,
}

impl CircleVertexProps {
    fn create(self) -> Vec<Vertex> {
        let num_vertices = self.num_subdivisions * 3 * 2;
        let mut vertex_data = vec![Vertex::zeroed(); num_vertices];

        let mut offset = 0;
        let mut add_vertex = |x: f32, y: f32| {
            vertex_data[offset] = Vertex { position: [x, y] };
            offset += 1;
        };

        // 2 triangles per subdivision
        //
        // 0--1 4
        // | / /|
        // |/ / |
        // 2 3--5
        for i in 0..self.num_subdivisions {
            let angle1 = self.start_angle
                + i as f32 * (self.end_angle - self.start_angle) / self.num_subdivisions as f32;
            let angle2 = self.start_angle
                + (i + 1) as f32 * (self.end_angle - self.start_angle)
                    / self.num_subdivisions as f32;

            let c1 = f32::cos(angle1);
            let s1 = f32::sin(angle1);
            let c2 = f32::cos(angle2);
            let s2 = f32::sin(angle2);

            // first triangle
            add_vertex(c1 * self.radius, s1 * self.radius);
            add_vertex(c2 * self.radius, s2 * self.radius);
            add_vertex(c1 * self.inner_radius, s1 * self.inner_radius);

            // second triangle
            add_vertex(c1 * self.inner_radius, s1 * self.inner_radius);
            add_vertex(c2 * self.radius, s2 * self.radius);
            add_vertex(c2 * self.inner_radius, s2 * self.inner_radius);
        }

        vertex_data
    }
}

struct ObjectInfo {
    default_scale: f32,
}

pub struct Pipeline {
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    objects: [ObjectInfo; NUM_OBJECTS],
    dynamic_storage_values: [DynamicProps; NUM_OBJECTS],
    dynamic_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    num_vertices: usize,
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
struct StaticProps {
    color: vec4f,
    offset: vec2f,
}
@group(0) @binding(0) var<storage, read> staticProps: array<StaticProps>;

struct DynamicProps {
    scale: vec2f,
}
@group(0) @binding(1) var<storage, read> dynamicProps: array<DynamicProps>;

struct Vertex {
    @location(0) position: vec2f,
}

struct VSOutput {
    @builtin(position) position: vec4f,
    @location(0) color: vec4f,
}

@vertex fn vs(
    vert: Vertex,
    @builtin(instance_index) instanceIndex: u32,
) -> VSOutput {
    var vsOut: VSOutput;
    vsOut.position = vec4f(
        vert.position * dynamicProps[instanceIndex].scale + staticProps[instanceIndex].offset, 0.0, 1.0,
    );
    vsOut.color = staticProps[instanceIndex].color;
    return vsOut;
}

@fragment fn fs(vsOut: VSOutput) -> @location(0) vec4f {
    return vsOut.color;
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
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        shader_location: 0,
                        offset: 0,
                        format: wgpu::VertexFormat::Float32x2,
                    }],
                }],
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
        let bind_group_layout = render_pipeline.get_bind_group_layout(0);

        let mut static_storage_values =
            [const { MaybeUninit::<StaticProps>::uninit() }; NUM_OBJECTS];
        let mut dynamic_storage_values =
            [const { MaybeUninit::<DynamicProps>::uninit() }; NUM_OBJECTS];

        let static_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("static buffer"),
            size: size_of_val(&static_storage_values).try_into().unwrap(),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dynamic_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dynamic buffer"),
            size: size_of_val(&dynamic_storage_values).try_into().unwrap(),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let vertex_data = CircleVertexProps {
            radius: 0.5,
            inner_radius: 0.25,
            num_subdivisions: 24,
            start_angle: 0.,
            end_angle: std::f32::consts::TAU,
        }
        .create();
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("storage buffer vertices"),
            size: size_of_val(&vertex_data[..]) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertex_data[..]));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &static_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &dynamic_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        let mut objects = [const { MaybeUninit::<ObjectInfo>::uninit() }; NUM_OBJECTS];
        for ((obj, static_storage), dynamic_storage) in objects
            .iter_mut()
            .zip(static_storage_values.iter_mut())
            .zip(dynamic_storage_values.iter_mut())
        {
            let default_scale = rand::random_range(0.2..=0.5);
            obj.write(ObjectInfo { default_scale });

            let static_props = StaticProps {
                color: [
                    rand::random_range(0.0..=1.0),
                    rand::random_range(0.0..=1.0),
                    rand::random_range(0.0..=1.0),
                    1.,
                ],
                offset: [
                    rand::random_range(-0.9..=0.9),
                    rand::random_range(-0.9..=0.9),
                ],
                _padding: Default::default(),
            };
            static_storage.write(static_props);

            let dynamic_props = DynamicProps {
                scale: [default_scale, default_scale],
            };
            dynamic_storage.write(dynamic_props);
        }

        // SAFETY: all elements have been initialized now.
        let (objects, static_storage_values, dynamic_storage_values) = unsafe {
            (
                std::mem::transmute::<
                    [MaybeUninit<ObjectInfo>; NUM_OBJECTS],
                    [ObjectInfo; NUM_OBJECTS],
                >(objects),
                std::mem::transmute::<
                    [MaybeUninit<StaticProps>; NUM_OBJECTS],
                    [StaticProps; NUM_OBJECTS],
                >(static_storage_values),
                std::mem::transmute::<
                    [MaybeUninit<DynamicProps>; NUM_OBJECTS],
                    [DynamicProps; NUM_OBJECTS],
                >(dynamic_storage_values),
            )
        };

        // These are only written to once, so might as well do it now
        queue.write_buffer(
            &static_buffer,
            0,
            bytemuck::bytes_of(&static_storage_values),
        );

        Self {
            render_pipeline,
            vertex_buffer,
            objects,
            dynamic_storage_values,
            dynamic_buffer,
            bind_group,
            num_vertices: vertex_data.len(),
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_texture: &wgpu::SurfaceTexture,
        _surface_format: wgpu::TextureFormat,
    ) {
        let width = surface_texture.texture.width() as f32;
        let height = surface_texture.texture.height() as f32;
        let aspect = width / height;

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
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

            for (obj, dynamic_storage) in self
                .objects
                .iter()
                .zip(self.dynamic_storage_values.iter_mut())
            {
                dynamic_storage.scale = [obj.default_scale / aspect, obj.default_scale];
            }

            queue.write_buffer(
                &self.dynamic_buffer,
                0,
                bytemuck::bytes_of(&self.dynamic_storage_values),
            );

            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..(self.num_vertices as u32), 0..(NUM_OBJECTS as u32));
        }

        let command_buffer = encoder.finish();
        queue.submit([command_buffer]);
    }
}
