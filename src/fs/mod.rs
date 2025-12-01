//! This module contains utilities to read & write settings data to files. How this works is we
//! define a custom file format, which is just a JSON file containing an array of our settings.

use crate::{audio::NUM_BINS, constants, shaders::compute_shader};

mod point_settings;
use bytemuck::Zeroable;
pub use point_settings::PointSettings;

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

pub fn write_settings(mut w: impl std::io::Write, settings: &[Settings]) -> std::io::Result<()> {
    let mut buf = Vec::<u8>::with_capacity(std::mem::size_of_val(settings));
    facet_json::to_writer(&settings, &mut buf)
        .map_err(|err| std::io::Error::other(format!("{:?}", err)))?;
    w.write_all(&buf)
}

pub fn read_settings(mut r: impl std::io::Read) -> std::io::Result<Vec<Settings>> {
    let mut buf = Vec::<u8>::new();
    r.read_to_end(&mut buf)?;
    facet_json::from_slice(&buf).map_err(|err| std::io::Error::other(format!("{}", err)))
}

/// Creates a default set of settings based on the Bleuje's original set.
pub fn default_settings() -> Vec<Settings> {
    constants::DEFAULT_POINT_SETTINGS
        .iter()
        .map(|settings| Settings {
            base: DisplaySettings {
                current: (*settings).into(),
                increment: constants::DEFAULT_INCREMENT_SETTINGS.into(),
            },
            fft: std::array::repeat(DisplaySettings {
                current: compute_shader::PointSettings::zeroed().into(),
                increment: constants::DEFAULT_INCREMENT_SETTINGS.into(),
            }),
        })
        .collect()
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
fn sample_base_setting(rng: &mut impl rand::Rng) -> f32 {
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
