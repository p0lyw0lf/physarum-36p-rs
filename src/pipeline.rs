use crate::constants::*;

pub struct Pipeline {
    constants_buffer: wgpu::Buffer,
    point_settings_buffer: wgpu::Buffer,
    trail_read_texture: wgpu::Texture,
    trail_write_texture: wgpu::Texture,
    particle_params_buffer: wgpu::Buffer,
    particle_counts_buffer: wgpu::Buffer,
    fbo_texture: wgpu::Texture,

    constants_bind_group: wgpu::BindGroup,
    trail_read_bind_group: wgpu::BindGroup,
    trail_write_bind_group: wgpu::BindGroup,
    state_bind_group: wgpu::BindGroup,

    setter_pipeline: wgpu::ComputePipeline,
    move_pipeline: wgpu::ComputePipeline,
    deposit_pipeline: wgpu::ComputePipeline,
    diffusion_pipeline: wgpu::ComputePipeline,

    fbo_sampler: wgpu::Sampler,
    fbo_bind_group: wgpu::BindGroup,
    fbo_pipeline: wgpu::RenderPipeline,
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let buffer = |name: &str, size: u64| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("{name}_buffer")),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };

        let constants_buffer = buffer("constants", size_of::<Constants>() as u64);
        queue.write_buffer(&constants_buffer, 0, bytemuck::bytes_of(&CONSTANTS));

        let point_settings_buffer = buffer("point_settings", size_of::<PointSettings>() as u64);
        // New point settings are written every frame

        // Randomly initialize the particles' starting positions and headings
        let mut particles = vec![0u16; SIMULATION_NUM_PARTICLES * 4];
        fn float_as_u16(f: f32) -> u16 {
            (f.clamp(0., 1.) * 65535.).round() as u16
        }
        for (i, p) in particles.iter_mut().enumerate() {
            if i % 4 == 0 {
                *p = float_as_u16(rand::random_range(0..SIMULATION_WIDTH) as f32);
            } else if i % 4 == 1 {
                *p = float_as_u16(rand::random_range(0..SIMULATION_HEIGHT) as f32);
            } else {
                *p = float_as_u16(rand::random_range(0..u16::MAX) as f32 / u16::MAX as f32);
            }
        }
        let particle_params_buffer = buffer("particles", particles.len() as u64 * 2);
        queue.write_buffer(
            &particle_params_buffer,
            0,
            bytemuck::cast_slice(particles.as_slice()),
        );

        let particle_counts_buffer =
            buffer("counter", (SIMULATION_WIDTH * SIMULATION_HEIGHT * 4) as u64);
        // The counter is re-initialized every frame

        let texture =
            |label: &str, format: wgpu::TextureFormat, view_formats: &[wgpu::TextureFormat]| {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(&format!("{label}_texture")),
                    size: wgpu::Extent3d {
                        width: SIMULATION_WIDTH,
                        height: SIMULATION_HEIGHT,
                        depth_or_array_layers: 1,
                    },
                    format,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats,
                })
            };

        let trail_read_texture = texture("trail_read", wgpu::TextureFormat::R16Float, &[]);
        let trail_write_texture = texture("trail_write", wgpu::TextureFormat::R16Float, &[]);
        let fbo_texture = texture("fbo", wgpu::TextureFormat::Rgba8Unorm, &[]);

        // Specifying a bind group layout lets us re-use the same bind group between shaders.
        let buffer_entry = |i: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
            binding: i,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let texture_entry = |i: u32| wgpu::BindGroupLayoutEntry {
            binding: i,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let constants_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("constants_bind_group_layout"),
                entries: &[
                    // constants
                    buffer_entry(0, wgpu::BufferBindingType::Uniform),
                    // point_settings
                    buffer_entry(1, wgpu::BufferBindingType::Uniform),
                ],
            });
        let trail_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("trail_bind_group_layout"),
                entries: &[
                    // trail_read
                    texture_entry(1),
                    // trail_write
                    texture_entry(2),
                ],
            });
        let state_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("storage_bind_group_layout"),
                entries: &[
                    // particle_params
                    buffer_entry(0, wgpu::BufferBindingType::Storage { read_only: false }),
                    // particle_counts
                    buffer_entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                    // fbo_display
                    texture_entry(2),
                ],
            });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[
                &constants_bind_group_layout,
                &trail_bind_group_layout,
                &state_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        fn buffer_resource<'a>(i: u32, buffer: &'a wgpu::Buffer) -> wgpu::BindGroupEntry<'a> {
            wgpu::BindGroupEntry {
                binding: i,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer,
                    offset: 0,
                    size: None,
                }),
            }
        }

        let constants_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("constants_bind_group"),
            layout: &constants_bind_group_layout,
            entries: &[
                buffer_resource(0, &constants_buffer),
                buffer_resource(1, &point_settings_buffer),
            ],
        });
        fn texture_view(
            label: &str,
            texture: &wgpu::Texture,
            format: Option<wgpu::TextureFormat>,
        ) -> wgpu::TextureView {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("{label}_texture_view")),
                format,
                dimension: Some(wgpu::TextureViewDimension::D2),
                usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            })
        }
        fn texture_resource<'a>(
            i: u32,
            texture_view: &'a wgpu::TextureView,
        ) -> wgpu::BindGroupEntry<'a> {
            wgpu::BindGroupEntry {
                binding: i,
                resource: wgpu::BindingResource::TextureView(texture_view),
            }
        }
        let trail_read_texture_view = texture_view("trail_read", &trail_read_texture, None);
        let trail_write_texture_view = texture_view("trail_write", &trail_write_texture, None);
        let trail_read_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("trail_read_bind_group"),
            layout: &trail_bind_group_layout,
            entries: &[
                texture_resource(0, &trail_read_texture_view),
                texture_resource(1, &trail_write_texture_view),
            ],
        });
        let trail_write_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("trail_write_bind_group"),
            layout: &trail_bind_group_layout,
            entries: &[
                texture_resource(0, &trail_write_texture_view),
                texture_resource(1, &trail_read_texture_view),
            ],
        });

        let fbo_texture_view = texture_view("fbo", &fbo_texture, None);

        let state_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("state_bind_group"),
            layout: &state_bind_group_layout,
            entries: &[
                buffer_resource(0, &particle_params_buffer),
                buffer_resource(1, &particle_counts_buffer),
                texture_resource(2, &fbo_texture_view),
            ],
        });

        let compute_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("./shaders/computeshader.wgsl"));

        let compute_pipeline = |entrypoint: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(&format!("{entrypoint}_pipeline")),
                layout: Some(&pipeline_layout),
                module: &compute_shader_module,
                entry_point: Some(entrypoint),
                compilation_options: Default::default(),
                cache: None,
            })
        };

        let setter_pipeline = compute_pipeline("cs_setter");
        let move_pipeline = compute_pipeline("cs_move");
        let deposit_pipeline = compute_pipeline("cs_deposit");
        let diffusion_pipeline = compute_pipeline("cs_diffusion");

        let fbo_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("./shaders/textureshader.wgsl"));

        let fbo_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fbo_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.,
            lod_max_clamp: 32.,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });

        let fbo_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fbo_pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &fbo_shader_module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &fbo_shader_module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: Default::default(),
            multisample: Default::default(),
            multiview: Default::default(),
            cache: Default::default(),
        });

        let fbo_texture_view = texture_view("fbo", &fbo_texture, None);

        let fbo_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fbo_bind_group"),
            layout: &fbo_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&fbo_sampler),
                },
                texture_resource(1, &fbo_texture_view),
            ],
        });

        Self {
            constants_buffer,
            point_settings_buffer,
            trail_read_texture,
            trail_write_texture,
            particle_params_buffer,
            particle_counts_buffer,
            fbo_texture,

            constants_bind_group,
            trail_read_bind_group,
            trail_write_bind_group,
            state_bind_group,

            setter_pipeline,
            move_pipeline,
            deposit_pipeline,
            diffusion_pipeline,

            fbo_sampler,
            fbo_bind_group,
            fbo_pipeline,
        }
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_texture: &wgpu::SurfaceTexture,
        surface_format: wgpu::TextureFormat,
    ) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("encoder"),
        });
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compute_pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.setter_pipeline);
            compute_pass.set_bind_group(0, &self.constants_bind_group, &[]);
            compute_pass.set_bind_group(2, &self.state_bind_group, &[]);
            compute_pass.dispatch_workgroups(
                SIMULATION_WIDTH / SIMULATION_WORK_GROUP_SIZE,
                SIMULATION_HEIGHT / SIMULATION_WORK_GROUP_SIZE,
                1,
            );

            compute_pass.set_pipeline(&self.move_pipeline);
            compute_pass.set_bind_group(0, &self.constants_bind_group, &[]);
            compute_pass.set_bind_group(1, &self.trail_read_bind_group, &[]);
            compute_pass.set_bind_group(2, &self.state_bind_group, &[]);
            compute_pass.dispatch_workgroups(
                (SIMULATION_NUM_PARTICLES / (SIMULATION_WORK_GROUP_SIZE * 4) as usize) as u32,
                1,
                1,
            );

            compute_pass.set_pipeline(&self.deposit_pipeline);
            compute_pass.set_bind_group(0, &self.constants_bind_group, &[]);
            compute_pass.set_bind_group(1, &self.trail_read_bind_group, &[]);
            compute_pass.set_bind_group(2, &self.state_bind_group, &[]);
            compute_pass.dispatch_workgroups(
                SIMULATION_WIDTH / SIMULATION_WORK_GROUP_SIZE,
                SIMULATION_HEIGHT / SIMULATION_WORK_GROUP_SIZE,
                1,
            );

            compute_pass.set_pipeline(&self.diffusion_pipeline);
            compute_pass.set_bind_group(0, &self.constants_bind_group, &[]);
            compute_pass.set_bind_group(1, &self.trail_write_bind_group, &[]);
            compute_pass.set_bind_group(2, &self.state_bind_group, &[]);
            compute_pass.dispatch_workgroups(
                SIMULATION_WIDTH / SIMULATION_WORK_GROUP_SIZE,
                SIMULATION_HEIGHT / SIMULATION_WORK_GROUP_SIZE,
                1,
            );
        }

        let surface_texture_view =
            surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("fbo_texture_view"),
                    format: Some(surface_format.add_srgb_suffix()),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    usage: Some(
                        wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::RENDER_ATTACHMENT,
                    ),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: None,
                });

        {
            // Create the renderpass which will clear the screen.
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_texture_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.fbo_pipeline);
            render_pass.set_bind_group(0, &self.fbo_bind_group, &[]);
            render_pass.draw_indexed(0..6, 0, 0..1);
        }

        queue.submit([encoder.finish()]);
    }
}
