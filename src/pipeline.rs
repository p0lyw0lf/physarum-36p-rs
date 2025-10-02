use std::{borrow::Cow, mem::MaybeUninit};

pub struct Pipeline {
    render_pipeline: wgpu::RenderPipeline,
    bind_groups: [wgpu::BindGroup; 8],
    pub bind_group_index: usize,
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("our hardcoded shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(
                "
struct VSOutput {
    @builtin(position) position: vec4f,
    @location(0) texcoord: vec2f,
}

@vertex fn vs(
    @builtin(vertex_index) vertexIndex: u32,
) -> VSOutput {
    let pos = array(
        // 1st triangle
        vec2f(0.0, 0.0), // center
        vec2f(1.0, 0.0), // right, center
        vec2f(0.0, 1.0), // center, top

        // 2nd triangle
        vec2f(0.0, 1.0), // center, top
        vec2f(1.0, 0.0), // right, center
        vec2f(1.0, 1.0), // right, top
    );

    var vsOut: VSOutput;
    let xy = pos[vertexIndex];
    vsOut.position = vec4f(xy, 0.0, 1.0);
    vsOut.texcoord = xy;
    return vsOut;
}

@group(0) @binding(0) var ourSampler: sampler;
@group(0) @binding(1) var ourTexture: texture_2d<f32>;

@fragment fn fs(vsOut: VSOutput) -> @location(0) vec4f {
    return textureSample(ourTexture, ourSampler, vsOut.texcoord);
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
                buffers: &[],
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

        const TEXTURE_WIDTH: usize = 5;
        const TEXTURE_HEIGHT: usize = 7;

        const R: [u8; 4] = [255, 0, 0, 255]; // red
        const Y: [u8; 4] = [255, 255, 0, 255]; // yellow
        const B: [u8; 4] = [0, 0, 255, 255];

        // NOTE: gpus expect "increasing Y goes up", so we need to flip the texture vertically,
        // compared to how we'd normally lay it out in memory (where "increasing Y goes down").
        let texture_data = [
            R, R, R, R, R, // 6
            R, Y, R, R, R, // 5
            R, Y, R, R, R, // 4
            R, Y, Y, R, R, // 3
            R, Y, R, R, R, // 2
            R, Y, Y, Y, R, // 1
            B, R, R, R, R, // 0
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        debug_assert_eq!(texture_data.len(), TEXTURE_WIDTH * TEXTURE_HEIGHT * 4);

        let texture_size = wgpu::Extent3d {
            width: TEXTURE_WIDTH as u32,
            height: TEXTURE_HEIGHT as u32,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("our texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: Default::default(),
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: Default::default(),
                aspect: Default::default(),
            },
            &texture_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((TEXTURE_WIDTH * 4) as u32),
                rows_per_image: Some(TEXTURE_HEIGHT as u32),
            },
            texture_size,
        );
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("our texture view"),
            ..Default::default()
        });

        let bind_group_layout = render_pipeline.get_bind_group_layout(0);
        let mut bind_groups = [const { MaybeUninit::<wgpu::BindGroup>::uninit() }; 8];
        for (i, bind_group) in bind_groups.iter_mut().enumerate() {
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(&format!("bind group #{i}")),
                // TODO: use bitflags if this were something real
                address_mode_u: if i & 1 == 0 {
                    wgpu::AddressMode::ClampToEdge
                } else {
                    wgpu::AddressMode::Repeat
                },
                address_mode_v: if i & 2 == 0 {
                    wgpu::AddressMode::ClampToEdge
                } else {
                    wgpu::AddressMode::Repeat
                },
                mag_filter: if i & 4 == 0 {
                    wgpu::FilterMode::Nearest
                } else {
                    wgpu::FilterMode::Linear
                },
                ..Default::default()
            });
            bind_group.write(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("our bind group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                ],
            }));
        }

        // SAFETY: all bind groups have been initialized
        let bind_groups = unsafe {
            std::mem::transmute::<[MaybeUninit<wgpu::BindGroup>; 8], [wgpu::BindGroup; 8]>(
                bind_groups,
            )
        };

        Self {
            render_pipeline,
            bind_groups,
            bind_group_index: 0,
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
            pass.set_bind_group(
                0,
                &self.bind_groups[self.bind_group_index % self.bind_groups.len()],
                &[],
            );

            pass.draw(0..6, 0..1);
        }

        let command_buffer = encoder.finish();
        queue.submit([command_buffer]);
    }
}
