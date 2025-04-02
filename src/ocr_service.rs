use anyhow::Result;
use eframe::egui::{self, Rect};
use image::RgbaImage;

pub trait OcrService {
    fn name(&self) -> &'static str;

    fn init(&mut self) -> Result<()>;
    fn terminate(&mut self) -> Result<()>;
    fn config_gui(&mut self, ui: &mut egui::Ui);

    fn ocr(&mut self, image: RgbaImage) -> Result<Vec<TextBound>>;
}

pub struct TextBound {
    pub rect: Rect,
    pub text: String,
}

pub struct DummyOcrService;

impl OcrService for DummyOcrService {
    fn name(&self) -> &'static str {
        "DummyOcrService"
    }

    fn init(&mut self) -> Result<()> {
        Ok(())
    }

    fn terminate(&mut self) -> Result<()> {
        Ok(())
    }

    fn config_gui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut false, "checkbox that does nothing");
    }

    fn ocr(&mut self, _image: RgbaImage) -> Result<Vec<TextBound>> {
        Ok(Vec::new())
    }
}
