use crate::constants::*;
use crate::shaders::compute_shader;
use crate::shaders::render_shader;

pub struct Pipeline {
    point_settings_buffer: wgpu::Buffer,
    point_settings: compute_shader::PointSettings,

    constants_bind_group: compute_shader::bind_groups::BindGroup0,
    state_bind_group: compute_shader::bind_groups::BindGroup1,
    trail_read_bind_group: compute_shader::bind_groups::BindGroup2,
    trail_write_bind_group: compute_shader::bind_groups::BindGroup2,

    setter_pipeline: wgpu::ComputePipeline,
    move_pipeline: wgpu::ComputePipeline,
    deposit_pipeline: wgpu::ComputePipeline,
    diffusion_pipeline: wgpu::ComputePipeline,

    render_uniforms_buffer: wgpu::Buffer,
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
            size_of::<compute_shader::Constants>() as u64,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        queue.write_buffer(&constants_buffer, 0, bytemuck::bytes_of(&CONSTANTS));

        let point_settings_buffer = buffer(
            "point_settings",
            size_of::<compute_shader::PointSettings>() as u64,
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
                usage: Some(usage),
                dimension: Some(wgpu::TextureViewDimension::D2),
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
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
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
            scale: glam::Vec2::new(2.0, 2.0),
            offset: glam::Vec2::new(-1.0, -1.0),
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
            point_settings_buffer,
            point_settings: DEFAULT_POINT_SETTINGS[0],

            constants_bind_group,
            trail_read_bind_group,
            trail_write_bind_group,
            state_bind_group,

            setter_pipeline,
            move_pipeline,
            deposit_pipeline,
            diffusion_pipeline,

            render_uniforms_buffer,
            render_bind_group,
            render_pipeline,
        }
    }

    pub fn prepare(&mut self, queue: &wgpu::Queue, surface_texture_size: wgpu::Extent3d) {
        // TODO: only write this as needed, instead of every frame.
        queue.write_buffer(
            &self.point_settings_buffer,
            0,
            bytemuck::bytes_of(&self.point_settings),
        );

        /*
         * source: 1u = SIMULATION_WIDTH
         * destination: 1u = surface_texture_size.width / 2 * x_scale
         *
         * source: 1v = SIMULATION_HEIGHT
         * destination: 1v = surface_texture_size.width / 2 * y_scale
         *
         * Desired: 1us / 1vs = 1ud / 1vd
         * -> SIMULATION_WIDTH / SIMULATION_HEIGHT = (width * x_scale) / (height * y_scale)
         * -> (SIMULATION_WIDTH / SIMULATION_HEIGHT) / (width / height) = x_scale / y_scale
         *
         * Desired: min(x_scale, y_scale) = 2.0, so that we always scale up, never down
         */

        let source_aspect = SIMULATION_WIDTH as f32 / SIMULATION_HEIGHT as f32;
        let destination_aspect =
            surface_texture_size.width as f32 / surface_texture_size.height as f32;

        // Calculate the maximum of each of these, assuming the other is == 1.0
        let x_scale = source_aspect / destination_aspect;
        let y_scale = destination_aspect / source_aspect;

        let render_uniforms = if x_scale > y_scale {
            render_shader::Uniforms {
                scale: glam::Vec2::new(2.0 * x_scale, 2.0),
                offset: glam::Vec2::new(-x_scale, -1.0),
            }
        } else {
            render_shader::Uniforms {
                scale: glam::Vec2::new(2.0, 2.0 * y_scale),
                offset: glam::Vec2::new(-1.0, -y_scale),
            }
        };
        queue.write_buffer(
            &self.render_uniforms_buffer,
            0,
            bytemuck::bytes_of(&render_uniforms),
        );
    }

    pub fn compute_pass(&self, compute_pass: &mut wgpu::ComputePass) {
        compute_pass.set_pipeline(&self.setter_pipeline);
        self.constants_bind_group.set(compute_pass);
        self.state_bind_group.set(compute_pass);
        self.trail_read_bind_group.set(compute_pass);
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
        self.trail_write_bind_group.set(compute_pass);
        // other bind groups are the same
        compute_pass.dispatch_workgroups(
            SIMULATION_WIDTH / SIMULATION_WORK_GROUP_SIZE,
            SIMULATION_HEIGHT / SIMULATION_WORK_GROUP_SIZE,
            1,
        );
    }

    pub fn render_pass(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_pipeline(&self.render_pipeline);
        self.render_bind_group.set(render_pass);
        render_pass.draw(0..6, 0..1);
    }
}
