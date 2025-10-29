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
use crate::shaders::compute_shader::PointSettings;

pub struct Pipeline<'a> {
    brush: TextBrush<FontRef<'a>>,
    section: OwnedSection,
}

fn display_settings(base_settings: &PointSettings, incr_settings: &PointSettings) -> String {
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

    format!(
        "\
SD0:{sd_base:>width$.prec$}({sd_base_incr:+.prec$})  \
SA0:{sa_base:>width$.prec$}({sa_base_incr:+.prec$})  \
RA0:{ra_base:>width$.prec$}({ra_base_incr:+.prec$})  \
MD0:{md_base:>width$.prec$}({md_base_incr:+.prec$})  \
DSF:{default_scaling_factor:>width$.prec$}({default_scaling_factor_incr:+.prec$})
SDA:{sd_amplitude:>width$.prec$}({sd_amplitude_incr:+.prec$})  \
SAA:{sa_amplitude:>width$.prec$}({sa_amplitude_incr:+.prec$})  \
RAA:{ra_amplitude:>width$.prec$}({ra_amplitude_incr:+.prec$})  \
MDA:{md_amplitude:>width$.prec$}({md_amplitude_incr:+.prec$})  \
SB1:{sensor_bias_1:>width$.prec$}({sensor_bias_1_incr:+.prec$})
SDE:{sd_exponent:>width$.prec$}({sd_exponent_incr:+.prec$})  \
SAE:{sa_exponent:>width$.prec$}({sa_exponent_incr:+.prec$})  \
RAE:{ra_exponent:>width$.prec$}({ra_exponent_incr:+.prec$})  \
MDE:{md_exponent:>width$.prec$}({md_exponent_incr:+.prec$})  \
SB2:{sensor_bias_2:>width$.prec$}({sensor_bias_2_incr:+.prec$})
",
        width = 8,
        prec = 3
    )
}

const FONT_SIZE: f32 = 20.0;

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

        Self { brush, section }
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, new_size: PhysicalSize<u32>) {
        self.section.bounds = (new_size.width as f32, HEADER_HEIGHT as f32);
        self.section.screen_position = (0.0, 0.0);
        self.brush
            .resize_view(new_size.width as f32, new_size.height as f32, queue);
    }

    pub fn set_settings(&mut self, base_settings: &PointSettings, incr_settings: &PointSettings) {
        self.section.text.clear();
        self.section.text.push(
            OwnedText::default()
                .with_text(display_settings(base_settings, incr_settings))
                .with_scale(FONT_SIZE)
                .with_color([1.0, 1.0, 1.0, 1.0]), // white
        );
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
