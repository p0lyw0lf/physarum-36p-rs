use winit::dpi::PhysicalSize;
use winit::keyboard::KeyCode;

use crate::constants::DEFAULT_INCREMENT_SETTINGS;
use crate::constants::DEFAULT_POINT_SETTINGS;
use crate::shaders::compute_shader::PointSettings;

mod physarum;
mod text;

enum Mode {
    Normal,
    ChangeParam(ChangeParamMode),
}

pub struct Pipeline {
    mode: Mode,
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
            mode: Mode::Normal,
            base_settings: DEFAULT_POINT_SETTINGS[0],
            incr_settings: DEFAULT_INCREMENT_SETTINGS,
            physarum: physarum::Pipeline::new(device, queue, surface_format),
            text: text::Pipeline::new(device, queue, size, surface_format),
        };

        out.set_settings(queue);

        out
    }

    fn set_settings(&mut self, queue: &wgpu::Queue) {
        self.physarum.set_settings(queue, &self.base_settings);
        self.text
            .set_settings(&self.base_settings, &self.incr_settings);
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, new_size: PhysicalSize<u32>) {
        self.physarum.resize(queue, new_size);
        self.text.resize(queue, new_size);
    }

    pub fn handle_keypress(&mut self, queue: &wgpu::Queue, key: KeyCode) {
        use ChangeParamMode::*;
        use KeyCode::*;
        use Mode::*;
        match self.mode {
            Normal => match key {
                KeyQ => self.mode = ChangeParam(SDBase),
                KeyA => self.mode = ChangeParam(SDAmplitude),
                KeyZ => self.mode = ChangeParam(SDExponent),
                KeyW => self.mode = ChangeParam(SABase),
                KeyS => self.mode = ChangeParam(SAAmplitude),
                KeyX => self.mode = ChangeParam(SAExponent),
                KeyE => self.mode = ChangeParam(RABase),
                KeyD => self.mode = ChangeParam(RAAmplitude),
                KeyC => self.mode = ChangeParam(RAExponent),
                KeyR => self.mode = ChangeParam(MDBase),
                KeyF => self.mode = ChangeParam(MDAmplitude),
                KeyV => self.mode = ChangeParam(MDExponent),
                KeyT => self.mode = ChangeParam(DefaultScalingFactor),
                KeyG => self.mode = ChangeParam(SensorBias1),
                KeyB => self.mode = ChangeParam(SensorBias2),
                _ => {}
            },
            ChangeParam(cp) => {
                if key == KeyCode::Escape {
                    self.mode = Normal;
                } else {
                    cp.apply(self, key);
                    self.set_settings(queue);
                }
            }
        }
    }
}

macro_rules! param_enum {
    (enum $name:ident { $(
        $case:ident = $param:ident,
    )* }) => {
        #[derive(Copy, Clone)]
        enum $name {
            $($case,)*
        }

        impl $name {
            fn apply(&self, state: &mut Pipeline, key: KeyCode) {
                match self { $(
                    $name::$case => match key {
                        KeyCode::ArrowUp => {
                            state.base_settings.$param += state.incr_settings.$param;
                        }
                        KeyCode::ArrowDown => {
                            state.base_settings.$param -= state.incr_settings.$param;
                        }
                        KeyCode::ArrowLeft if state.incr_settings.$param < 100.0 => {
                            state.incr_settings.$param *= 10.0;
                        }
                        KeyCode::ArrowRight if state.incr_settings.$param > 0.001 => {
                            state.incr_settings.$param /= 10.0;
                        }
                        _ => {}
                    }
                )* }
            }
        }
    }
}

param_enum!(
    enum ChangeParamMode {
        SDBase = sd_base,
        SDAmplitude = sd_amplitude,
        SDExponent = sd_exponent,
        SABase = sa_base,
        SAAmplitude = sa_amplitude,
        SAExponent = sa_exponent,
        RABase = ra_base,
        RAAmplitude = ra_amplitude,
        RAExponent = ra_exponent,
        MDBase = md_base,
        MDAmplitude = md_amplitude,
        MDExponent = md_exponent,
        DefaultScalingFactor = default_scaling_factor,
        SensorBias1 = sensor_bias_1,
        SensorBias2 = sensor_bias_2,
    }
);

impl Pipeline {
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
