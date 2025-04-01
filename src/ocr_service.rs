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

pub struct DummyOcrService;

impl OcrService for DummyOcrService {
    fn name(&self) -> &'static str {
        std::any::type_name_of_val(self)
    }

    fn init(&mut self) -> Result<()> {
        Ok(())
    }

    fn terminate(&mut self) -> Result<()> {
        Ok(())
    }

    fn ocr(&mut self, _image: RgbaImage) -> Result<Vec<TextBound>> {
        Ok(Vec::new())
    }
}
