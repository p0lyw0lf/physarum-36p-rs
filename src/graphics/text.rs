//! Common utilities shared among pipelines that need to render text

use std::sync::LazyLock;

use font_kit::family_name::FamilyName;
use font_kit::handle::Handle;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;
use wgpu_text::BrushBuilder;
use wgpu_text::TextBrush;
use wgpu_text::glyph_brush::Section;
use wgpu_text::glyph_brush::ab_glyph::FontRef;
use winit::dpi::PhysicalSize;

use crate::constants::HEADER_HEIGHT;

pub static MONOSPACE_FONT: LazyLock<FontRef> = LazyLock::new(|| {
    let font_handle = SystemSource::new()
        .select_best_match(&[FamilyName::Monospace], &Properties::new())
        .expect("Did not find system monospace font");
    let font_vec = match font_handle {
        Handle::Memory { bytes, .. } => Vec::clone(&bytes),
        Handle::Path { path, .. } => std::fs::read(path).expect("failed to read font file"),
    };
    FontRef::try_from_slice(font_vec.leak()).expect("invalid font")
});

/// We display 3 rows of text, so fill out the header completely.
pub const FONT_SIZE: f32 = HEADER_HEIGHT as f32 / 3.0;

pub const COLOR_WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
pub const COLOR_RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
pub const COLOR_GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
pub const COLOR_YELLOW: [f32; 4] = [1.0, 1.0, 0.0, 1.0];

/// Pipeline that helps render all the other text-including pipelines.
pub struct Pipeline {
    brush: TextBrush<FontRef<'static>>,
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        size: PhysicalSize<u32>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let brush_builder = BrushBuilder::using_font((*MONOSPACE_FONT).clone());
        let brush = brush_builder.build(device, size.width, size.height, surface_format);

        Self { brush }
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, new_size: PhysicalSize<u32>) {
        self.brush
            .resize_view(new_size.width as f32, new_size.height as f32, queue);
    }

    pub fn prepare<'a, I, S>(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, sections: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<std::borrow::Cow<'a, Section<'a>>>,
    {
        self.brush
            .queue(device, queue, sections)
            .expect("queueing brush");
    }

    pub fn render_pass<'pass>(&'pass self, render_pass: &mut wgpu::RenderPass<'pass>) {
        self.brush.draw(render_pass);
    }
}
