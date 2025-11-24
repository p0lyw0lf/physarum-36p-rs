use winit::dpi::PhysicalSize;
use winit::keyboard::KeyCode;

use crate::AudioDisplay;
use crate::fs::Settings;
use crate::fs::default_settings;

mod camera_2d;
mod fft;
mod geometry_2d;
mod physarum;
mod playback;
mod preset;
mod settings;
mod text;

#[derive(Copy, Clone)]
pub enum Mode {
    Normal,
    Base(Param),
    Fft {
        /// The parameter we're currently changing, if any
        param: Option<Param>,
        /// Which FFT bin we're changing for. MUST be in the range 0..NUM_BINS
        index: BinIndex,
    },
}

pub struct Pipeline {
    mode: Mode,
    /// The settings we are currently acting on. Needs to be manually written to settings_presets.
    settings: Settings,
    /// The list of pre-made settings that we can pull from.
    settings_presets: Vec<Settings>,
    /// The setting preset we last pulled from. Can be used to go to the next/previous preset, to
    /// write to the current preset, or to insert a new preset after the given index.
    /// MUST be in the range 0..settings_presets.len()
    settings_index: usize,

    playback: playback::Pipeline,
    fft_visualizer: fft::Pipeline,
    physarum: physarum::Pipeline,

    text: text::Pipeline,
    settings_text: settings::Text,
    preset_text: preset::Text,
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: PhysicalSize<u32>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        // TODO: read from file
        let settings_presets = default_settings();
        let mut out = Self {
            mode: Mode::Normal,
            settings: settings_presets[0].clone(),
            settings_presets,
            settings_index: 0,
            playback: playback::Pipeline::new(device, queue, surface_format),
            fft_visualizer: fft::Pipeline::new(device, queue, surface_format),
            physarum: physarum::Pipeline::new(device, queue, surface_format),
            text: text::Pipeline::new(device, size, surface_format),
            settings_text: settings::Text::new(),
            preset_text: preset::Text::new(),
        };

        out.set_settings_index(0);
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

        if self.handle_preset_keypress(key) {
            return;
        }

