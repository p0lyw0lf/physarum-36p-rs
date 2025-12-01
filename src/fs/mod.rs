//! This module contains utilities to read & write settings data to files. How this works is we
//! define a custom file format, which is just a JSON file containing an array of our settings.

use crate::fs::settings::BinIndex;
use crate::fs::settings::DisplaySettings;
use crate::fs::settings::Param;
use crate::fs::settings::Settings;
use crate::{constants, shaders::compute_shader};

use bytemuck::Zeroable;
use winit::keyboard::KeyCode;

pub mod point_settings;
pub mod settings;

fn write_settings(mut w: impl std::io::Write, settings: &[Settings]) -> std::io::Result<()> {
    let mut buf = Vec::<u8>::with_capacity(std::mem::size_of_val(settings));
    facet_json::to_writer(&settings, &mut buf)
        .map_err(|err| std::io::Error::other(format!("{:?}", err)))?;
    w.write_all(&buf)
}

fn read_settings(mut r: impl std::io::Read) -> std::io::Result<Vec<Settings>> {
    let mut buf = Vec::<u8>::new();
    r.read_to_end(&mut buf)?;
    facet_json::from_slice(&buf).map_err(|err| std::io::Error::other(format!("{}", err)))
}

/// These are the collection of all settings that can be loaded into memory at once. Only
/// `presets` is ever persisted to disk.
pub struct AllSettings {
    /// The settings we are currently acting on. Needs to be manually written to presets.
    settings: Settings,
    /// The list of pre-made settings that we can pull from.
    presets: Vec<Settings>,
    /// The setting preset we last pulled from. Can be used to go to the next/previous preset, to
    /// write to the current preset, or to insert a new preset after the given index.
    /// MUST be in the range 0..settings_presets.len()
    index: usize,
    /// Whether `settings != presets[index]`, cached for performance.
    dirty: bool,
}

impl AllSettings {
    fn write(&self, w: impl std::io::Write) -> std::io::Result<()> {
        write_settings(w, &self.presets)
    }

    fn from_presets(presets: Vec<Settings>) -> Self {
        Self {
            settings: presets[0].clone(),
            presets,
            index: 0,
            dirty: false,
        }
    }

    fn read(r: impl std::io::Read) -> std::io::Result<Self> {
        Ok(Self::from_presets(read_settings(r)?))
    }
}

impl Default for AllSettings {
    /// Creates a default set of settings based on the Bleuje's original set.
    fn default() -> Self {
        Self::from_presets(
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
                .collect(),
        )
    }
}

impl AllSettings {
    pub fn get_settings(&self) -> &Settings {
        &self.settings
    }

    pub fn get_index(&self) -> usize {
        self.index
    }

    pub fn get_dirty(&self) -> bool {
        self.dirty
    }

    /// Handles all the keypresses that have to do with manipulating setting presets.
    /// Returns true if the key was handled.
    pub fn handle_keypress(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::BracketLeft => {
                // Go to previous preset
                let next_index = if self.index == 0 {
                    self.presets.len() - 1
                } else {
                    self.index - 1
                };
                self.set_index(next_index);
            }
            KeyCode::BracketRight => {
                // Go to next preset
                let next_index = if self.index == self.presets.len() - 1 {
                    0
                } else {
                    self.index + 1
                };
                self.set_index(next_index);
            }
            KeyCode::Enter => {
                // Save settings to current preset
                self.presets[self.index] = self.settings.clone();
                self.dirty = false;
            }
            KeyCode::F1 => {
                // Create new preset after the current one, duplicating the current settings
                self.index += 1;
                self.presets.insert(self.index, self.settings.clone());
                self.dirty = false;
            }
            KeyCode::F5 => {
                // Reset current settings to default for the preset
                self.settings = self.presets[self.index].clone();
                self.dirty = false;
            }
            KeyCode::F9 if self.presets.len() > 1 => {
                // Delete the current preset, if we can
                self.presets.remove(self.index);
                self.index = std::cmp::min(self.index, self.presets.len() - 1);
                self.set_index(self.index);
            }
            KeyCode::Slash => {
                // Randomize current settings
                self.settings = Settings::random();
                self.dirty = true;
            }
            _ => return false,
        };
        true
    }

    pub fn handle_base_keypress(&mut self, param: Param, key: KeyCode) -> bool {
        let out = param.apply(&mut self.settings.base, key);
        if out {
            self.dirty = true;
        }
        out
    }

    pub fn handle_fft_keypress(&mut self, param: Param, index: BinIndex, key: KeyCode) -> bool {
        let out = param.apply(&mut self.settings.fft[index.0], key);
        if out {
            self.dirty = true;
        }
        out
    }

    fn set_index(&mut self, index: usize) {
        self.index = index;
        self.settings = self.presets[self.index].clone();
        self.dirty = false;
    }
}
