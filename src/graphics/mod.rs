use bytemuck::Zeroable;
use winit::dpi::PhysicalSize;
use winit::keyboard::KeyCode;

use crate::audio::NUM_FREQUENCY_RANGES;
use crate::constants::DEFAULT_INCREMENT_SETTINGS;
use crate::constants::DEFAULT_POINT_SETTINGS;
use crate::shaders::compute_shader::PointSettings;

mod camera_2d;
mod fft_visualizer;
mod physarum;
mod text;

#[derive(Copy, Clone)]
pub enum Mode {
    Normal,
    Base(Param),
    Fft {
        /// The parameter we're currently changing, if any
        param: Option<Param>,
        /// Which FFT bin we're changing for. MUST be in the range 0..NUM_FREQUENCY_RANGES
        index: BinIndex,
    },
}

#[derive(Clone)]
struct DisplaySettings {
    /// The actual settings used for calculation in the simulation.
    current: PointSettings,
    /// When a key is pressed, how much to increment a givens setting by.
    increment: PointSettings,
}

pub struct Pipeline {
    mode: Mode,
    /// The base point settings, before any scaling from FFT bins are applied.
    base_settings: DisplaySettings,
    /// How much to add to each base point, scaled by the amount in each FFT bin.
    fft_settings: [DisplaySettings; NUM_FREQUENCY_RANGES],

