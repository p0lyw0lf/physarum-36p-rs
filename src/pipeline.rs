use std::borrow::Cow;
use std::mem::MaybeUninit;

use bytemuck::Pod;
use bytemuck::Zeroable;

const NUM_OBJECTS: usize = 100;

// TODO: use wgsl_bindgen or wgsl_to_wgpu to automatically derive this
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct OurStruct {
    color: [f32; 4],
    scale: [f32; 2],
    offset: [f32; 2],
}

struct ObjectInfo {
    uniform_values: OurStruct,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    default_scale: f32,
}

pub struct Pipeline {
    render_pipeline: wgpu::RenderPipeline,
    objects: [ObjectInfo; NUM_OBJECTS],
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
struct OurStruct {
    color: vec4f,
    scale: vec2f,
    offset: vec2f,
}
@group(0) @binding(0) var<uniform> ourStruct: OurStruct;

@vertex fn vs(
    @builtin(vertex_index) vertexIndex: u32,
) -> @builtin(position) vec4f {
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

    return vec4f(
        pos[vertexIndex] * ourStruct.scale + ourStruct.offset, 0.0, 1.0,
    );
}

@fragment fn fs() -> @location(0) vec4f {
    return ourStruct.color;
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

        let bind_group_layout = render_pipeline.get_bind_group_layout(0);

        let mut objects = [const { MaybeUninit::<ObjectInfo>::uninit() }; NUM_OBJECTS];
        for (i, elem) in objects.iter_mut().enumerate() {
            let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("uniform buffer for obj: {i}")),
                size: size_of::<OurStruct>().try_into().unwrap(),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let uniform_values = OurStruct {
                color: [
                    rand::random_range(0.0..=1.0),
                    rand::random_range(0.0..=1.0),
                    rand::random_range(0.0..=1.0),
                    1.,
                ],
                scale: [0.5, 0.5],
                offset: [
                    rand::random_range(-0.9..=0.9),
                    rand::random_range(-0.9..=0.9),
                ],
            };
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("bind group for obj: {i}")),
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
            });
            let default_scale = rand::random_range(0.2..=0.5);

            elem.write(ObjectInfo {
                uniform_values,
                uniform_buffer,
                bind_group,
                default_scale,
            });
        }

        Self {
            render_pipeline,
            // SAFETY: all elements are initialized in the above loop
            objects: unsafe { std::mem::transmute(objects) },
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_texture: &wgpu::SurfaceTexture,
        surface_format: wgpu::TextureFormat,
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
            for obj in self.objects.iter_mut() {
                obj.uniform_values.scale = [obj.default_scale / aspect, obj.default_scale];
                queue.write_buffer(
                    &obj.uniform_buffer,
                    0,
                    bytemuck::bytes_of(&obj.uniform_values),
                );
                pass.set_bind_group(0, &obj.bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
        }

        let command_buffer = encoder.finish();
        queue.submit([command_buffer]);
    }
}
