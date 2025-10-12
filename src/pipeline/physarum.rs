use winit::dpi::PhysicalSize;

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
            scale: glam::vec2(2.0, 2.0),
            offset: glam::vec2(-1.0, -1.0),
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

    pub fn resize(&mut self, queue: &wgpu::Queue, new_size: PhysicalSize<u32>) {
        let render_uniforms = Self::calculate_uniforms(new_size);
        queue.write_buffer(
            &self.render_uniforms_buffer,
            0,
            bytemuck::bytes_of(&render_uniforms),
        );
    }

    fn calculate_uniforms(size: PhysicalSize<u32>) -> render_shader::Uniforms {
        let x = 0f32;
        let y = HEADER_HEIGHT as f32;
        let width = size.width as f32;
        let height = size.height.saturating_sub(HEADER_HEIGHT) as f32;
        if width == 0.0 || height == 0.0 {
            return render_shader::Uniforms {
                scale: glam::Vec2::ZERO,
                offset: glam::Vec2::ZERO,
            };
        }

        /*
         * The overall transformation we want to accomplish is transforming the "source pixels" of
         * the simulation to the "destination pixels" of the screen, while preserving aspect ratio.
         * This transformation can be modeled as follows:
         *
         * $$
         * t: pxs -> pxd
         * t(pxs) = pxs * (s, s) + (o_x, o_y)
         * $$
         *
         * When preserving aspect ratio, there are two things we can do: "fit" or "fill". Both look
         * at both possible scaling ratios, $w_d / w_s$ and $h_d / h_s$, where "fit" takes the
         * minimum and "fill" takes the maximum. Here, we decide to use "fill", though all
         * following equations will work with either:
         *
         * $$
         * s = max(w_d / w_s, h_d / h_s)
         * $$
         *
         * Then, we need to set a boundary condition to find the correct offset. In our case, we'd
         * like to center the image, which can be expressed as:
         *
         * $$
         * t(w_s/2, h_s/2) = (x + w_d/2, u + h_d/2)
         * $$
         *
         * And, solving:
         *
         * $$
         * => s * w_s/2 + o_x = x + w_d/2, s * h_s / 2 + o_y = y + h_d/2
         * => o_x = x + 0.5*w_d - s*0.5*w_s, o_y = y + 0.5*h_d - s*0.5*h_s
         * $$
         */
        let source_dims = glam::vec2(SIMULATION_WIDTH as f32, SIMULATION_HEIGHT as f32);
        let destination_dims = glam::vec2(width, height);
        let destination_offset = glam::vec2(x, y);
        let direct_scale = destination_dims / source_dims;
        let overall_scale = if direct_scale.x > direct_scale.y {
            direct_scale.x
        } else {
            direct_scale.y
        };
        let overall_offset =
            destination_offset + 0.5 * (destination_dims - overall_scale * source_dims);

        /*
         * However! There are a few more transformations that happen in the interim that we have to
         * account for. The first is the mapping from the "source pixels" to the actual texture
         * UVs.
         *
         * This mapping looks something like:
         *
         * 0     w_s       0      1
         * . ---- . 0      . ---- . 0
         * | tttt |     => | tttt |
         * | t    |        | t    |
         * . ---- . h_s => . ---- . 1
         *
         * This is represented by the following transformation:
         *
         * $$
         * pxs_to_uvs: pxs -> uvs
         * pxs_to_uvs(pxs) = pxs / (w_s, h_s)
         * $$
         *
         * The next transformation turns the source uvs into the destination uvs. This is the only
         * transformation we actually control as part of the shader.
         *
         * $$
         * uvs_to_uvd: uvs -> uvd
         * uvs_to_uvd(uvs) = uvs * scale + offset
         * $$
         *
         * Finally, there's the rendering of the destination uvs to the screen. This looks
         * something like:
         *
         * -1      0      1         0            w_d
         *  . ---- . ---- . 1       . ---- . ---- . 0
         *  |      |      |         |      |      |
         *  |      |      |         |      |      |
         *  . ---- . ---- . 0   =>  . ---- . ---- .
         *  |      |      |         |      |      |
         *  |      |      |         |      |      |
         *  . ---- . ---- . -1      . ---- . ---- . h_d
         *
         *
         * $$
         * uvd_to_pxd: uvd -> pxd
         * uvd_to_pxd(uvd) => uvd * (w_d/2, -h_d/2) + (w_d/2, h_d/2)
         * $$
         *
         * So, we want to satisfy the following equation, solving for the $scale$ and $offset$
         * vectors that make up $uvs_to_uvd$:
         *
         * $$
         * t(pxs) = uvd_to_pxd(uvs_to_uvd(pxs_to_uvs(pxs)))
         * $$
         *
         * It's possible to analyze that equation, but it's a bit tedious. Instead, let's model
         * each transformation with homogenous coordinates, so it just becomes a series of matrix
         * multiplications:
         *
         * $$
         *    T * pxs = uvd_to_pxd * uvs_to_uvd * pxs_to_uvs * pxs
         * => T = uvd_to_pxd * uvs_to_uvd * pxs_to_uvs
         * => uvd_to_pxd^{-1} * T * pxs_to_uvs^{-1} = uvs_to_uvd
         * => uvs_to_uvd = [[ w_d/2,    0, w_d/2 ],
         *                  [   0, -h_d/2, h_d/2 ],
         *                  [   0,    0,     1 ]]^{-1}
         *               * [[ s, 0, o_x ],
         *                  [ 0, s, o_y ],
         *                  [ 0, 0,   1 ]]
         *               * [[ 1/w_s,     0, 0 ]
         *                  [     0, 1/h_s, 0 ]
         *                  [     0,     0, 1 ]]^{-1}
         * => uvs_to_uvd = [[ 2/w_d,      0, -1 ],
         *                  [     0, -2/h_d,  1 ],
         *                  [     0,      0,    1 ]]
         *               * [[ s, 0, o_x ],
         *                  [ 0, s, o_y ],
         *                  [ 0, 0,   1 ]]
         *               * [[ w_s,   0, 0 ]
         *                  [   0, h_s, 0 ]
         *                  [   0,   0, 1 ]]
         * => uvs_to_uvd = [[ 2*s*w_s/w_d,            0, 2*o_x/w_d - 1 ]
         *                  [           0, -2*s*h_s/h_d, 1 - 2*o_y/h_d ]
         *                  [           0,            0,             1 ]]
         *
         * $$
         */
        let flip = glam::vec2(1.0, -1.0);
        let scale = 2.0 * overall_scale * source_dims / destination_dims;
        let offset = 2.0 * overall_offset / destination_dims - 1.0;
        render_shader::Uniforms {
            scale: scale * flip,
            offset: offset * flip,
        }
    }

    pub fn prepare(&mut self, queue: &wgpu::Queue) {
        // TODO: only write this as needed, instead of every frame.
        queue.write_buffer(
            &self.point_settings_buffer,
            0,
            bytemuck::bytes_of(&self.point_settings),
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