    physarum: physarum::Pipeline,
    text: text::Pipeline<'static>,
    fft_visualizer: fft_visualizer::Pipeline,
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
            base_settings: DisplaySettings {
                current: DEFAULT_POINT_SETTINGS[0],
                increment: DEFAULT_INCREMENT_SETTINGS,
            },
            fft_settings: std::array::repeat(DisplaySettings {
                current: PointSettings::zeroed(),
                increment: DEFAULT_INCREMENT_SETTINGS,
            }),
            physarum: physarum::Pipeline::new(device, queue, surface_format),
            text: text::Pipeline::new(device, queue, size, surface_format),
            fft_visualizer: fft_visualizer::Pipeline::new(device, queue, surface_format),
        };

        out.set_settings(queue);
        out.set_mode(queue, Mode::Normal);

        out
    }

    fn set_text_settings(&mut self) {
        let display_settings = match self.mode {
            Mode::Normal | Mode::Base(_) => &self.base_settings,
            Mode::Fft { index, param: _ } => &self.fft_settings[index.0],
        };
        self.text
            .set_settings(&display_settings.current, &display_settings.increment);
    }

    fn set_settings(&mut self, _queue: &wgpu::Queue) {
        // Don't need to call self.physarum.set_settings(), that is called every frame with the
        // latest settings anyways.
        self.set_text_settings();
    }

    fn set_mode(&mut self, queue: &wgpu::Queue, new_mode: Mode) {
        self.mode = new_mode;
        self.text.set_mode(self.mode);
        self.set_text_settings();
        self.fft_visualizer.set_mode(queue, self.mode);
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, new_size: PhysicalSize<u32>) {
        self.physarum.resize(queue, new_size);
        self.text.resize(queue, new_size);
        self.fft_visualizer.resize(queue, new_size);
    }

    pub fn handle_keypress(&mut self, queue: &wgpu::Queue, key: KeyCode) {
        use Mode::*;
        if key == KeyCode::Escape {
            self.set_mode(queue, Normal);
            return;
        }

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
                if param.apply_to_base(self, queue, key) {
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
                    && param.apply_to_fft(self, queue, key, index)
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
            fn apply_to_base(&self, state: &mut Pipeline, queue: &wgpu::Queue, key: KeyCode) -> bool {
                match self { $(
                    $name::$case => {
                        match key {
                            KeyCode::ArrowUp => {
                                state.base_settings.current.$param += state.base_settings.increment.$param;
                                state.set_settings(queue);
                            }
                            KeyCode::ArrowDown => {
                                state.base_settings.current.$param -= state.base_settings.increment.$param;
                                state.set_settings(queue);
                            }
                            KeyCode::ArrowLeft if state.base_settings.increment.$param < 100.0 => {
                                state.base_settings.increment.$param *= 10.0;
                                state.set_settings(queue);
                            }
                            KeyCode::ArrowRight if state.base_settings.increment.$param > 0.001 => {
                                state.base_settings.increment.$param /= 10.0;
                                state.set_settings(queue);
                            }
                            _ => return false,
                        };
                        true
                    }
                )* }
            }

            // Returns whether this has handled the keypress
            fn apply_to_fft(&self, state: &mut Pipeline, queue: &wgpu::Queue, key: KeyCode, index: BinIndex) -> bool {
                let display_settings = &mut state.fft_settings[index.0];
                match self { $(
                    $name::$case => {
                        match key {
                            KeyCode::ArrowUp => {
                                display_settings.current.$param += display_settings.increment.$param;
                                state.set_settings(queue);
                            }
                            KeyCode::ArrowDown => {
                                display_settings.current.$param -= display_settings.increment.$param;
                                state.set_settings(queue);
                            }
                            KeyCode::ArrowLeft if display_settings.increment.$param < 100.0 => {
                                display_settings.increment.$param *= 10.0;
                                state.set_settings(queue);
                            }
                            KeyCode::ArrowRight if display_settings.increment.$param > 0.001 => {
                                display_settings.increment.$param /= 10.0;
                                state.set_settings(queue);
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
        SDBase = sd_base = KeyQ,
        SDAmplitude = sd_amplitude = KeyA,
        SDExponent = sd_exponent = KeyZ,
        SABase = sa_base = KeyW,
        SAAmplitude = sa_amplitude = KeyS,
        SAExponent = sa_exponent = KeyX,
        RABase = ra_base = KeyE,
        RAAmplitude = ra_amplitude = KeyD,
        RAExponent = ra_exponent = KeyC,
        MDBase = md_base = KeyR,
        MDAmplitude = md_amplitude = KeyF,
        MDExponent = md_exponent = KeyV,
        DefaultScalingFactor = default_scaling_factor = KeyT,
        SensorBias1 = sensor_bias_1 = KeyG,
        SensorBias2 = sensor_bias_2 = KeyB,
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
        5 = BracketLeft,
    }
);

impl Pipeline {
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_texture: &wgpu::Texture,
        surface_format: wgpu::TextureFormat,
        bins: Option<&[f32; NUM_FREQUENCY_RANGES]>,
    ) {
        self.text.prepare(device, queue);
        let render_fft = match bins {
            Some(bins) => {
                self.fft_visualizer.prepare(queue, bins);
                let mut combined_settings = self.base_settings.current;
                for (bin_settings, scale) in self.fft_settings.iter().zip(bins.iter()) {
                    combined_settings = combined_settings + bin_settings.current * *scale;
                }
                self.physarum.set_settings(queue, &combined_settings);
                true
            }
            None => {
                self.physarum
                    .set_settings(queue, &self.base_settings.current);
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
                self.fft_visualizer.render_pass(&mut render_pass);
            }
        }

        queue.submit([encoder.finish()]);
    }
}

impl std::ops::Add<PointSettings> for PointSettings {
    type Output = PointSettings;
    fn add(self, rhs: PointSettings) -> Self::Output {
        PointSettings {
            default_scaling_factor: self.default_scaling_factor + rhs.default_scaling_factor,
            sd_base: self.sd_base + rhs.sd_base,
            sd_exponent: self.sd_exponent + rhs.sd_exponent,
            sd_amplitude: self.sd_amplitude + rhs.sd_amplitude,
            sa_base: self.sa_base + rhs.sa_base,
            sa_exponent: self.sa_exponent + rhs.sa_exponent,
            sa_amplitude: self.sa_amplitude + rhs.sa_amplitude,
            ra_base: self.ra_base + rhs.ra_base,
            ra_exponent: self.ra_exponent + rhs.ra_exponent,
            ra_amplitude: self.ra_amplitude + rhs.ra_amplitude,
            md_base: self.md_base + rhs.md_base,
            md_exponent: self.md_exponent + rhs.md_exponent,
            md_amplitude: self.md_amplitude + rhs.md_amplitude,
            sensor_bias_1: self.sensor_bias_1 + rhs.sensor_bias_1,
            sensor_bias_2: self.sensor_bias_2 + rhs.sensor_bias_2,
        }
    }
}

impl std::ops::Mul<f32> for PointSettings {
    type Output = PointSettings;
    fn mul(self, rhs: f32) -> Self::Output {
        PointSettings {
            default_scaling_factor: self.default_scaling_factor * rhs,
            sd_base: self.sd_base * rhs,
            sd_exponent: self.sd_exponent * rhs,
            sd_amplitude: self.sd_amplitude * rhs,
            sa_base: self.sa_base * rhs,
            sa_exponent: self.sa_exponent * rhs,
            sa_amplitude: self.sa_amplitude * rhs,
            ra_base: self.ra_base * rhs,
            ra_exponent: self.ra_exponent * rhs,
            ra_amplitude: self.ra_amplitude * rhs,
            md_base: self.md_base * rhs,
            md_exponent: self.md_exponent * rhs,
            md_amplitude: self.md_amplitude * rhs,
            sensor_bias_1: self.sensor_bias_1 * rhs,
            sensor_bias_2: self.sensor_bias_2 * rhs,
        }
    }
}
