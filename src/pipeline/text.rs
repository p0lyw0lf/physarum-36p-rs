use font_kit::family_name::FamilyName;
use font_kit::handle::Handle;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;
use wgpu_text::BrushBuilder;
use wgpu_text::TextBrush;
use wgpu_text::glyph_brush::Layout;
use wgpu_text::glyph_brush::OwnedSection;
use wgpu_text::glyph_brush::OwnedText;
use wgpu_text::glyph_brush::Section;
use wgpu_text::glyph_brush::ab_glyph::FontRef;
use winit::dpi::PhysicalSize;

use crate::constants::HEADER_HEIGHT;
use crate::pipeline::Mode;
use crate::shaders::compute_shader::PointSettings;

pub struct Pipeline<'a> {
    brush: TextBrush<FontRef<'a>>,
    section: OwnedSection,
    /// What portion of the text we should highlight
    highlighted_index: Option<usize>,
}

/// We display 3 rows of text, so fill out the header completely.
const FONT_SIZE: f32 = HEADER_HEIGHT as f32 / 3.0;

const NORMAL_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0]; // white
const HIGHLIGHTED_COLOR: [f32; 4] = [0.0, 1.0, 0.0, 1.0]; // green

fn display_settings(base_settings: &PointSettings, incr_settings: &PointSettings) -> [String; 15] {
    let PointSettings {
        default_scaling_factor,
        sd_base,
        sd_exponent,
        sd_amplitude,
        sa_base,
        sa_exponent,
        sa_amplitude,
        ra_base,
        ra_exponent,
        ra_amplitude,
        md_base,
        md_exponent,
        md_amplitude,
        sensor_bias_1,
        sensor_bias_2,
    } = base_settings;
    let PointSettings {
        default_scaling_factor: default_scaling_factor_incr,
        sd_base: sd_base_incr,
        sd_exponent: sd_exponent_incr,
        sd_amplitude: sd_amplitude_incr,
        sa_base: sa_base_incr,
        sa_exponent: sa_exponent_incr,
        sa_amplitude: sa_amplitude_incr,
        ra_base: ra_base_incr,
        ra_exponent: ra_exponent_incr,
        ra_amplitude: ra_amplitude_incr,
        md_base: md_base_incr,
        md_exponent: md_exponent_incr,
        md_amplitude: md_amplitude_incr,
        sensor_bias_1: sensor_bias_1_incr,
        sensor_bias_2: sensor_bias_2_incr,
    } = incr_settings;

    const WIDTH: usize = 8;
    const PREC: usize = 3;
    [
        format!("SD0:{sd_base:>WIDTH$.PREC$}({sd_base_incr:+.PREC$})  "),
        format!("SA0:{sa_base:>WIDTH$.PREC$}({sa_base_incr:+.PREC$})  "),
        format!("RA0:{ra_base:>WIDTH$.PREC$}({ra_base_incr:+.PREC$})  "),
        format!("MD0:{md_base:>WIDTH$.PREC$}({md_base_incr:+.PREC$})  "),
        format!(
            "DSF:{default_scaling_factor:>WIDTH$.PREC$}({default_scaling_factor_incr:+.PREC$}\n"
        ),
        format!("SDA:{sd_amplitude:>WIDTH$.PREC$}({sd_amplitude_incr:+.PREC$})  "),
        format!("SAA:{sa_amplitude:>WIDTH$.PREC$}({sa_amplitude_incr:+.PREC$})  "),
        format!("RAA:{ra_amplitude:>WIDTH$.PREC$}({ra_amplitude_incr:+.PREC$})  "),
        format!("MDA:{md_amplitude:>WIDTH$.PREC$}({md_amplitude_incr:+.PREC$})  "),
        format!("SB1:{sensor_bias_1:>WIDTH$.PREC$}({sensor_bias_1_incr:+.PREC$}\n"),
        format!("SDE:{sd_exponent:>WIDTH$.PREC$}({sd_exponent_incr:+.PREC$})  "),
        format!("SAE:{sa_exponent:>WIDTH$.PREC$}({sa_exponent_incr:+.PREC$})  "),
        format!("RAE:{ra_exponent:>WIDTH$.PREC$}({ra_exponent_incr:+.PREC$})  "),
        format!("MDE:{md_exponent:>WIDTH$.PREC$}({md_exponent_incr:+.PREC$})  "),
        format!("SB2:{sensor_bias_2:>WIDTH$.PREC$}({sensor_bias_2_incr:+.PREC$}\n"),
    ]
}

/// Calculate the highlighted_index given the current mode.
fn mode_to_index(mode: Mode) -> Option<usize> {
    use crate::pipeline::ChangeParamMode::*;
    match mode {
        Mode::ChangeParam(cpm) => Some(match cpm {
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
        }),
        _ => None,
    }
}

impl Pipeline<'_> {
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        size: PhysicalSize<u32>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let font_handle = SystemSource::new()
            .select_best_match(&[FamilyName::Monospace], &Properties::new())
            .expect("Did not find system monospace font");
        let font_vec = match font_handle {
            Handle::Memory { bytes, .. } => Vec::clone(&bytes),
            Handle::Path { path, .. } => std::fs::read(path).expect("failed to read font file"),
        };
        let font_bytes: &'static mut [u8] = font_vec.leak();
        let brush_builder =
            BrushBuilder::using_font_bytes(font_bytes).expect("failed to load font");
        let brush = brush_builder.build(device, size.width, size.height, surface_format);

        let section = Section::default().with_layout(Layout::default()).to_owned();

        Self {
            brush,
            section,
            highlighted_index: None,
        }
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, new_size: PhysicalSize<u32>) {
        self.section.bounds = (new_size.width as f32, HEADER_HEIGHT as f32);
        self.section.screen_position = (0.0, 0.0);
        self.brush
            .resize_view(new_size.width as f32, new_size.height as f32, queue);
    }

    pub fn set_settings(&mut self, base_settings: &PointSettings, incr_settings: &PointSettings) {
        self.section.text.clear();
        self.section.text.extend(
            display_settings(base_settings, incr_settings)
                .into_iter()
                .enumerate()
                .map(|(i, text)| {
                    OwnedText::default()
                        .with_text(text)
                        .with_scale(FONT_SIZE)
                        .with_color(if Some(i) == self.highlighted_index {
                            HIGHLIGHTED_COLOR
                        } else {
                            NORMAL_COLOR
                        })
                }),
        );
    }

    pub fn set_mode(&mut self, mode: Mode) {
        let prev_highlighted_index = self.highlighted_index;
        self.highlighted_index = mode_to_index(mode);

        if let Some(i) = prev_highlighted_index {
            self.section.text[i] = self.section.text[i].clone().with_color(NORMAL_COLOR);
        }
        if let Some(i) = self.highlighted_index {
            self.section.text[i] = self.section.text[i].clone().with_color(HIGHLIGHTED_COLOR);
        }
    }

    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.brush
            .queue(device, queue, [&self.section])
            .expect("queuing brush");
    }

    pub fn render_pass<'pass>(&'pass self, render_pass: &mut wgpu::RenderPass<'pass>) {
        self.brush.draw(render_pass);
    }
}