        use Mode::*;
        match self.mode {
            Normal => {
                if let Some(param) = Param::activate(key) {
                    self.set_mode(queue, Base(param));
                    return;
                }
                if let Some(index) = BinIndex::activate(key) {
                    self.set_mode(queue, Fft { param: None, index });
                }
            }
            Base(param) => {
                if param.apply_to_base(self, key) {
                    return;
                }
                if let Some(new_param) = Param::activate(key) {
                    if new_param == param {
                        self.set_mode(queue, Normal);
                    } else {
                        self.set_mode(queue, Base(new_param));
                    }
                    return;
                }
                if let Some(index) = BinIndex::activate(key) {
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
                    && param.apply_to_fft(self, key, index)
                {
                    return;
                }
                if let Some(new_param) = Param::activate(key) {
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
                if let Some(new_index) = BinIndex::activate(key) {
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

    /// Handles all the keypresses that have to do with manipulating setting presets.
    /// Returns true if the key was handled.
    fn handle_preset_keypress(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::BracketLeft => {
                let next_index = if self.settings_index == 0 {
                    self.settings_presets.len() - 1
                } else {
                    self.settings_index - 1
                };
                self.set_settings_index(next_index);
                true
            }
            KeyCode::BracketRight => {
                let next_index = if self.settings_index == self.settings_presets.len() - 1 {
                    0
                } else {
                    self.settings_index + 1
                };
                self.set_settings_index(next_index);
                true
            }
            _ => false,
        }
    }

    fn set_settings_index(&mut self, index: usize) {
        self.settings_index = index;
        self.settings = self.settings_presets[self.settings_index].clone();
        self.preset_text.set_index(index);
        self.set_settings();
        self.preset_text.set_dirty(false);
    }

    /// Called whenever `self.settings` is updated, to notify anything that renders based on it.
    fn set_settings(&mut self) {
        // Don't need to call self.physarum.set_settings(), that is called every frame with the
        // latest settings anyways.
        self.set_text_settings();
        self.preset_text.set_dirty(true);
    }

    fn set_text_settings(&mut self) {
        let display_settings = match self.mode {
            Mode::Normal | Mode::Base(_) => &self.settings.base,
            Mode::Fft { index, param: _ } => &self.settings.fft[index.0],
        };
        self.settings_text.set_settings(display_settings);
    }

    fn set_mode(&mut self, queue: &wgpu::Queue, new_mode: Mode) {
        self.mode = new_mode;
        self.settings_text.set_mode(self.mode);
        self.set_text_settings();
        self.fft_visualizer.set_mode(queue, self.mode);
    }
}

macro_rules! param_enum {
    (pub enum $name:ident { $(
        $case:ident = $param:ident = $key:ident,
    )* }) => {
        #[derive(Copy, Clone, PartialEq, Eq)]
        pub enum $name {
            $($case,)*
        }

        impl $name {
            // Returns whether this has handled the keypress
            fn apply_to_base(&self, state: &mut Pipeline, key: KeyCode) -> bool {
                match self { $(
                    $name::$case => {
                        match key {
                            KeyCode::ArrowUp => {
                                state.settings.base.current.$param += state.settings.base.increment.$param;
                                state.set_settings();
                            }
                            KeyCode::ArrowDown => {
                                state.settings.base.current.$param -= state.settings.base.increment.$param;
                                state.set_settings();
                            }
                            KeyCode::ArrowLeft if state.settings.base.increment.$param < 100.0 => {
                                state.settings.base.increment.$param *= 10.0;
                                state.set_settings();
                            }
                            KeyCode::ArrowRight if state.settings.base.increment.$param > 0.001 => {
                                state.settings.base.increment.$param /= 10.0;
                                state.set_settings();
                            }
                            _ => return false,
                        };
                        true
                    }
                )* }
            }

            // Returns whether this has handled the keypress
            fn apply_to_fft(&self, state: &mut Pipeline, key: KeyCode, index: BinIndex) -> bool {
                let display_settings = &mut state.settings.fft[index.0];
                match self { $(
                    $name::$case => {
                        match key {
                            KeyCode::ArrowUp => {
                                display_settings.current.$param += display_settings.increment.$param;
                                state.set_settings();
                            }
                            KeyCode::ArrowDown => {
                                display_settings.current.$param -= display_settings.increment.$param;
                                state.set_settings();
                            }
                            KeyCode::ArrowLeft if display_settings.increment.$param < 100.0 => {
                                display_settings.increment.$param *= 10.0;
                                state.set_settings();
                            }
                            KeyCode::ArrowRight if display_settings.increment.$param > 0.001 => {
                                display_settings.increment.$param /= 10.0;
                                state.set_settings();
                            }
                            _ => return false,
                        };
                        true
                    }
                )* }
            }

            fn activate(key: KeyCode) -> Option<Self> {
                match key { $(
                    KeyCode::$key => Some($name::$case),
                )*
                    _ => None
                }
            }
        }
    }
}

param_enum!(
    // Use the block in the left-hand side of the keyboard, exactly corresponding to where the
    // parameters will be rendered on the screen.
    pub enum Param {
        SDBase = sd0 = KeyQ,
        SDAmplitude = sda = KeyA,
        SDExponent = sde = KeyZ,
        SABase = sa0 = KeyW,
        SAAmplitude = saa = KeyS,
        SAExponent = sae = KeyX,
        RABase = ra0 = KeyE,
        RAAmplitude = raa = KeyD,
        RAExponent = rae = KeyC,
        MDBase = md0 = KeyR,
        MDAmplitude = mda = KeyF,
        MDExponent = mde = KeyV,
        DefaultScalingFactor = dsf = KeyT,
        SensorBias1 = sb1 = KeyG,
        SensorBias2 = sb2 = KeyB,
    }
);

macro_rules! bin_indices {
    (pub struct $name:ident { $(
        $index:literal = $key:ident,
    )* }) => {
        #[derive(Copy, Clone, PartialEq, Eq)]
        pub struct $name(pub usize);

        impl $name {
            fn activate(key: KeyCode) -> Option<Self> {
                match key { $(
                    KeyCode::$key => Some(Self($index)),
                )*
                    _ => None,
                }
            }
        }
    };
}

bin_indices!(
    // Use the top row to the right of the param block, again corresponding to where the bin will be
    // displayed on the screen.
    pub struct BinIndex {
        0 = KeyY,
        1 = KeyU,
        2 = KeyI,
        3 = KeyO,
        4 = KeyP,
    }
);

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
                let mut combined_settings = self.settings.base.current.clone();
                for (bin_settings, scale) in self.settings.fft.iter().zip(data.bins.iter()) {
                    combined_settings = combined_settings + bin_settings.current.clone() * *scale;
                }
                self.physarum.set_settings(queue, &combined_settings.into());
                true
            }
            None => {
                self.physarum
                    .set_settings(queue, &self.settings.base.current.clone().into());
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
