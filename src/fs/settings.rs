use bytemuck::Zeroable;
use winit::keyboard::KeyCode;

use crate::audio::NUM_BINS;
use crate::constants;
use crate::fs::point_settings::PointSettings;
use crate::shaders::compute_shader;

/// These are the settings that are displayed at any given moment.
#[derive(Debug, Clone, facet::Facet)]
pub struct DisplaySettings {
    /// The actual settings used for calculation in the simulation.
    pub current: PointSettings,
    /// When a key is pressed, how much to increment a givens setting by.
    pub increment: PointSettings,
}

/// These are the overall settings used to calculate the exact `PointSettings` fed into the
/// simulation in a given tick.
#[derive(Debug, Clone, facet::Facet)]
pub struct Settings {
    /// The base point settings, before any scaling from FFT bins are applied.
    pub base: DisplaySettings,
    /// How much to add to each base point, scaled by the amount in each FFT bin.
    pub fft: [DisplaySettings; NUM_BINS],
}

/// Creates an entirely random set of settings. Based on my own work.
impl Settings {
    pub fn random() -> Self {
        Self {
            base: DisplaySettings {
                current: PointSettings::random_base(),
                increment: constants::DEFAULT_INCREMENT_SETTINGS.into(),
            },
            fft: std::array::repeat(DisplaySettings {
                current: compute_shader::PointSettings::zeroed().into(),
                increment: constants::DEFAULT_INCREMENT_SETTINGS.into(),
            }),
        }
    }
}

/// Uses a custom probability CDF to get a point. Tuned for "pretty good" results, often requires
/// putting paramters into out-of-range places.
pub(super) fn sample_base_setting(rng: &mut impl rand::Rng) -> f32 {
    let decision: f32 = rng.random_range(0.0..1.0);
    if decision < 0.3 {
        0.0
    } else if decision < 0.93 {
        // exponential distribution with λ = 1
        // CDF = λe^-λx => inverse CDF = -ln(c/λ)/λ
        // sample a value in range (0, 1], so we don't have to worry about log(0)
        const LAMBDA: f32 = 1.0;
        let cdf: f32 = -rng.random_range(-1.0..0.0);
        -f32::ln(cdf / LAMBDA) / LAMBDA
    } else {
        // exponential distrubiton with λ = 2, for the negative numbers
        const LAMBDA: f32 = 2.0;
        let cdf: f32 = -rng.random_range(-1.0..0.0);
        f32::ln(cdf / LAMBDA) / LAMBDA
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
            pub fn apply(&self, settings: &mut DisplaySettings, key: KeyCode) -> bool {
                match self { $(
                    $name::$case => {
                        match key {
                            KeyCode::ArrowUp => {
                                settings.current.$param += settings.increment.$param;
                            }
                            KeyCode::ArrowDown => {
                                settings.current.$param -= settings.increment.$param;
                            }
                            KeyCode::ArrowLeft if settings.increment.$param < 100.0 => {
                                settings.increment.$param *= 10.0;
                            }
                            KeyCode::ArrowRight if settings.increment.$param > 0.001 => {
                                settings.increment.$param /= 10.0;
                            }
                            _ => return false,
                        };
                        true
                    }
                )* }
            }

            pub fn activate(key: KeyCode) -> Option<Self> {
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
            pub fn activate(key: KeyCode) -> Option<Self> {
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
