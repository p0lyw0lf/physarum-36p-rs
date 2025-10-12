use font_kit::family_name::FamilyName;
use font_kit::handle::Handle;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;
use wgpu_text::BrushBuilder;
use wgpu_text::TextBrush;
use wgpu_text::glyph_brush::Layout;
use wgpu_text::glyph_brush::OwnedSection;
use wgpu_text::glyph_brush::Section;
use wgpu_text::glyph_brush::Text;
use wgpu_text::glyph_brush::ab_glyph::FontRef;
use winit::dpi::PhysicalSize;

pub struct Pipeline<'a> {
    brush: TextBrush<FontRef<'a>>,
    section: OwnedSection,
}

const FONT_SIZE: f32 = 16.0;

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

        let section = Section::default()
            .add_text(
                Text::new("This is some text!")
                    .with_scale(FONT_SIZE)
                    .with_color([0.9, 0.5, 0.5, 1.0]),
            )
            .with_layout(Layout::default())
            .to_owned();

        Self { brush, section }
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, new_size: PhysicalSize<u32>) {
        self.section.bounds = (new_size.width as f32, 200.0);
        self.section.screen_position = (0.0, 0.0);
        self.brush
            .resize_view(new_size.width as f32, new_size.height as f32, queue);
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
