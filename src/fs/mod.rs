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
    let mut buf = Vec::<u8>::with_capacity(size_of::<Settings>() * settings.len());
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
