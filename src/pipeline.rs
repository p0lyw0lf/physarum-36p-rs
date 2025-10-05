use crate::constants::*;
use crate::shaders::compute_shader;
use crate::shaders::render_shader;

pub struct Pipeline {
    constants_bind_group: compute_shader::bind_groups::BindGroup0,
    state_bind_group: compute_shader::bind_groups::BindGroup1,
    trail_read_bind_group: compute_shader::bind_groups::BindGroup2,
    trail_write_bind_group: compute_shader::bind_groups::BindGroup2,

    setter_pipeline: wgpu::ComputePipeline,
    move_pipeline: wgpu::ComputePipeline,
    deposit_pipeline: wgpu::ComputePipeline,
    diffusion_pipeline: wgpu::ComputePipeline,

    render_bind_group: render_shader::bind_groups::BindGroup0,
    render_pipeline: wgpu::RenderPipeline,
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let buffer = |name: &str, size: u64, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("{name}_buffer")),
                size,
                usage,
                mapped_at_creation: false,
            })
        };

        let constants_buffer = buffer(
            "constants",
            size_of::<Constants>() as u64,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        queue.write_buffer(&constants_buffer, 0, bytemuck::bytes_of(&CONSTANTS));

        let point_settings_buffer = buffer(
            "point_settings",
            size_of::<PointSettings>() as u64,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        // New point settings are written every frame
        // TODO: make them only written on-demand

        let constants_bind_group = compute_shader::bind_groups::BindGroup0::from_bindings(
            device,
            compute_shader::bind_groups::BindGroupLayout0 {
                constants: constants_buffer.as_entire_buffer_binding(),
                params: point_settings_buffer.as_entire_buffer_binding(),
            },
        );

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
        let particle_params_buffer = buffer(
            "particle_params",
            particles.len() as u64 * 2,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        queue.write_buffer(
            &particle_params_buffer,
            0,
            bytemuck::cast_slice(particles.as_slice()),
        );

        let particle_counts_buffer = buffer(
            "particle_counts",
            (SIMULATION_WIDTH * SIMULATION_HEIGHT * 4) as u64,
            wgpu::BufferUsages::STORAGE,
        );
        // The counter is re-initialized by the shader every frame

        let texture = |label: &str, format: wgpu::TextureFormat, usage: wgpu::TextureUsages| {
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
                usage,
                view_formats: &[],
            })
        };
        let fbo_texture = texture(
            "fbo",
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        fn texture_view(
            label: &str,
            texture: &wgpu::Texture,
            format: Option<wgpu::TextureFormat>,
            usage: wgpu::TextureUsages,
        ) -> wgpu::TextureView {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("{label}_texture_view")),
                format,
                dimension: Some(wgpu::TextureViewDimension::D2),
                usage: Some(usage),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            })
        }
        let fbo_texture_view = texture_view(
            "fbo",
            &fbo_texture,
            None,
            wgpu::TextureUsages::STORAGE_BINDING,
        );

        let state_bind_group = compute_shader::bind_groups::BindGroup1::from_bindings(
            device,
            compute_shader::bind_groups::BindGroupLayout1 {
                particle_params: particle_params_buffer.as_entire_buffer_binding(),
                particle_counters: particle_counts_buffer.as_entire_buffer_binding(),
                fbo_display: &fbo_texture_view,
            },
        );

        let trail_read_texture = texture(
            "trail_read",
            wgpu::TextureFormat::R32Float,
            wgpu::TextureUsages::STORAGE_BINDING,
        );
        let trail_write_texture = texture(
            "trail_write",
            wgpu::TextureFormat::R32Float,
            wgpu::TextureUsages::STORAGE_BINDING,
        );

        let trail_read_texture_view = texture_view(
            "trail_read",
            &trail_read_texture,
            None,
            wgpu::TextureUsages::STORAGE_BINDING,
        );
        let trail_write_texture_view = texture_view(
            "trail_write",
            &trail_write_texture,
            None,
            wgpu::TextureUsages::STORAGE_BINDING,
        );

        let trail_read_bind_group = compute_shader::bind_groups::BindGroup2::from_bindings(
            device,
            compute_shader::bind_groups::BindGroupLayout2 {
                trail_read: &trail_read_texture_view,
                trail_write: &trail_write_texture_view,
            },
        );
        let trail_write_bind_group = compute_shader::bind_groups::BindGroup2::from_bindings(
            device,
            compute_shader::bind_groups::BindGroupLayout2 {
                trail_read: &trail_write_texture_view,
                trail_write: &trail_read_texture_view,
            },
        );

        let setter_pipeline = compute_shader::compute::create_cs_setter_pipeline(device);
        let move_pipeline = compute_shader::compute::create_cs_move_pipeline(device);
        let deposit_pipeline = compute_shader::compute::create_cs_deposit_pipeline(device);
        let diffusion_pipeline = compute_shader::compute::create_cs_diffusion_pipeline(device);

        let render_shader_module = render_shader::create_shader_module(device);
        let render_pipeline_layout = render_shader::create_pipeline_layout(device);
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: render_shader::vertex_state(&render_shader_module, &render_shader::vs_entry()),
            fragment: Some(render_shader::fragment_state(
                &render_shader_module,
                &render_shader::fs_entry([Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })]),
            )),
            primitive: Default::default(),
            depth_stencil: Default::default(),
            multisample: Default::default(),
            multiview: Default::default(),
            cache: Default::default(),
        });

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

        let fbo_render_texture_view = texture_view(
            "fbo_render",
            &fbo_texture,
            None,
            wgpu::TextureUsages::TEXTURE_BINDING,
        );

        // Only needs to be set once, when laying out where exactly on the surface we're rendering
        // the texture.
        let render_uniforms = render_shader::Uniforms {
            scale: glam::Vec2::new(1.0, 1.0),
            offset: glam::Vec2::new(0.0, 0.0),
        };
        let render_uniforms_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render_uniforms"),
            size: size_of::<render_shader::Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &render_uniforms_buffer,
            0,
            bytemuck::bytes_of(&render_uniforms),
        );

        let render_bind_group = render_shader::bind_groups::BindGroup0::from_bindings(
            device,
            render_shader::bind_groups::BindGroupLayout0 {
                uni: render_uniforms_buffer.as_entire_buffer_binding(),
                ourSampler: &fbo_sampler,
                ourTexture: &fbo_render_texture_view,
            },
        );

        Self {
            constants_bind_group,
            trail_read_bind_group,
            trail_write_bind_group,
            state_bind_group,

            setter_pipeline,
            move_pipeline,
            deposit_pipeline,
            diffusion_pipeline,

            render_bind_group,
            render_pipeline,
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
            self.constants_bind_group.set(&mut compute_pass);
            self.state_bind_group.set(&mut compute_pass);
            self.trail_read_bind_group.set(&mut compute_pass);
            compute_pass.dispatch_workgroups(
                SIMULATION_WIDTH / SIMULATION_WORK_GROUP_SIZE,
                SIMULATION_HEIGHT / SIMULATION_WORK_GROUP_SIZE,
                1,
            );

            compute_pass.set_pipeline(&self.move_pipeline);
            // bind groups are the same
            compute_pass.dispatch_workgroups(
                (SIMULATION_NUM_PARTICLES
                    / (SIMULATION_WORK_GROUP_SIZE * SIMULATION_WORK_GROUP_SIZE) as usize)
                    as u32,
                1,
                1,
            );

            compute_pass.set_pipeline(&self.deposit_pipeline);
            // bind groups are the same
            compute_pass.dispatch_workgroups(
                SIMULATION_WIDTH / SIMULATION_WORK_GROUP_SIZE,
                SIMULATION_HEIGHT / SIMULATION_WORK_GROUP_SIZE,
                1,
            );

            compute_pass.set_pipeline(&self.diffusion_pipeline);
            self.trail_write_bind_group.set(&mut compute_pass);
            // other bind groups are the same
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
                    usage: Some(wgpu::TextureUsages::RENDER_ATTACHMENT),
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

            render_pass.set_pipeline(&self.render_pipeline);
            self.render_bind_group.set(&mut render_pass);
            render_pass.draw(0..6, 0..1);
        }

        queue.submit([encoder.finish()]);
    }
}
