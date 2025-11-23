use wgpu_text::BrushBuilder;
use wgpu_text::TextBrush;
use wgpu_text::glyph_brush::HorizontalAlign;
use wgpu_text::glyph_brush::Layout;
use wgpu_text::glyph_brush::OwnedSection;
use wgpu_text::glyph_brush::OwnedText;
use wgpu_text::glyph_brush::Section;
use wgpu_text::glyph_brush::ab_glyph::FontRef;
use winit::dpi::PhysicalSize;

use crate::constants::{FFT_WIDTH, HEADER_HEIGHT};
use crate::graphics::text::{COLOR_WHITE, FONT_SIZE, MONOSPACE_FONT};

pub struct Pipeline {
    brush: TextBrush<FontRef<'static>>,
    section: OwnedSection,
    /// The preset index to render
    index: usize,
    /// Whether this index is "dirty" (has changes that are not reflected in the preset)
    dirty: bool,
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        size: PhysicalSize<u32>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let brush_builder = BrushBuilder::using_font((*MONOSPACE_FONT).clone());
        let brush = brush_builder.build(device, size.width, size.height, surface_format);

        let section = Section::default()
            .with_layout(Layout::default_wrap().h_align(HorizontalAlign::Right))
            .to_owned();

        Self {
            brush,
            section,
            index: 0,
            dirty: false,
        }
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, new_size: PhysicalSize<u32>) {
        let width = (new_size.width - FFT_WIDTH - 10) as f32;
        self.section.bounds = (width, HEADER_HEIGHT as f32);
        self.section.screen_position = (width, 0.0);
        self.brush
            .resize_view(new_size.width as f32, new_size.height as f32, queue);
    }

    pub fn set_index(&mut self, index: usize) {
        self.index = index;
        self.update_text();
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
        self.update_text();
    }

    fn update_text(&mut self) {
        let text = format!("{}{}", if self.dirty { "*" } else { "" }, self.index + 1);
        self.section.text.clear();
        self.section.text.push(
            OwnedText::default()
                .with_text(text)
                .with_scale(FONT_SIZE)
                .with_color(COLOR_WHITE),
        );
    }

    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.brush
            .queue(device, queue, [&self.section])
            .expect("preset: queueing brush");
    }

    pub fn render_pass<'pass>(&'pass self, render_pass: &mut wgpu::RenderPass<'pass>) {
        self.brush.draw(render_pass);
    }
}
