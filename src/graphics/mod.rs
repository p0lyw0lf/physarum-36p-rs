use winit::dpi::PhysicalSize;
use winit::keyboard::KeyCode;

use crate::AudioDisplay;
use crate::fs::AllSettings;
use crate::fs::settings;

mod camera_2d;
mod fft;
mod geometry_2d;
mod physarum;
mod playback;
mod preset;
#[path = "./settings.rs"]
mod settings_display;
mod text;

#[derive(Copy, Clone)]
pub enum Mode {
    Normal,
    Base(settings::Param),
    Fft {
        /// The parameter we're currently changing, if any
        param: Option<settings::Param>,
        /// Which FFT bin we're changing for. MUST be in the range 0..NUM_BINS
        index: settings::BinIndex,
    },
}

pub struct Pipeline {
    mode: Mode,

    settings: AllSettings,

    playback: playback::Pipeline,
    fft_visualizer: fft::Pipeline,
    physarum: physarum::Pipeline,

    text: text::Pipeline,
    settings_text: settings_display::Text,
    preset_text: preset::Text,
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
            // TODO: read from file
            settings: AllSettings::default(),
            playback: playback::Pipeline::new(device, queue, surface_format),
            fft_visualizer: fft::Pipeline::new(device, queue, surface_format),
            physarum: physarum::Pipeline::new(device, queue, surface_format),
            text: text::Pipeline::new(device, size, surface_format),
            settings_text: settings_display::Text::new(),
            preset_text: preset::Text::new(),
        };

        out.set_preset_text();
        out.set_mode(queue, Mode::Normal);

        out
    }

    pub fn set_playing(&mut self, playing: bool) {
        self.playback.set_playing(playing);
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, new_size: PhysicalSize<u32>) {
        self.playback.resize(queue, new_size);
        self.fft_visualizer.resize(queue, new_size);
        self.physarum.resize(queue, new_size);
        self.text.resize(queue, new_size);
        self.settings_text.resize(new_size);
        self.preset_text.resize(new_size);
    }

    pub fn handle_keypress(&mut self, queue: &wgpu::Queue, key: KeyCode) {
        if key == KeyCode::Escape {
            self.set_mode(queue, Normal);
            return;
        }

        if self.settings.handle_keypress(key) {
            self.set_settings_text();
            self.set_preset_text();
            return;
        }

        use Mode::*;
        match self.mode {
            Normal => {
                if let Some(param) = settings::Param::activate(key) {
                    self.set_mode(queue, Base(param));
                    return;
                }
                if let Some(index) = settings::BinIndex::activate(key) {
                    self.set_mode(queue, Fft { param: None, index });
                }
            }
            Base(param) => {
                if self.settings.handle_base_keypress(param, key) {
                    self.set_settings_text();
                    self.set_preset_text();
                    return;
                }
                if let Some(new_param) = settings::Param::activate(key) {
                    if new_param == param {
                        self.set_mode(queue, Normal);
                    } else {
                        self.set_mode(queue, Base(new_param));
                    }
                    return;
                }
                if let Some(index) = settings::BinIndex::activate(key) {
                    self.set_mode(
                        queue,
                        Fft {
                            param: Some(param),
                            index,
                        },
                    );
                }
            }
            Fft { param, index } => {
                if let Some(param) = param
                    && self.settings.handle_fft_keypress(param, index, key)
                {
                    self.set_settings_text();
                    self.set_preset_text();
                    return;
                }
                if let Some(new_param) = settings::Param::activate(key) {
                    if Some(new_param) == param {
                        self.set_mode(queue, Fft { param: None, index });
                    } else {
                        self.set_mode(
                            queue,
                            Fft {
                                param: Some(new_param),
                                index,
                            },
                        );
                    }
                    return;
                }
                if let Some(new_index) = settings::BinIndex::activate(key) {
                    if new_index == index {
                        self.set_mode(
                            queue,
                            match param {
                                Some(param) => Base(param),
                                None => Normal,
                            },
                        );
                    } else {
                        self.set_mode(
                            queue,
                            Fft {
                                param,
                                index: new_index,
                            },
                        );
                    }
                }
            }
        }
    }

    fn set_settings_text(&mut self) {
        let display_settings = match self.mode {
            Mode::Normal | Mode::Base(_) => &self.settings.get_settings().base,
            Mode::Fft { index, param: _ } => &self.settings.get_settings().fft[index.0],
        };
        self.settings_text.set_settings(display_settings);
    }

    fn set_preset_text(&mut self) {
        // TODO: collapse these into one call now that we always modify them together.
        self.preset_text.set_index(self.settings.get_index());
        self.preset_text.set_dirty(self.settings.get_dirty());
    }

    fn set_mode(&mut self, queue: &wgpu::Queue, new_mode: Mode) {
        self.mode = new_mode;
        self.settings_text.set_mode(self.mode);
        self.set_settings_text();
        self.fft_visualizer.set_mode(queue, self.mode);
    }
}

impl Pipeline {
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_texture: &wgpu::Texture,
        surface_format: wgpu::TextureFormat,
        data: Option<&AudioDisplay>,
    ) {
        self.text.prepare(
            device,
            queue,
            [
                self.settings_text.section(),
                self.preset_text.section(),
                self.playback.section(),
            ],
        );
        let render_fft = match data {
            Some(data) => {
                self.playback
                    .prepare(queue, data.position, data.total_duration);
                self.fft_visualizer.prepare(queue, &data.bins);
                let mut combined_settings = self.settings.get_settings().base.current.clone();
                for (bin_settings, scale) in self
                    .settings
                    .get_settings()
                    .fft
                    .iter()
                    .zip(data.bins.iter())
                {
                    combined_settings = combined_settings + bin_settings.current.clone() * *scale;
                }
                self.physarum.set_settings(queue, &combined_settings.into());
                true
            }
            None => {
                self.physarum.set_settings(
                    queue,
                    &self.settings.get_settings().base.current.clone().into(),
                );
                false
            }
        };

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
            if render_fft {
                self.playback.render_pass(&mut render_pass);
                self.fft_visualizer.render_pass(&mut render_pass);
            }
        }

        queue.submit([encoder.finish()]);
    }
}
