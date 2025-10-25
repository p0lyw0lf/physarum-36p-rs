use winit::dpi::PhysicalSize;

use crate::constants::DEFAULT_INCREMENT_SETTINGS;
use crate::constants::DEFAULT_POINT_SETTINGS;
use crate::shaders::compute_shader::PointSettings;

mod physarum;
mod text;

pub struct Pipeline {
    base_settings: PointSettings,
    incr_settings: PointSettings,

    physarum: physarum::Pipeline,
    text: text::Pipeline<'static>,
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: PhysicalSize<u32>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let mut out = Self {
            base_settings: DEFAULT_POINT_SETTINGS[0],
            incr_settings: DEFAULT_INCREMENT_SETTINGS,
            physarum: physarum::Pipeline::new(device, queue, surface_format),
            text: text::Pipeline::new(device, queue, size, surface_format),
        };

        out.set_settings(queue);

        out
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, new_size: PhysicalSize<u32>) {
        self.physarum.resize(queue, new_size);
        self.text.resize(queue, new_size);
    }

    fn set_settings(&mut self, queue: &wgpu::Queue) {
        self.physarum.set_settings(queue, &self.base_settings);
        self.text
            .set_settings(&self.base_settings, &self.incr_settings);
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_texture: &wgpu::Texture,
        surface_format: wgpu::TextureFormat,
    ) {
        self.text.prepare(device, queue);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compute_pass"),
                timestamp_writes: None,
            });

            self.physarum.compute_pass(&mut compute_pass);
        }

        let surface_texture_view = surface_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("surface_texture_view"),
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
            // Create the renderpass which will clear the screen before drawing anything
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

            self.physarum.render_pass(&mut render_pass);
            self.text.render_pass(&mut render_pass);
        }

        queue.submit([encoder.finish()]);
    }
}
