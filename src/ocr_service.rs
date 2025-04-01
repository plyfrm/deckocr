use anyhow::Result;
use eframe::egui::Rect;
use image::RgbaImage;

pub trait OcrService {
    fn name(&self) -> &'static str;

    fn init(&mut self) -> Result<()>;
    fn terminate(&mut self) -> Result<()>;

    fn ocr(&mut self, image: RgbaImage) -> Result<Vec<TextBound>>;
}

pub struct TextBound {
    pub rect: Rect,
    pub text: String,
}
