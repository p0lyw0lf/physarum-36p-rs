use wgpu_text::glyph_brush::Layout;
use wgpu_text::glyph_brush::OwnedSection;
use wgpu_text::glyph_brush::OwnedText;
use wgpu_text::glyph_brush::Section;
use winit::dpi::PhysicalSize;

use crate::constants::{FFT_WIDTH, HEADER_HEIGHT, PLAYBACK_WIDTH};
use crate::fs::{point_settings::PointSettings, settings::DisplaySettings, settings::Param};
use crate::graphics::Mode;
use crate::graphics::text::COLOR_GREEN;
use crate::graphics::text::COLOR_RED;
use crate::graphics::text::COLOR_WHITE;
use crate::graphics::text::COLOR_YELLOW;
use crate::graphics::text::FONT_SIZE;

pub struct Text {
    section: OwnedSection,
    /// What portion of the text we should highlight
    highlighted_index: Option<usize>,
    /// What overall "mode" we are in
    mode: TextMode,
}

#[derive(Copy, Clone)]
enum TextMode {
    Base,
    Fft,
}

impl TextMode {
    fn normal_color(&self) -> [f32; 4] {
        match self {
            Self::Base => COLOR_WHITE,
            Self::Fft => COLOR_RED,
        }
    }

    fn highlight_color(&self) -> [f32; 4] {
        match self {
            Self::Base => COLOR_GREEN,
            Self::Fft => COLOR_YELLOW,
        }
    }
}

fn format_display_settings(display_settings: &DisplaySettings) -> [String; 15] {
    let PointSettings {
        sd0,
        sde,
        sda,
        sa0,
        sae,
        saa,
        ra0,
        rae,
        raa,
        md0,
        mde,
        mda,
        dsf,
        sb1,
        sb2,
    } = &display_settings.current;
    let PointSettings {
        sd0: sd0_incr,
        sde: sde_incr,
        sda: sda_incr,
        sa0: sa0_incr,
        sae: sae_incr,
        saa: saa_incr,
        ra0: ra0_incr,
        rae: rae_incr,
        raa: raa_incr,
        md0: md0_incr,
        mde: mde_incr,
        mda: mda_incr,
        dsf: dsf_incr,
        sb1: sb1_incr,
        sb2: sb2_incr,
    } = &display_settings.increment;

    const WIDTH: usize = 8;
    const PREC: usize = 3;
    [
        format!("SD0:{sd0:>WIDTH$.PREC$}({sd0_incr:+.PREC$})  "),
        format!("SA0:{sa0:>WIDTH$.PREC$}({sa0_incr:+.PREC$})  "),
        format!("RA0:{ra0:>WIDTH$.PREC$}({ra0_incr:+.PREC$})  "),
        format!("MD0:{md0:>WIDTH$.PREC$}({md0_incr:+.PREC$})  "),
        format!("DSF:{dsf:>WIDTH$.PREC$}({dsf_incr:+.PREC$})\n"),
        format!("SDA:{sda:>WIDTH$.PREC$}({sda_incr:+.PREC$})  "),
        format!("SAA:{saa:>WIDTH$.PREC$}({saa_incr:+.PREC$})  "),
        format!("RAA:{raa:>WIDTH$.PREC$}({raa_incr:+.PREC$})  "),
        format!("MDA:{mda:>WIDTH$.PREC$}({mda_incr:+.PREC$})  "),
        format!("SB1:{sb1:>WIDTH$.PREC$}({sb1_incr:+.PREC$})\n"),
        format!("SDE:{sde:>WIDTH$.PREC$}({sde_incr:+.PREC$})  "),
        format!("SAE:{sae:>WIDTH$.PREC$}({sae_incr:+.PREC$})  "),
        format!("RAE:{rae:>WIDTH$.PREC$}({rae_incr:+.PREC$})  "),
        format!("MDE:{mde:>WIDTH$.PREC$}({mde_incr:+.PREC$})  "),
        format!("SB2:{sb2:>WIDTH$.PREC$}({sb2_incr:+.PREC$})\n"),
    ]
}

/// Calculate the highlighted_index given the current active param.
fn param_to_index(param: Param) -> usize {
    use Param::*;
    match param {
        SDBase => 0,
        SABase => 1,
        RABase => 2,
        MDBase => 3,
        DefaultScalingFactor => 4,
        SDAmplitude => 5,
        SAAmplitude => 6,
        RAAmplitude => 7,
        MDAmplitude => 8,
        SensorBias1 => 9,
        SDExponent => 10,
        SAExponent => 11,
        RAExponent => 12,
        MDExponent => 13,
        SensorBias2 => 14,
    }
}

/// Calculate the highlighted_index given the current mode.
fn mode_to_index(mode: Mode) -> Option<usize> {
    match mode {
        Mode::Base(param) => Some(param_to_index(param)),
        Mode::Fft { param, index: _ } => param.map(param_to_index),
        _ => None,
    }
}

impl Text {
    pub fn new() -> Self {
        Self {
            section: Section::default()
                .with_layout(Layout::default_wrap())
                .to_owned(),
            highlighted_index: None,
            mode: TextMode::Base,
        }
    }

    pub fn section(&self) -> &OwnedSection {
        &self.section
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.section.bounds = (
            (new_size.width - PLAYBACK_WIDTH - FFT_WIDTH) as f32,
            HEADER_HEIGHT as f32,
        );
        self.section.screen_position = (0.0, 0.0);
    }

    pub fn set_settings(&mut self, settings: &DisplaySettings) {
        let mode = self.mode;
        self.section.text.clear();
        self.section.text.extend(
            format_display_settings(settings)
                .into_iter()
                .enumerate()
                .map(|(i, text)| {
                    OwnedText::default()
                        .with_text(text)
                        .with_scale(FONT_SIZE)
                        .with_color(if Some(i) == self.highlighted_index {
                            mode.highlight_color()
                        } else {
                            mode.normal_color()
                        })
                }),
        );
    }

    pub fn set_mode(&mut self, mode: Mode) {
        let prev_highlighted_index = self.highlighted_index;
        self.highlighted_index = mode_to_index(mode);

        self.mode = mode.into();

        if let Some(i) = prev_highlighted_index {
            self.section.text[i] = self.section.text[i]
                .clone()
                .with_color(self.mode.normal_color());
        }
        if let Some(i) = self.highlighted_index {
            self.section.text[i] = self.section.text[i]
                .clone()
                .with_color(self.mode.highlight_color());
        }
    }
}

impl From<Mode> for TextMode {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Normal | Mode::Base(_) => Self::Base,
            Mode::Fft { .. } => Self::Fft,
        }
    }
}
