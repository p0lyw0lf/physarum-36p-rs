use wgpu_text::glyph_brush::HorizontalAlign;
use wgpu_text::glyph_brush::Layout;
use wgpu_text::glyph_brush::OwnedSection;
use wgpu_text::glyph_brush::OwnedText;
use wgpu_text::glyph_brush::Section;
use winit::dpi::PhysicalSize;

use crate::constants::PLAYBACK_WIDTH;
use crate::constants::{FFT_WIDTH, HEADER_HEIGHT};
use crate::graphics::text::{COLOR_WHITE, FONT_SIZE};

pub struct Text {
    section: OwnedSection,
    /// The preset index to render
    index: usize,
    /// Whether this index is "dirty" (has changes that are not reflected in the preset)
    dirty: bool,
}

impl Text {
    pub fn new() -> Self {
        Self {
            section: Section::default()
                .with_layout(Layout::default_wrap().h_align(HorizontalAlign::Right))
                .to_owned(),
            index: 0,
            dirty: false,
        }
    }

    pub fn section(&self) -> &OwnedSection {
        &self.section
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.section.bounds = (PLAYBACK_WIDTH as f32, HEADER_HEIGHT as f32);
        self.section.screen_position = ((new_size.width - FFT_WIDTH) as f32, 0.0);
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
}
